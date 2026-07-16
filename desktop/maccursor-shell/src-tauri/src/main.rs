#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    error::Error,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

mod diagnostics;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod native_cursor;

use diagnostics::{bounded_log_text, ShellDiagnostics, ShellLogInfo};
#[cfg(target_os = "macos")]
use native_cursor::{
    maccursor_configure, maccursor_diagnostics, maccursor_start, maccursor_stop,
    NativeCursorBackend,
};
use serde::Serialize;
use serde_json::json;
use tauri::{
    webview::{PageLoadEvent, PageLoadPayload},
    Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

const DEFAULT_PROFILE_ID: &str = "beta";
const SERVER_URL_ENV: &str = "RTS_DESKTOP_SERVER_URL";
const STARTUP_ENTRYPOINT: &str = "index.html";
const STARTUP_ERROR_URL: &str = "tauri://localhost/index.html";
const WINDOW_LABEL: &str = "main";
const NAVIGATION_LOAD_TIMEOUT_MS: u64 = 15_000;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellPlatform {
    Macos,
    Windows,
    Other,
}

impl ShellPlatform {
    fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Other
        }
    }

    fn runtime_name(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimePolicy {
    platform: ShellPlatform,
    native_cursor_backend: bool,
    native_cursor_capture: bool,
    pointer_lock_disabled: bool,
    aggressive_cursor_lock: bool,
}

impl RuntimePolicy {
    fn for_platform(platform: ShellPlatform) -> Self {
        let native_cursor = platform == ShellPlatform::Macos;
        Self {
            platform,
            native_cursor_backend: native_cursor,
            native_cursor_capture: native_cursor,
            pointer_lock_disabled: native_cursor,
            aggressive_cursor_lock: native_cursor,
        }
    }

    fn current() -> Self {
        Self::for_platform(ShellPlatform::current())
    }
}

#[derive(Debug, Clone)]
struct RuntimeScriptOptions {
    policy: RuntimePolicy,
    developer_server_url: Option<String>,
    autostart: bool,
    autolock: bool,
}

#[derive(Clone, Default)]
struct NavigationMonitor {
    inner: Arc<Mutex<NavigationMonitorState>>,
}

#[derive(Default)]
struct NavigationMonitorState {
    sequence: u64,
    pending: Option<PendingNavigation>,
}

#[derive(Clone, Debug)]
struct PendingNavigation {
    sequence: u64,
    url: String,
    profile_id: Option<&'static str>,
}

impl NavigationMonitor {
    fn start(&self, url: String, profile_id: Option<&'static str>) -> Option<PendingNavigation> {
        let mut state = self.inner.lock().ok()?;
        state.sequence = state.sequence.saturating_add(1);
        let pending = PendingNavigation {
            sequence: state.sequence,
            url,
            profile_id,
        };
        state.pending = Some(pending.clone());
        Some(pending)
    }

    fn clear(&self) -> Option<PendingNavigation> {
        self.inner.lock().ok()?.pending.take()
    }

    fn clear_sequence(&self, sequence: u64) -> Option<PendingNavigation> {
        let mut state = self.inner.lock().ok()?;
        let pending = state.pending.as_ref()?;
        if pending.sequence == sequence {
            return state.pending.take();
        }
        None
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("maccursor-shell failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> ShellResult<()> {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        maccursor_start,
        maccursor_configure,
        maccursor_stop,
        maccursor_diagnostics,
        desktop_log_info,
        desktop_reveal_logs,
        desktop_log_client_event,
        desktop_open_profile
    ]);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        desktop_log_info,
        desktop_reveal_logs,
        desktop_log_client_event,
        desktop_open_profile
    ]);

    builder
        .setup(|app| {
            let diagnostics = ShellDiagnostics::open(app.path().app_log_dir().map_err(|err| {
                shell_error(format!("failed to resolve app log directory: {err}"))
            })?)
            .map_err(shell_error)?;
            app.manage(diagnostics.clone());

            let initial_navigation = initial_navigation()?;
            log_shell_start(&diagnostics, &initial_navigation);
            log_startup_configuration(&diagnostics);
            let developer_navigation_url = initial_navigation
                .developer_url()
                .map(str::parse::<tauri::Url>)
                .transpose()
                .map_err(|err| {
                    shell_error(format!(
                        "invalid developer server URL from {SERVER_URL_ENV}: {err}"
                    ))
                })?;
            #[cfg(target_os = "macos")]
            let native_cursor = {
                let native_cursor = NativeCursorBackend::with_diagnostics(diagnostics.clone());
                app.manage(native_cursor.clone());
                native_cursor
            };
            let runtime_script = desktop_runtime_script(&RuntimeScriptOptions {
                policy: RuntimePolicy::current(),
                developer_server_url: initial_navigation.developer_url().map(str::to_string),
                autostart: env_flag("RTS_DESKTOP_AUTOSTART"),
                autolock: env_flag("RTS_DESKTOP_AUTOLOCK"),
            });
            let initial_webview_url = initial_navigation.webview_url()?;
            let navigation_monitor = NavigationMonitor::default();
            let navigation_policy_diagnostics = diagnostics.clone();
            let page_load_diagnostics = diagnostics.clone();
            let page_load_monitor = navigation_monitor.clone();
            let window = WebviewWindowBuilder::new(app, WINDOW_LABEL, initial_webview_url)
                .title("Bewegungskrieg")
                .inner_size(1280.0, 820.0)
                .min_inner_size(960.0, 640.0)
                .initialization_script(runtime_script)
                .on_navigation(move |url| {
                    let allowed = navigation_allowed(url, developer_navigation_url.as_ref());
                    if !allowed {
                        navigation_policy_diagnostics.log_event(
                            "navigation_rejected",
                            json!({ "url": redact_url_for_log(url.as_str()) }),
                        );
                    }
                    allowed
                })
                .on_page_load(move |window, payload| {
                    handle_page_load(&page_load_diagnostics, &page_load_monitor, &window, payload);
                })
                .build()?;
            #[cfg(target_os = "macos")]
            {
                native_cursor.install(&window);
                let _ = app
                    .handle()
                    .set_activation_policy(tauri::ActivationPolicy::Regular);
            }
            let _ = window.set_focus();
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != WINDOW_LABEL {
                return;
            }
            match event {
                WindowEvent::Focused(false) => {
                    #[cfg(target_os = "macos")]
                    let _ = window.state::<NativeCursorBackend>().stop("window blur");
                }
                WindowEvent::CloseRequested { .. } => {
                    #[cfg(target_os = "macos")]
                    let _ = window.state::<NativeCursorBackend>().stop("window closed");
                    window.app_handle().exit(0);
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())?;
    Ok(())
}

#[tauri::command]
fn desktop_log_info(
    window: WebviewWindow,
    diagnostics: tauri::State<'_, ShellDiagnostics>,
) -> Result<ShellLogInfo, String> {
    ensure_startup_context(&window)?;
    Ok(diagnostics.log_info())
}

#[tauri::command]
fn desktop_reveal_logs(
    window: WebviewWindow,
    diagnostics: tauri::State<'_, ShellDiagnostics>,
) -> Result<(), String> {
    ensure_startup_context(&window)?;
    std::fs::create_dir_all(diagnostics.log_dir())
        .map_err(|err| format!("failed to create shell log directory: {err}"))?;
    let reveal_program = log_reveal_program(ShellPlatform::current())?;
    std::process::Command::new(reveal_program)
        .arg(diagnostics.log_dir())
        .spawn()
        .map_err(|err| format!("failed to reveal shell log directory: {err}"))?;
    diagnostics.log_event("log_directory_revealed", json!({}));
    Ok(())
}

fn log_reveal_program(platform: ShellPlatform) -> Result<&'static str, String> {
    match platform {
        ShellPlatform::Macos => Ok("open"),
        ShellPlatform::Windows => Ok("explorer.exe"),
        ShellPlatform::Other => Err(
            "revealing the shell log directory is not supported on this desktop platform"
                .to_string(),
        ),
    }
}

#[tauri::command]
fn desktop_log_client_event(
    window: WebviewWindow,
    diagnostics: tauri::State<'_, ShellDiagnostics>,
    event: String,
    message: Option<String>,
    url: Option<String>,
) -> Result<(), String> {
    let current_url = window
        .url()
        .ok()
        .map(|url| redact_url_for_log(url.as_str()));
    diagnostics.log_event(
        "client_runtime_event",
        json!({
            "source": bounded_log_text(&event),
            "message": message.as_deref().map(bounded_log_text),
            "url": url.as_deref().map(redact_url_for_log),
            "currentUrl": current_url,
        }),
    );
    Ok(())
}

#[tauri::command]
fn desktop_open_profile(
    window: WebviewWindow,
    diagnostics: tauri::State<'_, ShellDiagnostics>,
    profile_id: String,
) -> Result<(), String> {
    ensure_startup_context(&window)?;
    let profile = server_profile_for_id(&profile_id)
        .ok_or_else(|| format!("unknown release channel {profile_id}"))?;
    let url: tauri::Url = profile.url.parse::<tauri::Url>().map_err(|err| {
        let message = format!("built-in release channel {} has an invalid URL", profile.id);
        diagnostics.log_event(
            "startup_profile_invalid",
            json!({
                "profileId": profile.id,
                "message": err.to_string(),
            }),
        );
        message
    })?;
    if !navigation_allowed(&url, None) {
        diagnostics.log_event(
            "navigation_rejected",
            json!({
                "profileId": profile.id,
                "url": redact_url_for_log(profile.url),
            }),
        );
        return Err(format!(
            "release channel {} is not allowed by the shell navigation policy",
            profile.label
        ));
    }

    diagnostics.log_event(
        "selected_profile",
        json!({
            "profileId": profile.id,
            "label": profile.label,
            "url": redact_url_for_log(profile.url),
        }),
    );
    window
        .navigate(url)
        .map_err(|err| format!("failed to open release channel {}: {err}", profile.label))
}

fn ensure_startup_context(window: &WebviewWindow) -> Result<(), String> {
    let url = window
        .url()
        .map_err(|err| format!("failed to read current WebView URL: {err}"))?;
    if app_url_allowed(&url) {
        Ok(())
    } else {
        Err("log-path commands are available only on the startup or shell error screen".to_string())
    }
}

fn server_profile_for_id(profile_id: &str) -> Option<&'static ServerProfile> {
    BUILT_IN_PROFILES
        .iter()
        .find(|profile| profile.id == profile_id)
}

