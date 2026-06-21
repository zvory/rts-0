use super::connection::send_or_log;
use super::crash_replay::{dump_crash_replay, panic_reason};
use super::dev_replay::{load_replay_artifact, match_seed};
use super::faction_validation::{
    default_faction_id_for, validate_faction_request, FactionRequestContext, FactionValidation,
};
use super::launch::{LaunchPrediction, LaunchRecipient};
use super::live_tick::{LiveTickDriver, LiveTickResult};
use super::participants::{CommandIssuer, Participants};
use super::projection::{ObserverAnalysisAudience, ProjectionPolicy, RecipientRole};
use super::replay_branch::{BranchLaunchError, BranchStagingState};
use super::session_policy::{RoomTimeOperation, RoomTimeSource, SessionPhase, SessionPolicy};
use super::snapshot_fanout::{SnapshotFanout, SnapshotFanoutPayload};
use super::tick_control::{RoomTimeClock, RoomTimeSpeed, ScheduledTickAction, TickControl};
use super::*;
use crate::protocol::{
    Command, LabClientOp, LabResult, LabScenarioLabMetadata, LabStartMetadata, LabStartRole,
    LabState, LabVisionMode, LivePauseState, NoticeSeverity, RoomTimeState, DEFAULT_FACTION_ID,
};
#[cfg(test)]
use crate::protocol::{
    MovementPathDiagnosticScope, SnapshotNetStatus, StartPayload, PREDICTION_PROTOCOL_VERSION,
};
use crate::structured_log::{self, MatchEndedLog, MatchStartedLog};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use rts_ai::{AiController, DEFAULT_LIVE_PROFILE_ID};
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{
    LabError, LabMoveEntity, LabOp, LabOpOutcome, LabScenarioV1 as SimLabScenarioV1,
    LabSetCompletedResearch, LabSetEntityOwner, LabSetPlayerResources, LabSpawnEntity,
};
use rts_sim::game::map::Map;
use rts_sim::game::replay::ReplayArtifactV1;
use rts_sim::game::upgrade::UpgradeKind;
use std::str::FromStr;
use std::time::Instant as StdInstant;
use tokio::time::Instant as TokioInstant;

/// A connected player as tracked inside a room.
pub(super) struct RoomPlayer {
    name: String,
    pub(super) color: String,
    ready: bool,
    pub(super) spectator: bool,
    pub(super) msg_tx: ConnectionSink,
    pub(super) head_of_line_count: u32,
    last_received_client_seq: u32,
    pub(super) last_sim_consumed_client_seq: u32,
    pub(super) last_sim_consumed_client_tick: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PendingClientCommandAck {
    pub(super) connection_id: u32,
    pub(super) client_seq: u32,
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
const LAB_PLAYER_ONE_ID: u32 = 1;
const LAB_PLAYER_TWO_ID: u32 = 2;
const LIVE_PAUSE_LIMIT: u8 = 3;

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
        if name.eq_ignore_ascii_case("smoke") {
            return true;
        }
        has_alpha |= name == "Alpha";
        has_bravo |= name == "Bravo";
    }
    has_alpha && has_bravo
}

fn late_spectator_notice_name(name: &str) -> String {
    let cleaned: String = name.trim().chars().filter(|ch| !ch.is_control()).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Commander".to_string()
    } else {
        cleaned.to_string()
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
pub(super) enum Phase {
    Lobby,
    InGame(Box<Game>),
    ReplayViewer(Box<ReplaySession>),
    BranchStaging(Box<BranchStagingState>),
}

#[derive(Clone, Copy)]
struct ReplayTickContext {
    scheduler_lag: Duration,
    tick_budget: Duration,
    tick_start: StdInstant,
    projection_policy: ProjectionPolicy,
}

#[derive(Clone)]
pub(super) enum RoomMode {
    Normal,
    DevScenario(DevScenarioConfig),
    Replay { artifact: ReplayArtifactV1 },
    ReplayArtifact { artifact: String },
    ReplayBranch { seed: ReplayBranchSeed },
    Lab(LabRoomConfig),
}

#[derive(Clone)]
pub(super) struct LabRoomConfig {
    pub(super) public_id: String,
    pub(super) map_name: String,
    pub(super) seed: Option<u32>,
}

#[derive(Clone)]
pub(super) struct DevScenarioConfig {
    pub(super) id: DevScenarioId,
    pub(super) unit: EntityKind,
    pub(super) count: usize,
    pub(super) blocker: Option<EntityKind>,
    pub(super) case: Option<&'static str>,
}

#[derive(Clone)]
pub(super) enum DevScenarioId {
    ScoutCarSnakingCorridor,
    DirectReverseOrder,
    ScoutCarWallChokepoint,
    VehicleCornerWall,
    VehicleSmallBlockBaseline,
    FactoryZeroGapPerpendicular,
    TankTrapLineHorizontal,
    TankTrapLineVertical,
    TankTrapLineDiagonal,
    TankTrapPathingMatrix,
}

enum DevDriver {
    Scenario(DevScenarioDriver),
}

impl DevDriver {
    fn enqueue_for_tick(&mut self, game: &mut Game) {
        match self {
            DevDriver::Scenario(scenario) => scenario.enqueue_for_tick(game),
        }
    }
}

struct LabSession {
    public_id: String,
    operator_id: u32,
    viewer_roles: HashMap<u32, LabStartRole>,
    dirty: bool,
    operation_log: Vec<LabOperationLogEntry>,
    vision_mode: LabVisionMode,
    view_player_id: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct LabOperationLogEntry {
    tick: u32,
    request_id: u32,
    operator_id: u32,
    op: String,
    result: String,
}

impl LabSession {
    fn new(config: &LabRoomConfig, operator_id: u32) -> Self {
        let mut viewer_roles = HashMap::new();
        viewer_roles.insert(operator_id, LabStartRole::Operator);
        Self {
            public_id: config.public_id.clone(),
            operator_id,
            viewer_roles,
            dirty: false,
            operation_log: Vec::new(),
            vision_mode: LabVisionMode::FullWorld,
            view_player_id: LAB_PLAYER_ONE_ID,
        }
    }

    fn add_viewer(&mut self, player_id: u32) {
        self.viewer_roles.insert(player_id, LabStartRole::Operator);
    }

    fn remove_viewer(&mut self, player_id: u32) {
        self.viewer_roles.remove(&player_id);
    }

    fn role_for(&self, player_id: u32) -> LabStartRole {
        self.viewer_roles
            .get(&player_id)
            .copied()
            .unwrap_or(LabStartRole::ReadOnly)
    }

    fn can_operate(&self, player_id: u32) -> bool {
        matches!(self.role_for(player_id), LabStartRole::Operator)
    }

    fn metadata_for(&self, player_id: u32) -> LabStartMetadata {
        LabStartMetadata {
            room: self.public_id.clone(),
            operator_id: self.operator_id,
            role: self.role_for(player_id),
            vision: self.vision_mode.clone(),
            dirty: self.dirty,
            operation_count: self.operation_log.len() as u32,
        }
    }

    fn state_for(&self, player_id: u32) -> LabState {
        LabState {
            room: self.public_id.clone(),
            operator_id: self.operator_id,
            role: self.role_for(player_id),
            vision: self.vision_mode.clone(),
            dirty: self.dirty,
            operation_count: self.operation_log.len() as u32,
        }
    }
}

fn players_on_teams(game: &Game, team_ids: impl IntoIterator<Item = TeamId>) -> Vec<u32> {
    let teams: HashSet<_> = team_ids.into_iter().collect();
    game.start_payload()
        .players
        .into_iter()
        .filter(|player| teams.contains(&player.team_id))
        .map(|player| player.id)
        .collect()
}

fn lab_op_kind(op: &LabClientOp) -> &'static str {
    match op {
        LabClientOp::ExportScenario { .. } => "exportScenario",
        LabClientOp::ImportScenario { .. } => "importScenario",
        LabClientOp::SpawnEntity { .. } => "spawnEntity",
        LabClientOp::DeleteEntity { .. } => "deleteEntity",
        LabClientOp::MoveEntity { .. } => "moveEntity",
        LabClientOp::SetEntityOwner { .. } => "setEntityOwner",
        LabClientOp::SetPlayerResources { .. } => "setPlayerResources",
        LabClientOp::SetCompletedResearch { .. } => "setCompletedResearch",
        LabClientOp::SetVision { .. } => "setVision",
        LabClientOp::IssueCommandAs { .. } => "issueCommandAs",
    }
}

fn lab_client_op_to_game_op(op: LabClientOp) -> Result<LabOp, String> {
    match op {
        LabClientOp::ImportScenario { scenario } => {
            validate_lab_scenario_vision(&scenario.metadata.lab.vision, &scenario.players)?;
            let scenario: SimLabScenarioV1 = serde_json::from_value(
                serde_json::to_value(scenario)
                    .map_err(|err| format!("invalid scenario payload: {err}"))?,
            )
            .map_err(|err| format!("invalid scenario payload: {err}"))?;
            Ok(LabOp::RestoreScenario(Box::new(scenario)))
        }
        LabClientOp::SpawnEntity {
            owner,
            kind,
            x,
            y,
            completed,
        } => {
            let kind =
                EntityKind::from_str(&kind).map_err(|_| "unknown entity kind".to_string())?;
            Ok(LabOp::SpawnEntity(LabSpawnEntity {
                owner,
                kind,
                x,
                y,
                completed,
            }))
        }
        LabClientOp::DeleteEntity { entity_id } => Ok(LabOp::DeleteEntity { entity_id }),
        LabClientOp::MoveEntity { entity_id, x, y } => {
            Ok(LabOp::MoveEntity(LabMoveEntity { entity_id, x, y }))
        }
        LabClientOp::SetEntityOwner { entity_id, owner } => {
            Ok(LabOp::SetEntityOwner(LabSetEntityOwner {
                entity_id,
                owner,
            }))
        }
        LabClientOp::SetPlayerResources {
            player_id,
            steel,
            oil,
        } => Ok(LabOp::SetPlayerResources(LabSetPlayerResources {
            player_id,
            steel,
            oil,
        })),
        LabClientOp::SetCompletedResearch {
            player_id,
            upgrade,
            completed,
        } => {
            let upgrade =
                UpgradeKind::from_str(&upgrade).map_err(|_| "unknown research id".to_string())?;
            Ok(LabOp::SetCompletedResearch(LabSetCompletedResearch {
                player_id,
                upgrade,
                completed,
            }))
        }
        LabClientOp::ExportScenario { .. }
        | LabClientOp::SetVision { .. }
        | LabClientOp::IssueCommandAs { .. } => Err("not a lab mutation".to_string()),
    }
}

fn validate_lab_vision(game: &Game, vision: &LabVisionMode) -> Result<(), String> {
    let players = game.start_payload().players;
    match vision {
        LabVisionMode::FullWorld => Ok(()),
        LabVisionMode::Team { team_id } => {
            if players.iter().any(|player| player.team_id == *team_id) {
                Ok(())
            } else {
                Err("unknown lab team id".to_string())
            }
        }
        LabVisionMode::Teams { team_ids } => {
            if team_ids.is_empty() {
                return Err("teamIds must not be empty".to_string());
            }
            let mut seen = HashSet::new();
            for team_id in team_ids {
                if !seen.insert(*team_id) {
                    return Err("teamIds must not contain duplicates".to_string());
                }
                if !players.iter().any(|player| player.team_id == *team_id) {
                    return Err("unknown lab team id".to_string());
                }
            }
            Ok(())
        }
    }
}

fn truncate_lab_scenario_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if out.len() + ch.len_utf8() > 80 {
            break;
        }
        out.push(ch);
    }
    out
}

fn validate_lab_scenario_vision(
    vision: &LabVisionMode,
    players: &[crate::protocol::LabScenarioPlayer],
) -> Result<(), String> {
    match vision {
        LabVisionMode::FullWorld => Ok(()),
        LabVisionMode::Team { team_id } => {
            if players.iter().any(|player| player.team_id == *team_id) {
                Ok(())
            } else {
                Err("unknown scenario lab team id".to_string())
            }
        }
        LabVisionMode::Teams { team_ids } => {
            if team_ids.is_empty() {
                return Err("teamIds must not be empty".to_string());
            }
            let mut seen = HashSet::new();
            for team_id in team_ids {
                if !seen.insert(*team_id) {
                    return Err("teamIds must not contain duplicates".to_string());
                }
                if !players.iter().any(|player| player.team_id == *team_id) {
                    return Err("unknown scenario lab team id".to_string());
                }
            }
            Ok(())
        }
    }
}

fn lab_result_error(request_id: u32, op: String, error: &str) -> LabResult {
    LabResult {
        request_id,
        ok: false,
        op,
        error: Some(error.to_string()),
        outcome: None,
    }
}

fn lab_error_text(err: &LabError) -> String {
    match err {
        LabError::StaleEntity { entity_id } => format!("stale entity id {entity_id}"),
        LabError::InvalidKind { kind, operation } => {
            format!("invalid kind {kind:?} for {operation}")
        }
        LabError::InvalidPlayer { player_id } => format!("invalid player id {player_id}"),
        LabError::InvalidOwner { owner } => format!("invalid owner id {owner}"),
        LabError::InvalidPosition { x, y, reason } => {
            format!("invalid position ({x}, {y}): {reason}")
        }
        LabError::OccupiedPosition { x, y } => format!("occupied position ({x}, {y})"),
        LabError::InvalidResearch { player_id, upgrade } => {
            format!("invalid research {upgrade:?} for player {player_id}")
        }
        LabError::InvalidScenarioVersion { version } => {
            format!("unsupported scenario version {version}")
        }
        LabError::InvalidScenario { reason } => reason.clone(),
        LabError::InvalidMap { name, reason } => format!("invalid map {name:?}: {reason}"),
        LabError::InvalidCommand { reason } => reason.clone(),
    }
}

fn lab_outcome_json(outcome: &LabOpOutcome) -> serde_json::Value {
    match outcome {
        LabOpOutcome::Spawned { entity_id } => serde_json::json!({ "entityId": entity_id }),
        LabOpOutcome::Deleted { entity_id } => serde_json::json!({ "entityId": entity_id }),
        LabOpOutcome::Moved { entity_id, x, y } => {
            serde_json::json!({ "entityId": entity_id, "x": x, "y": y })
        }
        LabOpOutcome::OwnerSet { entity_id, owner } => {
            serde_json::json!({ "entityId": entity_id, "owner": owner })
        }
        LabOpOutcome::PlayerResourcesSet {
            player_id,
            steel,
            oil,
        } => serde_json::json!({ "playerId": player_id, "steel": steel, "oil": oil }),
        LabOpOutcome::CompletedResearchSet {
            player_id,
            upgrade,
            completed,
        } => serde_json::json!({
            "playerId": player_id,
            "upgrade": upgrade.to_protocol_str(),
            "completed": completed
        }),
        LabOpOutcome::ScenarioRestored(restore) => serde_json::to_value(restore)
            .unwrap_or_else(|_| serde_json::json!({ "scenarioRestored": true })),
    }
}

