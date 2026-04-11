use anyhow::Result;

#[cfg(windows)]
pub fn notify_low_battery(percent: u8) -> Result<()> {
    use winrt_notification::{Duration, Sound, Toast};

    Toast::new(Toast::POWERSHELL_APP_ID)
        .title("INZONE Buds")
        .text1(&format!("Battery low: {percent}%"))
        .sound(Some(Sound::Default))
        .duration(Duration::Short)
        .show()?;

    Ok(())
}

#[cfg(not(windows))]
pub fn notify_low_battery(percent: u8) -> Result<()> {
    tracing::warn!("low battery notification is not supported on this platform: {percent}%");
    Ok(())
}
