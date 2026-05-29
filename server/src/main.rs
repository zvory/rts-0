//! RTS server entry point. See `DESIGN.md` §1, §3.
//!
//! Responsibilities of this binary:
//! - Serve the static JS/HTML client (so `cargo run` + open a browser is the whole dev loop).
//! - Upgrade `GET /ws` to a WebSocket and run one connection task per socket.
//! - Own a single shared [`Lobby`]; route each connection's messages to the right room.
//!
//! The simulation itself lives behind the `game` module's public API and is driven entirely by
//! the per-room task in `lobby`. This file never touches a `Game` directly.

mod config;
mod game;
mod lobby;
mod protocol;

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

use crate::lobby::{Lobby, RoomEvent};
use crate::protocol::{ClientMessage, ServerMessage};

/// Default room name used when a client's `join` omits `room`.
const DEFAULT_ROOM: &str = "main";

/// How long a connection may go without any inbound frame before we evict it. The client sends
/// app-level pings every ~15s, so a healthy connection never hits this; a silent/half-open socket
/// (or a stuck never-ready client) is dropped instead of wedging a shared room forever.
const IDLE_TIMEOUT: Duration = Duration::from_secs(40);

/// Shared application state handed to every request via axum's `State` extractor.
#[derive(Clone)]
struct AppState {
    lobby: Lobby,
}

#[tokio::main]
async fn main() {
    // Honor `RUST_LOG`; default to `info` so a fresh checkout logs something useful.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let state = AppState {
        lobby: Lobby::new(),
    };

    // Resolve the client dir relative to the crate, so the working directory doesn't matter.
    let client_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../client");
    // Static files for everything except `/ws`; unknown paths fall back to `index.html` so the
    // single-page client loads regardless of the requested path.
    let static_service =
        ServeDir::new(client_dir).fallback(ServeFile::new(format!("{client_dir}/index.html")));

    let app = Router::new()
        .route("/ws", get(ws_handler))
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
    info!("RTS server listening — open http://{bound}/");

    if let Err(err) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    {
        tracing::error!(%err, "server error");
    }
}

/// Axum handler for `GET /ws`: perform the WebSocket upgrade and hand the socket to a task.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    // Bound inbound frame/message size so multi-MB command frames never reach serde. Our protocol
    // is tiny JSON, so 256 KiB is generous headroom.
    ws.max_message_size(256 * 1024)
        .max_frame_size(256 * 1024)
        .on_upgrade(move |socket| handle_connection(socket, state.lobby))
}

/// Drive one client connection end to end.
///
/// Layout (see `DESIGN.md` §3.2):
/// - Split the socket into a sink (writer) and a stream (reader).
/// - Spawn a dedicated **writer task** that drains an `mpsc::Receiver<ServerMessage>` to the
///   sink. The room sends through the matching sender via [`RoomEvent::Join`], so a slow socket
///   only backs up its own channel — it never blocks the room.
/// - On this task, send `welcome`, then read `ClientMessage`s and translate them to
///   [`RoomEvent`]s for whichever room the client joins.
/// - On stream close (or any fatal read error) emit a final [`RoomEvent::Leave`].
///
/// Bad input is logged and skipped; we never panic on the read path.
async fn handle_connection(socket: WebSocket, lobby: Lobby) {
    let player_id = lobby::next_player_id();
    debug!(player_id, "connection opened");

    let (mut sink, mut stream) = socket.split();

    // Outbound channel: room (and this task, for welcome/pong) -> writer task -> socket.
    let (msg_tx, mut msg_rx) = mpsc::channel::<ServerMessage>(lobby::player_channel_cap());

    // Writer task: serialize each ServerMessage to a JSON TEXT frame and push it to the socket.
    // Exits when the channel closes (connection cleanup) or the socket errors.
    let writer = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if sink.send(Message::Text(json.into())).await.is_err() {
                        // Socket gone; stop writing. The reader side will emit Leave.
                        break;
                    }
                }
                Err(err) => {
                    // Should never happen for our own types, but never let it kill the task.
                    warn!(player_id, %err, "failed to serialize server message");
                }
            }
        }
        // Best-effort close; ignore errors since the socket may already be gone.
        let _ = sink.close().await;
    });

    // Announce the assigned id before anything else.
    if msg_tx
        .send(ServerMessage::Welcome { player_id })
        .await
        .is_err()
    {
        // Writer already gone — nothing more to do.
        writer.abort();
        return;
    }

    // The room this connection has joined, if any. A client must `join` before other actions.
    let mut current_room: Option<lobby::RoomHandle> = None;

    loop {
        // Bound the read so a silent/half-open client is evicted rather than parked forever. The
        // post-loop code emits `Leave`, which cleans up membership and (mid-match) eliminates them.
        let next = match tokio::time::timeout(IDLE_TIMEOUT, stream.next()).await {
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
                        debug!(player_id, %err, "ignoring malformed client message");
                        let _ = msg_tx.try_send(ServerMessage::Error {
                            msg: "malformed message".to_string(),
                        });
                        continue;
                    }
                };
                handle_client_message(player_id, parsed, &lobby, &msg_tx, &mut current_room).await;
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
    // Dropping `msg_tx` closes the writer's channel, ending the writer task.
    drop(msg_tx);
    let _ = writer.await;
    debug!(player_id, "connection closed");
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
    msg_tx: &mpsc::Sender<ServerMessage>,
    current_room: &mut Option<lobby::RoomHandle>,
) {
    match msg {
        ClientMessage::Join { name, room } => {
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
                    msg_tx: msg_tx.clone(),
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
        ClientMessage::Command { cmd } => {
            send_room_event(
                player_id,
                current_room,
                RoomEvent::Command { player_id, cmd },
            )
            .await;
        }
        ClientMessage::Ping { ts } => {
            // Answer directly so latency probes work regardless of room state.
            let _ = msg_tx.try_send(ServerMessage::Pong { ts });
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
