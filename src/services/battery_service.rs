use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use tracing::{debug, error, info, warn};
use winit::event_loop::EventLoopProxy;

use crate::app::AppEvent;
use crate::cli::app_device_filter;
use crate::config::settings::Settings;
use crate::hid::hid_manager::HidManager;
use crate::hid::inzone_hub::{hub_device_filter, query_hub_battery};
use crate::hid::inzone_parser::{parse_feature_reports, parse_report};
use crate::models::battery_status::BatteryStatus;
use crate::services::notification_service::notify_low_battery;

const REPORT_SIZE: usize = 64;
const READ_TIMEOUT_MS: i32 = 1000;

#[derive(Debug)]
pub enum ServiceCommand {
    Shutdown,
}

pub fn spawn_battery_service(
    settings: Settings,
    proxy: EventLoopProxy<AppEvent>,
) -> mpsc::Sender<ServiceCommand> {
    let (command_tx, command_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut legacy_hid = match HidManager::new(app_device_filter(&settings)) {
            Ok(hid) => hid,
            Err(error) => {
                error!("unable to start hid manager: {error}");
                return;
            }
        };
        let mut hub_hid = match HidManager::new(hub_device_filter(&settings)) {
            Ok(hid) => Some(hid),
            Err(error) => {
                warn!("unable to start hub-compatible hid manager: {error}");
                None
            }
        };

        let poll_interval = Duration::from_millis(settings.poll_interval_ms);
        let mut previous_status = BatteryStatus::disconnected();
        let mut left_low_notified = false;
        let mut right_low_notified = false;
        let mut hub_sequence = 1_u8;

        loop {
            if matches!(command_rx.try_recv(), Ok(ServiceCommand::Shutdown)) {
                info!("battery service shutting down");
                break;
            }

            let next_status =
                match poll_status(hub_hid.as_mut(), &mut legacy_hid, &settings, hub_sequence) {
                    Ok(status) => status,
                    Err(error) => {
                        warn!("hid poll failed: {error}");
                        BatteryStatus::disconnected()
                    }
                };
            hub_sequence = next_hub_sequence(hub_sequence);

            if next_status != previous_status {
                if next_status.connected && next_status.known {
                    info!(
                        "battery updated left={} right={} case={}",
                        BatteryStatus::format_level(next_status.left),
                        BatteryStatus::format_level(next_status.right),
                        BatteryStatus::format_level(next_status.case)
                    );
                } else if next_status.connected {
                    info!("device connected but battery values are not decoded yet");
                } else {
                    info!("device disconnected");
                }

                let _ = proxy.send_event(AppEvent::BatteryUpdated(next_status.clone()));
                previous_status = next_status.clone();
            }

            update_low_battery_notification(
                next_status.left,
                "Left",
                settings.low_battery_threshold,
                &mut left_low_notified,
            );
            update_low_battery_notification(
                next_status.right,
                "Right",
                settings.low_battery_threshold,
                &mut right_low_notified,
            );

            thread::sleep(poll_interval);
        }
    });

    command_tx
}

fn poll_status(
    hub_hid: Option<&mut HidManager>,
    legacy_hid: &mut HidManager,
    settings: &Settings,
    hub_sequence: u8,
) -> anyhow::Result<BatteryStatus> {
    if let Some(hid) = hub_hid {
        match query_hub_battery(hid, READ_TIMEOUT_MS, hub_sequence) {
            Ok(Some(report)) => {
                debug!("read hub-compatible battery report: {:02X?}", report.raw);
                return Ok(report.to_battery_status());
            }
            Ok(None) => {
                debug!("hub-compatible battery query returned no data");
                return Ok(BatteryStatus::unknown_connected());
            }
            Err(error) => {
                debug!("hub-compatible battery query failed: {error}");
                return Ok(BatteryStatus::unknown_connected());
            }
        }
    }

    if !settings.feature_report_ids.is_empty() {
        let mut reports = Vec::new();
        for &report_id in &settings.feature_report_ids {
            match legacy_hid.get_feature_report(report_id, settings.feature_report_size)? {
                Some(report) => {
                    debug!(
                        "read feature report {:02X} with {} bytes",
                        report_id,
                        report.len()
                    );
                    reports.push((report_id, report));
                }
                None => {
                    debug!("feature report {:02X} returned no data", report_id);
                }
            }
        }

        if !reports.is_empty() {
            return Ok(parse_feature_reports(&reports));
        }
    }

    match legacy_hid.read_report(REPORT_SIZE, READ_TIMEOUT_MS)? {
        Some(report) => {
            debug!("read input report with {} bytes", report.len());
            Ok(parse_report(&report))
        }
        None => Ok(BatteryStatus::disconnected()),
    }
}

/// Drive the per-earbud low battery notification with hysteresis.
///
/// Each earbud is tracked independently: it notifies once when it drops to or
/// below the threshold, and only re-arms after it recovers above the threshold.
/// An unavailable value (`None`, e.g. in case or disconnected) leaves the state
/// untouched so a brief dropout does not cause a repeat notification.
fn update_low_battery_notification(
    level: Option<u8>,
    side: &str,
    threshold: u8,
    notified: &mut bool,
) {
    match level {
        Some(percent) if percent <= threshold => {
            if !*notified {
                if let Err(error) = notify_low_battery(side, percent) {
                    warn!("failed to show low {side} battery notification: {error}");
                }
                *notified = true;
            }
        }
        Some(_) => *notified = false,
        None => {}
    }
}

fn next_hub_sequence(sequence: u8) -> u8 {
    match sequence.wrapping_add(1) {
        0 => 1,
        next => next,
    }
}
