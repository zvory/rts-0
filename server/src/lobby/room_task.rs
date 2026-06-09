use super::connection::send_or_log;
use super::connection::SnapshotSendStatus;
use super::crash_replay::{dump_crash_replay, panic_reason};
use super::dev_replay::{load_replay_artifact, match_seed};
use super::snapshots::{compact_snapshot_for_wire, union_events};
use super::*;
use crate::game::entity::EntityKind;
use crate::game::map::Map;
use crate::protocol::SnapshotNetStatus;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use rts_ai::{AiController, AiThinkContext};
use std::time::Instant as StdInstant;
use tokio::time::Instant as TokioInstant;

/// A connected player as tracked inside a room.
pub(super) struct RoomPlayer {
    name: String,
    pub(super) color: String,
    ready: bool,
    spectator: bool,
    msg_tx: ConnectionSink,
    head_of_line_count: u32,
}

/// A computer opponent seated in a room. Has an id (for the lobby list / removal) and a name, but
/// no socket — it is materialized into an AI-driven player only when the match starts.
struct AiSlot {
    id: u32,
    name: String,
}

const AUTOMATED_MATCH_HISTORY_ROOM_PREFIXES: [&str; 4] =
    ["itest-", "ai-itest-", "client-smoke-", "reg-"];

pub(super) fn is_automated_match_history_room(room: &str) -> bool {
    AUTOMATED_MATCH_HISTORY_ROOM_PREFIXES
        .iter()
        .any(|prefix| room.starts_with(prefix))
}

pub(super) fn match_history_participants_are_automated(participants: &[String]) -> bool {
    let mut has_alpha = false;
    let mut has_bravo = false;
    for participant in participants {
        let name = participant.trim();
        if name.starts_with("Computer ") || name.eq_ignore_ascii_case("smoke") {
            return true;
        }
        has_alpha |= name == "Alpha";
        has_bravo |= name == "Bravo";
    }
    has_alpha && has_bravo
}

fn live_ai_controllers(players: &[PlayerInit], seed: u32) -> Vec<AiController> {
    let mut rng = SmallRng::seed_from_u64((seed as u64) ^ 0xA17E_5EED);
    players
        .iter()
        .filter(|player| player.is_ai)
        .map(|player| {
            AiController::with_profile_id(player.id, rts_ai::random_live_profile_id(&mut rng))
        })
        .collect()
}

/// The room's current mode. `InGame` owns the live simulation outright.
enum Phase {
    Lobby,
    InGame(Box<Game>),
}

#[derive(Clone)]
pub(super) enum RoomMode {
    Normal,
    DevSelfPlay(DevSelfPlayConfig),
    DevScenario(DevScenarioConfig),
}

#[derive(Clone)]
pub(super) enum DevSelfPlayConfig {
    Live,
    Replay { artifact: String },
}

#[derive(Clone)]
pub(super) struct DevScenarioConfig {
    pub(super) id: DevScenarioId,
    pub(super) unit: EntityKind,
    pub(super) count: usize,
    pub(super) blocker: Option<EntityKind>,
}

#[derive(Clone)]
pub(super) enum DevScenarioId {
    ScoutCarSnakingCorridor,
    DirectReverseOrder,
    ScoutCarWallChokepoint,
    VehicleSmallBlockBaseline,
}

enum DevDriver {
    Live(LiveSelfPlay),
    Replay(ReplayDriver),
    Scenario(DevScenarioDriver),
}

struct DevScenarioDriver {
    player_id: u32,
    units: Vec<u32>,
    goal: (f32, f32),
    issued: bool,
}

impl DevScenarioDriver {
    fn enqueue_for_tick(&mut self, game: &mut Game) {
        if self.issued {
            return;
        }
        self.issued = true;
        game.enqueue(
            self.player_id,
            SimCommand::Move {
                units: self.units.clone(),
                x: self.goal.0,
                y: self.goal.1,
                queued: false,
            },
        );
    }
}

/// All state owned by a single room task. Lives entirely on that task — never shared.
pub(super) struct RoomTask {
    room: String,
    mode: RoomMode,
    /// Connected players in join order (join order drives lobby display and host fallback).
    order: Vec<u32>,
    pub(super) players: HashMap<u32, RoomPlayer>,
    /// Computer opponents the host has added, in add order. Persist across rematches; cleared
    /// only when the room empties of humans.
    ai_players: Vec<AiSlot>,
    /// Lobby toggle: start matches with boosted opening resources.
    quickstart: bool,
    /// Name of the map the host has selected (display name from JSON `name` field).
    selected_map: String,
    /// Current host (first joiner; reassigned to the next in `order` when the host leaves).
    host_id: Option<u32>,
    phase: Phase,
    /// Number of players (humans + AI) the in-progress match started with. Used so a lone-player
    /// sandbox never ends while a 2+ player match (including human-vs-AI) resolves to a winner.
    /// `0` outside a match.
    match_player_count: usize,
    /// Number of human (non-AI) players the in-progress match started with. Only matches where
    /// this is >= 2 are written to the DB. `0` outside a match.
    match_human_count: usize,
    /// Connected human players who already received a terminal score screen for the active match.
    outcome_sent: HashSet<u32>,
    dev_driver: Option<DevDriver>,
    dev_view_player_id: Option<u32>,
    ai_controllers: Vec<AiController>,
    /// Replay speed multiplier; 1.0 = real-time, 2.0 = 2× faster, etc.
    replay_speed: f32,
    slow_tick_count: u32,
    /// Optional persistence sink for resolved matches. `None` disables match-history writes.
    db: Option<Arc<Db>>,
    /// When true, rows written by this room are hidden from non-localhost match-history reads.
    match_history_local_only: bool,
    /// Wall-clock start time of the currently-running match. `None` outside `Phase::InGame`.
    match_started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Map name the active match was started on. Empty outside `Phase::InGame`.
    match_map_name: String,
    /// Display names of every participant (humans + AI) in seat order, for match-history rows.
    match_participants: Vec<String>,
}

