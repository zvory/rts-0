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
use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, error, info, warn};

use crate::config;
use crate::game::selfplay::{is_safe_artifact_name, LiveSelfPlay, ReplayArtifact, ReplayDriver};
use crate::game::{Game, PlayerInit};
use crate::observability;
use crate::protocol::{ClientPerfReport, Command, Event, LobbyPlayer, ServerMessage, StartPayload};

/// Player colors, assigned by join order. MUST match `client/src/config.js` `PLAYER_PALETTE`.
const PLAYER_PALETTE: [&str; 8] = [
    "#4878c8", "#c84848", "#30a090", "#8040c8", "#c83880", "#c87830", "#409840", "#c8b030",
];

/// Hard cap on players in a single match (humans + AI). The hardcoded map has four authored
/// player-start slots, so we never seat more than this.
const MAX_PLAYERS: usize = 4;

/// Bound on a player's outbound message queue. Generous enough to absorb a brief render stall
/// but small enough that a truly dead client is detected (a full queue ⇒ treated as gone) and
/// dropped instead of buffering unboundedly.
const PLAYER_CHANNEL_CAP: usize = 256;

/// Bound on a room's inbound event queue. Commands/joins past this are dropped rather than
/// allowed to grow without limit; in practice the room drains this every tick.
const ROOM_EVENT_CHANNEL_CAP: usize = 1024;
const DEV_SELFPLAY_ROOM_PREFIX: &str = "__dev_selfplay__";
const MATCH_SEED_ENV: &str = "RTS_MATCH_SEED";

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
    /// The host asked to add a computer opponent (lobby phase only; honored only from the host).
    AddAi { player_id: u32 },
    /// The host asked to remove an AI opponent by id (lobby phase only; honored only from host).
    RemoveAi { player_id: u32, target: u32 },
    /// The host toggled the lobby's start-with-more-money mode.
    SetQuickstart { player_id: u32, enabled: bool },
    /// A gameplay command (ignored unless the room is in-game and the sender is in the room).
    Command { player_id: u32, cmd: Command },
    /// Set replay playback speed multiplier (replay rooms only; ignored elsewhere).
    SetReplaySpeed { speed: f32 },
    /// Browser-side lag/performance report.
    ClientPerf {
        player_id: u32,
        report: ClientPerfReport,
    },
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
        let mode = room_mode_for(&name);
        tokio::spawn(async move {
            let mut task = RoomTask::new(name.clone(), mode);
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

/// A computer opponent seated in a room. Has an id (for the lobby list / removal) and a name, but
/// no socket — it is materialized into an AI-driven player only when the match starts.
struct AiSlot {
    id: u32,
    name: String,
}

/// The room's current mode. `InGame` owns the live simulation outright.
enum Phase {
    Lobby,
    InGame(Box<Game>),
}

#[derive(Clone)]
enum RoomMode {
    Normal,
    DevSelfPlay(DevSelfPlayConfig),
}

#[derive(Clone)]
enum DevSelfPlayConfig {
    Live,
    Replay { artifact: String },
}

enum DevDriver {
    Live(LiveSelfPlay),
    Replay(ReplayDriver),
}

/// All state owned by a single room task. Lives entirely on that task — never shared.
struct RoomTask {
    room: String,
    mode: RoomMode,
    /// Connected players in join order (join order drives color assignment and host fallback).
    order: Vec<u32>,
    players: HashMap<u32, RoomPlayer>,
    /// Computer opponents the host has added, in add order. Persist across rematches; cleared
    /// only when the room empties of humans.
    ai_players: Vec<AiSlot>,
    /// Lobby toggle: start matches with boosted opening resources.
    quickstart: bool,
    /// Current host (first joiner; reassigned to the next in `order` when the host leaves).
    host_id: Option<u32>,
    phase: Phase,
    /// Number of players (humans + AI) the in-progress match started with. Used so a lone-player
    /// sandbox never ends while a 2+ player match (including human-vs-AI) resolves to a winner.
    /// `0` outside a match.
    match_player_count: usize,
    dev_driver: Option<DevDriver>,
    dev_view_player_id: Option<u32>,
    /// Replay speed multiplier; 1.0 = real-time, 2.0 = 2× faster, etc.
    replay_speed: f32,
}

impl RoomTask {
    fn new(room: String, mode: RoomMode) -> Self {
        let replay_speed = match &mode {
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. }) => 1.5,
            _ => 1.0,
        };
        RoomTask {
            room,
            mode,
            order: Vec::new(),
            players: HashMap::new(),
            ai_players: Vec::new(),
            quickstart: false,
            host_id: None,
            phase: Phase::Lobby,
            match_player_count: 0,
            dev_driver: None,
            dev_view_player_id: None,
            replay_speed,
        }
    }

    /// Main loop: multiplex the fixed-rate tick against the inbound event stream. Returns (and
    /// the task ends) only when the event channel closes, which happens when the `Lobby`
    /// registry — and therefore the process — is gone.
    async fn run(&mut self, mut event_rx: mpsc::Receiver<RoomEvent>) {
        let mut ticker = self.make_ticker();

        loop {
            let mut speed_changed = false;
            tokio::select! {
                // Bias is irrelevant for correctness: events are timestamped only by arrival
                // order, and a tick handles whatever has been applied so far.
                _ = ticker.tick() => {
                    self.on_tick();
                }
                maybe_event = event_rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            let old_speed = self.replay_speed;
                            self.handle_event(event);
                            speed_changed = self.replay_speed != old_speed;
                        }
                        None => return, // registry dropped; shut the room down.
                    }
                }
            }
            if speed_changed {
                ticker = self.make_ticker();
            }
        }
    }

    fn make_ticker(&self) -> tokio::time::Interval {
        let dur = self.current_tick_interval();
        let mut t = interval(dur);
        // If the loop ever falls behind (e.g. a long GC pause), skip missed ticks rather than
        // bursting to catch up — the simulation stays close to real time.
        t.set_missed_tick_behavior(MissedTickBehavior::Skip);
        t
    }

    fn current_tick_interval(&self) -> Duration {
        let base = Duration::from_millis(config::TICK_MS);
        base.div_f32(self.replay_speed)
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
            RoomEvent::AddAi { player_id } => self.on_add_ai(player_id),
            RoomEvent::RemoveAi { player_id, target } => self.on_remove_ai(player_id, target),
            RoomEvent::SetQuickstart { player_id, enabled } => {
                self.on_set_quickstart(player_id, enabled)
            }
            RoomEvent::Command { player_id, cmd } => self.on_command(player_id, cmd),
            RoomEvent::SetReplaySpeed { speed } => self.on_set_replay_speed(speed),
            RoomEvent::ClientPerf { player_id, report } => self.on_client_perf(player_id, report),
        }
    }

    fn on_join(
        &mut self,
        player_id: u32,
        name: String,
        msg_tx: mpsc::Sender<ServerMessage>,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if matches!(self.mode, RoomMode::DevSelfPlay(_)) {
            self.on_join_dev_selfplay(player_id, name, msg_tx, ack);
            return;
        }
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
        let was_empty = self.players.is_empty();
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
        observability::global().player_joined();
        if was_empty {
            observability::global().room_activated();
        }
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
        observability::global().player_left();
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
            let was_in_match =
                matches!(self.phase, Phase::InGame(_)) && self.match_player_count > 0;
            observability::global().room_deactivated();
            if was_in_match {
                observability::global().match_ended(self.mode_label());
            }
            self.phase = Phase::Lobby;
            self.match_player_count = 0;
            self.host_id = None;
            // Drop AI opponents too: with no humans there is nobody to host them, and a fresh
            // joiner under this room name should start from a clean lobby.
            self.ai_players.clear();
            self.dev_driver = None;
            self.dev_view_player_id = None;
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
        if matches!(self.mode, RoomMode::DevSelfPlay(_)) {
            return;
        }
        if let Phase::Lobby = self.phase {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.ready = ready;
                self.broadcast_lobby();
            }
        }
    }

    fn on_start_request(&mut self, player_id: u32) {
        if matches!(self.mode, RoomMode::DevSelfPlay(_)) {
            return;
        }
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

    /// Host-only: seat a computer opponent. Ignored outside the lobby, from non-hosts, or once
    /// the room is full (humans + AI == [`MAX_PLAYERS`]).
    fn on_add_ai(&mut self, player_id: u32) {
        if matches!(self.mode, RoomMode::DevSelfPlay(_)) {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.total_player_count() >= MAX_PLAYERS {
            debug!(room = %self.room, "ignoring add-ai; room full");
            return;
        }
        let id = next_player_id();
        let name = format!("Computer {}", self.ai_players.len() + 1);
        self.ai_players.push(AiSlot { id, name });
        debug!(room = %self.room, ai_id = id, "AI opponent added");
        self.broadcast_lobby();
    }

    /// Host-only: remove a previously-added AI opponent by id. Ignored outside the lobby, from
    /// non-hosts, or for an unknown id.
    fn on_remove_ai(&mut self, player_id: u32, target: u32) {
        if matches!(self.mode, RoomMode::DevSelfPlay(_)) {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        let before = self.ai_players.len();
        self.ai_players.retain(|a| a.id != target);
        if self.ai_players.len() != before {
            debug!(room = %self.room, ai_id = target, "AI opponent removed");
            self.broadcast_lobby();
        }
    }

    /// Host-only: toggle the lobby's boosted opening resources.
    fn on_set_quickstart(&mut self, player_id: u32, enabled: bool) {
        if matches!(self.mode, RoomMode::DevSelfPlay(_)) {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.quickstart != enabled {
            self.quickstart = enabled;
            debug!(room = %self.room, enabled, "quickstart toggled");
            self.broadcast_lobby();
        }
    }

    /// Total seated players: connected humans plus AI opponents.
    fn total_player_count(&self) -> usize {
        self.order.len() + self.ai_players.len()
    }

    /// Color for the `seat`-th AI opponent. AI colors are drawn from the *tail* of the palette so
    /// they never collide with human colors (assigned from the head by join order), given the
    /// [`MAX_PLAYERS`] cap.
    fn ai_color(seat: usize) -> String {
        let idx = (PLAYER_PALETTE.len() - 1 - (seat % PLAYER_PALETTE.len())) % PLAYER_PALETTE.len();
        PLAYER_PALETTE[idx].to_string()
    }

    fn on_command(&mut self, player_id: u32, cmd: Command) {
        if matches!(self.mode, RoomMode::DevSelfPlay(_)) {
            return;
        }
        // Commands are ignored unless in-game and the sender is actually in this room. The
        // simulation itself validates ownership/affordability when it applies the command.
        if let Phase::InGame(game) = &mut self.phase {
            if self.players.contains_key(&player_id) {
                game.enqueue(player_id, cmd);
            }
        }
    }

    fn on_client_perf(&self, player_id: u32, report: ClientPerfReport) {
        if !self.players.contains_key(&player_id) {
            return;
        }
        let Some(report) = sanitize_client_perf(report) else {
            debug!(room = %self.room, player_id, "ignoring invalid client perf report");
            return;
        };
        observability::global().record_client_perf(
            &self.room,
            self.mode_label(),
            player_id,
            &report,
        );
    }

    fn mode_label(&self) -> &'static str {
        match &self.mode {
            RoomMode::Normal => "normal",
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Live) => "dev_selfplay_live",
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. }) => "dev_selfplay_replay",
        }
    }

    // -- Lobby phase ---------------------------------------------------------

    fn on_join_dev_selfplay(
        &mut self,
        player_id: u32,
        name: String,
        msg_tx: mpsc::Sender<ServerMessage>,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.players.contains_key(&player_id) {
            let _ = ack.send(false);
            return;
        }
        let was_empty = self.players.is_empty();
        self.order.push(player_id);
        self.players.insert(
            player_id,
            RoomPlayer {
                name,
                color: "#6f8fa8".to_string(),
                ready: true,
                msg_tx,
            },
        );
        observability::global().player_joined();
        if was_empty {
            observability::global().room_activated();
        }
        let _ = ack.send(true);
        if !matches!(self.phase, Phase::InGame(_)) {
            self.start_dev_session();
        } else {
            self.send_dev_start_to(player_id);
        }
    }

    /// A match may start with at least one player present and every player marked ready.
    fn can_start(&self) -> bool {
        !self.players.is_empty() && self.players.values().all(|p| p.ready)
    }

    /// Build and broadcast the current `lobby` message to everyone in the room.
    fn broadcast_lobby(&mut self) {
        let host_id = self.host_id.unwrap_or(0);
        // Humans first (in join order), then AI opponents. AIs always read as ready.
        let mut players: Vec<LobbyPlayer> = self
            .order
            .iter()
            .filter_map(|id| {
                self.players.get(id).map(|p| LobbyPlayer {
                    id: *id,
                    name: p.name.clone(),
                    ready: p.ready,
                    color: p.color.clone(),
                    is_ai: false,
                })
            })
            .collect();
        for (seat, ai) in self.ai_players.iter().enumerate() {
            players.push(LobbyPlayer {
                id: ai.id,
                name: ai.name.clone(),
                ready: true,
                color: Self::ai_color(seat),
                is_ai: true,
            });
        }
        let msg = ServerMessage::Lobby {
            room: self.room.clone(),
            host_id,
            players,
            can_start: self.can_start(),
            quickstart: self.quickstart,
        };
        self.broadcast(&msg);
    }

    /// Transition from `Lobby` to `InGame`: create the simulation and send each player their
    /// own `start` payload. Only called from `on_start_request` once preconditions hold.
    fn start_match(&mut self) {
        let mut inits: Vec<PlayerInit> = self
            .order
            .iter()
            .filter_map(|id| {
                self.players.get(id).map(|p| PlayerInit {
                    id: *id,
                    name: p.name.clone(),
                    color: p.color.clone(),
                    is_ai: false,
                })
            })
            .collect();
        // Seat AI opponents after the humans so colors match the lobby list and authored start
        // sites are assigned in the same order the lobby displays players.
        for (seat, ai) in self.ai_players.iter().enumerate() {
            inits.push(PlayerInit {
                id: ai.id,
                name: ai.name.clone(),
                color: Self::ai_color(seat),
                is_ai: true,
            });
        }

        let (starting_steel, starting_oil) = if self.quickstart {
            (1400, 600)
        } else {
            (config::STARTING_STEEL, config::STARTING_OIL)
        };
        let seed = match_seed();
        let game = Game::new_with_starting_resources(&inits, starting_steel, starting_oil, seed);
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
        observability::global().match_started(self.mode_label(), self.match_player_count);
        self.phase = Phase::InGame(Box::new(game));
    }

    fn start_dev_session(&mut self) {
        let (game, driver, view_player_id) = match self.build_dev_session() {
            Ok(session) => session,
            Err(err) => {
                warn!(room = %self.room, error = %err, "dev self-play bootstrap failed");
                self.send_dev_error(&err);
                return;
            }
        };
        self.phase = Phase::InGame(Box::new(game));
        self.match_player_count = 2;
        self.dev_driver = Some(driver);
        self.dev_view_player_id = Some(view_player_id);
        let recipients = self.order.clone();
        for player_id in recipients {
            self.send_dev_start_to(player_id);
        }
        info!(room = %self.room, "dev self-play session started");
        observability::global().match_started(self.mode_label(), self.match_player_count);
    }

    fn build_dev_session(&self) -> Result<(Game, DevDriver, u32), String> {
        match &self.mode {
            RoomMode::Normal => Err("room is not configured for dev self-play".to_string()),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Live) => {
                let driver = LiveSelfPlay::default_match();
                let players = driver.players().to_vec();
                let view_player_id = players
                    .first()
                    .map(|p| p.id)
                    .ok_or_else(|| "live self-play configured with no players".to_string())?;
                let seed = match_seed();
                let game = Game::new(&players, seed);
                Ok((game, DevDriver::Live(driver), view_player_id))
            }
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { artifact }) => {
                let artifact_name = artifact.clone();
                let artifact = load_replay_artifact(artifact)?;
                let (players, driver) = ReplayDriver::from_artifact(artifact);
                let view_player_id = players
                    .first()
                    .map(|p| p.id)
                    .ok_or_else(|| format!("replay artifact {artifact_name:?} has no players"))?;
                let game = Game::new_for_replay(&players, driver.seed());
                Ok((game, DevDriver::Replay(driver), view_player_id))
            }
        }
    }

    fn send_dev_start_to(&self, watcher_id: u32) {
        let Some(Phase::InGame(game)) = Some(&self.phase) else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        let per_player = StartPayload {
            player_id: self.dev_view_player_id.unwrap_or(watcher_id),
            ..game.start_payload()
        };
        send_or_log(
            &self.room,
            watcher_id,
            &player.msg_tx,
            ServerMessage::Start(per_player),
        );
    }

    fn send_dev_error(&self, msg: &str) {
        let payload = ServerMessage::Error {
            msg: msg.to_string(),
        };
        for &watcher_id in &self.order {
            let Some(player) = self.players.get(&watcher_id) else {
                continue;
            };
            send_or_log(&self.room, watcher_id, &player.msg_tx, payload.clone());
        }
    }

    // -- In-game phase -------------------------------------------------------

    /// One simulation step. No-op in the `Lobby` phase (the ticker keeps running so a room is
    /// always live and ready to start).
    fn on_tick(&mut self) {
        if matches!(self.mode, RoomMode::DevSelfPlay(_)) {
            self.on_tick_dev_selfplay();
            return;
        }
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
        // Wrap in `catch_unwind` so a panic on the tick path (including debug-build invariant
        // failures) writes a replay artifact and resets the room instead of killing the task.
        let tick_started = Instant::now();
        let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()));
        let tick_ms = tick_started.elapsed().as_secs_f64() * 1000.0;
        let mut per_player_events: HashMap<u32, Vec<Event>> = match tick_result {
            Ok(events) => events.into_iter().collect(),
            Err(payload) => {
                let reason = panic_reason(&payload);
                dump_crash_replay(&self.room, &game, &reason);
                self.end_match(None);
                return;
            }
        };

        // Fan out a fog-filtered snapshot to every connected player, merging in their events.
        let snapshot_started = Instant::now();
        let recipients: Vec<u32> = self.order.clone();
        let mut snapshot_recipients = 0usize;
        let mut entities_sent = 0usize;
        for id in &recipients {
            let Some(player) = self.players.get(id) else {
                continue;
            };
            let mut snapshot = game.snapshot_for(*id);
            if let Some(mut events) = per_player_events.remove(id) {
                snapshot.events.append(&mut events);
            }
            snapshot_recipients += 1;
            entities_sent += snapshot.entities.len();
            send_or_log(
                &self.room,
                *id,
                &player.msg_tx,
                ServerMessage::Snapshot(snapshot),
            );
        }
        let snapshot_ms = snapshot_started.elapsed().as_secs_f64() * 1000.0;
        observability::global().record_tick(
            &self.room,
            self.mode_label(),
            game.tick_count(),
            self.match_player_count,
            tick_ms,
            snapshot_ms,
            snapshot_recipients,
            entities_sent,
        );

        // Check for game over. A 1-player match never ends (sandbox/exploration mode).
        let alive = game.alive_players();
        if self.match_player_count >= 2 && alive.len() <= 1 {
            self.end_match(alive.first().copied());
            // end_match leaves us in the Lobby phase; do not restore the game.
            return;
        }

        self.phase = Phase::InGame(game);
    }

    fn on_tick_dev_selfplay(&mut self) {
        let mut game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::Lobby => return,
            Phase::InGame(game) => game,
        };
        let Some(mut driver) = self.dev_driver.take() else {
            self.phase = Phase::InGame(game);
            return;
        };
        match &mut driver {
            DevDriver::Live(scripted) => scripted.enqueue_for_tick(&mut game),
            DevDriver::Replay(replay) => replay.enqueue_for_tick(&mut game),
        }
        let tick_started = Instant::now();
        let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()));
        let tick_ms = tick_started.elapsed().as_secs_f64() * 1000.0;
        let mut per_player_events: HashMap<u32, Vec<Event>> = match tick_result {
            Ok(events) => events.into_iter().collect(),
            Err(payload) => {
                let reason = panic_reason(&payload);
                dump_crash_replay(&self.room, &game, &reason);
                if self.match_player_count > 0 {
                    observability::global().match_ended(self.mode_label());
                }
                self.phase = Phase::Lobby;
                self.match_player_count = 0;
                self.dev_driver = None;
                self.dev_view_player_id = None;
                return;
            }
        };

        let snapshot_started = Instant::now();
        let recipients = self.order.clone();
        let view_player_id = self.dev_view_player_id.unwrap_or(0);
        let mut snapshot_recipients = 0usize;
        let mut entities_sent = 0usize;
        for id in recipients {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            let mut snapshot = game.snapshot_full_for(view_player_id);
            if let Some(mut events) = per_player_events.remove(&id) {
                snapshot.events.append(&mut events);
            }
            snapshot_recipients += 1;
            entities_sent += snapshot.entities.len();
            send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::Snapshot(snapshot),
            );
        }
        let snapshot_ms = snapshot_started.elapsed().as_secs_f64() * 1000.0;
        observability::global().record_tick(
            &self.room,
            self.mode_label(),
            game.tick_count(),
            self.match_player_count,
            tick_ms,
            snapshot_ms,
            snapshot_recipients,
            entities_sent,
        );

        let alive = game.alive_players();
        if alive.len() <= 1 {
            if self.match_player_count > 0 {
                observability::global().match_ended(self.mode_label());
            }
            self.phase = Phase::Lobby;
            self.match_player_count = 0;
            self.dev_driver = None;
            self.dev_view_player_id = None;
            self.start_dev_session();
            return;
        }

        self.dev_driver = Some(driver);
        self.phase = Phase::InGame(game);
    }

    fn on_set_replay_speed(&mut self, speed: f32) {
        if !matches!(
            self.mode,
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. })
        ) {
            return;
        }
        // Clamp to sensible range matching the UI buttons (0.5× – 8×).
        let clamped = speed.clamp(0.125, 8.0);
        self.replay_speed = clamped;
    }

    /// Resolve a finished match: tell everyone who won and return to the lobby for a rematch.
    fn end_match(&mut self, winner_id: Option<u32>) {
        info!(room = %self.room, ?winner_id, "match over");
        if self.match_player_count > 0 {
            observability::global().match_ended(self.mode_label());
        }
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
                observability::global().outbound_dropped("full");
                warn!(room = %room, player_id, "outbound queue full; dropping message");
            }
            mpsc::error::TrySendError::Closed(_) => {
                observability::global().outbound_dropped("closed");
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

fn room_mode_for(room: &str) -> RoomMode {
    if room == format!("{DEV_SELFPLAY_ROOM_PREFIX}live") {
        return RoomMode::DevSelfPlay(DevSelfPlayConfig::Live);
    }
    if let Some(artifact) = room.strip_prefix(&format!("{DEV_SELFPLAY_ROOM_PREFIX}replay:")) {
        return RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay {
            artifact: artifact.to_string(),
        });
    }
    RoomMode::Normal
}

