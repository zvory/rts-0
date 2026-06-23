#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

mod native_cursor;

use native_cursor::{
    maccursor_configure, maccursor_diagnostics, maccursor_start, maccursor_stop,
    NativeCursorBackend,
};
use serde::Serialize;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const DEFAULT_PROFILE_ID: &str = "beta";
const SERVER_URL_ENV: &str = "RTS_DESKTOP_SERVER_URL";
const STARTUP_ENTRYPOINT: &str = "index.html";
const WINDOW_LABEL: &str = "main";

type ShellResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerProfile {
    id: &'static str,
    label: &'static str,
    url: &'static str,
    summary: &'static str,
}

static BUILT_IN_PROFILES: [ServerProfile; 2] = [
    ServerProfile {
        id: "beta",
        label: "Beta",
        url: "https://rts-0-zvorygin-beta.fly.dev/",
        summary: "Playtest channel",
    },
    ServerProfile {
        id: "mainline",
        label: "Mainline",
        url: "https://rts-0-zvorygin.fly.dev/",
        summary: "Current public release",
    },
];

#[derive(Debug, Clone)]
enum InitialNavigation {
    Startup,
    DeveloperServer { url: String },
}

impl InitialNavigation {
    fn developer_url(&self) -> Option<&str> {
        match self {
            InitialNavigation::Startup => None,
            InitialNavigation::DeveloperServer { url } => Some(url),
        }
    }

