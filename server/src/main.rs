//! Bewegungskrieg server entry point. See `docs/design/architecture.md` and
//! `docs/design/server-sim.md`.
//!
//! Serves the static client, upgrades `/ws`, and owns the shared [`Lobby`]. The simulation itself
//! lives behind the `game` module's public API and is driven entirely by the per-room task in
//! `lobby`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

mod client_optional_assets;
mod connection_writer;
mod dev_replay_pages;
mod dev_scenario_pages;
mod lab_interact_artifacts;
#[cfg(test)]
mod main_replay_tests;
mod wiki;

use rts_server::db::Db;
use rts_server::lab_scenario_submission::{
    LabScenarioSubmissionService, SCENARIO_SUBMISSION_CAPABILITY_PATH,
};
use rts_server::lab_scenarios::catalog_handler as lab_scenarios_handler;
use rts_server::lobby::{self, Lobby, RoomEvent};
use rts_server::protocol::{ClientMessage, ServerMessage};
use rts_server::structured_log;
use rts_sim::game::map::Map;
use rts_sim::game::replay::{self, ReplayArtifactV1};
use rts_sim::perf;

/// Default room name used when a client's `join` omits `room`.
const DEFAULT_ROOM: &str = "main";
const DEFAULT_CLIENT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../client");
const DEFAULT_MAPS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/maps");
const RTS_CLIENT_DIR_ENV: &str = "RTS_CLIENT_DIR";
const RTS_MAPS_DIR_ENV: &str = "RTS_MAPS_DIR";
const MAX_CLIENT_MESSAGE_BYTES: usize = 1_000_000 + 64 * 1024; // lab scenario cap + envelope

#[derive(Clone, Copy)]
struct ClientMessageTiming {
    received_unix_ms: u64,
    frame_received_at: Instant,
    deserialized_at: Instant,
}

/// Treat unset/empty/`0`/`false`/`no`/`off` as falsy; anything else is true.
fn env_truthy(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        ),
        Err(_) => false,
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn env_path_or_default(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn configured_client_dir() -> String {
    env_path_or_default(RTS_CLIENT_DIR_ENV, DEFAULT_CLIENT_DIR)
}

fn configured_maps_dir() -> String {
    env_path_or_default(RTS_MAPS_DIR_ENV, DEFAULT_MAPS_DIR)
}

/// How long a connection may go without any inbound frame before we evict it. The client sends
/// app-level pings every ~15s, so a healthy connection never hits this; a silent/half-open socket
/// (or a stuck never-ready client) is dropped instead of wedging a shared room forever.
const IDLE_TIMEOUT: Duration = Duration::from_secs(40);

/// On deploy shutdown, keep the process alive long enough for in-progress matches to finish.
/// Fly's shared-CPU `kill_timeout` caps at 300 seconds, so leave a few seconds for axum to stop
/// accepting connections and exit cleanly before the platform sends its final shutdown signal.
const DEPLOY_DRAIN_TIMEOUT: Duration = Duration::from_secs(295);

/// Shared application state handed to every request via axum's `State` extractor.
#[derive(Clone)]
struct AppState {
    lobby: Lobby,
    version: String,
    /// `index.html` with `?v=<build id>` appended to all JS/CSS asset URLs, computed once at
    /// startup so cache-busting survives browser caches without a hard refresh.
    index_html: String,
    maps_dir: String,
    /// Optional database for match history. `None` when `DATABASE_URL` is unset or the connect
    /// failed; the front-page `/api/matches` endpoint returns an empty list in that case.
    db: Option<Arc<Db>>,
    lab_scenario_submission: LabScenarioSubmissionService,
    lab_interact_artifacts: lab_interact_artifacts::LabInteractArtifactBridge,
}

#[tokio::main]
async fn main() {
    // Honor `RUST_LOG`; default to `info` so a fresh checkout logs something useful.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    perf::PerfConfig::global().enforce_release_build_for_server();

    // Load .env from the repo root if present. Errors (missing file) are non-fatal; production
    // deployments inject env vars directly.
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/../.env"));

    let db = rts_server::db::try_connect_from_env().await;

    // Match-history writes are opt-in for shared beta/mainline deploys. Local `cargo run` may
    // connect to the DB for reads, but it must not upload match rows or replay artifacts unless
    // this public gate is explicitly enabled.
    let record_matches = env_truthy("RTS_RECORD_MATCHES");
    let lobby_db = record_matches.then(|| db.clone()).flatten();
    if db.is_some() && !record_matches {
        rts_server::log_info!("RTS_RECORD_MATCHES unset; match history writes disabled");
    }

    let version = rts_server::build_info::build_id().to_string();
    let client_dir = configured_client_dir();
    let index_html = build_versioned_index(&client_dir, &version);
    let maps_dir = configured_maps_dir();
    let lab_scenario_submission = LabScenarioSubmissionService::from_env();
    let submission_capability = lab_scenario_submission.capability();
    if submission_capability.available {
        rts_server::log_info!(
            branch_prefix = %submission_capability.branch_prefix,
            "lab scenario PR submission enabled"
        );
    } else {
        rts_server::log_info!(
            unavailable_code = submission_capability
                .unavailable_code
                .as_deref()
                .unwrap_or("unknown"),
            "lab scenario PR submission unavailable"
        );
    }
    let state = AppState {
        lobby: Lobby::new()
            .with_match_history(lobby_db, false)
            .with_lab_scenario_submission(lab_scenario_submission.clone()),
        index_html,
        version,
        maps_dir: maps_dir.clone(),
        db,
        lab_scenario_submission,
        lab_interact_artifacts: lab_interact_artifacts::LabInteractArtifactBridge::from_env(),
    };
    let shutdown_lobby = state.lobby.clone();
    // Static files for everything except `/ws`; unknown app routes fall back to `index.html` so the
    // single-page client loads, but missing asset URLs stay 404 so packaging errors are visible.
    let static_service = ServeDir::new(&client_dir)
        .fallback(get(client_spa_fallback_handler).with_state(state.clone()));

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/beta", get(beta_redirect_handler))
        .route("/beta/", get(beta_redirect_handler))
        .route("/lab", get(index_handler))
        .route("/lab/", get(index_handler))
        .route("/version", get(version_handler))
        .route("/wiki", get(wiki::wiki_index_handler))
        .route("/wiki/", get(wiki::wiki_index_handler))
        .route("/wiki/{*path}", get(wiki::wiki_page_handler))
        .route("/ws", get(ws_handler))
        .route(
            "/dev/lab-interact/artifacts/export",
            post(lab_interact_artifacts::export_handler),
        )
        .route(
            "/dev/lab-interact/artifacts/upload",
            post(lab_interact_artifacts::upload_handler).layer(DefaultBodyLimit::max(
                lab_interact_artifacts::MAX_ARTIFACT_BYTES,
            )),
        )
        .route(
            "/dev/lab-interact/artifacts/import",
            post(lab_interact_artifacts::import_handler),
        )
        .route(
            "/dev/lab-interact/artifacts/cleanup",
            post(lab_interact_artifacts::cleanup_handler),
        )
        .route(
            "/dev/lab-interact/artifacts/{artifact_id}",
            get(lab_interact_artifacts::download_handler),
        )
        .route("/dev/replay-artifact", get(dev_replay_artifact_handler))
        .route(
            "/dev/replay-lobby",
            post(dev_replay_pages::dev_replay_lobby_handler),
        )
        .route(
            "/dev/scenario",
            get(dev_scenario_pages::dev_scenario_handler),
        )
        .route(
            "/dev/scenarios",
            get(dev_scenario_pages::dev_scenario_handler),
        )
        .route("/api/lab-scenarios", get(lab_scenarios_handler))
        .route(
            SCENARIO_SUBMISSION_CAPABILITY_PATH,
            get(lab_scenario_submission_capability_handler),
        )
        .route("/maps/catalog", get(map_catalog_handler))
        .route("/maps/save", post(map_save_handler))
        .route(
            "/api/lobbies",
            get(lobbies_handler).post(create_lobby_handler),
        )
        .route("/api/matches", get(matches_handler))
        .route("/api/observations/{match_run_id}", get(observation_handler))
        .route(
            "/api/matches/{id}/replay",
            post(match_replay_launch_handler),
        )
        .nest_service("/maps", ServeDir::new(maps_dir))
        .fallback_service(static_service)
        .with_state(state);

    let addr = std::env::var("RTS_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(err) => {
            // A failed bind is fatal and there is nothing to keep alive, so report and exit.
            rts_server::log_error!(%addr, %err, "failed to bind listen address");
            std::process::exit(1);
        }
    };

    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or(addr);
    rts_server::log_info!("Bewegungskrieg server listening — open http://{bound}/");

    if let Err(err) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(shutdown_lobby))
    .await
    {
        rts_server::log_error!(%err, "server error");
    }
}

