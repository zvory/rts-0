use super::connection::send_or_log;
use super::connection::SnapshotSendStatus;
use super::crash_replay::{dump_crash_replay, panic_reason};
use super::dev_replay::{load_replay_artifact, match_seed};
use super::faction_validation::{
    default_faction_id_for, validate_faction_request, FactionRequestContext, FactionValidation,
};
use super::replay_validation;
use super::snapshots::{compact_snapshot_for_wire, union_events};
use super::*;
use crate::game::entity::EntityKind;
use crate::game::map::Map;
use crate::game::replay::{ReplayArtifactV1, ReplayValidationError};
use crate::protocol::{ReplayPlaybackState, SnapshotNetStatus, PREDICTION_PROTOCOL_VERSION};
use crate::structured_log::{self, MatchEndedLog, MatchStartedLog};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use rts_ai::{AiController, AiThinkContext, DEFAULT_LIVE_PROFILE_ID};
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
    last_received_client_seq: u32,
    last_sim_consumed_client_seq: u32,
    last_sim_consumed_client_tick: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
struct PendingClientCommandAck {
    connection_id: u32,
    client_seq: u32,
}

fn normalize_start_team_id(player_id: u32, team_id: TeamId) -> TeamId {
    if team_id == 0 {
        player_id
    } else {
        team_id
    }
}

/// A computer opponent seated in a room. Has an id (for the lobby list / removal) and a name, but
/// no socket — it is materialized into an AI-driven player only when the match starts.
struct AiSlot {
    id: u32,
    name: String,
    team_id: TeamId,
    faction_id: String,
    profile_id: &'static str,
}

const MAX_LOBBY_TEAMS: TeamId = 4;

const AUTOMATED_MATCH_HISTORY_ROOM_PREFIXES: [&str; 4] =
    ["itest-", "ai-itest-", "client-smoke-", "reg-"];
const MATCH_COUNTDOWN_WORDS: [&str; 3] = ["Drei!", "Zwei!", "Eins!"];

fn match_countdown_duration() -> Duration {
    #[cfg(test)]
    {
        Duration::from_millis(1)
    }
    #[cfg(not(test))]
    {
        Duration::from_secs(3)
    }
}

fn server_build_sha() -> &'static str {
    crate::build_info::build_id()
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

fn live_ai_controllers(
    players: &[PlayerInit],
    ai_slots: &[AiSlot],
    seed: u32,
) -> Vec<AiController> {
    let mut rng = SmallRng::seed_from_u64((seed as u64) ^ 0xA17E_5EED);
    players
        .iter()
        .filter(|player| player.is_ai)
        .map(|player| {
            let profile_id = ai_slots
                .iter()
                .find(|ai| ai.id == player.id)
                .map(|ai| ai.profile_id)
                .unwrap_or_else(|| rts_ai::random_live_profile_id(&mut rng));
            AiController::with_profile_id(player.id, profile_id)
        })
        .collect()
}

/// The room's current mode. `InGame` owns the live simulation outright.
enum Phase {
    Lobby,
    InGame(Box<Game>),
    ReplayViewer(Box<ReplaySession>),
    BranchStaging(Box<BranchStagingState>),
}

#[derive(Clone)]
pub(super) enum RoomMode {
    Normal,
    DevSelfPlay(DevSelfPlayConfig),
    DevScenario(DevScenarioConfig),
    Replay { artifact: ReplayArtifactV1 },
    ReplayBranch { seed: ReplayBranchSeed },
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
    FactoryZeroGapPerpendicular,
}

enum DevDriver {
    Live(LiveSelfPlay),
    Scenario(DevScenarioDriver),
}

struct DevScenarioDriver {
    player_id: u32,
    units: Vec<u32>,
    goal: (f32, f32),
    issue_after_ticks: u32,
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
    keyframes: Vec<ReplayKeyframe>,
    duration_ticks: u32,
    speed: f32,
    viewer_vision: HashMap<u32, ReplayVisionSelection>,
    last_controller_id: Option<u32>,
    last_seek_at: Option<StdInstant>,
}

struct ReplayKeyframe {
    tick: u32,
    game: Box<Game>,
    next_command: usize,
}

struct BranchStagingState {
    seed: ReplayBranchSeed,
    claimed_by_seat: HashMap<u32, u32>,
}

impl BranchStagingState {
    fn new(seed: ReplayBranchSeed) -> Self {
        Self {
            seed,
            claimed_by_seat: HashMap::new(),
        }
    }

    fn source_tick(&self) -> u32 {
        self.seed.source_tick
    }

    fn can_start(&self) -> bool {
        self.seed
            .seats
            .iter()
            .filter(|seat| seat.claimable)
            .all(|seat| self.claimed_by_seat.contains_key(&seat.player_id))
    }

    fn claimant_for_occupant(&self, occupant_id: u32) -> Option<u32> {
        self.claimed_by_seat
            .iter()
            .find_map(|(seat_player_id, claimant_id)| {
                (*claimant_id == occupant_id).then_some(*seat_player_id)
            })
    }

    fn claim(&mut self, occupant_id: u32, seat_player_id: u32) -> Result<(), &'static str> {
        if !self
            .seed
            .seats
            .iter()
            .any(|seat| seat.player_id == seat_player_id && seat.claimable)
        {
            return Err("unknown branch seat");
        }
        if self.claimant_for_occupant(occupant_id).is_some() {
            return Err("occupant already claimed a branch seat");
        }
        if self.claimed_by_seat.contains_key(&seat_player_id) {
            return Err("branch seat already claimed");
        }
        self.claimed_by_seat.insert(seat_player_id, occupant_id);
        Ok(())
    }

    fn release(&mut self, occupant_id: u32, seat_player_id: u32) -> bool {
        if self.claimed_by_seat.get(&seat_player_id) != Some(&occupant_id) {
            return false;
        }
        self.claimed_by_seat.remove(&seat_player_id);
        true
    }

    fn release_occupant(&mut self, occupant_id: u32) {
        self.claimed_by_seat
            .retain(|_, claimant_id| *claimant_id != occupant_id);
    }

    fn connection_to_seat_map(&self) -> Option<HashMap<u32, u32>> {
        if !self.can_start() {
            return None;
        }
        Some(
            self.claimed_by_seat
                .iter()
                .map(|(seat_player_id, occupant_id)| (*occupant_id, *seat_player_id))
                .collect(),
        )
    }

    fn seats_for_message(&self, players: &HashMap<u32, RoomPlayer>) -> Vec<BranchStagingSeat> {
        self.seed
            .seats
            .iter()
            .map(|seat| {
                let claimant_id = self.claimed_by_seat.get(&seat.player_id).copied();
                let claimant_name =
                    claimant_id.and_then(|id| players.get(&id).map(|p| p.name.clone()));
                BranchStagingSeat {
                    player_id: seat.player_id,
                    team_id: seat.team_id,
                    faction_id: seat.faction_id.clone(),
                    name: seat.name.clone(),
                    color: seat.color.clone(),
                    claimant_id,
                    claimant_name,
                }
            })
            .collect()
    }
}

impl ReplaySession {
    #[allow(dead_code)]
    const DEFAULT_SPEED: f32 = 2.0;
    const PAUSED_SPEED: f32 = 0.0;
    const MIN_SPEED: f32 = 0.125;
    const MAX_SPEED: f32 = 8.0;
    const MAX_DURATION_TICKS: u32 = 30 * 60 * 60;
    const MAX_COMMAND_LOG_ENTRIES: usize = 200_000;
    const SEEK_COOLDOWN: Duration = Duration::from_millis(500);
    const KEYFRAME_INTERVAL_TICKS: u32 = 2_000;

