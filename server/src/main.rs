//! Bewegungskrieg server entry point. See `docs/design/architecture.md` and
//! `docs/design/server-sim.md`.
//!
//! Responsibilities of this binary:
//! - Serve the static JS/HTML client (so `cargo run` + open a browser is the whole dev loop).
//! - Upgrade `GET /ws` to a WebSocket and run one connection task per socket.
//! - Own a single shared [`Lobby`]; route each connection's messages to the right room.
//!
//! The simulation itself lives behind the `game` module's public API and is driven entirely by
//! the per-room task in `lobby`. This file never touches a `Game` directly.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Query, State};
use axum::http::header;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

use std::sync::Arc;

use rts_server::db::Db;
use rts_server::dev_scenarios::{
    all_dev_scenarios, dev_scenario_blocker_label, dev_scenario_unit_label,
    parse_dev_scenario_launch,
};
use rts_server::game::SimCommand;
use rts_server::lobby::{self, Lobby, RoomEvent};
use rts_server::perf;
use rts_server::protocol::{serialize_compact_snapshot, ClientMessage, ServerMessage};

/// Default room name used when a client's `join` omits `room`.
const DEFAULT_ROOM: &str = "main";

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

/// How long a connection may go without any inbound frame before we evict it. The client sends
/// app-level pings every ~15s, so a healthy connection never hits this; a silent/half-open socket
/// (or a stuck never-ready client) is dropped instead of wedging a shared room forever.
const IDLE_TIMEOUT: Duration = Duration::from_secs(40);

/// On deploy shutdown, keep the process alive long enough for in-progress matches to finish.
/// Fly's `kill_timeout` is configured to the same duration so the platform's hard stop matches
/// the app-level drain deadline.
const DEPLOY_DRAIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Shared application state handed to every request via axum's `State` extractor.
#[derive(Clone)]
struct AppState {
    lobby: Lobby,
    version: String,
    /// `index.html` with `?v=<COMMIT_HASH>` appended to all JS/CSS asset URLs, computed once at
    /// startup so cache-busting survives browser caches without a hard refresh.
    index_html: String,
    maps_dir: String,
    /// Optional database for match history. `None` when `DATABASE_URL` is unset or the connect
    /// failed; the front-page `/api/matches` endpoint returns an empty list in that case.
    db: Option<Arc<Db>>,
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

    // Public match-history writes are opt-in. When a DB is configured but the public gate is off,
    // local `cargo run` still writes rows tagged local-only so debugging games can be inspected
    // from localhost without polluting beta/mainline recent matches.
    let record_matches = env_truthy("RTS_RECORD_MATCHES");
    let match_history_local_only = !record_matches;
    let lobby_db = db.clone();
    if db.is_some() && match_history_local_only {
        info!("RTS_RECORD_MATCHES unset; match history writes enabled as localhost-only rows");
    }

    let version = git_version();
    let client_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../client");
    let index_html = build_versioned_index(client_dir, &version);
    let maps_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/maps").to_string();
    let state = AppState {
        lobby: Lobby::new().with_match_history(lobby_db, match_history_local_only),
        index_html,
        version,
        maps_dir: maps_dir.clone(),
        db,
    };
    let shutdown_lobby = state.lobby.clone();
    // Static files for everything except `/ws`; unknown paths fall back to `index.html` so the
    // single-page client loads regardless of the requested path.
    let static_service =
        ServeDir::new(client_dir).fallback(ServeFile::new(format!("{client_dir}/index.html")));

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/beta", get(beta_redirect_handler))
        .route("/beta/", get(beta_redirect_handler))
        .route("/version", get(version_handler))
        .route("/ws", get(ws_handler))
        .route("/dev/selfplay", get(dev_selfplay_handler))
        .route("/dev/scenario", get(dev_scenario_handler))
        .route("/dev/scenarios", get(dev_scenario_handler))
        .route("/maps/save", post(map_save_handler))
        .route("/api/matches", get(matches_handler))
        .nest_service("/maps", ServeDir::new(maps_dir))
        .fallback_service(static_service)
        .with_state(state);