async fn shutdown_signal(lobby: Lobby) {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            rts_server::log_warn!(%err, "failed to install Ctrl-C shutdown handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                rts_server::log_warn!(%err, "failed to install SIGTERM shutdown handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => rts_server::log_info!("shutdown requested by Ctrl-C"),
        _ = terminate => rts_server::log_info!("shutdown requested by SIGTERM"),
    }

    lobby.run_deploy_drain(DEPLOY_DRAIN_TIMEOUT).await;
}

/// Axum handler for `GET /ws`: perform the WebSocket upgrade and hand the socket to a task.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    // Bound inbound frame/message size while leaving room for capped lab scenario imports.
    ws.max_message_size(MAX_CLIENT_MESSAGE_BYTES)
        .max_frame_size(MAX_CLIENT_MESSAGE_BYTES)
        .on_upgrade(move |socket| handle_connection(socket, state.lobby))
}

/// Serve `index.html` with `Cache-Control: no-cache` so browsers always revalidate it.
/// The embedded asset URLs already carry `?v=<hash>`, so JS/CSS are fetched fresh only when the
/// hash changes — subsequent loads hit the browser cache for unchanged builds.
async fn index_handler(State(state): State<AppState>) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        state.index_html,
    )
}

async fn client_spa_fallback_handler(uri: Uri, State(state): State<AppState>) -> impl IntoResponse {
    if let Some(response) = client_optional_assets::fallback(uri.path()) {
        return response;
    }
    if is_client_asset_path(uri.path()) {
        return (StatusCode::NOT_FOUND, "static asset not found").into_response();
    }
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        state.index_html,
    )
        .into_response()
}

fn is_client_asset_path(path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path);
    let normalized = path.trim_start_matches('/');
    if normalized.is_empty() {
        return false;
    }
    if matches!(
        normalized.split('/').next(),
        Some("src" | "assets" | "vendor")
    ) {
        return true;
    }
    let Some(last_segment) = normalized.rsplit('/').next() else {
        return false;
    };
    last_segment.contains('.')
}

/// Return the short git commit SHA that identifies this build.
async fn version_handler(State(state): State<AppState>) -> String {
    state.version
}

#[derive(Deserialize)]
struct CreateLobbyRequest {
    room: String,
}

#[derive(Serialize)]
struct CreateLobbyResponse {
    room: String,
}

/// GET /api/lobbies — browser-safe summaries for public normal rooms.
async fn lobbies_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.lobby.summaries().await)
}

/// GET /api/lab-scenarios/submission — deployment capability for draft PR submission.
async fn lab_scenario_submission_capability_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    Json(state.lab_scenario_submission.capability())
}

/// POST /api/lobbies — reserve a new normal lobby name without joining an existing room.
async fn create_lobby_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateLobbyRequest>,
) -> impl IntoResponse {
    match state.lobby.create_lobby(&request.room).await {
        Ok(room) => (StatusCode::CREATED, Json(CreateLobbyResponse { room })).into_response(),
        Err(err) => create_lobby_error_response(err),
    }
}