fn log_shell_start(diagnostics: &ShellDiagnostics, initial_navigation: &InitialNavigation) {
    diagnostics.log_event(
        "shell_start",
        json!({
            "appVersion": env!("CARGO_PKG_VERSION"),
            "buildId": shell_build_id(),
            "packagingMode": if cfg!(debug_assertions) { "dev" } else { "packaged" },
            "initialMode": match initial_navigation {
                InitialNavigation::Startup => "startup",
                InitialNavigation::DeveloperServer { .. } => "developer",
            },
            "developerServerUrl": initial_navigation
                .developer_url()
                .map(redact_url_for_log),
        }),
    );
}

fn log_startup_configuration(diagnostics: &ShellDiagnostics) {
    let profiles: Vec<_> = BUILT_IN_PROFILES
        .iter()
        .map(|profile| {
            let parsed = profile.url.parse::<tauri::Url>();
            if let Err(err) = &parsed {
                diagnostics.log_event(
                    "startup_profile_invalid",
                    json!({
                        "profileId": profile.id,
                        "label": profile.label,
                        "message": err.to_string(),
                    }),
                );
            }
            json!({
                "profileId": profile.id,
                "label": profile.label,
                "url": parsed
                    .as_ref()
                    .map(|url| redact_url_for_log(url.as_str()))
                    .unwrap_or_else(|_| "<invalid-url>".to_string()),
                "valid": parsed.is_ok(),
            })
        })
        .collect();
    diagnostics.log_event(
        "startup_profiles_configured",
        json!({
            "defaultProfileId": DEFAULT_PROFILE_ID,
            "profiles": profiles,
        }),
    );
}