/// Persist a replayable artifact when a room's tick panics (a true crash or, in debug
/// builds, an `assert_invariants` failure). The path is logged and the full file contents
/// are emitted to the log so an operator can copy them out of terminal output even if the
/// disk write later disappears or the box is ephemeral.
fn dump_crash_replay(room: &str, game: &Game, reason: &str) {
    let artifact = ReplayArtifact {
        replay_commands: game.command_log().to_vec(),
        players: game.player_inits(),
        seed: game.seed(),
    };
    let json = match serde_json::to_string_pretty(&artifact) {
        Ok(s) => s,
        Err(e) => {
            error!(room = %room, reason = %reason, error = %e, "tick panic: failed to serialize crash replay");
            return;
        }
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let sanitized: String = room
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let dir_name = format!("crash-{sanitized}-{}-{now_ms}", std::process::id());
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("selfplay-failures")
        .join(&dir_name);
    let path = dir.join("replay.json");
    match fs::create_dir_all(&dir).and_then(|_| fs::write(&path, &json)) {
        Ok(_) => {
            error!(
                room = %room,
                tick = game.tick_count(),
                reason = %reason,
                path = %path.display(),
                "tick panic: crash replay written"
            );
        }
        Err(e) => {
            error!(
                room = %room,
                tick = game.tick_count(),
                reason = %reason,
                error = %e,
                "tick panic: failed to write crash replay; dumping inline only"
            );
        }
    }
    error!(
        room = %room,
        reason = %reason,
        "tick panic: full crash replay follows (artifact name: {dir_name})\n----BEGIN CRASH REPLAY----\n{json}\n----END CRASH REPLAY----"
    );
}

fn panic_reason(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "panic with non-string payload".to_string()
}

fn match_seed() -> u32 {
    if let Ok(raw) = std::env::var(MATCH_SEED_ENV) {
        match raw.parse::<u32>() {
            Ok(seed) => return seed,
            Err(err) => warn!(
                env = MATCH_SEED_ENV,
                value = %raw,
                error = %err,
                "invalid match seed override; using time-based seed"
            ),
        }
    }

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u32)
        .unwrap_or(0x1234_5678)
}