fn create_lobby_error_response(err: lobby::CreateLobbyError) -> axum::response::Response {
    let status = match &err {
        lobby::CreateLobbyError::Duplicate => StatusCode::CONFLICT,
        lobby::CreateLobbyError::Draining(_) => StatusCode::SERVICE_UNAVAILABLE,
        lobby::CreateLobbyError::EmptyName
        | lobby::CreateLobbyError::NameTooLong { .. }
        | lobby::CreateLobbyError::InvalidCharacters
        | lobby::CreateLobbyError::ReservedName => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(ApiError {
            error: err.message().to_string(),
        }),
    )
        .into_response()
}

#[derive(Deserialize)]
struct MatchesQuery {
    limit: Option<i64>,
}

/// GET /api/matches?limit=N — most-recent resolved matches in newest-first order.
/// Returns `[]` (200) when no DB is configured so the client doesn't need to special-case.
async fn matches_handler(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Query(params): Query<MatchesQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20);
    let Some(db) = state.db else {
        return Json(Vec::<rts_server::db::MatchSummary>::new()).into_response();
    };
    let include_local = request_allows_local_match_history(&remote);
    match db.recent_matches(limit, include_local).await {
        Ok(mut rows) => {
            for row in &mut rows {
                apply_replay_summary_compatibility(row, &state.version);
            }
            Json(rows).into_response()
        }
        Err(err) => {
            rts_server::log_warn!(%err, "match history query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "match history unavailable",
            )
                .into_response()
        }
    }
}

/// GET /api/observations/{match_run_id} — recover the hidden AI-only match row associated with
/// the run id shown at the end of a watched matchup. The returned `id` can be passed to the
/// existing replay-launch endpoint; structured logs use the same run id.
async fn observation_handler(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path(match_run_id): Path<String>,
) -> impl IntoResponse {
    if !valid_observation_run_id(&match_run_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid observation id.".to_string(),
            }),
        )
            .into_response();
    }
    let Some(db) = state.db else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "Observation history is not configured.".to_string(),
            }),
        )
            .into_response();
    };
    let include_local = request_allows_local_match_history(&remote);
    match db.observation_by_run_id(&match_run_id, include_local).await {
        Ok(Some(mut row)) => {
            apply_replay_summary_compatibility(&mut row, &state.version);
            Json(row).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "Observation not found. It may still be saving its replay.".to_string(),
            }),
        )
            .into_response(),
        Err(err) => {
            rts_server::log_warn!(%err, match_run_id = %match_run_id, "AI observation query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: "Observation history is unavailable.".to_string(),
                }),
            )
                .into_response()
        }
    }
}

fn valid_observation_run_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[derive(Serialize)]
struct MatchReplayLaunchResponse {
    room: String,
}

#[derive(Serialize)]
struct ApiError {
    error: String,
}

/// POST /api/matches/{id}/replay — create a spectator replay room for a compatible persisted match.
async fn match_replay_launch_handler(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path(match_id): Path<i64>,
) -> impl IntoResponse {
    let Some(db) = state.db.clone() else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "Replay is unavailable because match history is not configured.".to_string(),
            }),
        )
            .into_response();
    };
    let include_local = request_allows_local_match_history(&remote);
    let artifact = match db.replay_artifact_for_match(match_id, include_local).await {
        Ok(Some(artifact)) => artifact,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiError {
                    error: "Replay is unavailable for this match.".to_string(),
                }),
            )
                .into_response();
        }
        Err(err) => {
            rts_server::log_warn!(%err, match_id, "match replay load failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: "Replay could not be loaded.".to_string(),
                }),
            )
                .into_response();
        }
    };

    if let Some(reason) = replay_incompatibility_reason(&artifact, &state.version) {
        return (StatusCode::CONFLICT, Json(ApiError { error: reason })).into_response();
    }

    let room = state.lobby.create_replay_room(artifact).await;
    Json(MatchReplayLaunchResponse { room }).into_response()
}

fn request_allows_local_match_history(remote: &SocketAddr) -> bool {
    remote.ip().is_loopback()
}

fn apply_replay_summary_compatibility(row: &mut rts_server::db::MatchSummary, build_sha: &str) {
    let Some(meta) = &row.replay_metadata else {
        row.replay_available = false;
        row.replay_unavailable_reason = Some("Replay was not recorded for this match.".to_string());
        return;
    };
    let supported_schema = u32::try_from(meta.artifact_schema_version)
        .ok()
        .is_some_and(replay::is_supported_replay_artifact_schema);
    if !supported_schema {
        row.replay_available = false;
        row.replay_unavailable_reason = Some(format!(
            "Replay schema {} is not supported by this server.",
            meta.artifact_schema_version
        ));
        return;
    }
    let running_map = match Map::metadata_for_name(&meta.map_name) {
        Ok(metadata) => metadata,
        Err(_) => {
            row.replay_available = false;
            row.replay_unavailable_reason = Some(format!(
                "Replay map {:?} is not available on this server.",
                meta.map_name
            ));
            return;
        }
    };
    if meta.map_schema_version != running_map.schema_version as i32 {
        row.replay_available = false;
        row.replay_unavailable_reason = Some(format!(
            "Replay map {:?} schema is {}; running map schema is {}.",
            meta.map_name, meta.map_schema_version, running_map.schema_version
        ));
        return;
    }
    if meta.map_hash != running_map.content_hash {
        row.replay_available = false;
        row.replay_unavailable_reason = Some(format!(
            "Replay map {:?} has changed on this server.",
            meta.map_name
        ));
        return;
    }
    if meta.build_sha != build_sha {
        row.replay_available = true;
        row.replay_unavailable_reason = Some(replay_build_warning(build_sha, &meta.build_sha));
        return;
    }
    row.replay_available = true;
    row.replay_unavailable_reason = None;
}

fn replay_incompatibility_reason(artifact: &ReplayArtifactV1, build_sha: &str) -> Option<String> {
    lobby::replay_launch_incompatibility_reason(artifact, build_sha)
}

fn replay_build_warning(server_build_sha: &str, replay_build_sha: &str) -> String {
    format!(
        "Replay Potentially Incompatible With Current Server (server: {server_build_sha}, replay: {replay_build_sha})"
    )
}