    let addr = std::env::var("RTS_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(err) => {
            // A failed bind is fatal and there is nothing to keep alive, so report and exit.
            tracing::error!(%addr, %err, "failed to bind listen address");
            std::process::exit(1);
        }
    };

    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or(addr);
    info!("Bewegungskrieg server listening — open http://{bound}/");

    if let Err(err) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(shutdown_lobby))
    .await
    {
        tracing::error!(%err, "server error");
    }
}

async fn shutdown_signal(lobby: Lobby) {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            warn!(%err, "failed to install Ctrl-C shutdown handler");
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
                warn!(%err, "failed to install SIGTERM shutdown handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("shutdown requested by Ctrl-C"),
        _ = terminate => info!("shutdown requested by SIGTERM"),
    }

    run_deploy_drain(lobby, DEPLOY_DRAIN_TIMEOUT).await;
}

async fn run_deploy_drain(lobby: Lobby, timeout: Duration) {
    lobby.begin_draining().await;
    let active_matches = lobby.active_match_count();
    if active_matches == 0 {
        info!("shutdown drain complete; no active matches");
        lobby.request_connection_shutdown();
        return;
    }

    info!(
        active_matches,
        timeout_secs = timeout.as_secs(),
        "shutdown drain started; waiting for active matches"
    );
    tokio::select! {
        _ = lobby.wait_for_matches_to_drain() => {
            info!("shutdown drain complete; all matches finished");
        }
        _ = tokio::time::sleep(timeout) => {
            warn!(
                active_matches = lobby.active_match_count(),
                timeout_secs = timeout.as_secs(),
                "shutdown drain deadline reached; continuing shutdown"
            );
        }
    }
    lobby.request_connection_shutdown();
}

/// Axum handler for `GET /ws`: perform the WebSocket upgrade and hand the socket to a task.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    // Bound inbound frame/message size so multi-MB command frames never reach serde. Our protocol
    // is tiny JSON, so 256 KiB is generous headroom.
    ws.max_message_size(256 * 1024)
        .max_frame_size(256 * 1024)
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

/// Return the short git commit SHA that identifies this build.
async fn version_handler(State(state): State<AppState>) -> String {
    state.version
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
        Ok(rows) => Json(rows).into_response(),
        Err(err) => {
            warn!(%err, "match history query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "match history unavailable",
            )
                .into_response()
        }
    }
}

fn request_allows_local_match_history(remote: &SocketAddr) -> bool {
    remote.ip().is_loopback()
}

async fn beta_redirect_handler() -> impl IntoResponse {
    (
        StatusCode::MOVED_PERMANENTLY,
        [(header::LOCATION, "https://rts-0-zvorygin-beta.fly.dev/")],
    )
}

async fn dev_selfplay_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let mut target = "/?watchSelfplay=1".to_string();
    if let Some(replay) = params.get("replay").filter(|s| !s.trim().is_empty()) {
        target.push_str("&replay=");
        target.push_str(replay);
    }
    Redirect::temporary(&target)
}

async fn dev_scenario_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let id = params.get("id").map(|s| s.trim()).unwrap_or("");
    let unit = params.get("unit").map(|s| s.trim()).unwrap_or("");
    let count = params.get("count").map(|s| s.trim()).unwrap_or("");
    let blocker = params
        .get("blocker")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    if id.is_empty() && unit.is_empty() && count.is_empty() {
        return Html(dev_scenario_index_html()).into_response();
    }
    if let Some(launch) = parse_dev_scenario_launch(id, unit, count, blocker) {
        let mut target = format!(
            "/?watchScenario=1&id={}&unit={}&count={}",
            launch.id, launch.unit, launch.count
        );
        if let Some(blocker) = launch.blocker {
            target.push_str("&blocker=");
            target.push_str(blocker.stable_id());
        } else if launch.id == "vehicle_small_block_baseline" {
            target.push_str("&blocker=none");
        }
        return Redirect::temporary(&target).into_response();
    }
    (
        StatusCode::BAD_REQUEST,
        "supported dev scenario urls are listed at /dev/scenarios",
    )
        .into_response()
}

