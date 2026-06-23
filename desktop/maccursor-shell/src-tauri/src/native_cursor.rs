use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::WebviewWindow;

use crate::diagnostics::ShellDiagnostics;

const BACKEND: &str = "native-macos";
const VISUAL: &str = "dom-event-time";
const DEFAULT_WIDTH: f64 = 1280.0;
const DEFAULT_HEIGHT: f64 = 820.0;

#[derive(Clone, Default)]
pub struct NativeCursorBackend {
    inner: Arc<Mutex<NativeCursorSession>>,
    diagnostics: Option<ShellDiagnostics>,
}

#[derive(Debug)]
struct NativeCursorSession {
    active: bool,
    cursor_hidden: bool,
    cursor_disconnected: bool,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    native_events_received: u64,
    js_events_dispatched: u64,
    dropped_events: u64,
    last_reason: String,
    last_error: Option<String>,
    window: Option<WebviewWindow>,
}

impl Default for NativeCursorSession {
    fn default() -> Self {
        Self {
            active: false,
            cursor_hidden: false,
            cursor_disconnected: false,
            x: DEFAULT_WIDTH / 2.0,
            y: DEFAULT_HEIGHT / 2.0,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            native_events_received: 0,
            js_events_dispatched: 0,
            dropped_events: 0,
            last_reason: "ready".to_string(),
            last_error: None,
            window: None,
        }
    }
}