async fn beta_redirect_handler() -> impl IntoResponse {
    (
        StatusCode::MOVED_PERMANENTLY,
        [(header::LOCATION, "https://bewegungskrieg-beta.fly.dev/")],
    )
}

async fn dev_replay_artifact_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(replay) = params.get("replay").map(|s| s.trim()).filter(|s| {
        !s.is_empty()
            && !s.contains('/')
            && !s.contains('\\')
            && !s.contains("..")
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }) else {
        return (
            StatusCode::BAD_REQUEST,
            "expected /dev/replay-artifact?replay=<artifact_name>",
        )
            .into_response();
    };
    Redirect::temporary(&format!("/?replayArtifact={replay}")).into_response()
}

/// Read `index.html`, inject a versioned import map for all `/src/*.js` modules, and append
/// `?v=<version>` to the top-level cacheable app asset URLs.
///
/// The import map causes the browser to rewrite every `import "./foo.js"` inside ES modules to
/// `./foo.js?v=<version>`, so sub-modules (hud.js, net.js, …) are cache-busted alongside
/// main.js without a build step.
fn build_versioned_index(client_dir: &str, version: &str) -> String {
    let path = format!("{client_dir}/index.html");
    let html = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        rts_server::log_error!(%path, %err, "failed to read index.html");
        String::new()
    });

    // Collect every .js file under client/src/ to populate the import map.
    let src_dir = format!("{client_dir}/src");
    let mut entries = String::new();
    let mut names = Vec::new();
    collect_js_modules(
        std::path::Path::new(&src_dir),
        std::path::Path::new(""),
        &mut names,
    );
    names.sort();
    for name in names {
        entries.push_str(&format!(
            "    \"/src/{name}\": \"/src/{name}?v={version}\",\n"
        ));
    }
    // Remove the trailing comma from the last entry so the JSON is valid.
    if entries.ends_with(",\n") {
        entries.truncate(entries.len() - 2);
        entries.push('\n');
    }
    let import_map = format!(
        "<script type=\"importmap\">\n{{\n  \"imports\": {{\n{entries}  }}\n}}\n</script>\n  "
    );

    // Insert the import map just before the main <script type="module"> tag.
    let html = html.replace(
        "<script type=\"module\"",
        &format!("{import_map}<script type=\"module\""),
    );

    // Also version the top-level entry point, stylesheet, and web manifest.
    html.replace("./src/main.js\"", &format!("./src/main.js?v={version}\""))
        .replace("./styles.css\"", &format!("./styles.css?v={version}\""))
        .replace(
            "/manifest.webmanifest\"",
            &format!("/manifest.webmanifest?v={version}\""),
        )
}

