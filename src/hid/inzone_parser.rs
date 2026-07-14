use crate::models::battery_status::BatteryStatus;

pub fn parse_report(data: &[u8]) -> BatteryStatus {
    let left = data.get(5).copied().unwrap_or(0).min(100);
    let right = data.get(6).copied().unwrap_or(0).min(100);
    let case = data.get(7).copied().unwrap_or(0).min(100);

    BatteryStatus::with_levels(left, right, case)
}

pub fn parse_feature_reports(reports: &[(u8, Vec<u8>)]) -> BatteryStatus {
    for (_, report) in reports {
        if let Some(status) = parse_battery_triplet(report) {
            return status;
        }
    }

    BatteryStatus::unknown_connected()
}

fn parse_battery_triplet(data: &[u8]) -> Option<BatteryStatus> {
    data.windows(3).find_map(|window| {
        let [left, right, case] = [window[0], window[1], window[2]];
        if left <= 100 && right <= 100 && case <= 100 && plausibly_battery_values(left, right, case)
        {
            Some(BatteryStatus::with_levels(left, right, case))
        } else {
            None
        }
    })
}

fn plausibly_battery_values(left: u8, right: u8, case: u8) -> bool {
    let all_zero = left == 0 && right == 0 && case == 0;
    let all_same = left == right && right == case;
    let at_least_one_non_zero = left > 0 || right > 0 || case > 0;
    at_least_one_non_zero && !all_zero && (!all_same || left == 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_report_reads_left_right_case_offsets() {
        let mut data = vec![0_u8; 8];
        data[5] = 85;
        data[6] = 82;
        data[7] = 100;
        let status = parse_report(&data);
        assert_eq!(status.left, Some(85));
        assert_eq!(status.right, Some(82));
        assert_eq!(status.case, Some(100));
        assert!(status.connected);
    }

    #[test]
    fn parse_report_clamps_values_above_100() {
        let mut data = vec![0_u8; 8];
        data[5] = 200;
        data[6] = 150;
        data[7] = 101;
        let status = parse_report(&data);
        assert_eq!(status.left, Some(100));
        assert_eq!(status.right, Some(100));
        assert_eq!(status.case, Some(100));
    }

    #[test]
    fn feature_reports_decode_first_plausible_triplet() {
        // The heuristic scans 3-byte windows left to right and returns the
        // first plausible battery triplet, so lead with the real values.
        let reports = vec![(0xA0_u8, vec![85, 82, 100])];
        let status = parse_feature_reports(&reports);
        assert_eq!(status.left, Some(85));
        assert_eq!(status.right, Some(82));
        assert_eq!(status.case, Some(100));
    }

    #[test]
    fn feature_reports_without_plausible_values_are_unknown() {
        let reports = vec![(0xA0_u8, vec![0x00, 0x00, 0x00, 0x00])];
        let status = parse_feature_reports(&reports);
        assert!(status.connected);
        assert!(!status.known);
    }
}
