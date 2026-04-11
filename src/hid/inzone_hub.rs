use std::time::{Duration, Instant};

use anyhow::Result;

use crate::config::settings::Settings;
use crate::hid::hid_manager::{HidDeviceFilter, HidManager, OutputReportSendMethod};
use crate::models::battery_status::BatteryStatus;

pub const HUB_USAGE_PAGE: u16 = 0xFF04;
pub const HUB_USAGE: u16 = 0x0001;
pub const HUB_REPORT_ID: u8 = 0x02;
pub const HUB_REPORT_SIZE: usize = 64;
pub const HUB_BATTERY_SUBCOMMAND: u8 = 0x04;

const HUB_REQUEST_COMMAND: u8 = 0x41;
const HUB_RESPONSE_COMMAND: u8 = 0x14;
const HUB_REQUEST_CHECKSUM_BASE: u8 = 0x9B;
const HUB_REQUEST_KIND: u8 = 0x01;
const HUB_BATTERY_OFFSETS: [usize; 3] = [0x0E, 0x10, 0x12];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubBatteryReport {
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case: Option<u8>,
    pub raw: Vec<u8>,
}

impl HubBatteryReport {
    pub fn to_battery_status(&self) -> BatteryStatus {
        BatteryStatus::with_optional_levels(self.left, self.right, self.case)
    }
}

pub fn hub_device_filter(settings: &Settings) -> HidDeviceFilter {
    HidDeviceFilter {
        vendor_id: Some(settings.vendor_id),
        product_id: settings.product_id,
        interface_number: settings.interface_number,
        usage_page: Some(HUB_USAGE_PAGE),
        usage: Some(HUB_USAGE),
    }
}

pub fn query_hub_battery(
    hid: &mut HidManager,
    timeout_ms: i32,
    sequence: u8,
) -> Result<Option<HubBatteryReport>> {
    flush_hub_input(hid);

    if !send_hub_request(hid, HUB_BATTERY_SUBCOMMAND, sequence)? {
        return Ok(None);
    }

    let mut fallback = None;
    for report in read_hub_reports_for(hid, timeout_ms)? {
        if let Some(parsed) = parse_hub_battery_report_for_sequence(&report, sequence) {
            return Ok(Some(parsed));
        }

        if let Some(parsed) = parse_hub_battery_report(&report) {
            fallback = Some(parsed);
        }
    }

    Ok(fallback)
}

pub fn send_hub_request(hid: &mut HidManager, subcommand: u8, sequence: u8) -> Result<bool> {
    Ok(send_hub_request_with_method(hid, subcommand, sequence)?.is_some())
}

pub fn send_hub_request_with_method(
    hid: &mut HidManager,
    subcommand: u8,
    sequence: u8,
) -> Result<Option<OutputReportSendMethod>> {
    let request = build_hub_request(subcommand, sequence);
    hid.send_output_report_with_method(&request)
}

pub fn read_hub_reports_for(hid: &mut HidManager, timeout_ms: i32) -> Result<Vec<Vec<u8>>> {
    let timeout = Duration::from_millis(timeout_ms.max(1) as u64);
    let deadline = Instant::now() + timeout;
    let mut reports = Vec::new();

    loop {
        let now = Instant::now();
        if now >= deadline {
            return Ok(reports);
        }

        let remaining_ms = (deadline - now).as_millis().clamp(1, i32::MAX as u128) as i32;
        let Some(report) = hid.read_report(HUB_REPORT_SIZE, remaining_ms)? else {
            continue;
        };

        reports.push(report);
        if reports.len() >= 16 {
            return Ok(reports);
        }
    }
}

pub fn build_hub_request(subcommand: u8, sequence: u8) -> Vec<u8> {
    let mut request = vec![0_u8; HUB_REPORT_SIZE];
    request[0] = HUB_REPORT_ID;
    request[1] = 0x0C;
    request[2] = 0x01;
    request[3] = 0x00;
    request[4] = 0xFC;
    request[5] = 0x08;
    request[6] = 0x96;
    request[7] = 0xC3;
    request[8] = HUB_REQUEST_COMMAND;
    request[9] = subcommand;
    request[10] = HUB_REQUEST_KIND;
    request[11] = sequence;
    request[12] = 0x00;
    request[13] = hub_request_checksum(subcommand, sequence);
    request
}

pub fn parse_hub_battery_report(data: &[u8]) -> Option<HubBatteryReport> {
    if data.len() <= HUB_BATTERY_OFFSETS[2] {
        return None;
    }

    if data.first().copied()? != HUB_REPORT_ID
        || data.get(8).copied()? != HUB_RESPONSE_COMMAND
        || data.get(9).copied()? != HUB_BATTERY_SUBCOMMAND
    {
        return None;
    }

    Some(HubBatteryReport {
        left: decode_percent(data[HUB_BATTERY_OFFSETS[0]]),
        right: decode_percent(data[HUB_BATTERY_OFFSETS[1]]),
        case: decode_percent(data[HUB_BATTERY_OFFSETS[2]]),
        raw: data.to_vec(),
    })
}

pub fn parse_hub_battery_report_for_sequence(
    data: &[u8],
    sequence: u8,
) -> Option<HubBatteryReport> {
    let report = parse_hub_battery_report(data)?;
    (data.get(10).copied() == Some(0x10) && data.get(11).copied() == Some(sequence))
        .then_some(report)
}

fn hub_request_checksum(subcommand: u8, sequence: u8) -> u8 {
    HUB_REQUEST_CHECKSUM_BASE
        .wrapping_add(subcommand)
        .wrapping_add(sequence)
}

fn decode_percent(value: u8) -> Option<u8> {
    (value <= 100).then_some(value)
}

pub fn flush_hub_input(hid: &mut HidManager) {
    for _ in 0..8 {
        match hid.read_report(HUB_REPORT_SIZE, 1) {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }
}
