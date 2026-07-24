use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow};
use windows_sys::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{
        ChangeDisplaySettingsExW, EnumDisplaySettingsW, GetMonitorInfoW, MonitorFromWindow,
        CDS_FULLSCREEN, DEVMODEW, DISP_CHANGE_SUCCESSFUL, DM_BITSPERPEL, DM_DISPLAYFREQUENCY,
        DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS, MONITORINFOEXW,
        MONITOR_DEFAULTTONEAREST,
    },
};

use crate::diagnostics::ShellDiagnostics;

#[derive(Clone, Default)]
pub struct ExclusiveFullscreen {
    inner: Arc<Mutex<ExclusiveFullscreenSession>>,
    diagnostics: Option<ShellDiagnostics>,
}

#[derive(Default)]
struct ExclusiveFullscreenSession {
    requested: bool,
    active: bool,
    restoration: Option<WindowRestoration>,
    last_reason: String,
    last_error: Option<String>,
}

struct WindowRestoration {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    decorated: bool,
    maximized: bool,
    device_name: [u16; 32],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusiveFullscreenSnapshot {
    supported: bool,
    requested: bool,
    active: bool,
    mode: &'static str,
    last_reason: String,
    last_error: Option<String>,
}

impl ExclusiveFullscreen {
    pub fn with_diagnostics(diagnostics: ShellDiagnostics) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ExclusiveFullscreenSession {
                requested: false,
                active: false,
                restoration: None,
                last_reason: "ready".to_string(),
                last_error: None,
            })),
            diagnostics: Some(diagnostics),
        }
    }

    pub fn active(&self) -> bool {
        self.inner
            .lock()
            .map(|session| session.active)
            .unwrap_or(false)
    }

    pub fn set_enabled(
        &self,
        window: &WebviewWindow,
        enabled: bool,
    ) -> Result<ExclusiveFullscreenSnapshot, String> {
        {
            let mut session = self.lock_session()?;
            session.requested = enabled;
            session.last_reason = if enabled {
                "preference-enabled"
            } else {
                "preference-disabled"
            }
            .to_string();
        }
        if enabled {
            self.enter(window, "preference-enabled")
        } else {
            self.exit(window, "preference-disabled")
        }
    }

    pub fn suspend(
        &self,
        window: &WebviewWindow,
        reason: &str,
    ) -> Result<ExclusiveFullscreenSnapshot, String> {
        self.exit(window, reason)
    }

    pub fn resume(&self, window: &WebviewWindow) -> Result<ExclusiveFullscreenSnapshot, String> {
        if !self.lock_session()?.requested {
            return self.snapshot();
        }
        self.enter(window, "window-focused")
    }

    pub fn shutdown(
        &self,
        window: &WebviewWindow,
        reason: &str,
    ) -> Result<ExclusiveFullscreenSnapshot, String> {
        {
            let mut session = self.lock_session()?;
            session.requested = false;
        }
        self.exit(window, reason)
    }

    pub fn snapshot(&self) -> Result<ExclusiveFullscreenSnapshot, String> {
        let session = self.lock_session()?;
        Ok(snapshot_from_session(&session))
    }

    fn enter(
        &self,
        window: &WebviewWindow,
        reason: &str,
    ) -> Result<ExclusiveFullscreenSnapshot, String> {
        if self.active() {
            return self.snapshot();
        }

        let restoration = capture_window_restoration(window)?;
        let display = display_mode_for_window(window)?;
        let change_result = unsafe {
            ChangeDisplaySettingsExW(
                display.device_name.as_ptr(),
                &display.mode,
                std::ptr::null_mut(),
                CDS_FULLSCREEN,
                std::ptr::null(),
            )
        };
        if change_result != DISP_CHANGE_SUCCESSFUL {
            return self.fail(format!(
                "ChangeDisplaySettingsExW(CDS_FULLSCREEN) failed with code {change_result}"
            ));
        }

        let window_result = configure_exclusive_window(window, display.monitor_rect);
        if let Err(err) = window_result {
            restore_display_mode(&display.device_name);
            let _ = restore_window(window, &restoration);
            return self.fail(err);
        }

        let snapshot = {
            let mut session = self.lock_session()?;
            session.active = true;
            session.restoration = Some(WindowRestoration {
                device_name: display.device_name,
                ..restoration
            });
            session.last_reason = reason.to_string();
            session.last_error = None;
            snapshot_from_session(&session)
        };
        self.log_event(
            "exclusive_fullscreen_entered",
            serde_json::json!({
                "reason": reason,
                "width": display.mode.dmPelsWidth,
                "height": display.mode.dmPelsHeight,
                "refreshHz": display.mode.dmDisplayFrequency,
            }),
        );
        Ok(snapshot)
    }

    fn exit(
        &self,
        window: &WebviewWindow,
        reason: &str,
    ) -> Result<ExclusiveFullscreenSnapshot, String> {
        let restoration = {
            let mut session = self.lock_session()?;
            if !session.active {
                session.last_reason = reason.to_string();
                return Ok(snapshot_from_session(&session));
            }
            session.active = false;
            session.last_reason = reason.to_string();
            session.last_error = None;
            session.restoration.take()
        };

        let Some(restoration) = restoration else {
            return self.fail("exclusive fullscreen restoration state is missing".to_string());
        };
        restore_display_mode(&restoration.device_name);
        if let Err(err) = restore_window(window, &restoration) {
            return self.fail(err);
        }
        self.log_event(
            "exclusive_fullscreen_exited",
            serde_json::json!({ "reason": reason }),
        );
        self.snapshot()
    }

    fn fail(&self, message: String) -> Result<ExclusiveFullscreenSnapshot, String> {
        if let Ok(mut session) = self.inner.lock() {
            session.active = false;
            session.last_reason = "exclusive-fullscreen-failed".to_string();
            session.last_error = Some(message.clone());
        }
        self.log_event(
            "exclusive_fullscreen_failed",
            serde_json::json!({ "message": message }),
        );
        Err(message)
    }

    fn lock_session(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ExclusiveFullscreenSession>, String> {
        self.inner
            .lock()
            .map_err(|_| "exclusive fullscreen state is poisoned".to_string())
    }

    fn log_event(&self, event: &str, fields: serde_json::Value) {
        if let Some(diagnostics) = &self.diagnostics {
            diagnostics.log_event(event, fields);
        }
    }
}

