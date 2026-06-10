use super::connection::send_or_log;
use super::connection::SnapshotSendStatus;
use super::crash_replay::{dump_crash_replay, panic_reason};
use super::dev_replay::{load_replay_artifact, match_seed};
use super::snapshots::{compact_snapshot_for_wire, union_events};
use super::*;
use crate::game::entity::EntityKind;
use crate::game::map::Map;
use crate::game::replay::ReplayArtifactV1;
use crate::protocol::{ReplayPlaybackState, SnapshotNetStatus};
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

fn server_build_sha() -> &'static str {
    env!("COMMIT_HASH")
}

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

pub(super) fn validate_replay_vision_request(
    vision: &ReplayVisionRequest,
    valid_player_ids: &[u32],
) -> Result<(), &'static str> {
    let valid: HashSet<u32> = valid_player_ids.iter().copied().collect();
    match vision {
        ReplayVisionRequest::All => {
            if valid.is_empty() {
                Err("no replay players")
            } else {
                Ok(())
            }
        }
        ReplayVisionRequest::Player { player_id } => {
            if valid.contains(player_id) {
                Ok(())
            } else {
                Err("unknown replay player")
            }
        }
        ReplayVisionRequest::Players { player_ids } => {
            if player_ids.is_empty() {
                return Err("empty replay player subset");
            }
            let mut seen = HashSet::new();
            for player_id in player_ids {
                if !valid.contains(player_id) {
                    return Err("unknown replay player");
                }
                if !seen.insert(*player_id) {
                    return Err("duplicate replay player");
                }
            }
            Ok(())
        }
    }
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
    ReplayViewer(Box<ReplaySession>),
}

#[derive(Clone)]
pub(super) enum RoomMode {
    Normal,
    DevSelfPlay(DevSelfPlayConfig),
    DevScenario(DevScenarioConfig),
    Replay { artifact: ReplayArtifactV1 },
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
    VehicleCornerWall,
    VehicleSmallBlockBaseline,
}

enum DevDriver {
    Live(LiveSelfPlay),
    Scenario(DevScenarioDriver),
}

struct DevScenarioDriver {
    player_id: u32,
    units: Vec<u32>,
    goal: (f32, f32),
    issued: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ReplayVisionSelection {
    All,
    Players(Vec<u32>),
}

impl ReplayVisionSelection {
    fn from_request(request: ReplayVisionRequest) -> Self {
        match request {
            ReplayVisionRequest::All => ReplayVisionSelection::All,
            ReplayVisionRequest::Player { player_id } => {
                ReplayVisionSelection::Players(vec![player_id])
            }
            ReplayVisionRequest::Players { player_ids } => {
                ReplayVisionSelection::Players(player_ids)
            }
        }
    }

    fn player_ids(&self, all_players: &[u32]) -> Vec<u32> {
        match self {
            ReplayVisionSelection::All => all_players.to_vec(),
            ReplayVisionSelection::Players(ids) => ids.clone(),
        }
    }
}

/// Reusable server-side replay runtime. It owns the artifact and a rebuilt simulation, and the
/// room task drives it exactly like a live game.
struct ReplaySession {
    artifact: ReplayArtifactV1,
    game: Box<Game>,
    next_command: usize,
    duration_ticks: u32,
    speed: f32,
    viewer_vision: HashMap<u32, ReplayVisionSelection>,
    last_controller_id: Option<u32>,
    last_seek_at: Option<StdInstant>,
}

impl ReplaySession {
    #[allow(dead_code)]
    const DEFAULT_SPEED: f32 = 2.0;
    const MIN_SPEED: f32 = 0.125;
    const MAX_SPEED: f32 = 8.0;
    const MAX_DURATION_TICKS: u32 = 30 * 60 * 60;
    const MAX_COMMAND_LOG_ENTRIES: usize = 200_000;
    const SEEK_COOLDOWN: Duration = Duration::from_millis(500);

    #[allow(dead_code)]
    fn new(artifact: ReplayArtifactV1) -> Result<Self, String> {
        Self::validate_artifact_limits(&artifact)?;
        let duration_ticks = artifact.duration_ticks;
        let build_start = StdInstant::now();
        let game = Box::new(Self::build_game(&artifact)?);
        info!(
            map = %artifact.map_name,
            duration_ticks,
            command_count = artifact.command_log.len(),
            player_count = artifact.players.len(),
            build_ms = build_start.elapsed().as_millis(),
            "replay session built"
        );
        Ok(ReplaySession {
            artifact,
            game,
            next_command: 0,
            duration_ticks,
            speed: Self::DEFAULT_SPEED,
            viewer_vision: HashMap::new(),
            last_controller_id: None,
            last_seek_at: None,
        })
    }

    fn validate_artifact_limits(artifact: &ReplayArtifactV1) -> Result<(), String> {
        if artifact.players.is_empty() {
            return Err("replay artifact has no players".to_string());
        }
        if artifact.players.len() > MAX_PLAYERS {
            return Err(format!(
                "replay artifact has {} players; maximum is {MAX_PLAYERS}",
                artifact.players.len()
            ));
        }
        let mut seen_players = HashSet::new();
        for player in &artifact.players {
            if !seen_players.insert(player.id) {
                return Err(format!(
                    "replay artifact has duplicate player id {}",
                    player.id
                ));
            }
        }
        if artifact.duration_ticks > Self::MAX_DURATION_TICKS {
            return Err(format!(
                "replay duration {} exceeds maximum {}",
                artifact.duration_ticks,
                Self::MAX_DURATION_TICKS
            ));
        }
        if artifact.command_log.len() > Self::MAX_COMMAND_LOG_ENTRIES {
            return Err(format!(
                "replay command log has {} entries; maximum is {}",
                artifact.command_log.len(),
                Self::MAX_COMMAND_LOG_ENTRIES
            ));
        }
        let mut previous_tick = 0;
        for (index, entry) in artifact.command_log.iter().enumerate() {
            if !seen_players.contains(&entry.player_id) {
                return Err(format!(
                    "replay command {index} references unknown player {}",
                    entry.player_id
                ));
            }
            if entry.tick == 0 {
                return Err(format!("replay command {index} has invalid tick 0"));
            }
            if entry.tick > artifact.duration_ticks {
                return Err(format!(
                    "replay command {index} tick {} exceeds duration {}",
                    entry.tick, artifact.duration_ticks
                ));
            }
            if entry.tick < previous_tick {
                return Err(format!(
                    "replay command {index} is out of order: tick {} before {}",
                    entry.tick, previous_tick
                ));
            }
            previous_tick = entry.tick;
        }
        Ok(())
    }

