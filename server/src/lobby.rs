//! Lobby & room orchestration. See `DESIGN.md` §3.2.
//!
//! Concurrency model: there is exactly **one tokio task per room** and that task is the
//! sole owner/writer of its [`Game`]. There are no locks around the simulation. Everything
//! else — connections, the lobby registry — talks to a room only by sending it
//! [`RoomEvent`]s over an `mpsc` channel. A room task multiplexes (via `tokio::select!`)
//! between its fixed-rate tick and that event stream, so a slow or disconnected client can
//! never stall the simulation.
//!
//! Lifecycle:
//! 1. A connection joins a room (creating it if needed). The room task is spawned lazily.
//! 2. In the `Lobby` phase the room broadcasts a `lobby` message on every membership/ready
//!    change. The host may start the match when everyone is ready.
//! 3. In the `InGame` phase the room advances [`Game`] once per tick, fans out a fog-filtered
//!    snapshot to each connected player, and detects game-over. When the match resolves the
//!    room returns to the `Lobby` phase (ready flags reset) so the same players can rematch.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, info, warn};

use crate::config;
use crate::game::{Game, PlayerInit};
use crate::protocol::{Command, Event, LobbyPlayer, ServerMessage, StartPayload};

/// Player colors, assigned by join order. MUST match `client/src/config.js` `PLAYER_PALETTE`.
const PLAYER_PALETTE: [&str; 6] = [
    "#3aa0ff", "#ff5a4d", "#46d36b", "#f0c64a", "#b96cff", "#ff9a3c",
];

/// Bound on a player's outbound message queue. Generous enough to absorb a brief render stall
/// but small enough that a truly dead client is detected (a full queue ⇒ treated as gone) and
/// dropped instead of buffering unboundedly.
const PLAYER_CHANNEL_CAP: usize = 256;

/// Bound on a room's inbound event queue. Commands/joins past this are dropped rather than
/// allowed to grow without limit; in practice the room drains this every tick.
const ROOM_EVENT_CHANNEL_CAP: usize = 1024;

/// Monotonic source of globally-unique player ids (ids are never reused within a process run).
static NEXT_PLAYER_ID: AtomicU32 = AtomicU32::new(1);

/// Allocate a fresh, process-unique player id. Called once per connection.
pub fn next_player_id() -> u32 {
    NEXT_PLAYER_ID.fetch_add(1, Ordering::Relaxed)
}

/// Internal message from a connection (or the lobby) to a room task. The room task is the
/// only consumer; see module docs.
#[derive(Debug)]
pub enum RoomEvent {
    /// A player joins this room. `msg_tx` is the connection's outbound sink. `ack` carries the
    /// accept/reject decision back to the connection: `true` once the player is actually in the
    /// room, `false` if the join was rejected (duplicate, or mid-match). The connection must not
    /// mark itself joined until it sees a `true`, so a rejected join doesn't wedge the socket.
    Join {
        player_id: u32,
        name: String,
        msg_tx: mpsc::Sender<ServerMessage>,
        ack: tokio::sync::oneshot::Sender<bool>,
    },
    /// A player left (socket closed). During a match this eliminates them so it can resolve.
    Leave { player_id: u32 },
    /// A player toggled their lobby ready flag.
    Ready { player_id: u32, ready: bool },
    /// The host requested the match to begin (honored only from the host when `can_start`).
    StartRequest { player_id: u32 },
    /// A gameplay command (ignored unless the room is in-game and the sender is in the room).
    Command { player_id: u32, cmd: Command },
}

/// Handle the lobby keeps for each live room: just the channel into its task.
#[derive(Clone)]
pub struct RoomHandle {
    pub event_tx: mpsc::Sender<RoomEvent>,
}

/// Registry of rooms by name. Cheaply cloneable; share one instance across all connections.
#[derive(Clone)]
pub struct Lobby {
    rooms: Arc<Mutex<HashMap<String, RoomHandle>>>,
}