fn collect_js_modules(dir: &std::path::Path, prefix: &std::path::Path, out: &mut Vec<String>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let next_prefix = prefix.join(name);
        if path.is_dir() {
            collect_js_modules(&path, &next_prefix, out);
        } else if path.extension().is_some_and(|ext| ext == "js") {
            if let Some(name) = next_prefix.to_str() {
                out.push(name.replace('\\', "/"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_asset_env<R>(
        client_dir: Option<&str>,
        maps_dir: Option<&str>,
        f: impl FnOnce() -> R,
    ) -> R {
        let _guard = env_lock().lock().unwrap();
        let prior_client = std::env::var(RTS_CLIENT_DIR_ENV).ok();
        let prior_maps = std::env::var(RTS_MAPS_DIR_ENV).ok();

        match client_dir {
            Some(value) => std::env::set_var(RTS_CLIENT_DIR_ENV, value),
            None => std::env::remove_var(RTS_CLIENT_DIR_ENV),
        }
        match maps_dir {
            Some(value) => std::env::set_var(RTS_MAPS_DIR_ENV, value),
            None => std::env::remove_var(RTS_MAPS_DIR_ENV),
        }

        let result = f();

        match prior_client {
            Some(value) => std::env::set_var(RTS_CLIENT_DIR_ENV, value),
            None => std::env::remove_var(RTS_CLIENT_DIR_ENV),
        }
        match prior_maps {
            Some(value) => std::env::set_var(RTS_MAPS_DIR_ENV, value),
            None => std::env::remove_var(RTS_MAPS_DIR_ENV),
        }

        result
    }
    const TEST_PLAYER_ID: u32 = 42;

    #[test]
    fn sanitize_name_uses_commander_for_blank_names() {
        assert_eq!(sanitize_name(" \n\t ".to_string()), "Commander");
    }

    async fn start_one_player_test_match(lobby: &Lobby, room: &str) -> lobby::RoomHandle {
        let handle = lobby.get_or_create(room).await;
        let (msg_tx, _writer) = lobby::ConnectionSink::new();
        let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
        handle
            .event_tx
            .send(RoomEvent::Join {
                player_id: TEST_PLAYER_ID,
                name: "Drain Test".to_string(),
                spectator: false,
                replay_ok: false,
                msg_tx,
                ack: ack_tx,
            })
            .await
            .expect("room task should accept join event");
        assert_eq!(ack_rx.await, Ok(true));
        handle
            .event_tx
            .send(RoomEvent::Ready {
                player_id: TEST_PLAYER_ID,
                ready: true,
            })
            .await
            .expect("room task should accept ready event");
        handle
            .event_tx
            .send(RoomEvent::StartRequest {
                player_id: TEST_PLAYER_ID,
            })
            .await
            .expect("room task should accept start event");
        wait_for_active_match_count(lobby, 1).await;
        handle
    }

    async fn wait_for_active_match_count(lobby: &Lobby, expected: usize) {
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                if lobby.active_match_count() == expected {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("active match count did not settle");
    }

    fn test_message_timing() -> ClientMessageTiming {
        let now = Instant::now();
        ClientMessageTiming {
            received_unix_ms: 0,
            frame_received_at: now,
            deserialized_at: now,
        }
    }

    #[tokio::test]
    async fn join_to_different_room_transfers_connection_and_leaves_previous_room() {
        let lobby = Lobby::new();
        let (conn_tx, mut writer) = lobby::ConnectionSink::new();
        let mut current_room = None;
        let mut current_room_name = None;

        handle_client_message(
            TEST_PLAYER_ID,
            ClientMessage::Join {
                name: "Transfer Test".to_string(),
                room: Some("transfer-source".to_string()),
                spectator: false,
                replay_ok: false,
            },
            test_message_timing(),
            &lobby,
            &conn_tx,
            &mut current_room,
            &mut current_room_name,
        )
        .await;
        assert_eq!(current_room_name.as_deref(), Some("transfer-source"));
        let source_handle = current_room.clone().expect("source room should be joined");

        handle_client_message(
            TEST_PLAYER_ID,
            ClientMessage::Join {
                name: "Transfer Test".to_string(),
                room: Some("transfer-target".to_string()),
                spectator: true,
                replay_ok: false,
            },
            test_message_timing(),
            &lobby,
            &conn_tx,
            &mut current_room,
            &mut current_room_name,
        )
        .await;
        assert_eq!(current_room_name.as_deref(), Some("transfer-target"));
        assert!(
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).any(|msg| {
                matches!(msg, ServerMessage::Lobby { room, .. } if room == "transfer-target")
            })
        );

        tokio::time::timeout(Duration::from_secs(1), source_handle.event_tx.closed())
            .await
            .expect("empty source room should be disposed after transfer");
        if let Some(handle) = current_room {
            handle
                .event_tx
                .send(RoomEvent::Leave {
                    player_id: TEST_PLAYER_ID,
                })
                .await
                .expect("target cleanup leave should send");
        }
    }

    #[tokio::test]
    async fn deploy_drain_waits_for_active_match_to_finish() {
        let lobby = Lobby::new();
        let handle = start_one_player_test_match(&lobby, "unit-drain-finish").await;
        let mut shutdown_rx = lobby.subscribe_connection_shutdown();
        let drain_lobby = lobby.clone();
        let drain =
            tokio::spawn(async move { drain_lobby.run_deploy_drain(Duration::from_secs(5)).await });

        tokio::time::sleep(Duration::from_millis(25)).await;
        assert!(
            !*shutdown_rx.borrow_and_update(),
            "connections should stay open while an active match is still draining"
        );

        handle
            .event_tx
            .send(RoomEvent::GiveUp {
                player_id: TEST_PLAYER_ID,
            })
            .await
            .expect("room task should accept give-up event");
        tokio::time::timeout(Duration::from_secs(1), drain)
            .await
            .expect("deploy drain should complete after the active match ends")
            .expect("deploy drain task should not panic");
        assert!(
            *shutdown_rx.borrow_and_update(),
            "connections should close after match drain completes"
        );
        wait_for_active_match_count(&lobby, 0).await;
    }

    #[tokio::test]
    async fn deploy_drain_deadline_forces_active_match_before_closing_connections() {
        let lobby = Lobby::new();
        let handle = start_one_player_test_match(&lobby, "unit-drain-timeout").await;
        let mut shutdown_rx = lobby.subscribe_connection_shutdown();

        tokio::time::timeout(
            Duration::from_secs(1),
            lobby.run_deploy_drain_with_budget(lobby::DeployDrainBudget {
                natural_match_drain: Duration::from_millis(25),
                forced_finalization: Duration::from_millis(500),
                match_history_write_wait: Duration::from_millis(10),
                shutdown_slack: Duration::ZERO,
            }),
        )
        .await
        .expect("deploy drain should honor the short deadline");

        assert_eq!(
            lobby.active_match_count(),
            0,
            "the short natural-drain deadline should force-finalize the active match"
        );
        assert!(
            *shutdown_rx.borrow_and_update(),
            "connections should close when the drain deadline expires"
        );

        handle
            .event_tx
            .send(RoomEvent::Leave {
                player_id: TEST_PLAYER_ID,
            })
            .await
            .expect("room task should accept cleanup leave event");
        wait_for_active_match_count(&lobby, 0).await;
    }

    #[test]
    fn versioned_index_cache_busts_nested_js_modules() {
        let client_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../client");
        let html = build_versioned_index(client_dir, "test-version");
        assert!(html.contains("\"/src/main.js\": \"/src/main.js?v=test-version\""));
        assert!(
            html.contains(
                "\"/src/renderer/terrain.js\": \"/src/renderer/terrain.js?v=test-version\""
            ),
            "nested ES modules must be versioned so browser clients do not run stale renderer code"
        );
        assert!(html.contains("./src/main.js?v=test-version\""));
        assert!(html.contains("./styles.css?v=test-version\""));
        assert!(html.contains("/manifest.webmanifest?v=test-version\""));
    }

    #[test]
    fn client_asset_path_detection_keeps_missing_assets_out_of_spa_fallback() {
        for path in [
            "/vendor/sim-wasm/rts_sim_wasm.js",
            "/vendor/sim-wasm/rts_sim_wasm_bg.wasm",
            "/src/main.js",
            "/assets/decals/infantry-splash-01.svg",
            "/styles.css",
            "/manifest.webmanifest",
            "/favicon.ico",
        ] {
            assert!(
                is_client_asset_path(path),
                "{path} should be treated as a static asset"
            );
        }

        for path in ["/", "/lab", "/lab/", "/beta", "/rooms/open"] {
            assert!(
                !is_client_asset_path(path),
                "{path} should stay eligible for SPA fallback"
            );
        }
    }

    #[tokio::test]
    async fn beta_redirect_targets_current_beta_app() {
        let response = beta_redirect_handler().await.into_response();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "https://bewegungskrieg-beta.fly.dev/"
        );
    }

    #[test]
    fn asset_paths_default_to_source_tree() {
        with_asset_env(None, None, || {
            assert_eq!(configured_client_dir(), DEFAULT_CLIENT_DIR);
            assert_eq!(configured_maps_dir(), DEFAULT_MAPS_DIR);
        });
    }

    #[test]
    fn asset_paths_use_non_empty_env_overrides() {
        with_asset_env(Some("/tmp/rts-client"), Some("/tmp/rts-maps"), || {
            assert_eq!(configured_client_dir(), "/tmp/rts-client");
            assert_eq!(configured_maps_dir(), "/tmp/rts-maps");
        });
    }

    #[test]
    fn blank_asset_path_overrides_fall_back_to_source_tree() {
        with_asset_env(Some("  "), Some(""), || {
            assert_eq!(configured_client_dir(), DEFAULT_CLIENT_DIR);
            assert_eq!(configured_maps_dir(), DEFAULT_MAPS_DIR);
        });
    }

    #[test]
    fn local_match_history_allowed_for_loopback_remotes() {
        for remote in ["127.0.0.1:50000", "[::1]:50000"] {
            let remote: SocketAddr = remote.parse().unwrap();
            assert!(request_allows_local_match_history(&remote));
        }
    }

    #[test]
    fn local_match_history_rejected_for_public_remote() {
        let remote: SocketAddr = "203.0.113.10:50000".parse().unwrap();
        assert!(!request_allows_local_match_history(&remote));
    }
}

/// Drive one client connection end to end.
///
/// Layout (see `docs/design/server-sim.md` §3.2):
/// - Split the socket into a sink (writer) and a stream (reader).
/// - Spawn a dedicated **writer task** that drains reliable messages and latest-only snapshots
///   to the sink. Observer analysis uses its own latest-only lane behind snapshots, so diagnostic
///   panel updates cannot starve world-state delivery. The room sends through the matching
///   connection sink via [`RoomEvent::Join`], so a slow socket only backs up its own outbound state
///   — it never blocks the room.
/// - On this task, send `welcome`, then read `ClientMessage`s and translate them to
///   [`RoomEvent`]s for whichever room the client joins.
/// - On stream close (or any fatal read error) emit a final [`RoomEvent::Leave`].
///
/// Bad input is logged and skipped; we never panic on the read path.
async fn handle_connection(socket: WebSocket, lobby: Lobby) {
    let player_id = lobby::next_player_id();
    rts_server::log_debug!(player_id, "connection opened");

    let (sink, mut stream) = socket.split();

    let (conn_tx, writer_rx) = lobby::ConnectionSink::new();

    let writer = connection_writer::spawn(player_id, sink, writer_rx);

    // Announce the assigned id before anything else.
    if conn_tx
        .send_reliable(ServerMessage::Welcome { player_id })
        .await
        .is_err()
    {
        // Writer already gone — nothing more to do.
        writer.abort();
        return;
    }

    // The room this connection has joined, if any. A client must `join` before other actions.
    let mut current_room: Option<lobby::RoomHandle> = None;
    let mut current_room_name: Option<String> = None;
    let mut shutdown_rx = lobby.subscribe_connection_shutdown();

    loop {
        // Bound the read so a silent/half-open client is evicted rather than parked forever. The
        // post-loop code emits `Leave`, which cleans up membership and (mid-match) eliminates them.
        let next = tokio::select! {
            _ = wait_for_connection_shutdown(&mut shutdown_rx) => {
                rts_server::log_debug!(player_id, "server shutdown; closing connection");
                break;
            }
            next = tokio::time::timeout(IDLE_TIMEOUT, stream.next()) => next,
        };
        let next = match next {
            Ok(next) => next,
            Err(_) => {
                rts_server::log_debug!(player_id, "idle timeout; closing");
                break;
            }
        };
        let Some(frame) = next else {
            break;
        };
        let frame = match frame {
            Ok(f) => f,
            Err(err) => {
                rts_server::log_debug!(player_id, %err, "websocket read error; closing");
                break;
            }
        };

        match frame {
            Message::Text(text) => {
                let frame_received_at = Instant::now();
                let received_unix_ms = current_unix_ms();
                let parsed: ClientMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(err) => {
                        // Malformed input is the client's problem; tell it and keep the socket.
                        rts_server::log_debug!(player_id, %err, text = %text, "ignoring malformed client message");
                        let _ = conn_tx.try_send_reliable(ServerMessage::Error {
                            msg: "malformed message".to_string(),
                        });
                        continue;
                    }
                };
                let timing = ClientMessageTiming {
                    received_unix_ms,
                    frame_received_at,
                    deserialized_at: Instant::now(),
                };
                handle_client_message(
                    player_id,
                    parsed,
                    timing,
                    &lobby,
                    &conn_tx,
                    &mut current_room,
                    &mut current_room_name,
                )
                .await;
            }
            Message::Binary(_) => {
                // Client-to-server messages remain JSON text only; ignore stray binary frames.
                rts_server::log_debug!(player_id, "ignoring unexpected binary frame");
            }
            Message::Ping(_) | Message::Pong(_) => {
                // axum answers protocol-level pings automatically; nothing to do.
            }
            Message::Close(_) => {
                rts_server::log_debug!(player_id, "client sent close");
                break;
            }
        }
    }

    // Connection is done — notify the room (if joined) so it can resolve membership / the match.
    if let Some(handle) = &current_room {
        let _ = handle.event_tx.send(RoomEvent::Leave { player_id }).await;
    }
    // Dropping `conn_tx` closes the writer's reliable channel, ending the writer task after it
    // flushes any pending latest snapshot.
    drop(conn_tx);
    let _ = writer.await;
    rts_server::log_debug!(player_id, "connection closed");
}

async fn wait_for_connection_shutdown(shutdown_rx: &mut tokio::sync::watch::Receiver<bool>) {
    loop {
        if *shutdown_rx.borrow_and_update() {
            return;
        }
        if shutdown_rx.changed().await.is_err() {
            return;
        }
    }
}

/// Translate one parsed [`ClientMessage`] into the appropriate side effect.
///
/// `join` resolves (or creates) the target room and registers this connection's outbound sender;
/// everything else forwards a [`RoomEvent`] to the already-joined room (silently ignored before a
/// join). `ping` is answered directly so it works even outside a room.
async fn handle_client_message(
    player_id: u32,
    msg: ClientMessage,
    timing: ClientMessageTiming,
    lobby: &Lobby,
    conn_tx: &lobby::ConnectionSink,
    current_room: &mut Option<lobby::RoomHandle>,
    current_room_name: &mut Option<String>,
) {
    match msg {
        ClientMessage::Join {
            name,
            room,
            spectator,
            replay_ok,
            ..
        } => {
            let room_name = room
                .filter(|r| !r.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_ROOM.to_string());
            if current_room_name.as_deref() == Some(room_name.as_str()) {
                rts_server::log_debug!(
                    player_id,
                    room = %room_name,
                    "ignoring duplicate join on already-joined connection"
                );
                return;
            }
            let name = sanitize_name(name);
            let handle = match lobby.get_or_create_join_target(&room_name).await {
                Ok(handle) => handle,
                Err(notice) => {
                    let _ = conn_tx.try_send_reliable(ServerMessage::ShutdownWarning {
                        deadline_unix_ms: notice.deadline_unix_ms,
                        seconds_remaining: notice.seconds_remaining,
                    });
                    let _ = conn_tx.try_send_reliable(ServerMessage::Error {
                        msg: "Server is draining for deploy; new rooms are disabled.".to_string(),
                    });
                    rts_server::log_debug!(player_id, room = %room_name, "rejecting new room while server is draining");
                    return;
                }
            };
            let previous_room = current_room.clone();
            let previous_room_name = current_room_name.clone();
            // The room decides whether the join is accepted (it may reject a mid-match join). Wait
            // for its ack and only switch membership on `true`, so a rejected transfer leaves the
            // client in its previous room and a rejected first join leaves it free to try another.
            let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
            let sent = handle
                .event_tx
                .send(RoomEvent::Join {
                    player_id,
                    name,
                    spectator,
                    replay_ok,
                    msg_tx: conn_tx.clone(),
                    ack: ack_tx,
                })
                .await
                .is_ok();
            if !sent {
                rts_server::log_warn!(player_id, room = %room_name, "room task gone; cannot join");
                return;
            }
            match ack_rx.await {
                Ok(true) => {
                    if let Some(previous) = previous_room {
                        let _ = previous.event_tx.send(RoomEvent::Leave { player_id }).await;
                        rts_server::log_debug!(
                            player_id,
                            from = previous_room_name.as_deref().unwrap_or(""),
                            to = %room_name,
                            "transferred connection to room"
                        );
                    }
                    *current_room = Some(handle);
                    *current_room_name = Some(room_name);
                }
                Ok(false) => {
                    rts_server::log_debug!(player_id, room = %room_name, "join rejected by room");
                }
                Err(_) => {
                    rts_server::log_warn!(player_id, room = %room_name, "room dropped join ack; cannot join");
                }
            }
        }
        ClientMessage::Ready { ready } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::Ready { player_id, ready },
            )
            .await;
        }
        ClientMessage::Start => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::StartRequest { player_id },
            )
            .await;
        }
        ClientMessage::SetTeamPreset { preset } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetTeamPreset { player_id, preset },
            )
            .await;
        }
        ClientMessage::SetTeam {
            id: target,
            team_id,
        } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetTeam {
                    player_id,
                    target,
                    team_id,
                },
            )
            .await;
        }
        ClientMessage::SetFaction { faction_id } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetFaction {
                    player_id,
                    faction_id,
                },
            )
            .await;
        }
        ClientMessage::AddAi {
            team_id,
            ai_profile_id,
        } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::AddAi {
                    player_id,
                    team_id,
                    ai_profile_id,
                },
            )
            .await;
        }
        ClientMessage::SetAiProfile { id, ai_profile_id } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetAiProfile {
                    player_id,
                    target: id,
                    ai_profile_id,
                },
            )
            .await;
        }
        ClientMessage::RemoveAi { id } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::RemoveAi {
                    player_id,
                    target: id,
                },
            )
            .await;
        }
        ClientMessage::SetSpectator { spectator, id } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetSpectator {
                    player_id,
                    target: id.unwrap_or(player_id),
                    spectator,
                },
            )
            .await;
        }
        ClientMessage::Command { client_seq, cmd } => {
            lobby::send_command_room_event(
                player_id,
                current_room,
                client_seq,
                cmd,
                timing.received_unix_ms,
                timing.frame_received_at,
                timing.deserialized_at,
            )
            .await;
        }
        ClientMessage::GiveUp => {
            send_room_event(player_id, current_room, RoomEvent::GiveUp { player_id }).await;
        }
        ClientMessage::PauseGame => {
            send_room_event(player_id, current_room, RoomEvent::PauseGame { player_id }).await;
        }
        ClientMessage::UnpauseGame => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::UnpauseGame { player_id },
            )
            .await;
        }
        ClientMessage::ReturnToLobby => {
            if let Some(handle) = current_room.take() {
                let _ = handle
                    .event_tx
                    .send(RoomEvent::ReturnToLobby { player_id })
                    .await;
                *current_room_name = None;
            }
        }
        ClientMessage::Ping { ts } => {
            // Answer directly so latency probes work regardless of room state.
            let _ = conn_tx.try_send_reliable(ServerMessage::Pong { ts });
        }
        ClientMessage::NetReport { report } => {
            let outbound = conn_tx.consume_report_stats();
            structured_log::log_client_net_report(
                player_id,
                current_room_name.as_deref(),
                *report,
                outbound,
            );
        }
        ClientMessage::SetRoomTimeSpeed { speed } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetRoomTimeSpeed { player_id, speed },
            )
            .await;
        }
        ClientMessage::StepRoomTime => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::StepRoomTime { player_id },
            )
            .await;
        }
        ClientMessage::SeekRoomTime { ticks_back } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SeekRoomTime {
                    player_id,
                    ticks_back,
                },
            )
            .await;
        }
        ClientMessage::SeekRoomTimeTo { tick } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SeekRoomTimeTo { player_id, tick },
            )
            .await;
        }
        ClientMessage::SetVisionSelection { selection } => {
            let event = RoomEvent::SetVisionSelection {
                player_id,
                selection,
            };
            send_room_event(player_id, current_room, event).await;
        }
        ClientMessage::Lab { request_id, op } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::Lab {
                    player_id,
                    request_id,
                    op: *op,
                },
            )
            .await;
        }
        ClientMessage::RequestBranchFromTick => {
            request_branch_from_tick(player_id, lobby, conn_tx, current_room).await;
        }
        ClientMessage::ClaimBranchSeat {
            player_id: seat_player_id,
        } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::ClaimBranchSeat {
                    player_id,
                    seat_player_id,
                },
            )
            .await;
        }
        ClientMessage::ReleaseBranchSeat {
            player_id: seat_player_id,
        } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::ReleaseBranchSeat {
                    player_id,
                    seat_player_id,
                },
            )
            .await;
        }
        ClientMessage::StartBranch => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::StartBranch { player_id },
            )
            .await;
        }
        ClientMessage::SelectMap { map } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SelectMap { player_id, map },
            )
            .await;
        }
        #[allow(unreachable_patterns)]
        _ => {
            rts_server::log_debug!(player_id, "ignoring unsupported client message");
        }
    }
}