    fn build_game(artifact: &ReplayArtifactV1) -> Result<Game, String> {
        let metadata = Map::metadata_for_name(&artifact.map_name)
            .map_err(|err| format!("cannot load replay map metadata: {err}"))?;
        artifact
            .validate_against(server_build_sha(), &metadata)
            .map_err(|err| err.to_string())?;
        let map = Map::load(&artifact.map_name, artifact.players.len(), artifact.seed)
            .map_err(|err| format!("cannot load replay map: {err}"))?;
        Ok(Game::new_for_replay_with_map_metadata(
            &artifact.players,
            artifact.starting_steel,
            artifact.starting_oil,
            artifact.seed,
            artifact.starting_loadout_mode.clone(),
            map,
            metadata,
        ))
    }

    fn active_player_ids(&self) -> Vec<u32> {
        self.artifact.players.iter().map(|p| p.id).collect()
    }

    fn start_payload_for(&self, viewer_id: u32) -> StartPayload {
        StartPayload {
            player_id: viewer_id,
            spectator: true,
            replay: Some(self.artifact.start_metadata()),
            ..self.game.start_payload()
        }
    }

    fn state(&self) -> ReplayPlaybackState {
        ReplayPlaybackState {
            current_tick: self.current_tick(),
            duration_ticks: self.duration_ticks,
            speed: self.speed,
            paused: self.speed == 0.0,
            ended: self.current_tick() >= self.duration_ticks,
            controller_id: self.last_controller_id,
        }
    }

    fn current_tick(&self) -> u32 {
        self.game.tick_count()
    }

    fn set_speed(&mut self, controller_id: u32, speed: f32) {
        self.speed = speed.clamp(Self::MIN_SPEED, Self::MAX_SPEED);
        self.last_controller_id = Some(controller_id);
    }

    fn set_vision(&mut self, viewer_id: u32, vision: ReplayVisionRequest) {
        self.viewer_vision
            .insert(viewer_id, ReplayVisionSelection::from_request(vision));
    }

    fn vision_player_ids_for(&self, viewer_id: u32) -> Vec<u32> {
        let all_players = self.active_player_ids();
        self.viewer_vision
            .get(&viewer_id)
            .unwrap_or(&ReplayVisionSelection::All)
            .player_ids(&all_players)
    }

    fn enqueue_for_current_tick(&mut self) -> Result<(), String> {
        let tick = self.current_tick().saturating_add(1);
        while let Some(entry) = self.artifact.command_log.get(self.next_command) {
            if entry.tick < tick {
                return Err(format!(
                    "replay command {} is out of order: tick {} before {}",
                    self.next_command, entry.tick, tick
                ));
            }
            if entry.tick != tick {
                break;
            }
            self.game.enqueue(
                entry.player_id,
                SimCommand::from_protocol(entry.command.clone()),
            );
            self.next_command += 1;
        }
        Ok(())
    }

    fn tick(&mut self, perf: Option<&mut crate::perf::TickPerf>) -> HashMap<u32, Vec<Event>> {
        self.game.tick_with_perf(perf).into_iter().collect()
    }

    fn seek_back(
        &mut self,
        room: &str,
        viewer_count: usize,
        controller_id: u32,
        ticks_back: u32,
    ) -> Result<u32, String> {
        if self
            .last_seek_at
            .is_some_and(|last_seek| last_seek.elapsed() < Self::SEEK_COOLDOWN)
        {
            return Err("Replay seek ignored; wait before seeking again.".to_string());
        }
        let target_tick = self
            .current_tick()
            .saturating_sub(ticks_back)
            .min(self.duration_ticks);
        let from_tick = self.current_tick();
        let seek_start = StdInstant::now();
        self.rebuild_to(target_tick)?;
        self.last_seek_at = Some(StdInstant::now());
        self.last_controller_id = Some(controller_id);
        info!(
            room = %room,
            controller_id,
            viewer_count,
            from_tick,
            to_tick = target_tick,
            duration_ticks = self.duration_ticks,
            command_count = self.artifact.command_log.len(),
            rebuild_ms = seek_start.elapsed().as_millis(),
            "replay seek rebuilt"
        );
        Ok(target_tick)
    }

    fn rebuild_to(&mut self, target_tick: u32) -> Result<(), String> {
        *self.game = Self::build_game(&self.artifact)?;
        self.next_command = 0;
        for _ in 0..target_tick {
            self.enqueue_for_current_tick()?;
            self.game.tick();
        }
        Ok(())
    }
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
    /// Dev-watch pause flag. Kept separate from replay_speed so interval creation never divides
    /// by zero and resume can restore the previous non-zero multiplier.
    dev_watch_paused: bool,
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
    drain: DrainHandle,
    match_tracked_for_drain: bool,
}

impl RoomTask {
    pub(super) fn new(
        room: String,
        mode: RoomMode,
        db: Option<Arc<Db>>,
        match_history_local_only: bool,
        drain: DrainHandle,
    ) -> Self {
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
            replay_speed: 1.0,
            dev_watch_paused: false,
            slow_tick_count: 0,
            db,
            match_history_local_only,
            match_started_at: None,
            match_map_name: String::new(),
            match_participants: Vec::new(),
            drain,
            match_tracked_for_drain: false,
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
                            let old_speed = self.current_speed_multiplier();
                            self.handle_event(event);
                            speed_changed = self.current_speed_multiplier() != old_speed;
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
        base.div_f32(self.current_speed_multiplier())
    }