fn dev_scenario_index_html() -> String {
    let mut items = String::new();
    for scenario in all_dev_scenarios() {
        let mut counts = Vec::new();
        let mut rows_by_variant = Vec::new();
        for launch in scenario.launches {
            if !counts.contains(&launch.count) {
                counts.push(launch.count);
            }
            let variant = (launch.unit, launch.blocker);
            if !rows_by_variant.contains(&variant) {
                rows_by_variant.push(variant);
            }
        }
        counts.sort_unstable();

        let mut header_cells = String::new();
        for count in &counts {
            header_cells.push_str(&format!("<th scope=\"col\">x{count}</th>"));
        }

        let mut rows = String::new();
        for (unit, blocker) in rows_by_variant {
            let mut cells = String::new();
            for count in &counts {
                if scenario.launches.iter().any(|candidate| {
                    candidate.unit == unit
                        && candidate.count == *count
                        && candidate.blocker == blocker
                }) {
                    let blocker_query = match blocker {
                        Some(kind) => format!("&blocker={}", kind.stable_id()),
                        None if scenario.id == "vehicle_small_block_baseline" => {
                            "&blocker=none".to_string()
                        }
                        None => String::new(),
                    };
                    cells.push_str(&format!(
                        "<td><a class=\"scenario-link\" href=\"/dev/scenarios?id={}&unit={}&count={}{}\">Open</a></td>",
                        scenario.id,
                        unit,
                        count,
                        blocker_query
                    ));
                } else {
                    cells.push_str("<td class=\"scenario-missing\">-</td>");
                }
            }
            let row_label = if scenario.id == "vehicle_small_block_baseline" {
                format!(
                    "{} / blocker: {}",
                    dev_scenario_unit_label(unit),
                    dev_scenario_blocker_label(blocker)
                )
            } else {
                dev_scenario_unit_label(unit).to_string()
            };
            rows.push_str(&format!(
                "<tr>\
                    <th scope=\"row\">{}</th>\
                    {}\
                 </tr>",
                row_label, cells
            ));
        }

        items.push_str(&format!(
            "<section class=\"scenario-panel\">\
                <div class=\"scenario-copy\">\
                  <h2>{}</h2>\
                  <p><code>{}</code></p>\
                  <p>{}</p>\
                </div>\
                <table class=\"scenario-table\">\
                  <thead>\
                    <tr>\
                      <th scope=\"col\">Unit</th>\
                      {}\
                    </tr>\
                  </thead>\
                  <tbody>{}</tbody>\
                </table>\
             </section>",
            scenario.title, scenario.id, scenario.description, header_cells, rows
        ));
    }

    format!(
        "<!DOCTYPE html>\
        <html lang=\"en\">\
          <head>\
            <meta charset=\"UTF-8\" />\
            <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\
            <title>Dev Scenarios</title>\
            <link rel=\"stylesheet\" href=\"/styles.css\" />\
            <style>\
              html, body {{ min-height: 100%; height: auto; overflow-y: auto; overflow-x: hidden; }}\
              body {{ background: var(--void); color: var(--ink); font-family: var(--font); image-rendering: auto; }}\
              .scenario-page {{ width: min(960px, 100%); margin: 0 auto; padding: 32px 20px 48px; }}\
              .scenario-page h1 {{ margin: 0 0 6px; color: var(--accent); font-size: 28px; line-height: 1.1; letter-spacing: 0.08em; text-transform: uppercase; text-shadow: 2px 2px 0 #191710; }}\
              .scenario-page > p {{ margin: 0; color: var(--ink-dim); }}\
              .scenario-grid {{ display: grid; gap: 14px; margin-top: 22px; }}\
              .scenario-panel {{ display: grid; grid-template-columns: minmax(220px, 0.8fr) minmax(0, 1.2fr); gap: 18px; align-items: start; border: 1px solid var(--panel-edge); border-radius: var(--radius); background: rgba(39, 37, 31, 0.98); box-shadow: var(--shadow), inset 0 1px 0 rgba(255, 255, 255, 0.08); padding: 18px; }}\
              .scenario-copy h2 {{ margin: 0 0 8px; font-size: 16px; letter-spacing: 0.04em; text-transform: uppercase; }}\
              .scenario-copy p {{ margin: 0 0 8px; color: var(--ink-dim); }}\
              .scenario-copy code {{ color: var(--ink-faint); font-family: var(--mono); font-size: 12px; }}\
              .scenario-table {{ width: 100%; border-collapse: collapse; border: 1px solid rgba(91, 83, 65, 0.7); background: rgba(17, 17, 15, 0.34); }}\
              .scenario-table th, .scenario-table td {{ padding: 9px 10px; border-bottom: 1px solid rgba(91, 83, 65, 0.45); text-align: left; }}\
              .scenario-table thead th {{ color: var(--ink-dim); font-family: var(--mono); font-size: 11px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; }}\
              .scenario-table tbody th {{ color: var(--ink); font-weight: 600; }}\
              .scenario-table tbody tr:last-child th, .scenario-table tbody tr:last-child td {{ border-bottom: 0; }}\
              .scenario-link {{ display: inline-flex; align-items: center; justify-content: center; min-width: 56px; padding: 6px 10px; border: 1px solid var(--panel-edge); border-radius: var(--radius-sm); background: var(--panel); color: var(--ink); font-weight: 600; text-decoration: none; }}\
              .scenario-link:hover {{ border-color: var(--accent); box-shadow: inset 0 0 0 1px var(--panel-glow); }}\
              .scenario-missing {{ color: var(--ink-faint); font-family: var(--mono); }}\
              @media (max-width: 720px) {{ .scenario-panel {{ grid-template-columns: 1fr; }} .scenario-page {{ padding: 24px 12px 36px; }} .scenario-table th, .scenario-table td {{ padding: 8px; }} }}\
            </style>\
          </head>\
          <body>\
            <main class=\"scenario-page\">\
              <h1>Dev Scenarios</h1>\
              <p>Available local scenario launches. Pick one to open the live no-fog watcher.</p>\
              <div class=\"scenario-grid\">{items}</div>\
            </main>\
          </body>\
        </html>"
    )
}