impl RoomTask {
    pub(super) fn new(
        room: String,
        mode: RoomMode,
        db: Option<Arc<Db>>,
        match_history_local_only: bool,
    ) -> Self {
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
            selected_map: "Default".to_string(),
            host_id: None,
            phase: Phase::Lobby,
            match_player_count: 0,
            match_human_count: 0,
            outcome_sent: HashSet::new(),
            dev_driver: None,
            dev_view_player_id: None,
            ai_controllers: Vec::new(),
            replay_speed,
            slow_tick_count: 0,
            db,
            match_history_local_only,
            match_started_at: None,
            match_map_name: String::new(),
            match_participants: Vec::new(),
        }
    }

    /// Main loop: multiplex the fixed-rate tick against the inbound event stream. Returns (and
    /// the task ends) only when the event channel closes, which happens when the `Lobby`
    /// registry — and therefore the process — is gone.
    pub(super) async fn run(&mut self, mut event_rx: mpsc::Receiver<RoomEvent>) {
        let mut ticker = self.make_ticker();

        loop {
            let mut speed_changed = false;
            tokio::select! {
                // Bias is irrelevant for correctness: events are timestamped only by arrival
                // order, and a tick handles whatever has been applied so far.
                scheduled = ticker.tick() => {
                    self.on_tick(scheduled);
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

    pub(super) fn current_tick_interval(&self) -> Duration {
        let base =
            test_tick_interval_override().unwrap_or_else(|| Duration::from_millis(config::TICK_MS));
        base.div_f32(self.replay_speed)
    }

    fn is_dev_watch(&self) -> bool {
        matches!(
            self.mode,
            RoomMode::DevSelfPlay(_) | RoomMode::DevScenario(_)
        )
    }

    fn should_record_match_history(&self) -> bool {
        self.match_human_count >= 2
            && !self.is_dev_watch()
            && !is_automated_match_history_room(&self.room)
            && !match_history_participants_are_automated(&self.match_participants)
    }

    // -- Event handling ------------------------------------------------------

    fn handle_event(&mut self, event: RoomEvent) {
        match event {
            RoomEvent::Join {
                player_id,
                name,
                spectator,
                msg_tx,
                ack,
            } => self.on_join(player_id, name, spectator, msg_tx, ack),
            RoomEvent::Leave { player_id } => self.on_leave(player_id),
            RoomEvent::Ready { player_id, ready } => self.on_ready(player_id, ready),
            RoomEvent::StartRequest { player_id } => self.on_start_request(player_id),
            RoomEvent::AddAi { player_id } => self.on_add_ai(player_id),
            RoomEvent::RemoveAi { player_id, target } => self.on_remove_ai(player_id, target),
            RoomEvent::SetQuickstart { player_id, enabled } => {
                self.on_set_quickstart(player_id, enabled)
            }
            RoomEvent::SetSpectator {
                player_id,
                spectator,
            } => self.on_set_spectator(player_id, spectator),
            RoomEvent::Command { player_id, cmd } => self.on_command(player_id, cmd),
            RoomEvent::GiveUp { player_id } => self.on_give_up(player_id),
            RoomEvent::SetReplaySpeed { speed } => self.on_set_replay_speed(speed),
            RoomEvent::SeekReplay { ticks_back } => self.on_seek_replay(ticks_back),
            RoomEvent::SelectMap { player_id, map } => self.on_select_map(player_id, map),
        }
    }

    pub(super) fn on_join(
        &mut self,
        player_id: u32,
        name: String,
        spectator: bool,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.is_dev_watch() {
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
        let color = if spectator {
            "#6f8fa8".to_string()
        } else {
            self.next_human_color()
        };
        self.order.push(player_id);
        self.players.insert(
            player_id,
            RoomPlayer {
                name,
                color,
                ready: false,
                spectator,
                msg_tx,
                head_of_line_count: 0,
            },
        );
        self.reassign_host_if_needed();
        debug!(room = %self.room, player_id, "joined");
        // The player is now in the room; tell the connection it may mark itself joined.
        let _ = ack.send(true);
        if matches!(self.phase, Phase::Lobby) {
            self.broadcast_lobby();
        }
    }

    pub(super) fn on_leave(&mut self, player_id: u32) {
        let Some(removed) = self.players.remove(&player_id) else {
            return;
        };
        let was_spectator = removed.spectator;
        self.order.retain(|&id| id != player_id);
        self.outcome_sent.remove(&player_id);
        self.reassign_host_if_needed();
        debug!(room = %self.room, player_id, "left");

        // If the room emptied out, fully reset it to a clean lobby so its name is never stuck
        // mid-match (otherwise a 1-player sandbox — which never "ends" — would poison the room
        // for the next person who joins under the same name). The idle room task lives on cheaply.
        if self.players.is_empty() {
            self.phase = Phase::Lobby;
            self.match_player_count = 0;
            self.match_human_count = 0;
            self.outcome_sent.clear();
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
                if !was_spectator {
                    game.eliminate(player_id);
                }
            }
        }
    }

    fn on_ready(&mut self, player_id: u32, ready: bool) {
        if self.is_dev_watch() {
            return;
        }
        if let Phase::Lobby = self.phase {
            if let Some(player) = self.players.get_mut(&player_id) {
                if player.spectator {
                    return;
                }
                player.ready = ready;
                self.broadcast_lobby();
            }
        }
    }

    fn on_start_request(&mut self, player_id: u32) {
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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

    /// Host-only: select a map by name. Ignored outside the lobby or from non-hosts.
    fn on_select_map(&mut self, player_id: u32, map: String) {
        if self.is_dev_watch() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.selected_map != map {
            self.selected_map = map;
            debug!(room = %self.room, map = %self.selected_map, "map selected");
            self.broadcast_lobby();
        }
    }

    fn on_set_spectator(&mut self, player_id: u32, spectator: bool) {
        if self.is_dev_watch() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) {
            return;
        }
        let current = self.players.get(&player_id).map(|p| p.spectator);
        if current == Some(spectator) || current.is_none() {
            return;
        }
        if spectator {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.spectator = true;
                player.ready = false;
                player.color = "#6f8fa8".to_string();
            }
        } else {
            if self.total_player_count() >= MAX_PLAYERS {
                debug!(room = %self.room, player_id, "ignoring player role switch; room full");
                return;
            }
            let color = self.next_human_color();
            if let Some(player) = self.players.get_mut(&player_id) {
                player.spectator = false;
                player.ready = false;
                player.color = color;
            }
        }
        self.broadcast_lobby();
    }

    /// Total seated players: connected humans plus AI opponents.
    fn total_player_count(&self) -> usize {
        self.active_human_count() + self.ai_players.len()
    }

    fn active_human_count(&self) -> usize {
        self.order
            .iter()
            .filter(|id| self.players.get(id).map(|p| !p.spectator).unwrap_or(false))
            .count()
    }

    fn active_human_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.order
            .iter()
            .copied()
            .filter(|id| self.players.get(id).map(|p| !p.spectator).unwrap_or(false))
    }

    fn spectator_visible_player_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.active_human_ids().collect();
        ids.extend(self.ai_players.iter().map(|ai| ai.id));
        ids
    }

    fn reassign_host_if_needed(&mut self) {
        if self
            .host_id
            .and_then(|id| self.players.get(&id).map(|_| id))
            .is_some()
        {
            return;
        }
        self.host_id = self.order.first().copied();
    }

    /// Pick the first palette color not currently held by a human player. Join order alone is
    /// not enough because earlier seats can leave while later players keep their colors.
    fn next_human_color(&self) -> String {
        PLAYER_PALETTE
            .iter()
            .copied()
            .find(|color| !self.players.values().any(|p| p.color == *color))
            .unwrap_or(PLAYER_PALETTE[self.active_human_count() % PLAYER_PALETTE.len()])
            .to_string()
    }

    /// Color for the `seat`-th AI opponent. AI colors are drawn from the *tail* of the palette so
    /// they never collide with human colors (assigned from the first available head colors),
    /// given the [`MAX_PLAYERS`] cap.
    fn ai_color(seat: usize) -> String {
        let idx = (PLAYER_PALETTE.len() - 1 - (seat % PLAYER_PALETTE.len())) % PLAYER_PALETTE.len();
        PLAYER_PALETTE[idx].to_string()
    }

    fn on_command(&mut self, player_id: u32, cmd: SimCommand) {
        if self.is_dev_watch() {
            return;
        }
        // Commands are ignored unless in-game and the sender is actually in this room. The
        // simulation itself validates ownership/affordability when it applies the command.
        if let Phase::InGame(game) = &mut self.phase {
            let active_player = self
                .players
                .get(&player_id)
                .map(|p| !p.spectator)
                .unwrap_or(false);
            if active_player && !self.outcome_sent.contains(&player_id) {
                game.enqueue(player_id, cmd);
            }
        }
    }

    fn on_give_up(&mut self, player_id: u32) {
        if self.is_dev_watch() {
            return;
        }
        let active_player = self
            .players
            .get(&player_id)
            .map(|p| !p.spectator)
            .unwrap_or(false);
        if !active_player || self.outcome_sent.contains(&player_id) {
            return;
        }

        let mut game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::Lobby => {
                self.phase = Phase::Lobby;
                return;
            }
            Phase::InGame(game) => game,
        };

        debug!(room = %self.room, player_id, "player gave up");
        game.eliminate(player_id);
        let alive = game.alive_players();
        let scores = game.scores();

        if self.match_player_count >= 2 && alive.len() <= 1 {
            self.end_match(alive.first().copied(), scores);
            return;
        }

        if let Some(player) = self.players.get(&player_id) {
            send_or_log(
                &self.room,
                player_id,
                &player.msg_tx,
                ServerMessage::GameOver {
                    winner_id: None,
                    you: "lost".to_string(),
                    scores,
                },
            );
            self.outcome_sent.insert(player_id);
        }

        if self.match_player_count < 2 {
            self.phase = Phase::Lobby;
            self.match_player_count = 0;
            self.match_human_count = 0;
            self.outcome_sent.clear();
            for player in self.players.values_mut() {
                player.ready = false;
            }
            self.broadcast_lobby();
        } else {
            self.phase = Phase::InGame(game);
        }
    }

    // -- Lobby phase ---------------------------------------------------------

    fn on_join_dev_selfplay(
        &mut self,
        player_id: u32,
        name: String,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.players.contains_key(&player_id) {
            let _ = ack.send(false);
            return;
        }
        self.order.push(player_id);
        self.players.insert(
            player_id,
            RoomPlayer {
                name,
                color: "#6f8fa8".to_string(),
                ready: true,
                spectator: true,
                msg_tx,
                head_of_line_count: 0,
            },
        );
        let _ = ack.send(true);
        if !matches!(self.phase, Phase::InGame(_)) {
            self.start_dev_session();
        } else {
            self.send_dev_start_to(player_id);
        }
    }

    /// A match may start with at least one active participant and every active human ready.
    /// Spectators can host and watch from the lobby, but they do not block readiness.
    fn can_start(&self) -> bool {
        self.total_player_count() > 0
            && self
                .players
                .values()
                .filter(|p| !p.spectator)
                .all(|p| p.ready)
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
                    is_spectator: p.spectator,
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
                is_spectator: false,
            });
        }
        let msg = ServerMessage::Lobby {
            room: self.room.clone(),
            host_id,
            players,
            can_start: self.can_start(),
            quickstart: self.quickstart,
            map: self.selected_map.clone(),
            maps: Map::list_available(),
        };
        self.broadcast(&msg);
    }

    /// Transition from `Lobby` to `InGame`: create the simulation and send each player their
    /// own `start` payload. Only called from `on_start_request` once preconditions hold.
    fn start_match(&mut self) {
        self.reset_match_net_status();
        let mut inits: Vec<PlayerInit> = self
            .active_human_ids()
            .filter_map(|id| {
                self.players.get(&id).map(|p| PlayerInit {
                    id,
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
            (config::QUICKSTART_STEEL, config::QUICKSTART_OIL)
        } else {
            (config::STARTING_STEEL, config::STARTING_OIL)
        };
        let seed = match_seed();

        // Load the selected map from disk. On failure, send an error to the host and abort.
        let map = match Map::load(&self.selected_map, inits.len(), seed) {
            Ok(m) => m,
            Err(err) => {
                let msg = format!("Cannot load map \"{}\": {err}", self.selected_map);
                warn!(room = %self.room, error = %err, "map load failed at start");
                if let Some(host_id) = self.host_id {
                    if let Some(player) = self.players.get(&host_id) {
                        send_or_log(
                            &self.room,
                            host_id,
                            &player.msg_tx,
                            ServerMessage::Error { msg },
                        );
                    }
                }
                return;
            }
        };

        let game = if self.quickstart {
            Game::new_with_debug_starting_loadout_and_random_ai_profiles_and_map(
                &inits,
                starting_steel,
                starting_oil,
                seed,
                map,
            )
        } else {
            Game::new_with_random_ai_profiles_and_map(&inits, seed, map)
        };
        let payload = game.start_payload();
        self.match_player_count = inits.len();
        self.match_human_count = inits.iter().filter(|p| !p.is_ai).count();
        self.match_started_at = Some(chrono::Utc::now());
        self.match_map_name = self.selected_map.clone();
        self.match_participants = inits.iter().map(|p| p.name.clone()).collect();
        self.outcome_sent.clear();
        self.ai_controllers = live_ai_controllers(&inits, seed);

        // Each player gets the shared static payload but stamped with their own id.
        for &id in &self.order {
            if let Some(player) = self.players.get(&id) {
                let per_player = StartPayload {
                    player_id: id,
                    spectator: player.spectator,
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
        self.phase = Phase::InGame(Box::new(game));
    }

    fn start_dev_session(&mut self) {
        self.reset_match_net_status();
        let (game, driver, view_player_id) = match self.build_dev_session() {
            Ok(session) => session,
            Err(err) => {
                warn!(room = %self.room, error = %err, "dev session bootstrap failed");
                self.send_dev_error(&err);
                return;
            }
        };
        self.phase = Phase::InGame(Box::new(game));
        self.match_player_count = 2;
        self.dev_driver = Some(driver);
        self.dev_view_player_id = Some(view_player_id);
        self.ai_controllers.clear();
        let recipients = self.order.clone();
        for player_id in recipients {
            self.send_dev_start_to(player_id);
        }
        info!(room = %self.room, "dev session started");
    }

    fn build_dev_session(&self) -> Result<(Game, DevDriver, u32), String> {
        match &self.mode {
            RoomMode::Normal => Err("room is not configured for a dev session".to_string()),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Live) => {
                let driver = LiveSelfPlay::default_match();
                let players = driver.players().to_vec();
                let view_player_id = players
                    .first()
                    .map(|p| p.id)
                    .ok_or_else(|| "live self-play configured with no players".to_string())?;
                let seed = match_seed();
                let game = Game::new_without_ai_controllers(&players, seed);
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
                let game = Game::new_for_replay_with_starting_resources(
                    &players,
                    driver.starting_steel(),
                    driver.starting_oil(),
                    driver.seed(),
                );
                Ok((game, DevDriver::Replay(driver), view_player_id))
            }
            RoomMode::DevScenario(config) => match config.id {
                DevScenarioId::ScoutCarSnakingCorridor => {
                    let setup = Game::new_snaking_corridor_scenario(
                        config.unit,
                        config.count,
                        match_seed(),
                    )?;
                    let driver = DevScenarioDriver {
                        player_id: setup.player_id,
                        units: setup.units,
                        goal: setup.goal,
                        issued: false,
                    };
                    Ok((setup.game, DevDriver::Scenario(driver), setup.player_id))
                }
                DevScenarioId::DirectReverseOrder => {
                    let setup = Game::new_direct_reverse_order_scenario(
                        config.unit,
                        config.count,
                        match_seed(),
                    )?;
                    let driver = DevScenarioDriver {
                        player_id: setup.player_id,
                        units: setup.units,
                        goal: setup.goal,
                        issued: false,
                    };
                    Ok((setup.game, DevDriver::Scenario(driver), setup.player_id))
                }
                DevScenarioId::ScoutCarWallChokepoint => {
                    let setup = Game::new_scout_car_wall_chokepoint_scenario(
                        config.unit,
                        config.count,
                        match_seed(),
                    )?;
                    let driver = DevScenarioDriver {
                        player_id: setup.player_id,
                        units: setup.units,
                        goal: setup.goal,
                        issued: false,
                    };
                    Ok((setup.game, DevDriver::Scenario(driver), setup.player_id))
                }
                DevScenarioId::VehicleSmallBlockBaseline => {
                    let setup = Game::new_vehicle_small_block_baseline_scenario(
                        config.unit,
                        config.count,
                        config.blocker,
                        match_seed(),
                    )?;
                    let driver = DevScenarioDriver {
                        player_id: setup.player_id,
                        units: setup.units,
                        goal: setup.goal,
                        issued: false,
                    };
                    Ok((setup.game, DevDriver::Scenario(driver), setup.player_id))
                }
            },
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
            spectator: true,
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
    fn on_tick(&mut self, scheduled: TokioInstant) {
        if self.is_dev_watch() {
            self.on_tick_dev_selfplay(scheduled);
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
        let scheduler_lag = scheduled.elapsed();
        let tick_start = StdInstant::now();
        let mut perf = crate::perf::TickPerf::maybe_new();

        // Advance the simulation; collect this tick's per-player transient events.
        // Wrap in `catch_unwind` so a panic on the tick path (including debug-build invariant
        // failures) writes a replay artifact and resets the room instead of killing the task.
        let game_tick_start = StdInstant::now();
        let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.enqueue_live_ai_commands(&mut game, perf.as_mut());
            game.tick_with_perf(perf.as_mut())
        }));
        if let Some(perf) = perf.as_mut() {
            perf.record_phase("game_tick", game_tick_start.elapsed());
        }
        let mut per_player_events: HashMap<u32, Vec<Event>> = match tick_result {
            Ok(events) => events.into_iter().collect(),
            Err(payload) => {
                let reason = panic_reason(&payload);
                dump_crash_replay(&self.room, &game, &reason);
                self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
                self.end_match(None, game.scores());
                return;
            }
        };
        let full_vision_events = crate::perf::timed(perf.as_mut(), "event_union", || {
            union_events(per_player_events.values())
        });

        // Fan out fog-filtered snapshots to active players and union-fog snapshots to lobby-time
        // spectators, merging in the events each recipient is allowed to observe.
        let tick_budget = self.current_tick_interval();
        let mut slow_tick_counted = false;
        let fanout_start = StdInstant::now();
        let recipients: Vec<u32> = self.order.clone();
        let spectator_visible_players = self.spectator_visible_player_ids();
        for id in &recipients {
            if self.outcome_sent.contains(id) {
                continue;
            }
            let Some(player) = self.players.get(id) else {
                continue;
            };
            let snapshot_start = StdInstant::now();
            let mut snapshot = if player.spectator {
                game.snapshot_for_spectator(&spectator_visible_players)
            } else {
                game.snapshot_for(*id)
            };
            if player.spectator {
                snapshot.events.extend(full_vision_events.clone());
            } else if let Some(mut events) = per_player_events.remove(id) {
                snapshot.events.append(&mut events);
            }
            let snapshot_duration = snapshot_start.elapsed();
            let entity_count = snapshot.entities.len();
            let resource_delta_count = snapshot.resource_deltas.len();
            let event_count = snapshot.events.len();
            let tick_elapsed = tick_start.elapsed();
            let slow_tick = scheduler_lag >= tick_budget || tick_elapsed >= tick_budget;
            if slow_tick && !slow_tick_counted {
                self.slow_tick_count = self.slow_tick_count.saturating_add(1);
                slow_tick_counted = true;
            }
            snapshot.net_status =
                self.snapshot_net_status(player, scheduler_lag, tick_elapsed, slow_tick);
            let compact_start = StdInstant::now();
            compact_snapshot_for_wire(&mut snapshot);
            let compact_duration = compact_start.elapsed();
            if let Some(perf) = perf.as_mut() {
                perf.record_snapshot(crate::perf::SnapshotRecord {
                    player_id: *id,
                    spectator: player.spectator,
                    snapshot: snapshot_duration,
                    compact: compact_duration,
                    entities: entity_count,
                    resource_deltas: resource_delta_count,
                    events: event_count,
                });
            }
            let enqueue_status = send_or_log(
                &self.room,
                *id,
                &player.msg_tx,
                ServerMessage::Snapshot(snapshot),
            );
            if matches!(enqueue_status, Some(SnapshotSendStatus::Replaced)) {
                if let Some(player) = self.players.get_mut(id) {
                    player.head_of_line_count = player.head_of_line_count.saturating_add(1);
                }
            }
            if let (Some(perf), Some(status)) = (perf.as_mut(), enqueue_status) {
                perf.record_enqueue(snapshot_enqueue_status(status));
            }
        }
        if let Some(perf) = perf.as_mut() {
            perf.record_phase("snapshot_fanout", fanout_start.elapsed());
        }

        // Check for game over. A 1-player match never ends (sandbox/exploration mode).
        let outcome_start = StdInstant::now();
        let alive = game.alive_players();
        if self.match_player_count >= 2 && alive.len() <= 1 {
            if let Some(perf) = perf.as_mut() {
                perf.record_phase("outcome_checks", outcome_start.elapsed());
            }
            self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
            self.end_match(alive.first().copied(), game.scores());
            // end_match leaves us in the Lobby phase; do not restore the game.
            return;
        }
        if self.match_player_count >= 2 {
            self.send_new_defeats(&game, &alive);
        }
        if let Some(perf) = perf.as_mut() {
            perf.record_phase("outcome_checks", outcome_start.elapsed());
        }

        self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
        self.phase = Phase::InGame(game);
    }

    fn enqueue_live_ai_commands(
        &mut self,
        game: &mut Game,
        perf: Option<&mut crate::perf::TickPerf>,
    ) {
        crate::perf::timed(perf, "ai_think", || {
            if self.ai_controllers.is_empty() {
                return;
            }
            let start = game.start_payload();
            let mut commands = Vec::new();
            for controller in &mut self.ai_controllers {
                let player_id = controller.player_id();
                if !game.alive_players().contains(&player_id) {
                    continue;
                }
                let snapshot = game.snapshot_for(player_id);
                commands.extend(
                    controller
                        .think(AiThinkContext {
                            start: &start,
                            snapshot: &snapshot,
                            retreat_commands: game.worker_retreat_commands_for(player_id),
                        })
                        .into_iter()
                        .map(|command| (player_id, command)),
                );
            }
            for (player_id, command) in commands {
                game.enqueue(player_id, command);
            }
        });
    }

    fn on_tick_dev_selfplay(&mut self, scheduled: TokioInstant) {
        let mut game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::Lobby => return,
            Phase::InGame(game) => game,
        };
        let scheduler_lag = scheduled.elapsed();
        let tick_start = StdInstant::now();
        let mut perf = crate::perf::TickPerf::maybe_new();
        let Some(mut driver) = self.dev_driver.take() else {
            self.phase = Phase::InGame(game);
            return;
        };
        crate::perf::timed(perf.as_mut(), "dev_driver_enqueue", || match &mut driver {
            DevDriver::Live(scripted) => scripted.enqueue_for_tick(&mut game),
            DevDriver::Replay(replay) => replay.enqueue_for_tick(&mut game),
            DevDriver::Scenario(scenario) => scenario.enqueue_for_tick(&mut game),
        });
        let game_tick_start = StdInstant::now();
        let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            game.tick_with_perf(perf.as_mut())
        }));
        if let Some(perf) = perf.as_mut() {
            perf.record_phase("game_tick", game_tick_start.elapsed());
        }
        let mut per_player_events: HashMap<u32, Vec<Event>> = match tick_result {
            Ok(events) => events.into_iter().collect(),
            Err(payload) => {
                let reason = panic_reason(&payload);
                dump_crash_replay(&self.room, &game, &reason);
                self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
                self.phase = Phase::Lobby;
                self.dev_driver = None;
                self.dev_view_player_id = None;
                return;
            }
        };

        let tick_budget = self.current_tick_interval();
        let mut slow_tick_counted = false;
        let fanout_start = StdInstant::now();
        let recipients = self.order.clone();
        let view_player_id = self.dev_view_player_id.unwrap_or(0);
        for id in recipients {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            let snapshot_start = StdInstant::now();
            let mut snapshot = game.snapshot_full_for(view_player_id);
            if let Some(mut events) = per_player_events.remove(&view_player_id) {
                snapshot.events.append(&mut events);
            }
            let snapshot_duration = snapshot_start.elapsed();
            let entity_count = snapshot.entities.len();
            let resource_delta_count = snapshot.resource_deltas.len();
            let event_count = snapshot.events.len();
            let tick_elapsed = tick_start.elapsed();
            let slow_tick = scheduler_lag >= tick_budget || tick_elapsed >= tick_budget;
            if slow_tick && !slow_tick_counted {
                self.slow_tick_count = self.slow_tick_count.saturating_add(1);
                slow_tick_counted = true;
            }
            snapshot.net_status =
                self.snapshot_net_status(player, scheduler_lag, tick_elapsed, slow_tick);
            let compact_start = StdInstant::now();
            compact_snapshot_for_wire(&mut snapshot);
            let compact_duration = compact_start.elapsed();
            if let Some(perf) = perf.as_mut() {
                perf.record_snapshot(crate::perf::SnapshotRecord {
                    player_id: id,
                    spectator: player.spectator,
                    snapshot: snapshot_duration,
                    compact: compact_duration,
                    entities: entity_count,
                    resource_deltas: resource_delta_count,
                    events: event_count,
                });
            }
            let enqueue_status = send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::Snapshot(snapshot),
            );
            if matches!(enqueue_status, Some(SnapshotSendStatus::Replaced)) {
                if let Some(player) = self.players.get_mut(&id) {
                    player.head_of_line_count = player.head_of_line_count.saturating_add(1);
                }
            }
            if let (Some(perf), Some(status)) = (perf.as_mut(), enqueue_status) {
                perf.record_enqueue(snapshot_enqueue_status(status));
            }
        }
        if let Some(perf) = perf.as_mut() {
            perf.record_phase("snapshot_fanout", fanout_start.elapsed());
        }

        let outcome_start = StdInstant::now();
        let scenario_keeps_running = matches!(self.mode, RoomMode::DevScenario(_));
        let alive = game.alive_players();
        if !scenario_keeps_running && alive.len() <= 1 {
            if let Some(perf) = perf.as_mut() {
                perf.record_phase("outcome_checks", outcome_start.elapsed());
            }
            self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
            self.phase = Phase::Lobby;
            self.dev_driver = None;
            self.dev_view_player_id = None;
            self.start_dev_session();
            return;
        }

        if let Some(perf) = perf.as_mut() {
            perf.record_phase("outcome_checks", outcome_start.elapsed());
        }
        self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
        self.dev_driver = Some(driver);
        self.phase = Phase::InGame(game);
    }

    pub(super) fn on_set_replay_speed(&mut self, speed: f32) {
        if !matches!(
            self.mode,
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. }) | RoomMode::DevScenario(_)
        ) {
            return;
        }
        // Clamp to sensible range matching the UI buttons (0.5× – 8×).
        let clamped = speed.clamp(0.125, 8.0);
        self.replay_speed = clamped;
    }

    /// Rewind a replay by `ticks_back` ticks. Pass `u32::MAX` to reset to the start.
    /// No-op outside replay rooms or when no game is active.
    fn on_seek_replay(&mut self, ticks_back: u32) {
        if !matches!(
            self.mode,
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. })
        ) {
            return;
        }
        let current_tick = match &self.phase {
            Phase::InGame(game) => game.tick_count(),
            Phase::Lobby => return,
        };
        let target_tick = current_tick.saturating_sub(ticks_back);

        let (mut game, mut driver, view_player_id) = match self.build_dev_session() {
            Ok(session) => session,
            Err(err) => {
                warn!(room = %self.room, error = %err, "replay seek rebuild failed");
                self.send_dev_error(&err);
                return;
            }
        };

        // Fast-forward the fresh game by replaying commands up to `target_tick`.
        // Snapshots and events from these ticks are discarded — the client gets a fresh Start.
        for _ in 0..target_tick {
            match &mut driver {
                DevDriver::Live(scripted) => scripted.enqueue_for_tick(&mut game),
                DevDriver::Replay(replay) => replay.enqueue_for_tick(&mut game),
                DevDriver::Scenario(scenario) => scenario.enqueue_for_tick(&mut game),
            }
            let tick_result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()));
            if let Err(payload) = tick_result {
                let reason = panic_reason(&payload);
                dump_crash_replay(&self.room, &game, &reason);
                self.phase = Phase::Lobby;
                self.dev_driver = None;
                self.dev_view_player_id = None;
                return;
            }
        }

        self.phase = Phase::InGame(Box::new(game));
        self.dev_driver = Some(driver);
        self.dev_view_player_id = Some(view_player_id);
        let recipients = self.order.clone();
        for player_id in recipients {
            self.send_dev_start_to(player_id);
        }
        info!(
            room = %self.room,
            from_tick = current_tick,
            to_tick = target_tick,
            "replay seek"
        );
    }

    /// Send a one-time score screen to connected players who have been eliminated while a
    /// multi-player match continues.
    fn send_new_defeats(&mut self, game: &Game, alive: &[u32]) {
        let alive: HashSet<u32> = alive.iter().copied().collect();
        let recipients: Vec<u32> = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                self.players.get(id).map(|p| !p.spectator).unwrap_or(false)
                    && !alive.contains(id)
                    && !self.outcome_sent.contains(id)
            })
            .collect();
        if recipients.is_empty() {
            return;
        }
        let scores = game.scores();
        for id in recipients {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::GameOver {
                    winner_id: None,
                    you: "lost".to_string(),
                    scores: scores.clone(),
                },
            );
            self.outcome_sent.insert(id);
        }
    }

    /// Resolve a finished match: tell everyone who won and return to the lobby for a rematch.
    fn end_match(&mut self, winner_id: Option<u32>, scores: Vec<PlayerScore>) {
        info!(room = %self.room, ?winner_id, "match over");

        // Persist match history only for real public matches. Human-vs-AI, AI-only,
        // 1-player sandboxes, dev/scenario/replay rooms, and automated test rooms are excluded.
        if let (Some(db), Some(started_at)) = (self.db.clone(), self.match_started_at) {
            if self.should_record_match_history() {
                let ended_at = chrono::Utc::now();
                let duration_ms = ended_at
                    .signed_duration_since(started_at)
                    .num_milliseconds()
                    .clamp(0, i32::MAX as i64) as i32;
                let winner_name = winner_id
                    .and_then(|wid| scores.iter().find(|s| s.id == wid).map(|s| s.name.clone()));
                let score_json = serde_json::to_value(&scores).unwrap_or(serde_json::Value::Null);
                let rec = crate::db::MatchRecord {
                    started_at,
                    ended_at,
                    duration_ms,
                    map_name: self.match_map_name.clone(),
                    winner_name,
                    participants: self.match_participants.clone(),
                    score_screen: score_json,
                    local_only: self.match_history_local_only,
                };
                // Detached: a slow Supabase write must never stall the room transitioning back to
                // lobby. Errors are logged inside `record_match`.
                tokio::spawn(async move { db.record_match(rec).await });
            }
        }
        self.match_started_at = None;
        self.match_map_name.clear();
        self.match_participants.clear();

        let recipients: Vec<u32> = self.order.clone();
        for id in &recipients {
            if self.outcome_sent.contains(id) {
                continue;
            }
            let Some(player) = self.players.get(id) else {
                continue;
            };
            let you = if player.spectator {
                "draw"
            } else {
                match winner_id {
                    Some(w) if w == *id => "won",
                    Some(_) => "lost",
                    None => "draw",
                }
            }
            .to_string();
            send_or_log(
                &self.room,
                *id,
                &player.msg_tx,
                ServerMessage::GameOver {
                    winner_id,
                    you,
                    scores: scores.clone(),
                },
            );
            self.outcome_sent.insert(*id);
        }

        // Reset for the next match: drop the game, clear ready flags, and re-advertise the lobby.
        self.phase = Phase::Lobby;
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.outcome_sent.clear();
        for player in self.players.values_mut() {
            player.ready = false;
        }
        self.broadcast_lobby();
    }

    // -- Sending helpers -----------------------------------------------------

    fn finish_perf_tick(
        &self,
        perf: Option<&crate::perf::TickPerf>,
        game: &Game,
        scheduler_lag: Duration,
        tick_start: StdInstant,
    ) {
        let Some(perf) = perf else {
            return;
        };
        perf.finish(crate::perf::TickContext {
            room: &self.room,
            tick: game.current_tick(),
            scheduler_lag,
            total: tick_start.elapsed(),
            players: self.players.values().filter(|p| !p.spectator).count(),
            spectators: self.players.values().filter(|p| p.spectator).count(),
            ai_players: self.ai_players.len(),
            counts: game.perf_entity_counts(),
        });
    }

    /// Send one message to every connected player. Closed sinks are logged and skipped; the
    /// owning connection task is responsible for emitting the eventual `Leave`.
    fn broadcast(&self, msg: &ServerMessage) {
        for &id in &self.order {
            if let Some(player) = self.players.get(&id) {
                send_or_log(&self.room, id, &player.msg_tx, msg.clone());
            }
        }
    }

    fn reset_match_net_status(&mut self) {
        self.slow_tick_count = 0;
        for player in self.players.values_mut() {
            player.head_of_line_count = 0;
        }
    }

    fn snapshot_net_status(
        &self,
        player: &RoomPlayer,
        scheduler_lag: Duration,
        tick_elapsed: Duration,
        slow_tick: bool,
    ) -> SnapshotNetStatus {
        let head_of_line = player.msg_tx.has_pending_snapshot();
        SnapshotNetStatus {
            server_lag_ms: saturating_duration_ms_u16(scheduler_lag),
            tick_ms: saturating_duration_ms_u16(tick_elapsed),
            slow_tick,
            slow_tick_count: self.slow_tick_count,
            head_of_line,
            head_of_line_count: player
                .head_of_line_count
                .saturating_add(u32::from(head_of_line)),
        }
    }
}

fn test_tick_interval_override() -> Option<Duration> {
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        std::env::var("RTS_TEST_TICK_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|&millis| millis > 0)
            .map(Duration::from_millis)
    }
}

fn snapshot_enqueue_status(status: SnapshotSendStatus) -> crate::perf::SnapshotEnqueue {
    match status {
        SnapshotSendStatus::Stored => crate::perf::SnapshotEnqueue::Stored,
        SnapshotSendStatus::Replaced => crate::perf::SnapshotEnqueue::Replaced,
        SnapshotSendStatus::Closed => crate::perf::SnapshotEnqueue::Closed,
    }
}

fn saturating_duration_ms_u16(duration: Duration) -> u16 {
    duration.as_millis().min(u16::MAX as u128) as u16
}
