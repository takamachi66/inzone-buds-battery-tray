use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub low_battery_threshold: u8,
    pub poll_interval_ms: u64,
    #[serde(default = "default_vendor_id")]
    pub vendor_id: u16,
    #[serde(default = "default_product_id")]
    pub product_id: Option<u16>,
    #[serde(default)]
    pub interface_number: Option<i32>,
    #[serde(default)]
    pub usage_page: Option<u16>,
    #[serde(default)]
    pub usage: Option<u16>,
    #[serde(default = "default_feature_report_ids")]
    pub feature_report_ids: Vec<u8>,
    #[serde(default = "default_feature_report_size")]
    pub feature_report_size: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            low_battery_threshold: 20,
            poll_interval_ms: 60_000,
            vendor_id: default_vendor_id(),
            product_id: default_product_id(),
            interface_number: default_interface_number(),
            usage_page: default_usage_page(),
            usage: default_usage(),
            feature_report_ids: default_feature_report_ids(),
            feature_report_size: default_feature_report_size(),
        }
    }
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::path();
        if !path.exists() {
            let settings = Self::default();
            settings.save()?;
            return Ok(settings);
        }

        let raw = fs::read_to_string(&path)?;
        Ok(serde_json::from_str::<Self>(&raw)?)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn path() -> PathBuf {
        Path::new("config").join("settings.json")
    }
}

const fn default_vendor_id() -> u16 {
    0x054c
}

const fn default_product_id() -> Option<u16> {
    Some(0x0ec2)
}

const fn default_interface_number() -> Option<i32> {
    Some(5)
}

const fn default_usage_page() -> Option<u16> {
    Some(0xff03)
}

const fn default_usage() -> Option<u16> {
    Some(0x0020)
}

fn default_feature_report_ids() -> Vec<u8> {
    vec![0xA0, 0xA1]
}

const fn default_feature_report_size() -> usize {
    256
}
