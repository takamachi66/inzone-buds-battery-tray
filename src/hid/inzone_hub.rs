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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a battery response report as documented in `docs/protocol.md`:
    /// `02 12 04 FF 0F 00 96 C3 14 04 10 SS 00 00 LL 00 RR FF CC`.
    fn battery_response(sequence: u8, left: u8, right: u8, case: u8) -> Vec<u8> {
        let mut data = vec![0_u8; HUB_REPORT_SIZE];
        data[0] = HUB_REPORT_ID;
        data[8] = HUB_RESPONSE_COMMAND;
        data[9] = HUB_BATTERY_SUBCOMMAND;
        data[10] = 0x10;
        data[11] = sequence;
        data[HUB_BATTERY_OFFSETS[0]] = left;
        data[HUB_BATTERY_OFFSETS[1]] = right;
        data[HUB_BATTERY_OFFSETS[2]] = case;
        data
    }

    #[test]
    fn parses_both_earbuds_out() {
        let report = parse_hub_battery_report(&battery_response(1, 100, 100, 100)).unwrap();
        assert_eq!(report.left, Some(100));
        assert_eq!(report.right, Some(100));
        assert_eq!(report.case, Some(100));
    }

    #[test]
    fn decodes_0xff_as_unavailable() {
        // Left earbud only: the right earbud reports 0xFF (in case / disconnected).
        let report = parse_hub_battery_report(&battery_response(1, 100, 0xFF, 100)).unwrap();
        assert_eq!(report.left, Some(100));
        assert_eq!(report.right, None);
        assert_eq!(report.case, Some(100));
    }

    #[test]
    fn rejects_report_with_wrong_command() {
        let mut data = battery_response(1, 100, 100, 100);
        data[8] = 0x00;
        assert!(parse_hub_battery_report(&data).is_none());
    }

    #[test]
    fn rejects_report_that_is_too_short() {
        assert!(parse_hub_battery_report(&[HUB_REPORT_ID, 0x12, 0x04]).is_none());
    }

    #[test]
    fn matches_report_only_for_expected_sequence() {
        let data = battery_response(7, 50, 60, 70);
        assert!(parse_hub_battery_report_for_sequence(&data, 7).is_some());
        assert!(parse_hub_battery_report_for_sequence(&data, 8).is_none());
    }

    #[test]
    fn builds_request_matching_protocol_example() {
        // From docs/protocol.md, sequence 1: 02 0C 01 00 FC 08 96 C3 41 04 01 01 00 A0
        let request = build_hub_request(HUB_BATTERY_SUBCOMMAND, 1);
        assert_eq!(
            &request[..14],
            &[0x02, 0x0C, 0x01, 0x00, 0xFC, 0x08, 0x96, 0xC3, 0x41, 0x04, 0x01, 0x01, 0x00, 0xA0,]
        );
    }
}