fn handle_page_load(
    diagnostics: &ShellDiagnostics,
    monitor: &NavigationMonitor,
    window: &WebviewWindow,
    payload: PageLoadPayload<'_>,
) {
    let url = payload.url();
    if app_url_allowed(url) {
        let _ = monitor.clear();
        return;
    }

    match payload.event() {
        PageLoadEvent::Started => {
            let profile_id = release_profile_for_url(url).map(|profile| profile.id);
            let redacted_url = redact_url_for_log(url.as_str());
            diagnostics.log_event(
                "navigation_started",
                json!({
                    "profileId": profile_id,
                    "url": redacted_url,
                }),
            );
            if let Some(pending) = monitor.start(redacted_url, profile_id) {
                spawn_navigation_timeout(
                    diagnostics.clone(),
                    monitor.clone(),
                    window.clone(),
                    pending,
                );
            }
        }
        PageLoadEvent::Finished => {
            let pending = monitor.clear();
            diagnostics.log_event(
                "navigation_finished",
                json!({
                    "profileId": pending.as_ref().and_then(|pending| pending.profile_id),
                    "url": redact_url_for_log(url.as_str()),
                }),
            );
        }
    }
}

fn spawn_navigation_timeout(
    diagnostics: ShellDiagnostics,
    monitor: NavigationMonitor,
    window: WebviewWindow,
    pending: PendingNavigation,
) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(NAVIGATION_LOAD_TIMEOUT_MS));
        let Some(current) = monitor.clear_sequence(pending.sequence) else {
            return;
        };
        let message = startup_failure_message("load-timeout");
        diagnostics.log_event(
            "navigation_timeout",
            json!({
                "profileId": current.profile_id,
                "url": current.url,
                "timeoutMs": NAVIGATION_LOAD_TIMEOUT_MS,
                "message": message,
            }),
        );
        match startup_failure_url("load-timeout", message, Some(&current.url)) {
            Ok(url) => {
                if let Err(err) = window.navigate(url) {
                    diagnostics.log_event(
                        "startup_failure_navigation_failed",
                        json!({ "message": err.to_string() }),
                    );
                }
            }
            Err(err) => diagnostics.log_event(
                "startup_failure_navigation_failed",
                json!({ "message": err }),
            ),
        }
    });
}

