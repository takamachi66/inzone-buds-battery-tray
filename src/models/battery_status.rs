use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatteryStatus {
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case: Option<u8>,
    pub connected: bool,
    pub known: bool,
}

impl BatteryStatus {
    pub fn disconnected() -> Self {
        Self {
            left: None,
            right: None,
            case: None,
            connected: false,
            known: false,
        }
    }

    pub fn unknown_connected() -> Self {
        Self {
            left: None,
            right: None,
            case: None,
            connected: true,
            known: false,
        }
    }

    pub fn with_levels(left: u8, right: u8, case: u8) -> Self {
        Self::with_optional_levels(Some(left), Some(right), Some(case))
    }

    pub fn with_optional_levels(left: Option<u8>, right: Option<u8>, case: Option<u8>) -> Self {
        Self {
            left,
            right,
            case,
            connected: true,
            known: left.is_some() || right.is_some() || case.is_some(),
        }
    }

    pub fn min_percent(&self) -> Option<u8> {
        [self.left, self.right, self.case]
            .into_iter()
            .flatten()
            .min()
    }

    pub fn has_displayable_values(&self) -> bool {
        self.connected && self.known
    }

    pub fn summary_lines(&self) -> [String; 4] {
        if self.has_displayable_values() {
            [
                "Status: Connected".to_string(),
                format!("Left: {}", format_percent(self.left)),
                format!("Right: {}", format_percent(self.right)),
                format!("Case: {}", format_percent(self.case)),
            ]
        } else {
            [
                "Status: Disconnected".to_string(),
                "Left: --".to_string(),
                "Right: --".to_string(),
                "Case: --".to_string(),
            ]
        }
    }

    pub fn format_level(value: Option<u8>) -> String {
        format_percent(value)
    }
}

fn format_percent(value: Option<u8>) -> String {
    value
        .map(|percent| format!("{percent}%"))
        .unwrap_or_else(|| "?".to_string())
}