    fn current_speed_multiplier(&self) -> f32 {
        if self.dev_watch_paused {
            return 1.0;
        }
        match &self.phase {
            Phase::ReplayViewer(session) => session.speed,
            _ => self.replay_speed,
        }
    }

    fn is_live_dev_watch(&self) -> bool {
        matches!(
            self.mode,
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Live) | RoomMode::DevScenario(_)
        )
    }

    fn should_record_match_history(&self) -> bool {
        self.match_human_count >= 2
            && !self.is_live_dev_watch()
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
            RoomEvent::ReturnToLobby { player_id } => self.on_return_to_lobby(player_id),
            RoomEvent::SetReplaySpeed { player_id, speed } => {
                self.on_set_replay_speed(player_id, speed)
            }
            RoomEvent::StepDevTick { player_id } => self.on_step_dev_tick(player_id),
            RoomEvent::SeekReplay {
                player_id,
                ticks_back,
            } => self.on_seek_replay(player_id, ticks_back),
            RoomEvent::SetReplayVision { player_id, vision } => {
                self.on_set_replay_vision(player_id, vision)
            }
            RoomEvent::SelectMap { player_id, map } => self.on_select_map(player_id, map),
            RoomEvent::DrainStarted(notice) => self.on_drain_started(notice),
        }
    }

    fn on_drain_started(&mut self, notice: DrainNotice) {
        self.broadcast_shutdown_warning(notice);
        if matches!(self.phase, Phase::Lobby) {
            self.broadcast_lobby();
        }
    }

    fn send_current_shutdown_warning_to(&self, player_id: u32) {
        let Some(notice) = self.drain.notice() else {
            return;
        };
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::ShutdownWarning {
                deadline_unix_ms: notice.deadline_unix_ms,
                seconds_remaining: notice.seconds_remaining,
            },
        );
    }

    fn broadcast_shutdown_warning(&self, notice: DrainNotice) {
        let msg = ServerMessage::ShutdownWarning {
            deadline_unix_ms: notice.deadline_unix_ms,
            seconds_remaining: notice.seconds_remaining,
        };
        self.broadcast(&msg);
    }

    pub(super) fn on_join(
        &mut self,
        player_id: u32,
        name: String,
        spectator: bool,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.is_live_dev_watch() {
            self.on_join_dev_selfplay(player_id, name, msg_tx, ack);
            return;
        }
        if matches!(
            self.mode,
            RoomMode::Replay { .. } | RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. })
        ) {
            self.on_join_replay_room(player_id, name, msg_tx, ack);
            return;
        }
        if matches!(self.phase, Phase::ReplayViewer(_)) {
            self.on_join_replay_viewer(player_id, name, msg_tx, ack);
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
        self.send_current_shutdown_warning_to(player_id);
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
            self.mark_match_finished_for_drain();
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
            Phase::ReplayViewer(session) => {
                session.viewer_vision.remove(&player_id);
            }
        }
    }

    pub(super) fn on_ready(&mut self, player_id: u32, ready: bool) {
        if self.is_live_dev_watch() {
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

    pub(super) fn on_start_request(&mut self, player_id: u32) {
        if self.is_live_dev_watch() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) {
            return;
        }
        if self.drain.is_draining() {
            if let Some(player) = self.players.get(&player_id) {
                send_or_log(
                    &self.room,
                    player_id,
                    &player.msg_tx,
                    ServerMessage::Error {
                        msg: "Server is draining for deploy; new matches are disabled.".to_string(),
                    },
                );
            }
            debug!(room = %self.room, player_id, "ignoring start while server is draining");
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
        if self.is_live_dev_watch() {
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
        if self.is_live_dev_watch() {
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
        if self.is_live_dev_watch() {
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
        if self.is_live_dev_watch() {
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
        if self.is_live_dev_watch() {
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
        if self.is_live_dev_watch() {
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
        if self.is_live_dev_watch() {
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
            Phase::ReplayViewer(session) => {
                self.phase = Phase::ReplayViewer(session);
                return;
            }
        };

        debug!(room = %self.room, player_id, "player gave up");
        game.eliminate(player_id);
        let alive = game.alive_players();
        let scores = game.scores();

        if self.match_player_count >= 2 && alive.len() <= 1 {
            self.end_match(alive.first().copied(), scores, Some(&game));
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
            self.mark_match_finished_for_drain();
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

    fn on_join_replay_viewer(
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
        self.send_replay_start_to(player_id);
        self.send_replay_state_to(player_id);
    }

    fn on_join_replay_room(
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

        match &self.phase {
            Phase::ReplayViewer(_) => {
                self.send_replay_start_to(player_id);
                self.send_replay_state_to(player_id);
            }
            Phase::Lobby => match self.replay_session_for_mode() {
                Ok(session) => self.transition_to_replay_viewer(session),
                Err(err) => {
                    warn!(room = %self.room, error = %err, "replay setup failed");
                    if let Some(player) = self.players.get(&player_id) {
                        send_or_log(
                            &self.room,
                            player_id,
                            &player.msg_tx,
                            ServerMessage::Error { msg: err },
                        );
                    }
                }
            },
            Phase::InGame(_) => {}
        }
    }

    fn replay_session_for_mode(&self) -> Result<ReplaySession, String> {
        let artifact = match &self.mode {
            RoomMode::Replay { artifact } => artifact.clone(),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { artifact }) => {
                load_replay_artifact(artifact)?
            }
            _ => return Err("room is not configured for replay playback".to_string()),
        };
        ReplaySession::new(artifact)
    }

    /// A match may start with at least one active participant and every active human ready.
    /// Spectators can host and watch from the lobby, but they do not block readiness.
    fn can_start(&self) -> bool {
        !self.drain.is_draining()
            && self.total_player_count() > 0
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
        let map_metadata = match Map::metadata_for_name(&self.selected_map) {
            Ok(metadata) => metadata,
            Err(err) => {
                let msg = format!("Cannot load map \"{}\": {err}", self.selected_map);
                warn!(room = %self.room, error = %err, "map metadata load failed at start");
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
            Game::new_with_debug_starting_loadout_and_random_ai_profiles_and_map_metadata(
                &inits,
                starting_steel,
                starting_oil,
                seed,
                map,
                map_metadata,
            )
        } else {
            Game::new_with_random_ai_profiles_and_map_metadata(&inits, seed, map, map_metadata)
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
        self.mark_match_started_for_drain();
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
        self.mark_match_started_for_drain();
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
            RoomMode::Replay { .. } => Err("room is not configured for a dev session".to_string()),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. }) => {
                Err("saved self-play replays use the replay viewer".to_string())
            }
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
                DevScenarioId::VehicleCornerWall => {
                    let setup = Game::new_vehicle_corner_wall_scenario(
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

    fn send_replay_start_to(&self, watcher_id: u32) {
        let Phase::ReplayViewer(session) = &self.phase else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        send_or_log(
            &self.room,
            watcher_id,
            &player.msg_tx,
            ServerMessage::Start(session.start_payload_for(watcher_id)),
        );
    }

    fn send_replay_state_to(&self, watcher_id: u32) {
        let Phase::ReplayViewer(session) = &self.phase else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        send_or_log(
            &self.room,
            watcher_id,
            &player.msg_tx,
            ServerMessage::ReplayState(session.state()),
        );
    }

    fn broadcast_replay_state_for(&self, session: &ReplaySession) {
        let msg = ServerMessage::ReplayState(session.state());
        self.broadcast(&msg);
    }

    fn fanout_replay_snapshots(
        &mut self,
        session: &ReplaySession,
        per_player_events: HashMap<u32, Vec<Event>>,
        scheduler_lag: Duration,
        tick_start: StdInstant,
        mut perf: Option<&mut crate::perf::TickPerf>,
    ) {
        let tick_budget = self.current_tick_interval();
        let mut slow_tick_counted = false;
        let fanout_start = StdInstant::now();
        let recipients = self.order.clone();
        for id in recipients {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            let visible_players = session.vision_player_ids_for(id);
            let snapshot_start = StdInstant::now();
            let mut snapshot = session.game.snapshot_for_spectator(&visible_players);
            snapshot.events.extend(union_events(
                visible_players
                    .iter()
                    .filter_map(|player_id| per_player_events.get(player_id)),
            ));
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
                    spectator: true,
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
        if self.is_live_dev_watch() {
            if self.dev_watch_paused {
                return;
            }
            self.on_tick_dev_selfplay(scheduled);
            return;
        }
        if matches!(self.phase, Phase::ReplayViewer(_)) {
            self.on_tick_replay_viewer(scheduled);
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
            Phase::ReplayViewer(session) => {
                self.phase = Phase::ReplayViewer(session);
                return;
            }
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
                self.end_match(None, game.scores(), None);
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
            self.end_match(alive.first().copied(), game.scores(), Some(&game));
            // end_match drops the live game and moves to replay or lobby; do not restore it.
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
            Phase::ReplayViewer(session) => {
                self.phase = Phase::ReplayViewer(session);
                return;
            }
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

    fn on_tick_replay_viewer(&mut self, scheduled: TokioInstant) {
        let mut session = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::ReplayViewer(session) => session,
            other => {
                self.phase = other;
                return;
            }
        };
        let scheduler_lag = scheduled.elapsed();
        let tick_start = StdInstant::now();
        let mut perf = crate::perf::TickPerf::maybe_new();

        if session.current_tick() < session.duration_ticks {
            if let Err(err) = session.enqueue_for_current_tick() {
                warn!(room = %self.room, error = %err, "replay command enqueue failed");
                self.send_dev_error(&err);
                self.phase = Phase::ReplayViewer(session);
                return;
            }
            let game_tick_start = StdInstant::now();
            let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                session.tick(perf.as_mut())
            }));
            if let Some(perf) = perf.as_mut() {
                perf.record_phase("game_tick", game_tick_start.elapsed());
            }
            let per_player_events = match tick_result {
                Ok(events) => events,
                Err(payload) => {
                    let reason = panic_reason(&payload);
                    dump_crash_replay(&self.room, &session.game, &reason);
                    self.send_dev_error("Replay playback failed");
                    self.phase = Phase::Lobby;
                    return;
                }
            };
            self.fanout_replay_snapshots(
                &session,
                per_player_events,
                scheduler_lag,
                tick_start,
                perf.as_mut(),
            );
        } else {
            self.broadcast_replay_state_for(&session);
        }

        self.finish_perf_tick(perf.as_ref(), &session.game, scheduler_lag, tick_start);
        self.phase = Phase::ReplayViewer(session);
    }

    pub(super) fn on_set_replay_speed(&mut self, player_id: u32, speed: f32) {
        if let Phase::ReplayViewer(session) = &mut self.phase {
            if !self.players.contains_key(&player_id) {
                return;
            }
            session.set_speed(player_id, speed);
            let state = session.state();
            self.broadcast(&ServerMessage::ReplayState(state));
            return;
        }
        if !matches!(self.mode, RoomMode::DevScenario(_)) {
            return;
        }
        if speed == 0.0 {
            self.dev_watch_paused = true;
            return;
        }
        self.dev_watch_paused = false;
        // Clamp to sensible range matching the UI buttons (0.125× – 8×).
        let clamped = speed.clamp(0.125, 8.0);
        self.replay_speed = clamped;
    }

    fn on_step_dev_tick(&mut self, player_id: u32) {
        if !self.players.contains_key(&player_id)
            || !self.dev_watch_paused
            || !matches!(self.mode, RoomMode::DevScenario(_))
        {
            return;
        }
        self.on_tick_dev_selfplay(TokioInstant::now());
    }

    fn on_set_replay_vision(&mut self, player_id: u32, vision: ReplayVisionRequest) {
        if let Phase::ReplayViewer(session) = &mut self.phase {
            if !self.players.contains_key(&player_id) {
                return;
            }
            let valid_ids = session.active_player_ids();
            if validate_replay_vision_request(&vision, &valid_ids).is_err() {
                if let Some(player) = self.players.get(&player_id) {
                    send_or_log(
                        &self.room,
                        player_id,
                        &player.msg_tx,
                        ServerMessage::Error {
                            msg: "Invalid replay vision selection".to_string(),
                        },
                    );
                }
                return;
            }
            session.set_vision(player_id, vision);
        }
    }

    /// Rewind a replay by `ticks_back` ticks. Pass `u32::MAX` to reset to the start.
    /// No-op outside replay rooms or when no game is active.
    fn on_seek_replay(&mut self, player_id: u32, ticks_back: u32) {
        if let Phase::ReplayViewer(session) = &mut self.phase {
            if !self.players.contains_key(&player_id) {
                return;
            }
            let viewer_count = self.players.len();
            let seek_result = session.seek_back(&self.room, viewer_count, player_id, ticks_back);
            let starts = if seek_result.is_ok() {
                self.order
                    .iter()
                    .filter_map(|viewer_id| {
                        self.players.get(viewer_id).map(|player| {
                            (
                                *viewer_id,
                                player.msg_tx.clone(),
                                session.start_payload_for(*viewer_id),
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let state = seek_result.as_ref().ok().map(|_| session.state());
            match seek_result {
                Ok(_) => {
                    for (viewer_id, msg_tx, start) in starts {
                        send_or_log(&self.room, viewer_id, &msg_tx, ServerMessage::Start(start));
                    }
                    if let Some(state) = state {
                        self.broadcast(&ServerMessage::ReplayState(state));
                    }
                }
                Err(err) => {
                    warn!(room = %self.room, error = %err, "replay seek failed");
                    self.send_dev_error(&err);
                }
            }
        }
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

    /// Resolve a finished match: tell everyone who won and start post-match replay playback.
    fn end_match(&mut self, winner_id: Option<u32>, scores: Vec<PlayerScore>, game: Option<&Game>) {
        info!(room = %self.room, ?winner_id, "match over");
        let replay_artifact = game.filter(|_| !self.is_live_dev_watch()).map(|game| {
            ReplayArtifactV1::capture_from_game(game, server_build_sha(), winner_id, scores.clone())
        });

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
                let replay = replay_artifact
                    .as_ref()
                    .and_then(|artifact| match crate::db::MatchReplayRecord::from_artifact(artifact)
                    {
                        Ok(replay) => Some(replay),
                        Err(err) => {
                            warn!(room = %self.room, error = %err, "failed to serialize replay artifact for match history");
                            None
                        }
                    });
                let rec = crate::db::MatchRecord {
                    started_at,
                    ended_at,
                    duration_ms,
                    map_name: self.match_map_name.clone(),
                    winner_name,
                    participants: self.match_participants.clone(),
                    score_screen: score_json,
                    local_only: self.match_history_local_only,
                    replay,
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

        self.mark_match_finished_for_drain();
        if let Some(artifact) = replay_artifact {
            match ReplaySession::new(artifact) {
                Ok(session) => {
                    self.transition_to_replay_viewer(session);
                    return;
                }
                Err(err) => {
                    warn!(room = %self.room, error = %err, "post-match replay setup failed");
                    self.broadcast(&ServerMessage::Error {
                        msg: "Post-match replay could not be started.".to_string(),
                    });
                }
            }
        }
        self.return_to_lobby();
    }

    fn transition_to_replay_viewer(&mut self, session: ReplaySession) {
        self.phase = Phase::ReplayViewer(Box::new(session));
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.outcome_sent.clear();
        for player in self.players.values_mut() {
            player.ready = false;
            player.msg_tx.clear_pending_snapshot();
        }
        let recipients = self.order.clone();
        for id in recipients {
            self.send_replay_start_to(id);
            self.send_replay_state_to(id);
        }
        info!(
            room = %self.room,
            viewer_count = self.players.len(),
            "replay viewer active"
        );
    }

    fn on_return_to_lobby(&mut self, player_id: u32) {
        if !self.players.contains_key(&player_id) || !matches!(self.phase, Phase::ReplayViewer(_)) {
            return;
        }
        if matches!(
            self.mode,
            RoomMode::Replay { .. } | RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. })
        ) {
            return;
        }
        self.return_to_lobby();
    }

    fn return_to_lobby(&mut self) {
        // Reset for the next match: drop the game/replay, clear ready flags, and re-advertise
        // the lobby. AI slots, map selection, and quickstart persist for rematches.
        self.phase = Phase::Lobby;
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.outcome_sent.clear();
        for player in self.players.values_mut() {
            player.ready = false;
            player.msg_tx.clear_pending_snapshot();
        }
        self.broadcast_lobby();
    }

    fn mark_match_started_for_drain(&mut self) {
        if !self.match_tracked_for_drain && !self.is_live_dev_watch() {
            self.match_tracked_for_drain = true;
            self.drain.match_started();
        }
    }

    fn mark_match_finished_for_drain(&mut self) {
        if self.match_tracked_for_drain {
            self.match_tracked_for_drain = false;
            self.drain.match_finished();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn replay_test_players(count: usize) -> Vec<PlayerInit> {
        (1..=count as u32)
            .map(|id| PlayerInit {
                id,
                name: format!("Player {id}"),
                color: PLAYER_PALETTE[(id as usize - 1) % PLAYER_PALETTE.len()].to_string(),
                is_ai: false,
            })
            .collect()
    }

    fn replay_test_game(players: &[PlayerInit], seed: u32) -> Game {
        let metadata = Map::metadata_for_name("Default").unwrap();
        let map = Map::load("Default", players.len(), seed).unwrap();
        Game::new_with_random_ai_profiles_and_map_metadata(players, seed, map, metadata)
    }

    fn replay_test_artifact(players: &[PlayerInit], ticks: u32) -> (Game, ReplayArtifactV1) {
        let seed = 0x5150_2202;
        let mut game = replay_test_game(players, seed);
        for _ in 0..ticks {
            game.tick();
        }
        let artifact =
            ReplayArtifactV1::capture_from_game(&game, server_build_sha(), None, game.scores());
        (game, artifact)
    }

    fn add_test_room_player(task: &mut RoomTask, id: u32, ready: bool) -> ConnectionWriter {
        let (msg_tx, writer) = ConnectionSink::new();
        task.order.push(id);
        task.players.insert(
            id,
            RoomPlayer {
                name: format!("Player {id}"),
                color: PLAYER_PALETTE[(id as usize - 1) % PLAYER_PALETTE.len()].to_string(),
                ready,
                spectator: false,
                msg_tx,
                head_of_line_count: 0,
            },
        );
        writer
    }

    fn replay_transition_test_snapshot(tick: u32) -> Snapshot {
        Snapshot {
            tick,
            steel: 75,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
            entities: Vec::new(),
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            visible_tiles: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: Vec::new(),
            net_status: SnapshotNetStatus::default(),
        }
    }

    fn in_game_tick(task: &RoomTask) -> u32 {
        match &task.phase {
            Phase::InGame(game) => game.tick_count(),
            Phase::ReplayViewer(session) => session.current_tick(),
            Phase::Lobby => 0,
        }
    }

    #[test]
    fn replay_vision_validation_rejects_unknown_and_empty_subsets() {
        let valid = [1, 2, 3];

        assert!(validate_replay_vision_request(&ReplayVisionRequest::All, &valid).is_ok());
        assert!(validate_replay_vision_request(
            &ReplayVisionRequest::Player { player_id: 2 },
            &valid,
        )
        .is_ok());
        assert!(validate_replay_vision_request(
            &ReplayVisionRequest::Players {
                player_ids: vec![1, 3],
            },
            &valid,
        )
        .is_ok());

        assert!(validate_replay_vision_request(
            &ReplayVisionRequest::Player { player_id: 99 },
            &valid,
        )
        .is_err());
        assert!(validate_replay_vision_request(
            &ReplayVisionRequest::Players { player_ids: vec![] },
            &valid,
        )
        .is_err());
        assert!(validate_replay_vision_request(
            &ReplayVisionRequest::Players {
                player_ids: vec![1, 99],
            },
            &valid,
        )
        .is_err());
        assert!(validate_replay_vision_request(
            &ReplayVisionRequest::Players {
                player_ids: vec![1, 1],
            },
            &valid,
        )
        .is_err());
    }

    #[test]
    fn replay_session_reaches_live_final_snapshots() {
        let players = replay_test_players(2);
        let (live, artifact) = replay_test_artifact(&players, 5);
        let mut replay = ReplaySession::new(artifact).unwrap();

        while replay.current_tick() < replay.duration_ticks {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }

        for player in &players {
            assert_eq!(
                replay.game.snapshot_for(player.id),
                live.snapshot_for(player.id)
            );
        }
    }

    #[test]
    fn replay_viewer_snapshot_hides_resource_outside_union_fog() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let replay = ReplaySession::new(artifact).unwrap();

        let full = replay.game.snapshot_full_for(players[0].id);
        let union = replay
            .game
            .snapshot_for_spectator(&replay.active_player_ids());

        assert!(
            full.resource_deltas.len() > union.resource_deltas.len(),
            "default replay spectator fog should not expose every resource node"
        );
    }

    #[test]
    fn single_player_replay_fog_matches_player_visibility() {
        let players = replay_test_players(1);
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let replay = ReplaySession::new(artifact).unwrap();

        let player = replay.game.snapshot_for(players[0].id);
        let replay_view = replay.game.snapshot_for_spectator(&[players[0].id]);

        assert_eq!(replay_view.visible_tiles, player.visible_tiles);
        assert_eq!(
            replay_view
                .entities
                .iter()
                .map(|entity| entity.id)
                .collect::<Vec<_>>(),
            player
                .entities
                .iter()
                .map(|entity| entity.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn replay_vision_selection_is_per_viewer() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let mut replay = ReplaySession::new(artifact).unwrap();

        replay.set_vision(
            100,
            ReplayVisionRequest::Player {
                player_id: players[0].id,
            },
        );
        replay.set_vision(
            101,
            ReplayVisionRequest::Player {
                player_id: players[1].id,
            },
        );

        assert_eq!(replay.vision_player_ids_for(100), vec![players[0].id]);
        assert_eq!(replay.vision_player_ids_for(101), vec![players[1].id]);
        assert_eq!(
            replay.vision_player_ids_for(102),
            replay.active_player_ids()
        );
    }

    #[test]
    fn replay_speed_and_seek_are_clamped_in_state() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 4);
        let mut replay = ReplaySession::new(artifact).unwrap();
        for _ in 0..3 {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }

        replay.set_speed(42, 99.0);
        assert_eq!(replay.state().speed, ReplaySession::MAX_SPEED);
        assert_eq!(replay.state().controller_id, Some(42));

        let target = replay.seek_back("test", 1, 42, u32::MAX).unwrap();
        assert_eq!(target, 0);
        assert_eq!(replay.state().current_tick, 0);
    }

    #[test]
    fn replay_seek_frequency_is_bounded() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 4);
        let mut replay = ReplaySession::new(artifact).unwrap();
        for _ in 0..3 {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }

        assert!(replay.seek_back("test", 1, 42, 1).is_ok());
        let err = replay.seek_back("test", 1, 42, 1).unwrap_err();
        assert!(
            err.contains("wait before seeking again"),
            "unexpected seek reject: {err}"
        );
    }

    #[test]
    fn replay_artifact_limits_reject_malformed_command_logs() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 2);
        artifact
            .command_log
            .push(crate::game::replay::CommandLogEntry {
                tick: artifact.duration_ticks + 1,
                player_id: players[0].id,
                command: crate::protocol::Command::Stop { units: vec![1] },
            });

        let err = match ReplaySession::new(artifact) {
            Ok(_) => panic!("malformed replay artifact should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("exceeds duration"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_artifact_limits_reject_duplicate_players() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 0);
        artifact.players.push(artifact.players[0].clone());

        let err = match ReplaySession::new(artifact) {
            Ok(_) => panic!("duplicate-player replay artifact should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("duplicate player id"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_artifact_limits_reject_oversized_duration() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 0);
        artifact.duration_ticks = ReplaySession::MAX_DURATION_TICKS + 1;

        let err = match ReplaySession::new(artifact) {
            Ok(_) => panic!("oversized replay artifact should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("exceeds maximum"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_room_rejects_rapid_seek_without_resetting_viewers() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 4);
        let mut replay = ReplaySession::new(artifact).unwrap();
        for _ in 0..3 {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }
        let mut task = RoomTask::new(
            "replay-seek-rate-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer = add_test_room_player(&mut task, 99, true);
        task.phase = Phase::ReplayViewer(Box::new(replay));

        task.on_seek_replay(99, 1);
        assert!(std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
            .any(|msg| matches!(msg, ServerMessage::ReplayState(_))));

        task.on_seek_replay(99, 1);
        let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(messages.iter().any(|msg| {
            matches!(msg, ServerMessage::Error { msg } if msg.contains("wait before seeking again"))
        }));
        assert!(!messages
            .iter()
            .any(|msg| matches!(msg, ServerMessage::Start(_))));
        assert!(matches!(task.phase, Phase::ReplayViewer(_)));
    }

    #[test]
    fn rapid_replay_vision_changes_remain_per_viewer() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 1);
        let replay = ReplaySession::new(artifact).unwrap();
        let mut task = RoomTask::new(
            "replay-vision-stress-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let writer_a = add_test_room_player(&mut task, 100, true);
        let writer_b = add_test_room_player(&mut task, 101, true);
        task.phase = Phase::ReplayViewer(Box::new(replay));

        for _ in 0..8 {
            task.on_set_replay_vision(
                100,
                ReplayVisionRequest::Player {
                    player_id: players[0].id,
                },
            );
            task.on_set_replay_vision(
                101,
                ReplayVisionRequest::Player {
                    player_id: players[1].id,
                },
            );
        }
        task.on_tick_replay_viewer(TokioInstant::now());

        let snapshot_a = writer_a.snapshots.take().expect("viewer A snapshot");
        let snapshot_b = writer_b.snapshots.take().expect("viewer B snapshot");
        let Phase::ReplayViewer(session) = &task.phase else {
            panic!("replay phase should remain active");
        };
        let expected_a = session.game.snapshot_for_spectator(&[players[0].id]);
        let expected_b = session.game.snapshot_for_spectator(&[players[1].id]);

        assert_eq!(snapshot_a.visible_tiles, expected_a.visible_tiles);
        assert_eq!(snapshot_b.visible_tiles, expected_b.visible_tiles);
        assert_ne!(
            snapshot_a.visible_tiles, snapshot_b.visible_tiles,
            "test setup should exercise different fog perspectives"
        );
    }

    #[test]
    fn replay_phase_ignores_gameplay_commands() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let replay = ReplaySession::new(artifact).unwrap();
        let mut task = RoomTask::new(
            "replay-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        task.phase = Phase::ReplayViewer(Box::new(replay));

        task.on_command(
            players[0].id,
            SimCommand::Stop {
                units: vec![1, 2, 3],
            },
        );

        let Phase::ReplayViewer(replay) = &task.phase else {
            panic!("replay phase should remain active");
        };
        assert!(replay.game.command_log().is_empty());
    }

    #[test]
    fn paused_dev_scenario_steps_one_tick_at_a_time() {
        let mut task = RoomTask::new(
            "dev-scenario-step-test".to_string(),
            RoomMode::DevScenario(DevScenarioConfig {
                id: DevScenarioId::VehicleCornerWall,
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
            }),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, _writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Viewer".to_string(), true, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert_eq!(in_game_tick(&task), 0);

        task.on_set_replay_speed(99, 0.0);
        task.on_tick(TokioInstant::now());
        assert_eq!(
            in_game_tick(&task),
            0,
            "scheduled ticks should not advance while paused"
        );

        task.on_step_dev_tick(99);
        assert_eq!(in_game_tick(&task), 1);
        task.on_step_dev_tick(99);
        assert_eq!(in_game_tick(&task), 2);

        task.on_set_replay_speed(99, 1.0);
        task.on_tick(TokioInstant::now());
        assert_eq!(
            in_game_tick(&task),
            3,
            "scheduled ticks should resume after selecting a non-zero speed"
        );
    }

    #[test]
    fn persisted_replay_room_join_starts_replay_viewer() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 3);
        let mut task = RoomTask::new(
            "persisted-replay-test".to_string(),
            RoomMode::Replay { artifact },
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(99, "Viewer".to_string(), false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert!(matches!(task.phase, Phase::ReplayViewer(_)));
        assert!(task.players.get(&99).is_some_and(|p| p.spectator));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::Start(payload) if payload.spectator && payload.replay.is_some()
        ));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::ReplayState(_)
        ));
    }

    #[test]
    fn end_match_transitions_all_connected_players_to_tick_zero_replay() {
        let players = replay_test_players(2);
        let (game, _artifact) = replay_test_artifact(&players, 3);
        let mut task = RoomTask::new(
            "post-match-replay-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_test_room_player(&mut task, players[0].id, true);
        let mut writer_b = add_test_room_player(&mut task, players[1].id, true);
        task.match_player_count = 2;
        task.match_human_count = 2;
        task.outcome_sent.insert(players[1].id);

        task.players
            .get(&players[0].id)
            .unwrap()
            .msg_tx
            .try_send_snapshot(replay_transition_test_snapshot(99));
        task.players
            .get(&players[1].id)
            .unwrap()
            .msg_tx
            .try_send_snapshot(replay_transition_test_snapshot(100));

        task.end_match(Some(players[0].id), game.scores(), Some(&game));

        let Phase::ReplayViewer(session) = &task.phase else {
            panic!("match should transition into replay viewer");
        };
        assert_eq!(session.current_tick(), 0);
        assert_eq!(session.speed, ReplaySession::DEFAULT_SPEED);
        assert_eq!(session.vision_player_ids_for(players[0].id), vec![1, 2]);
        assert!(writer_a.snapshots.take().is_none());
        assert!(writer_b.snapshots.take().is_none());

        let a_messages: Vec<_> =
            std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).collect();
        let b_messages: Vec<_> =
            std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()).collect();
        assert!(a_messages
            .iter()
            .any(|msg| matches!(msg, ServerMessage::GameOver { .. })));
        assert!(a_messages.iter().any(|msg| {
            matches!(msg, ServerMessage::Start(payload) if payload.replay.is_some() && payload.tick == 0)
        }));
        assert!(a_messages.iter().any(
            |msg| matches!(msg, ServerMessage::ReplayState(state) if state.current_tick == 0)
        ));
        assert!(!b_messages
            .iter()
            .any(|msg| matches!(msg, ServerMessage::GameOver { .. })));
        assert!(b_messages.iter().any(|msg| {
            matches!(msg, ServerMessage::Start(payload) if payload.replay.is_some() && payload.tick == 0)
        }));
        assert!(b_messages.iter().any(
            |msg| matches!(msg, ServerMessage::ReplayState(state) if state.current_tick == 0)
        ));
    }

    #[test]
    fn replay_viewer_can_return_to_clean_lobby_for_rematch() {
        let players = replay_test_players(2);
        let (game, _artifact) = replay_test_artifact(&players, 1);
        let mut task = RoomTask::new(
            "post-match-lobby-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_test_room_player(&mut task, players[0].id, true);
        let _writer_b = add_test_room_player(&mut task, players[1].id, true);
        task.match_player_count = 2;
        task.match_human_count = 2;

        task.end_match(Some(players[0].id), game.scores(), Some(&game));
        assert!(matches!(task.phase, Phase::ReplayViewer(_)));

        task.players
            .get(&players[0].id)
            .unwrap()
            .msg_tx
            .try_send_snapshot(replay_transition_test_snapshot(101));

        task.on_return_to_lobby(players[0].id);

        assert!(matches!(task.phase, Phase::Lobby));
        assert_eq!(task.match_player_count, 0);
        assert_eq!(task.match_human_count, 0);
        assert!(task.players.values().all(|player| !player.ready));
        assert!(writer_a.snapshots.take().is_none());
        let messages: Vec<_> =
            std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).collect();
        assert!(messages
            .iter()
            .any(|msg| matches!(msg, ServerMessage::Lobby { can_start, .. } if !can_start)));
    }

    #[test]
    fn dedicated_replay_room_return_to_lobby_does_not_stop_other_viewers() {
        let players = replay_test_players(2);
        let (_game, artifact) = replay_test_artifact(&players, 2);
        let mut task = RoomTask::new(
            "persisted-replay-return-test".to_string(),
            RoomMode::Replay { artifact },
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx_a, _writer_a) = ConnectionSink::new();
        let (ack_a, mut ack_rx_a) = tokio::sync::oneshot::channel();
        let (msg_tx_b, writer_b) = ConnectionSink::new();
        let (ack_b, mut ack_rx_b) = tokio::sync::oneshot::channel();

        task.on_join(99, "Viewer A".to_string(), true, msg_tx_a, ack_a);
        task.on_join(100, "Viewer B".to_string(), true, msg_tx_b, ack_b);

        assert_eq!(ack_rx_a.try_recv(), Ok(true));
        assert_eq!(ack_rx_b.try_recv(), Ok(true));
        assert!(matches!(task.phase, Phase::ReplayViewer(_)));

        task.on_return_to_lobby(99);

        assert!(matches!(task.phase, Phase::ReplayViewer(_)));
        assert!(task.players.contains_key(&99));
        assert!(task.players.contains_key(&100));

        task.on_tick_replay_viewer(TokioInstant::now());
        assert!(
            writer_b.snapshots.take().is_some(),
            "other viewers should keep receiving replay snapshots"
        );
    }
}