async fn request_branch_from_tick(
    player_id: u32,
    lobby: &Lobby,
    conn_tx: &lobby::ConnectionSink,
    current_room: &Option<lobby::RoomHandle>,
) {
    let Some(handle) = current_room else {
        rts_server::log_debug!(player_id, "ignoring branch-from-tick request before join");
        return;
    };
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    if handle
        .event_tx
        .send(RoomEvent::RequestBranchFromTick {
            player_id,
            reply: reply_tx,
        })
        .await
        .is_err()
    {
        rts_server::log_warn!(player_id, "room task gone; cannot request branch from tick");
        return;
    }
    let seed = match reply_rx.await {
        Ok(Ok(seed)) => seed,
        Ok(Err(msg)) => {
            let _ = conn_tx.try_send_reliable(ServerMessage::Error { msg });
            return;
        }
        Err(_) => {
            rts_server::log_warn!(player_id, "room dropped branch-from-tick reply");
            return;
        }
    };
    let source_tick = seed.source_tick;
    let seats = seed.seats.clone();
    let branch_room = lobby.create_replay_branch_room(seed).await;
    if handle
        .event_tx
        .send(RoomEvent::AnnounceBranchFromTick {
            branch_room: branch_room.clone(),
            source_tick,
            seats: seats.clone(),
        })
        .await
        .is_err()
    {
        let _ = conn_tx.try_send_reliable(ServerMessage::BranchFromTickCreated {
            branch_room,
            source_tick,
            seats,
        });
    }
}

