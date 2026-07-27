use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::WebviewWindow;
use windows::Win32::{
    Foundation::HWND as TaskbarHwnd,
    System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    },
    UI::Shell::{ITaskbarList2, TaskbarList},
};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        ChangeDisplaySettingsExW, EnumDisplaySettingsW, GetMonitorInfoW, MonitorFromWindow,
        CDS_FULLSCREEN, CDS_RESET, DEVMODEW, DISP_CHANGE_SUCCESSFUL, DM_BITSPERPEL,
        DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS, MONITORINFOEXW,
        MONITOR_DEFAULTTONEAREST,
    },
    UI::WindowsAndMessaging::{
        GetWindowLongPtrW, GetWindowPlacement, GetWindowRect, PeekMessageW, SetForegroundWindow,
        SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE,
        HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, MSG, PM_NOREMOVE, SWP_FRAMECHANGED,
        SWP_NOOWNERZORDER, SWP_SHOWWINDOW, SW_RESTORE, WINDOWPLACEMENT, WS_EX_TOPMOST, WS_MAXIMIZE,
        WS_MINIMIZE, WS_OVERLAPPEDWINDOW,
    },
};

use crate::diagnostics::ShellDiagnostics;

#[derive(Clone, Default)]
pub struct ExclusiveFullscreen {
    inner: Arc<Mutex<ExclusiveFullscreenSession>>,
    operation: Arc<Mutex<()>>,
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

#[derive(Clone, Copy)]
struct WindowRestoration {
    rect: RECT,
    placement: WINDOWPLACEMENT,
    style: isize,
    ex_style: isize,
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
            operation: Arc::new(Mutex::new(())),
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
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "exclusive fullscreen operation lock is poisoned".to_string())?;
        if self.active() {
            return self.snapshot();
        }
        let restoration_pending = { self.lock_session()?.restoration.is_some() };
        if restoration_pending {
            return self.fail(
                "exclusive fullscreen cannot re-enter while restoration is pending".to_string(),
            );
        }

        let display = display_mode_for_window(window)?;
        let restoration = WindowRestoration {
            device_name: display.device_name,
            ..capture_window_restoration(window)?
        };
        {
            let mut session = self.lock_session()?;
            session.restoration = Some(restoration);
        }
        let change_result = unsafe {
            ChangeDisplaySettingsExW(
                display.device_name.as_ptr(),
                &display.mode,
                std::ptr::null_mut(),
                CDS_FULLSCREEN | CDS_RESET,
                std::ptr::null(),
            )
        };
        if change_result != DISP_CHANGE_SUCCESSFUL {
            if let Ok(mut session) = self.inner.lock() {
                session.restoration = None;
            }
            return self.fail(format!(
                "ChangeDisplaySettingsExW(CDS_FULLSCREEN) failed with code {change_result}"
            ));
        }
        pump_window_messages();

        let window_rect = match configure_exclusive_window(window, display.monitor_rect) {
            Ok(rect) => rect,
            Err(err) => {
                let display_rollback = restore_display_mode(&display.device_name);
                pump_window_messages();
                let taskbar_rollback = mark_taskbar_fullscreen(window_hwnd(window)?, false);
                let window_rollback = restore_window(window, &restoration);
                let rollback_errors = [display_rollback.err(), window_rollback.err()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                if rollback_errors.is_empty() {
                    if let Ok(mut session) = self.inner.lock() {
                        session.restoration = None;
                    }
                }
                if let Err(taskbar_err) = taskbar_rollback {
                    self.log_event(
                        "exclusive_fullscreen_taskbar_restore_failed",
                        serde_json::json!({ "message": taskbar_err }),
                    );
                }
                let message = if rollback_errors.is_empty() {
                    err
                } else {
                    format!("{err}; rollback failed: {}", rollback_errors.join("; "))
                };
                return self.fail(message);
            }
        };

        let snapshot = {
            let mut session = self.lock_session()?;
            session.active = true;
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
                "windowLeft": window_rect.left,
                "windowTop": window_rect.top,
                "windowWidth": window_rect.right.saturating_sub(window_rect.left),
                "windowHeight": window_rect.bottom.saturating_sub(window_rect.top),
                "monitorLeft": display.monitor_rect.left,
                "monitorTop": display.monitor_rect.top,
                "monitorWidth": display.monitor_rect.right.saturating_sub(display.monitor_rect.left),
                "monitorHeight": display.monitor_rect.bottom.saturating_sub(display.monitor_rect.top),
            }),
        );
        Ok(snapshot)
    }

    fn exit(
        &self,
        window: &WebviewWindow,
        reason: &str,
    ) -> Result<ExclusiveFullscreenSnapshot, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "exclusive fullscreen operation lock is poisoned".to_string())?;
        let restoration = {
            let mut session = self.lock_session()?;
            if !session.active && session.restoration.is_none() {
                session.last_reason = reason.to_string();
                return Ok(snapshot_from_session(&session));
            }
            session.restoration
        };

        let Some(restoration) = restoration else {
            return self.fail("exclusive fullscreen restoration state is missing".to_string());
        };
        let taskbar_result = mark_taskbar_fullscreen(window_hwnd(window)?, false);
        let display_result = restore_display_mode(&restoration.device_name);
        pump_window_messages();
        let window_result = restore_window(window, &restoration);
        if display_result.is_err() || window_result.is_err() {
            let message = [display_result.err(), window_result.err()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("; ");
            if let Ok(mut session) = self.inner.lock() {
                session.last_reason = "exclusive-fullscreen-restore-failed".to_string();
                session.last_error = Some(message.clone());
            }
            self.log_event(
                "exclusive_fullscreen_restore_failed",
                serde_json::json!({ "message": message, "reason": reason }),
            );
            return Err(message);
        }
        if let Err(message) = taskbar_result {
            self.log_event(
                "exclusive_fullscreen_taskbar_restore_failed",
                serde_json::json!({ "message": message, "reason": reason }),
            );
        }
        {
            let mut session = self.lock_session()?;
            session.active = false;
            session.last_reason = reason.to_string();
            session.last_error = None;
            session.restoration = None;
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
            let _ = restore_display_mode(&restoration.device_name);
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
    let hwnd = window_hwnd(window)?;
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return Err("GetWindowRect failed while capturing window restoration".to_string());
    }
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..WINDOWPLACEMENT::default()
    };
    if unsafe { GetWindowPlacement(hwnd, &mut placement) } == 0 {
        return Err("GetWindowPlacement failed while capturing window restoration".to_string());
    }
    Ok(WindowRestoration {
        rect,
        placement,
        style: unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) },
        ex_style: unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) },
        device_name: [0; 32],
    })
}

