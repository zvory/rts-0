#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

mod native_cursor;

use native_cursor::{
    maccursor_configure, maccursor_diagnostics, maccursor_start, maccursor_stop,
    NativeCursorBackend,
};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const SERVER_READY_TIMEOUT: Duration = Duration::from_secs(120);
const SERVER_URL_ENV: &str = "RTS_DESKTOP_SERVER_URL";
const WINDOW_LABEL: &str = "main";

type ShellResult<T> = Result<T, Box<dyn Error>>;

struct ManagedServer {
    child: Mutex<Option<Child>>,
}

impl Drop for ManagedServer {
    fn drop(&mut self) {
        let Ok(mut child) = self.child.lock() else {
            return;
        };
        if let Some(mut child) = child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Debug, Clone)]
struct ServerLaunch {
    url: String,
    mode: &'static str,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("maccursor-shell failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> ShellResult<()> {
    #[cfg(not(target_os = "macos"))]
    return Err(shell_error("maccursor-shell is a macOS-only spike"));

    #[cfg(target_os = "macos")]
    {
        tauri::Builder::default()
            .invoke_handler(tauri::generate_handler![
                maccursor_start,
                maccursor_configure,
                maccursor_stop,
                maccursor_diagnostics
            ])
            .setup(|app| {
                let server = launch_server(app.handle())?;
                let url: tauri::Url = server.url.parse().map_err(|err| {
                    shell_error(format!("invalid server URL {}: {err}", server.url))
                })?;
                let native_cursor = NativeCursorBackend::default();
                app.manage(native_cursor.clone());
                let runtime_script = desktop_runtime_script(&server);
                let window =
                    WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::External(url))
                        .title("Bewegungskrieg")
                        .inner_size(1280.0, 820.0)
                        .min_inner_size(960.0, 640.0)
                        .initialization_script(runtime_script)
                        .on_navigation(|url| {
                            matches!(url.host_str(), Some("127.0.0.1") | Some("localhost"))
                        })
                        .build()?;
                native_cursor.install(&window);
                Ok(())
            })
            .on_window_event(|window, event| {
                if window.label() != WINDOW_LABEL {
                    return;
                }
                match event {
                    WindowEvent::Focused(false) => {
                        let _ = window.state::<NativeCursorBackend>().stop("window blur");
                    }
                    WindowEvent::CloseRequested { .. } => {
                        let _ = window.state::<NativeCursorBackend>().stop("window closed");
                        window.app_handle().exit(0);
                    }
                    _ => {}
                }
            })
            .run(tauri::generate_context!())?;
        Ok(())
    }
}

fn launch_server(app: &tauri::AppHandle) -> ShellResult<ServerLaunch> {
    if let Ok(url) = std::env::var(SERVER_URL_ENV) {
        let url = normalize_server_url(&url)?;
        return Ok(ServerLaunch {
            url,
            mode: "external",
        });
    }

    let repo_root = repo_root()?;
    let mut child = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(repo_root.join("server/Cargo.toml"))
        .arg("--bin")
        .arg("rts-server")
        .env("RTS_ADDR", "127.0.0.1:0")
        .env("RTS_CLIENT_DIR", repo_root.join("client"))
        .env("RTS_MAPS_DIR", repo_root.join("server/assets/maps"))
        .env("RTS_DESKTOP_SHELL", "maccursor")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| shell_error(format!("failed to spawn rts-server through cargo: {err}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| shell_error("failed to capture rts-server stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| shell_error("failed to capture rts-server stderr"))?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || forward_server_output("stdout", stdout, Some(tx)));
    thread::spawn(move || forward_server_output("stderr", stderr, None));

    let deadline = Instant::now() + SERVER_READY_TIMEOUT;
    let url = loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(url) => match normalize_server_url(&url) {
                Ok(url) => break url,
                Err(err) => {
                    stop_child(&mut child);
                    return Err(err);
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                stop_child(&mut child);
                return Err(shell_error(
                    "rts-server output closed before a listen URL was reported",
                ));
            }
        }
        if let Some(status) = child.try_wait()? {
            return Err(shell_error(format!(
                "rts-server exited before startup: {status}"
            )));
        }
        if Instant::now() >= deadline {
            stop_child(&mut child);
            return Err(shell_error(format!(
                "timed out after {}s waiting for rts-server to report its listen URL",
                SERVER_READY_TIMEOUT.as_secs()
            )));
        }
    };

    app.manage(ManagedServer {
        child: Mutex::new(Some(child)),
    });

    Ok(ServerLaunch {
        url,
        mode: "spawned",
    })
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn repo_root() -> ShellResult<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| shell_error("failed to resolve repository root from CARGO_MANIFEST_DIR"))
}