/// Forward a [`RoomEvent`] to the connection's room, if it has joined one. Logs and ignores the
/// message otherwise (a client acting before `join`).
async fn send_room_event(
    player_id: u32,
    current_room: &Option<lobby::RoomHandle>,
    event: RoomEvent,
) {
    match current_room {
        Some(handle) => {
            if handle.event_tx.send(event).await.is_err() {
                rts_server::log_warn!(player_id, "room task gone; dropping event");
            }
        }
        None => rts_server::log_debug!(player_id, "ignoring event before join"),
    }
}

/// Trim and bound a player-supplied display name so it stays sane in lobby UIs and logs.
fn sanitize_name(name: String) -> String {
    const MAX_NAME_LEN: usize = 24;
    let trimmed = name.trim();
    let cleaned: String = trimmed.chars().take(MAX_NAME_LEN).collect();
    if cleaned.is_empty() {
        "Commander".to_string()
    } else {
        cleaned
    }
}

#[derive(Deserialize)]
struct MapSaveRequest {
    name: String,
    payload: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MapCatalogEntry {
    file: String,
    name: String,
    description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MapCatalogResponse {
    maps: Vec<MapCatalogEntry>,
}

/// GET /maps/catalog — list built-in authored map JSON files for editor selection.
async fn map_catalog_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut entries = match tokio::fs::read_dir(&state.maps_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            rts_server::log_warn!(%e, maps_dir = %state.maps_dir, "map catalog: read_dir failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "cannot read maps directory",
            )
                .into_response();
        }
    };
    let mut maps = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Some(file) = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let Ok(json) = tokio::fs::read_to_string(&path).await else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
            continue;
        };
        if value.get("version").and_then(|v| v.as_u64()) != Some(2) {
            continue;
        }
        let stem = file.trim_end_matches(".json");
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(stem)
            .to_string();
        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or(&name)
            .to_string();
        maps.push(MapCatalogEntry {
            file,
            name,
            description,
        });
    }
    maps.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.file.cmp(&b.file)));
    Json(MapCatalogResponse { maps }).into_response()
}

/// POST /maps/save — write a map JSON file directly into the server's assets/maps directory.
/// Only accepts filenames matching `[a-z0-9-]+` to prevent path traversal.
async fn map_save_handler(
    State(state): State<AppState>,
    Json(req): Json<MapSaveRequest>,
) -> impl IntoResponse {
    let name = req.name.trim().to_string();
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return (
            StatusCode::BAD_REQUEST,
            "name must be 1-64 lowercase alphanumeric or hyphen characters",
        )
            .into_response();
    }

    let filename = format!("{name}.json");
    let path = std::path::Path::new(&state.maps_dir).join(&filename);

    let json_bytes = match serde_json::to_vec_pretty(&req.payload) {
        Ok(mut b) => {
            b.push(b'\n');
            b
        }
        Err(e) => {
            rts_server::log_warn!(%e, "map save: payload serialization failed");
            return (StatusCode::BAD_REQUEST, "invalid payload").into_response();
        }
    };

    if let Err(e) = tokio::fs::write(&path, &json_bytes).await {
        rts_server::log_warn!(%e, ?path, "map save: write failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response();
    }

    rts_server::log_info!(?path, "map saved");
    (StatusCode::OK, filename).into_response()
}