impl Lobby {
    pub fn new() -> Self {
        Lobby {
            rooms: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the handle for `room`, spawning the room task on first use. The `Mutex` here only
    /// guards the small name→handle map (cheap, never held across `.await` of game work); it is
    /// emphatically *not* a lock around any `Game`.
    pub async fn get_or_create(&self, room: &str) -> RoomHandle {
        let mut rooms = self.rooms.lock().await;
        if let Some(handle) = rooms.get(room) {
            return handle.clone();
        }
        let (event_tx, event_rx) = mpsc::channel(ROOM_EVENT_CHANNEL_CAP);
        let handle = RoomHandle { event_tx };
        rooms.insert(room.to_string(), handle.clone());

        let name = room.to_string();
        tokio::spawn(async move {
            let mut task = RoomTask::new(name.clone());
            task.run(event_rx).await;
            info!(room = %name, "room task exited");
        });
        info!(room = %room, "room created");
        handle
    }
}

impl Default for Lobby {
    fn default() -> Self {
        Self::new()
    }
}

/// A connected player as tracked inside a room.
struct RoomPlayer {
    name: String,
    color: String,
    ready: bool,
    msg_tx: mpsc::Sender<ServerMessage>,
}

/// The room's current mode. `InGame` owns the live simulation outright.
enum Phase {
    Lobby,
    InGame(Game),
}

/// All state owned by a single room task. Lives entirely on that task — never shared.
struct RoomTask {
    room: String,
    /// Connected players in join order (join order drives color assignment and host fallback).
    order: Vec<u32>,
    players: HashMap<u32, RoomPlayer>,
    /// Current host (first joiner; reassigned to the next in `order` when the host leaves).
    host_id: Option<u32>,
    phase: Phase,
    /// Number of human players the in-progress match started with. Used so a 1-player sandbox
    /// match never ends while a 2+ player match resolves to a winner. `0` outside a match.
    match_player_count: usize,
}

impl RoomTask {
    fn new(room: String) -> Self {
        RoomTask {
            room,
            order: Vec::new(),
            players: HashMap::new(),
            host_id: None,
            phase: Phase::Lobby,
            match_player_count: 0,
        }
    }

    /// Main loop: multiplex the fixed-rate tick against the inbound event stream. Returns (and
    /// the task ends) only when the event channel closes, which happens when the `Lobby`
    /// registry — and therefore the process — is gone.
    async fn run(&mut self, mut event_rx: mpsc::Receiver<RoomEvent>) {
        let mut ticker = interval(Duration::from_millis(config::TICK_MS));
        // If the loop ever falls behind (e.g. a long GC pause), skip missed ticks rather than
        // bursting to catch up — the simulation stays close to real time.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Bias is irrelevant for correctness: events are timestamped only by arrival
                // order, and a tick handles whatever has been applied so far.
                _ = ticker.tick() => {
                    self.on_tick();
                }
                maybe_event = event_rx.recv() => {
                    match maybe_event {
                        Some(event) => self.handle_event(event),
                        None => return, // registry dropped; shut the room down.
                    }
                }
            }
        }
    }

    // -- Event handling ------------------------------------------------------

    fn handle_event(&mut self, event: RoomEvent) {
        match event {
            RoomEvent::Join {
                player_id,
                name,
                msg_tx,
                ack,
            } => self.on_join(player_id, name, msg_tx, ack),
            RoomEvent::Leave { player_id } => self.on_leave(player_id),
            RoomEvent::Ready { player_id, ready } => self.on_ready(player_id, ready),
            RoomEvent::StartRequest { player_id } => self.on_start_request(player_id),
            RoomEvent::Command { player_id, cmd } => self.on_command(player_id, cmd),
        }
    }