fn normalize_server_url(value: &str) -> ShellResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(shell_error("server URL is empty"));
    }
    let url: tauri::Url = trimmed
        .parse()
        .map_err(|err| shell_error(format!("invalid server URL {trimmed}: {err}")))?;
    if url.scheme() != "http" {
        return Err(shell_error(format!(
            "desktop shell expects an http loopback server URL, got {trimmed}"
        )));
    }
    if !matches!(url.host_str(), Some("127.0.0.1") | Some("localhost")) {
        return Err(shell_error(format!(
            "desktop shell refuses non-loopback server URL {trimmed}"
        )));
    }
    Ok(url.to_string())
}

fn forward_server_output<R: Read + Send + 'static>(
    stream: &'static str,
    reader: R,
    ready_tx: Option<Sender<String>>,
) {
    let mut ready_tx = ready_tx;
    for line in BufReader::new(reader).lines().map_while(Result::ok) {
        eprintln!("[rts-server:{stream}] {line}");
        if let (Some(tx), Some(url)) = (ready_tx.as_ref(), extract_server_url(&line)) {
            let _ = tx.send(url);
            ready_tx = None;
        }
    }
}

fn extract_server_url(line: &str) -> Option<String> {
    let start = line.find("open http://")? + "open ".len();
    line[start..]
        .split_whitespace()
        .next()
        .map(|url| url.trim_end_matches(['.', ',']).to_string())
}