fn configure_exclusive_window(window: &WebviewWindow, rect: RECT) -> Result<RECT, String> {
    let hwnd = window_hwnd(window)?;
    // A maximized HWND remains governed by its work-area placement even after its frame is
    // removed. Normalize it first; the captured WINDOWPLACEMENT restores its zoom state later.
    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
    }
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    let fullscreen_style = fullscreen_window_style(style);
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, fullscreen_style);
    }
    let (width, height) = rect_size(rect);
    if unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOP,
            rect.left,
            rect.top,
            width,
            height,
            SWP_FRAMECHANGED | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
        )
    } == 0
    {
        return Err("SetWindowPos failed while entering exclusive fullscreen".to_string());
    }
    unsafe {
        SetForegroundWindow(hwnd);
    }
    let actual_style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    if actual_style != fullscreen_style {
        return Err(format!(
            "exclusive window style did not apply: expected {fullscreen_style:#x}, got {actual_style:#x}"
        ));
    }
    let actual = current_window_rect(hwnd)?;
    if !rect_matches(actual, rect) {
        return Err(format!(
            "exclusive window bounds did not match monitor: expected {}x{} at {},{}; got {}x{} at {},{}",
            width,
            height,
            rect.left,
            rect.top,
            actual.right.saturating_sub(actual.left),
            actual.bottom.saturating_sub(actual.top),
            actual.left,
            actual.top,
        ));
    }
    mark_taskbar_fullscreen(hwnd, true)?;
    Ok(actual)
}

fn restore_window(window: &WebviewWindow, restoration: &WindowRestoration) -> Result<(), String> {
    let hwnd = window_hwnd(window)?;
    let unzoomed_style = restoration.style & !(WS_MAXIMIZE as isize | WS_MINIMIZE as isize);
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, unzoomed_style);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, restoration.ex_style);
    }
    verify_window_styles(
        hwnd,
        unzoomed_style,
        restoration.ex_style,
        "before placement",
    )?;
    let insert_after = if restoration.ex_style & WS_EX_TOPMOST as isize != 0 {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };
    let (width, height) = rect_size(restoration.rect);
    if unsafe {
        SetWindowPos(
            hwnd,
            insert_after,
            restoration.rect.left,
            restoration.rect.top,
            width,
            height,
            SWP_FRAMECHANGED | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
        )
    } == 0
    {
        return Err("SetWindowPos failed while restoring the window".to_string());
    }
    if unsafe { SetWindowPlacement(hwnd, &restoration.placement) } == 0 {
        return Err("SetWindowPlacement failed while restoring the window".to_string());
    }
    verify_restored_window(hwnd, restoration)?;
    Ok(())
}

fn fullscreen_window_style(style: isize) -> isize {
    style & !(WS_OVERLAPPEDWINDOW as isize | WS_MAXIMIZE as isize | WS_MINIMIZE as isize)
}