struct DevScenarioDriver {
    player_id: u32,
    units: Vec<u32>,
    goal: (f32, f32),
    issue_after_ticks: u32,
    issued: bool,
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
    /// Wall-clock creation/reset time for the public lobby browser age column.
    created_at_unix_ms: u64,
    /// Empty lobbies created through POST /api/lobbies are briefly reserved for the creating
    /// client's follow-up WebSocket join. If that join never arrives, the name can be reclaimed.
    empty_lobby_reserved_until_unix_ms: Option<u64>,
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
    /// Number of human (non-AI) players the in-progress match started with. `0` outside a match.
    match_human_count: usize,
    /// Connected human players who already received a terminal score screen for the active match.
    outcome_sent: HashSet<u32>,
    /// In replay branch live matches, connected ids differ from original replay player ids.
    branch_live_seat_by_connection: HashMap<u32, u32>,
    /// Live-match pause is room-owned control-plane state, separate from replay/dev room-time.
    live_paused: bool,
    live_paused_by: Option<u32>,
    live_pause_counts: HashMap<u32, u8>,
    lab_session: Option<LabSession>,
    dev_driver: Option<DevDriver>,
    dev_view_player_id: Option<u32>,
    ai_controllers: Vec<AiController>,
    /// Room-time speed multiplier; 1.0 = real-time, 2.0 = 2x faster, etc.
    room_time_speed: f32,
    /// Room-time pause flag. Kept separate from room_time_speed so interval creation never divides
    /// by zero and resume can restore the previous non-zero multiplier.
    room_time_paused: bool,
    slow_tick_count: u32,
    pending_client_command_acks: Vec<PendingClientCommandAck>,
    /// Recipient-specific room-owned notices appended to the next live snapshot for each
    /// connection id. Used when the notice is about room membership rather than sim events.
    pending_recipient_notices: HashMap<u32, Vec<Event>>,
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
            created_at_unix_ms: current_unix_ms(),
            empty_lobby_reserved_until_unix_ms: None,
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
            live_paused: false,
            live_paused_by: None,
            live_pause_counts: HashMap::new(),
            lab_session: None,
            dev_driver: None,
            dev_view_player_id: None,
            ai_controllers: Vec::new(),
            room_time_speed: 1.0,
            room_time_paused: false,
            slow_tick_count: 0,
            pending_client_command_acks: Vec::new(),
            pending_recipient_notices: HashMap::new(),
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
        self.tick_control().tick_interval(base)
    }

    fn current_speed_multiplier(&self) -> f32 {
        self.tick_control().speed_multiplier()
    }

    fn tick_control(&self) -> TickControl {
        let policy = self.session_policy();
        let room_time = match (&self.phase, policy.clock.room_time_source()) {
            (Phase::ReplayViewer(session), Some(RoomTimeSource::ReplayPlayback)) => {
                Some(RoomTimeClock {
                    speed: session.speed(),
                    paused: session.is_paused(),
                })
            }
            (_, Some(RoomTimeSource::DevScenario)) => Some(RoomTimeClock {
                speed: self.room_time_speed,
                paused: self.room_time_paused,
            }),
            _ => None,
        };
        TickControl::new(
            policy.clock,
            room_time,
            self.room_time_speed,
            self.match_countdown_deadline.is_some(),
        )
    }

    fn session_phase(&self) -> SessionPhase {
        match &self.phase {
            Phase::Lobby => SessionPhase::Lobby,
            Phase::InGame(_) => SessionPhase::LiveMatch,
            Phase::ReplayViewer(_) => SessionPhase::ReplayViewer,
            Phase::BranchStaging(_) => SessionPhase::BranchStaging,
        }
    }

    fn session_policy(&self) -> SessionPolicy {
        SessionPolicy::for_room(&self.mode, self.session_phase())
    }

    fn projection_policy(&self) -> ProjectionPolicy {
        self.projection_policy_for_phase(self.session_phase())
    }

    fn projection_policy_for_phase(&self, phase: SessionPhase) -> ProjectionPolicy {
        let policy = self.session_policy();
        let policy = if policy.phase == phase {
            policy
        } else {
            SessionPolicy::for_room(&self.mode, phase)
        };
        let projection = ProjectionPolicy::new(policy.visibility, policy.diagnostics);
        if self.owner_movement_diagnostics_enabled_for_phase(phase) {
            projection.with_owner_movement_paths()
        } else {
            projection
        }
    }

    fn owner_movement_diagnostics_enabled_for_phase(&self, phase: SessionPhase) -> bool {
        matches!(self.mode, RoomMode::Normal) && phase == SessionPhase::LiveMatch && self.quickstart
    }

    fn is_dev_watch(&self) -> bool {
        self.session_policy().is_dev_watch()
    }

    fn should_persist_match_history(&self) -> bool {
        let match_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        self.match_player_count >= 1
            && match_policy.has_authoritative_mutation()
            && match_policy.allows_match_history()
            && !is_automated_match_history_room(&self.room)
            && !match_history_participants_are_automated(&self.match_participants)
    }

    fn should_capture_post_match_replay(&self) -> bool {
        let match_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        match_policy.captures_post_match_replay()
    }

    fn should_attach_match_history_replay_artifact(&self) -> bool {
        let match_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        match_policy.attaches_match_history_replay_artifact()
    }

    // -- Event handling ------------------------------------------------------

    fn handle_event(&mut self, event: RoomEvent) {
        match event {
            RoomEvent::Summary { reply } => {
                let _ = reply.send(self.lobby_summary());
            }
            RoomEvent::ReserveEmptyPublicLobby { reply } => {
                let _ = reply.send(self.try_reserve_empty_public_lobby_name());
            }
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
                target,
                spectator,
            } => self.on_set_spectator(player_id, target, spectator),
            RoomEvent::Command {
                player_id,
                client_seq,
                cmd,
            } => self.on_command(player_id, client_seq, cmd),
            RoomEvent::GiveUp { player_id } => self.on_give_up(player_id),
            RoomEvent::PauseGame { player_id } => self.on_pause_game(player_id),
            RoomEvent::UnpauseGame { player_id } => self.on_unpause_game(player_id),
            RoomEvent::ReturnToLobby { player_id } => self.on_return_to_lobby(player_id),
            RoomEvent::SetRoomTimeSpeed { player_id, speed } => {
                self.on_set_room_time_speed(player_id, speed)
            }
            RoomEvent::StepRoomTime { player_id } => self.on_step_room_time(player_id),
            RoomEvent::SeekRoomTime {
                player_id,
                ticks_back,
            } => self.on_seek_room_time(player_id, ticks_back),
            RoomEvent::SeekRoomTimeTo { player_id, tick } => {
                self.on_seek_room_time_to(player_id, tick)
            }
            RoomEvent::SetReplayVision { player_id, vision } => {
                self.on_set_replay_vision(player_id, vision)
            }
            RoomEvent::Lab {
                player_id,
                request_id,
                op,
            } => self.on_lab_request(player_id, request_id, op),
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

    pub(super) fn lobby_summary(&self) -> Option<LobbySummary> {
        let policy = self.session_policy();
        if !policy.is_public_lobby_browser_room() {
            return None;
        }
        let host_id = self.host_id?;
        let host_name = self
            .players
            .get(&host_id)
            .map(|player| player.name.clone())?;
        let (phase, join_state, map) = if self.match_countdown_deadline.is_some() {
            (
                LobbySummaryPhase::Countdown,
                LobbyJoinState::Starting,
                self.selected_map.clone(),
            )
        } else {
            match &self.phase {
                Phase::Lobby => {
                    let join_state = if self.total_player_count() >= MAX_PLAYERS {
                        LobbyJoinState::FullSpectatorOnly
                    } else {
                        LobbyJoinState::Open
                    };
                    (
                        LobbySummaryPhase::Lobby,
                        join_state,
                        self.selected_map.clone(),
                    )
                }
                Phase::InGame(_) => (
                    LobbySummaryPhase::InGame,
                    LobbyJoinState::InGame,
                    self.match_map_name.clone(),
                ),
                Phase::ReplayViewer(_) | Phase::BranchStaging(_) => return None,
            }
        };
        Some(LobbySummary {
            room: self.room.clone(),
            host_name: Some(host_name),
            map,
            created_at_unix_ms: self.created_at_unix_ms,
            occupied_slots: self.total_player_count(),
            max_slots: MAX_PLAYERS,
            spectator_count: self
                .players
                .values()
                .filter(|player| player.spectator)
                .count(),
            phase,
            join_state,
        })
    }

    pub(super) fn reserve_empty_public_lobby_name(&mut self) {
        let now = current_unix_ms();
        self.created_at_unix_ms = now;
        self.empty_lobby_reserved_until_unix_ms =
            Some(now.saturating_add(EMPTY_LOBBY_RESERVATION_TTL_MS));
    }

    fn try_reserve_empty_public_lobby_name(&mut self) -> bool {
        if !self.empty_public_lobby_name_is_reusable() {
            return false;
        }
        self.reserve_empty_public_lobby_name();
        true
    }

    fn empty_public_lobby_name_is_reusable(&self) -> bool {
        matches!(self.mode, RoomMode::Normal)
            && matches!(self.phase, Phase::Lobby)
            && self.players.is_empty()
            && self.ai_players.is_empty()
            && self.match_countdown_deadline.is_none()
            && !self.empty_lobby_reservation_is_active(current_unix_ms())
    }

    fn empty_lobby_reservation_is_active(&self, now_unix_ms: u64) -> bool {
        self.empty_lobby_reserved_until_unix_ms
            .is_some_and(|reserved_until| reserved_until > now_unix_ms)
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
        let policy = self.session_policy();
        if policy.is_dev_watch() {
            self.on_join_dev_watch(player_id, name, msg_tx, ack);
            return;
        }
        if policy.uses_replay_room_join() {
            if !replay_ok {
                self.prompt_for_replay_join(player_id, &msg_tx, ack);
                return;
            }
            self.on_join_replay_room(player_id, name, msg_tx, ack);
            return;
        }
        if policy.uses_branch_room_join() {
            self.on_join_branch_staging(player_id, name, msg_tx, ack);
            return;
        }
        if policy.uses_lab_room_join() {
            self.on_join_lab(player_id, name, msg_tx, ack);
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
            if policy.allows_live_spectator_attach() {
                self.on_join_live_spectator(player_id, name, spectator, msg_tx, ack);
                return;
            }
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
        self.empty_lobby_reserved_until_unix_ms = None;
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

    fn on_join_live_spectator(
        &mut self,
        player_id: u32,
        name: String,
        spectator: bool,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if !spectator {
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: "Match already in progress in this room — join as a spectator or try another room."
                        .to_string(),
                },
            );
            crate::log_debug!(room = %self.room, player_id, "rejecting active join; match in progress");
            let _ = ack.send(false);
            return;
        }

        let mut payload = match &self.phase {
            Phase::InGame(game) => game.start_payload(),
            _ => {
                send_or_log(
                    &self.room,
                    player_id,
                    &msg_tx,
                    ServerMessage::Error {
                        msg: "Match already in progress in this room — try another room."
                            .to_string(),
                    },
                );
                crate::log_debug!(room = %self.room, player_id, "rejecting spectator join; no live match payload");
                let _ = ack.send(false);
                return;
            }
        };
        payload.match_run_id = self.match_run_id.clone();

        let notice_recipients = self.late_spectator_notice_recipient_ids();
        let notice_name = late_spectator_notice_name(&name);

        self.insert_human_spectator(player_id, name, msg_tx);
        crate::log_debug!(room = %self.room, player_id, "joined live match as spectator");
        let _ = ack.send(true);
        self.send_current_shutdown_warning_to(player_id);
        self.enqueue_late_spectator_join_notice(notice_recipients, notice_name);

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        super::launch::send_start_payloads(
            &self.room,
            &payload,
            [LaunchRecipient {
                connection_id: player_id,
                payload_player_id: player_id,
                spectator: true,
                prediction: LaunchPrediction::Disabled,
                capabilities: start_policy.start_capabilities(false),
                diagnostics: projection_policy
                    .diagnostic_capabilities_for(RecipientRole::Spectator),
                clear_pending_snapshot: true,
                lab: None,
                msg_tx: player.msg_tx.clone(),
            }],
        );
        if self.live_pause_controls_available() {
            self.send_live_pause_state_to(player_id);
        }
    }

    fn insert_human_spectator(&mut self, player_id: u32, name: String, msg_tx: ConnectionSink) {
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
    }

    fn late_spectator_notice_recipient_ids(&self) -> Vec<u32> {
        self.order
            .iter()
            .copied()
            .filter(|id| self.players.contains_key(id))
            .collect()
    }

    fn enqueue_late_spectator_join_notice(&mut self, recipients: Vec<u32>, spectator_name: String) {
        if recipients.is_empty() {
            return;
        }
        let notice = Event::Notice {
            msg: format!("{spectator_name} has joined the match as a spectator"),
            severity: NoticeSeverity::Info,
            x: None,
            y: None,
        };
        for id in recipients {
            self.pending_recipient_notices
                .entry(id)
                .or_default()
                .push(notice.clone());
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
        self.pending_recipient_notices.remove(&player_id);
        if let Some(session) = &mut self.lab_session {
            session.remove_viewer(player_id);
        }
        self.outcome_sent.remove(&player_id);
        self.reassign_host_if_needed();
        crate::log_debug!(room = %self.room, player_id, "left");

        // If the room emptied out, fully reset it to a clean lobby so its name is never stuck
        // mid-match (otherwise a 1-player sandbox — which never "ends" — would poison the room
        // for the next person who joins under the same name). The idle room task lives on cheaply.
        if self.players.is_empty() {
            self.mark_match_finished_for_drain();
            self.reset_empty_room_state();
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
                session.remove_viewer(player_id);
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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
        if self.is_dev_watch() {
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

    fn on_set_spectator(&mut self, player_id: u32, target: u32, spectator: bool) {
        if self.is_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) {
            return;
        }
        if target != player_id && self.host_id != Some(player_id) {
            crate::log_debug!(
                room = %self.room,
                player_id,
                target,
                "ignoring non-host spectator assignment"
            );
            return;
        }
        let current = self.players.get(&target).map(|p| p.spectator);
        if current == Some(spectator) || current.is_none() {
            return;
        }
        if spectator {
            if let Some(player) = self.players.get_mut(&target) {
                player.spectator = true;
                player.ready = false;
                player.color = "#6f8fa8".to_string();
            }
            self.human_team_assignments.remove(&target);
            self.human_faction_assignments.remove(&target);
        } else {
            if self.total_player_count() >= MAX_PLAYERS {
                crate::log_debug!(room = %self.room, player_id, target, "ignoring player role switch; room full");
                return;
            }
            let color = self.next_human_color();
            if let Some(player) = self.players.get_mut(&target) {
                player.spectator = false;
                player.ready = false;
                player.color = color;
            }
            self.assign_missing_team_for(target);
            self.assign_missing_faction_for(target);
        }
        self.broadcast_lobby();
    }

    /// Total seated players: connected humans plus AI opponents.
    fn total_player_count(&self) -> usize {
        self.active_human_count() + self.ai_players.len()
    }

    fn active_human_count(&self) -> usize {
        self.participants().active_human_count()
    }

    fn active_human_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.participants().active_human_ids().into_iter()
    }

    fn active_seat_ids(&self) -> Vec<u32> {
        self.participants()
            .active_seat_ids(self.ai_players.iter().map(|ai| ai.id))
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
        let counts = self.team_counts_except(Some(new_id));
        if let Some(next_after_occupied) = counts
            .keys()
            .copied()
            .filter(|team_id| (1..=MAX_LOBBY_TEAMS).contains(team_id))
            .max()
            .and_then(|team_id| team_id.checked_add(1))
            .filter(|team_id| *team_id <= MAX_LOBBY_TEAMS && !counts.contains_key(team_id))
        {
            return next_after_occupied;
        }
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
        self.participants()
            .spectator_visible_player_ids(self.ai_players.iter().map(|ai| ai.id))
    }

    fn live_seat_id_for_connection(&self, connection_id: u32) -> Option<u32> {
        self.participants()
            .live_seat_id_for_connection(connection_id)
    }

    fn live_connection_is_player(&self, connection_id: u32) -> bool {
        self.participants().live_connection_is_player(connection_id)
    }

    fn command_issuer_for_connection(&self, connection_id: u32) -> Option<CommandIssuer> {
        self.participants()
            .command_issuer_for_connection(connection_id, &self.outcome_sent)
    }

    fn reassign_host_if_needed(&mut self) {
        self.host_id = self.participants().host_with_fallback(self.host_id);
    }

    fn participants(&self) -> Participants<'_> {
        Participants::new(
            &self.order,
            &self.players,
            &self.branch_live_seat_by_connection,
        )
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
        if self.is_dev_watch() {
            return;
        }
        if client_seq == 0 {
            crate::log_debug!(room = %self.room, player_id, "ignoring command with reserved clientSeq 0");
            self.send_command_receipt(player_id, client_seq, 0, false, Some("invalidSeq"));
            return;
        }
        let issuer = self.command_issuer_for_connection(player_id);
        // Commands are ignored unless in-game and the sender is actually in this room. The
        // simulation itself validates ownership/affordability when it applies the command.
        let receipt = if let Phase::InGame(game) = &mut self.phase {
            let server_tick = game.current_tick();
            if let Some(issuer) = issuer {
                if let Some(player) = self.players.get_mut(&player_id) {
                    if client_seq <= player.last_received_client_seq {
                        crate::log_debug!(
                            room = %self.room,
                            player_id,
                            client_seq,
                            last_received = player.last_received_client_seq,
                            "ignoring stale or wrapped command sequence"
                        );
                        (server_tick, false, Some("staleSeq"))
                    } else {
                        player.last_received_client_seq = client_seq;
                        game.enqueue(issuer.seat_id, cmd);
                        self.pending_client_command_acks
                            .push(PendingClientCommandAck {
                                connection_id: issuer.connection_id,
                                client_seq,
                            });
                        (server_tick, true, None)
                    }
                } else {
                    (server_tick, false, Some("notJoined"))
                }
            } else {
                (server_tick, false, Some("notPlayer"))
            }
        } else {
            (0, false, Some("notInGame"))
        };
        let (server_tick, accepted, reason) = receipt;
        self.send_command_receipt(player_id, client_seq, server_tick, accepted, reason);
    }

    fn send_command_receipt(
        &self,
        player_id: u32,
        client_seq: u32,
        server_tick: u32,
        accepted: bool,
        reason: Option<&str>,
    ) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::CommandReceipt {
                client_seq,
                server_tick,
                accepted,
                reason: reason.map(str::to_string),
            },
        );
    }

    fn live_pause_controls_available(&self) -> bool {
        self.session_policy()
            .start_capabilities(true)
            .match_controls
            .pause
    }

    fn live_pause_seat_for_connection(&self, connection_id: u32) -> Option<u32> {
        if !matches!(self.phase, Phase::InGame(_)) || !self.live_pause_controls_available() {
            return None;
        }
        if self.outcome_sent.contains(&connection_id) {
            return None;
        }
        self.live_connection_is_player(connection_id).then(|| {
            self.live_seat_id_for_connection(connection_id)
                .unwrap_or(connection_id)
        })
    }

    fn live_pause_state_for(&self, connection_id: u32) -> LivePauseState {
        let seat_id = self.live_pause_seat_for_connection(connection_id);
        let pauses_remaining = seat_id.map(|seat_id| {
            LIVE_PAUSE_LIMIT
                .saturating_sub(self.live_pause_counts.get(&seat_id).copied().unwrap_or(0))
        });
        let can_pause = pauses_remaining
            .map(|remaining| !self.live_paused && remaining > 0)
            .unwrap_or(false);
        LivePauseState {
            paused: self.live_paused,
            paused_by: self.live_paused_by,
            pauses_remaining,
            pause_limit: LIVE_PAUSE_LIMIT,
            can_pause,
            can_unpause: self.live_paused && seat_id.is_some(),
        }
    }

    fn send_live_pause_state_to(&self, connection_id: u32) {
        let Some(player) = self.players.get(&connection_id) else {
            return;
        };
        send_or_log(
            &self.room,
            connection_id,
            &player.msg_tx,
            ServerMessage::LivePauseState(self.live_pause_state_for(connection_id)),
        );
    }

    fn broadcast_live_pause_state(&self) {
        if !matches!(self.phase, Phase::InGame(_)) || !self.live_pause_controls_available() {
            return;
        }
        for &connection_id in &self.order {
            self.send_live_pause_state_to(connection_id);
        }
    }

    fn on_pause_game(&mut self, player_id: u32) {
        let Some(seat_id) = self.live_pause_seat_for_connection(player_id) else {
            self.send_live_pause_state_to(player_id);
            return;
        };
        if self.live_paused {
            self.send_live_pause_state_to(player_id);
            return;
        }
        let used = self.live_pause_counts.get(&seat_id).copied().unwrap_or(0);
        if used >= LIVE_PAUSE_LIMIT {
            self.send_live_pause_state_to(player_id);
            return;
        }
        self.live_pause_counts
            .insert(seat_id, used.saturating_add(1));
        self.live_paused = true;
        self.live_paused_by = Some(seat_id);
        crate::log_info!(room = %self.room, player_id, seat_id, "live match paused");
        self.broadcast_live_pause_state();
    }

    fn on_unpause_game(&mut self, player_id: u32) {
        if self.live_pause_seat_for_connection(player_id).is_none() {
            self.send_live_pause_state_to(player_id);
            return;
        }
        if !self.live_paused {
            self.send_live_pause_state_to(player_id);
            return;
        }
        self.live_paused = false;
        self.live_paused_by = None;
        crate::log_info!(room = %self.room, player_id, "live match unpaused");
        self.broadcast_live_pause_state();
    }

    fn on_give_up(&mut self, player_id: u32) {
        if self.is_dev_watch() {
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
            self.end_match(None, scores, Some(&game));
        } else {
            self.phase = Phase::InGame(game);
        }
    }

    // -- Lobby phase ---------------------------------------------------------

    fn on_join_dev_watch(
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
        self.send_room_time_state_to(player_id);
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
                self.send_room_time_state_to(player_id);
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

    fn on_join_lab(
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
        if !matches!(self.phase, Phase::Lobby | Phase::InGame(_)) {
            let _ = ack.send(false);
            return;
        }
        let config = match &self.mode {
            RoomMode::Lab(config) => config.clone(),
            _ => {
                let _ = ack.send(false);
                return;
            }
        };
        if self.lab_session.is_none() {
            self.lab_session = Some(LabSession::new(&config, player_id));
        } else if let Some(session) = &mut self.lab_session {
            session.add_viewer(player_id);
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

        if matches!(self.phase, Phase::Lobby) {
            self.start_lab_session();
        } else {
            self.send_lab_start_to(player_id);
        }
    }

    fn replay_session_for_mode(&self) -> Result<ReplaySession, String> {
        let artifact = match &self.mode {
            RoomMode::Replay { artifact } => artifact.clone(),
            RoomMode::ReplayArtifact { artifact } => load_replay_artifact(artifact)?,
            RoomMode::ReplayBranch { .. } => {
                return Err("room is not configured for replay playback".to_string());
            }
            RoomMode::Lab(_) => {
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
        !self.session_policy().countdown_eligible
            || self.quickstart
            || self.total_player_count() <= 1
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
        self.prepare_live_match_launch();
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
        let match_player_count = inits.len();
        let match_human_count = inits.iter().filter(|p| !p.is_ai).count();
        let match_map_name = self.selected_map.clone();
        let match_participants = inits.iter().map(|p| p.name.clone()).collect();
        self.record_live_match_started(
            match_player_count,
            match_human_count,
            match_map_name,
            match_participants,
        );
        let mut payload = game.start_payload();
        payload.match_run_id = self.match_run_id.clone();
        self.ai_controllers = live_ai_controllers(&inits, &self.ai_players, seed);

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        let recipients: Vec<_> = self
            .order
            .iter()
            .filter_map(|&id| {
                let role = self.players.get(&id).map(|player| {
                    if player.spectator {
                        RecipientRole::Spectator
                    } else {
                        RecipientRole::ActivePlayer
                    }
                })?;
                self.players.get(&id).map(|player| LaunchRecipient {
                    connection_id: id,
                    payload_player_id: id,
                    spectator: player.spectator,
                    prediction: if player.spectator {
                        LaunchPrediction::Disabled
                    } else {
                        LaunchPrediction::Enabled
                    },
                    capabilities: start_policy.start_capabilities(!player.spectator),
                    diagnostics: projection_policy.diagnostic_capabilities_for(role),
                    clear_pending_snapshot: false,
                    lab: None,
                    msg_tx: player.msg_tx.clone(),
                })
            })
            .collect();
        super::launch::send_start_payloads(&self.room, &payload, recipients);

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
        self.broadcast_live_pause_state();
    }

    fn start_branch_live(&mut self) {
        self.prepare_live_match_launch();
        let staging = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::BranchStaging(staging) => staging,
            other => {
                self.phase = other;
                return;
            }
        };
        let launch = match staging
            .prepare_launch(|connection_id| self.players.contains_key(&connection_id))
        {
            Ok(launch) => launch,
            Err(BranchLaunchError::UnsupportedFaction {
                seat_player_id,
                requested,
                reason,
            }) => {
                crate::log_warn!(
                    room = %self.room,
                    seat_player_id,
                    faction_id = ?requested,
                    reason = ?reason,
                    "replay branch seat rejected by faction policy"
                );
                self.phase = Phase::BranchStaging(staging);
                self.broadcast_branch_staging();
                return;
            }
            Err(BranchLaunchError::NotReady | BranchLaunchError::MissingOccupant) => {
                self.phase = Phase::BranchStaging(staging);
                self.broadcast_branch_staging();
                return;
            }
        };

        let game = launch.game;
        self.branch_live_seat_by_connection = launch.seat_by_connection;
        self.record_live_match_started(
            launch.match_player_count,
            launch.match_player_count,
            launch.map_name,
            launch.participants,
        );
        let mut payload = game.start_payload();
        payload.match_run_id = self.match_run_id.clone();
        self.ai_controllers.clear();
        self.dev_driver = None;
        self.dev_view_player_id = None;

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        let mut recipients = Vec::new();
        for &connection_id in &self.order {
            let Some(player) = self.players.get_mut(&connection_id) else {
                continue;
            };
            let mapped_seat = self
                .branch_live_seat_by_connection
                .get(&connection_id)
                .copied();
            let role = if mapped_seat.is_some() {
                RecipientRole::ActivePlayer
            } else {
                RecipientRole::Spectator
            };
            player.spectator = mapped_seat.is_none();
            player.ready = false;
            recipients.push(LaunchRecipient {
                connection_id,
                payload_player_id: mapped_seat.unwrap_or(connection_id),
                spectator: mapped_seat.is_none(),
                prediction: if mapped_seat.is_some() {
                    LaunchPrediction::Enabled
                } else {
                    LaunchPrediction::Disabled
                },
                capabilities: start_policy.start_capabilities(mapped_seat.is_some()),
                diagnostics: projection_policy.diagnostic_capabilities_for(role),
                clear_pending_snapshot: true,
                lab: None,
                msg_tx: player.msg_tx.clone(),
            });
        }
        super::launch::send_start_payloads(&self.room, &payload, recipients);

        structured_log::log_match_started(MatchStartedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
            mode: "replay_branch",
            map: &self.match_map_name,
            seed: launch.seed,
            players: self.match_player_count,
            humans: self.match_human_count,
            ai: 0,
            quickstart: false,
            participants: &self.match_participants,
        });
        self.mark_match_started_for_drain();
        self.phase = Phase::InGame(Box::new(game));
        self.broadcast_live_pause_state();
    }

    fn start_lab_session(&mut self) {
        self.prepare_live_match_launch();
        let config = match &self.mode {
            RoomMode::Lab(config) => config.clone(),
            _ => return,
        };
        if self.lab_session.is_none() {
            if let Some(operator_id) = self.order.first().copied() {
                self.lab_session = Some(LabSession::new(&config, operator_id));
            }
        }
        let seed = config.seed.unwrap_or_else(match_seed);
        let inits = self.default_lab_player_template();
        let start_players: Vec<_> = inits
            .iter()
            .map(|player| {
                (
                    player.id,
                    normalize_start_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map_metadata = match Map::metadata_for_name(&config.map_name) {
            Ok(metadata) => metadata,
            Err(err) => {
                self.send_lab_error(format!(
                    "Cannot load lab map \"{}\": {err}",
                    config.map_name
                ));
                return;
            }
        };
        let map = match Map::load_for_players(&config.map_name, &start_players, seed) {
            Ok(map) => map,
            Err(err) => {
                self.send_lab_error(format!(
                    "Cannot load lab map \"{}\": {err}",
                    config.map_name
                ));
                return;
            }
        };
        let game =
            Game::new_with_random_ai_profiles_and_map_metadata(&inits, seed, map, map_metadata);
        self.record_live_match_started(
            inits.len(),
            0,
            config.map_name.clone(),
            inits.iter().map(|player| player.name.clone()).collect(),
        );
        let mut payload = game.start_payload();
        payload.match_run_id = self.match_run_id.clone();
        self.ai_controllers.clear();
        self.dev_driver = None;
        self.dev_view_player_id = None;

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        let recipients: Vec<_> = self
            .order
            .iter()
            .filter_map(|&id| {
                self.players.get(&id).map(|player| LaunchRecipient {
                    connection_id: id,
                    payload_player_id: self
                        .lab_session
                        .as_ref()
                        .map(|session| session.view_player_id)
                        .unwrap_or(LAB_PLAYER_ONE_ID),
                    spectator: true,
                    prediction: LaunchPrediction::Disabled,
                    capabilities: start_policy.start_capabilities(false),
                    diagnostics: projection_policy
                        .diagnostic_capabilities_for(RecipientRole::Spectator),
                    clear_pending_snapshot: false,
                    lab: self.lab_start_metadata_for(id),
                    msg_tx: player.msg_tx.clone(),
                })
            })
            .collect();
        super::launch::send_start_payloads(&self.room, &payload, recipients);

        structured_log::log_match_started(MatchStartedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
            mode: "lab",
            map: &self.match_map_name,
            seed,
            players: self.match_player_count,
            humans: self.match_human_count,
            ai: 0,
            quickstart: false,
            participants: &self.match_participants,
        });
        self.mark_match_started_for_drain();
        self.phase = Phase::InGame(Box::new(game));
    }

    fn default_lab_player_template(&self) -> Vec<PlayerInit> {
        vec![
            PlayerInit {
                id: LAB_PLAYER_ONE_ID,
                team_id: 1,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: "Lab Alpha".to_string(),
                color: PLAYER_PALETTE[0].to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: LAB_PLAYER_TWO_ID,
                team_id: 2,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: "Lab Bravo".to_string(),
                color: PLAYER_PALETTE[1].to_string(),
                is_ai: false,
            },
        ]
    }

    fn send_lab_error(&self, msg: String) {
        let error = ServerMessage::Error { msg };
        self.broadcast(&error);
    }

    fn start_dev_session(&mut self) {
        self.prepare_live_match_launch();
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
            RoomMode::ReplayArtifact { .. } => {
                Err("room is not configured for a dev session".to_string())
            }
            RoomMode::ReplayBranch { .. } => {
                Err("room is not configured for a dev session".to_string())
            }
            RoomMode::Lab(_) => Err("room is not configured for a dev session".to_string()),
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
                    DevScenarioId::TankTrapLineHorizontal
                    | DevScenarioId::TankTrapLineVertical
                    | DevScenarioId::TankTrapLineDiagonal => {
                        let scenario_id = match config.id {
                            DevScenarioId::TankTrapLineHorizontal => "tank_trap_line_horizontal",
                            DevScenarioId::TankTrapLineVertical => "tank_trap_line_vertical",
                            DevScenarioId::TankTrapLineDiagonal => "tank_trap_line_diagonal",
                            _ => unreachable!("outer match selects Tank Trap line scenarios"),
                        };
                        let setup = Game::new_tank_trap_line_build_scenario(
                            scenario_id,
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
                    DevScenarioId::TankTrapPathingMatrix => {
                        let scenario_case = config
                            .case
                            .ok_or_else(|| "missing Tank Trap pathing case".to_string())?;
                        let setup = Game::new_tank_trap_pathing_scenario(
                            scenario_case,
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
        let payload = game.start_payload();
        let diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        let capabilities = self.session_policy().start_capabilities(false);
        super::launch::send_start_payloads(
            &self.room,
            &payload,
            [LaunchRecipient {
                connection_id: watcher_id,
                payload_player_id: self.dev_view_player_id.unwrap_or(watcher_id),
                spectator: true,
                prediction: LaunchPrediction::Disabled,
                capabilities,
                diagnostics,
                clear_pending_snapshot: false,
                lab: None,
                msg_tx: player.msg_tx.clone(),
            }],
        );
    }

    fn send_lab_start_to(&self, watcher_id: u32) {
        let Some(Phase::InGame(game)) = Some(&self.phase) else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        let payload = game.start_payload();
        let diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        let capabilities = self.session_policy().start_capabilities(false);
        super::launch::send_start_payloads(
            &self.room,
            &payload,
            [LaunchRecipient {
                connection_id: watcher_id,
                payload_player_id: self
                    .lab_session
                    .as_ref()
                    .map(|session| session.view_player_id)
                    .unwrap_or(LAB_PLAYER_ONE_ID),
                spectator: true,
                prediction: LaunchPrediction::Disabled,
                capabilities,
                diagnostics,
                clear_pending_snapshot: false,
                lab: self.lab_start_metadata_for(watcher_id),
                msg_tx: player.msg_tx.clone(),
            }],
        );
    }

    fn lab_start_metadata_for(&self, player_id: u32) -> Option<LabStartMetadata> {
        self.lab_session
            .as_ref()
            .map(|session| session.metadata_for(player_id))
    }

    fn lab_snapshot_projection_inputs(&self, game: &Game) -> (Option<u32>, Option<Vec<u32>>) {
        let Some(session) = &self.lab_session else {
            return (None, None);
        };
        match &session.vision_mode {
            LabVisionMode::FullWorld => (Some(session.view_player_id), None),
            LabVisionMode::Team { team_id } => (
                None,
                Some(players_on_teams(game, std::iter::once(*team_id))),
            ),
            LabVisionMode::Teams { team_ids } => {
                (None, Some(players_on_teams(game, team_ids.iter().copied())))
            }
        }
    }

    fn send_replay_start_to(&self, watcher_id: u32) {
        let Phase::ReplayViewer(session) = &self.phase else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        let mut payload = session.start_payload_for(watcher_id);
        payload.diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        payload.capabilities = self.session_policy().start_capabilities(false);
        send_or_log(
            &self.room,
            watcher_id,
            &player.msg_tx,
            ServerMessage::Start(payload),
        );
    }

    fn send_room_time_state_to(&self, watcher_id: u32) {
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
            ServerMessage::RoomTimeState(session.state()),
        );
    }

    fn send_observer_analysis_to(&self, watcher_id: u32) {
        let Phase::ReplayViewer(session) = &self.phase else {
            return;
        };
        if self.projection_policy().observer_analysis_audience()
            != ObserverAnalysisAudience::ReplayViewers
        {
            return;
        }
        self.send_observer_analysis_to_ids(
            [watcher_id],
            ServerMessage::ObserverAnalysis(session.game().observer_analysis()),
        );
    }

    fn broadcast_room_time_state_for(&self, session: &ReplaySession) {
        let msg = ServerMessage::RoomTimeState(session.state());
        self.broadcast(&msg);
    }

    fn broadcast_observer_analysis_for(
        &self,
        session: &ReplaySession,
        projection_policy: ProjectionPolicy,
    ) {
        if projection_policy.observer_analysis_audience() != ObserverAnalysisAudience::ReplayViewers
        {
            return;
        }
        self.send_observer_analysis_to_ids(
            self.order.clone(),
            ServerMessage::ObserverAnalysis(session.game().observer_analysis()),
        );
    }

    fn send_observer_analysis_to_ids(
        &self,
        recipient_ids: impl IntoIterator<Item = u32>,
        msg: ServerMessage,
    ) {
        for id in recipient_ids {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            send_or_log(&self.room, id, &player.msg_tx, msg.clone());
        }
    }

    fn broadcast_dev_watch_state(&self) {
        if !self.session_policy().is_dev_watch() {
            return;
        }
        let Phase::InGame(game) = &self.phase else {
            return;
        };
        self.broadcast(&ServerMessage::RoomTimeState(RoomTimeState {
            current_tick: game.tick_count(),
            duration_ticks: 0,
            keyframe_ticks: Vec::new(),
            speed: if self.room_time_paused {
                0.0
            } else {
                self.room_time_speed
            },
            paused: self.room_time_paused,
            ended: false,
            controller_id: None,
        }));
    }

    fn fanout_replay_snapshots(
        &mut self,
        session: &ReplaySession,
        mut per_player_events: HashMap<u32, Vec<Event>>,
        context: ReplayTickContext,
        perf: Option<&mut rts_sim::perf::TickPerf>,
    ) {
        let recipients = self.order.clone();
        SnapshotFanout::new(
            &self.room,
            context.scheduler_lag,
            context.tick_budget,
            context.tick_start,
            &mut self.slow_tick_count,
            perf,
        )
        .send_to_recipients(&mut self.players, recipients, |id, _player| {
            let projection = context
                .projection_policy
                .replay_snapshot_for(session.vision_player_ids_for(id));
            let snapshot =
                projection.snapshot_with_events(session.game(), &mut per_player_events, &[]);
            Some(SnapshotFanoutPayload::new(snapshot, true))
        });
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
        match self.tick_control().scheduled_action() {
            ScheduledTickAction::Noop => return,
            ScheduledTickAction::Countdown => {
                self.finish_match_countdown_if_due();
                return;
            }
            ScheduledTickAction::RoomControlled(RoomTimeSource::ReplayPlayback) => {
                self.on_tick_replay_viewer(scheduled);
                return;
            }
            ScheduledTickAction::RoomControlled(RoomTimeSource::DevScenario) => {
                self.on_tick_dev_watch(scheduled);
                return;
            }
            ScheduledTickAction::LiveMatch => {}
        }
        if self.live_paused && self.live_pause_controls_available() {
            return;
        }
        // Take ownership of the game for the duration of the tick so we can both mutate it and
        // freely borrow `self` for sending. Restored (or replaced with `Lobby`) before return.
        let projection_policy = self.projection_policy();
        let game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
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
        let tick_budget = self.current_tick_interval();
        let match_run_id = self.match_run_id.as_deref();
        let ai_player_count = self.ai_players.len();
        let spectator_visible_players = self.spectator_visible_player_ids();
        let (full_world_view_player_id, lab_visible_player_ids) =
            self.lab_snapshot_projection_inputs(&game);
        let result = LiveTickDriver {
            room: &self.room,
            scheduled,
            tick_budget,
            match_run_id,
            match_player_count: self.match_player_count,
            ai_player_count,
            players: &mut self.players,
            order: &self.order,
            outcome_sent: &mut self.outcome_sent,
            branch_live_seat_by_connection: &self.branch_live_seat_by_connection,
            ai_controllers: &mut self.ai_controllers,
            pending_client_command_acks: &mut self.pending_client_command_acks,
            pending_recipient_notices: &mut self.pending_recipient_notices,
            slow_tick_count: &mut self.slow_tick_count,
            spectator_visible_players,
            full_world_view_player_id,
            lab_visible_player_ids,
            projection_policy,
        }
        .run(game);

        match result {
            LiveTickResult::Continue(game) => {
                self.phase = Phase::InGame(game);
            }
            LiveTickResult::EndMatch {
                game,
                winner_id,
                scores,
            } => {
                self.end_match(winner_id, scores, Some(&game));
            }
            LiveTickResult::PanicEnd { scores } => {
                self.end_match(None, scores, None);
            }
        }
    }

    fn on_tick_dev_watch(&mut self, scheduled: TokioInstant) {
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
        let mut perf = rts_sim::perf::TickPerf::maybe_new();
        let Some(mut driver) = self.dev_driver.take() else {
            self.phase = Phase::InGame(game);
            return;
        };
        rts_sim::perf::timed(perf.as_mut(), "dev_driver_enqueue", || {
            driver.enqueue_for_tick(&mut game)
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
        let recipients = self.order.clone();
        let view_player_id = self.dev_view_player_id.unwrap_or(0);
        let projection = self.projection_policy().dev_snapshot_for(view_player_id);
        SnapshotFanout::new(
            &self.room,
            scheduler_lag,
            tick_budget,
            tick_start,
            &mut self.slow_tick_count,
            perf.as_mut(),
        )
        .send_to_recipients(&mut self.players, recipients, |_id, player| {
            let snapshot = projection.snapshot_with_events(&game, &mut per_player_events, &[]);
            Some(SnapshotFanoutPayload::new(snapshot, player.spectator))
        });

        self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
        self.dev_driver = Some(driver);
        self.phase = Phase::InGame(game);
    }

    fn on_tick_replay_viewer(&mut self, scheduled: TokioInstant) {
        let context = ReplayTickContext {
            scheduler_lag: scheduled.elapsed(),
            tick_budget: self.current_tick_interval(),
            tick_start: StdInstant::now(),
            projection_policy: self.projection_policy_for_phase(SessionPhase::ReplayViewer),
        };
        let mut session = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::ReplayViewer(session) => session,
            other => {
                self.phase = other;
                return;
            }
        };
        let mut perf = rts_sim::perf::TickPerf::maybe_new();

        if session.has_remaining_ticks() {
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
                    dump_crash_replay(&self.room, session.game(), &reason);
                    self.send_dev_error("Replay playback failed");
                    self.phase = Phase::Lobby;
                    return;
                }
            };
            session.record_keyframe_if_due();
            self.fanout_replay_snapshots(&session, per_player_events, context, perf.as_mut());
            self.broadcast_observer_analysis_for(&session, context.projection_policy);
        } else {
            self.broadcast_room_time_state_for(&session);
            self.broadcast_observer_analysis_for(&session, context.projection_policy);
        }

        self.finish_perf_tick(
            perf.as_ref(),
            session.game(),
            context.scheduler_lag,
            context.tick_start,
        );
        self.phase = Phase::ReplayViewer(session);
    }

    pub(super) fn on_set_room_time_speed(&mut self, player_id: u32, speed: f32) {
        if !self.tick_control().allows_room_time_operation(
            RoomTimeOperation::SetSpeed,
            self.players.contains_key(&player_id),
        ) {
            return;
        }

        match self.session_policy().clock.room_time_source() {
            Some(RoomTimeSource::ReplayPlayback) => {}
            Some(RoomTimeSource::DevScenario) => {
                match TickControl::room_time_speed(speed) {
                    RoomTimeSpeed::Paused => {
                        self.room_time_paused = true;
                    }
                    RoomTimeSpeed::Running(speed) => {
                        self.room_time_paused = false;
                        self.room_time_speed = speed;
                    }
                }
                self.broadcast_dev_watch_state();
                return;
            }
            None => return,
        }

        if let Phase::ReplayViewer(session) = &mut self.phase {
            session.set_speed(player_id, speed);
            let state = session.state();
            self.broadcast(&ServerMessage::RoomTimeState(state));
        }
    }

    fn on_step_room_time(&mut self, player_id: u32) {
        if !self
            .tick_control()
            .can_step_room_time(self.players.contains_key(&player_id))
        {
            return;
        }
        if self.session_policy().clock.room_time_source() != Some(RoomTimeSource::DevScenario) {
            return;
        }
        self.on_tick_dev_watch(TokioInstant::now());
        self.broadcast_dev_watch_state();
    }

    fn on_set_replay_vision(&mut self, player_id: u32, vision: ReplayVisionRequest) {
        let send_analysis = self.projection_policy().observer_analysis_audience()
            == ObserverAnalysisAudience::ReplayViewers;
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
            let analysis = send_analysis.then(|| session.game().observer_analysis());
            if let (Some(analysis), Some(player)) = (analysis, self.players.get(&player_id)) {
                send_or_log(
                    &self.room,
                    player_id,
                    &player.msg_tx,
                    ServerMessage::ObserverAnalysis(analysis),
                );
            }
        }
    }

    fn on_lab_request(&mut self, player_id: u32, request_id: u32, op: LabClientOp) {
        let op_kind = lab_op_kind(&op).to_string();
        if request_id == 0 {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some("requestId must be nonzero".to_string()),
                    outcome: None,
                },
            );
            return;
        }
        let policy = self.session_policy();
        if !policy.allows_lab_privileged_ops() {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some("lab requests are only valid in lab rooms".to_string()),
                    outcome: None,
                },
            );
            return;
        }
        if matches!(
            op,
            LabClientOp::ExportScenario { .. } | LabClientOp::ImportScenario { .. }
        ) && !policy.allows_lab_scenario_io()
        {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some(
                        "lab scenario import/export is not enabled in this room".to_string(),
                    ),
                    outcome: None,
                },
            );
            return;
        }
        if !self
            .lab_session
            .as_ref()
            .map(|session| session.can_operate(player_id))
            .unwrap_or(false)
        {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some("only lab operators can send lab requests".to_string()),
                    outcome: None,
                },
            );
            return;
        }

        let result = match op {
            LabClientOp::SetVision { vision } => {
                self.apply_lab_vision(player_id, request_id, vision)
            }
            LabClientOp::ExportScenario { name } => self.export_lab_scenario(request_id, name),
            LabClientOp::IssueCommandAs {
                player_id: command_player_id,
                cmd,
            } => self.apply_lab_issue_command(request_id, player_id, command_player_id, cmd),
            op => self.apply_lab_mutation(player_id, request_id, op),
        };
        self.send_lab_result_to(player_id, result);
    }

    fn apply_lab_vision(
        &mut self,
        operator_id: u32,
        request_id: u32,
        vision: LabVisionMode,
    ) -> LabResult {
        let op = "setVision".to_string();
        let Some(game) = self.live_game() else {
            return lab_result_error(request_id, op, "lab game is not running");
        };
        if let Err(err) = validate_lab_vision(game, &vision) {
            return lab_result_error(request_id, op, &err);
        }
        let tick = game.tick_count();
        let log_operations = self.session_policy().logs_lab_operations();
        if let Some(session) = &mut self.lab_session {
            session.vision_mode = vision;
            if log_operations {
                session.operation_log.push(LabOperationLogEntry {
                    tick,
                    request_id,
                    operator_id,
                    op: op.clone(),
                    result: "accepted".to_string(),
                });
            }
        }
        self.broadcast_lab_state();
        LabResult {
            request_id,
            ok: true,
            op,
            error: None,
            outcome: None,
        }
    }

    fn apply_lab_issue_command(
        &mut self,
        request_id: u32,
        operator_id: u32,
        command_player_id: u32,
        cmd: Command,
    ) -> LabResult {
        let op = "issueCommandAs".to_string();
        let log_operations = self.session_policy().logs_lab_operations();
        let tick = {
            let Some(game) = self.live_game_mut() else {
                return lab_result_error(request_id, op, "lab game is not running");
            };
            if let Err(err) = game.issue_lab_command_as(command_player_id, cmd) {
                return lab_result_error(request_id, op, &lab_error_text(&err));
            }
            game.tick_count()
        };
        if let Some(session) = &mut self.lab_session {
            session.dirty = true;
            if log_operations {
                session.operation_log.push(LabOperationLogEntry {
                    tick,
                    request_id,
                    operator_id,
                    op: op.clone(),
                    result: format!("playerId={command_player_id}"),
                });
            }
        }
        self.broadcast_lab_state();
        LabResult {
            request_id,
            ok: true,
            op,
            error: None,
            outcome: None,
        }
    }

    fn export_lab_scenario(&self, request_id: u32, name: Option<String>) -> LabResult {
        let op = "exportScenario".to_string();
        if !self.session_policy().allows_lab_scenario_io() {
            return lab_result_error(request_id, op, "lab scenario export is not enabled");
        }
        let Some(game) = self.live_game() else {
            return lab_result_error(request_id, op, "lab game is not running");
        };
        let Some(session) = &self.lab_session else {
            return lab_result_error(request_id, op, "lab session is not running");
        };
        let mut scenario = match serde_json::to_value(game.export_lab_scenario()) {
            Ok(value) => value,
            Err(err) => {
                return lab_result_error(request_id, op, &format!("scenario export failed: {err}"));
            }
        };
        if let Some(object) = scenario.as_object_mut() {
            let scenario_name = name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Untitled lab scenario");
            object.insert(
                "name".to_string(),
                serde_json::Value::String(truncate_lab_scenario_name(scenario_name)),
            );
            if let Some(metadata) = object
                .get_mut("metadata")
                .and_then(|value| value.as_object_mut())
            {
                metadata.insert(
                    "lab".to_string(),
                    serde_json::to_value(LabScenarioLabMetadata {
                        vision: session.vision_mode.clone(),
                    })
                    .unwrap_or_else(|_| serde_json::json!({ "vision": { "mode": "fullWorld" } })),
                );
            }
        }
        LabResult {
            request_id,
            ok: true,
            op,
            error: None,
            outcome: Some(serde_json::json!({ "scenario": scenario })),
        }
    }

    fn apply_lab_mutation(
        &mut self,
        operator_id: u32,
        request_id: u32,
        op: LabClientOp,
    ) -> LabResult {
        let op_kind = lab_op_kind(&op).to_string();
        let imported_vision = match &op {
            LabClientOp::ImportScenario { scenario } => Some(scenario.metadata.lab.vision.clone()),
            _ => None,
        };
        let lab_op = match lab_client_op_to_game_op(op) {
            Ok(op) => op,
            Err(err) => return lab_result_error(request_id, op_kind, &err),
        };
        let log_operations = self.session_policy().logs_lab_operations();
        let (tick, outcome_json) = {
            let Some(game) = self.live_game_mut() else {
                return lab_result_error(request_id, op_kind, "lab game is not running");
            };
            let outcome = match game.apply_lab_op(lab_op) {
                Ok(outcome) => outcome,
                Err(err) => return lab_result_error(request_id, op_kind, &lab_error_text(&err)),
            };
            (game.tick_count(), lab_outcome_json(&outcome))
        };
        if let Some(session) = &mut self.lab_session {
            session.dirty = true;
            if let Some(vision) = imported_vision {
                session.vision_mode = vision;
            }
            if log_operations {
                session.operation_log.push(LabOperationLogEntry {
                    tick,
                    request_id,
                    operator_id,
                    op: op_kind.clone(),
                    result: outcome_json.to_string(),
                });
            }
        }
        self.broadcast_lab_state();
        LabResult {
            request_id,
            ok: true,
            op: op_kind,
            error: None,
            outcome: Some(outcome_json),
        }
    }

    fn live_game(&self) -> Option<&Game> {
        match &self.phase {
            Phase::InGame(game) => Some(game),
            _ => None,
        }
    }

    fn live_game_mut(&mut self) -> Option<&mut Game> {
        match &mut self.phase {
            Phase::InGame(game) => Some(game),
            _ => None,
        }
    }

    fn send_lab_result_to(&self, player_id: u32, result: LabResult) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::LabResult(result),
        );
    }

    fn broadcast_lab_state(&self) {
        let Some(session) = &self.lab_session else {
            return;
        };
        for &id in &self.order {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::LabState(session.state_for(id)),
            );
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
        staging.message(
            self.room.clone(),
            self.host_id.unwrap_or(0),
            occupants,
            self.match_countdown_deadline.is_none() && !self.drain.is_draining(),
        )
    }

    fn broadcast_branch_staging(&self) {
        let Some(staging) = self.branch_staging() else {
            return;
        };
        self.broadcast(&self.branch_staging_message(staging));
    }

    /// Rewind room-controlled replay time by `ticks_back` ticks. Pass `u32::MAX` to reset to the start.
    /// No-op outside rooms whose clock capability allows relative seek.
    fn on_seek_room_time(&mut self, player_id: u32, ticks_back: u32) {
        if !self.tick_control().allows_room_time_operation(
            RoomTimeOperation::SeekRelative,
            self.players.contains_key(&player_id),
        ) {
            return;
        }
        let send_analysis = self.projection_policy().observer_analysis_audience()
            == ObserverAnalysisAudience::ReplayViewers;
        let start_diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        let start_capabilities = self.session_policy().start_capabilities(false);
        if let Phase::ReplayViewer(session) = &mut self.phase {
            let viewer_count = self.players.len();
            let seek_result = session.seek_back(&self.room, viewer_count, player_id, ticks_back);
            let starts = if seek_result.is_ok() {
                self.order
                    .iter()
                    .filter_map(|viewer_id| {
                        self.players.get(viewer_id).map(|player| {
                            let mut start = session.start_payload_for(*viewer_id);
                            start.diagnostics = start_diagnostics;
                            start.capabilities = start_capabilities;
                            (*viewer_id, player.msg_tx.clone(), start)
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
                .filter(|_| send_analysis)
                .map(|_| session.game().observer_analysis());
            match seek_result {
                Ok(_) => {
                    for (viewer_id, msg_tx, start) in starts {
                        send_or_log(&self.room, viewer_id, &msg_tx, ServerMessage::Start(start));
                    }
                    if let Some(state) = state {
                        self.broadcast(&ServerMessage::RoomTimeState(state));
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

    /// Seek room-controlled replay time to an absolute tick. No-op outside rooms whose clock
    /// capability allows absolute seek.
    fn on_seek_room_time_to(&mut self, player_id: u32, tick: u32) {
        if !self.tick_control().allows_room_time_operation(
            RoomTimeOperation::SeekAbsolute,
            self.players.contains_key(&player_id),
        ) {
            return;
        }
        let send_analysis = self.projection_policy().observer_analysis_audience()
            == ObserverAnalysisAudience::ReplayViewers;
        let start_diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        let start_capabilities = self.session_policy().start_capabilities(false);
        if let Phase::ReplayViewer(session) = &mut self.phase {
            let viewer_count = self.players.len();
            let seek_result = session.seek_to(&self.room, viewer_count, player_id, tick);
            let starts = if seek_result.is_ok() {
                self.order
                    .iter()
                    .filter_map(|viewer_id| {
                        self.players.get(viewer_id).map(|player| {
                            let mut start = session.start_payload_for(*viewer_id);
                            start.diagnostics = start_diagnostics;
                            start.capabilities = start_capabilities;
                            (*viewer_id, player.msg_tx.clone(), start)
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
                .filter(|_| send_analysis)
                .map(|_| session.game().observer_analysis());
            match seek_result {
                Ok(_) => {
                    for (viewer_id, msg_tx, start) in starts {
                        send_or_log(&self.room, viewer_id, &msg_tx, ServerMessage::Start(start));
                    }
                    if let Some(state) = state {
                        self.broadcast(&ServerMessage::RoomTimeState(state));
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
        let replay_artifact = game
            .filter(|_| self.should_capture_post_match_replay())
            .map(|game| {
                ReplayArtifactV1::capture_from_game(
                    game,
                    server_build_sha(),
                    winner_id,
                    scores.clone(),
                )
            });
        let will_record_history = self.db.is_some()
            && self.match_started_at.is_some()
            && self.should_persist_match_history();
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

        // Persist replay-backed history for deploy-recorded matches. The Recent Matches endpoint
        // filters debug and AI-only rows; persistence keeps their replay artifacts available for
        // follow-up diagnostics without exposing them on the lobby front page.
        if let (Some(db), Some(started_at)) = (self.db.clone(), self.match_started_at) {
            if self.should_persist_match_history() {
                let duration_ms = ended_at
                    .signed_duration_since(started_at)
                    .num_milliseconds()
                    .clamp(0, i32::MAX as i64) as i32;
                let winner_name = winner_id
                    .and_then(|wid| scores.iter().find(|s| s.id == wid).map(|s| s.name.clone()));
                let score_json = serde_json::to_value(&scores).unwrap_or(serde_json::Value::Null);
                let replay = if self.should_attach_match_history_replay_artifact() {
                    replay_artifact.as_ref().and_then(|artifact| {
                        match crate::db::MatchReplayRecord::from_artifact(artifact) {
                            Ok(replay) => Some(replay),
                            Err(err) => {
                                crate::log_warn!(room = %self.room, error = %err, "failed to serialize replay artifact for match history");
                                None
                            }
                        }
                    })
                } else {
                    None
                };
                let rec = crate::db::MatchRecord {
                    started_at,
                    ended_at,
                    duration_ms,
                    map_name: self.match_map_name.clone(),
                    winner_name,
                    participants: self.match_participants.clone(),
                    score_screen: score_json,
                    human_count: i32::try_from(self.match_human_count).unwrap_or(i32::MAX),
                    debug_mode: self.quickstart,
                    local_only: self.match_history_local_only,
                    replay,
                };
                // Detached: a slow Supabase write must never stall the room transitioning back to
                // lobby. Errors are logged inside `record_match`.
                tokio::spawn(async move { db.record_match(rec).await });
            }
        }
        self.clear_finished_match_identity();

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
        self.reset_after_live_match_for_room_phase();
        let recipients = self.order.clone();
        for id in recipients {
            self.send_replay_start_to(id);
            self.send_room_time_state_to(id);
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
        self.reset_after_live_match_for_room_phase();
        self.broadcast_lobby();
    }

    fn prepare_live_match_launch(&mut self) {
        self.match_countdown_deadline = None;
        self.reset_match_net_status();
        self.reset_live_pause_state();
    }

    fn record_live_match_started(
        &mut self,
        player_count: usize,
        human_count: usize,
        map_name: String,
        participants: Vec<String>,
    ) {
        self.match_player_count = player_count;
        self.match_human_count = human_count;
        self.match_started_at = Some(chrono::Utc::now());
        self.match_run_id = Some(structured_log::new_match_run_id(&self.room));
        self.match_map_name = map_name;
        self.match_participants = participants;
        self.outcome_sent.clear();
    }

    fn reset_after_live_match_for_room_phase(&mut self) {
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.outcome_sent.clear();
        self.branch_live_seat_by_connection.clear();
        self.pending_recipient_notices.clear();
        self.reset_live_pause_state();
        for player in self.players.values_mut() {
            player.ready = false;
            player.msg_tx.clear_pending_snapshot();
        }
    }

    fn clear_finished_match_identity(&mut self) {
        self.match_started_at = None;
        self.match_run_id = None;
        self.match_map_name.clear();
        self.match_participants.clear();
    }

    fn reset_empty_room_state(&mut self) {
        self.phase = Phase::Lobby;
        self.created_at_unix_ms = current_unix_ms();
        self.empty_lobby_reserved_until_unix_ms = None;
        self.match_countdown_deadline = None;
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.outcome_sent.clear();
        self.branch_live_seat_by_connection.clear();
        self.pending_recipient_notices.clear();
        self.reset_live_pause_state();
        self.lab_session = None;
        self.host_id = None;
        // Drop AI opponents too: with no humans there is nobody to host them, and a fresh
        // joiner under this room name should start from a clean lobby.
        self.ai_players.clear();
        self.human_team_assignments.clear();
        self.human_faction_assignments.clear();
        self.dev_driver = None;
        self.dev_view_player_id = None;
        self.ai_controllers.clear();
        self.pending_client_command_acks.clear();
        self.clear_finished_match_identity();
        if matches!(self.mode, RoomMode::ReplayBranch { .. }) {
            self.mode = RoomMode::Normal;
        }
    }

    fn reset_live_pause_state(&mut self) {
        self.live_paused = false;
        self.live_paused_by = None;
        self.live_pause_counts.clear();
    }

    fn mark_match_started_for_drain(&mut self) {
        if !self.match_tracked_for_drain && !self.is_dev_watch() {
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
        perf: Option<&rts_sim::perf::TickPerf>,
        game: &Game,
        scheduler_lag: Duration,
        tick_start: StdInstant,
    ) {
        let Some(perf) = perf else {
            return;
        };
        perf.finish(rts_sim::perf::TickContext {
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
        self.pending_recipient_notices.clear();
        for player in self.players.values_mut() {
            player.head_of_line_count = 0;
            player.last_received_client_seq = 0;
            player.last_sim_consumed_client_seq = 0;
            player.last_sim_consumed_client_tick = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DEFAULT_FACTION_ID;
    use rts_rules::faction::{EKAT_FACTION_ID, EMPTY_FIXTURE_FACTION_ID};

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

    fn lab_config() -> LabRoomConfig {
        LabRoomConfig {
            public_id: "sandbox".to_string(),
            map_name: "Default".to_string(),
            seed: Some(0x1A2B_3C4D),
        }
    }

    fn summary_task(room: &str) -> RoomTask {
        let mut task = RoomTask::new(
            room.to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        task.created_at_unix_ms = 123_456;
        task.host_id = Some(1);
        add_test_room_player(&mut task, 1, false);
        task.assign_missing_team_for(1);
        task.assign_missing_faction_for(1);
        task
    }

    #[test]
    fn lobby_summary_reports_open_waiting_room_state() {
        let task = summary_task("open-summary");

        let summary = task
            .lobby_summary()
            .expect("normal hosted lobby should be summarized");

        assert_eq!(summary.room, "open-summary");
        assert_eq!(summary.host_name.as_deref(), Some("Player 1"));
        assert_eq!(summary.map, "Default");
        assert_eq!(summary.created_at_unix_ms, 123_456);
        assert_eq!(summary.occupied_slots, 1);
        assert_eq!(summary.max_slots, MAX_PLAYERS);
        assert_eq!(summary.spectator_count, 0);
        assert_eq!(summary.phase, LobbySummaryPhase::Lobby);
        assert_eq!(summary.join_state, LobbyJoinState::Open);
    }

    #[test]
    fn lobby_summary_marks_full_waiting_rooms_spectator_joinable() {
        let mut task = summary_task("full-summary");
        for id in 2..=4 {
            add_test_room_player(&mut task, id, false);
            task.assign_missing_team_for(id);
            task.assign_missing_faction_for(id);
        }
        add_test_room_spectator(&mut task, 99);

        let summary = task
            .lobby_summary()
            .expect("full waiting lobby should remain visible");

        assert_eq!(summary.occupied_slots, MAX_PLAYERS);
        assert_eq!(summary.spectator_count, 1);
        assert_eq!(summary.phase, LobbySummaryPhase::Lobby);
        assert_eq!(summary.join_state, LobbyJoinState::FullSpectatorOnly);
    }

    #[test]
    fn lobby_summary_marks_countdown_as_starting() {
        let mut task = summary_task("countdown-summary");
        add_test_room_player(&mut task, 2, true);
        task.match_countdown_deadline = Some(TokioInstant::now() + Duration::from_secs(3));

        let summary = task
            .lobby_summary()
            .expect("countdown lobby should remain visible");

        assert_eq!(summary.phase, LobbySummaryPhase::Countdown);
        assert_eq!(summary.join_state, LobbyJoinState::Starting);
    }

    #[test]
    fn lobby_summary_includes_live_normal_rooms_as_non_joinable() {
        let mut task = summary_task("ingame-summary");
        let players = replay_test_players(2);
        task.phase = Phase::InGame(Box::new(replay_test_game(&players, 0)));
        task.match_map_name = "Default".to_string();

        let summary = task
            .lobby_summary()
            .expect("normal live room should remain visible");

        assert_eq!(summary.map, "Default");
        assert_eq!(summary.phase, LobbySummaryPhase::InGame);
        assert_eq!(summary.join_state, LobbyJoinState::InGame);
    }

    #[test]
    fn lobby_summary_hides_internal_room_modes() {
        let replay_players = replay_test_players(2);
        let (_live, replay_artifact) = replay_test_artifact(&replay_players, 0);
        let branch_seed = replay_branch_test_seed(&replay_players, 0);

        let mut lab = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        lab.host_id = Some(1);
        add_test_room_spectator(&mut lab, 1);
        assert!(lab.lobby_summary().is_none());

        let mut saved_replay = RoomTask::new(
            "__replay_artifact__:demo".to_string(),
            RoomMode::ReplayArtifact {
                artifact: "demo".to_string(),
            },
            None,
            false,
            DrainHandle::default(),
        );
        saved_replay.host_id = Some(1);
        add_test_room_spectator(&mut saved_replay, 1);
        assert!(saved_replay.lobby_summary().is_none());

        let mut persisted_replay = RoomTask::new(
            "__match_replay__:00000001".to_string(),
            RoomMode::Replay {
                artifact: replay_artifact,
            },
            None,
            false,
            DrainHandle::default(),
        );
        persisted_replay.host_id = Some(1);
        add_test_room_spectator(&mut persisted_replay, 1);
        assert!(persisted_replay.lobby_summary().is_none());

        let mut branch = RoomTask::new(
            "__replay_branch__:00000001".to_string(),
            RoomMode::ReplayBranch { seed: branch_seed },
            None,
            false,
            DrainHandle::default(),
        );
        branch.host_id = Some(1);
        add_test_room_spectator(&mut branch, 1);
        assert!(branch.lobby_summary().is_none());

        let mut dev = RoomTask::new(
            "__dev_scenario__:demo".to_string(),
            RoomMode::DevScenario(DevScenarioConfig {
                id: DevScenarioId::DirectReverseOrder,
                unit: EntityKind::Worker,
                count: 1,
                blocker: None,
                case: None,
            }),
            None,
            false,
            DrainHandle::default(),
        );
        dev.host_id = Some(1);
        add_test_room_spectator(&mut dev, 1);
        assert!(dev.lobby_summary().is_none());
    }

    #[test]
    fn set_faction_accepts_playable_and_rejects_fixture_in_lobby() {
        let mut task = RoomTask::new(
            "faction-lobby-policy".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        task.host_id = Some(1);
        add_test_room_player(&mut task, 1, true);
        task.assign_missing_faction_for(1);

        task.on_set_faction(1, EKAT_FACTION_ID.to_string());
        assert_eq!(
            task.human_faction_assignments.get(&1).map(String::as_str),
            Some(EKAT_FACTION_ID)
        );

        task.on_set_faction(1, EMPTY_FIXTURE_FACTION_ID.to_string());
        assert_eq!(
            task.human_faction_assignments.get(&1).map(String::as_str),
            Some(EKAT_FACTION_ID),
            "fixture-only catalog ids must not overwrite a playable lobby selection"
        );

        task.on_set_faction(1, "unknown_faction".to_string());
        assert_eq!(
            task.human_faction_assignments.get(&1).map(String::as_str),
            Some(EKAT_FACTION_ID),
            "unknown catalog ids must be ignored"
        );
    }

    #[test]
    fn set_faction_is_ignored_for_spectators_countdown_and_in_game() {
        let mut spectator_task = RoomTask::new(
            "faction-spectator-policy".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        add_test_room_spectator(&mut spectator_task, 1);
        spectator_task.on_set_faction(1, EKAT_FACTION_ID.to_string());
        assert!(
            !spectator_task.human_faction_assignments.contains_key(&1),
            "spectator setFaction requests must not create active-seat faction state"
        );

        let mut countdown_task = RoomTask::new(
            "faction-countdown-policy".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        add_test_room_player(&mut countdown_task, 1, true);
        countdown_task.assign_missing_faction_for(1);
        countdown_task.match_countdown_deadline = Some(TokioInstant::now());
        countdown_task.on_set_faction(1, EKAT_FACTION_ID.to_string());
        assert_eq!(
            countdown_task
                .human_faction_assignments
                .get(&1)
                .map(String::as_str),
            Some(DEFAULT_FACTION_ID),
            "countdown setFaction requests must preserve the pre-countdown selection"
        );

        let players = replay_test_players(2);
        let mut in_game_task = RoomTask::new(
            "faction-in-game-policy".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        add_test_room_player(&mut in_game_task, 1, true);
        in_game_task.assign_missing_faction_for(1);
        in_game_task.phase = Phase::InGame(Box::new(replay_test_game(&players, 0)));
        in_game_task.on_set_faction(1, EKAT_FACTION_ID.to_string());
        assert_eq!(
            in_game_task
                .human_faction_assignments
                .get(&1)
                .map(String::as_str),
            Some(DEFAULT_FACTION_ID),
            "in-game setFaction requests must not mutate active match faction state"
        );
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

    #[test]
    fn host_can_move_another_human_to_spectators() {
        let mut task = RoomTask::new(
            "host-spectator-target".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        task.host_id = Some(1);
        add_test_room_player(&mut task, 1, true);
        add_test_room_player(&mut task, 2, true);
        add_test_room_player(&mut task, 3, true);
        task.human_team_assignments.insert(2, 2);
        task.human_faction_assignments
            .insert(2, "kriegsia".to_string());

        task.on_set_spectator(3, 2, true);
        assert!(
            !task.players.get(&2).unwrap().spectator,
            "non-host targeted spectator move must be ignored"
        );

        task.on_set_spectator(1, 2, true);

        let target = task.players.get(&2).unwrap();
        assert!(target.spectator);
        assert!(!target.ready);
        assert_eq!(target.color, "#6f8fa8");
        assert!(!task.human_team_assignments.contains_key(&2));
        assert!(!task.human_faction_assignments.contains_key(&2));
    }

    #[test]
    fn host_can_move_spectator_back_to_active_lobby_seat() {
        let mut task = RoomTask::new(
            "host-spectator-return".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        task.host_id = Some(1);
        add_test_room_player(&mut task, 1, true);
        add_test_room_spectator(&mut task, 2);
        task.human_team_assignments.insert(1, 1);

        task.on_set_spectator(1, 2, false);

        let target = task.players.get(&2).unwrap();
        assert!(!target.spectator);
        assert!(!target.ready);
        assert_ne!(target.color, "#6f8fa8");
        assert_eq!(task.human_team_assignments.get(&2), Some(&2));
        assert_eq!(
            task.human_faction_assignments.get(&2).map(String::as_str),
            Some("kriegsia")
        );
    }

    #[test]
    fn default_ai_team_appends_after_occupied_teams_when_possible() {
        let mut task = RoomTask::new(
            "ai-default-team-append".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        task.host_id = Some(1);
        add_test_room_player(&mut task, 1, true);
        task.human_team_assignments.insert(1, 2);

        assert_eq!(task.next_default_team_for_new_seat(999_999), 3);
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

    fn start_payloads(writer: &mut ConnectionWriter) -> Vec<StartPayload> {
        std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
            .filter_map(|msg| match msg {
                ServerMessage::Start(payload) => Some(payload),
                _ => None,
            })
            .collect()
    }

    fn lab_results(writer: &mut ConnectionWriter) -> Vec<LabResult> {
        std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
            .filter_map(|msg| match msg {
                ServerMessage::LabResult(result) => Some(result),
                _ => None,
            })
            .collect()
    }

    fn branch_staging_messages(writer: &mut ConnectionWriter) -> Vec<ServerMessage> {
        std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
            .filter(|msg| matches!(msg, ServerMessage::BranchStaging { .. }))
            .collect()
    }

    fn snapshot_notice_events(writer: &mut ConnectionWriter) -> Vec<Event> {
        writer
            .snapshots
            .take()
            .map(|snapshot| {
                snapshot
                    .events
                    .into_iter()
                    .filter(|event| matches!(event, Event::Notice { .. }))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn assert_single_late_spectator_notice(writer: &mut ConnectionWriter, expected_msg: &str) {
        let notices = snapshot_notice_events(writer);
        assert_eq!(
            notices
                .iter()
                .filter(|event| matches!(event, Event::Notice { msg, .. } if msg == expected_msg))
                .count(),
            1,
            "expected exactly one notice {expected_msg:?}, got {notices:?}"
        );
        assert!(notices.iter().any(|event| matches!(
            event,
            Event::Notice {
                msg,
                severity: NoticeSeverity::Info,
                x: None,
                y: None
            } if msg == expected_msg
        )));
    }

    fn assert_no_late_spectator_notice(writer: &mut ConnectionWriter, expected_msg: &str) {
        let notices = snapshot_notice_events(writer);
        assert!(
            !notices
                .iter()
                .any(|event| matches!(event, Event::Notice { msg, .. } if msg == expected_msg)),
            "unexpected notice {expected_msg:?}: {notices:?}"
        );
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
            ability_objects: Vec::new(),
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

        task.on_set_room_time_speed(99, 0.0);
        assert_eq!(
            task.current_tick_interval(),
            Duration::from_millis(config::TICK_MS)
        );
        task.on_tick(TokioInstant::now());
        assert_eq!(in_game_tick(&task), 0);

        task.on_set_room_time_speed(99, 1.0);
        task.on_tick(TokioInstant::now());
        assert_eq!(in_game_tick(&task), 1);
    }

    #[test]
    fn room_task_tick_control_preserves_current_intervals_by_mode() {
        let base = Duration::from_millis(config::TICK_MS);

        let normal = RoomTask::new(
            "tick-normal".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        assert_eq!(normal.current_tick_interval(), base);

        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 3);
        let mut replay = ReplaySession::new(artifact).unwrap();
        replay.set_speed(99, 2.0);
        let mut replay_task = RoomTask::new(
            "tick-replay".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        add_test_room_player(&mut replay_task, 99, true);
        replay_task.phase = Phase::ReplayViewer(Box::new(replay));
        assert_eq!(replay_task.current_tick_interval(), base.div_f32(2.0));

        replay_task.on_set_room_time_speed(99, 0.0);
        assert_eq!(replay_task.current_tick_interval(), base);

        let mut dev = RoomTask::new(
            "tick-dev".to_string(),
            RoomMode::DevScenario(DevScenarioConfig {
                id: DevScenarioId::VehicleCornerWall,
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
                case: None,
            }),
            None,
            false,
            DrainHandle::default(),
        );
        add_test_room_player(&mut dev, 99, true);
        dev.on_set_room_time_speed(99, 2.0);
        assert_eq!(dev.current_tick_interval(), base.div_f32(2.0));
        dev.on_set_room_time_speed(99, 0.0);
        assert_eq!(dev.current_tick_interval(), base);

        let seed = replay_branch_test_seed(&players, 1);
        let mut branch = RoomTask::new(
            "tick-branch".to_string(),
            RoomMode::ReplayBranch { seed: seed.clone() },
            None,
            false,
            DrainHandle::default(),
        );
        branch.room_time_speed = 4.0;
        branch.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));
        assert_eq!(branch.current_tick_interval(), base);
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

        task.on_seek_room_time(99, 1);
        let first_seek_messages: Vec<_> =
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(first_seek_messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::Start(payload)
                if payload.capabilities.room_time.seek_relative
                    && payload.capabilities.room_time.seek_absolute
                    && payload.capabilities.visibility.replay_vision
        )));
        assert!(first_seek_messages
            .iter()
            .any(|msg| matches!(msg, ServerMessage::RoomTimeState(_))));

        task.on_seek_room_time(99, 1);
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
        task.send_room_time_state_to(99);
        task.send_observer_analysis_to(99);
        let join_messages: Vec<_> =
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(join_messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::ObserverAnalysis(analysis) if analysis.tick == 3 && analysis.players.len() == 2
        )));

        task.on_seek_room_time_to(99, 1);
        let seek_messages: Vec<_> =
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(seek_messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::Start(payload)
                if payload.capabilities.room_time.seek_relative
                    && payload.capabilities.room_time.seek_absolute
                    && payload.capabilities.visibility.replay_vision
        )));
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
    fn lobby_phase_ignores_gameplay_commands() {
        let mut task = RoomTask::new(
            "lobby-command-readonly-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer = add_test_room_player(&mut task, 1, true);

        task.on_command(
            1,
            1,
            SimCommand::Stop {
                units: vec![1, 2, 3],
            },
        );

        assert!(matches!(task.phase, Phase::Lobby));
        assert!(task.pending_client_command_acks.is_empty());
        assert_eq!(
            task.players.get(&1).unwrap().last_received_client_seq,
            0,
            "lobby-phase commands must not consume client sequence state"
        );
        assert!(
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).any(|msg| {
                matches!(
                    msg,
                    ServerMessage::CommandReceipt {
                        client_seq: 1,
                        accepted: false,
                        reason: Some(reason),
                        ..
                    } if reason == "notInGame"
                )
            })
        );
    }

    #[test]
    fn normal_live_player_commands_use_connection_authority_and_ack_sequence() {
        let mut task = RoomTask::new(
            "live-command-authority-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer = add_test_room_player(&mut task, 1, true);

        task.start_match();
        while writer.reliable_rx.try_recv().is_ok() {}
        task.on_command(
            1,
            1,
            SimCommand::Stop {
                units: vec![1, 2, 3],
            },
        );

        assert_eq!(task.players.get(&1).unwrap().last_received_client_seq, 1);
        assert_eq!(task.pending_client_command_acks.len(), 1);
        assert_eq!(task.pending_client_command_acks[0].connection_id, 1);
        assert_eq!(task.pending_client_command_acks[0].client_seq, 1);
        assert!(
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).any(|msg| {
                matches!(
                    msg,
                    ServerMessage::CommandReceipt {
                        client_seq: 1,
                        accepted: true,
                        ..
                    }
                )
            })
        );

        task.on_tick(TokioInstant::now());

        let Phase::InGame(game) = &task.phase else {
            panic!("normal live match should remain active");
        };
        assert_eq!(game.command_log().len(), 1);
        assert_eq!(game.command_log()[0].player_id, 1);
        assert!(task.pending_client_command_acks.is_empty());
        assert_eq!(
            task.players.get(&1).unwrap().last_sim_consumed_client_seq,
            1
        );
    }

    #[test]
    fn live_pause_authorizes_active_players_and_tracks_limit() {
        let mut task = RoomTask::new(
            "live-pause-authority-test".to_string(),
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

        task.on_pause_game(99);
        assert!(!task.live_paused, "spectators must not pause live matches");

        task.on_pause_game(1);
        assert!(task.live_paused);
        assert_eq!(task.live_paused_by, Some(1));
        assert_eq!(task.live_pause_counts.get(&1), Some(&1));
        let active_state = std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok())
            .find_map(|msg| match msg {
                ServerMessage::LivePauseState(state) => Some(state),
                _ => None,
            })
            .expect("active pause state");
        assert_eq!(active_state.pauses_remaining, Some(2));
        assert!(!active_state.can_pause);
        assert!(active_state.can_unpause);
        let spectator_state = std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok())
            .find_map(|msg| match msg {
                ServerMessage::LivePauseState(state) => Some(state),
                _ => None,
            })
            .expect("spectator pause state");
        assert_eq!(spectator_state.pauses_remaining, None);
        assert!(!spectator_state.can_unpause);

        task.on_pause_game(1);
        assert_eq!(
            task.live_pause_counts.get(&1),
            Some(&1),
            "repeated pause while paused must not spend another charge"
        );

        for expected_used in 1..=3 {
            if !task.live_paused {
                task.on_pause_game(1);
            }
            assert_eq!(task.live_pause_counts.get(&1), Some(&expected_used));
            task.on_unpause_game(2);
            assert!(!task.live_paused, "any active player can unpause");
        }

        task.on_pause_game(1);
        assert!(
            !task.live_paused,
            "fourth successful pause by one player is denied"
        );
        assert_eq!(task.live_pause_counts.get(&1), Some(&3));
        let denied_state = std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok())
            .filter_map(|msg| match msg {
                ServerMessage::LivePauseState(state) => Some(state),
                _ => None,
            })
            .last()
            .expect("denied pause state");
        assert_eq!(denied_state.pauses_remaining, Some(0));
        assert!(!denied_state.can_pause);
        drop(writer_b);
    }

    #[test]
    fn live_pause_skips_live_tick_work_until_unpaused() {
        let mut task = RoomTask::new(
            "live-pause-tick-skip-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer = add_test_room_player(&mut task, 1, true);
        add_test_room_player(&mut task, 2, true);

        task.start_match();
        while writer.reliable_rx.try_recv().is_ok() {}
        task.on_command(
            1,
            1,
            SimCommand::Stop {
                units: vec![1, 2, 3],
            },
        );
        task.on_pause_game(1);
        task.on_tick(TokioInstant::now());
        let Phase::InGame(game) = &task.phase else {
            panic!("normal live match should remain active");
        };
        assert_eq!(
            game.tick_count(),
            0,
            "paused scheduled tick must not advance sim"
        );
        assert_eq!(
            task.pending_client_command_acks.len(),
            1,
            "paused scheduled tick must not consume command acks"
        );

        task.on_unpause_game(2);
        task.on_tick(TokioInstant::now());
        let Phase::InGame(game) = &task.phase else {
            panic!("normal live match should remain active");
        };
        assert_eq!(game.tick_count(), 1);
        assert!(task.pending_client_command_acks.is_empty());
    }

    #[test]
    fn defeated_live_players_cannot_issue_more_commands() {
        let mut task = RoomTask::new(
            "defeated-command-authority-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        add_test_room_player(&mut task, 1, true);
        add_test_room_player(&mut task, 2, true);

        task.start_match();
        task.outcome_sent.insert(1);
        task.on_command(
            1,
            1,
            SimCommand::Stop {
                units: vec![1, 2, 3],
            },
        );

        let Phase::InGame(game) = &task.phase else {
            panic!("normal live match should remain active");
        };
        assert!(game.command_log().is_empty());
        assert!(task.pending_client_command_acks.is_empty());
        assert_eq!(task.players.get(&1).unwrap().last_received_client_seq, 0);
    }

    #[test]
    fn normal_live_start_payloads_stamp_active_players_and_spectators() {
        let mut task = RoomTask::new(
            "live-start-payload-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_player = add_test_room_player(&mut task, 1, true);
        let mut writer_spectator = add_test_room_spectator(&mut task, 99);

        task.start_match();

        let player_starts = start_payloads(&mut writer_player);
        assert_eq!(player_starts.len(), 1);
        let player_payload = &player_starts[0];
        assert_eq!(player_payload.player_id, 1);
        assert!(!player_payload.spectator);
        assert!(player_payload.prediction_build_id.is_some());
        assert_eq!(
            player_payload.prediction_version,
            PREDICTION_PROTOCOL_VERSION
        );
        assert!(player_payload.replay.is_none());
        assert!(player_payload.lab.is_none());
        assert!(player_payload.diagnostics.is_empty());

        let spectator_starts = start_payloads(&mut writer_spectator);
        assert_eq!(spectator_starts.len(), 1);
        let spectator_payload = &spectator_starts[0];
        assert_eq!(spectator_payload.player_id, 99);
        assert!(spectator_payload.spectator);
        assert!(spectator_payload.prediction_build_id.is_none());
        assert_eq!(spectator_payload.prediction_version, 0);
        assert!(spectator_payload.replay.is_none());
        assert!(spectator_payload.lab.is_none());
        assert_eq!(
            spectator_payload.diagnostics.movement_paths,
            MovementPathDiagnosticScope::None
        );
        assert!(spectator_payload.diagnostics.observer_analysis);
    }

    #[test]
    fn debug_mode_start_payloads_advertise_owner_only_movement_diagnostics() {
        let mut task = RoomTask::new(
            "debug-diagnostics-start-payload-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_player = add_test_room_player(&mut task, 1, true);
        let mut writer_spectator = add_test_room_spectator(&mut task, 99);
        task.host_id = Some(1);
        task.on_set_quickstart(1, true);

        task.start_match();

        let player_payload = start_payloads(&mut writer_player)
            .pop()
            .expect("active player should receive start");
        assert_eq!(
            player_payload.diagnostics.movement_paths,
            MovementPathDiagnosticScope::OwnerOnly
        );
        assert!(!player_payload.diagnostics.observer_analysis);

        let spectator_payload = start_payloads(&mut writer_spectator)
            .pop()
            .expect("spectator should receive start");
        assert_eq!(
            spectator_payload.diagnostics.movement_paths,
            MovementPathDiagnosticScope::None
        );
        assert!(spectator_payload.diagnostics.observer_analysis);
    }

    #[test]
    fn lab_room_join_launches_real_game_with_lab_start_metadata() {
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert!(matches!(task.phase, Phase::InGame(_)));
        assert_eq!(task.match_player_count, 2);
        assert_eq!(task.match_human_count, 0);
        assert!(!task.session_policy().allows_match_history());
        let session = task.lab_session.as_ref().expect("lab session");
        assert_eq!(session.operator_id, 99);
        assert_eq!(session.role_for(99), LabStartRole::Operator);

        let starts = start_payloads(&mut writer);
        assert_eq!(starts.len(), 1);
        let payload = &starts[0];
        assert_eq!(payload.player_id, LAB_PLAYER_ONE_ID);
        assert!(payload.spectator);
        assert!(payload.prediction_build_id.is_none());
        assert_eq!(payload.prediction_version, 0);
        assert!(payload.replay.is_none());
        assert_eq!(payload.players.len(), 2);
        assert_eq!(payload.players[0].team_id, 1);
        assert_eq!(payload.players[1].team_id, 2);
        let lab = payload.lab.as_ref().expect("lab metadata");
        assert_eq!(lab.room, "sandbox");
        assert_eq!(lab.operator_id, 99);
        assert_eq!(lab.role, LabStartRole::Operator);
        assert_eq!(lab.vision, LabVisionMode::FullWorld);
        assert!(!lab.dirty);
        assert_eq!(lab.operation_count, 0);
    }

    #[test]
    fn lab_room_additional_joiner_gets_operator_lab_start() {
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        let (operator_tx, _operator_writer) = ConnectionSink::new();
        let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(
            99,
            "Operator".to_string(),
            true,
            false,
            operator_tx,
            operator_ack,
        );

        let (viewer_tx, mut viewer_writer) = ConnectionSink::new();
        let (viewer_ack, mut viewer_ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(
            100,
            "Viewer".to_string(),
            true,
            false,
            viewer_tx,
            viewer_ack,
        );

        assert_eq!(viewer_ack_rx.try_recv(), Ok(true));
        let starts = start_payloads(&mut viewer_writer);
        assert_eq!(starts.len(), 1);
        let lab = starts[0].lab.as_ref().expect("lab metadata");
        assert_eq!(lab.operator_id, 99);
        assert_eq!(lab.role, LabStartRole::Operator);
        assert_eq!(lab.vision, LabVisionMode::FullWorld);
    }

    #[test]
    fn lab_room_snapshot_uses_full_world_projection() {
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, _ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
        while writer.reliable_rx.try_recv().is_ok() {}

        task.on_tick(TokioInstant::now());

        let snapshot = writer.snapshots.take().expect("lab snapshot");
        let Phase::InGame(game) = &task.phase else {
            panic!("lab should remain live");
        };
        let mut expected = game.snapshot_full_for(LAB_PLAYER_ONE_ID);
        compact_snapshot_for_wire(&mut expected);
        assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
        assert!(snapshot.visible_tiles.is_empty());
        assert_eq!(snapshot.entities.len(), expected.entities.len());
        assert_eq!(snapshot.player_resources, expected.player_resources);
        assert_eq!(snapshot.net_status.prediction_version, 0);
    }

    #[test]
    fn lab_operator_mutation_returns_result_broadcasts_state_and_logs() {
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, _ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
        while writer.reliable_rx.try_recv().is_ok() {}

        task.on_lab_request(
            99,
            7,
            LabClientOp::SetPlayerResources {
                player_id: LAB_PLAYER_ONE_ID,
                steel: 1234,
                oil: 55,
            },
        );

        let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        let results: Vec<_> = messages
            .iter()
            .filter_map(|msg| match msg {
                ServerMessage::LabResult(result) => Some(result),
                _ => None,
            })
            .collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].ok);
        assert_eq!(results[0].request_id, 7);
        assert_eq!(results[0].op, "setPlayerResources");
        let states: Vec<_> = messages
            .iter()
            .filter_map(|msg| match msg {
                ServerMessage::LabState(state) => Some(state),
                _ => None,
            })
            .collect();
        assert_eq!(states.len(), 1);
        assert!(states[0].dirty);
        assert_eq!(states[0].operation_count, 1);
        let session = task.lab_session.as_ref().unwrap();
        assert_eq!(session.operation_log.len(), 1);
        assert_eq!(session.operation_log[0].request_id, 7);
        assert_eq!(session.operation_log[0].operator_id, 99);
        assert_eq!(session.operation_log[0].tick, 0);
        assert_eq!(session.operation_log[0].op, "setPlayerResources");
        assert!(session.operation_log[0].result.contains("playerId"));
    }

    #[test]
    fn lab_collaborators_can_mutate_issue_commands_and_log_requester() {
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        let (operator_tx, mut operator_writer) = ConnectionSink::new();
        let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(
            99,
            "Operator".to_string(),
            true,
            false,
            operator_tx,
            operator_ack,
        );
        let (collab_tx, mut collab_writer) = ConnectionSink::new();
        let (collab_ack, _collab_ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(
            100,
            "Collaborator".to_string(),
            true,
            false,
            collab_tx,
            collab_ack,
        );
        while operator_writer.reliable_rx.try_recv().is_ok() {}
        while collab_writer.reliable_rx.try_recv().is_ok() {}

        task.on_lab_request(
            99,
            30,
            LabClientOp::SetPlayerResources {
                player_id: LAB_PLAYER_ONE_ID,
                steel: 456,
                oil: 78,
            },
        );
        assert!(lab_results(&mut operator_writer)[0].ok);

        let Phase::InGame(game) = &task.phase else {
            panic!("lab should be running");
        };
        let worker = game
            .snapshot_full_for(LAB_PLAYER_ONE_ID)
            .entities
            .iter()
            .find(|entity| {
                entity.owner == LAB_PLAYER_ONE_ID && entity.kind == crate::protocol::kinds::WORKER
            })
            .unwrap()
            .id;

        task.on_lab_request(
            100,
            31,
            LabClientOp::IssueCommandAs {
                player_id: LAB_PLAYER_ONE_ID,
                cmd: Command::Stop {
                    units: vec![worker],
                },
            },
        );
        assert!(lab_results(&mut collab_writer)[0].ok);

        let session = task.lab_session.as_ref().unwrap();
        assert_eq!(session.role_for(99), LabStartRole::Operator);
        assert_eq!(session.role_for(100), LabStartRole::Operator);
        assert_eq!(session.operation_log.len(), 2);
        assert_eq!(session.operation_log[0].request_id, 30);
        assert_eq!(session.operation_log[0].operator_id, 99);
        assert_eq!(session.operation_log[0].op, "setPlayerResources");
        assert_eq!(session.operation_log[1].request_id, 31);
        assert_eq!(session.operation_log[1].operator_id, 100);
        assert_eq!(session.operation_log[1].op, "issueCommandAs");
    }

    #[test]
    fn lab_scenario_export_and_import_round_trip_through_room_ops() {
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, _ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
        while writer.reliable_rx.try_recv().is_ok() {}

        task.on_lab_request(
            99,
            20,
            LabClientOp::SetPlayerResources {
                player_id: LAB_PLAYER_ONE_ID,
                steel: 777,
                oil: 66,
            },
        );
        assert!(lab_results(&mut writer)[0].ok);

        task.on_lab_request(
            99,
            21,
            LabClientOp::SetVision {
                vision: LabVisionMode::Team { team_id: 2 },
            },
        );
        assert!(lab_results(&mut writer)[0].ok);

        task.on_lab_request(
            99,
            22,
            LabClientOp::ExportScenario {
                name: Some("saved setup".to_string()),
            },
        );
        let export_result = lab_results(&mut writer).pop().expect("export result");
        assert!(export_result.ok);
        let scenario: crate::protocol::LabScenarioV1 = serde_json::from_value(
            export_result
                .outcome
                .as_ref()
                .and_then(|outcome| outcome.get("scenario"))
                .cloned()
                .expect("scenario outcome"),
        )
        .expect("scenario JSON");
        assert_eq!(scenario.kind, "labScenario");
        assert_eq!(scenario.name, "saved setup");
        assert_eq!(
            scenario.metadata.lab.vision,
            LabVisionMode::Team { team_id: 2 }
        );
        assert!(scenario.players.iter().any(|player| {
            player.id == LAB_PLAYER_ONE_ID && player.steel == 777 && player.oil == 66
        }));

        task.on_lab_request(
            99,
            23,
            LabClientOp::SetPlayerResources {
                player_id: LAB_PLAYER_ONE_ID,
                steel: 1,
                oil: 1,
            },
        );
        assert!(lab_results(&mut writer)[0].ok);

        task.on_lab_request(99, 24, LabClientOp::ImportScenario { scenario });
        let import_result = lab_results(&mut writer).pop().expect("import result");
        assert!(import_result.ok);
        assert_eq!(import_result.op, "importScenario");
        let Phase::InGame(game) = &task.phase else {
            panic!("lab should still be live after import");
        };
        let snapshot = game.snapshot_full_for(LAB_PLAYER_ONE_ID);
        assert!(snapshot.player_resources.iter().any(|player| {
            player.id == LAB_PLAYER_ONE_ID && player.steel == 777 && player.oil == 66
        }));
        assert_eq!(
            task.lab_session.as_ref().unwrap().vision_mode,
            LabVisionMode::Team { team_id: 2 }
        );
    }

    #[test]
    fn normal_room_rejects_lab_request() {
        let mut normal = RoomTask::new(
            "normal-lab-reject-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut normal_writer = add_test_room_player(&mut normal, 1, true);
        normal.on_lab_request(
            1,
            9,
            LabClientOp::SetVision {
                vision: LabVisionMode::FullWorld,
            },
        );
        let normal_results = lab_results(&mut normal_writer);
        assert_eq!(normal_results.len(), 1);
        assert!(!normal_results[0].ok);
        assert!(normal_results[0]
            .error
            .as_deref()
            .unwrap()
            .contains("lab rooms"));
    }

    #[test]
    fn lab_issue_as_accepts_single_owner_and_rejects_mixed_owner_commands() {
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, _ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
        while writer.reliable_rx.try_recv().is_ok() {}
        let Phase::InGame(game) = &task.phase else {
            panic!("lab should be running");
        };
        let snapshot = game.snapshot_full_for(LAB_PLAYER_ONE_ID);
        let unit_one = snapshot
            .entities
            .iter()
            .find(|entity| {
                entity.owner == LAB_PLAYER_ONE_ID && entity.kind == crate::protocol::kinds::WORKER
            })
            .unwrap()
            .id;
        let unit_two = snapshot
            .entities
            .iter()
            .find(|entity| {
                entity.owner == LAB_PLAYER_TWO_ID && entity.kind == crate::protocol::kinds::WORKER
            })
            .unwrap()
            .id;

        task.on_lab_request(
            99,
            10,
            LabClientOp::IssueCommandAs {
                player_id: LAB_PLAYER_ONE_ID,
                cmd: Command::Stop {
                    units: vec![unit_one],
                },
            },
        );
        task.on_lab_request(
            99,
            11,
            LabClientOp::IssueCommandAs {
                player_id: LAB_PLAYER_ONE_ID,
                cmd: Command::Stop {
                    units: vec![unit_one, unit_two],
                },
            },
        );

        let results = lab_results(&mut writer);
        assert_eq!(results.len(), 2);
        assert!(results[0].ok);
        assert!(!results[1].ok);
        task.on_tick(TokioInstant::now());
        let Phase::InGame(game) = &task.phase else {
            panic!("lab should remain running");
        };
        assert_eq!(game.command_log().len(), 1);
        assert_eq!(game.command_log()[0].player_id, LAB_PLAYER_ONE_ID);
    }

    #[test]
    fn lab_team_vision_uses_server_projection() {
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, _ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
        while writer.reliable_rx.try_recv().is_ok() {}

        task.on_lab_request(
            99,
            12,
            LabClientOp::SetVision {
                vision: LabVisionMode::Team { team_id: 2 },
            },
        );
        assert!(lab_results(&mut writer)[0].ok);
        while writer.reliable_rx.try_recv().is_ok() {}
        task.on_tick(TokioInstant::now());

        let snapshot = writer.snapshots.take().expect("lab team snapshot");
        let Phase::InGame(game) = &task.phase else {
            panic!("lab should remain running");
        };
        let mut expected = game.snapshot_for_spectator(&[LAB_PLAYER_TWO_ID]);
        compact_snapshot_for_wire(&mut expected);
        assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
        assert_eq!(snapshot.entities.len(), expected.entities.len());
    }

    #[test]
    fn empty_lab_room_resets_session_without_changing_lab_mode() {
        let drain = DrainHandle::default();
        let mut task = RoomTask::new(
            "__lab__:sandbox:map=Default".to_string(),
            RoomMode::Lab(lab_config()),
            None,
            false,
            drain.clone(),
        );
        let (msg_tx, _writer) = ConnectionSink::new();
        let (ack, _ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

        task.on_leave(99);

        assert!(matches!(task.phase, Phase::Lobby));
        assert!(task.players.is_empty());
        assert!(task.lab_session.is_none());
        assert_eq!(drain.active_matches(), 0);
        assert!(matches!(task.mode, RoomMode::Lab(_)));
    }

    #[test]
    fn normal_live_spectator_start_payload_is_read_only() {
        let mut task = RoomTask::new(
            "live-spectator-readonly-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer_player = add_test_room_player(&mut task, 1, true);
        let mut writer_spectator = add_test_room_spectator(&mut task, 99);

        task.start_match();
        let start_messages: Vec<_> =
            std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
        assert!(start_messages.iter().any(|msg| {
            matches!(
                msg,
                ServerMessage::Start(payload)
                    if payload.player_id == 99
                        && payload.spectator
                        && payload.prediction_build_id.is_none()
                        && payload.prediction_version == 0
                        && payload.replay.is_none()
            )
        }));

        task.on_command(
            99,
            1,
            SimCommand::Stop {
                units: vec![1, 2, 3],
            },
        );
        task.on_tick(TokioInstant::now());

        let Phase::InGame(game) = &task.phase else {
            panic!("normal live match should remain active");
        };
        assert!(game.command_log().is_empty());
        assert!(task.pending_client_command_acks.is_empty());
        assert_eq!(task.players.get(&99).unwrap().last_received_client_seq, 0);
        let snapshot = writer_spectator
            .snapshots
            .take()
            .expect("spectator snapshot");
        assert_eq!(snapshot.net_status.prediction_version, 0);
        assert_eq!(snapshot.net_status.last_sim_consumed_client_seq, 0);
    }

    #[test]
    fn late_spectator_join_gets_read_only_live_start_and_snapshot() {
        let mut task = RoomTask::new(
            "late-spectator-live-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer_player = add_test_room_player(&mut task, 1, true);
        task.start_match();
        task.on_tick(TokioInstant::now());
        let current_tick = in_game_tick(&task);

        let (msg_tx, mut writer_spectator) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Late Spectator".to_string(), true, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        let player = task.players.get(&99).expect("late spectator inserted");
        assert!(player.spectator);
        assert!(player.ready);
        assert_eq!(player.color, "#6f8fa8");
        assert!(!task.human_team_assignments.contains_key(&99));
        assert!(!task.human_faction_assignments.contains_key(&99));
        assert_eq!(task.match_player_count, 1);
        assert_eq!(task.active_human_count(), 1);

        let payload = start_payloads(&mut writer_spectator)
            .pop()
            .expect("late spectator start payload");
        assert_eq!(payload.player_id, 99);
        assert!(payload.spectator);
        assert!(payload.prediction_build_id.is_none());
        assert_eq!(payload.prediction_version, 0);
        assert_eq!(payload.tick, current_tick);
        assert_eq!(payload.players.len(), 1);
        assert_eq!(payload.players[0].id, 1);
        assert!(!payload.capabilities.commands.gameplay);
        assert!(!payload.capabilities.match_controls.pause);
        assert!(payload.diagnostics.observer_analysis);

        task.on_tick(TokioInstant::now());
        let snapshot = writer_spectator
            .snapshots
            .take()
            .expect("late spectator snapshot");
        let Phase::InGame(game) = &task.phase else {
            panic!("normal live match should remain active");
        };
        let mut expected = game.snapshot_for_spectator(&[1]);
        compact_snapshot_for_wire(&mut expected);
        assert_eq!(snapshot.tick, expected.tick);
        assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
        assert_eq!(snapshot.player_resources, expected.player_resources);
        assert_eq!(snapshot.net_status.prediction_version, 0);
        assert_eq!(snapshot.net_status.last_sim_consumed_client_seq, 0);
        let tick_messages: Vec<_> =
            std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
        assert!(tick_messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::ObserverAnalysis(analysis)
                if analysis.tick == expected.tick && !analysis.players.is_empty()
        )));
    }

    #[test]
    fn late_spectator_notice_targets_existing_recipients_once() {
        let mut task = RoomTask::new(
            "late-spectator-notice-targeting-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_active_one = add_test_room_player(&mut task, 1, true);
        let mut writer_active_two = add_test_room_player(&mut task, 2, true);
        let mut writer_existing_spectator = add_test_room_spectator(&mut task, 50);

        task.start_match();
        let _ = start_payloads(&mut writer_active_one);
        let _ = start_payloads(&mut writer_active_two);
        let _ = start_payloads(&mut writer_existing_spectator);
        task.on_tick(TokioInstant::now());
        let _ = writer_active_one.snapshots.take();
        let _ = writer_active_two.snapshots.take();
        let _ = writer_existing_spectator.snapshots.take();

        let (msg_tx, mut writer_new_spectator) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Late Scout".to_string(), true, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert_eq!(task.match_player_count, 2);
        assert_eq!(task.match_human_count, 2);
        assert_eq!(
            task.match_participants,
            vec!["Player 1".to_string(), "Player 2".to_string()]
        );
        let summary = task
            .lobby_summary()
            .expect("live room should stay in the public browser");
        assert_eq!(summary.spectator_count, 2);

        task.on_tick(TokioInstant::now());
        let expected = "Late Scout has joined the match as a spectator";
        assert_single_late_spectator_notice(&mut writer_active_one, expected);
        assert_single_late_spectator_notice(&mut writer_active_two, expected);
        assert_single_late_spectator_notice(&mut writer_existing_spectator, expected);
        assert_no_late_spectator_notice(&mut writer_new_spectator, expected);
        assert!(task.pending_recipient_notices.is_empty());

        task.on_tick(TokioInstant::now());
        assert_no_late_spectator_notice(&mut writer_active_one, expected);
        assert_no_late_spectator_notice(&mut writer_active_two, expected);
        assert_no_late_spectator_notice(&mut writer_existing_spectator, expected);
        assert_no_late_spectator_notice(&mut writer_new_spectator, expected);
    }

    #[test]
    fn late_spectator_notice_uses_commander_for_blank_or_control_name() {
        let mut task = RoomTask::new(
            "late-spectator-notice-commander-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_active = add_test_room_player(&mut task, 1, true);
        task.start_match();
        let _ = start_payloads(&mut writer_active);
        task.on_tick(TokioInstant::now());
        let _ = writer_active.snapshots.take();

        let (msg_tx, mut writer_new_spectator) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, " \n\u{0007}\t ".to_string(), true, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        task.on_tick(TokioInstant::now());
        let expected = "Commander has joined the match as a spectator";
        assert_single_late_spectator_notice(&mut writer_active, expected);
        assert_no_late_spectator_notice(&mut writer_new_spectator, expected);
    }

    #[test]
    fn late_spectator_notice_is_not_emitted_for_rejected_active_join() {
        let mut task = RoomTask::new(
            "late-spectator-notice-active-reject-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_active = add_test_room_player(&mut task, 1, true);
        task.start_match();
        let _ = start_payloads(&mut writer_active);
        task.on_tick(TokioInstant::now());
        let _ = writer_active.snapshots.take();

        let (msg_tx, mut writer_rejected) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Late Active".to_string(), false, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(false));
        assert!(!task.players.contains_key(&99));
        assert!(task.pending_recipient_notices.is_empty());
        assert!(matches!(
            writer_rejected.reliable_rx.try_recv().unwrap(),
            ServerMessage::Error { msg } if msg.contains("join as a spectator")
        ));

        task.on_tick(TokioInstant::now());
        assert_no_late_spectator_notice(
            &mut writer_active,
            "Late Active has joined the match as a spectator",
        );
    }

    #[test]
    fn late_spectator_notice_queues_while_live_paused() {
        let mut task = RoomTask::new(
            "late-spectator-notice-live-pause-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_active_one = add_test_room_player(&mut task, 1, true);
        let mut writer_active_two = add_test_room_player(&mut task, 2, true);

        task.start_match();
        let _ = start_payloads(&mut writer_active_one);
        let _ = start_payloads(&mut writer_active_two);
        task.on_tick(TokioInstant::now());
        let _ = writer_active_one.snapshots.take();
        let _ = writer_active_two.snapshots.take();
        task.on_pause_game(1);
        assert!(task.live_paused);

        let (msg_tx, mut writer_new_spectator) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Paused Scout".to_string(), true, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert!(!task.pending_recipient_notices.is_empty());
        task.on_tick(TokioInstant::now());
        assert!(
            writer_active_one.snapshots.take().is_none(),
            "paused live ticks should not fan out snapshots"
        );
        assert!(
            writer_active_two.snapshots.take().is_none(),
            "paused live ticks should not fan out snapshots"
        );

        task.on_unpause_game(2);
        task.on_tick(TokioInstant::now());
        let expected = "Paused Scout has joined the match as a spectator";
        assert_single_late_spectator_notice(&mut writer_active_one, expected);
        assert_single_late_spectator_notice(&mut writer_active_two, expected);
        assert_no_late_spectator_notice(&mut writer_new_spectator, expected);
        assert!(task.pending_recipient_notices.is_empty());
    }

    #[test]
    fn late_spectator_notice_lifecycle_keeps_active_match_counts() {
        let mut task = RoomTask::new(
            "late-spectator-notice-lifecycle-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let mut writer_active_one = add_test_room_player(&mut task, 1, true);
        let mut writer_active_two = add_test_room_player(&mut task, 2, true);
        task.start_match();
        let _ = start_payloads(&mut writer_active_one);
        let _ = start_payloads(&mut writer_active_two);
        task.on_tick(TokioInstant::now());
        let _ = writer_active_one.snapshots.take();
        let _ = writer_active_two.snapshots.take();

        let (msg_tx, _writer_late_spectator) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Lifecycle Scout".to_string(), true, false, msg_tx, ack);
        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert_eq!(task.match_player_count, 2);
        assert_eq!(task.active_human_count(), 2);
        assert_eq!(
            task.match_participants,
            vec!["Player 1".to_string(), "Player 2".to_string()]
        );

        let before_alive = match &task.phase {
            Phase::InGame(game) => game.alive_players(),
            _ => panic!("expected live match"),
        };
        assert_eq!(before_alive.len(), 2);

        task.on_leave(99);
        let summary = task
            .lobby_summary()
            .expect("live room should stay in the public browser after spectator leaves");
        assert_eq!(summary.spectator_count, 0);
        assert_eq!(task.match_player_count, 2);
        assert_eq!(task.active_human_count(), 2);
        let after_alive = match &task.phase {
            Phase::InGame(game) => game.alive_players(),
            _ => panic!("expected live match"),
        };
        assert_eq!(after_alive, before_alive);
    }

    #[test]
    fn late_spectator_phase_rejects_active_joins_without_claiming_socket() {
        let mut task = RoomTask::new(
            "late-spectator-active-reject-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer_player = add_test_room_player(&mut task, 1, true);
        task.start_match();

        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
        task.on_join(99, "Late Active".to_string(), false, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(false));
        assert!(!task.players.contains_key(&99));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::Error { msg } if msg.contains("join as a spectator")
        ));

        let mut other = RoomTask::new(
            "late-spectator-retry-room".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let (retry_tx, _retry_writer) = ConnectionSink::new();
        let (retry_ack, mut retry_ack_rx) = tokio::sync::oneshot::channel();
        other.on_join(
            99,
            "Late Active".to_string(),
            false,
            false,
            retry_tx,
            retry_ack,
        );

        assert_eq!(retry_ack_rx.try_recv(), Ok(true));
        assert!(other.players.contains_key(&99));
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
        assert_eq!(staging.claimant_for_seat(players[1].id), None);
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
    fn branch_launch_preparation_preserves_original_replay_seat_mapping() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 0);
        let mut staging = BranchStagingState::new(seed);
        staging.claim(101, players[1].id).unwrap();
        staging.claim(100, players[0].id).unwrap();

        let launch = staging
            .prepare_launch(|connection_id| matches!(connection_id, 100 | 101))
            .unwrap();

        assert_eq!(launch.seat_by_connection.get(&100), Some(&players[0].id));
        assert_eq!(launch.seat_by_connection.get(&101), Some(&players[1].id));
        assert_eq!(
            launch.participants,
            vec![players[0].name.clone(), players[1].name.clone()]
        );
        assert_eq!(launch.game.tick_count(), 0);
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
        task.on_set_spectator(100, 100, false);
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
    fn branch_launch_countdown_promotes_to_live_start_payloads() {
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
                    && payload.prediction_build_id.is_some()
                    && payload.prediction_version == PREDICTION_PROTOCOL_VERSION
                    && payload.replay.is_none()
                    && payload.players.iter().all(|player| player.faction_id == DEFAULT_FACTION_ID))
        }));
        let starts_spectator: Vec<_> =
            std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
        assert!(starts_spectator.iter().any(|msg| {
            matches!(
                msg,
                ServerMessage::Start(payload)
                    if payload.player_id == 102
                        && payload.spectator
                        && payload.prediction_build_id.is_none()
                        && payload.prediction_version == 0
                        && payload.replay.is_none()
            )
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
        assert_eq!(task.pending_client_command_acks.len(), 1);
        assert_eq!(task.pending_client_command_acks[0].connection_id, 100);
        assert_eq!(task.pending_client_command_acks[0].client_seq, 1);
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
            task.players.get(&100).unwrap().last_sim_consumed_client_seq,
            1
        );
        assert_eq!(task.players.get(&102).unwrap().last_received_client_seq, 0);
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

        assert!(!task.should_persist_match_history());
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
    fn match_history_persistence_allows_solo_and_human_ai_matches() {
        let mut solo = RoomTask::new(
            "solo-history-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        solo.match_player_count = 1;
        solo.match_human_count = 1;
        solo.match_participants = vec!["Player".to_string()];
        assert!(solo.should_persist_match_history());

        let mut human_ai = RoomTask::new(
            "human-ai-history-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        human_ai.match_player_count = 2;
        human_ai.match_human_count = 1;
        human_ai.match_participants = vec!["Player".to_string(), "Computer 1".to_string()];
        assert!(human_ai.should_persist_match_history());
    }

    #[test]
    fn match_history_persistence_allows_ai_only_but_skips_test_matches() {
        let mut ai_only = RoomTask::new(
            "ai-only-history-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        ai_only.match_player_count = 2;
        ai_only.match_human_count = 0;
        ai_only.match_participants = vec!["Computer 1".to_string(), "Computer 2".to_string()];
        assert!(ai_only.should_persist_match_history());

        let mut smoke = RoomTask::new(
            "smoke-history-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        smoke.match_player_count = 2;
        smoke.match_human_count = 1;
        smoke.match_participants = vec!["smoke".to_string(), "Computer 1".to_string()];
        assert!(!smoke.should_persist_match_history());

        let mut automated_room = RoomTask::new(
            "itest-history-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        automated_room.match_player_count = 2;
        automated_room.match_human_count = 2;
        automated_room.match_participants = vec!["Player 1".to_string(), "Player 2".to_string()];
        assert!(!automated_room.should_persist_match_history());
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
    fn empty_live_room_clears_lifecycle_bookkeeping_and_drain_tracking() {
        let drain = DrainHandle::default();
        let mut task = RoomTask::new(
            "live-empty-test".to_string(),
            RoomMode::Normal,
            None,
            false,
            drain.clone(),
        );
        let (msg_tx, _writer) = ConnectionSink::new();
        let (ack_tx, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(1, "Player 1".to_string(), false, false, msg_tx, ack_tx);
        assert_eq!(ack_rx.try_recv(), Ok(true));
        task.on_ready(1, true);
        task.on_start_request(1);

        assert!(matches!(task.phase, Phase::InGame(_)));
        assert_eq!(drain.active_matches(), 1);
        assert!(task.match_started_at.is_some());
        assert!(task.match_run_id.is_some());
        assert_eq!(task.match_player_count, 1);
        assert_eq!(task.match_human_count, 1);
        assert!(!task.match_map_name.is_empty());
        assert_eq!(task.match_participants, vec!["Player 1".to_string()]);

        task.on_leave(1);

        assert!(matches!(task.phase, Phase::Lobby));
        assert_eq!(drain.active_matches(), 0);
        assert!(!task.match_tracked_for_drain);
        assert!(task.players.is_empty());
        assert_eq!(task.host_id, None);
        assert_eq!(task.match_player_count, 0);
        assert_eq!(task.match_human_count, 0);
        assert!(task.match_started_at.is_none());
        assert!(task.match_run_id.is_none());
        assert!(task.match_map_name.is_empty());
        assert!(task.match_participants.is_empty());
    }

    #[test]
    fn dev_scenario_start_payload_is_read_only_viewer_payload() {
        let mut task = RoomTask::new(
            "dev-start-payload-test".to_string(),
            RoomMode::DevScenario(DevScenarioConfig {
                id: DevScenarioId::VehicleCornerWall,
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
                case: None,
            }),
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(99, "Viewer".to_string(), true, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        let starts = start_payloads(&mut writer);
        assert_eq!(starts.len(), 1);
        let payload = &starts[0];
        assert_eq!(payload.player_id, task.dev_view_player_id.unwrap());
        assert!(payload.spectator);
        assert!(payload.prediction_build_id.is_none());
        assert_eq!(payload.prediction_version, 0);
        assert!(payload.replay.is_none());
        assert_eq!(
            payload.diagnostics.movement_paths,
            MovementPathDiagnosticScope::All
        );
        assert!(!payload.diagnostics.observer_analysis);
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
                case: None,
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

        task.on_set_room_time_speed(99, 0.0);
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::RoomTimeState(state)
                if state.paused && state.speed == 0.0 && state.current_tick == 0
        ));
        task.on_tick(TokioInstant::now());
        assert_eq!(
            in_game_tick(&task),
            0,
            "scheduled ticks should not advance while paused"
        );

        task.on_step_room_time(99);
        assert_eq!(in_game_tick(&task), 1);
        let snapshot = writer.snapshots.take().expect("dev watch snapshot");
        let Phase::InGame(game) = &task.phase else {
            panic!("dev scenario should remain live");
        };
        let expected = game.snapshot_full_for(task.dev_view_player_id.unwrap());
        assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
        assert_eq!(snapshot.net_status.prediction_version, 0);
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::RoomTimeState(state)
                if state.paused && state.speed == 0.0 && state.current_tick == 1
        ));
        task.on_step_room_time(99);
        assert_eq!(in_game_tick(&task), 2);
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::RoomTimeState(state)
                if state.paused && state.speed == 0.0 && state.current_tick == 2
        ));

        task.on_set_room_time_speed(99, 1.0);
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::RoomTimeState(state)
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
            ServerMessage::Start(payload)
                if payload.spectator
                    && payload.replay.is_some()
                    && payload.diagnostics.observer_analysis
        ));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::RoomTimeState(_)
        ));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::ObserverAnalysis(analysis)
                if analysis.tick == 0 && analysis.players.len() == players.len()
        ));

        task.on_tick_replay_viewer(TokioInstant::now());
        let snapshot = writer.snapshots.take().expect("replay viewer snapshot");
        let Phase::ReplayViewer(session) = &task.phase else {
            panic!("confirmed replay join should keep replay viewer active");
        };
        let visible_players = players.iter().map(|player| player.id).collect::<Vec<_>>();
        let expected = session.game.snapshot_for_spectator(&visible_players);
        assert_eq!(snapshot.tick, expected.tick);
        assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
        let tick_messages: Vec<_> =
            std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(tick_messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::ObserverAnalysis(analysis)
                if analysis.tick == expected.tick && analysis.players.len() == players.len()
        )));
    }

    #[test]
    fn saved_artifact_replay_join_uses_replay_viewer_runtime() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 3);
        let artifact_name = format!("room_task_saved_selfplay_{}", std::process::id());
        let artifact_dir = write_selfplay_replay_test_artifact(&artifact_name, &artifact);
        let mut task = RoomTask::new(
            "saved-artifact-replay-test".to_string(),
            RoomMode::ReplayArtifact {
                artifact: artifact_name,
            },
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(99, "Viewer".to_string(), false, true, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        let Phase::ReplayViewer(session) = &task.phase else {
            panic!("saved artifact replay should start the shared replay viewer runtime");
        };
        assert_eq!(session.artifact.command_log, artifact.command_log);
        assert_eq!(session.vision_player_ids_for(99), vec![1, 2]);
        assert!(task.players.get(&99).is_some_and(|p| p.spectator));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::Start(payload)
                if payload.spectator
                    && payload.replay.is_some()
                    && payload.diagnostics.observer_analysis
        ));
        assert!(matches!(
            writer.reliable_rx.try_recv().unwrap(),
            ServerMessage::RoomTimeState(_)
        ));

        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    #[test]
    fn replay_branch_room_join_initializes_staging_and_broadcasts_seats() {
        let players = replay_test_players(2);
        let seed = replay_branch_test_seed(&players, 2);
        let mut task = RoomTask::new(
            "branch-join-baseline-test".to_string(),
            RoomMode::ReplayBranch { seed },
            None,
            false,
            DrainHandle::default(),
        );
        let (msg_tx, mut writer) = ConnectionSink::new();
        let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

        task.on_join(100, "Viewer 100".to_string(), true, false, msg_tx, ack);

        assert_eq!(ack_rx.try_recv(), Ok(true));
        assert_eq!(task.host_id, Some(100));
        assert!(matches!(task.phase, Phase::BranchStaging(_)));
        let updates = branch_staging_messages(&mut writer);
        let Some(ServerMessage::BranchStaging {
            room,
            source_tick,
            host_id,
            seats,
            occupants,
            can_start,
        }) = updates.last()
        else {
            panic!("branch join should broadcast staging state");
        };
        assert_eq!(room, "branch-join-baseline-test");
        assert_eq!(*source_tick, 2);
        assert_eq!(*host_id, 100);
        assert_eq!(seats.len(), players.len());
        assert!(seats.iter().all(|seat| seat.claimant_id.is_none()));
        assert_eq!(occupants.len(), 1);
        assert_eq!(occupants[0].id, 100);
        assert_eq!(occupants[0].name, "Viewer 100");
        assert!(!can_start);
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
        assert_eq!(session.speed(), ReplaySession::DEFAULT_SPEED);
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
            |msg| matches!(msg, ServerMessage::RoomTimeState(state) if state.current_tick == 0)
        ));
        assert!(!b_messages
            .iter()
            .any(|msg| matches!(msg, ServerMessage::GameOver { .. })));
        assert!(b_messages.iter().any(|msg| {
            matches!(msg, ServerMessage::Start(payload) if payload.replay.is_some() && payload.tick == 0)
        }));
        assert!(b_messages.iter().any(
            |msg| matches!(msg, ServerMessage::RoomTimeState(state) if state.current_tick == 0)
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