fn desktop_runtime_script(server: &ServerLaunch) -> String {
    format!(
        r#"
(() => {{
  const runtime = Object.freeze({{
    shell: "tauri",
    platform: "macos",
    nativeCursorBackend: true,
    nativeCursorCapture: true,
    pointerLockDisabled: true,
    serverMode: "{mode}",
    serverUrl: "{url}"
  }});
  Object.defineProperty(window, "__RTS_DESKTOP_RUNTIME", {{
    value: runtime,
    configurable: false,
    writable: false
  }});

  const denied = () => Promise.reject(new DOMException(
    "Pointer Lock is disabled in the macOS native-cursor shell.",
    "NotAllowedError"
  ));
  const replace = (target, name) => {{
    if (!target || typeof target[name] !== "function") return;
    try {{
      Object.defineProperty(target, name, {{
        value: denied,
        configurable: true,
        writable: false
      }});
    }} catch {{}}
  }};

  const invoke = (cmd, payload = {{}}) => {{
    const tauriInvoke = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
    if (typeof tauriInvoke !== "function") {{
      return Promise.reject(new Error("Tauri invoke bridge is unavailable."));
    }}
    return tauriInvoke(cmd, payload);
  }};
  const listeners = new Set();
  const diagnostics = {{
    supported: true,
    backend: "native-macos",
    active: false,
    visual: "dom-event-time",
    movementBatched: false,
    nativeEventsReceived: 0,
    jsEventsProcessed: 0,
    droppedEvents: 0,
    backloggedEvents: 0,
    lastSequence: 0,
    lastDeliveryLatencyMs: null,
    lastReason: "ready",
    lastError: null
  }};
  const mergeDiagnostics = (snapshot) => {{
    if (!snapshot || typeof snapshot !== "object") return snapshot;
    diagnostics.active = !!snapshot.active;
    diagnostics.nativeEventsReceived = Number(snapshot.nativeEventsReceived || diagnostics.nativeEventsReceived);
    diagnostics.droppedEvents = Number(snapshot.droppedEvents || diagnostics.droppedEvents);
    diagnostics.lastReason = snapshot.lastReason || diagnostics.lastReason;
    diagnostics.lastError = snapshot.lastError || null;
    return snapshot;
  }};
  const dispatchNativeEvent = (detail) => {{
    if (!detail || typeof detail !== "object") return;
    diagnostics.nativeEventsReceived = Number(detail.nativeEventsReceived || diagnostics.nativeEventsReceived + 1);
    diagnostics.jsEventsProcessed += 1;
    if (Number.isFinite(detail.sequence)) {{
      if (diagnostics.lastSequence && detail.sequence > diagnostics.lastSequence + 1) {{
        diagnostics.droppedEvents += detail.sequence - diagnostics.lastSequence - 1;
      }}
      diagnostics.lastSequence = detail.sequence;
    }}
    if (Number.isFinite(detail.sentAtMs)) {{
      diagnostics.lastDeliveryLatencyMs = Math.max(0, Date.now() - detail.sentAtMs);
    }}
    if (detail.type === "capture") diagnostics.active = false;
    for (const listener of Array.from(listeners)) {{
      try {{
        listener(detail);
      }} catch (err) {{
        diagnostics.lastError = err && err.message ? err.message : String(err);
      }}
    }}
  }};
  Object.defineProperty(window, "__RTS_NATIVE_CURSOR", {{
    value: Object.freeze({{
      supported: () => true,
      backend: "native-macos",
      visual: "dom-event-time",
      start: (bounds = {{}}) => invoke("maccursor_start", {{
        x: Number(bounds.x || 0),
        y: Number(bounds.y || 0),
        width: Number(bounds.width || 0),
        height: Number(bounds.height || 0)
      }}).then((snapshot) => {{
        mergeDiagnostics(snapshot);
        diagnostics.active = !!snapshot?.active;
        return snapshot;
      }}),
      configure: (bounds = {{}}) => invoke("maccursor_configure", {{
        width: Number(bounds.width || 0),
        height: Number(bounds.height || 0)
      }}).then(mergeDiagnostics),
      stop: (reason = "js-stop") => invoke("maccursor_stop", {{ reason }}).then((snapshot) => {{
        mergeDiagnostics(snapshot);
        diagnostics.active = false;
        return snapshot;
      }}),
      diagnostics: () => Object.freeze({{ ...diagnostics }}),
      nativeDiagnostics: () => invoke("maccursor_diagnostics").then(mergeDiagnostics),
      onEvent: (listener) => {{
        if (typeof listener !== "function") return () => {{}};
        listeners.add(listener);
        return () => listeners.delete(listener);
      }},
      __dispatchNativeEvent: dispatchNativeEvent
    }}),
    configurable: false,
    writable: false
  }});

  const elementProto = typeof Element !== "undefined" ? Element.prototype : null;
  const htmlElementProto = typeof HTMLElement !== "undefined" ? HTMLElement.prototype : null;
  replace(elementProto, "requestPointerLock");
  replace(elementProto, "webkitRequestPointerLock");
  replace(htmlElementProto, "requestPointerLock");
  replace(htmlElementProto, "webkitRequestPointerLock");
}})();
"#,
        mode = server.mode,
        url = server.url
    )
}

fn shell_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        message.into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_server_url_from_log_line() {
        let line = "INFO Bewegungskrieg server listening - open http://127.0.0.1:41231/";
        assert_eq!(
            extract_server_url(line).as_deref(),
            Some("http://127.0.0.1:41231/")
        );
    }

    #[test]
    fn normalizes_loopback_server_url() {
        assert_eq!(
            normalize_server_url(" http://localhost:8080/ ").unwrap(),
            "http://localhost:8080/"
        );
    }

    #[test]
    fn rejects_non_loopback_server_url() {
        assert!(normalize_server_url("https://example.com/").is_err());
        assert!(normalize_server_url("http://192.0.2.10:8080/").is_err());
    }

    #[test]
    fn runtime_script_exposes_desktop_flag_and_disables_pointer_lock() {
        let script = desktop_runtime_script(&ServerLaunch {
            url: "http://127.0.0.1:4000/".to_string(),
            mode: "spawned",
        });
        assert!(script.contains("__RTS_DESKTOP_RUNTIME"));
        assert!(script.contains("nativeCursorBackend: true"));
        assert!(script.contains("__RTS_NATIVE_CURSOR"));
        assert!(script.contains("maccursor_start"));
        assert!(script.contains("pointerLockDisabled: true"));
        assert!(script.contains("requestPointerLock"));
    }
}