impl Drop for ExclusiveFullscreenSession {
    fn drop(&mut self) {
        if let Some(restoration) = self.restoration.take() {
            restore_display_mode(&restoration.device_name);
        }
    }
}

#[tauri::command]
pub fn desktop_set_exclusive_fullscreen(
    window: WebviewWindow,
    state: tauri::State<'_, ExclusiveFullscreen>,
    native_cursor: tauri::State<'_, crate::native_cursor::NativeCursorBackend>,
    enabled: bool,
) -> Result<ExclusiveFullscreenSnapshot, String> {
    if !enabled {
        native_cursor.stop("exclusive-fullscreen-disabled")?;
    }
    state.set_enabled(&window, enabled)
}

#[tauri::command]
pub fn desktop_exclusive_fullscreen_diagnostics(
    state: tauri::State<'_, ExclusiveFullscreen>,
) -> Result<ExclusiveFullscreenSnapshot, String> {
    state.snapshot()
}

fn snapshot_from_session(session: &ExclusiveFullscreenSession) -> ExclusiveFullscreenSnapshot {
    ExclusiveFullscreenSnapshot {
        supported: true,
        requested: session.requested,
        active: session.active,
        mode: "windows-display-mode",
        last_reason: session.last_reason.clone(),
        last_error: session.last_error.clone(),
    }
}