    fn webview_url(&self) -> ShellResult<WebviewUrl> {
        match self {
            InitialNavigation::Startup => Ok(WebviewUrl::App(STARTUP_ENTRYPOINT.into())),
            InitialNavigation::DeveloperServer { url } => {
                let url: tauri::Url = url.parse().map_err(|err| {
                    shell_error(format!("invalid developer server URL {url}: {err}"))
                })?;
                Ok(WebviewUrl::External(url))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeveloperServerPolicy {
    Enabled,
    Disabled,
}

impl DeveloperServerPolicy {
    fn current_build() -> Self {
        if cfg!(debug_assertions) {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeScriptOptions {
    developer_server_url: Option<String>,
    autostart: bool,
    autolock: bool,
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
                let initial_navigation = initial_navigation()?;
                let developer_navigation_url = initial_navigation
                    .developer_url()
                    .map(str::parse::<tauri::Url>)
                    .transpose()
                    .map_err(|err| {
                        shell_error(format!(
                            "invalid developer server URL from {SERVER_URL_ENV}: {err}"
                        ))
                    })?;
                let native_cursor = NativeCursorBackend::default();
                app.manage(native_cursor.clone());
                let runtime_script = desktop_runtime_script(&RuntimeScriptOptions {
                    developer_server_url: initial_navigation.developer_url().map(str::to_string),
                    autostart: env_flag("RTS_DESKTOP_AUTOSTART"),
                    autolock: env_flag("RTS_DESKTOP_AUTOLOCK"),
                });
                let initial_webview_url = initial_navigation.webview_url()?;
                let window = WebviewWindowBuilder::new(app, WINDOW_LABEL, initial_webview_url)
                    .title("Bewegungskrieg")
                    .inner_size(1280.0, 820.0)
                    .min_inner_size(960.0, 640.0)
                    .initialization_script(runtime_script)
                    .on_navigation(move |url| {
                        navigation_allowed(url, developer_navigation_url.as_ref())
                    })
                    .build()?;
                native_cursor.install(&window);
                let _ = app
                    .handle()
                    .set_activation_policy(tauri::ActivationPolicy::Regular);
                let _ = window.set_focus();
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

fn initial_navigation() -> ShellResult<InitialNavigation> {
    let policy = DeveloperServerPolicy::current_build();
    if policy == DeveloperServerPolicy::Disabled {
        return Ok(InitialNavigation::Startup);
    }

    match std::env::var(SERVER_URL_ENV) {
        Ok(url) => initial_navigation_from_developer_url(Some(url.as_str()), policy),
        Err(std::env::VarError::NotPresent) => initial_navigation_from_developer_url(None, policy),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(shell_error(format!("{SERVER_URL_ENV} is not valid UTF-8")))
        }
    }
}

fn initial_navigation_from_developer_url(
    value: Option<&str>,
    policy: DeveloperServerPolicy,
) -> ShellResult<InitialNavigation> {
    if policy == DeveloperServerPolicy::Disabled {
        return Ok(InitialNavigation::Startup);
    }

    match value {
        Some(url) => Ok(InitialNavigation::DeveloperServer {
            url: normalize_developer_server_url(url)?,
        }),
        None => Ok(InitialNavigation::Startup),
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref().map(str::trim),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES") | Ok("on") | Ok("ON")
    )
}

fn normalize_developer_server_url(value: &str) -> ShellResult<String> {
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

fn navigation_allowed(url: &tauri::Url, developer_url: Option<&tauri::Url>) -> bool {
    app_url_allowed(url)
        || release_profile_for_url(url).is_some()
        || developer_url
            .map(|developer_url| same_origin(url, developer_url))
            .unwrap_or(false)
}

fn app_url_allowed(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "tauri")
        || matches!(
            (url.scheme(), url.host_str()),
            ("http" | "https", Some("tauri.localhost"))
        )
}

fn release_profile_for_url(url: &tauri::Url) -> Option<&'static ServerProfile> {
    BUILT_IN_PROFILES.iter().find(|profile| {
        profile
            .url
            .parse::<tauri::Url>()
            .map(|profile_url| same_origin(url, &profile_url))
            .unwrap_or(false)
    })
}

fn same_origin(left: &tauri::Url, right: &tauri::Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn desktop_runtime_script(options: &RuntimeScriptOptions) -> String {
    let autostart_script = if options.autostart {
        gated_automation_script(desktop_autostart_script())
    } else {
        String::new()
    };
    let autolock_script = if options.autolock {
        gated_automation_script(desktop_autolock_script())
    } else {
        String::new()
    };
    let profiles_json =
        serde_json::to_string(&BUILT_IN_PROFILES).expect("built-in profiles serialize");
    let default_profile_id =
        serde_json::to_string(DEFAULT_PROFILE_ID).expect("default profile id serializes");
    let developer_server_url =
        serde_json::to_string(&options.developer_server_url).expect("developer URL serializes");
    format!(
        r#"
(() => {{
  const profiles = Object.freeze({profiles}.map((profile) => Object.freeze({{ ...profile }})));
  const defaultProfileId = {default_profile_id};
  const developerServerUrl = {developer_server_url};
  const parseUrl = (value) => {{
    try {{
      return new URL(value);
    }} catch {{
      return null;
    }}
  }};
  const sameOrigin = (left, right) => {{
    if (!left || !right) return false;
    return left.protocol === right.protocol && left.hostname === right.hostname && left.port === right.port;
  }};
  const currentUrl = parseUrl(window.location.href);
  const selectedProfile = profiles.find((profile) => sameOrigin(currentUrl, parseUrl(profile.url))) || null;
  const developerSelected = !!developerServerUrl && sameOrigin(currentUrl, parseUrl(developerServerUrl));

  Object.defineProperty(window, "__RTS_DESKTOP_STARTUP", {{
    value: Object.freeze({{
      profiles,
      defaultProfileId,
      developerServerUrl
    }}),
    configurable: false,
    writable: false
  }});

  const runtime = Object.freeze({{
    shell: "tauri",
    platform: "macos",
    nativeCursorBackend: true,
    nativeCursorCapture: true,
    pointerLockDisabled: true,
    autostart: {autostart},
    autolock: {autolock},
    serverMode: selectedProfile ? "release" : developerSelected ? "developer" : "startup",
    serverUrl: selectedProfile ? selectedProfile.url : developerSelected ? developerServerUrl : null,
    releaseChannel: selectedProfile ? selectedProfile.id : null
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
  const invoke = (cmd, payload = {{}}) => {{
    const candidates = [
      window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke,
      window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke,
      window.__TAURI__ && window.__TAURI__.tauri && window.__TAURI__.tauri.invoke,
      window.__TAURI__ && window.__TAURI__.invoke
    ];
    const tauriInvoke = candidates.find((candidate) => typeof candidate === "function");
    if (typeof tauriInvoke !== "function") {{
      diagnostics.lastError = "Tauri invoke bridge is unavailable.";
      return Promise.reject(new Error(diagnostics.lastError));
    }}
    return tauriInvoke(cmd, payload);
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
      }}).catch((err) => {{
        diagnostics.active = false;
        diagnostics.lastError = err && err.message ? err.message : String(err);
        diagnostics.lastReason = "capture-start-failed";
        throw err;
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

{autostart_script}
{autolock_script}
}})();
"#,
        autostart = options.autostart,
        autolock = options.autolock,
        autostart_script = autostart_script,
        autolock_script = autolock_script,
        default_profile_id = default_profile_id,
        developer_server_url = developer_server_url,
        profiles = profiles_json
    )
}

fn gated_automation_script(script: &'static str) -> String {
    format!(
        r#"
  if (runtime.serverMode !== "startup") {{
{script}
  }}
"#
    )
}

fn desktop_autostart_script() -> &'static str {
    r##"
  const desktopAutostartSleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const desktopAutostartWaitFor = async (probe, timeoutMs = 10000) => {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const value = probe();
      if (value) return value;
      await desktopAutostartSleep(100);
    }
    throw new Error("desktop autostart timed out");
  };
  const desktopAutostartNote = (message) => {
    diagnostics.lastReason = `autostart:${message}`;
    window.__RTS_DESKTOP_AUTOSTART_STATUS = message;
    console.warn("[RTS_DESKTOP_AUTOSTART]", message);
  };
  const desktopAutostartInput = (el, value) => {
    if (!el) return;
    el.value = value;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  };
  const desktopAutostart = async () => {
    desktopAutostartNote("waiting-for-lobby");
    await desktopAutostartWaitFor(() => document.querySelector("#lobby-name"));
    await desktopAutostartSleep(250);
    const room = `Desktop Cursor ${Date.now().toString(36)}`;
    desktopAutostartInput(document.querySelector("#lobby-name"), "Commander");
    desktopAutostartInput(document.querySelector("#lobby-room"), room);
    desktopAutostartNote(`creating:${room}`);
    const response = await fetch("/api/lobbies", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ room })
    });
    if (!response.ok && response.status !== 409) {
      throw new Error(`lobby create failed ${response.status}`);
    }
    document.querySelector("#lobby-join")?.click();
    desktopAutostartNote(`joining:${room}`);
    const startButton = await desktopAutostartWaitFor(() => {
      const button = document.querySelector("#lobby-start");
      return button && !button.disabled ? button : null;
    }).catch(async () => {
      const ready = document.querySelector("#lobby-ready");
      if (ready && !ready.disabled) ready.click();
      return await desktopAutostartWaitFor(() => {
        const button = document.querySelector("#lobby-start");
        return button && !button.disabled ? button : null;
      });
    });
    startButton.click();
    desktopAutostartNote(`started:${room}`);
  };
  const runDesktopAutostart = () => {
    void desktopAutostart().catch((err) => {
      diagnostics.lastError = err && err.message ? err.message : String(err);
      diagnostics.lastReason = "autostart-failed";
      console.error("[RTS_DESKTOP_AUTOSTART]", diagnostics.lastError);
    });
  };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", runDesktopAutostart, { once: true });
  } else {
    runDesktopAutostart();
  }
"##
}

fn desktop_autolock_script() -> &'static str {
    r##"
  const runDesktopAutolock = () => {
    void (async () => {
      const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
      const waitFor = async (probe, timeoutMs = 10000) => {
        const deadline = Date.now() + timeoutMs;
        while (Date.now() < deadline) {
          const value = probe();
          if (value) return value;
          await sleep(100);
        }
        throw new Error("desktop autolock timed out");
      };
      const note = (message) => {
        diagnostics.lastReason = `autolock:${message}`;
        window.__RTS_DESKTOP_AUTOLOCK_STATUS = message;
        console.warn("[RTS_DESKTOP_AUTOLOCK]", message);
      };
      note("waiting-for-match");
      await waitFor(() => {
        const screen = document.querySelector("#game-screen");
        return screen && !screen.hidden ? screen : null;
      }, 15000);
      await waitFor(() => document.querySelector("#viewport canvas") || document.querySelector("#viewport"), 5000);
      await sleep(500);
      const settingsButton = await waitFor(() => {
        const button = document.querySelector("#settings-button");
        return button && !button.disabled ? button : null;
      }, 5000);
      settingsButton.click();
      note("settings-opened");
      const lockButton = await waitFor(() => {
        const button = document.querySelector("#pointer-lock-toggle");
        return button && !button.hidden && !button.disabled ? button : null;
      }, 5000);
      lockButton.click();
      note("cursor-lock-clicked");
    })().catch((err) => {
      diagnostics.lastError = err && err.message ? err.message : String(err);
      diagnostics.lastReason = "autolock-failed";
      console.error("[RTS_DESKTOP_AUTOLOCK]", diagnostics.lastError);
    });
  };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", runDesktopAutolock, { once: true });
  } else {
    runDesktopAutolock();
  }
"##
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
    fn built_in_profiles_match_release_urls() {
        assert_eq!(BUILT_IN_PROFILES.len(), 2);
        assert_eq!(BUILT_IN_PROFILES[0].id, "beta");
        assert_eq!(
            BUILT_IN_PROFILES[0].url,
            "https://rts-0-zvorygin-beta.fly.dev/"
        );
        assert_eq!(BUILT_IN_PROFILES[1].id, "mainline");
        assert_eq!(BUILT_IN_PROFILES[1].url, "https://rts-0-zvorygin.fly.dev/");
        for profile in BUILT_IN_PROFILES.iter() {
            let url: tauri::Url = profile.url.parse().unwrap();
            assert_eq!(url.scheme(), "https");
        }
        assert!(BUILT_IN_PROFILES
            .iter()
            .any(|profile| profile.id == DEFAULT_PROFILE_ID));
    }

    #[test]
    fn shipped_profiles_are_remote_urls_without_local_server_command() {
        let script = desktop_runtime_script(&RuntimeScriptOptions {
            developer_server_url: None,
            autostart: false,
            autolock: false,
        });

        assert!(script.contains("const developerServerUrl = null"));
        for profile in BUILT_IN_PROFILES.iter() {
            let url: tauri::Url = profile.url.parse().unwrap();
            assert_eq!(url.scheme(), "https");
            assert!(!matches!(
                url.host_str(),
                Some("127.0.0.1") | Some("localhost")
            ));
        }
        for forbidden in ["127.0.0.1", "localhost", "rts-server", "cargo run"] {
            assert!(
                !script.contains(forbidden),
                "release startup runtime should not reference {forbidden}"
            );
        }
    }

    #[test]
    fn bundle_config_excludes_game_runtime_assets() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["build"]["frontendDist"], "../ui");

        let bundle = &config["bundle"];
        assert!(bundle.get("externalBin").is_none());
        assert!(bundle
            .get("resources")
            .and_then(serde_json::Value::as_array)
            .map(|resources| resources.is_empty())
            .unwrap_or(true));

        let raw_config = include_str!("../tauri.conf.json");
        for forbidden in [
            "rts-server",
            "../client",
            "../../client",
            "maps",
            "lab-scenarios",
            "match-history",
            "server/Cargo.toml",
        ] {
            assert!(
                !raw_config.contains(forbidden),
                "bundle config should not include game asset path {forbidden}"
            );
        }
    }

    #[test]
    fn defaults_to_startup_selector_without_developer_url() {
        let navigation =
            initial_navigation_from_developer_url(None, DeveloperServerPolicy::Enabled).unwrap();
        assert!(matches!(navigation, InitialNavigation::Startup));
        assert_eq!(navigation.developer_url(), None);
        assert_eq!(
            navigation.webview_url().unwrap().to_string(),
            STARTUP_ENTRYPOINT
        );
    }

    #[test]
    fn normalizes_loopback_developer_server_url() {
        assert_eq!(
            normalize_developer_server_url(" http://localhost:8080/ ").unwrap(),
            "http://localhost:8080/"
        );
        let navigation = initial_navigation_from_developer_url(
            Some(" http://127.0.0.1:41231/ "),
            DeveloperServerPolicy::Enabled,
        )
        .unwrap();
        assert_eq!(navigation.developer_url(), Some("http://127.0.0.1:41231/"));
    }

    #[test]
    fn packaged_policy_ignores_developer_server_url_override() {
        let navigation = initial_navigation_from_developer_url(
            Some("http://127.0.0.1:41231/"),
            DeveloperServerPolicy::Disabled,
        )
        .unwrap();
        assert!(matches!(navigation, InitialNavigation::Startup));
        assert_eq!(navigation.developer_url(), None);
        assert_eq!(
            navigation.webview_url().unwrap().to_string(),
            STARTUP_ENTRYPOINT
        );
    }

    #[test]
    fn rejects_non_loopback_developer_server_url() {
        assert!(normalize_developer_server_url("https://example.com/").is_err());
        assert!(normalize_developer_server_url("http://192.0.2.10:8080/").is_err());
    }

    #[test]
    fn navigation_policy_allows_startup_release_and_developer_origins() {
        let startup_url: tauri::Url = "tauri://localhost/index.html".parse().unwrap();
        assert!(navigation_allowed(&startup_url, None));

        let beta_url: tauri::Url = "https://rts-0-zvorygin-beta.fly.dev/rooms".parse().unwrap();
        let mainline_url: tauri::Url = "https://rts-0-zvorygin.fly.dev/?room=test".parse().unwrap();
        assert!(navigation_allowed(&beta_url, None));
        assert!(navigation_allowed(&mainline_url, None));

        let developer_url: tauri::Url = "http://localhost:41231/".parse().unwrap();
        let developer_path: tauri::Url = "http://localhost:41231/play".parse().unwrap();
        let other_port: tauri::Url = "http://localhost:41232/play".parse().unwrap();
        let unrelated: tauri::Url = "https://example.com/".parse().unwrap();
        assert!(navigation_allowed(&developer_path, Some(&developer_url)));
        assert!(!navigation_allowed(&other_port, Some(&developer_url)));
        assert!(!navigation_allowed(&unrelated, None));
    }

    #[test]
    fn runtime_script_exposes_desktop_flag_and_disables_pointer_lock() {
        let script = desktop_runtime_script(&RuntimeScriptOptions {
            developer_server_url: Some("http://127.0.0.1:4000/".to_string()),
            autostart: false,
            autolock: false,
        });
        assert!(script.contains("__RTS_DESKTOP_STARTUP"));
        assert!(script.contains("__RTS_DESKTOP_RUNTIME"));
        assert!(script.contains("https://rts-0-zvorygin-beta.fly.dev/"));
        assert!(script.contains("https://rts-0-zvorygin.fly.dev/"));
        assert!(script.contains("const defaultProfileId = \"beta\""));
        assert!(script.contains("nativeCursorBackend: true"));
        assert!(script.contains("autostart: false"));
        assert!(script.contains("autolock: false"));
        assert!(script.contains("serverMode: selectedProfile ? \"release\""));
        assert!(script.contains("releaseChannel: selectedProfile ? selectedProfile.id : null"));
        assert!(script.contains("http://127.0.0.1:4000/"));
        assert!(script.contains("__RTS_NATIVE_CURSOR"));
        assert!(script.contains("maccursor_start"));
        assert!(script.contains("__TAURI__.core"));
        assert!(script.contains("capture-start-failed"));
        assert!(script.contains("pointerLockDisabled: true"));
        assert!(script.contains("requestPointerLock"));
    }
}
