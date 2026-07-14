use std::path::PathBuf;

/// Base directory for app data such as `config/` and `logs/`.
///
/// Paths are resolved relative to the directory containing the executable, not
/// the current working directory. This keeps config and logs next to the exe
/// regardless of how the app is launched (double-click, autostart, scheduled
/// task), where the working directory is often `C:\Windows\System32`.
///
/// Falls back to the current directory only if the executable path cannot be
/// determined.
pub fn base_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}
