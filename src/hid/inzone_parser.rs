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