/// Return the short git commit SHA that identifies this build.
///
/// The hash is resolved at **compile time** by `build.rs` and baked into the binary via
/// `cargo:rustc-env=COMMIT_HASH`. This works both for local `cargo run` (where `.git` is
/// available at build time) and for deployed Docker images (where `.git` is present in the
/// builder layer or injected via a `COMMIT_HASH` build arg).
fn git_version() -> String {
    env!("COMMIT_HASH").to_string()
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
        tracing::error!(%path, %err, "failed to read index.html");
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

    const TEST_PLAYER_ID: u32 = 42;

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
        tokio::time::timeout(Duration::from_secs(1), async {
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

    #[tokio::test]
    async fn deploy_drain_waits_for_active_match_to_finish() {
        let lobby = Lobby::new();
        let handle = start_one_player_test_match(&lobby, "unit-drain-finish").await;
        let mut shutdown_rx = lobby.subscribe_connection_shutdown();
        let drain = tokio::spawn(run_deploy_drain(lobby.clone(), Duration::from_secs(5)));

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
    async fn deploy_drain_deadline_closes_connections_with_match_still_active() {
        let lobby = Lobby::new();
        let handle = start_one_player_test_match(&lobby, "unit-drain-timeout").await;
        let mut shutdown_rx = lobby.subscribe_connection_shutdown();

        tokio::time::timeout(
            Duration::from_secs(1),
            run_deploy_drain(lobby.clone(), Duration::from_millis(25)),
        )
        .await
        .expect("deploy drain should honor the short deadline");

        assert_eq!(
            lobby.active_match_count(),
            1,
            "the short deadline should not require the match to have ended"
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
    fn scenario_index_lists_supported_launches() {
        let html = dev_scenario_index_html();
        assert!(html.contains("Scout Car Snaking Corridor"));
        assert!(html.contains("Direct Reverse Order"));
        assert!(html.contains("Vehicle Wall Chokepoint"));
        assert!(html.contains("Vehicle Small-Unit Block Baseline"));
        assert!(html.contains("<table class=\"scenario-table\">"));
        assert!(html.contains("/dev/scenarios?id=scout_car_snaking_corridor&unit=worker&count=1"));
        assert!(html.contains("/dev/scenarios?id=scout_car_snaking_corridor&unit=tank&count=4"));
        assert!(html.contains("/dev/scenarios?id=direct_reverse_order&unit=tank&count=1"));
        assert!(
            html.contains("/dev/scenarios?id=scout_car_wall_chokepoint&unit=scout_car&count=15")
        );
        assert!(html.contains("/dev/scenarios?id=scout_car_wall_chokepoint&unit=tank&count=15"));
        assert!(html.contains("/dev/scenarios?id=scout_car_wall_chokepoint&unit=at_team&count=15"));
        assert!(
            html.contains("/dev/scenarios?id=vehicle_small_block_baseline&unit=scout_car&count=5")
        );
        assert!(html.contains(
            "/dev/scenarios?id=vehicle_small_block_baseline&unit=scout_car&count=5&blocker=none"
        ));
        assert!(html.contains(
            "/dev/scenarios?id=vehicle_small_block_baseline&unit=scout_car&count=5&blocker=machine_gunner"
        ));
        assert!(html.contains(
            "/dev/scenarios?id=vehicle_small_block_baseline&unit=tank&count=5&blocker=at_team"
        ));
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
            "nested ES modules must be versioned so desktop webviews do not run stale renderer code"
        );
        assert!(html.contains("./src/main.js?v=test-version\""));
        assert!(html.contains("./styles.css?v=test-version\""));
        assert!(html.contains("/manifest.webmanifest?v=test-version\""));
    }

    #[test]
    fn local_match_history_allowed_for_loopback_remote() {
        let remote: SocketAddr = "127.0.0.1:50000".parse().unwrap();
        assert!(request_allows_local_match_history(&remote));
    }

    #[test]
    fn local_match_history_allowed_for_ipv6_loopback_remote() {
        let remote: SocketAddr = "[::1]:50000".parse().unwrap();
        assert!(request_allows_local_match_history(&remote));
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
///   to the sink. The room sends through the matching connection sink via [`RoomEvent::Join`],
///   so a slow socket only backs up its own outbound state — it never blocks the room.
/// - On this task, send `welcome`, then read `ClientMessage`s and translate them to
///   [`RoomEvent`]s for whichever room the client joins.
/// - On stream close (or any fatal read error) emit a final [`RoomEvent::Leave`].
///
/// Bad input is logged and skipped; we never panic on the read path.
async fn handle_connection(socket: WebSocket, lobby: Lobby) {
    let player_id = lobby::next_player_id();
    debug!(player_id, "connection opened");

    let (mut sink, mut stream) = socket.split();

    // Outbound path: room (and this task, for welcome/pong) -> writer task -> socket.
    let (conn_tx, writer_rx) = lobby::ConnectionSink::new();

    // Writer task: serialize each ServerMessage to a JSON TEXT frame and push it to the socket.
    // Reliable messages stay object-shaped JSON; snapshots use the compact v1 JSON schema.
    // Reliable messages are FIFO and prioritized over snapshots. Snapshots are latest-only:
    // while the socket is busy, newer snapshots replace older unsent snapshots.
    let writer = tokio::spawn(async move {
        let lobby::ConnectionWriter {
            mut reliable_rx,
            snapshots,
        } = writer_rx;
        let mut reliable_closed = false;

        'write_loop: loop {
            while !reliable_closed {
                match reliable_rx.try_recv() {
                    Ok(msg) => {
                        if !send_server_message(player_id, &mut sink, msg).await {
                            break 'write_loop;
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        reliable_closed = true;
                        break;
                    }
                }
            }

            if let Some(snapshot) = snapshots.take() {
                if !send_server_message(player_id, &mut sink, ServerMessage::Snapshot(snapshot))
                    .await
                {
                    break 'write_loop;
                }
                // Send at most one snapshot before checking reliable messages again.
                continue;
            }

            if reliable_closed {
                break;
            }

            tokio::select! {
                maybe_msg = reliable_rx.recv() => {
                    match maybe_msg {
                        Some(msg) => {
                            if !send_server_message(player_id, &mut sink, msg).await {
                                break 'write_loop;
                            }
                        }
                        None => reliable_closed = true,
                    }
                }
                _ = snapshots.notified() => {}
            }
        }

        // Best-effort close; ignore errors since the socket may already be gone.
        let _ = sink.close().await;
    });

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
    let mut shutdown_rx = lobby.subscribe_connection_shutdown();

    loop {
        // Bound the read so a silent/half-open client is evicted rather than parked forever. The
        // post-loop code emits `Leave`, which cleans up membership and (mid-match) eliminates them.
        let next = tokio::select! {
            _ = wait_for_connection_shutdown(&mut shutdown_rx) => {
                debug!(player_id, "server shutdown; closing connection");
                break;
            }
            next = tokio::time::timeout(IDLE_TIMEOUT, stream.next()) => next,
        };
        let next = match next {
            Ok(next) => next,
            Err(_) => {
                debug!(player_id, "idle timeout; closing");
                break;
            }
        };
        let Some(frame) = next else {
            break;
        };
        let frame = match frame {
            Ok(f) => f,
            Err(err) => {
                debug!(player_id, %err, "websocket read error; closing");
                break;
            }
        };

        match frame {
            Message::Text(text) => {
                let parsed: ClientMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(err) => {
                        // Malformed input is the client's problem; tell it and keep the socket.
                        debug!(player_id, %err, text = %text, "ignoring malformed client message");
                        let _ = conn_tx.try_send_reliable(ServerMessage::Error {
                            msg: "malformed message".to_string(),
                        });
                        continue;
                    }
                };
                handle_client_message(player_id, parsed, &lobby, &conn_tx, &mut current_room).await;
            }
            Message::Binary(_) => {
                // The protocol is JSON text only; ignore stray binary frames.
                debug!(player_id, "ignoring unexpected binary frame");
            }
            Message::Ping(_) | Message::Pong(_) => {
                // axum answers protocol-level pings automatically; nothing to do.
            }
            Message::Close(_) => {
                debug!(player_id, "client sent close");
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
    debug!(player_id, "connection closed");
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

async fn send_server_message(
    player_id: u32,
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    msg: ServerMessage,
) -> bool {
    let message_kind = match &msg {
        ServerMessage::Snapshot(_) => "snapshot",
        ServerMessage::Lobby { .. } => "lobby",
        ServerMessage::Welcome { .. } => "welcome",
        ServerMessage::Start(_) => "start",
        ServerMessage::ReplayState(_) => "replay_state",
        ServerMessage::Error { .. } => "error",
        ServerMessage::GameOver { .. } => "game_over",
        ServerMessage::Pong { .. } => "pong",
        #[allow(unreachable_patterns)]
        _ => "other",
    };
    let serialize_start = Instant::now();
    let encoded = match msg {
        ServerMessage::Snapshot(snapshot) => serialize_compact_snapshot(&snapshot),
        reliable => serde_json::to_string(&reliable),
    };
    let serialize_duration = serialize_start.elapsed();
    match encoded {
        Ok(json) => {
            let bytes = json.len();
            let send_start = Instant::now();
            if sink.send(Message::Text(json.into())).await.is_err() {
                // Socket gone; stop writing. The reader side will emit Leave.
                perf::log_writer_message(
                    player_id,
                    message_kind,
                    serialize_duration,
                    send_start.elapsed(),
                    bytes,
                );
                return false;
            }
            perf::log_writer_message(
                player_id,
                message_kind,
                serialize_duration,
                send_start.elapsed(),
                bytes,
            );
        }
        Err(err) => {
            // Should never happen for our own types, but never let it kill the task.
            warn!(player_id, %err, "failed to serialize server message");
        }
    }
    true
}

/// Translate one parsed [`ClientMessage`] into the appropriate side effect.
///
/// `join` resolves (or creates) the target room and registers this connection's outbound sender;
/// everything else forwards a [`RoomEvent`] to the already-joined room (silently ignored before a
/// join). `ping` is answered directly so it works even outside a room.
async fn handle_client_message(
    player_id: u32,
    msg: ClientMessage,
    lobby: &Lobby,
    conn_tx: &lobby::ConnectionSink,
    current_room: &mut Option<lobby::RoomHandle>,
) {
    match msg {
        ClientMessage::Join {
            name,
            room,
            spectator,
        } => {
            // Re-joining a different room is not supported; the first join wins. Subsequent
            // joins from the same connection are ignored to keep room membership unambiguous.
            if current_room.is_some() {
                debug!(
                    player_id,
                    "ignoring extra join on already-joined connection"
                );
                return;
            }
            let room_name = room
                .filter(|r| !r.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_ROOM.to_string());
            let name = sanitize_name(name);
            let handle = lobby.get_or_create(&room_name).await;
            // The room decides whether the join is accepted (it may reject a mid-match join). Wait
            // for its ack and only mark ourselves joined on `true`, so a rejected join leaves
            // `current_room` None and the client is free to try another room.
            let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
            let sent = handle
                .event_tx
                .send(RoomEvent::Join {
                    player_id,
                    name,
                    spectator,
                    msg_tx: conn_tx.clone(),
                    ack: ack_tx,
                })
                .await
                .is_ok();
            if !sent {
                warn!(player_id, room = %room_name, "room task gone; cannot join");
                return;
            }
            match ack_rx.await {
                Ok(true) => *current_room = Some(handle),
                Ok(false) => {
                    debug!(player_id, room = %room_name, "join rejected by room");
                }
                Err(_) => {
                    warn!(player_id, room = %room_name, "room dropped join ack; cannot join");
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
        ClientMessage::AddAi => {
            send_room_event(player_id, current_room, RoomEvent::AddAi { player_id }).await;
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
        ClientMessage::SetQuickstart { enabled } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetQuickstart { player_id, enabled },
            )
            .await;
        }
        ClientMessage::SetSpectator { spectator } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetSpectator {
                    player_id,
                    spectator,
                },
            )
            .await;
        }
        ClientMessage::Command { cmd } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::Command {
                    player_id,
                    cmd: SimCommand::from_protocol(cmd),
                },
            )
            .await;
        }
        ClientMessage::GiveUp => {
            send_room_event(player_id, current_room, RoomEvent::GiveUp { player_id }).await;
        }
        ClientMessage::ReturnToLobby => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::ReturnToLobby { player_id },
            )
            .await;
        }
        ClientMessage::Ping { ts } => {
            // Answer directly so latency probes work regardless of room state.
            let _ = conn_tx.try_send_reliable(ServerMessage::Pong { ts });
        }
        ClientMessage::SetReplaySpeed { speed } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetReplaySpeed { player_id, speed },
            )
            .await;
        }
        ClientMessage::SeekReplay { ticks_back } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SeekReplay {
                    player_id,
                    ticks_back,
                },
            )
            .await;
        }
        ClientMessage::SetReplayVision { vision } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::SetReplayVision { player_id, vision },
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
            debug!(player_id, "ignoring unsupported client message");
        }
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
                warn!(player_id, "room task gone; dropping event");
            }
        }
        None => debug!(player_id, "ignoring event before join"),
    }
}

/// Trim and bound a player-supplied display name so it stays sane in lobby UIs and logs.
fn sanitize_name(name: String) -> String {
    const MAX_NAME_LEN: usize = 24;
    let trimmed = name.trim();
    let cleaned: String = trimmed.chars().take(MAX_NAME_LEN).collect();
    if cleaned.is_empty() {
        "Anonymous".to_string()
    } else {
        cleaned
    }
}

#[derive(Deserialize)]
struct MapSaveRequest {
    name: String,
    payload: serde_json::Value,
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
            warn!(%e, "map save: payload serialization failed");
            return (StatusCode::BAD_REQUEST, "invalid payload").into_response();
        }
    };

    if let Err(e) = tokio::fs::write(&path, &json_bytes).await {
        warn!(%e, ?path, "map save: write failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response();
    }

    info!(?path, "map saved");
    (StatusCode::OK, filename).into_response()
}