    #[allow(dead_code)]
    fn new(artifact: ReplayArtifactV1) -> Result<Self, String> {
        Self::validate_artifact_limits(&artifact)?;
        let duration_ticks = artifact.duration_ticks;
        let build_start = StdInstant::now();
        let game = Box::new(Self::build_game(&artifact)?);
        let keyframes = vec![ReplayKeyframe {
            tick: 0,
            game: Box::new(game.clone_for_replay_keyframe()),
            next_command: 0,
        }];
        crate::log_info!(
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
            keyframes,
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
        replay_validation::validate_faction_loadouts(artifact)?;
        let seen_players: HashSet<u32> = artifact.players.iter().map(|player| player.id).collect();
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
            .or_else(|err| match err {
                ReplayValidationError::BuildShaMismatch { artifact, running } => {
                    crate::log_warn!(
                        replay_build_sha = %artifact,
                        server_build_sha = %running,
                        "replay build differs from current server; attempting playback"
                    );
                    Ok(())
                }
                err => Err(err),
            })
            .map_err(|err| err.to_string())?;
        let replay_start_players: Vec<_> = artifact
            .players
            .iter()
            .map(|player| {
                (
                    player.id,
                    normalize_start_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map = Map::load_for_players(&artifact.map_name, &replay_start_players, artifact.seed)
            .map_err(|err| format!("cannot load replay map: {err}"))?;
        Ok(Game::new_for_replay_with_map_metadata(
            &artifact.players,
            artifact.seed,
            &artifact.player_loadouts,
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
            keyframe_ticks: self
                .keyframes
                .iter()
                .map(|keyframe| keyframe.tick)
                .collect(),
            speed: self.speed,
            paused: self.speed == Self::PAUSED_SPEED,
            ended: self.current_tick() >= self.duration_ticks,
            controller_id: self.last_controller_id,
        }
    }

    fn current_tick(&self) -> u32 {
        self.game.tick_count()
    }

    fn branch_seed(&self) -> Result<ReplayBranchSeed, String> {
        if self.artifact.players.iter().any(|player| player.is_ai) {
            return Err("Replay branching does not support replays with AI seats yet.".to_string());
        }
        let source_tick = self.current_tick();
        let seats = self
            .artifact
            .players
            .iter()
            .map(|player| ReplayBranchSeat {
                player_id: player.id,
                team_id: normalize_start_team_id(player.id, player.team_id),
                faction_id: player.faction_id.clone(),
                name: player.name.clone(),
                color: player.color.clone(),
                claimable: true,
            })
            .collect();
        Ok(ReplayBranchSeed {
            source_replay: self.artifact.start_metadata(),
            source_tick,
            game: Box::new(self.game.clone_for_replay_keyframe()),
            seats,
        })
    }

    fn set_speed(&mut self, controller_id: u32, speed: f32) {
        self.speed = if speed == Self::PAUSED_SPEED {
            Self::PAUSED_SPEED
        } else {
            speed.clamp(Self::MIN_SPEED, Self::MAX_SPEED)
        };
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

    fn record_keyframe_if_due(&mut self) {
        let tick = self.current_tick();
        if tick == 0 || !tick.is_multiple_of(Self::KEYFRAME_INTERVAL_TICKS) {
            return;
        }
        match self
            .keyframes
            .binary_search_by_key(&tick, |keyframe| keyframe.tick)
        {
            Ok(_) => (),
            Err(index) => self.keyframes.insert(
                index,
                ReplayKeyframe {
                    tick,
                    game: Box::new(self.game.clone_for_replay_keyframe()),
                    next_command: self.next_command,
                },
            ),
        }
    }

    fn seek_back(
        &mut self,
        room: &str,
        viewer_count: usize,
        controller_id: u32,
        ticks_back: u32,
    ) -> Result<u32, String> {
        let target_tick = self
            .current_tick()
            .saturating_sub(ticks_back)
            .min(self.duration_ticks);
        self.seek_to(room, viewer_count, controller_id, target_tick)
    }

    fn seek_to(
        &mut self,
        room: &str,
        viewer_count: usize,
        controller_id: u32,
        target_tick: u32,
    ) -> Result<u32, String> {
        if self
            .last_seek_at
            .is_some_and(|last_seek| last_seek.elapsed() < Self::SEEK_COOLDOWN)
        {
            return Err("Replay seek ignored; wait before seeking again.".to_string());
        }
        let from_tick = self.current_tick();
        let target_tick = target_tick.min(self.duration_ticks);
        let seek_start = StdInstant::now();
        let keyframe_tick = self.rebuild_to(target_tick)?;
        self.last_seek_at = Some(StdInstant::now());
        self.last_controller_id = Some(controller_id);
        crate::log_info!(
            room = %room,
            controller_id,
            viewer_count,
            from_tick,
            to_tick = target_tick,
            keyframe_tick,
            duration_ticks = self.duration_ticks,
            command_count = self.artifact.command_log.len(),
            keyframe_count = self.keyframes.len(),
            rebuild_ms = seek_start.elapsed().as_millis(),
            "replay seek rebuilt"
        );
        Ok(target_tick)
    }

    fn rebuild_to(&mut self, target_tick: u32) -> Result<u32, String> {
        let (keyframe_tick, keyframe_game, keyframe_next_command) = self
            .keyframes
            .iter()
            .rev()
            .find(|keyframe| keyframe.tick <= target_tick)
            .map(|keyframe| {
                (
                    keyframe.tick,
                    keyframe.game.clone_for_replay_keyframe(),
                    keyframe.next_command,
                )
            })
            .ok_or_else(|| "replay has no valid keyframe".to_string())?;
        *self.game = keyframe_game;
        self.next_command = keyframe_next_command;
        while self.current_tick() < target_tick {
            self.enqueue_for_current_tick()?;
            self.game.tick();
            self.record_keyframe_if_due();
        }
        Ok(keyframe_tick)
    }
}

impl DevScenarioDriver {
    fn enqueue_for_tick(&mut self, game: &mut Game) {
        if self.issued {
            return;
        }
        if game.tick_count() < self.issue_after_ticks {
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
    /// Team ids are freeform host-managed slots in the range `1..=MAX_LOBBY_TEAMS`.
    /// Per-human active-seat team assignment. Spectators are omitted and broadcast as team 0.
    human_team_assignments: HashMap<u32, TeamId>,
    /// Per-human active-seat faction selection. Spectators are omitted.
    human_faction_assignments: HashMap<u32, String>,
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
    /// In replay branch live matches, connected ids differ from original replay player ids.
    branch_live_seat_by_connection: HashMap<u32, u32>,
    dev_driver: Option<DevDriver>,
    dev_view_player_id: Option<u32>,
    ai_controllers: Vec<AiController>,
    /// Replay speed multiplier; 1.0 = real-time, 2.0 = 2× faster, etc.
    replay_speed: f32,
    /// Dev-watch pause flag. Kept separate from replay_speed so interval creation never divides
    /// by zero and resume can restore the previous non-zero multiplier.
    dev_watch_paused: bool,
    slow_tick_count: u32,
    pending_client_command_acks: Vec<PendingClientCommandAck>,
    /// Optional persistence sink for resolved matches. `None` disables match-history writes.
    db: Option<Arc<Db>>,
    /// When true, rows written by this room are hidden from non-localhost match-history reads.
    match_history_local_only: bool,
    /// Wall-clock start time of the currently-running match. `None` outside `Phase::InGame`.
    match_started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Correlates every high-signal log line for one live match attempt.
    match_run_id: Option<String>,
    /// Map name the active match was started on. Empty outside `Phase::InGame`.
    match_map_name: String,
    /// Display names of every participant (humans + AI) in seat order, for match-history rows.
    match_participants: Vec<String>,
    /// Pre-match countdown deadline. While set, lobby membership/settings are frozen and the
    /// match starts on the first room tick at or after this instant.
    match_countdown_deadline: Option<TokioInstant>,
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
            human_team_assignments: HashMap::new(),
            human_faction_assignments: HashMap::new(),
            quickstart: false,
            selected_map: "Default".to_string(),
            host_id: None,
            phase: Phase::Lobby,
            match_player_count: 0,
            match_human_count: 0,
            outcome_sent: HashSet::new(),
            branch_live_seat_by_connection: HashMap::new(),
            dev_driver: None,
            dev_view_player_id: None,
            ai_controllers: Vec::new(),
            replay_speed: 1.0,
            dev_watch_paused: false,
            slow_tick_count: 0,
            pending_client_command_acks: Vec::new(),
            db,
            match_history_local_only,
            match_started_at: None,
            match_run_id: None,
            match_map_name: String::new(),
            match_participants: Vec::new(),
            match_countdown_deadline: None,
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
            Phase::ReplayViewer(session) if session.speed == ReplaySession::PAUSED_SPEED => 1.0,
            Phase::ReplayViewer(session) => session.speed,
            Phase::BranchStaging(_) => 1.0,
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
            && !matches!(self.mode, RoomMode::ReplayBranch { .. })
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
                replay_ok,
                msg_tx,
                ack,
            } => self.on_join(player_id, name, spectator, replay_ok, msg_tx, ack),
            RoomEvent::Leave { player_id } => self.on_leave(player_id),
            RoomEvent::Ready { player_id, ready } => self.on_ready(player_id, ready),
            RoomEvent::StartRequest { player_id } => self.on_start_request(player_id),
            RoomEvent::SetTeamPreset { player_id, preset } => {
                self.on_set_team_preset(player_id, preset)
            }
            RoomEvent::SetTeam {
                player_id,
                target,
                team_id,
            } => self.on_set_team(player_id, target, team_id),
            RoomEvent::SetFaction {
                player_id,
                faction_id,
            } => self.on_set_faction(player_id, faction_id),
            RoomEvent::AddAi {
                player_id,
                team_id,
                ai_profile_id,
            } => self.on_add_ai(player_id, team_id, ai_profile_id),
            RoomEvent::SetAiProfile {
                player_id,
                target,
                ai_profile_id,
            } => self.on_set_ai_profile(player_id, target, ai_profile_id),
            RoomEvent::RemoveAi { player_id, target } => self.on_remove_ai(player_id, target),
            RoomEvent::SetQuickstart { player_id, enabled } => {
                self.on_set_quickstart(player_id, enabled)
            }
            RoomEvent::SetSpectator {
                player_id,
                spectator,
            } => self.on_set_spectator(player_id, spectator),
            RoomEvent::Command {
                player_id,
                client_seq,
                cmd,
            } => self.on_command(player_id, client_seq, cmd),
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
            RoomEvent::SeekReplayTo { player_id, tick } => self.on_seek_replay_to(player_id, tick),
            RoomEvent::SetReplayVision { player_id, vision } => {
                self.on_set_replay_vision(player_id, vision)
            }
            RoomEvent::RequestReplayBranch { player_id, reply } => {
                let _ = reply.send(self.on_request_replay_branch(player_id));
            }
            RoomEvent::ClaimBranchSeat {
                player_id,
                seat_player_id,
            } => self.on_claim_branch_seat(player_id, seat_player_id),
            RoomEvent::ReleaseBranchSeat {
                player_id,
                seat_player_id,
            } => self.on_release_branch_seat(player_id, seat_player_id),
            RoomEvent::StartBranch { player_id } => self.on_start_branch(player_id),
            RoomEvent::AnnounceReplayBranch {
                branch_room,
                source_tick,
                seats,
            } => self.on_announce_replay_branch(branch_room, source_tick, seats),
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
        replay_ok: bool,
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
            if !replay_ok {
                self.prompt_for_replay_join(player_id, &msg_tx, ack);
                return;
            }
            self.on_join_replay_room(player_id, name, msg_tx, ack);
            return;
        }
        if matches!(self.mode, RoomMode::ReplayBranch { .. })
            || matches!(self.phase, Phase::BranchStaging(_))
        {
            self.on_join_branch_staging(player_id, name, msg_tx, ack);
            return;
        }
        if matches!(self.phase, Phase::ReplayViewer(_)) {
            if !replay_ok {
                self.prompt_for_replay_join(player_id, &msg_tx, ack);
                return;
            }
            self.on_join_replay_viewer(player_id, name, msg_tx, ack);
            return;
        }
        if self.match_countdown_deadline.is_some() {
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: "Match is starting in this room — try another room.".to_string(),
                },
            );
            crate::log_debug!(room = %self.room, player_id, "rejecting join; match countdown active");
            let _ = ack.send(false);
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
            crate::log_debug!(room = %self.room, player_id, "rejecting join; match in progress");
            let _ = ack.send(false);
            return;
        }
        if !spectator && self.total_player_count() >= MAX_PLAYERS {
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: "Lobby is full — try another room.".to_string(),
                },
            );
            crate::log_debug!(room = %self.room, player_id, "rejecting join; lobby full");
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
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        self.reassign_host_if_needed();
        if !spectator {
            self.assign_missing_team_for(player_id);
            self.assign_missing_faction_for(player_id);
        }
        crate::log_debug!(room = %self.room, player_id, "joined");
        // The player is now in the room; tell the connection it may mark itself joined.
        let _ = ack.send(true);
        self.send_current_shutdown_warning_to(player_id);
        if matches!(self.phase, Phase::Lobby) {
            self.broadcast_lobby();
        }
    }

    fn prompt_for_replay_join(
        &self,
        player_id: u32,
        msg_tx: &ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        send_or_log(
            &self.room,
            player_id,
            msg_tx,
            ServerMessage::JoinReplayPrompt {
                room: self.room.clone(),
            },
        );
        let _ = ack.send(false);
    }

    pub(super) fn on_leave(&mut self, player_id: u32) {
        let Some(removed) = self.players.remove(&player_id) else {
            return;
        };
        let was_spectator = removed.spectator;
        self.order.retain(|&id| id != player_id);
        self.human_team_assignments.remove(&player_id);
        self.human_faction_assignments.remove(&player_id);
        self.outcome_sent.remove(&player_id);
        self.reassign_host_if_needed();
        crate::log_debug!(room = %self.room, player_id, "left");

        // If the room emptied out, fully reset it to a clean lobby so its name is never stuck
        // mid-match (otherwise a 1-player sandbox — which never "ends" — would poison the room
        // for the next person who joins under the same name). The idle room task lives on cheaply.
        if self.players.is_empty() {
            self.mark_match_finished_for_drain();
            self.phase = Phase::Lobby;
            self.match_countdown_deadline = None;
            self.match_player_count = 0;
            self.match_human_count = 0;
            self.outcome_sent.clear();
            self.branch_live_seat_by_connection.clear();
            self.host_id = None;
            // Drop AI opponents too: with no humans there is nobody to host them, and a fresh
            // joiner under this room name should start from a clean lobby.
            self.ai_players.clear();
            self.human_team_assignments.clear();
            self.human_faction_assignments.clear();
            self.dev_driver = None;
            self.dev_view_player_id = None;
            if matches!(self.mode, RoomMode::ReplayBranch { .. }) {
                self.mode = RoomMode::Normal;
            }
            crate::log_debug!(room = %self.room, "room emptied; reset to lobby");
            return;
        }

        let mut broadcast_branch_staging = false;
        let removed_live_seat_id = (!was_spectator).then(|| {
            self.live_seat_id_for_connection(player_id)
                .unwrap_or(player_id)
        });
        match &mut self.phase {
            Phase::Lobby => self.broadcast_lobby(),
            Phase::InGame(game) => {
                // Remove their army so the match can still resolve to a winner.
                if let Some(seat_id) = removed_live_seat_id {
                    game.eliminate(seat_id);
                }
                self.branch_live_seat_by_connection.remove(&player_id);
            }
            Phase::ReplayViewer(session) => {
                session.viewer_vision.remove(&player_id);
            }
            Phase::BranchStaging(staging) => {
                staging.release_occupant(player_id);
                broadcast_branch_staging = true;
            }
        }
        if broadcast_branch_staging {
            self.broadcast_branch_staging();
        }
    }

    pub(super) fn on_ready(&mut self, player_id: u32, ready: bool) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
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
        if self.match_countdown_deadline.is_some() {
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
            crate::log_debug!(room = %self.room, player_id, "ignoring start while server is draining");
            return;
        }
        if self.host_id != Some(player_id) {
            crate::log_debug!(room = %self.room, player_id, "ignoring start from non-host");
            return;
        }
        if !self.can_start() {
            crate::log_debug!(room = %self.room, "ignoring start; not all players ready");
            return;
        }
        if self.should_skip_match_countdown() {
            self.start_match();
            return;
        }
        self.start_match_countdown();
    }

    fn on_set_team_preset(&mut self, player_id: u32, preset: String) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        crate::log_debug!(room = %self.room, preset = %preset, "ignoring deprecated team preset command");
    }