    fn on_join(
        &mut self,
        player_id: u32,
        name: String,
        msg_tx: mpsc::Sender<ServerMessage>,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.players.contains_key(&player_id) {
            // Defensive: a connection should only ever join once.
            let _ = ack.send(false);
            return;
        }
        if !matches!(self.phase, Phase::Lobby) {
            // The room is mid-match. Joining an in-progress game isn't supported (the player
            // isn't in the live `Game`), so rather than strand them on a silent screen, reject
            // the join with a notice. The client stays on the lobby and can pick another room.
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: "Match already in progress in this room — try another room.".to_string(),
                },
            );
            debug!(room = %self.room, player_id, "rejecting join; match in progress");
            let _ = ack.send(false);
            return;
        }
        let color = PLAYER_PALETTE[self.order.len() % PLAYER_PALETTE.len()].to_string();
        self.order.push(player_id);
        self.players.insert(
            player_id,
            RoomPlayer {
                name,
                color,
                ready: false,
                msg_tx,
            },
        );
        if self.host_id.is_none() {
            self.host_id = Some(player_id);
        }
        debug!(room = %self.room, player_id, "joined");
        // The player is now in the room; tell the connection it may mark itself joined.
        let _ = ack.send(true);
        // A player joining mid-match just sits in the (still in-game) room until it resolves;
        // they receive snapshots immediately. They are not added to the live `Game`.
        if matches!(self.phase, Phase::Lobby) {
            self.broadcast_lobby();
        }
    }

    fn on_leave(&mut self, player_id: u32) {
        if self.players.remove(&player_id).is_none() {
            return;
        }
        self.order.retain(|&id| id != player_id);
        if self.host_id == Some(player_id) {
            // Reassign the host to the next remaining player in join order.
            self.host_id = self.order.first().copied();
        }
        debug!(room = %self.room, player_id, "left");

        // If the room emptied out, fully reset it to a clean lobby so its name is never stuck
        // mid-match (otherwise a 1-player sandbox — which never "ends" — would poison the room
        // for the next person who joins under the same name). The idle room task lives on cheaply.
        if self.players.is_empty() {
            self.phase = Phase::Lobby;
            self.match_player_count = 0;
            self.host_id = None;
            debug!(room = %self.room, "room emptied; reset to lobby");
            return;
        }

        match &mut self.phase {
            Phase::Lobby => self.broadcast_lobby(),
            Phase::InGame(game) => {
                // Remove their army so the match can still resolve to a winner.
                game.eliminate(player_id);
            }
        }
    }

    fn on_ready(&mut self, player_id: u32, ready: bool) {
        if let Phase::Lobby = self.phase {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.ready = ready;
                self.broadcast_lobby();
            }
        }
    }

    fn on_start_request(&mut self, player_id: u32) {
        if !matches!(self.phase, Phase::Lobby) {
            return;
        }
        if self.host_id != Some(player_id) {
            debug!(room = %self.room, player_id, "ignoring start from non-host");
            return;
        }
        if !self.can_start() {
            debug!(room = %self.room, "ignoring start; not all players ready");
            return;
        }
        self.start_match();
    }

    fn on_command(&mut self, player_id: u32, cmd: Command) {
        // Commands are ignored unless in-game and the sender is actually in this room. The
        // simulation itself validates ownership/affordability when it applies the command.
        if let Phase::InGame(game) = &mut self.phase {
            if self.players.contains_key(&player_id) {
                game.enqueue(player_id, cmd);
            }
        }
    }

    // -- Lobby phase ---------------------------------------------------------

    /// A match may start with at least one player present and every player marked ready.
    fn can_start(&self) -> bool {
        !self.players.is_empty() && self.players.values().all(|p| p.ready)
    }

    /// Build and broadcast the current `lobby` message to everyone in the room.
    fn broadcast_lobby(&mut self) {
        let host_id = self.host_id.unwrap_or(0);
        let players: Vec<LobbyPlayer> = self
            .order
            .iter()
            .filter_map(|id| {
                self.players.get(id).map(|p| LobbyPlayer {
                    id: *id,
                    name: p.name.clone(),
                    ready: p.ready,
                    color: p.color.clone(),
                })
            })
            .collect();
        let msg = ServerMessage::Lobby {
            room: self.room.clone(),
            host_id,
            players,
            can_start: self.can_start(),
        };
        self.broadcast(&msg);
    }

    /// Transition from `Lobby` to `InGame`: create the simulation and send each player their
    /// own `start` payload. Only called from `on_start_request` once preconditions hold.
    fn start_match(&mut self) {
        let inits: Vec<PlayerInit> = self
            .order
            .iter()
            .filter_map(|id| {
                self.players.get(id).map(|p| PlayerInit {
                    id: *id,
                    name: p.name.clone(),
                    color: p.color.clone(),
                })
            })
            .collect();

        let game = Game::new(&inits);
        let payload = game.start_payload();
        self.match_player_count = inits.len();

        // Each player gets the shared static payload but stamped with their own id.
        for &id in &self.order {
            if let Some(player) = self.players.get(&id) {
                let per_player = StartPayload {
                    player_id: id,
                    ..payload.clone()
                };
                send_or_log(
                    &self.room,
                    id,
                    &player.msg_tx,
                    ServerMessage::Start(per_player),
                );
            }
        }

        info!(room = %self.room, players = self.match_player_count, "match started");
        self.phase = Phase::InGame(game);
    }

    // -- In-game phase -------------------------------------------------------

    /// One simulation step. No-op in the `Lobby` phase (the ticker keeps running so a room is
    /// always live and ready to start).
    fn on_tick(&mut self) {
        // Take ownership of the game for the duration of the tick so we can both mutate it and
        // freely borrow `self` for sending. Restored (or replaced with `Lobby`) before return.
        let mut game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::Lobby => {
                // Stay in lobby; nothing to simulate.
                self.phase = Phase::Lobby;
                return;
            }
            Phase::InGame(game) => game,
        };

        // Advance the simulation; collect this tick's per-player transient events.
        let mut per_player_events: HashMap<u32, Vec<Event>> =
            game.tick().into_iter().collect::<HashMap<_, _>>();

        // Fan out a fog-filtered snapshot to every connected player, merging in their events.
        let recipients: Vec<u32> = self.order.clone();
        for id in &recipients {
            let Some(player) = self.players.get(id) else {
                continue;
            };
            let mut snapshot = game.snapshot_for(*id);
            if let Some(mut events) = per_player_events.remove(id) {
                snapshot.events.append(&mut events);
            }
            send_or_log(
                &self.room,
                *id,
                &player.msg_tx,
                ServerMessage::Snapshot(snapshot),
            );
        }

        // Check for game over. A 1-player match never ends (sandbox/exploration mode).
        let alive = game.alive_players();
        if self.match_player_count >= 2 && alive.len() <= 1 {
            self.end_match(alive.first().copied());
            // end_match leaves us in the Lobby phase; do not restore the game.
            return;
        }

        self.phase = Phase::InGame(game);
    }

    /// Resolve a finished match: tell everyone who won and return to the lobby for a rematch.
    fn end_match(&mut self, winner_id: Option<u32>) {
        info!(room = %self.room, ?winner_id, "match over");
        let recipients: Vec<u32> = self.order.clone();
        for id in &recipients {
            let Some(player) = self.players.get(id) else {
                continue;
            };
            let you = match winner_id {
                Some(w) if w == *id => "won",
                Some(_) => "lost",
                None => "draw",
            }
            .to_string();
            send_or_log(
                &self.room,
                *id,
                &player.msg_tx,
                ServerMessage::GameOver { winner_id, you },
            );
        }

        // Reset for the next match: drop the game, clear ready flags, and re-advertise the lobby.
        self.phase = Phase::Lobby;
        self.match_player_count = 0;
        for player in self.players.values_mut() {
            player.ready = false;
        }
        self.broadcast_lobby();
    }

    // -- Sending helpers -----------------------------------------------------

    /// Send one message to every connected player. Closed sinks are logged and skipped; the
    /// owning connection task is responsible for emitting the eventual `Leave`.
    fn broadcast(&self, msg: &ServerMessage) {
        for &id in &self.order {
            if let Some(player) = self.players.get(&id) {
                send_or_log(&self.room, id, &player.msg_tx, msg.clone());
            }
        }
    }
}

/// Send to one player's sink without ever blocking the room task. `try_send` is used so a slow
/// or dead client cannot stall the tick loop: a full or closed channel is logged and the message
/// is dropped (snapshots are idempotent, so a dropped one is harmless — the next tick supersedes
/// it). A persistently dead socket is cleaned up when its connection task sends `Leave`.
fn send_or_log(room: &str, player_id: u32, tx: &mpsc::Sender<ServerMessage>, msg: ServerMessage) {
    if let Err(err) = tx.try_send(msg) {
        match err {
            mpsc::error::TrySendError::Full(_) => {
                warn!(room = %room, player_id, "outbound queue full; dropping message");
            }
            mpsc::error::TrySendError::Closed(_) => {
                debug!(room = %room, player_id, "outbound channel closed; client gone");
            }
        }
    }
}

/// Capacity for a new connection's outbound channel. Re-exported so `main.rs` builds the writer
/// channel with the same bound the room expects.
pub const fn player_channel_cap() -> usize {
    PLAYER_CHANNEL_CAP
}
