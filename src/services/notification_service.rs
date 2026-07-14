use anyhow::Result;

/// AppUserModelID used for toast notifications.
///
/// Registering our own AUMID (instead of borrowing `Toast::POWERSHELL_APP_ID`)
/// makes notifications appear as "INZONE Buds" and lets users manage them
/// independently in Windows notification settings.
pub const APP_ID: &str = "InzoneBuds.BatteryTray";
const APP_DISPLAY_NAME: &str = "INZONE Buds";

/// Register the AUMID in the current user's registry so Windows shows toasts
/// under our own display name. Safe to call on every startup (idempotent).
#[cfg(windows)]
pub fn register_app_id() -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = format!("Software\\Classes\\AppUserModelId\\{APP_ID}");
    let (key, _) = hkcu.create_subkey(path)?;
    key.set_value("DisplayName", &APP_DISPLAY_NAME.to_string())?;
    Ok(())
}

#[cfg(not(windows))]
pub fn register_app_id() -> Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn notify_low_battery(percent: u8) -> Result<()> {
    use winrt_notification::{Duration, Sound, Toast};

    Toast::new(APP_ID)
        .title(APP_DISPLAY_NAME)
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
