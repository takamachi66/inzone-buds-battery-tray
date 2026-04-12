use anyhow::Context;

#[cfg(target_os = "windows")]
pub struct SingleInstanceGuard {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(not(target_os = "windows"))]
pub struct SingleInstanceGuard;

#[cfg(target_os = "windows")]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        // SAFETY: The handle is returned by CreateMutexW and owned by this guard.
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn try_acquire(name: &str) -> anyhow::Result<Option<SingleInstanceGuard>> {
    let mut wide_name: Vec<u16> = name.encode_utf16().collect();
    wide_name.push(0);

    // SAFETY: The pointer stays valid for the duration of the call and is NUL-terminated.
    let handle = unsafe {
        windows_sys::Win32::System::Threading::CreateMutexW(
            std::ptr::null_mut(),
            0,
            wide_name.as_ptr(),
        )
    };

    if handle.is_null() {
        return Err(std::io::Error::last_os_error())
            .context("failed to create single-instance mutex");
    }

    // SAFETY: GetLastError can be queried immediately after CreateMutexW.
    let last_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    if last_error == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS {
        // SAFETY: Handle is valid and must be closed on this path.
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
        }
        return Ok(None);
    }

    Ok(Some(SingleInstanceGuard { handle }))
}

#[cfg(not(target_os = "windows"))]
pub fn try_acquire(_name: &str) -> anyhow::Result<Option<SingleInstanceGuard>> {
    Ok(Some(SingleInstanceGuard))
}