struct DisplayMode {
    device_name: [u16; 32],
    mode: DEVMODEW,
    monitor_rect: windows_sys::Win32::Foundation::RECT,
}

fn display_mode_for_window(window: &WebviewWindow) -> Result<DisplayMode, String> {
    let hwnd = window
        .hwnd()
        .map_err(|err| format!("failed to resolve Windows window handle: {err}"))?
        .0 as HWND;
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return Err("failed to resolve the window monitor".to_string());
    }

    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if unsafe {
        GetMonitorInfoW(
            monitor,
            &mut monitor_info as *mut MONITORINFOEXW
                as *mut windows_sys::Win32::Graphics::Gdi::MONITORINFO,
        )
    } == 0
    {
        return Err("GetMonitorInfoW failed".to_string());
    }

    let mut mode = DEVMODEW::default();
    mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    if unsafe {
        EnumDisplaySettingsW(
            monitor_info.szDevice.as_ptr(),
            ENUM_CURRENT_SETTINGS,
            &mut mode,
        )
    } == 0
    {
        return Err("EnumDisplaySettingsW failed for the active monitor".to_string());
    }
    mode.dmFields = DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
    Ok(DisplayMode {
        device_name: monitor_info.szDevice,
        mode,
        monitor_rect: monitor_info.monitorInfo.rcMonitor,
    })
}

fn capture_window_restoration(window: &WebviewWindow) -> Result<WindowRestoration, String> {
    Ok(WindowRestoration {
        position: window
            .outer_position()
            .map_err(|err| format!("failed to read window position: {err}"))?,
        size: window
            .outer_size()
            .map_err(|err| format!("failed to read window size: {err}"))?,
        decorated: window
            .is_decorated()
            .map_err(|err| format!("failed to read window decorations: {err}"))?,
        maximized: window
            .is_maximized()
            .map_err(|err| format!("failed to read maximized state: {err}"))?,
        device_name: [0; 32],
    })
}

fn configure_exclusive_window(
    window: &WebviewWindow,
    rect: windows_sys::Win32::Foundation::RECT,
) -> Result<(), String> {
    window
        .unmaximize()
        .map_err(|err| format!("failed to unmaximize window: {err}"))?;
    window
        .set_decorations(false)
        .map_err(|err| format!("failed to remove window decorations: {err}"))?;
    window
        .set_position(Position::Physical(PhysicalPosition::new(
            rect.left, rect.top,
        )))
        .map_err(|err| format!("failed to position exclusive window: {err}"))?;
    let width = rect.right.saturating_sub(rect.left).max(1) as u32;
    let height = rect.bottom.saturating_sub(rect.top).max(1) as u32;
    window
        .set_size(Size::Physical(PhysicalSize::new(width, height)))
        .map_err(|err| format!("failed to size exclusive window: {err}"))?;
    Ok(())
}

fn restore_window(window: &WebviewWindow, restoration: &WindowRestoration) -> Result<(), String> {
    window
        .set_decorations(restoration.decorated)
        .map_err(|err| format!("failed to restore window decorations: {err}"))?;
    window
        .set_position(Position::Physical(restoration.position))
        .map_err(|err| format!("failed to restore window position: {err}"))?;
    window
        .set_size(Size::Physical(restoration.size))
        .map_err(|err| format!("failed to restore window size: {err}"))?;
    if restoration.maximized {
        window
            .maximize()
            .map_err(|err| format!("failed to restore maximized state: {err}"))?;
    }
    Ok(())
}

fn restore_display_mode(device_name: &[u16; 32]) {
    unsafe {
        ChangeDisplaySettingsExW(
            device_name.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_defaults_to_opted_out_windowed_mode() {
        let fullscreen = ExclusiveFullscreen::default();
        let snapshot = fullscreen.snapshot().unwrap();
        assert!(snapshot.supported);
        assert!(!snapshot.requested);
        assert!(!snapshot.active);
        assert_eq!(snapshot.mode, "windows-display-mode");
    }
}