    fn on_set_team(&mut self, player_id: u32, target: u32, team_id: TeamId) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if team_id == 0 {
            crate::log_debug!(room = %self.room, target, "ignoring zero team id");
            return;
        }
        if self
            .players
            .get(&target)
            .map(|player| player.spectator)
            .unwrap_or(false)
        {
            crate::log_debug!(room = %self.room, target, "ignoring spectator team assignment");
            return;
        }
        let known_target = self.human_team_assignments.contains_key(&target)
            || self.ai_players.iter().any(|ai| ai.id == target);
        if !known_target || !self.team_move_allowed(target, team_id) {
            crate::log_debug!(room = %self.room, target, team_id, "ignoring invalid team assignment");
            return;
        }
        if let Some(ai) = self.ai_players.iter_mut().find(|ai| ai.id == target) {
            ai.team_id = team_id;
        } else if self.players.contains_key(&target) {
            self.human_team_assignments.insert(target, team_id);
        }
        self.broadcast_lobby();
    }

    /// Active humans can select their own playable faction in the lobby. The server validates and
    /// ignores unknown, fixture, spectator, countdown, and in-game requests.
    fn on_set_faction(&mut self, player_id: u32, faction_id: String) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) {
            return;
        }
        if self
            .players
            .get(&player_id)
            .map(|player| player.spectator)
            .unwrap_or(true)
        {
            crate::log_debug!(room = %self.room, player_id, "ignoring spectator faction selection");
            return;
        }
        let context = if self.quickstart {
            FactionRequestContext::Quickstart
        } else {
            FactionRequestContext::NormalLobby
        };
        let accepted = match validate_faction_request(context, Some(&faction_id)) {
            FactionValidation::AcceptedPlayable { faction_id }
            | FactionValidation::Defaulted { faction_id } => faction_id,
            FactionValidation::AcceptedFixture { .. } => return,
            FactionValidation::Rejected { requested, reason } => {
                crate::log_debug!(
                    room = %self.room,
                    player_id,
                    faction_id = ?requested,
                    reason = ?reason,
                    "ignoring invalid faction selection"
                );
                return;
            }
        };
        if self.human_faction_for(player_id) == accepted {
            return;
        }
        self.human_faction_assignments.insert(player_id, accepted);
        self.broadcast_lobby();
    }

    /// Host-only: seat a computer opponent. Ignored outside the lobby, from non-hosts, or once
    /// the room is full (humans + AI == [`MAX_PLAYERS`]).
    fn on_add_ai(
        &mut self,
        player_id: u32,
        requested_team_id: Option<TeamId>,
        requested_profile_id: Option<String>,
    ) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.total_player_count() >= MAX_PLAYERS {
            crate::log_debug!(room = %self.room, "ignoring add-ai; room full");
            return;
        }
        let id = next_player_id();
        let name = format!("Computer {}", self.ai_players.len() + 1);
        let team_id = if let Some(team_id) = requested_team_id {
            if !self.team_move_allowed(id, team_id) {
                crate::log_debug!(room = %self.room, team_id, "ignoring invalid AI team assignment");
                return;
            }
            team_id
        } else {
            self.next_default_team_for_new_seat(id)
        };
        self.ai_players.push(AiSlot {
            id,
            name,
            team_id,
            faction_id: default_faction_id_for(FactionRequestContext::AiSeat),
            profile_id: requested_profile_id
                .as_deref()
                .and_then(rts_ai::canonical_live_profile_id)
                .unwrap_or(DEFAULT_LIVE_PROFILE_ID),
        });
        crate::log_debug!(room = %self.room, ai_id = id, "AI opponent added");
        self.broadcast_lobby();
    }

    /// Host-only: select which supported live AI profile an AI opponent will use next match.
    fn on_set_ai_profile(&mut self, player_id: u32, target: u32, requested_profile_id: String) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        let Some(profile_id) = rts_ai::canonical_live_profile_id(&requested_profile_id) else {
            crate::log_debug!(
                room = %self.room,
                target,
                ai_profile_id = %requested_profile_id,
                "ignoring invalid AI profile selection"
            );
            return;
        };
        let Some(ai) = self.ai_players.iter_mut().find(|ai| ai.id == target) else {
            return;
        };
        if ai.profile_id == profile_id {
            return;
        }
        ai.profile_id = profile_id;
        crate::log_debug!(
            room = %self.room,
            ai_id = target,
            ai_profile_id = %profile_id,
            "AI profile selected"
        );
        self.broadcast_lobby();
    }

    /// Host-only: remove a previously-added AI opponent by id. Ignored outside the lobby, from
    /// non-hosts, or for an unknown id.
    fn on_remove_ai(&mut self, player_id: u32, target: u32) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        let before = self.ai_players.len();
        self.ai_players.retain(|a| a.id != target);
        if self.ai_players.len() != before {
            crate::log_debug!(room = %self.room, ai_id = target, "AI opponent removed");
            self.broadcast_lobby();
        }
    }

    /// Host-only: toggle the lobby's boosted opening resources.
    pub(super) fn on_set_quickstart(&mut self, player_id: u32, enabled: bool) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.quickstart != enabled {
            self.quickstart = enabled;
            crate::log_debug!(room = %self.room, enabled, "quickstart toggled");
            self.broadcast_lobby();
        }
    }

    /// Host-only: select a map by name. Ignored outside the lobby or from non-hosts.
    fn on_select_map(&mut self, player_id: u32, map: String) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.selected_map != map {
            self.selected_map = map;
            crate::log_debug!(room = %self.room, map = %self.selected_map, "map selected");
            self.broadcast_lobby();
        }
    }

    fn on_set_spectator(&mut self, player_id: u32, spectator: bool) {
        if self.is_live_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
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
            self.human_team_assignments.remove(&player_id);
            self.human_faction_assignments.remove(&player_id);
        } else {
            if self.total_player_count() >= MAX_PLAYERS {
                crate::log_debug!(room = %self.room, player_id, "ignoring player role switch; room full");
                return;
            }
            let color = self.next_human_color();
            if let Some(player) = self.players.get_mut(&player_id) {
                player.spectator = false;
                player.ready = false;
                player.color = color;
            }
            self.assign_missing_team_for(player_id);
            self.assign_missing_faction_for(player_id);
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

    fn active_seat_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.active_human_ids().collect();
        ids.extend(self.ai_players.iter().map(|ai| ai.id));
        ids
    }

    fn team_id_for_active_seat(&self, id: u32) -> TeamId {
        if let Some(team_id) = self.human_team_assignments.get(&id) {
            return *team_id;
        }
        if let Some(ai) = self.ai_players.iter().find(|ai| ai.id == id) {
            return ai.team_id;
        }
        id
    }

    fn human_faction_for(&self, id: u32) -> String {
        self.human_faction_assignments
            .get(&id)
            .cloned()
            .unwrap_or_else(|| default_faction_id_for(FactionRequestContext::NormalLobby))
    }

    fn team_counts_except(&self, except_id: Option<u32>) -> HashMap<TeamId, usize> {
        let mut counts = HashMap::new();
        for id in self.active_seat_ids() {
            if Some(id) == except_id {
                continue;
            }
            let team_id = self.team_id_for_active_seat(id);
            *counts.entry(team_id).or_insert(0) += 1;
        }
        counts
    }

    fn team_move_allowed(&self, target: u32, team_id: TeamId) -> bool {
        if team_id == 0 {
            return false;
        }
        let mut counts = self.team_counts_except(Some(target));
        let new_count = counts
            .entry(team_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        team_id <= MAX_LOBBY_TEAMS && *new_count <= MAX_PLAYERS
    }

    fn next_default_team_for_new_seat(&self, new_id: u32) -> TeamId {
        let counts = self.team_counts_except(None);
        for team_id in 1..=MAX_LOBBY_TEAMS {
            if counts.get(&team_id).copied().unwrap_or(0) == 0 {
                return team_id;
            }
        }
        new_id.clamp(1, MAX_LOBBY_TEAMS)
    }

    fn assign_missing_team_for(&mut self, player_id: u32) {
        if self.human_team_assignments.contains_key(&player_id) {
            return;
        }
        let team_id = self.next_default_team_for_new_seat(player_id);
        self.human_team_assignments.insert(player_id, team_id);
    }

    fn assign_missing_faction_for(&mut self, player_id: u32) {
        if self.human_faction_assignments.contains_key(&player_id) {
            return;
        }
        self.human_faction_assignments.insert(
            player_id,
            default_faction_id_for(FactionRequestContext::NormalLobby),
        );
    }

    fn team_composition_valid(&self) -> bool {
        let active_ids = self.active_seat_ids();
        if active_ids.is_empty() || active_ids.len() > MAX_PLAYERS {
            return false;
        }
        for id in active_ids {
            let team_id = self.team_id_for_active_seat(id);
            if team_id == 0 || team_id > MAX_LOBBY_TEAMS {
                return false;
            }
        }
        true
    }

    fn spectator_visible_player_ids(&self) -> Vec<u32> {
        if !self.branch_live_seat_by_connection.is_empty() {
            return self
                .branch_live_seat_by_connection
                .values()
                .copied()
                .collect();
        }
        let mut ids: Vec<u32> = self.active_human_ids().collect();
        ids.extend(self.ai_players.iter().map(|ai| ai.id));
        ids
    }

    fn live_seat_id_for_connection(&self, connection_id: u32) -> Option<u32> {
        self.branch_live_seat_by_connection
            .get(&connection_id)
            .copied()
            .or_else(|| {
                self.players
                    .contains_key(&connection_id)
                    .then_some(connection_id)
            })
    }

    fn live_connection_is_player(&self, connection_id: u32) -> bool {
        self.players
            .get(&connection_id)
            .map(|p| !p.spectator)
            .unwrap_or(false)
            && (self.branch_live_seat_by_connection.is_empty()
                || self
                    .branch_live_seat_by_connection
                    .contains_key(&connection_id))
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

    /// Color for the `seat`-th AI opponent. AIs use the same accessible order as humans while
    /// skipping colors already held by active humans, so mixed human/AI rooms stay distinct
    /// without bunching every AI into the palette tail.
    fn ai_color(&self, seat: usize) -> String {
        PLAYER_PALETTE
            .iter()
            .copied()
            .filter(|color| {
                !self
                    .players
                    .values()
                    .any(|player| !player.spectator && player.color == *color)
            })
            .nth(seat)
            .unwrap_or(PLAYER_PALETTE[(self.active_human_count() + seat) % PLAYER_PALETTE.len()])
            .to_string()
    }

    fn on_command(&mut self, player_id: u32, client_seq: u32, cmd: SimCommand) {
        if self.is_live_dev_watch() {
            return;
        }
        if client_seq == 0 {
            crate::log_debug!(room = %self.room, player_id, "ignoring command with reserved clientSeq 0");
            return;
        }
        let live_seat_id = (self.live_connection_is_player(player_id)
            && !self.outcome_sent.contains(&player_id))
        .then(|| {
            self.live_seat_id_for_connection(player_id)
                .unwrap_or(player_id)
        });
        // Commands are ignored unless in-game and the sender is actually in this room. The
        // simulation itself validates ownership/affordability when it applies the command.
        if let Phase::InGame(game) = &mut self.phase {
            if let Some(seat_id) = live_seat_id {
                let Some(player) = self.players.get_mut(&player_id) else {
                    return;
                };
                if client_seq <= player.last_received_client_seq {
                    crate::log_debug!(
                        room = %self.room,
                        player_id,
                        client_seq,
                        last_received = player.last_received_client_seq,
                        "ignoring stale or wrapped command sequence"
                    );
                    return;
                }
                player.last_received_client_seq = client_seq;
                game.enqueue(seat_id, cmd);
                self.pending_client_command_acks
                    .push(PendingClientCommandAck {
                        connection_id: player_id,
                        client_seq,
                    });
            }
        }
    }

    fn on_give_up(&mut self, player_id: u32) {
        if self.is_live_dev_watch() {
            return;
        }
        if !self.live_connection_is_player(player_id) || self.outcome_sent.contains(&player_id) {
            return;
        }
        let seat_id = self
            .live_seat_id_for_connection(player_id)
            .unwrap_or(player_id);

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
            Phase::BranchStaging(staging) => {
                self.phase = Phase::BranchStaging(staging);
                return;
            }
        };

        crate::log_debug!(room = %self.room, player_id, "player gave up");
        game.eliminate(seat_id);
        let alive = game.alive_players();
        let alive_teams = game.alive_team_ids();
        let scores = game.scores();

        if self.match_player_count >= 2 && alive_teams.len() <= 1 {
            let winner_id = alive_teams
                .first()
                .and_then(|team_id| game.first_alive_player_on_team(*team_id));
            self.end_match(winner_id, scores, Some(&game));
            return;
        }

        if self.match_player_count >= 2 {
            self.send_new_defeats(&game, &alive);
        }

        if self.match_player_count < 2 {
            self.mark_match_finished_for_drain();
            self.phase = Phase::Lobby;
            self.match_player_count = 0;
            self.match_human_count = 0;
            self.branch_live_seat_by_connection.clear();
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
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
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
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        let _ = ack.send(true);
        self.send_replay_start_to(player_id);
        self.send_replay_state_to(player_id);
        self.send_observer_analysis_to(player_id);
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
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        let _ = ack.send(true);

        match &self.phase {
            Phase::ReplayViewer(_) => {
                self.send_replay_start_to(player_id);
                self.send_replay_state_to(player_id);
                self.send_observer_analysis_to(player_id);
            }
            Phase::Lobby => match self.replay_session_for_mode() {
                Ok(session) => self.transition_to_replay_viewer(session),
                Err(err) => {
                    crate::log_warn!(room = %self.room, error = %err, "replay setup failed");
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
            Phase::BranchStaging(_) => {}
        }
    }

    fn on_join_branch_staging(
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
        if !matches!(self.phase, Phase::BranchStaging(_)) {
            let seed = match &self.mode {
                RoomMode::ReplayBranch { seed } => seed.clone(),
                _ => {
                    let _ = ack.send(false);
                    return;
                }
            };
            self.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));
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
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        self.reassign_host_if_needed();
        let _ = ack.send(true);
        self.send_current_shutdown_warning_to(player_id);
        self.broadcast_branch_staging();
    }

    fn replay_session_for_mode(&self) -> Result<ReplaySession, String> {
        let artifact = match &self.mode {
            RoomMode::Replay { artifact } => artifact.clone(),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { artifact }) => {
                load_replay_artifact(artifact)?
            }
            RoomMode::ReplayBranch { .. } => {
                return Err("room is not configured for replay playback".to_string());
            }
            _ => return Err("room is not configured for replay playback".to_string()),
        };
        ReplaySession::new(artifact)
    }

    /// A match may start with at least one active participant and every active human ready.
    /// Spectators can host and watch from the lobby, but they do not block readiness.
    fn can_start(&self) -> bool {
        self.match_countdown_deadline.is_none() && self.can_start_now()
    }

    fn can_start_now(&self) -> bool {
        if let Phase::BranchStaging(staging) = &self.phase {
            return !self.drain.is_draining() && staging.can_start();
        }
        !self.drain.is_draining()
            && self.total_player_count() > 0
            && self.team_composition_valid()
            && self
                .players
                .values()
                .filter(|p| !p.spectator)
                .all(|p| p.ready)
    }

    fn should_skip_match_countdown(&self) -> bool {
        self.quickstart || self.total_player_count() <= 1
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
                    team_id: if p.spectator {
                        0
                    } else {
                        self.team_id_for_active_seat(*id)
                    },
                    faction_id: if p.spectator {
                        default_faction_id_for(FactionRequestContext::NormalLobby)
                    } else {
                        self.human_faction_for(*id)
                    },
                    name: p.name.clone(),
                    ready: p.ready,
                    color: p.color.clone(),
                    is_ai: false,
                    ai_profile_id: None,
                    is_spectator: p.spectator,
                })
            })
            .collect();
        for (seat, ai) in self.ai_players.iter().enumerate() {
            players.push(LobbyPlayer {
                id: ai.id,
                team_id: ai.team_id,
                faction_id: ai.faction_id.clone(),
                name: ai.name.clone(),
                ready: true,
                color: self.ai_color(seat),
                is_ai: true,
                ai_profile_id: Some(ai.profile_id.to_string()),
                is_spectator: false,
            });
        }
        let msg = ServerMessage::Lobby {
            room: self.room.clone(),
            host_id,
            players,
            can_start: self.can_start(),
            quickstart: self.quickstart,
            team_preset: "custom".to_string(),
            map: self.selected_map.clone(),
            maps: Map::list_available(),
        };
        self.broadcast(&msg);
    }

    fn start_match_countdown(&mut self) {
        let duration = match_countdown_duration();
        self.match_countdown_deadline = Some(TokioInstant::now() + duration);
        if matches!(self.phase, Phase::BranchStaging(_)) {
            self.broadcast_branch_staging();
        } else {
            self.broadcast_lobby();
        }
        let msg = ServerMessage::MatchCountdown {
            duration_ms: duration.as_millis() as u32,
            words: MATCH_COUNTDOWN_WORDS
                .iter()
                .map(|word| (*word).to_string())
                .collect(),
        };
        self.broadcast(&msg);
        crate::log_info!(room = %self.room, "match countdown started");
    }

    fn finish_match_countdown_if_due(&mut self) -> bool {
        let Some(deadline) = self.match_countdown_deadline else {
            return false;
        };
        if TokioInstant::now() < deadline {
            return true;
        }
        self.match_countdown_deadline = None;
        if self.can_start_now() {
            if matches!(self.phase, Phase::BranchStaging(_)) {
                self.start_branch_live();
            } else {
                self.start_match();
            }
        } else {
            crate::log_debug!(room = %self.room, "match countdown aborted; start preconditions changed");
            if matches!(self.phase, Phase::BranchStaging(_)) {
                self.broadcast_branch_staging();
            } else {
                self.broadcast_lobby();
            }
        }
        true
    }

    /// Transition from `Lobby` to `InGame`: create the simulation and send each player their
    /// own `start` payload. Only called from `on_start_request` once preconditions hold.
    fn start_match(&mut self) {
        self.match_countdown_deadline = None;
        self.reset_match_net_status();
        let mut inits: Vec<PlayerInit> = self
            .active_human_ids()
            .filter_map(|id| {
                self.players.get(&id).map(|p| PlayerInit {
                    id,
                    team_id: self.team_id_for_active_seat(id),
                    faction_id: self.human_faction_for(id),
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
                team_id: ai.team_id,
                faction_id: ai.faction_id.clone(),
                name: ai.name.clone(),
                color: self.ai_color(seat),
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
                crate::log_warn!(room = %self.room, error = %err, "map metadata load failed at start");
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
        let start_players: Vec<_> = inits
            .iter()
            .map(|player| {
                (
                    player.id,
                    normalize_start_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map = match Map::load_for_players(&self.selected_map, &start_players, seed) {
            Ok(m) => m,
            Err(err) => {
                let msg = format!("Cannot load map \"{}\": {err}", self.selected_map);
                crate::log_warn!(room = %self.room, error = %err, "map load failed at start");
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
        let match_run_id = structured_log::new_match_run_id(&self.room);
        self.match_run_id = Some(match_run_id);
        self.match_map_name = self.selected_map.clone();
        self.match_participants = inits.iter().map(|p| p.name.clone()).collect();
        self.outcome_sent.clear();
        self.ai_controllers = live_ai_controllers(&inits, &self.ai_players, seed);

        // Each player gets the shared static payload but stamped with their own id.
        for &id in &self.order {
            if let Some(player) = self.players.get(&id) {
                let per_player = StartPayload {
                    player_id: id,
                    spectator: player.spectator,
                    prediction_build_id: (!player.spectator)
                        .then(|| server_build_sha().to_string()),
                    prediction_version: if player.spectator {
                        0
                    } else {
                        PREDICTION_PROTOCOL_VERSION
                    },
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

        structured_log::log_match_started(MatchStartedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
            mode: "live",
            map: &self.match_map_name,
            seed,
            players: self.match_player_count,
            humans: self.match_human_count,
            ai: self.ai_players.len(),
            quickstart: self.quickstart,
            participants: &self.match_participants,
        });
        self.mark_match_started_for_drain();
        self.phase = Phase::InGame(Box::new(game));
    }

    fn start_branch_live(&mut self) {
        self.match_countdown_deadline = None;
        self.reset_match_net_status();
        let staging = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::BranchStaging(staging) => staging,
            other => {
                self.phase = other;
                return;
            }
        };
        for seat in &staging.seed.seats {
            if let FactionValidation::Rejected { requested, reason } = validate_faction_request(
                FactionRequestContext::ReplayBranch,
                Some(&seat.faction_id),
            ) {
                crate::log_warn!(
                    room = %self.room,
                    seat_player_id = seat.player_id,
                    faction_id = ?requested,
                    reason = ?reason,
                    "replay branch seat rejected by faction policy"
                );
                self.phase = Phase::BranchStaging(staging);
                self.broadcast_branch_staging();
                return;
            }
        }
        let Some(seat_by_connection) = staging.connection_to_seat_map() else {
            self.phase = Phase::BranchStaging(staging);
            self.broadcast_branch_staging();
            return;
        };
        if !seat_by_connection
            .keys()
            .all(|connection_id| self.players.contains_key(connection_id))
        {
            self.phase = Phase::BranchStaging(staging);
            self.broadcast_branch_staging();
            return;
        }

        let game = staging.seed.game.clone_for_replay_keyframe();
        let payload = game.start_payload();
        let active_seats: HashSet<u32> = seat_by_connection.values().copied().collect();
        self.branch_live_seat_by_connection = seat_by_connection;
        self.match_player_count = active_seats.len();
        self.match_human_count = active_seats.len();
        self.match_started_at = Some(chrono::Utc::now());
        let match_run_id = structured_log::new_match_run_id(&self.room);
        self.match_run_id = Some(match_run_id);
        self.match_map_name = staging.seed.source_replay.map_name.clone();
        self.match_participants = staging
            .seed
            .seats
            .iter()
            .filter(|seat| active_seats.contains(&seat.player_id))
            .map(|seat| seat.name.clone())
            .collect();
        self.outcome_sent.clear();
        self.ai_controllers.clear();
        self.dev_driver = None;
        self.dev_view_player_id = None;

        for &connection_id in &self.order {
            let Some(player) = self.players.get_mut(&connection_id) else {
                continue;
            };
            let mapped_seat = self
                .branch_live_seat_by_connection
                .get(&connection_id)
                .copied();
            player.spectator = mapped_seat.is_none();
            player.ready = false;
            player.msg_tx.clear_pending_snapshot();
            let per_player = StartPayload {
                player_id: mapped_seat.unwrap_or(connection_id),
                spectator: mapped_seat.is_none(),
                prediction_build_id: mapped_seat.map(|_| server_build_sha().to_string()),
                prediction_version: if mapped_seat.is_some() {
                    PREDICTION_PROTOCOL_VERSION
                } else {
                    0
                },
                replay: None,
                ..payload.clone()
            };
            send_or_log(
                &self.room,
                connection_id,
                &player.msg_tx,
                ServerMessage::Start(per_player),
            );
        }

        structured_log::log_match_started(MatchStartedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
            mode: "replay_branch",
            map: &self.match_map_name,
            seed: staging.seed.source_replay.seed,
            players: self.match_player_count,
            humans: self.match_human_count,
            ai: 0,
            quickstart: false,
            participants: &self.match_participants,
        });
        self.mark_match_started_for_drain();
        self.phase = Phase::InGame(Box::new(game));
    }

    fn start_dev_session(&mut self) {
        self.reset_match_net_status();
        let (game, driver, view_player_id) = match self.build_dev_session() {
            Ok(session) => session,
            Err(err) => {
                crate::log_warn!(room = %self.room, error = %err, "dev session bootstrap failed");
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
        crate::log_info!(room = %self.room, "dev session started");
    }

    fn build_dev_session(&self) -> Result<(Game, DevDriver, u32), String> {
        match &self.mode {
            RoomMode::Normal => Err("room is not configured for a dev session".to_string()),
            RoomMode::Replay { .. } => Err("room is not configured for a dev session".to_string()),
            RoomMode::ReplayBranch { .. } => {
                Err("room is not configured for a dev session".to_string())
            }
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay { .. }) => {
                Err("saved self-play replays use the replay viewer".to_string())
            }
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Live) => {
                let driver = LiveSelfPlay::default_match();
                let players = driver.players().to_vec();
                for player in &players {
                    if let FactionValidation::Rejected { requested, reason } =
                        validate_faction_request(
                            FactionRequestContext::SelfPlay,
                            Some(&player.faction_id),
                        )
                    {
                        return Err(format!(
                            "self-play player {} has unsupported faction {:?}: {:?}",
                            player.id, requested, reason
                        ));
                    }
                }
                let view_player_id = players
                    .first()
                    .map(|p| p.id)
                    .ok_or_else(|| "live self-play configured with no players".to_string())?;
                let seed = match_seed();
                let game = Game::new_without_ai_controllers(&players, seed);
                Ok((game, DevDriver::Live(driver), view_player_id))
            }
            RoomMode::DevScenario(config) => {
                let _scenario_faction_id =
                    default_faction_id_for(FactionRequestContext::DevScenario);
                match config.id {
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
                            issue_after_ticks: setup.issue_after_ticks,
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
                            issue_after_ticks: setup.issue_after_ticks,
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
                            issue_after_ticks: setup.issue_after_ticks,
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
                            issue_after_ticks: setup.issue_after_ticks,
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
                            issue_after_ticks: setup.issue_after_ticks,
                            issued: false,
                        };
                        Ok((setup.game, DevDriver::Scenario(driver), setup.player_id))
                    }
                    DevScenarioId::FactoryZeroGapPerpendicular => {
                        let setup = Game::new_factory_zero_gap_perpendicular_scenario(
                            config.unit,
                            config.count,
                            match_seed(),
                        )?;
                        let driver = DevScenarioDriver {
                            player_id: setup.player_id,
                            units: setup.units,
                            goal: setup.goal,
                            issue_after_ticks: setup.issue_after_ticks,
                            issued: false,
                        };
                        Ok((setup.game, DevDriver::Scenario(driver), setup.player_id))
                    }
                }
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
            spectator: true,
            prediction_build_id: None,
            prediction_version: 0,
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

    fn send_observer_analysis_to(&self, watcher_id: u32) {
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
            ServerMessage::ObserverAnalysis(session.game.observer_analysis()),
        );
    }

    fn broadcast_replay_state_for(&self, session: &ReplaySession) {
        let msg = ServerMessage::ReplayState(session.state());
        self.broadcast(&msg);
    }

    fn broadcast_observer_analysis_for(&self, session: &ReplaySession) {
        let msg = ServerMessage::ObserverAnalysis(session.game.observer_analysis());
        self.broadcast(&msg);
    }

    fn broadcast_live_observer_analysis_to_spectators(&self, game: &Game) {
        let spectator_ids: Vec<u32> = self
            .order
            .iter()
            .copied()
            .filter(|id| self.players.get(id).is_some_and(|player| player.spectator))
            .collect();
        if spectator_ids.is_empty() {
            return;
        }

        let msg = ServerMessage::ObserverAnalysis(game.observer_analysis());
        for id in spectator_ids {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            send_or_log(&self.room, id, &player.msg_tx, msg.clone());
        }
    }

    fn broadcast_dev_watch_state(&self) {
        if !matches!(self.mode, RoomMode::DevScenario(_)) {
            return;
        }
        let Phase::InGame(game) = &self.phase else {
            return;
        };
        self.broadcast(&ServerMessage::ReplayState(ReplayPlaybackState {
            current_tick: game.tick_count(),
            duration_ticks: 0,
            keyframe_ticks: Vec::new(),
            speed: if self.dev_watch_paused {
                0.0
            } else {
                self.replay_speed
            },
            paused: self.dev_watch_paused,
            ended: false,
            controller_id: None,
        }));
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
            if matches!(&self.phase, Phase::ReplayViewer(session) if session.speed == ReplaySession::PAUSED_SPEED)
            {
                return;
            }
            self.on_tick_replay_viewer(scheduled);
            return;
        }
        if self.finish_match_countdown_if_due() {
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
            Phase::BranchStaging(staging) => {
                self.phase = Phase::BranchStaging(staging);
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
        self.record_consumed_client_sequences(game.tick_count());
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
            let mapped_seat = self.branch_live_seat_by_connection.get(id).copied();
            let snapshot_start = StdInstant::now();
            let mut snapshot = if player.spectator {
                game.snapshot_for_spectator(&spectator_visible_players)
            } else {
                game.snapshot_for(mapped_seat.unwrap_or(*id))
            };
            if player.spectator {
                snapshot.events.extend(full_vision_events.clone());
            } else if let Some(mut events) = per_player_events.remove(&mapped_seat.unwrap_or(*id)) {
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
        self.broadcast_live_observer_analysis_to_spectators(&game);

        // Check for game over. A 1-player match never ends (sandbox/exploration mode).
        let outcome_start = StdInstant::now();
        let alive = game.alive_players();
        let alive_teams = game.alive_team_ids();
        if self.match_player_count >= 2 && alive_teams.len() <= 1 {
            if let Some(perf) = perf.as_mut() {
                perf.record_phase("outcome_checks", outcome_start.elapsed());
            }
            self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
            let winner_id = alive_teams
                .first()
                .and_then(|team_id| game.first_alive_player_on_team(*team_id));
            self.end_match(winner_id, game.scores(), Some(&game));
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
            let alive_player_ids = game.alive_players();
            let mut commands = Vec::new();
            for controller in &mut self.ai_controllers {
                let player_id = controller.player_id();
                if !alive_player_ids.contains(&player_id) {
                    continue;
                }
                let snapshot = game.snapshot_for(player_id);
                commands.extend(
                    controller
                        .think(AiThinkContext {
                            start: &start,
                            snapshot: &snapshot,
                            alive_player_ids: &alive_player_ids,
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
            Phase::BranchStaging(staging) => {
                self.phase = Phase::BranchStaging(staging);
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
                crate::log_warn!(room = %self.room, error = %err, "replay command enqueue failed");
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
            session.record_keyframe_if_due();
            self.fanout_replay_snapshots(
                &session,
                per_player_events,
                scheduler_lag,
                tick_start,
                perf.as_mut(),
            );
            self.broadcast_observer_analysis_for(&session);
        } else {
            self.broadcast_replay_state_for(&session);
            self.broadcast_observer_analysis_for(&session);
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
            self.broadcast_dev_watch_state();
            return;
        }
        self.dev_watch_paused = false;
        // Clamp to sensible range matching the UI buttons (0.125× – 8×).
        let clamped = speed.clamp(0.125, 8.0);
        self.replay_speed = clamped;
        self.broadcast_dev_watch_state();
    }

    fn on_step_dev_tick(&mut self, player_id: u32) {
        if !self.players.contains_key(&player_id)
            || !self.dev_watch_paused
            || !matches!(self.mode, RoomMode::DevScenario(_))
        {
            return;
        }
        self.on_tick_dev_selfplay(TokioInstant::now());
        self.broadcast_dev_watch_state();
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
            let analysis = session.game.observer_analysis();
            if let Some(player) = self.players.get(&player_id) {
                send_or_log(
                    &self.room,
                    player_id,
                    &player.msg_tx,
                    ServerMessage::ObserverAnalysis(analysis),
                );
            }
        }
    }

    fn on_request_replay_branch(&self, player_id: u32) -> Result<ReplayBranchSeed, String> {
        if !self.players.contains_key(&player_id) {
            return Err("Cannot branch replay: viewer is not in this room.".to_string());
        }
        let Phase::ReplayViewer(session) = &self.phase else {
            return Err("Cannot branch replay outside replay playback.".to_string());
        };
        session.branch_seed()
    }

    fn on_claim_branch_seat(&mut self, player_id: u32, seat_player_id: u32) {
        if !self.players.contains_key(&player_id) {
            return;
        }
        let result = match &mut self.phase {
            Phase::BranchStaging(staging) => staging.claim(player_id, seat_player_id),
            _ => return,
        };
        match result {
            Ok(()) => self.broadcast_branch_staging(),
            Err(err) => self.send_error_to(player_id, err),
        }
    }

    fn on_release_branch_seat(&mut self, player_id: u32, seat_player_id: u32) {
        if !self.players.contains_key(&player_id) {
            return;
        }
        let released = match &mut self.phase {
            Phase::BranchStaging(staging) => staging.release(player_id, seat_player_id),
            _ => return,
        };
        if released {
            self.broadcast_branch_staging();
        }
    }

    fn on_start_branch(&mut self, player_id: u32) {
        if self.host_id != Some(player_id) {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if self.drain.is_draining() {
            self.send_error_to(
                player_id,
                "Server is draining for deploy; new matches are disabled.",
            );
            return;
        }
        let Some(staging) = self.branch_staging() else {
            return;
        };
        if !staging.can_start() {
            self.send_error_to(
                player_id,
                "All original branch seats must be claimed before launch.",
            );
            return;
        }
        self.start_match_countdown();
    }

    fn on_announce_replay_branch(
        &self,
        branch_room: String,
        source_tick: u32,
        seats: Vec<ReplayBranchSeat>,
    ) {
        if !matches!(self.phase, Phase::ReplayViewer(_)) {
            return;
        }
        self.broadcast(&ServerMessage::ReplayBranchCreated {
            branch_room,
            source_tick,
            seats,
        });
    }

    fn branch_staging(&self) -> Option<&BranchStagingState> {
        match &self.phase {
            Phase::BranchStaging(staging) => Some(staging),
            _ => None,
        }
    }

    fn branch_staging_message(&self, staging: &BranchStagingState) -> ServerMessage {
        let occupants = self
            .order
            .iter()
            .filter_map(|id| {
                self.players.get(id).map(|player| BranchStagingOccupant {
                    id: *id,
                    name: player.name.clone(),
                })
            })
            .collect();
        ServerMessage::BranchStaging {
            room: self.room.clone(),
            source_tick: staging.source_tick(),
            host_id: self.host_id.unwrap_or(0),
            seats: staging.seats_for_message(&self.players),
            occupants,
            can_start: self.match_countdown_deadline.is_none()
                && !self.drain.is_draining()
                && staging.can_start(),
        }
    }

    fn broadcast_branch_staging(&self) {
        let Some(staging) = self.branch_staging() else {
            return;
        };
        self.broadcast(&self.branch_staging_message(staging));
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
            let analysis = seek_result
                .as_ref()
                .ok()
                .map(|_| session.game.observer_analysis());
            match seek_result {
                Ok(_) => {
                    for (viewer_id, msg_tx, start) in starts {
                        send_or_log(&self.room, viewer_id, &msg_tx, ServerMessage::Start(start));
                    }
                    if let Some(state) = state {
                        self.broadcast(&ServerMessage::ReplayState(state));
                    }
                    if let Some(analysis) = analysis {
                        self.broadcast(&ServerMessage::ObserverAnalysis(analysis));
                    }
                }
                Err(err) => {
                    crate::log_warn!(room = %self.room, error = %err, "replay seek failed");
                    self.send_dev_error(&err);
                }
            }
        }
    }

    /// Seek a replay to an absolute tick. No-op outside replay rooms or when no game is active.
    fn on_seek_replay_to(&mut self, player_id: u32, tick: u32) {
        if let Phase::ReplayViewer(session) = &mut self.phase {
            if !self.players.contains_key(&player_id) {
                return;
            }
            let viewer_count = self.players.len();
            let seek_result = session.seek_to(&self.room, viewer_count, player_id, tick);
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
            let analysis = seek_result
                .as_ref()
                .ok()
                .map(|_| session.game.observer_analysis());
            match seek_result {
                Ok(_) => {
                    for (viewer_id, msg_tx, start) in starts {
                        send_or_log(&self.room, viewer_id, &msg_tx, ServerMessage::Start(start));
                    }
                    if let Some(state) = state {
                        self.broadcast(&ServerMessage::ReplayState(state));
                    }
                    if let Some(analysis) = analysis {
                        self.broadcast(&ServerMessage::ObserverAnalysis(analysis));
                    }
                }
                Err(err) => {
                    crate::log_warn!(room = %self.room, error = %err, "replay seek failed");
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
                self.live_connection_is_player(*id)
                    && self
                        .live_seat_id_for_connection(*id)
                        .map(|seat_id| {
                            !alive.contains(&seat_id) && !game.team_has_alive_player(seat_id)
                        })
                        .unwrap_or(false)
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
                    winner_team_id: None,
                    you: "lost".to_string(),
                    scores: scores.clone(),
                },
            );
            self.outcome_sent.insert(id);
        }
    }

    fn team_id_for_score_seat(
        game: Option<&Game>,
        scores: &[PlayerScore],
        seat_id: u32,
    ) -> Option<TeamId> {
        game.and_then(|game| game.team_of_player(seat_id))
            .or_else(|| {
                scores
                    .iter()
                    .find(|score| score.id == seat_id)
                    .map(|score| score.team_id)
                    .filter(|team_id| *team_id != 0)
            })
    }

    /// Resolve a finished match: tell everyone who won and start post-match replay playback.
    fn end_match(&mut self, winner_id: Option<u32>, scores: Vec<PlayerScore>, game: Option<&Game>) {
        let winner_team_id =
            winner_id.and_then(|id| Self::team_id_for_score_seat(game, &scores, id));
        let ended_at = chrono::Utc::now();
        let duration_ms = self.match_started_at.map(|started_at| {
            ended_at
                .signed_duration_since(started_at)
                .num_milliseconds()
                .clamp(0, i32::MAX as i64)
        });
        let duration_ticks = game.map(Game::tick_count);
        let max_head_of_line_count = self
            .players
            .values()
            .map(|player| player.head_of_line_count)
            .max()
            .unwrap_or(0);
        let replay_artifact = game.filter(|_| !self.is_live_dev_watch()).map(|game| {
            ReplayArtifactV1::capture_from_game(game, server_build_sha(), winner_id, scores.clone())
        });
        let will_record_history = self.db.is_some()
            && self.match_started_at.is_some()
            && self.should_record_match_history();
        structured_log::log_match_ended(MatchEndedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref(),
            map: &self.match_map_name,
            participants: &self.match_participants,
            winner_id,
            winner_team_id,
            duration_ms,
            duration_ticks,
            slow_tick_count: self.slow_tick_count,
            max_head_of_line_count,
            score_count: scores.len(),
            replay_captured: replay_artifact.is_some(),
            will_record_history,
        });

        // Persist match history only for real public matches. Human-vs-AI, AI-only,
        // 1-player sandboxes, dev/scenario/replay rooms, and automated test rooms are excluded.
        if let (Some(db), Some(started_at)) = (self.db.clone(), self.match_started_at) {
            if self.should_record_match_history() {
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
                            crate::log_warn!(room = %self.room, error = %err, "failed to serialize replay artifact for match history");
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
        self.match_run_id = None;
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
                let seat_id = self.live_seat_id_for_connection(*id).unwrap_or(*id);
                let seat_team_id = Self::team_id_for_score_seat(game, &scores, seat_id);
                match (winner_team_id, winner_id) {
                    (Some(winner_team_id), _) if seat_team_id == Some(winner_team_id) => "won",
                    (Some(_), _) => "lost",
                    (None, Some(winner_id)) if winner_id == seat_id => "won",
                    (None, Some(_)) => "lost",
                    (None, None) => "draw",
                }
            }
            .to_string();
            send_or_log(
                &self.room,
                *id,
                &player.msg_tx,
                ServerMessage::GameOver {
                    winner_id,
                    winner_team_id,
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
                    crate::log_warn!(room = %self.room, error = %err, "post-match replay setup failed");
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
        self.branch_live_seat_by_connection.clear();
        for player in self.players.values_mut() {
            player.ready = false;
            player.msg_tx.clear_pending_snapshot();
        }
        let recipients = self.order.clone();
        for id in recipients {
            self.send_replay_start_to(id);
            self.send_replay_state_to(id);
            self.send_observer_analysis_to(id);
        }
        crate::log_info!(
            room = %self.room,
            viewer_count = self.players.len(),
            "replay viewer active"
        );
    }

    fn on_return_to_lobby(&mut self, player_id: u32) {
        if !self.players.contains_key(&player_id) || !matches!(self.phase, Phase::ReplayViewer(_)) {
            return;
        }
        self.on_leave(player_id);
    }

    fn return_to_lobby(&mut self) {
        // Reset for the next match: drop the game/replay, clear ready flags, and re-advertise
        // the lobby. AI slots, map selection, and quickstart persist for rematches.
        self.phase = Phase::Lobby;
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.match_run_id = None;
        self.outcome_sent.clear();
        self.branch_live_seat_by_connection.clear();
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
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
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

    fn send_error_to(&self, player_id: u32, msg: &str) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::Error {
                msg: msg.to_string(),
            },
        );
    }

    fn reset_match_net_status(&mut self) {
        self.slow_tick_count = 0;
        self.pending_client_command_acks.clear();
        for player in self.players.values_mut() {
            player.head_of_line_count = 0;
            player.last_received_client_seq = 0;
            player.last_sim_consumed_client_seq = 0;
            player.last_sim_consumed_client_tick = None;
        }
    }

    fn record_consumed_client_sequences(&mut self, tick: u32) {
        let pending = std::mem::take(&mut self.pending_client_command_acks);
        for ack in pending {
            let Some(player) = self.players.get_mut(&ack.connection_id) else {
                continue;
            };
            if ack.client_seq == player.last_sim_consumed_client_seq.saturating_add(1) {
                player.last_sim_consumed_client_seq = ack.client_seq;
                player.last_sim_consumed_client_tick = Some(tick);
            }
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
        let include_prediction_ack = !player.spectator;
        SnapshotNetStatus {
            server_lag_ms: saturating_duration_ms_u16(scheduler_lag),
            tick_ms: saturating_duration_ms_u16(tick_elapsed),
            slow_tick,
            slow_tick_count: self.slow_tick_count,
            head_of_line,
            head_of_line_count: player
                .head_of_line_count
                .saturating_add(u32::from(head_of_line)),
            prediction_version: if include_prediction_ack {
                PREDICTION_PROTOCOL_VERSION
            } else {
                0
            },
            last_sim_consumed_client_seq: if include_prediction_ack {
                player.last_sim_consumed_client_seq
            } else {
                0
            },
            last_sim_consumed_client_tick: if include_prediction_ack {
                player.last_sim_consumed_client_tick
            } else {
                None
            },
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
    use crate::protocol::DEFAULT_FACTION_ID;
    use rts_rules::faction::EMPTY_FIXTURE_FACTION_ID;

    fn replay_test_players(count: usize) -> Vec<PlayerInit> {
        (1..=count as u32)
            .map(|id| PlayerInit {
                id,
                team_id: id,
                faction_id: "kriegsia".to_string(),
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

    fn replay_branch_test_seed(players: &[PlayerInit], ticks: u32) -> ReplayBranchSeed {
        let (_live, artifact) = replay_test_artifact(players, ticks);
        let mut replay = ReplaySession::new(artifact).unwrap();
        while replay.current_tick() < ticks {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }
        replay.branch_seed().unwrap()
    }

    fn write_selfplay_replay_test_artifact(
        name: &str,
        artifact: &ReplayArtifactV1,
    ) -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("selfplay-artifacts")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("replay.json"),
            serde_json::to_vec_pretty(artifact).unwrap(),
        )
        .unwrap();
        dir
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
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        writer
    }

    fn add_test_room_spectator(task: &mut RoomTask, id: u32) -> ConnectionWriter {
        let (msg_tx, writer) = ConnectionSink::new();
        task.order.push(id);
        task.players.insert(
            id,
            RoomPlayer {
                name: format!("Spectator {id}"),
                color: "#6f8fa8".to_string(),
                ready: true,
                spectator: true,
                msg_tx,
                head_of_line_count: 0,
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        writer
    }

    #[test]
    fn ai_colors_start_at_accessibility_palette_head_without_humans() {
        let task = RoomTask::new(
            "ai-colors".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );

        let colors: Vec<String> = (0..4).map(|seat| task.ai_color(seat)).collect();

        assert_eq!(
            colors,
            vec![
                PLAYER_PALETTE[0].to_string(),
                PLAYER_PALETTE[1].to_string(),
                PLAYER_PALETTE[2].to_string(),
                PLAYER_PALETTE[3].to_string(),
            ]
        );
    }

    #[test]
    fn ai_colors_skip_active_human_colors_in_palette_order() {
        let mut task = RoomTask::new(
            "mixed-colors".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        add_test_room_player(&mut task, 1, false);

        let colors: Vec<String> = (0..3).map(|seat| task.ai_color(seat)).collect();

        assert_eq!(
            colors,
            vec![
                PLAYER_PALETTE[1].to_string(),
                PLAYER_PALETTE[2].to_string(),
                PLAYER_PALETTE[3].to_string(),
            ]
        );
    }

    fn add_branch_occupant(task: &mut RoomTask, id: u32) -> ConnectionWriter {
        let (msg_tx, writer) = ConnectionSink::new();
        task.order.push(id);
        task.players.insert(
            id,
            RoomPlayer {
                name: format!("Viewer {id}"),
                color: "#6f8fa8".to_string(),
                ready: true,
                spectator: true,
                msg_tx,
                head_of_line_count: 0,
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        task.reassign_host_if_needed();
        writer
    }

    fn branch_staging_messages(writer: &mut ConnectionWriter) -> Vec<ServerMessage> {
        std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
            .filter(|msg| matches!(msg, ServerMessage::BranchStaging { .. }))
            .collect()
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
            remembered_buildings: Vec::new(),
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
            Phase::BranchStaging(staging) => staging.source_tick(),
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
    fn replay_session_records_keyframes_and_restores_nearest_before_seek_target() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 0);
        artifact.duration_ticks = 2_001;
        let mut replay = ReplaySession::new(artifact).unwrap();

        while replay.current_tick() < replay.duration_ticks {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
            replay.record_keyframe_if_due();
        }

        assert_eq!(
            replay
                .keyframes
                .iter()
                .map(|keyframe| keyframe.tick)
                .collect::<Vec<_>>(),
            vec![0, 2_000]
        );

        let mut expected = replay
            .keyframes
            .iter()
            .find(|keyframe| keyframe.tick == 2_000)
            .expect("replay should record the first interval keyframe")
            .game
            .clone_for_replay_keyframe();
        expected.tick();

        let restored_from = replay.rebuild_to(2_001).unwrap();

        assert_eq!(restored_from, 2_000);
        assert_eq!(replay.current_tick(), 2_001);

        for player in &players {
            assert_eq!(
                replay.game.snapshot_for(player.id),
                expected.snapshot_for(player.id)
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
        assert_eq!(replay.state().keyframe_ticks, vec![0]);

        replay.set_speed(42, 0.0);
        assert_eq!(replay.state().speed, ReplaySession::PAUSED_SPEED);
        assert!(replay.state().paused);

        let target = replay.seek_back("test", 1, 42, u32::MAX).unwrap();
        assert_eq!(target, 0);
        assert_eq!(replay.state().current_tick, 0);
    }

    #[test]
    fn observer_analysis_restores_from_keyframe_without_accumulating_extra_losses() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 1);
        let mut replay = ReplaySession::new(artifact).unwrap();

        replay.game.eliminate(players[1].id);
        let expected = replay.game.observer_analysis();
        replay.keyframes[0] = ReplayKeyframe {
            tick: replay.current_tick(),
            game: Box::new(replay.game.clone_for_replay_keyframe()),
            next_command: replay.next_command,
        };

        replay.game.eliminate(players[0].id);
        replay.rebuild_to(0).unwrap();

        assert_eq!(replay.game.observer_analysis(), expected);
    }

    #[test]
    fn paused_replay_viewer_does_not_advance_on_scheduled_tick() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 3);
        let replay = ReplaySession::new(artifact).unwrap();
        let mut task = RoomTask::new(
            "replay-pause-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        add_test_room_player(&mut task, 99, true);
        task.phase = Phase::ReplayViewer(Box::new(replay));

        task.on_set_replay_speed(99, 0.0);
        assert_eq!(
            task.current_tick_interval(),
            Duration::from_millis(config::TICK_MS)
        );
        task.on_tick(TokioInstant::now());
        assert_eq!(in_game_tick(&task), 0);

        task.on_set_replay_speed(99, 1.0);
        task.on_tick(TokioInstant::now());
        assert_eq!(in_game_tick(&task), 1);
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
    fn replay_session_allows_build_sha_mismatch() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 1);
        artifact.server_build_sha = "older-build".to_string();

        let replay = ReplaySession::new(artifact).unwrap();

        assert_eq!(replay.current_tick(), 0);
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
    fn replay_artifact_limits_require_matching_player_loadouts() {
        let players = replay_test_players(2);
        let (_live, mut missing_artifact) = replay_test_artifact(&players, 0);
        missing_artifact
            .player_loadouts
            .retain(|loadout| loadout.player_id != players[0].id);

        let err = match ReplaySession::new(missing_artifact) {
            Ok(_) => panic!("missing replay loadout should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("missing a loadout"),
            "unexpected artifact reject: {err}"
        );

        let (_live, mut mismatched_artifact) = replay_test_artifact(&players, 0);
        mismatched_artifact.player_loadouts[0].faction_id = "ekat".to_string();

        let err = match ReplaySession::new(mismatched_artifact) {
            Ok(_) => panic!("mismatched replay loadout should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("loadout faction"),
            "unexpected artifact reject: {err}"
        );

        let (_live, mut unknown_loadout_artifact) = replay_test_artifact(&players, 0);
        unknown_loadout_artifact.player_loadouts[0].loadout_id = "kriegsia.missing".to_string();

        let err = match ReplaySession::new(unknown_loadout_artifact) {
            Ok(_) => panic!("unknown replay loadout should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("unknown loadout"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_artifact_limits_reject_unknown_player_loadout() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 0);
        let mut extra_loadout = artifact.player_loadouts[0].clone();
        extra_loadout.player_id = 999;
        artifact.player_loadouts.push(extra_loadout);

        let err = match ReplaySession::new(artifact) {
            Ok(_) => panic!("unknown-player replay loadout should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("unknown player 999"),
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
    fn replay_session_rejects_unknown_or_fixture_faction_ids() {
        let players = replay_test_players(2);
        let (_live, mut unknown_artifact) = replay_test_artifact(&players, 0);
        unknown_artifact.players[0].faction_id = "unknown-faction".to_string();

        let err = match ReplaySession::new(unknown_artifact) {
            Ok(_) => panic!("unsupported replay faction should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("unknown faction"),
            "unexpected artifact reject: {err}"
        );

        let (_live, mut fixture_artifact) = replay_test_artifact(&players, 0);
        fixture_artifact.players[0].faction_id = EMPTY_FIXTURE_FACTION_ID.to_string();

        let err = match ReplaySession::new(fixture_artifact) {
            Ok(_) => panic!("fixture replay faction should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("fixture-only"),
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
    fn replay_join_and_seek_emit_authoritative_analysis() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 4);
        let mut replay = ReplaySession::new(artifact).unwrap();
        for _ in 0..3 {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }
        let mut task = RoomTask::new(
            "replay-analysis-send-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer = add_test_room_player(&mut task, 99, true);
        task.phase = Phase::ReplayViewer(Box::new(replay));

        task.send_replay_start_to(99);
        task.send_replay_state_to(99);
        task.send_observer_analysis_to(99);
        let join_messages: Vec<_> =
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(join_messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::ObserverAnalysis(analysis) if analysis.tick == 3 && analysis.players.len() == 2
        )));

        task.on_seek_replay_to(99, 1);
        let seek_messages: Vec<_> =
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(seek_messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::ObserverAnalysis(analysis) if analysis.tick == 1 && analysis.players.len() == 2
        )));
    }

    #[test]
    fn live_spectator_receives_observer_analysis_but_active_players_do_not() {
        let mut task = RoomTask::new(
            "live-spectator-analysis-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_test_room_player(&mut task, 1, true);
        let mut writer_b = add_test_room_player(&mut task, 2, true);
        let mut writer_spectator = add_test_room_spectator(&mut task, 99);

        task.start_match();
        while writer_a.reliable_rx.try_recv().is_ok() {}
        while writer_b.reliable_rx.try_recv().is_ok() {}
        while writer_spectator.reliable_rx.try_recv().is_ok() {}

        task.on_tick(TokioInstant::now());

        let spectator_messages: Vec<_> =
            std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
        assert!(spectator_messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::ObserverAnalysis(analysis) if analysis.tick == 1 && analysis.players.len() == 2
        )));
        let mut active_messages: Vec<_> =
            std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).collect();
        active_messages.extend(std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()));
        assert!(!active_messages
            .iter()
            .any(|msg| matches!(msg, ServerMessage::ObserverAnalysis(_))));
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
            1,
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
    fn replay_branch_request_rejects_outside_replay_viewer() {
        let mut task = RoomTask::new(
            "branch-outside-replay-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer = add_test_room_player(&mut task, 99, true);

        let err = match task.on_request_replay_branch(99) {
            Ok(_) => panic!("branch request outside replay should fail"),
            Err(err) => err,
        };

        assert!(
            err.contains("outside replay playback"),
            "unexpected branch reject: {err}"
        );
        assert!(matches!(task.phase, Phase::Lobby));
    }

    #[test]
    fn replay_branch_seed_captures_current_authoritative_tick() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 5);
        let mut replay = ReplaySession::new(artifact).unwrap();
        for _ in 0..3 {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }
        let mut task = RoomTask::new(
            "branch-current-tick-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer = add_test_room_player(&mut task, 99, true);
        task.phase = Phase::ReplayViewer(Box::new(replay));

        let seed = task.on_request_replay_branch(99).unwrap();

        assert_eq!(seed.source_tick, 3);
        assert_eq!(seed.game.tick_count(), 3);
        assert_eq!(seed.source_replay.duration_ticks, 5);
        assert_eq!(seed.seats.len(), 2);
        assert!(seed.seats.iter().all(|seat| seat.claimable));
    }

    #[test]
    fn replay_branch_seed_preserves_team_and_faction_ids() {
        let mut players = replay_test_players(4);
        players[0].team_id = 1;
        players[1].team_id = 1;
        players[2].team_id = 2;
        players[3].team_id = 2;
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let replay = ReplaySession::new(artifact).unwrap();
        let seed = replay.branch_seed().unwrap();

        assert_eq!(
            seed.seats
                .iter()
                .map(|seat| seat.team_id)
                .collect::<Vec<_>>(),
            vec![1, 1, 2, 2]
        );
        assert!(seed
            .seats
            .iter()
            .all(|seat| seat.faction_id == DEFAULT_FACTION_ID));

        let mut old_players = replay_test_players(2);
        old_players[0].team_id = 0;
        old_players[1].team_id = 0;
        let (_live, old_artifact) = replay_test_artifact(&old_players, 0);
        let old_replay = ReplaySession::new(old_artifact).unwrap();
        let old_seed = old_replay.branch_seed().unwrap();

        assert_eq!(
            old_seed
                .seats
                .iter()
                .map(|seat| seat.team_id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(old_seed
            .seats
            .iter()
            .all(|seat| seat.faction_id == DEFAULT_FACTION_ID));
    }

    #[test]
    fn replay_branch_request_keeps_source_replay_session_intact() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 4);
        let replay = ReplaySession::new(artifact).unwrap();
        let mut task = RoomTask::new(
            "branch-source-intact-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer = add_test_room_player(&mut task, 99, true);
        task.phase = Phase::ReplayViewer(Box::new(replay));

        let seed = task.on_request_replay_branch(99).unwrap();
        let Phase::ReplayViewer(session) = &task.phase else {
            panic!("source room should remain a replay viewer");
        };

        assert_eq!(session.current_tick(), 0);
        assert_eq!(session.duration_ticks, 4);
        assert_eq!(session.artifact.command_log.len(), 0);
        assert_eq!(seed.game.tick_count(), session.current_tick());
    }

    #[test]
    fn replay_branch_request_rejects_ai_seats_without_creating_seed() {
        let mut players = replay_test_players(2);
        players[1].is_ai = true;
        let (_live, artifact) = replay_test_artifact(&players, 1);
        let replay = ReplaySession::new(artifact).unwrap();
        let mut task = RoomTask::new(
            "branch-ai-reject-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer = add_test_room_player(&mut task, 99, true);
        task.phase = Phase::ReplayViewer(Box::new(replay));

        let err = match task.on_request_replay_branch(99) {
            Ok(_) => panic!("branch request with AI seats should fail"),
            Err(err) => err,
        };

        assert!(err.contains("AI seats"), "unexpected branch reject: {err}");
        assert!(matches!(task.phase, Phase::ReplayViewer(_)));
    }

    #[test]
    fn replay_branch_announcement_broadcasts_to_all_viewers() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let replay = ReplaySession::new(artifact).unwrap();
        let mut task = RoomTask::new(
            "branch-broadcast-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_test_room_player(&mut task, 100, true);
        let mut writer_b = add_test_room_player(&mut task, 101, true);
        task.phase = Phase::ReplayViewer(Box::new(replay));

        task.on_announce_replay_branch(
            "__replay_branch__:00000001".to_string(),
            12,
            vec![ReplayBranchSeat {
                player_id: players[0].id,
                team_id: players[0].team_id,
                faction_id: players[0].faction_id.clone(),
                name: players[0].name.clone(),
                color: players[0].color.clone(),
                claimable: true,
            }],
        );

        for writer in [&mut writer_a, &mut writer_b] {
            let msg = writer.reliable_rx.try_recv().expect("branch message");
            assert!(matches!(
                msg,
                ServerMessage::ReplayBranchCreated {
                    branch_room,
                    source_tick: 12,
                    seats
                } if branch_room == "__replay_branch__:00000001"
                    && seats.len() == 1
                    && seats[0].player_id == players[0].id
            ));
        }
    }

    #[test]
    fn branch_staging_seat_claims_are_exclusive() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 2);
        let mut task = RoomTask::new(
            "branch-exclusive-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_branch_occupant(&mut task, 100);
        let mut writer_b = add_branch_occupant(&mut task, 101);
        task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

        task.on_claim_branch_seat(100, players[0].id);
        task.on_claim_branch_seat(101, players[0].id);

        let messages = branch_staging_messages(&mut writer_a);
        let last = messages.last().expect("branch staging update");
        assert!(matches!(
            last,
            ServerMessage::BranchStaging { seats, .. }
                if seats[0].claimant_id == Some(100)
        ));
        assert!(
            std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()).any(|msg| {
                matches!(msg, ServerMessage::Error { msg } if msg.contains("already claimed"))
            })
        );
    }

    #[test]
    fn branch_staging_one_occupant_cannot_claim_multiple_seats() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 0);
        let mut task = RoomTask::new(
            "branch-single-claim-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer = add_branch_occupant(&mut task, 100);
        task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

        task.on_claim_branch_seat(100, players[0].id);
        task.on_claim_branch_seat(100, players[1].id);

        assert!(std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).any(|msg| {
            matches!(msg, ServerMessage::Error { msg } if msg.contains("already claimed a branch seat"))
        }));
        let Phase::BranchStaging(staging) = &task.phase else {
            panic!("branch staging should stay active");
        };
        assert_eq!(staging.claimant_for_occupant(100), Some(players[0].id));
        assert!(!staging.claimed_by_seat.contains_key(&players[1].id));
    }

    #[test]
    fn branch_staging_requires_all_original_seats_before_can_start() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 0);
        let mut task = RoomTask::new(
            "branch-can-start-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_branch_occupant(&mut task, 100);
        let _writer_b = add_branch_occupant(&mut task, 101);
        task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

        task.broadcast_branch_staging();
        task.on_claim_branch_seat(100, players[0].id);
        task.on_claim_branch_seat(101, players[1].id);

        let updates = branch_staging_messages(&mut writer_a);
        assert!(matches!(
            updates.first(),
            Some(ServerMessage::BranchStaging {
                can_start: false,
                ..
            })
        ));
        assert!(matches!(
            updates.last(),
            Some(ServerMessage::BranchStaging {
                can_start: true,
                ..
            })
        ));
    }

    #[test]
    fn branch_staging_allows_release_and_reclaim() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 0);
        let mut task = RoomTask::new(
            "branch-release-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let _writer_a = add_branch_occupant(&mut task, 100);
        let mut writer_b = add_branch_occupant(&mut task, 101);
        task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

        task.on_claim_branch_seat(100, players[0].id);
        task.on_release_branch_seat(100, players[0].id);
        task.on_claim_branch_seat(101, players[0].id);

        let updates = branch_staging_messages(&mut writer_b);
        assert!(matches!(
            updates.last(),
            Some(ServerMessage::BranchStaging { seats, .. })
                if seats[0].claimant_id == Some(101)
        ));
        let Phase::BranchStaging(staging) = &task.phase else {
            panic!("branch staging should stay active");
        };
        assert_eq!(staging.claimant_for_occupant(100), None);
        assert_eq!(staging.claimant_for_occupant(101), Some(players[0].id));
    }

    #[test]
    fn branch_staging_leave_releases_claim_and_reassigns_host() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 0);
        let mut task = RoomTask::new(
            "branch-leave-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let _writer_a = add_branch_occupant(&mut task, 100);
        let mut writer_b = add_branch_occupant(&mut task, 101);
        task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));
        task.on_claim_branch_seat(100, players[0].id);

        task.on_leave(100);

        assert_eq!(task.host_id, Some(101));
        let updates = branch_staging_messages(&mut writer_b);
        assert!(matches!(
            updates.last(),
            Some(ServerMessage::BranchStaging { seats, host_id: 101, .. })
                if seats[0].claimant_id.is_none()
        ));
    }

    #[test]
    fn branch_staging_rejects_normal_lobby_only_controls() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 0);
        let mut task = RoomTask::new(
            "branch-lobby-controls-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer = add_branch_occupant(&mut task, 100);
        task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));
        task.host_id = Some(100);

        task.on_ready(100, true);
        task.on_add_ai(100, None, None);
        task.on_remove_ai(100, 999);
        task.on_set_quickstart(100, true);
        task.on_set_spectator(100, false);
        task.on_select_map(100, "Badlands".to_string());
        task.on_start_request(100);

        assert!(task.ai_players.is_empty());
        assert!(!task.quickstart);
        assert_eq!(task.selected_map, "Default");
        assert!(matches!(task.phase, Phase::BranchStaging(_)));
        assert!(!std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
            .any(|msg| matches!(msg, ServerMessage::Lobby { .. } | ServerMessage::Start(_))));
    }

    #[test]
    fn branch_start_countdown_promotes_to_live_start_payloads() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 3);
        let mut task = RoomTask::new(
            "branch-promote-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_branch_occupant(&mut task, 100);
        let mut writer_b = add_branch_occupant(&mut task, 101);
        let mut writer_spectator = add_branch_occupant(&mut task, 102);
        task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

        task.on_claim_branch_seat(100, players[0].id);
        task.on_claim_branch_seat(101, players[1].id);
        task.on_start_branch(100);

        assert!(std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok())
            .any(|msg| matches!(msg, ServerMessage::MatchCountdown { .. })));
        std::thread::sleep(match_countdown_duration().saturating_mul(2));
        task.on_tick(TokioInstant::now());

        assert!(matches!(task.phase, Phase::InGame(_)));
        assert_eq!(
            task.branch_live_seat_by_connection.get(&100),
            Some(&players[0].id)
        );
        assert_eq!(
            task.branch_live_seat_by_connection.get(&101),
            Some(&players[1].id)
        );
        assert!(!task.branch_live_seat_by_connection.contains_key(&102));
        assert!(!task.players.get(&100).unwrap().spectator);
        assert!(!task.players.get(&101).unwrap().spectator);
        assert!(task.players.get(&102).unwrap().spectator);

        let starts_b: Vec<_> =
            std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()).collect();
        assert!(starts_b.iter().any(|msg| {
            matches!(msg, ServerMessage::Start(payload)
                if payload.player_id == players[1].id
                    && !payload.spectator
                    && payload.replay.is_none()
                    && payload.players.iter().all(|player| player.faction_id == DEFAULT_FACTION_ID))
        }));
        let starts_spectator: Vec<_> =
            std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
        assert!(starts_spectator.iter().any(|msg| {
            matches!(msg, ServerMessage::Start(payload)
                if payload.player_id == 102 && payload.spectator && payload.replay.is_none())
        }));
    }

    #[test]
    fn branch_live_launch_rejects_unsupported_recorded_faction_ids() {
        let players = replay_test_players(2);
        let mut seed = replay_branch_test_seed(&players, 0);
        seed.seats[0].faction_id = EMPTY_FIXTURE_FACTION_ID.to_string();
        let mut task = RoomTask::new(
            "branch-faction-reject-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_branch_occupant(&mut task, 100);
        let mut writer_b = add_branch_occupant(&mut task, 101);
        let mut staging = BranchStagingState::new(seed);
        staging.claim(100, players[0].id).unwrap();
        staging.claim(101, players[1].id).unwrap();
        task.phase = Phase::BranchStaging(Box::new(staging));

        task.start_branch_live();

        assert!(matches!(task.phase, Phase::BranchStaging(_)));
        assert!(task.branch_live_seat_by_connection.is_empty());
        let a_messages: Vec<_> =
            std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).collect();
        let b_messages: Vec<_> =
            std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()).collect();
        assert!(!a_messages
            .iter()
            .chain(b_messages.iter())
            .any(|msg| matches!(msg, ServerMessage::Start(_))));
    }

    #[test]
    fn branch_live_commands_and_snapshots_use_mapped_original_seats() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 0);
        let mut task = RoomTask::new(
            "branch-live-map-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let writer_a = add_branch_occupant(&mut task, 100);
        let writer_b = add_branch_occupant(&mut task, 101);
        let writer_spectator = add_branch_occupant(&mut task, 102);
        let mut staging = BranchStagingState::new(seed);
        staging.claim(100, players[0].id).unwrap();
        staging.claim(101, players[1].id).unwrap();
        task.phase = Phase::BranchStaging(Box::new(staging));
        task.start_branch_live();

        task.on_command(
            100,
            1,
            SimCommand::Stop {
                units: vec![1, 2, 3],
            },
        );
        task.on_command(
            102,
            1,
            SimCommand::Stop {
                units: vec![4, 5, 6],
            },
        );
        task.on_tick(TokioInstant::now());

        let snapshot_a = writer_a.snapshots.take().expect("claimed A snapshot");
        let snapshot_b = writer_b.snapshots.take().expect("claimed B snapshot");
        let snapshot_spectator = writer_spectator
            .snapshots
            .take()
            .expect("spectator snapshot");
        let Phase::InGame(game) = &task.phase else {
            panic!("branch should be live");
        };
        assert_eq!(game.command_log().len(), 1);
        assert_eq!(game.command_log()[0].player_id, players[0].id);
        assert_eq!(
            snapshot_a.visible_tiles,
            game.snapshot_for(players[0].id).visible_tiles
        );
        assert_eq!(
            snapshot_b.visible_tiles,
            game.snapshot_for(players[1].id).visible_tiles
        );
        assert_eq!(
            snapshot_spectator.visible_tiles,
            game.snapshot_for_spectator(&[players[0].id, players[1].id])
                .visible_tiles
        );
    }

    #[test]
    fn branch_live_give_up_resolves_by_original_seat_and_skips_public_history() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 0);
        let mut task = RoomTask::new(
            "branch-give-up-test".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_a = add_branch_occupant(&mut task, 100);
        let _writer_b = add_branch_occupant(&mut task, 101);
        let mut staging = BranchStagingState::new(seed);
        staging.claim(100, players[0].id).unwrap();
        staging.claim(101, players[1].id).unwrap();
        task.phase = Phase::BranchStaging(Box::new(staging));
        task.start_branch_live();

        assert!(!task.should_record_match_history());
        task.on_give_up(100);

        assert!(matches!(task.phase, Phase::ReplayViewer(_)));
        assert!(
            std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).any(|msg| {
                matches!(msg, ServerMessage::GameOver { winner_id: Some(id), you, .. }
                if id == players[1].id && you == "lost")
            })
        );
        assert!(task.branch_live_seat_by_connection.is_empty());
    }

    #[test]
    fn empty_branch_room_drops_frozen_state_and_resets_room_name() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 1);
        let mut task = RoomTask::new(
            "branch-empty-test".to_string(),
            RoomMode::ReplayBranch { seed },
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, _writer) = ConnectionSink::new();
        let (ack_tx, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(100, "Viewer".to_string(), true, false, msg_tx, ack_tx);
        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert_eq!(task.host_id, Some(100));
        assert!(matches!(task.phase, Phase::BranchStaging(_)));

        task.on_leave(100);

        assert!(matches!(task.phase, Phase::Lobby));
        assert!(matches!(task.mode, RoomMode::Normal));
        assert!(task.players.is_empty());
        assert_eq!(task.host_id, None);
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
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Viewer".to_string(), true, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert_eq!(in_game_tick(&task), 0);
        while writer.reliable_rx.try_recv().is_ok() {}

        task.on_set_replay_speed(99, 0.0);
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::ReplayState(state)
                if state.paused && state.speed == 0.0 && state.current_tick == 0
        ));
        task.on_tick(TokioInstant::now());
        assert_eq!(
            in_game_tick(&task),
            0,
            "scheduled ticks should not advance while paused"
        );

        task.on_step_dev_tick(99);
        assert_eq!(in_game_tick(&task), 1);
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::ReplayState(state)
                if state.paused && state.speed == 0.0 && state.current_tick == 1
        ));
        task.on_step_dev_tick(99);
        assert_eq!(in_game_tick(&task), 2);
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::ReplayState(state)
                if state.paused && state.speed == 0.0 && state.current_tick == 2
        ));

        task.on_set_replay_speed(99, 1.0);
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::ReplayState(state)
                if !state.paused && state.speed == 1.0 && state.current_tick == 2
        ));
        task.on_tick(TokioInstant::now());
        assert_eq!(
            in_game_tick(&task),
            3,
            "scheduled ticks should resume after selecting a non-zero speed"
        );
    }

    #[test]
    fn persisted_replay_room_join_prompts_before_playback() {
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

        task.on_join(99, "Viewer".to_string(), false, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(false));
        assert!(matches!(task.phase, Phase::Lobby));
        assert!(!task.players.contains_key(&99));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::JoinReplayPrompt { room } if room == "persisted-replay-test"
        ));
    }

    #[test]
    fn persisted_replay_room_confirmed_join_starts_replay_viewer() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 3);
        let mut task = RoomTask::new(
            "persisted-replay-confirmed-test".to_string(),
            RoomMode::Replay { artifact },
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(99, "Viewer".to_string(), false, true, msg_tx, ack);

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
    fn saved_selfplay_replay_join_uses_replay_viewer_runtime() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 3);
        let artifact_name = format!("room_task_saved_selfplay_{}", std::process::id());
        let artifact_dir = write_selfplay_replay_test_artifact(&artifact_name, &artifact);
        let mut task = RoomTask::new(
            "saved-selfplay-replay-test".to_string(),
            RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay {
                artifact: artifact_name,
            }),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(99, "Viewer".to_string(), false, true, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        let Phase::ReplayViewer(session) = &task.phase else {
            panic!("saved self-play replay should start the shared replay viewer runtime");
        };
        assert_eq!(session.artifact.command_log, artifact.command_log);
        assert_eq!(session.vision_player_ids_for(99), vec![1, 2]);
        assert!(task.players.get(&99).is_some_and(|p| p.spectator));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::Start(payload) if payload.spectator && payload.replay.is_some()
        ));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::ReplayState(_)
        ));

        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    #[test]
    fn post_match_replay_join_prompts_before_attaching_viewer() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 1);
        let replay = ReplaySession::new(artifact).unwrap();
        let mut task = RoomTask::new(
            "post-match-replay-prompt-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        task.phase = Phase::ReplayViewer(Box::new(replay));
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(99, "Viewer".to_string(), false, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(false));
        assert!(!task.players.contains_key(&99));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::JoinReplayPrompt { room } if room == "post-match-replay-prompt-test"
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
    fn replay_viewer_return_detaches_only_requesting_viewer() {
        let players = replay_test_players(2);
        let (game, _artifact) = replay_test_artifact(&players, 1);
        let mut task = RoomTask::new(
            "post-match-lobby-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer_a = add_test_room_player(&mut task, players[0].id, true);
        let writer_b = add_test_room_player(&mut task, players[1].id, true);
        task.match_player_count = 2;
        task.match_human_count = 2;

        task.end_match(Some(players[0].id), game.scores(), Some(&game));
        assert!(matches!(task.phase, Phase::ReplayViewer(_)));

        task.on_return_to_lobby(players[0].id);

        assert!(matches!(task.phase, Phase::ReplayViewer(_)));
        assert!(!task.players.contains_key(&players[0].id));
        assert!(task.players.contains_key(&players[1].id));
        assert_eq!(task.host_id, Some(players[1].id));

        task.on_tick_replay_viewer(TokioInstant::now());
        assert!(
            writer_b.snapshots.take().is_some(),
            "remaining viewers should keep receiving replay snapshots"
        );
    }

    #[test]
    fn replay_viewer_return_resets_room_when_last_viewer_leaves() {
        let players = replay_test_players(2);
        let (game, _artifact) = replay_test_artifact(&players, 1);
        let mut task = RoomTask::new(
            "post-match-empty-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer_a = add_test_room_player(&mut task, players[0].id, true);
        let _writer_b = add_test_room_player(&mut task, players[1].id, true);
        task.match_player_count = 2;
        task.match_human_count = 2;

        task.end_match(Some(players[0].id), game.scores(), Some(&game));
        assert!(matches!(task.phase, Phase::ReplayViewer(_)));

        task.on_return_to_lobby(players[0].id);
        task.on_return_to_lobby(players[1].id);

        assert!(matches!(task.phase, Phase::Lobby));
        assert!(task.players.is_empty());
        assert_eq!(task.host_id, None);
        assert_eq!(task.match_player_count, 0);
        assert_eq!(task.match_human_count, 0);
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

        task.on_join(99, "Viewer A".to_string(), true, true, msg_tx_a, ack_a);
        task.on_join(100, "Viewer B".to_string(), true, true, msg_tx_b, ack_b);

        assert_eq!(ack_rx_a.try_recv(), Ok(true));
        assert_eq!(ack_rx_b.try_recv(), Ok(true));
        assert!(matches!(task.phase, Phase::ReplayViewer(_)));

        task.on_return_to_lobby(99);

        assert!(matches!(task.phase, Phase::ReplayViewer(_)));
        assert!(!task.players.contains_key(&99));
        assert!(task.players.contains_key(&100));

        task.on_tick_replay_viewer(TokioInstant::now());
        assert!(
            writer_b.snapshots.take().is_some(),
            "other viewers should keep receiving replay snapshots"
        );
    }
}
