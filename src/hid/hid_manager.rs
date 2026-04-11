use anyhow::{anyhow, Context, Result};
use hidapi::{DeviceInfo, HidApi, HidDevice};

#[derive(Debug, Clone, Default)]
pub struct HidDeviceFilter {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub interface_number: Option<i32>,
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct HidDeviceSnapshot {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface_number: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub manufacturer_string: Option<String>,
    pub product_string: Option<String>,
    pub serial_number: Option<String>,
}

pub struct HidManager {
    api: HidApi,
    device: Option<HidDevice>,
    filter: HidDeviceFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputReportSendMethod {
    SetOutputReport,
    WriteFile,
}

impl HidManager {
    pub fn new(filter: HidDeviceFilter) -> Result<Self> {
        let api = HidApi::new().context("failed to initialize hidapi")?;
        Ok(Self {
            api,
            device: None,
            filter,
        })
    }

    pub fn refresh_device_list(&mut self) -> Result<()> {
        self.api
            .refresh_devices()
            .context("failed to refresh hid device list")
    }

    pub fn connect(&mut self) -> Result<bool> {
        self.refresh_device_list()?;

        for device_info in self.api.device_list() {
            if !device_matches(device_info, &self.filter) {
                continue;
            }

            if let Ok(device) = device_info.open_device(&self.api) {
                self.device = Some(device);
                return Ok(true);
            }
        }

        self.device = None;
        Ok(false)
    }

    pub fn ensure_connected(&mut self) -> Result<bool> {
        if self.device.is_some() {
            return Ok(true);
        }
        self.connect()
    }

    pub fn read_report(&mut self, report_size: usize, timeout_ms: i32) -> Result<Option<Vec<u8>>> {
        if !self.ensure_connected()? {
            return Ok(None);
        }

        let mut buffer = vec![0_u8; report_size];
        let result = {
            let device = self.device.as_ref().context("device disappeared")?;
            device.read_timeout(&mut buffer, timeout_ms)
        };

        match result {
            Ok(0) => Ok(None),
            Ok(size) => {
                buffer.truncate(size);
                Ok(Some(buffer))
            }
            Err(error) => {
                self.device = None;
                Err(anyhow::Error::new(error).context("failed to read hid report"))
            }
        }
    }

    pub fn get_feature_report(
        &mut self,
        report_id: u8,
        report_size: usize,
    ) -> Result<Option<Vec<u8>>> {
        if !self.ensure_connected()? {
            return Ok(None);
        }

        let mut buffer = vec![0_u8; report_size];
        buffer[0] = report_id;

        let result = {
            let device = self.device.as_ref().context("device disappeared")?;
            device.get_feature_report(&mut buffer)
        };

        match result {
            Ok(0) => Ok(None),
            Ok(size) => {
                buffer.truncate(size);
                Ok(Some(buffer))
            }
            Err(error) => {
                self.device = None;
                Err(anyhow::Error::new(error).context("failed to read feature report"))
            }
        }
    }

    pub fn send_output_report_with_method(
        &mut self,
        report: &[u8],
    ) -> Result<Option<OutputReportSendMethod>> {
        if !self.ensure_connected()? {
            return Ok(None);
        }

        let set_output_result = {
            let device = self.device.as_ref().context("device disappeared")?;
            device.send_output_report(report)
        };

        if set_output_result.is_ok() {
            return Ok(Some(OutputReportSendMethod::SetOutputReport));
        }

        let write_result = {
            let device = self.device.as_ref().context("device disappeared")?;
            device.write(report)
        };

        match write_result {
            Ok(_) => Ok(Some(OutputReportSendMethod::WriteFile)),
            Err(write_error) => {
                let set_output_error = set_output_result
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                self.device = None;
                Err(anyhow!(
                    "failed to send output report; HidD_SetOutputReport={set_output_error}; WriteFile={write_error}"
                ))
            }
        }
    }
}

pub fn enumerate_devices(filter: &HidDeviceFilter) -> Result<Vec<HidDeviceSnapshot>> {
    let mut api = HidApi::new().context("failed to initialize hidapi")?;
    api.refresh_devices()
        .context("failed to refresh hid device list")?;

    Ok(api
        .device_list()
        .filter(|device| device_matches(device, filter))
        .map(snapshot_from_device)
        .collect())
}

pub fn open_matching_devices(
    filter: &HidDeviceFilter,
) -> Result<Vec<(HidDeviceSnapshot, HidDevice)>> {
    let mut api = HidApi::new().context("failed to initialize hidapi")?;
    api.refresh_devices()
        .context("failed to refresh hid device list")?;

    let mut devices = Vec::new();
    for device_info in api.device_list() {
        if !device_matches(device_info, filter) {
            continue;
        }

        let snapshot = snapshot_from_device(device_info);
        let device = device_info
            .open_device(&api)
            .with_context(|| format!("failed to open {}", snapshot.path))?;
        devices.push((snapshot, device));
    }

    Ok(devices)
}

fn device_matches(device: &DeviceInfo, filter: &HidDeviceFilter) -> bool {
    filter
        .vendor_id
        .map(|vendor_id| device.vendor_id() == vendor_id)
        .unwrap_or(true)
        && filter
            .product_id
            .map(|product_id| device.product_id() == product_id)
            .unwrap_or(true)
        && filter
            .interface_number
            .map(|interface_number| device.interface_number() == interface_number)
            .unwrap_or(true)
        && filter
            .usage_page
            .map(|usage_page| device.usage_page() == usage_page)
            .unwrap_or(true)
        && filter
            .usage
            .map(|usage| device.usage() == usage)
            .unwrap_or(true)
}

fn snapshot_from_device(device: &DeviceInfo) -> HidDeviceSnapshot {
    HidDeviceSnapshot {
        path: device.path().to_string_lossy().into_owned(),
        vendor_id: device.vendor_id(),
        product_id: device.product_id(),
        interface_number: device.interface_number(),
        usage_page: device.usage_page(),
        usage: device.usage(),
        manufacturer_string: device.manufacturer_string().map(str::to_string),
        product_string: device.product_string().map(str::to_string),
        serial_number: device.serial_number().map(str::to_string),
    }
}