impl Drop for NativeCursorSession {
    fn drop(&mut self) {
        restore_system_cursor(self.cursor_hidden, self.cursor_disconnected);
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeCursorSnapshot {
    supported: bool,
    backend: &'static str,
    active: bool,
    visual: &'static str,
    movement_batched: bool,
    native_events_received: u64,
    js_events_dispatched: u64,
    dropped_events: u64,
    last_reason: String,
    last_error: Option<String>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCursorEvent {
    #[serde(rename = "type")]
    event_type: &'static str,
    backend: &'static str,
    visual: &'static str,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    button: Option<i32>,
    delta_x: f64,
    delta_y: f64,
    shift_key: bool,
    ctrl_key: bool,
    meta_key: bool,
    alt_key: bool,
    sequence: u64,
    native_events_received: u64,
    sent_at_ms: f64,
    event_timestamp: f64,
}

#[derive(Clone, Debug)]
struct NativeCursorEventInput {
    event_type: &'static str,
    dx: f64,
    dy: f64,
    button: Option<i32>,
    delta_x: f64,
    delta_y: f64,
    shift_key: bool,
    ctrl_key: bool,
    meta_key: bool,
    alt_key: bool,
    event_timestamp: f64,
}

impl NativeCursorBackend {
    pub fn with_diagnostics(diagnostics: ShellDiagnostics) -> Self {
        Self {
            inner: Arc::new(Mutex::new(NativeCursorSession::default())),
            diagnostics: Some(diagnostics),
        }
    }

    pub fn install(&self, window: &WebviewWindow) {
        configure_window_for_mouse_motion(window);
        install_native_event_monitor(self.clone());
    }

    fn start(
        &self,
        window: WebviewWindow,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<NativeCursorSnapshot, String> {
        let mut session = self.lock_session()?;
        session.window = Some(window);
        set_bounds(&mut session, width, height);
        session.x = clamp(finite_or(x, session.width / 2.0), 0.0, session.width);
        session.y = clamp(finite_or(y, session.height / 2.0), 0.0, session.height);

        if !session.active {
            eprintln!(
                "maccursor-shell native capture start requested x={:.1} y={:.1} width={:.1} height={:.1}",
                session.x, session.y, session.width, session.height
            );
            self.log_event(
                "native_cursor_capture_start_requested",
                serde_json::json!({
                    "x": session.x,
                    "y": session.y,
                    "width": session.width,
                    "height": session.height,
                }),
            );
            match start_system_cursor_capture() {
                Ok(capture) => {
                    session.cursor_hidden = capture.cursor_hidden;
                    session.cursor_disconnected = capture.cursor_disconnected;
                    session.active = true;
                    session.last_reason = "capture-start".to_string();
                    session.last_error = None;
                    eprintln!("maccursor-shell native capture started");
                    self.log_event("native_cursor_capture_started", serde_json::json!({}));
                }
                Err(err) => {
                    session.last_reason = "capture-start-failed".to_string();
                    session.last_error = Some(err.clone());
                    eprintln!("maccursor-shell native capture failed: {err}");
                    self.log_event(
                        "native_cursor_capture_start_failed",
                        serde_json::json!({ "message": err }),
                    );
                    return Err(err);
                }
            }
        }

        Ok(snapshot_from_session(&session))
    }

    fn configure(&self, width: f64, height: f64) -> Result<NativeCursorSnapshot, String> {
        let mut session = self.lock_session()?;
        set_bounds(&mut session, width, height);
        session.x = clamp(session.x, 0.0, session.width);
        session.y = clamp(session.y, 0.0, session.height);
        Ok(snapshot_from_session(&session))
    }

    pub fn stop(&self, reason: &str) -> Result<NativeCursorSnapshot, String> {
        let (window, event, snapshot) = {
            let mut session = self.lock_session()?;
            let was_active = session.active;
            if was_active {
                restore_system_cursor(session.cursor_hidden, session.cursor_disconnected);
                eprintln!("maccursor-shell native capture stopped reason={reason}");
                self.log_event(
                    "native_cursor_capture_stopped",
                    serde_json::json!({ "reason": reason }),
                );
            }
            session.active = false;
            session.cursor_hidden = false;
            session.cursor_disconnected = false;
            session.last_reason = reason.to_string();
            session.last_error = None;
            let event = if was_active {
                session.native_events_received = session.native_events_received.saturating_add(1);
                Some(NativeCursorEvent {
                    event_type: "capture",
                    backend: BACKEND,
                    visual: VISUAL,
                    x: session.x,
                    y: session.y,
                    dx: 0.0,
                    dy: 0.0,
                    button: None,
                    delta_x: 0.0,
                    delta_y: 0.0,
                    shift_key: false,
                    ctrl_key: false,
                    meta_key: false,
                    alt_key: false,
                    sequence: session.native_events_received,
                    native_events_received: session.native_events_received,
                    sent_at_ms: now_ms(),
                    event_timestamp: 0.0,
                })
            } else {
                None
            };
            (
                session.window.clone(),
                event,
                snapshot_from_session(&session),
            )
        };
        if let (Some(window), Some(event)) = (window, event) {
            self.dispatch_to_js(&window, &event)?;
        }
        Ok(snapshot)
    }

    fn diagnostics(&self) -> Result<NativeCursorSnapshot, String> {
        let session = self.lock_session()?;
        Ok(snapshot_from_session(&session))
    }

    fn handle_native_event(&self, input: NativeCursorEventInput) -> bool {
        let (window, event) = {
            let mut session = match self.inner.lock() {
                Ok(session) => session,
                Err(_) => return false,
            };
            if !session.active {
                return false;
            }

            session.x = clamp(session.x + input.dx, 0.0, session.width);
            session.y = clamp(session.y + input.dy, 0.0, session.height);
            session.native_events_received = session.native_events_received.saturating_add(1);
            session.last_reason = "native-event".to_string();
            session.last_error = None;

            let event = NativeCursorEvent {
                event_type: input.event_type,
                backend: BACKEND,
                visual: VISUAL,
                x: session.x,
                y: session.y,
                dx: input.dx,
                dy: input.dy,
                button: input.button,
                delta_x: input.delta_x,
                delta_y: input.delta_y,
                shift_key: input.shift_key,
                ctrl_key: input.ctrl_key,
                meta_key: input.meta_key,
                alt_key: input.alt_key,
                sequence: session.native_events_received,
                native_events_received: session.native_events_received,
                sent_at_ms: now_ms(),
                event_timestamp: input.event_timestamp,
            };
            (session.window.clone(), event)
        };

        match window {
            Some(window) => {
                if let Err(err) = self.dispatch_to_js(&window, &event) {
                    if let Ok(mut session) = self.inner.lock() {
                        session.dropped_events = session.dropped_events.saturating_add(1);
                        session.last_error = Some(err.clone());
                    }
                    self.log_event(
                        "native_cursor_dispatch_failed",
                        serde_json::json!({ "message": err }),
                    );
                }
            }
            None => {
                if let Ok(mut session) = self.inner.lock() {
                    session.dropped_events = session.dropped_events.saturating_add(1);
                    session.last_error =
                        Some("native cursor event had no WebView target".to_string());
                }
                self.log_event(
                    "native_cursor_dispatch_failed",
                    serde_json::json!({ "message": "native cursor event had no WebView target" }),
                );
            }
        }
        true
    }

    fn dispatch_to_js(
        &self,
        window: &WebviewWindow,
        event: &NativeCursorEvent,
    ) -> Result<(), String> {
        let payload = serde_json::to_string(event).map_err(|err| err.to_string())?;
        let script = format!(
            "window.__RTS_NATIVE_CURSOR && window.__RTS_NATIVE_CURSOR.__dispatchNativeEvent({payload});"
        );
        window.eval(script).map_err(|err| err.to_string())?;
        let mut session = self.lock_session()?;
        session.js_events_dispatched = session.js_events_dispatched.saturating_add(1);
        Ok(())
    }

    fn lock_session(&self) -> Result<std::sync::MutexGuard<'_, NativeCursorSession>, String> {
        self.inner
            .lock()
            .map_err(|_| "native cursor backend state is poisoned".to_string())
    }

    fn log_event(&self, event: &str, fields: serde_json::Value) {
        if let Some(diagnostics) = &self.diagnostics {
            diagnostics.log_event(event, fields);
        }
    }
}

#[tauri::command]
pub fn maccursor_start(
    window: WebviewWindow,
    state: tauri::State<'_, NativeCursorBackend>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<NativeCursorSnapshot, String> {
    state.start(window, x, y, width, height)
}

#[tauri::command]
pub fn maccursor_configure(
    state: tauri::State<'_, NativeCursorBackend>,
    width: f64,
    height: f64,
) -> Result<NativeCursorSnapshot, String> {
    state.configure(width, height)
}

#[tauri::command]
pub fn maccursor_stop(
    state: tauri::State<'_, NativeCursorBackend>,
    reason: Option<String>,
) -> Result<NativeCursorSnapshot, String> {
    state.stop(reason.as_deref().unwrap_or("js-stop"))
}

#[tauri::command]
pub fn maccursor_diagnostics(
    state: tauri::State<'_, NativeCursorBackend>,
) -> Result<NativeCursorSnapshot, String> {
    state.diagnostics()
}

fn snapshot_from_session(session: &NativeCursorSession) -> NativeCursorSnapshot {
    NativeCursorSnapshot {
        supported: cfg!(target_os = "macos"),
        backend: BACKEND,
        active: session.active,
        visual: VISUAL,
        movement_batched: false,
        native_events_received: session.native_events_received,
        js_events_dispatched: session.js_events_dispatched,
        dropped_events: session.dropped_events,
        last_reason: session.last_reason.clone(),
        last_error: session.last_error.clone(),
        x: session.x,
        y: session.y,
        width: session.width,
        height: session.height,
    }
}

fn set_bounds(session: &mut NativeCursorSession, width: f64, height: f64) {
    session.width = finite_dimension(width, DEFAULT_WIDTH);
    session.height = finite_dimension(height, DEFAULT_HEIGHT);
}

fn finite_dimension(value: f64, fallback: f64) -> f64 {
    let value = finite_or(value, fallback);
    if value >= 1.0 {
        value
    } else {
        fallback
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

#[derive(Clone, Copy)]
struct SystemCaptureState {
    cursor_hidden: bool,
    cursor_disconnected: bool,
}

#[cfg(target_os = "macos")]
fn start_system_cursor_capture() -> Result<SystemCaptureState, String> {
    use objc2_core_graphics::{
        CGAssociateMouseAndMouseCursorPosition, CGDisplayHideCursor, CGDisplayShowCursor, CGError,
        CGMainDisplayID,
    };

    let display = CGMainDisplayID();
    let hide = CGDisplayHideCursor(display);
    let associate = CGAssociateMouseAndMouseCursorPosition(false);
    let cursor_hidden = hide == CGError::Success;
    let cursor_disconnected = associate == CGError::Success;
    if cursor_hidden && cursor_disconnected {
        return Ok(SystemCaptureState {
            cursor_hidden,
            cursor_disconnected,
        });
    }

    if cursor_disconnected {
        let _ = CGAssociateMouseAndMouseCursorPosition(true);
    }
    if cursor_hidden {
        let _ = CGDisplayShowCursor(display);
    }
    Err(format!(
        "failed to start native cursor capture: hide={} associate={}",
        hide.0, associate.0
    ))
}

#[cfg(not(target_os = "macos"))]
fn start_system_cursor_capture() -> Result<SystemCaptureState, String> {
    Err("native cursor capture is only available on macOS".to_string())
}

#[cfg(target_os = "macos")]
fn restore_system_cursor(cursor_hidden: bool, cursor_disconnected: bool) {
    use objc2_core_graphics::{
        CGAssociateMouseAndMouseCursorPosition, CGDisplayShowCursor, CGMainDisplayID,
    };

    if cursor_disconnected {
        let _ = CGAssociateMouseAndMouseCursorPosition(true);
    }
    if cursor_hidden {
        let _ = CGDisplayShowCursor(CGMainDisplayID());
    }
}

#[cfg(not(target_os = "macos"))]
fn restore_system_cursor(_cursor_hidden: bool, _cursor_disconnected: bool) {}

#[cfg(target_os = "macos")]
fn configure_window_for_mouse_motion(window: &WebviewWindow) {
    use objc2_app_kit::NSWindow;

    let Ok(ns_window) = window.ns_window() else {
        return;
    };
    if ns_window.is_null() {
        return;
    }
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    ns_window.setAcceptsMouseMovedEvents(true);
}

#[cfg(not(target_os = "macos"))]
fn configure_window_for_mouse_motion(_window: &WebviewWindow) {}

#[cfg(target_os = "macos")]
fn install_native_event_monitor(backend: NativeCursorBackend) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};

    let block = RcBlock::new(move |event_ptr: NonNull<NSEvent>| -> *mut NSEvent {
        let event = unsafe { event_ptr.as_ref() };
        let input = native_event_input(event);
        if let Some(input) = input {
            if backend.handle_native_event(input) {
                return std::ptr::null_mut();
            }
        }
        event_ptr.as_ptr()
    });

    let mask = NSEventMask::MouseMoved
        | NSEventMask::LeftMouseDragged
        | NSEventMask::RightMouseDragged
        | NSEventMask::OtherMouseDragged
        | NSEventMask::LeftMouseDown
        | NSEventMask::LeftMouseUp
        | NSEventMask::RightMouseDown
        | NSEventMask::RightMouseUp
        | NSEventMask::OtherMouseDown
        | NSEventMask::OtherMouseUp
        | NSEventMask::ScrollWheel;

    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
    if let Some(monitor) = monitor {
        let _ = objc2::rc::Retained::<AnyObject>::into_raw(monitor);
    }
    let _ = RcBlock::into_raw(block);

    fn native_event_input(event: &NSEvent) -> Option<NativeCursorEventInput> {
        let ty = event.r#type();
        let flags = event.modifierFlags();
        let modifiers = (
            flags.contains(NSEventModifierFlags::Shift),
            flags.contains(NSEventModifierFlags::Control),
            flags.contains(NSEventModifierFlags::Command),
            flags.contains(NSEventModifierFlags::Option),
        );
        let base = |event_type: &'static str| NativeCursorEventInput {
            event_type,
            dx: 0.0,
            dy: 0.0,
            button: None,
            delta_x: 0.0,
            delta_y: 0.0,
            shift_key: modifiers.0,
            ctrl_key: modifiers.1,
            meta_key: modifiers.2,
            alt_key: modifiers.3,
            event_timestamp: event.timestamp(),
        };

        match ty {
            NSEventType::MouseMoved
            | NSEventType::LeftMouseDragged
            | NSEventType::RightMouseDragged
            | NSEventType::OtherMouseDragged => {
                let mut input = base("move");
                input.dx = event.deltaX();
                input.dy = event.deltaY();
                Some(input)
            }
            NSEventType::LeftMouseDown => {
                let mut input = base("down");
                input.button = Some(0);
                Some(input)
            }
            NSEventType::LeftMouseUp => {
                let mut input = base("up");
                input.button = Some(0);
                Some(input)
            }
            NSEventType::RightMouseDown => {
                let mut input = base("down");
                input.button = Some(2);
                Some(input)
            }
            NSEventType::RightMouseUp => {
                let mut input = base("up");
                input.button = Some(2);
                Some(input)
            }
            NSEventType::OtherMouseDown => {
                let mut input = base("down");
                input.button = Some(event.buttonNumber() as i32);
                Some(input)
            }
            NSEventType::OtherMouseUp => {
                let mut input = base("up");
                input.button = Some(event.buttonNumber() as i32);
                Some(input)
            }
            NSEventType::ScrollWheel => {
                let mut input = base("wheel");
                input.delta_x = -event.scrollingDeltaX();
                input.delta_y = -event.scrollingDeltaY();
                Some(input)
            }
            _ => None,
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn install_native_event_monitor(_backend: NativeCursorBackend) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_dimensions_reject_bad_values() {
        assert_eq!(finite_dimension(f64::NAN, 44.0), 44.0);
        assert_eq!(finite_dimension(0.0, 44.0), 44.0);
        assert_eq!(finite_dimension(12.0, 44.0), 12.0);
    }

    #[test]
    fn snapshot_reports_dom_event_time_visual_backend() {
        let backend = NativeCursorBackend::default();
        let snapshot = backend.diagnostics().unwrap();
        assert_eq!(snapshot.backend, BACKEND);
        assert_eq!(snapshot.visual, VISUAL);
        assert!(!snapshot.movement_batched);
        assert!(!snapshot.active);
    }
}