fn startup_failure_message(code: &str) -> &'static str {
    match code {
        "load-timeout" => {
            "The selected release channel did not finish loading. Check network connectivity or try another channel."
        }
        _ => "The desktop shell could not open the selected release channel.",
    }
}

fn startup_failure_url(code: &str, message: &str, url: Option<&str>) -> Result<tauri::Url, String> {
    let mut startup_url =
        tauri::Url::parse(STARTUP_ERROR_URL).map_err(|err| format!("bad startup URL: {err}"))?;
    {
        let mut query = startup_url.query_pairs_mut();
        query
            .append_pair("failure", code)
            .append_pair("message", message);
        if let Some(url) = url {
            query.append_pair("url", &redact_url_for_log(url));
        }
    }
    Ok(startup_url)
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

fn redact_url_for_log(value: &str) -> String {
    match value.parse::<tauri::Url>() {
        Ok(url) if url.scheme() == "tauri" => "tauri://localhost/index.html".to_string(),
        Ok(url) => {
            let host = url.host_str().unwrap_or("<no-host>");
            let port = url
                .port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default();
            let path = if url.path().is_empty() {
                "/"
            } else {
                url.path()
            };
            format!("{}://{}{}{}", url.scheme(), host, port, path)
        }
        Err(_) => bounded_log_text(value),
    }
}

fn shell_build_id() -> Option<String> {
    std::env::var("RTS_DESKTOP_BUILD_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| option_env!("GITHUB_SHA").map(str::to_string))
        .or_else(|| option_env!("VERGEN_GIT_SHA").map(str::to_string))
        .map(|value| bounded_log_text(&value))
}

const NATIVE_CURSOR_SCRIPT_BEGIN: &str = "/* RTS_NATIVE_CURSOR_BEGIN */";
const NATIVE_CURSOR_SCRIPT_END: &str = "/* RTS_NATIVE_CURSOR_END */";

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
    let mut script = format!(
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
    platform: {platform},
    nativeCursorBackend: {native_cursor_backend},
    nativeCursorCapture: {native_cursor_capture},
    pointerLockDisabled: {pointer_lock_disabled},
    aggressiveCursorLock: {aggressive_cursor_lock},
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

  const diagnostics = {{
    supported: runtime.nativeCursorBackend,
    backend: runtime.nativeCursorBackend ? "native-macos" : "browser-raw",
    active: false,
    visual: runtime.nativeCursorBackend ? "dom-event-time" : null,
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
      const message = "Tauri invoke bridge is unavailable.";
      diagnostics.lastError = message;
      return Promise.reject(new Error(message));
    }}
    return tauriInvoke(cmd, payload);
  }};
  const desktopLogEvent = (event, message) => {{
    const safeMessage = message == null ? null : String(message).slice(0, 600);
    return invoke("desktop_log_client_event", {{
      event,
      message: safeMessage,
      url: window.location.href
    }}).catch(() => {{}});
  }};
  const sameOriginTargetBlankUrl = (anchor) => {{
    if (!anchor || String(anchor.target || "").toLowerCase() !== "_blank") return null;
    const href = anchor.getAttribute("href") || "";
    if (!href || anchor.hasAttribute("download")) return null;
    try {{
      const url = new URL(href, window.location.href);
      return url.origin === window.location.origin ? url : null;
    }} catch {{
      return null;
    }}
  }};
  const redirectSameOriginTargetBlank = (event) => {{
    if (event.defaultPrevented || event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const target = event.target;
    const element = target && typeof target.closest === "function" ? target : target && target.parentElement;
    const anchor = element && typeof element.closest === "function" ? element.closest("a[target]") : null;
    const url = sameOriginTargetBlankUrl(anchor);
    if (!url) return;
    event.preventDefault();
    void desktopLogEvent("desktop_same_origin_target_blank", url.pathname);
    window.location.assign(url.href);
  }};
  if (typeof document !== "undefined") {{
    document.addEventListener("click", redirectSameOriginTargetBlank, true);
  }}

  /* RTS_NATIVE_CURSOR_BEGIN */
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
  const showDesktopShellFailure = (message) => {{
    if (document.getElementById("rts-desktop-shell-failure")) return;
    const panel = document.createElement("aside");
    panel.id = "rts-desktop-shell-failure";
    panel.setAttribute("role", "status");
    panel.style.cssText = [
      "position:fixed",
      "left:16px",
      "bottom:16px",
      "z-index:2147483647",
      "max-width:min(420px,calc(100vw - 32px))",
      "padding:12px 14px",
      "border:1px solid rgba(154,47,47,.35)",
      "border-radius:8px",
      "background:#fff",
      "color:#181a1b",
      "box-shadow:0 12px 36px rgba(0,0,0,.18)",
      "font:13px/1.35 system-ui,-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif"
    ].join(";");
    panel.textContent = message;
    document.body?.append(panel);
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
        void desktopLogEvent("native_cursor_capture_start_failed", diagnostics.lastError);
        showDesktopShellFailure("Native cursor capture failed. Logs are available from the startup screen.");
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
  /* RTS_NATIVE_CURSOR_END */

{autostart_script}
{autolock_script}
}})();
"#,
        autostart = options.autostart,
        autolock = options.autolock,
        platform = serde_json::to_string(options.policy.platform.runtime_name())
            .expect("platform name serializes"),
        native_cursor_backend = options.policy.native_cursor_backend,
        native_cursor_capture = options.policy.native_cursor_capture,
        pointer_lock_disabled = options.policy.pointer_lock_disabled,
        aggressive_cursor_lock = options.policy.aggressive_cursor_lock,
        autostart_script = autostart_script,
        autolock_script = autolock_script,
        default_profile_id = default_profile_id,
        developer_server_url = developer_server_url,
        profiles = profiles_json
    );
    if !options.policy.native_cursor_backend {
        let start = script
            .find(NATIVE_CURSOR_SCRIPT_BEGIN)
            .expect("native cursor script start marker exists");
        let end = script
            .find(NATIVE_CURSOR_SCRIPT_END)
            .expect("native cursor script end marker exists")
            + NATIVE_CURSOR_SCRIPT_END.len();
        script.replace_range(start..end, "");
    }
    script
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
      void desktopLogEvent("desktop_autostart_failed", diagnostics.lastError);
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
      if (lockButton.getAttribute("aria-checked") === "true") {
        note("cursor-lock-already-active");
      } else {
        lockButton.click();
        note("cursor-lock-clicked");
      }
    })().catch((err) => {
      diagnostics.lastError = err && err.message ? err.message : String(err);
      diagnostics.lastReason = "autolock-failed";
      void desktopLogEvent("desktop_autolock_failed", diagnostics.lastError);
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
            policy: RuntimePolicy::for_platform(ShellPlatform::Macos),
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
        assert_eq!(config["productName"], "Bewegungskrieg");
        assert_eq!(config["identifier"], "dev.bewegungskrieg.Bewegungskrieg");
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
        let beta_lab_url: tauri::Url =
            "https://rts-0-zvorygin-beta.fly.dev/lab?room=default&map=Chokes"
                .parse()
                .unwrap();
        let mainline_url: tauri::Url = "https://rts-0-zvorygin.fly.dev/?room=test".parse().unwrap();
        assert!(navigation_allowed(&beta_url, None));
        assert!(navigation_allowed(&beta_lab_url, None));
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
    fn redacts_query_strings_from_logged_urls() {
        assert_eq!(
            redact_url_for_log("https://rts-0-zvorygin-beta.fly.dev/play?token=secret#frag"),
            "https://rts-0-zvorygin-beta.fly.dev/play"
        );
        assert_eq!(
            redact_url_for_log("tauri://localhost/index.html?message=secret"),
            "tauri://localhost/index.html"
        );
    }

    #[test]
    fn startup_failure_url_formats_error_state_without_raw_query() {
        let message = startup_failure_message("load-timeout");
        let url = startup_failure_url(
            "load-timeout",
            message,
            Some("https://rts-0-zvorygin-beta.fly.dev/?token=secret"),
        )
        .unwrap();
        assert_eq!(url.scheme(), "tauri");
        assert_eq!(url.host_str(), Some("localhost"));
        assert!(url.as_str().contains("failure=load-timeout"));
        assert!(url.as_str().contains("message="));
        assert!(!url.as_str().contains("secret"));
        assert!(message.contains("network connectivity"));
    }

    #[test]
    fn macos_runtime_script_exposes_native_cursor_and_disables_pointer_lock() {
        let script = desktop_runtime_script(&RuntimeScriptOptions {
            policy: RuntimePolicy::for_platform(ShellPlatform::Macos),
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
        assert!(script.contains("platform: \"macos\""));
        assert!(script.contains("aggressiveCursorLock: true"));
        assert!(script.contains("autostart: false"));
        assert!(script.contains("autolock: false"));
        assert!(script.contains("serverMode: selectedProfile ? \"release\""));
        assert!(script.contains("releaseChannel: selectedProfile ? selectedProfile.id : null"));
        assert!(script.contains("http://127.0.0.1:4000/"));
        assert!(script.contains("__RTS_NATIVE_CURSOR"));
        assert!(script.contains("maccursor_start"));
        assert!(script.contains("desktop_log_client_event"));
        assert!(script.contains("sameOriginTargetBlankUrl"));
        assert!(script.contains("redirectSameOriginTargetBlank"));
        assert!(script.contains("desktop_same_origin_target_blank"));
        assert!(script.contains("__TAURI__.core"));
        assert!(script.contains("capture-start-failed"));
        assert!(script.contains("native_cursor_capture_start_failed"));
        assert!(script.contains("pointerLockDisabled: true"));
        assert!(script.contains("requestPointerLock"));
    }

    #[test]
    fn windows_runtime_script_uses_raw_browser_pointer_lock_without_native_bridge() {
        let script = desktop_runtime_script(&RuntimeScriptOptions {
            policy: RuntimePolicy::for_platform(ShellPlatform::Windows),
            developer_server_url: Some("http://127.0.0.1:4000/".to_string()),
            autostart: false,
            autolock: false,
        });

        assert!(script.contains("platform: \"windows\""));
        assert!(script.contains("nativeCursorBackend: false"));
        assert!(script.contains("nativeCursorCapture: false"));
        assert!(script.contains("pointerLockDisabled: false"));
        assert!(script.contains("aggressiveCursorLock: false"));
        assert!(script.contains("__RTS_DESKTOP_RUNTIME"));
        assert!(script.contains("sameOriginTargetBlankUrl"));
        assert!(script.contains("desktop_log_client_event"));
        assert!(!script.contains("__RTS_NATIVE_CURSOR"));
        assert!(!script.contains("maccursor_start"));
        assert!(!script.contains("requestPointerLock"));
        assert!(!script.contains(NATIVE_CURSOR_SCRIPT_BEGIN));
        assert!(!script.contains(NATIVE_CURSOR_SCRIPT_END));
    }

    #[test]
    fn log_reveal_program_matches_supported_platforms() {
        assert_eq!(log_reveal_program(ShellPlatform::Macos).unwrap(), "open");
        assert_eq!(
            log_reveal_program(ShellPlatform::Windows).unwrap(),
            "explorer.exe"
        );
        assert!(log_reveal_program(ShellPlatform::Other).is_err());
    }

    #[test]
    fn desktop_autolock_helper_preserves_existing_cursor_capture() {
        let script = desktop_runtime_script(&RuntimeScriptOptions {
            policy: RuntimePolicy::for_platform(ShellPlatform::Macos),
            developer_server_url: Some("http://127.0.0.1:4000/".to_string()),
            autostart: false,
            autolock: true,
        });
        assert!(script.contains("cursor-lock-already-active"));
        assert!(script.contains("cursor-lock-clicked"));
    }
}