fn verify_window_styles(
    hwnd: HWND,
    expected_style: isize,
    expected_ex_style: isize,
    phase: &str,
) -> Result<(), String> {
    let actual_style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    let actual_ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if actual_style != expected_style || actual_ex_style != expected_ex_style {
        return Err(format!(
            "window styles did not restore {phase}: expected {expected_style:#x}/{expected_ex_style:#x}, got {actual_style:#x}/{actual_ex_style:#x}"
        ));
    }
    Ok(())
}

fn verify_restored_window(hwnd: HWND, restoration: &WindowRestoration) -> Result<(), String> {
    let mut actual_placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..WINDOWPLACEMENT::default()
    };
    if unsafe { GetWindowPlacement(hwnd, &mut actual_placement) } == 0 {
        return Err("GetWindowPlacement failed while verifying window restoration".to_string());
    }
    if actual_placement.showCmd != restoration.placement.showCmd
        || !rect_matches(
            actual_placement.rcNormalPosition,
            restoration.placement.rcNormalPosition,
        )
    {
        return Err(format!(
            "window placement did not restore: expected show {} normal {},{}-{},{}; got show {} normal {},{}-{},{}",
            restoration.placement.showCmd,
            restoration.placement.rcNormalPosition.left,
            restoration.placement.rcNormalPosition.top,
            restoration.placement.rcNormalPosition.right,
            restoration.placement.rcNormalPosition.bottom,
            actual_placement.showCmd,
            actual_placement.rcNormalPosition.left,
            actual_placement.rcNormalPosition.top,
            actual_placement.rcNormalPosition.right,
            actual_placement.rcNormalPosition.bottom,
        ));
    }
    verify_window_styles(
        hwnd,
        restoration.style,
        restoration.ex_style,
        "after placement",
    )
}

fn window_hwnd(window: &WebviewWindow) -> Result<HWND, String> {
    window
        .hwnd()
        .map(|handle| handle.0 as HWND)
        .map_err(|err| format!("failed to resolve Windows window handle: {err}"))
}

fn current_window_rect(hwnd: HWND) -> Result<RECT, String> {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return Err("GetWindowRect failed while verifying exclusive fullscreen".to_string());
    }
    Ok(rect)
}

fn rect_size(rect: RECT) -> (i32, i32) {
    (
        rect.right.saturating_sub(rect.left).max(1),
        rect.bottom.saturating_sub(rect.top).max(1),
    )
}

fn rect_matches(actual: RECT, expected: RECT) -> bool {
    actual.left == expected.left
        && actual.top == expected.top
        && actual.right == expected.right
        && actual.bottom == expected.bottom
}

fn pump_window_messages() {
    unsafe {
        let mut message = MSG::default();
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }
}

fn mark_taskbar_fullscreen(hwnd: HWND, fullscreen: bool) -> Result<(), String> {
    let com_result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let should_uninitialize = com_result.is_ok();
    let result = (|| -> windows::core::Result<()> {
        let taskbar: ITaskbarList2 = unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_ALL) }?;
        unsafe {
            taskbar.HrInit()?;
            taskbar.MarkFullscreenWindow(TaskbarHwnd(hwnd), fullscreen)?;
        }
        Ok(())
    })()
    .map_err(|err| format!("ITaskbarList2::MarkFullscreenWindow failed: {err}"));
    if should_uninitialize {
        unsafe {
            CoUninitialize();
        }
    }
    result
}

fn restore_display_mode(device_name: &[u16; 32]) -> Result<(), String> {
    let result = unsafe {
        ChangeDisplaySettingsExW(
            device_name.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
        )
    };
    if result == DISP_CHANGE_SUCCESSFUL {
        Ok(())
    } else {
        Err(format!(
            "ChangeDisplaySettingsExW failed to restore the display mode with code {result}"
        ))
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

    #[test]
    fn exclusive_style_removes_the_complete_overlapped_frame() {
        let original = 0x14cf_0000isize | WS_MAXIMIZE as isize | WS_MINIMIZE as isize;
        let fullscreen = fullscreen_window_style(original);
        assert_eq!(fullscreen & WS_OVERLAPPEDWINDOW as isize, 0);
        assert_eq!(fullscreen & WS_MAXIMIZE as isize, 0);
        assert_eq!(fullscreen & WS_MINIMIZE as isize, 0);
        assert_eq!(
            fullscreen
                & !(WS_OVERLAPPEDWINDOW as isize | WS_MAXIMIZE as isize | WS_MINIMIZE as isize),
            original
                & !(WS_OVERLAPPEDWINDOW as isize | WS_MAXIMIZE as isize | WS_MINIMIZE as isize)
        );
    }

    #[test]
    fn fullscreen_bounds_require_the_exact_monitor_rectangle() {
        let monitor = RECT {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        assert_eq!(rect_size(monitor), (1920, 1080));
        assert!(rect_matches(monitor, monitor));
        assert!(!rect_matches(
            RECT {
                right: -1,
                ..monitor
            },
            monitor
        ));
    }
}