fn load_replay_artifact(name: &str) -> Result<ReplayArtifact, String> {
    if !is_safe_artifact_name(name) {
        return Err("invalid replay artifact name".to_string());
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");
    let candidates = [
        root.join("selfplay-artifacts")
            .join(name)
            .join("replay.json"),
        root.join("selfplay-failures")
            .join(name)
            .join("replay.json"),
    ];
    for path in candidates {
        if let Ok(json) = fs::read_to_string(&path) {
            return serde_json::from_str(&json)
                .map_err(|e| format!("failed to parse replay artifact: {e}"));
        }
    }
    Err(format!(
        "failed to read replay artifact {name:?} from target/selfplay-artifacts or target/selfplay-failures"
    ))
}

fn sanitize_client_perf(mut report: ClientPerfReport) -> Option<ClientPerfReport> {
    report.fps = bounded_f32(report.fps, 0.0, 240.0)?;
    report.avg_frame_ms = bounded_f32(report.avg_frame_ms, 0.0, 10_000.0)?;
    report.max_frame_ms = bounded_f32(report.max_frame_ms, 0.0, 10_000.0)?;
    report.snapshot_gap_ms = sanitize_optional_ms(report.snapshot_gap_ms);
    report.rtt_ms = sanitize_optional_ms(report.rtt_ms);
    report.slow_frames = report.slow_frames.min(10_000);
    Some(report)
}

fn sanitize_optional_ms(value: Option<f32>) -> Option<f32> {
    value.and_then(|v| bounded_f32(v, 0.0, 60_000.0))
}

fn bounded_f32(value: f32, min: f32, max: f32) -> Option<f32> {
    if value.is_finite() && value >= min && value <= max {
        Some(value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_rooms_default_to_1_5x_speed() {
        let normal = RoomTask::new("r".to_string(), RoomMode::Normal);
        let live = RoomTask::new(
            "r".to_string(),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Live),
        );
        let replay = RoomTask::new(
            "r".to_string(),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay {
                artifact: "demo".to_string(),
            }),
        );
        assert_eq!(normal.current_tick_interval(), Duration::from_millis(33));
        assert_eq!(live.current_tick_interval(), Duration::from_millis(33));
        // 33ms / 1.5 = 22ms
        assert_eq!(replay.current_tick_interval(), Duration::from_millis(22));
    }

    #[test]
    fn replay_speed_clamped_and_applied() {
        let mut task = RoomTask::new(
            "r".to_string(),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay {
                artifact: "demo".to_string(),
            }),
        );
        task.on_set_replay_speed(2.0);
        // 33ms / 2.0 = 16.5ms → rounds to 16ms via div_f32
        assert!(task.current_tick_interval() < Duration::from_millis(17));
        assert!(task.current_tick_interval() > Duration::from_millis(15));
    }

    #[test]
    fn client_perf_reports_are_bounded() {
        let good = sanitize_client_perf(ClientPerfReport {
            fps: 59.0,
            avg_frame_ms: 16.8,
            max_frame_ms: 55.0,
            snapshot_gap_ms: Some(80.0),
            rtt_ms: Some(25.0),
            slow_frames: 2,
        })
        .expect("valid report");
        assert_eq!(good.slow_frames, 2);

        let bad_required = sanitize_client_perf(ClientPerfReport {
            fps: f32::INFINITY,
            avg_frame_ms: 16.8,
            max_frame_ms: 55.0,
            snapshot_gap_ms: Some(80.0),
            rtt_ms: Some(25.0),
            slow_frames: 2,
        });
        assert!(bad_required.is_none());

        let bad_optional = sanitize_client_perf(ClientPerfReport {
            fps: 59.0,
            avg_frame_ms: 16.8,
            max_frame_ms: 55.0,
            snapshot_gap_ms: Some(90_000.0),
            rtt_ms: Some(25.0),
            slow_frames: 20_000,
        })
        .expect("invalid optional values are dropped");
        assert_eq!(bad_optional.snapshot_gap_ms, None);
        assert_eq!(bad_optional.slow_frames, 10_000);
    }
}
