//! Wire protocol (JSON + binary snapshots over WebSocket). See `docs/design/protocol.md`.
//!
//! This file is the authoritative Rust side of the contract. `client/src/protocol.js`
//! is its JavaScript mirror — change both together.
//!
//! Tag conventions: top-level messages use `"t"`, commands use `"c"`, events use `"e"`.
//! Coordinates are world pixels (floats) unless the field name ends in `Tile`.

use serde::{Deserialize, Serialize};
use std::fmt;

mod compact_snapshot;
mod contract_metadata;
mod messagepack_frame;

#[cfg(test)]
use contract_metadata::{ability_code, kind_code};

pub use contract_metadata::{
    abilities, ability_object_kinds, kinds, protocol_contract, states, terrain, upgrades,
    CompactSlotSchemas, ProtocolCompactCodes, ProtocolContract, ProtocolMessageTags,
    ProtocolVocabularies, SlotField, SnapshotCodecContract, COMPACT_SNAPSHOT_VERSION,
    COMPACT_UNKNOWN_CODE, PREDICTION_PROTOCOL_VERSION, SNAPSHOT_CODEC_COMPACT_JSON,
    SNAPSHOT_CODEC_MESSAGEPACK_COMPACT, SNAPSHOT_CODEC_VERSION, SNAPSHOT_FRAME_KIND_BINARY,
    SNAPSHOT_FRAME_KIND_TEXT,
};
pub use messagepack_frame::MESSAGEPACK_SNAPSHOT_FRAME_MAGIC;
pub use rts_contract::{
    AbilityCooldownView, AbilityObjectOwnerStateView, AbilityObjectView, ActionCapabilities,
    AttackReveal, CommandCapabilities, DebugPathPoint, DebugPathView, DiagnosticCapabilities,
    EntityView, Event, LabScenarioResearch, LabScenarioResources, LabStartMetadata, LabStartRole,
    LabVisionMode, MapInfo, MatchControlCapabilities, MovementPathDiagnosticScope, NoticeSeverity,
    OrderPlanMarker, PlayerResourceSnapshot, PlayerScore, PlayerStart, RememberedBuildingView,
    ReplayStartMetadata, ResourceDelta, ResourceNode, RoomCapabilities, RoomTimeCapabilities,
    RoomTimeState, SmokeCloudView, Snapshot, SnapshotNetStatus, StartPayload, TeamId,
    VisibilityCapabilities, DEFAULT_FACTION_ID,
};

fn is_false(value: &bool) -> bool {
    !*value
}

// ---------------------------------------------------------------------------
// Client -> Server
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ClientMessage {
    /// Join (or create) a room. `room` defaults to "main" when absent. Spectators must join
    /// before the match starts; they observe only and are not seated in the simulation.
    Join {
        name: String,
        #[serde(default)]
        room: Option<String>,
        #[serde(default)]
        spectator: bool,
        #[serde(rename = "replayOk")]
        #[serde(default)]
        replay_ok: bool,
    },
    /// Toggle ready state in the lobby.
    Ready { ready: bool },
    /// Host requests the match to begin.
    Start,
    /// Host selects a lobby team preset (lobby phase only).
    SetTeamPreset { preset: String },
    /// Host assigns one active lobby seat to a nonzero team id (lobby phase only).
    SetTeam {
        id: u32,
        #[serde(rename = "teamId")]
        team_id: TeamId,
    },
    /// A player selects their own playable faction while in the lobby.
    SetFaction {
        #[serde(rename = "factionId")]
        faction_id: String,
    },
    /// Host adds a computer-controlled opponent to the room (lobby phase only).
    AddAi {
        #[serde(rename = "teamId")]
        #[serde(default)]
        team_id: Option<TeamId>,
        #[serde(rename = "aiProfileId")]
        #[serde(default)]
        ai_profile_id: Option<String>,
    },
    /// Host selects the live AI profile for one AI lobby seat.
    SetAiProfile {
        id: u32,
        #[serde(rename = "aiProfileId")]
        ai_profile_id: String,
    },
    /// Host removes a previously-added AI opponent by its player id (lobby phase only).
    RemoveAi { id: u32 },
    /// Switch between player and spectator role while still in the lobby. `id` is optional for
    /// self-targeting compatibility; host-targeted changes include the target human player id.
    SetSpectator {
        spectator: bool,
        #[serde(default)]
        id: Option<u32>,
    },
    /// Issue a gameplay command (ignored unless in-game).
    Command {
        #[serde(rename = "clientSeq")]
        client_seq: u32,
        cmd: Command,
    },
    /// Give up the current match, removing this player's army and showing the score screen.
    GiveUp,
    /// Pause a live match. Honored only from active live players with pauses remaining.
    PauseGame,
    /// Unpause a paused live match. Honored only from active live players.
    UnpauseGame,
    /// Leave replay playback and return the room to a clean lobby.
    ReturnToLobby,
    /// Latency probe.
    Ping { ts: f64 },
    /// Client-observed network/render health aggregate for server logs.
    NetReport { report: Box<ClientNetReport> },
    /// Set room-controlled time speed. `0` pauses rooms whose clock supports pause.
    SetRoomTimeSpeed { speed: f32 },
    /// Advance room-controlled time by one simulation tick where the clock allows stepping.
    StepRoomTime,
    /// Rewind room-controlled time by `ticks_back` ticks where relative seek is allowed.
    SeekRoomTime {
        #[serde(rename = "ticksBack")]
        ticks_back: u32,
    },
    /// Seek room-controlled time to an absolute simulation tick where absolute seek is allowed.
    SeekRoomTimeTo { tick: u32 },
    /// Select which players' fog to use while viewing a replay. Per-viewer only.
    SetVisionSelection { selection: VisionSelectionRequest },
    /// Privileged lab request envelope. Only lab rooms route these requests.
    Lab {
        #[serde(rename = "requestId")]
        request_id: u32,
        op: LabClientOp,
    },
    /// Request a new practice branch room from this replay room's current authoritative tick.
    RequestBranchFromTick,
    /// Claim one original player seat in a replay branch staging room.
    ClaimBranchSeat {
        #[serde(rename = "playerId")]
        player_id: u32,
    },
    /// Release one original player seat in a replay branch staging room.
    ReleaseBranchSeat {
        #[serde(rename = "playerId")]
        player_id: u32,
    },
    /// Host asks to launch a replay branch from staging.
    StartBranch,
    /// Host selects a map by name (lobby phase only; ignored from non-hosts).
    SelectMap { map: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientNetReport {
    pub schema_version: u8,
    #[serde(default)]
    pub match_run_id: String,
    pub elapsed_ms: u32,
    pub match_tick: u32,
    pub rtt_ms: u16,
    pub rtt_max_ms: u16,
    pub bad_rtt_samples: u32,
    pub snapshot_jitter_ms: u16,
    pub snapshot_gap_max_ms: u16,
    pub jitter_samples: u32,
    pub snapshots: u32,
    #[serde(default)]
    pub snapshot_late_frame_count: u32,
    #[serde(default)]
    pub predicted_snapshot_late_frame_count: u32,
    #[serde(default)]
    pub snapshot_bytes_total: u32,
    #[serde(default)]
    pub snapshot_bytes_max: u32,
    #[serde(default)]
    pub snapshot_bytes_avg: u32,
    #[serde(default)]
    pub snapshot_message_count: u32,
    #[serde(default)]
    pub snapshot_byte_source: String,
    #[serde(default)]
    pub snapshot_codec: String,
    #[serde(default)]
    pub snapshot_codec_version: u16,
    #[serde(default)]
    pub snapshot_frame_kind: String,
    #[serde(default)]
    pub snapshot_bytes_p95: u32,
    #[serde(default)]
    pub snapshot_segment_budget_bytes: u32,
    #[serde(default)]
    pub snapshot_over_segment_budget_count: u32,
    #[serde(default)]
    pub snapshot_over_segment_budget_pct_x100: u16,
    #[serde(default)]
    pub snapshot_parse_max_ms: u16,
    #[serde(default)]
    pub snapshot_parse_p95_ms: u16,
    #[serde(default)]
    pub snapshot_decode_max_ms: u16,
    #[serde(default)]
    pub snapshot_decode_p95_ms: u16,
    #[serde(default)]
    pub websocket_extensions: String,
    #[serde(default)]
    pub websocket_compression: String,
    #[serde(default)]
    pub snapshot_apply_max_ms: u16,
    #[serde(default)]
    pub snapshot_apply_p95_ms: u16,
    #[serde(default)]
    pub prediction_apply_max_ms: u16,
    #[serde(default)]
    pub prediction_apply_p95_ms: u16,
    #[serde(default)]
    pub snapshot_tick_gap_max: u32,
    #[serde(default)]
    pub stale_snapshot_count: u32,
    #[serde(default)]
    pub duplicate_snapshot_count: u32,
    #[serde(default)]
    pub skipped_snapshot_count: u32,
    #[serde(default)]
    pub snapshot_burst_count: u32,
    #[serde(default)]
    pub snapshot_burst_max: u32,
    pub frame_gap_max_ms: u16,
    pub fps_estimate: u16,
    #[serde(default)]
    pub frame_work_max_ms: u16,
    #[serde(default)]
    pub frame_work_p95_ms: u16,
    #[serde(default)]
    pub slow_frame_count: u32,
    #[serde(default)]
    pub worst_frame_phase: String,
    #[serde(default)]
    pub worst_frame_phase_ms: u16,
    #[serde(default)]
    pub renderer_max_ms: u16,
    #[serde(default)]
    pub renderer_p95_ms: u16,
    #[serde(default)]
    pub entity_count: u32,
    #[serde(default)]
    pub selected_count: u16,
    #[serde(default)]
    pub visible_tile_count: u32,
    #[serde(default)]
    pub viewport_width: u16,
    #[serde(default)]
    pub viewport_height: u16,
    #[serde(default)]
    pub device_pixel_ratio_x100: u16,
    #[serde(default)]
    pub command_burst_bucket_ms: u16,
    #[serde(default)]
    pub command_burst_max: u16,
    #[serde(default)]
    pub command_burst_frame_gap_max_ms: u16,
    #[serde(default)]
    pub command_burst_worst_frame_phase: String,
    #[serde(default)]
    pub command_burst_worst_frame_phase_ms: u16,
    pub hidden: bool,
    pub focused: bool,
    #[serde(default)]
    pub desktop_runtime_present: bool,
    #[serde(default)]
    pub native_cursor_bridge_present: bool,
    #[serde(default)]
    pub native_cursor_supported: bool,
    #[serde(default)]
    pub native_cursor_active: bool,
    #[serde(default)]
    pub native_cursor_last_reason: String,
    #[serde(default)]
    pub native_cursor_last_error: String,
    #[serde(default)]
    pub tauri_internals_present: bool,
    #[serde(default)]
    pub tauri_global_present: bool,
    #[serde(default)]
    pub tauri_globals: String,
    pub ws_buffered_bytes: u32,
    pub server_tick_ms: u16,
    pub server_lag_ms: u16,
    pub slow_tick_count: u32,
    pub head_of_line_count: u32,
    #[serde(default)]
    pub prediction_mode: String,
    #[serde(default)]
    pub pending_command_count: u16,
    #[serde(default)]
    pub acknowledged_command_latency_ms: u16,
    #[serde(default)]
    pub commands_issued: u32,
    #[serde(default)]
    pub command_socket_send_accepted: u32,
    #[serde(default)]
    pub command_server_received: u32,
    #[serde(default)]
    pub command_sim_acknowledged: u32,
    #[serde(default)]
    pub command_rejected: u32,
    #[serde(default)]
    pub command_issue_to_server_receipt_latest_ms: u16,
    #[serde(default)]
    pub command_issue_to_server_receipt_max_ms: u16,
    #[serde(default)]
    pub command_issue_to_server_receipt_p95_ms: u16,
    #[serde(default)]
    pub command_server_receipt_to_sim_ack_latest_ms: u16,
    #[serde(default)]
    pub command_server_receipt_to_sim_ack_max_ms: u16,
    #[serde(default)]
    pub command_server_receipt_to_sim_ack_p95_ms: u16,
    #[serde(default)]
    pub command_issue_to_sim_ack_latest_ms: u16,
    #[serde(default)]
    pub command_issue_to_sim_ack_max_ms: u16,
    #[serde(default)]
    pub command_issue_to_sim_ack_p95_ms: u16,
    #[serde(default)]
    pub command_ack_snapshot_received_to_applied_latest_ms: u16,
    #[serde(default)]
    pub command_ack_snapshot_received_to_applied_max_ms: u16,
    #[serde(default)]
    pub command_ack_snapshot_received_to_applied_p95_ms: u16,
    #[serde(default)]
    pub oldest_pending_command_age_ms: u16,
    #[serde(default)]
    pub max_pending_command_count: u16,
    #[serde(default)]
    pub correction_distance_px: u16,
    #[serde(default)]
    pub correction_count: u32,
    #[serde(default)]
    pub prediction_disable_count: u32,
    #[serde(default)]
    pub prediction_disable_user_count: u32,
    #[serde(default)]
    pub prediction_disable_replay_count: u32,
    #[serde(default)]
    pub prediction_disable_spectator_count: u32,
    #[serde(default)]
    pub prediction_disable_compatibility_count: u32,
    #[serde(default)]
    pub prediction_disable_wasm_count: u32,
    #[serde(default)]
    pub prediction_disable_other_count: u32,
    #[serde(default)]
    pub wasm_tick_ms: u16,
    #[serde(default)]
    pub wasm_memory_bytes: u32,
    #[serde(default)]
    pub prediction_replay_ticks: u16,
    #[serde(default)]
    pub prediction_replay_max_ms: u16,
    #[serde(default)]
    pub prediction_replay_max_ticks: u16,
    #[serde(default)]
    pub prediction_replay_budget_exceeded_count: u32,
}

/// A gameplay command. Validated when applied, not when received.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "c", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    Move {
        units: Vec<u32>,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    AttackMove {
        units: Vec<u32>,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    Attack {
        units: Vec<u32>,
        target: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    Deconstruct {
        units: Vec<u32>,
        target: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    SetupAntiTankGuns {
        units: Vec<u32>,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    TearDownAntiTankGuns {
        units: Vec<u32>,
    },
    Charge {
        units: Vec<u32>,
    },
    UseAbility {
        ability: String,
        units: Vec<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        x: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        y: Option<f32>,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    RecastAbility {
        ability: String,
        units: Vec<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_object_id: Option<u32>,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    SetAutocast {
        ability: String,
        units: Vec<u32>,
        enabled: bool,
    },
    Gather {
        units: Vec<u32>,
        node: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    Build {
        units: Vec<u32>,
        building: String,
        tile_x: u32,
        tile_y: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    Train {
        building: u32,
        unit: String,
    },
    Research {
        building: u32,
        upgrade: String,
    },
    Cancel {
        building: u32,
    },
    Stop {
        units: Vec<u32>,
    },
    HoldPosition {
        units: Vec<u32>,
    },
    /// Set or append a unit-producing building rally stage. `kind` defaults to a plain move stage
    /// on the wire; production applies plain rally stages as attack-move for non-workers.
    SetRally {
        building: u32,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VisionSelectionRequest {
    All,
    Player { player_id: u32 },
    Players { player_ids: Vec<u32> },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LabClientOp {
    ExportScenario {
        #[serde(default)]
        name: Option<String>,
    },
    ImportScenario {
        scenario: LabScenarioV1,
    },
    ValidateScenario {
        metadata: LabScenarioAuthoringMetadata,
    },
    SpawnEntity {
        owner: u32,
        kind: String,
        x: f32,
        y: f32,
        #[serde(default)]
        completed: bool,
    },
    DeleteEntity {
        entity_id: u32,
    },
    MoveEntity {
        entity_id: u32,
        x: f32,
        y: f32,
    },
    SetEntityOwner {
        entity_id: u32,
        owner: u32,
    },
    SetPlayerResources {
        player_id: u32,
        steel: u32,
        oil: u32,
    },
    SetPlayerGodMode {
        player_id: u32,
        enabled: bool,
    },
    SetCompletedResearch {
        player_id: u32,
        upgrade: String,
        completed: bool,
    },
    SetVision {
        vision: LabVisionMode,
    },
    IssueCommandAs {
        player_id: u32,
        cmd: Command,
        #[serde(default, skip_serializing_if = "is_false")]
        ignore_command_limits: bool,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioAuthoringMetadata {
    pub slug: String,
    pub name: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioV1 {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub seed: u32,
    pub map: LabScenarioMap,
    pub players: Vec<LabScenarioPlayer>,
    pub entities: Vec<LabScenarioEntity>,
    pub metadata: LabScenarioMetadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioMap {
    pub name: String,
    pub schema_version: u32,
    pub content_hash: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPlayer {
    pub id: u32,
    pub team_id: u32,
    pub faction_id: String,
    pub name: String,
    pub color: String,
    pub is_ai: bool,
    pub resources: LabScenarioResources,
    pub research: LabScenarioResearch,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioEntity {
    pub id: u32,
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub completed: bool,
    pub construction_progress: Option<u32>,
    pub construction_total: Option<u32>,
    pub resource_remaining: Option<u32>,
    #[serde(default)]
    pub facing: Option<f32>,
    #[serde(default)]
    pub weapon_facing: Option<f32>,
    #[serde(default)]
    pub set_up: bool,
    #[serde(default)]
    pub setup_facing: Option<f32>,
    #[serde(default)]
    pub setup_target: Option<LabScenarioPoint>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioMetadata {
    pub exported_tick: u32,
    pub lab: LabScenarioLabMetadata,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioLabMetadata {
    pub vision: LabVisionMode,
}

// ---------------------------------------------------------------------------
// Server -> Client
// ---------------------------------------------------------------------------

/// A map entry sent to clients in `ServerMessage::Lobby`. `name` is the stable key used for map
/// selection; `description` is the text shown in the lobby selector.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableMap {
    pub name: String,
    pub description: String,
}

/// Original replay seat announced when a practice branch room is created.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayBranchSeat {
    pub player_id: u32,
    pub team_id: TeamId,
    pub faction_id: String,
    pub name: String,
    pub color: String,
    pub claimable: bool,
}

/// Original replay seat plus current claimant in a branch staging room.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BranchStagingSeat {
    pub player_id: u32,
    pub team_id: TeamId,
    pub faction_id: String,
    pub name: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimant_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimant_name: Option<String>,
}

/// Human occupant viewing a branch staging room without necessarily claiming a seat.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BranchStagingOccupant {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    Welcome {
        player_id: u32,
    },
    Lobby {
        room: String,
        host_id: u32,
        players: Vec<LobbyPlayer>,
        can_start: bool,
        team_preset: String,
        /// Currently selected map name.
        map: String,
        /// All available maps (populated from disk at broadcast time).
        maps: Vec<AvailableMap>,
    },
    /// Reliable pre-match countdown shown to every lobby participant before the `start` payload.
    MatchCountdown {
        duration_ms: u32,
        words: Vec<String>,
    },
    /// Match start (flattened: carries StartPayload's fields alongside `"t":"start"`).
    Start(StartPayload),
    /// Per-player, fog-filtered world state.
    Snapshot(Snapshot),
    /// Shared room-controlled time cursor/state. Sent reliably outside snapshot cadence.
    RoomTimeState(RoomTimeState),
    /// Authoritative live-match pause state. Sent reliably after start and on every transition.
    LivePauseState(LivePauseState),
    /// Authoritative observer analysis data for replay viewers and live spectators.
    ObserverAnalysis(ObserverAnalysisPayload),
    /// The requested room is currently replay playback. The client should confirm before retrying
    /// `join` with `replayOk: true`.
    JoinReplayPrompt {
        room: String,
    },
    /// A practice branch room was created from the current replay tick.
    BranchFromTickCreated {
        branch_room: String,
        source_tick: u32,
        seats: Vec<ReplayBranchSeat>,
    },
    /// Current state of a replay branch staging room.
    BranchStaging {
        room: String,
        source_tick: u32,
        host_id: u32,
        seats: Vec<BranchStagingSeat>,
        occupants: Vec<BranchStagingOccupant>,
        can_start: bool,
    },
    /// Reliable lab control-plane state. World state still travels through `snapshot`.
    LabState(LabState),
    /// Reliable result for one lab request.
    LabResult(LabResult),
    /// Server shutdown drain has started. Existing matches may continue until the deadline, but
    /// new match starts are disabled.
    ShutdownWarning {
        deadline_unix_ms: u64,
        seconds_remaining: u64,
    },
    GameOver {
        winner_id: Option<u32>,
        winner_team_id: Option<TeamId>,
        /// "won" | "lost" | "draw"
        you: String,
        /// Frozen per-player score snapshot for the score screen.
        scores: Vec<PlayerScore>,
    },
    Pong {
        ts: f64,
    },
    /// Reliable diagnostics-only command receipt. This is not the sim-consumption ack.
    CommandReceipt {
        client_seq: u32,
        server_tick: u32,
        accepted: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Error {
        msg: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LivePauseState {
    pub paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_by: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pauses_remaining: Option<u8>,
    pub pause_limit: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_pause: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_unpause: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabState {
    pub room: String,
    pub operator_id: u32,
    pub role: LabStartRole,
    pub vision: LabVisionMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub god_mode_players: Vec<u32>,
    pub dirty: bool,
    pub operation_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabResult {
    pub request_id: u32,
    pub ok: bool,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisPayload {
    pub tick: u32,
    pub players: Vec<ObserverAnalysisPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisPlayer {
    pub id: u32,
    pub units: Vec<ObserverAnalysisKindCount>,
    pub production: Vec<ObserverAnalysisProduction>,
    pub units_lost: Vec<ObserverAnalysisKindCount>,
    pub resources_lost: ObserverAnalysisResourcesLost,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisKindCount {
    pub kind: String,
    pub count: u32,
    pub steel_value: u32,
    pub oil_value: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisProduction {
    pub building_id: u32,
    pub building_kind: String,
    pub item_kind: String,
    /// `"unit"` or `"upgrade"`.
    pub item_type: String,
    /// 0.0..1.0 completion of the front queued item.
    pub progress: f32,
    pub queue_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObserverAnalysisResourcesLost {
    pub steel: u32,
    pub oil: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyPlayer {
    pub id: u32,
    pub team_id: TeamId,
    pub faction_id: String,
    pub name: String,
    pub ready: bool,
    pub color: String,
    /// True for computer opponents (no socket). The client uses this to label the row and show a
    /// host-only "remove" control instead of a ready indicator.
    pub is_ai: bool,
    /// Live AI profile id for computer opponents. Omitted for human players and spectators.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_profile_id: Option<String>,
    /// True for human observers. Spectators do not count toward match starts or win conditions.
    pub is_spectator: bool,
}

// ---------------------------------------------------------------------------
// Compact snapshot transport encoding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCodec {
    CompactJson,
    MessagePackCompact,
}

impl SnapshotCodec {
    pub fn name(self) -> &'static str {
        match self {
            SnapshotCodec::CompactJson => SNAPSHOT_CODEC_COMPACT_JSON,
            SnapshotCodec::MessagePackCompact => SNAPSHOT_CODEC_MESSAGEPACK_COMPACT,
        }
    }

    pub fn version(self) -> u16 {
        match self {
            SnapshotCodec::CompactJson | SnapshotCodec::MessagePackCompact => {
                SNAPSHOT_CODEC_VERSION
            }
        }
    }

    pub fn frame_kind(self) -> &'static str {
        match self {
            SnapshotCodec::CompactJson => SNAPSHOT_FRAME_KIND_TEXT,
            SnapshotCodec::MessagePackCompact => SNAPSHOT_FRAME_KIND_BINARY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotFrame {
    Text(String),
    Binary(Vec<u8>),
}

impl SnapshotFrame {
    pub fn frame_kind(&self) -> &'static str {
        match self {
            SnapshotFrame::Text(_) => SNAPSHOT_FRAME_KIND_TEXT,
            SnapshotFrame::Binary(_) => SNAPSHOT_FRAME_KIND_BINARY,
        }
    }
}

pub fn default_snapshot_codec() -> SnapshotCodec {
    SnapshotCodec::MessagePackCompact
}

pub fn supported_snapshot_codec(name: &str, version: u16) -> bool {
    name == SNAPSHOT_CODEC_MESSAGEPACK_COMPACT && version == SNAPSHOT_CODEC_VERSION
}

#[derive(Debug)]
pub enum SnapshotEncodeError {
    Json(serde_json::Error),
    UnsupportedNumber,
    ContainerTooLarge(&'static str, usize),
}

impl fmt::Display for SnapshotEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnapshotEncodeError::Json(err) => write!(f, "{err}"),
            SnapshotEncodeError::UnsupportedNumber => {
                write!(f, "compact snapshot contains an unsupported number")
            }
            SnapshotEncodeError::ContainerTooLarge(kind, len) => {
                write!(
                    f,
                    "compact snapshot {kind} length {len} exceeds MessagePack bounds"
                )
            }
        }
    }
}

impl std::error::Error for SnapshotEncodeError {}

impl From<serde_json::Error> for SnapshotEncodeError {
    fn from(value: serde_json::Error) -> Self {
        SnapshotEncodeError::Json(value)
    }
}

pub fn encode_snapshot_frame(
    snapshot: &Snapshot,
    codec: SnapshotCodec,
) -> Result<SnapshotFrame, SnapshotEncodeError> {
    match codec {
        SnapshotCodec::CompactJson => serialize_compact_snapshot(snapshot)
            .map(SnapshotFrame::Text)
            .map_err(SnapshotEncodeError::from),
        SnapshotCodec::MessagePackCompact => {
            serialize_messagepack_compact_snapshot(snapshot).map(SnapshotFrame::Binary)
        }
    }
}

/// Serialize one semantic snapshot as a compact JSON text frame payload.
pub fn serialize_compact_snapshot(snapshot: &Snapshot) -> serde_json::Result<String> {
    compact_snapshot::serialize_compact_snapshot(snapshot)
}

/// Serialize one semantic snapshot as a versioned MessagePack compact binary frame payload.
pub fn serialize_messagepack_compact_snapshot(
    snapshot: &Snapshot,
) -> Result<Vec<u8>, SnapshotEncodeError> {
    compact_snapshot::serialize_messagepack_compact_snapshot(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_contract_metadata_matches_public_constants() {
        let contract = serde_json::to_value(protocol_contract()).unwrap();

        assert_eq!(contract["schemaVersion"], serde_json::json!(1));
        assert_eq!(
            contract["compactSnapshotVersion"],
            serde_json::json!(COMPACT_SNAPSHOT_VERSION)
        );
        assert_eq!(
            contract["predictionProtocolVersion"],
            serde_json::json!(PREDICTION_PROTOCOL_VERSION)
        );
        assert_eq!(
            contract["unknownCodeSentinel"],
            serde_json::json!(COMPACT_UNKNOWN_CODE)
        );
        assert_eq!(
            contract["snapshotCodecs"]["defaultCodec"],
            serde_json::json!(SNAPSHOT_CODEC_MESSAGEPACK_COMPACT)
        );
        assert_eq!(
            contract["compactCodes"]["kind"][kinds::WORKER],
            serde_json::json!(kind_code(kinds::WORKER))
        );
        assert_eq!(
            contract["compactCodes"]["ability"][abilities::EKAT_MAGIC_ANCHOR],
            serde_json::json!(ability_code(abilities::EKAT_MAGIC_ANCHOR))
        );
        assert_eq!(
            contract["compactSlotSchemas"]["entity"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["name"],
            serde_json::json!("buildActive")
        );
    }

    #[test]
    fn client_net_report_deserializes() {
        let raw = r#"{
            "t":"netReport",
            "report":{
                "schemaVersion":1,
                "matchRunId":"main-123",
                "elapsedMs":10000,
                "matchTick":300,
                "rttMs":82,
                "rttMaxMs":240,
                "badRttSamples":3,
                "snapshotJitterMs":48,
                "snapshotGapMaxMs":420,
                "jitterSamples":12,
                "snapshots":289,
                "snapshotLateFrameCount":6,
                "predictedSnapshotLateFrameCount":4,
                "snapshotBytesTotal":18496000,
                "snapshotBytesMax":92000,
                "snapshotBytesAvg":64000,
                "snapshotMessageCount":289,
                "snapshotByteSource":"messagepack-application-payload",
                "snapshotCodec":"messagepack-compact",
                "snapshotCodecVersion":1,
                "snapshotFrameKind":"binary",
                "snapshotBytesP95":85000,
                "snapshotSegmentBudgetBytes":1280,
                "snapshotOverSegmentBudgetCount":280,
                "snapshotOverSegmentBudgetPctX100":9689,
                "snapshotParseMaxMs":9,
                "snapshotParseP95Ms":4,
                "snapshotDecodeMaxMs":11,
                "snapshotDecodeP95Ms":8,
                "websocketExtensions":"permessage-deflate; client_max_window_bits",
                "websocketCompression":"permessage-deflate",
                "snapshotApplyMaxMs":13,
                "snapshotApplyP95Ms":8,
                "predictionApplyMaxMs":7,
                "predictionApplyP95Ms":4,
                "snapshotTickGapMax":3,
                "staleSnapshotCount":1,
                "duplicateSnapshotCount":2,
                "skippedSnapshotCount":3,
                "snapshotBurstCount":4,
                "snapshotBurstMax":5,
                "frameGapMaxMs":37,
                "fpsEstimate":58,
                "frameWorkMaxMs":42,
                "frameWorkP95Ms":24,
                "slowFrameCount":2,
                "worstFramePhase":"match.renderer",
                "worstFramePhaseMs":22,
                "rendererMaxMs":20,
                "rendererP95Ms":16,
                "entityCount":325,
                "selectedCount":9,
                "visibleTileCount":918,
                "viewportWidth":1440,
                "viewportHeight":900,
                "devicePixelRatioX100":200,
                "commandBurstBucketMs":250,
                "commandBurstMax":7,
                "commandBurstFrameGapMaxMs":37,
                "commandBurstWorstFramePhase":"match.input",
                "commandBurstWorstFramePhaseMs":18,
                "hidden":false,
                "focused":true,
                "wsBufferedBytes":0,
                "serverTickMs":30,
                "serverLagMs":1,
                "slowTickCount":2,
                "headOfLineCount":7,
                "commandsIssued":4,
                "commandSocketSendAccepted":4,
                "commandServerReceived":3,
                "commandSimAcknowledged":2,
                "commandRejected":1,
                "commandIssueToServerReceiptLatestMs":80,
                "commandIssueToServerReceiptMaxMs":120,
                "commandIssueToServerReceiptP95Ms":100,
                "commandServerReceiptToSimAckLatestMs":33,
                "commandServerReceiptToSimAckMaxMs":66,
                "commandServerReceiptToSimAckP95Ms":50,
                "commandIssueToSimAckLatestMs":113,
                "commandIssueToSimAckMaxMs":180,
                "commandIssueToSimAckP95Ms":150,
                "commandAckSnapshotReceivedToAppliedLatestMs":4,
                "commandAckSnapshotReceivedToAppliedMaxMs":9,
                "commandAckSnapshotReceivedToAppliedP95Ms":8,
                "oldestPendingCommandAgeMs":250,
                "maxPendingCommandCount":5,
                "predictionDisableUserCount":1,
                "predictionDisableReplayCount":2,
                "predictionDisableSpectatorCount":3,
                "predictionDisableCompatibilityCount":4,
                "predictionDisableWasmCount":5,
                "predictionDisableOtherCount":6,
                "predictionReplayMaxMs":9,
                "predictionReplayMaxTicks":8,
                "predictionReplayBudgetExceededCount":7
            }
        }"#;
        let msg: ClientMessage = serde_json::from_str(raw).unwrap();
        match msg {
            ClientMessage::NetReport { report } => {
                assert_eq!(report.schema_version, 1);
                assert_eq!(report.match_run_id, "main-123");
                assert_eq!(report.snapshot_gap_max_ms, 420);
                assert_eq!(report.snapshot_late_frame_count, 6);
                assert_eq!(report.predicted_snapshot_late_frame_count, 4);
                assert_eq!(report.snapshot_bytes_max, 92_000);
                assert_eq!(
                    report.snapshot_byte_source,
                    "messagepack-application-payload"
                );
                assert_eq!(report.snapshot_codec, "messagepack-compact");
                assert_eq!(report.snapshot_codec_version, 1);
                assert_eq!(report.snapshot_frame_kind, "binary");
                assert_eq!(report.snapshot_bytes_p95, 85_000);
                assert_eq!(report.snapshot_segment_budget_bytes, 1_280);
                assert_eq!(report.snapshot_over_segment_budget_count, 280);
                assert_eq!(report.snapshot_over_segment_budget_pct_x100, 9_689);
                assert_eq!(report.snapshot_decode_p95_ms, 8);
                assert_eq!(
                    report.websocket_extensions,
                    "permessage-deflate; client_max_window_bits"
                );
                assert_eq!(report.websocket_compression, "permessage-deflate");
                assert_eq!(report.snapshot_burst_max, 5);
                assert_eq!(report.frame_work_max_ms, 42);
                assert_eq!(report.worst_frame_phase, "match.renderer");
                assert_eq!(report.entity_count, 325);
                assert_eq!(report.device_pixel_ratio_x100, 200);
                assert_eq!(report.command_burst_bucket_ms, 250);
                assert_eq!(report.command_burst_max, 7);
                assert_eq!(report.command_burst_frame_gap_max_ms, 37);
                assert_eq!(report.command_burst_worst_frame_phase, "match.input");
                assert_eq!(report.command_burst_worst_frame_phase_ms, 18);
                assert_eq!(report.head_of_line_count, 7);
                assert_eq!(report.commands_issued, 4);
                assert_eq!(report.command_server_received, 3);
                assert_eq!(report.command_rejected, 1);
                assert_eq!(report.command_issue_to_sim_ack_max_ms, 180);
                assert_eq!(report.oldest_pending_command_age_ms, 250);
                assert_eq!(report.max_pending_command_count, 5);
                assert_eq!(report.prediction_disable_user_count, 1);
                assert_eq!(report.prediction_disable_replay_count, 2);
                assert_eq!(report.prediction_disable_spectator_count, 3);
                assert_eq!(report.prediction_disable_compatibility_count, 4);
                assert_eq!(report.prediction_disable_wasm_count, 5);
                assert_eq!(report.prediction_disable_other_count, 6);
                assert_eq!(report.prediction_replay_max_ms, 9);
                assert_eq!(report.prediction_replay_max_ticks, 8);
                assert_eq!(report.prediction_replay_budget_exceeded_count, 7);
            }
            other => panic!("expected net report, got {other:?}"),
        }
    }

    #[test]
    fn client_net_report_defaults_perf_fields() {
        let raw = r#"{
            "t":"netReport",
            "report":{
                "schemaVersion":1,
                "elapsedMs":10000,
                "matchTick":300,
                "rttMs":82,
                "rttMaxMs":82,
                "badRttSamples":0,
                "snapshotJitterMs":0,
                "snapshotGapMaxMs":33,
                "jitterSamples":0,
                "snapshots":300,
                "frameGapMaxMs":16,
                "fpsEstimate":60,
                "hidden":false,
                "focused":true,
                "wsBufferedBytes":0,
                "serverTickMs":4,
                "serverLagMs":0,
                "slowTickCount":0,
                "headOfLineCount":0
            }
        }"#;
        let msg: ClientMessage = serde_json::from_str(raw).unwrap();
        match msg {
            ClientMessage::NetReport { report } => {
                assert_eq!(report.frame_work_max_ms, 0);
                assert_eq!(report.worst_frame_phase, "");
                assert_eq!(report.renderer_p95_ms, 0);
                assert_eq!(report.entity_count, 0);
                assert_eq!(report.snapshot_bytes_total, 0);
                assert_eq!(report.snapshot_byte_source, "");
                assert_eq!(report.snapshot_codec, "");
                assert_eq!(report.snapshot_codec_version, 0);
                assert_eq!(report.snapshot_frame_kind, "");
                assert_eq!(report.snapshot_bytes_p95, 0);
                assert_eq!(report.snapshot_segment_budget_bytes, 0);
                assert_eq!(report.snapshot_over_segment_budget_count, 0);
                assert_eq!(report.snapshot_over_segment_budget_pct_x100, 0);
                assert_eq!(report.snapshot_parse_max_ms, 0);
                assert_eq!(report.websocket_extensions, "");
                assert_eq!(report.websocket_compression, "");
                assert_eq!(report.snapshot_tick_gap_max, 0);
                assert_eq!(report.match_run_id, "");
                assert_eq!(report.command_burst_max, 0);
                assert_eq!(report.command_issue_to_server_receipt_max_ms, 0);
                assert_eq!(report.max_pending_command_count, 0);
                assert_eq!(report.prediction_disable_wasm_count, 0);
                assert_eq!(report.prediction_replay_max_ms, 0);
                assert_eq!(report.prediction_replay_budget_exceeded_count, 0);
            }
            other => panic!("expected net report, got {other:?}"),
        }
    }

    #[test]
    fn seek_replay_to_deserializes_absolute_tick() {
        let msg: ClientMessage = serde_json::from_str(r#"{"t":"seekRoomTimeTo","tick":4100}"#)
            .expect("seekRoomTimeTo should deserialize");

        match msg {
            ClientMessage::SeekRoomTimeTo { tick } => assert_eq!(tick, 4_100),
            other => panic!("expected seekRoomTimeTo, got {other:?}"),
        }
    }

    #[test]
    fn set_vision_selection_deserializes() {
        let msg: ClientMessage = serde_json::from_str(
            r#"{"t":"setVisionSelection","selection":{"mode":"player","playerId":7}}"#,
        )
        .expect("setVisionSelection should deserialize");

        match msg {
            ClientMessage::SetVisionSelection {
                selection: VisionSelectionRequest::Player { player_id },
            } => assert_eq!(player_id, 7),
            other => panic!("expected setVisionSelection, got {other:?}"),
        }
    }

    #[test]
    fn request_branch_from_tick_deserializes() {
        let msg: ClientMessage = serde_json::from_str(r#"{"t":"requestBranchFromTick"}"#)
            .expect("requestBranchFromTick should deserialize");

        assert!(matches!(msg, ClientMessage::RequestBranchFromTick));
    }

    #[test]
    fn branch_staging_client_messages_deserialize() {
        let claim: ClientMessage = serde_json::from_str(r#"{"t":"claimBranchSeat","playerId":7}"#)
            .expect("claimBranchSeat should deserialize");
        let release: ClientMessage =
            serde_json::from_str(r#"{"t":"releaseBranchSeat","playerId":7}"#)
                .expect("releaseBranchSeat should deserialize");
        let start: ClientMessage =
            serde_json::from_str(r#"{"t":"startBranch"}"#).expect("startBranch should deserialize");

        assert!(matches!(
            claim,
            ClientMessage::ClaimBranchSeat { player_id: 7 }
        ));
        assert!(matches!(
            release,
            ClientMessage::ReleaseBranchSeat { player_id: 7 }
        ));
        assert!(matches!(start, ClientMessage::StartBranch));
    }

    #[test]
    fn branch_from_tick_created_serializes_contract_shape() {
        let msg = ServerMessage::BranchFromTickCreated {
            branch_room: "__replay_branch__:00000001".to_string(),
            source_tick: 123,
            seats: vec![ReplayBranchSeat {
                player_id: 7,
                team_id: 7,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: "Player 7".to_string(),
                color: "#4878c8".to_string(),
                claimable: true,
            }],
        };
        let json = serde_json::to_value(msg).expect("branch message should serialize");

        assert_eq!(json["t"], "branchFromTickCreated");
        assert_eq!(json["branchRoom"], "__replay_branch__:00000001");
        assert_eq!(json["sourceTick"], 123);
        assert_eq!(json["seats"][0]["playerId"], 7);
        assert_eq!(json["seats"][0]["teamId"], 7);
        assert_eq!(json["seats"][0]["factionId"], DEFAULT_FACTION_ID);
        assert_eq!(json["seats"][0]["name"], "Player 7");
        assert_eq!(json["seats"][0]["color"], "#4878c8");
        assert_eq!(json["seats"][0]["claimable"], true);
    }

    #[test]
    fn observer_analysis_serializes_contract_shape() {
        let msg = ServerMessage::ObserverAnalysis(ObserverAnalysisPayload {
            tick: 77,
            players: vec![ObserverAnalysisPlayer {
                id: 1,
                units: vec![ObserverAnalysisKindCount {
                    kind: kinds::RIFLEMAN.to_string(),
                    count: 3,
                    steel_value: 180,
                    oil_value: 0,
                }],
                production: vec![ObserverAnalysisProduction {
                    building_id: 10,
                    building_kind: kinds::BARRACKS.to_string(),
                    item_kind: kinds::MACHINE_GUNNER.to_string(),
                    item_type: "unit".to_string(),
                    progress: 0.5,
                    queue_depth: 2,
                }],
                units_lost: vec![ObserverAnalysisKindCount {
                    kind: kinds::WORKER.to_string(),
                    count: 1,
                    steel_value: 50,
                    oil_value: 0,
                }],
                resources_lost: ObserverAnalysisResourcesLost { steel: 50, oil: 0 },
            }],
        });
        let json = serde_json::to_value(msg).expect("observer analysis should serialize");

        assert_eq!(json["t"], "observerAnalysis");
        assert_eq!(json["tick"], 77);
        assert_eq!(json["players"][0]["id"], 1);
        assert_eq!(json["players"][0]["units"][0]["kind"], "rifleman");
        assert_eq!(json["players"][0]["units"][0]["count"], 3);
        assert_eq!(json["players"][0]["units"][0]["steelValue"], 180);
        assert_eq!(json["players"][0]["production"][0]["buildingId"], 10);
        assert_eq!(json["players"][0]["production"][0]["itemType"], "unit");
        assert_eq!(json["players"][0]["production"][0]["queueDepth"], 2);
        assert_eq!(json["players"][0]["unitsLost"][0]["kind"], "worker");
        assert_eq!(json["players"][0]["resourcesLost"]["steel"], 50);
    }

    #[test]
    fn branch_staging_serializes_contract_shape() {
        let msg = ServerMessage::BranchStaging {
            room: "__replay_branch__:00000001".to_string(),
            source_tick: 123,
            host_id: 100,
            seats: vec![BranchStagingSeat {
                player_id: 7,
                team_id: 7,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: "Player 7".to_string(),
                color: "#4878c8".to_string(),
                claimant_id: Some(100),
                claimant_name: Some("Viewer 100".to_string()),
            }],
            occupants: vec![BranchStagingOccupant {
                id: 100,
                name: "Viewer 100".to_string(),
            }],
            can_start: true,
        };
        let json = serde_json::to_value(msg).expect("branch staging should serialize");

        assert_eq!(json["t"], "branchStaging");
        assert_eq!(json["room"], "__replay_branch__:00000001");
        assert_eq!(json["sourceTick"], 123);
        assert_eq!(json["hostId"], 100);
        assert_eq!(json["seats"][0]["playerId"], 7);
        assert_eq!(json["seats"][0]["teamId"], 7);
        assert_eq!(json["seats"][0]["factionId"], DEFAULT_FACTION_ID);
        assert_eq!(json["seats"][0]["claimantId"], 100);
        assert_eq!(json["seats"][0]["claimantName"], "Viewer 100");
        assert_eq!(json["occupants"][0]["id"], 100);
        assert_eq!(json["canStart"], true);
    }

    fn representative_snapshot() -> Snapshot {
        let mut worker = EntityView::new(1, 1, kinds::WORKER, 10.0, 20.0, 40, 40, states::GATHER);
        worker.facing = Some(1.5);
        worker.weapon_facing = Some(1.75);
        worker.latched_node = Some(200);
        worker.target_id = Some(9);
        worker.order_plan = vec![
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 96.0,
                y: 112.0,
            },
            OrderPlanMarker {
                kind: "setupAntiTankGuns".to_string(),
                x: 128.0,
                y: 160.0,
            },
            OrderPlanMarker {
                kind: abilities::CHARGE.to_string(),
                x: 176.0,
                y: 208.0,
            },
            OrderPlanMarker {
                kind: abilities::SMOKE.to_string(),
                x: 192.0,
                y: 224.0,
            },
            OrderPlanMarker {
                kind: abilities::POINT_FIRE.to_string(),
                x: 320.0,
                y: 352.0,
            },
        ];
        worker.charge_cooldown_left = Some(87);
        worker.abilities = vec![AbilityCooldownView {
            ability: abilities::CHARGE.to_string(),
            cooldown_left: 87,
            remaining_uses: Some(2),
            autocast_enabled: None,
            active_object_id: None,
            available_tick: None,
            lockout_until_tick: None,
            expires_in: None,
        }];
        worker.vision_only = true;

        let mut gunner = EntityView::new(
            2,
            1,
            kinds::MACHINE_GUNNER,
            30.0,
            40.0,
            55,
            55,
            states::ATTACK,
        );
        gunner.target_id = Some(7);
        gunner.setup_state = Some("deployed".to_string());

        let mut center = EntityView::new(
            3,
            1,
            kinds::CITY_CENTRE,
            100.0,
            120.0,
            450,
            500,
            states::TRAIN,
        );
        center.prod_kind = Some(kinds::WORKER.to_string());
        center.prod_progress = Some(0.25);
        center.prod_queue = Some(2);
        center.build_progress = Some(0.75);
        center.build_active = true;
        center.rally = Some([256.0, 512.0]);
        center.rally_plan = vec![
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 256.0,
                y: 512.0,
            },
            OrderPlanMarker {
                kind: "attackMove".to_string(),
                x: 320.0,
                y: 544.0,
            },
        ];

        Snapshot {
            tick: 42,
            steel: 100,
            oil: 25,
            supply_used: 3,
            supply_cap: 10,
            entities: vec![worker, gunner, center],
            resource_deltas: vec![ResourceDelta {
                id: 200,
                remaining: 1498,
            }],
            smokes: vec![SmokeCloudView {
                id: 50,
                x: 320.0,
                y: 352.0,
                radius_tiles: 2.0,
                expires_in: 120,
            }],
            ability_objects: vec![AbilityObjectView {
                id: 70,
                owner: 1,
                ability: abilities::EKAT_TELEPORT.to_string(),
                kind: ability_object_kinds::RETURN_MARKER.to_string(),
                x: 384.0,
                y: 416.0,
                expires_in: Some(90),
                source_caster_id: Some(7),
                owner_state: Some(AbilityObjectOwnerStateView {
                    earliest_return_tick: Some(45),
                    ..Default::default()
                }),
            }],
            visible_tiles: vec![1, 1, 0, 0, 0, 1],
            remembered_buildings: vec![RememberedBuildingView {
                id: 99,
                owner: 2,
                kind: kinds::DEPOT.to_string(),
                x: 640.0,
                y: 672.0,
                footprint: vec![[20, 21], [21, 21]],
                observed_tick: 39,
            }],
            events: vec![
                Event::Attack {
                    from: 1,
                    to: 7,
                    reveal: Some(AttackReveal {
                        owner: 1,
                        kind: kinds::ANTI_TANK_GUN.to_string(),
                        x: 12.0,
                        y: 24.0,
                        facing: Some(0.5),
                        weapon_facing: Some(0.75),
                        setup_state: Some("deployed".to_string()),
                    }),
                    to_pos: Some([48.0, 96.0]),
                },
                Event::Overpenetration { to: 8 },
                Event::Death {
                    id: 200,
                    x: 64.0,
                    y: 96.0,
                    kind: kinds::STEEL.to_string(),
                },
                Event::Build {
                    id: 3,
                    kind: kinds::CITY_CENTRE.to_string(),
                },
                Event::Notice {
                    msg: "Not enough steel".to_string(),
                    x: None,
                    y: None,
                    severity: NoticeSeverity::Info,
                },
                Event::MortarLaunch {
                    from: 9,
                    from_x: 256.0,
                    from_y: 272.0,
                    to_x: 320.0,
                    to_y: 352.0,
                    radius_tiles: 1.5,
                    delay_ticks: 68,
                },
                Event::ArtilleryTarget {
                    from: 10,
                    x: 320.0,
                    y: 352.0,
                    radius_tiles: 3.0,
                    delay_ticks: 120,
                },
                Event::ArtilleryFiring {
                    owner: 1,
                    x: 288.0,
                    y: 304.0,
                    facing: 0.25,
                },
                Event::ArtilleryImpact {
                    x: 336.0,
                    y: 368.0,
                    radius_tiles: 3.0,
                },
            ],
            upgrades: vec![upgrades::ARTILLERY_UNLOCK.to_string()],
            player_resources: Vec::new(),
            net_status: SnapshotNetStatus {
                server_lag_ms: 4,
                tick_ms: 17,
                slow_tick: false,
                slow_tick_count: 2,
                head_of_line: true,
                head_of_line_count: 3,
                prediction_version: PREDICTION_PROTOCOL_VERSION,
                last_sim_consumed_client_seq: 8,
                last_sim_consumed_client_tick: Some(42),
            },
        }
    }

    #[test]
    fn command_messages_require_client_sequence_envelope() {
        let msg: ClientMessage = serde_json::from_str(
            r#"{"t":"command","clientSeq":7,"cmd":{"c":"move","units":[1,2],"x":10.0,"y":20.0}}"#,
        )
        .expect("sequenced command should deserialize");

        match msg {
            ClientMessage::Command { client_seq, cmd } => {
                assert_eq!(client_seq, 7);
                assert!(matches!(cmd, Command::Move { units, .. } if units == vec![1, 2]));
            }
            other => panic!("unexpected message: {other:?}"),
        }

        let missing_seq = serde_json::from_str::<ClientMessage>(
            r#"{"t":"command","cmd":{"c":"move","units":[1],"x":10.0,"y":20.0}}"#,
        );
        assert!(missing_seq.is_err());
    }

    #[test]
    fn compact_snapshot_is_versioned_and_smaller_than_object_json() {
        let snapshot = representative_snapshot();
        let compact = serialize_compact_snapshot(&snapshot).unwrap();
        let object = serde_json::to_string(&ServerMessage::Snapshot(snapshot.clone())).unwrap();

        assert!(
            compact.len() < object.len(),
            "compact={} object={}",
            compact.len(),
            object.len()
        );

        let value: serde_json::Value = serde_json::from_str(&compact).unwrap();
        assert_eq!(value["t"], "snapshot");
        assert_eq!(value["v"], COMPACT_SNAPSHOT_VERSION);
        assert_eq!(value["s"], serde_json::json!([42, 100, 25, 3, 10]));
        assert_eq!(value["e"].as_array().unwrap().len(), 3);
        assert_eq!(value["e"][0][8], serde_json::json!(1.5));
        assert_eq!(value["e"][0][9], serde_json::json!(1.75));
        assert_eq!(value["e"][0][14], serde_json::json!(200));
        assert_eq!(value["e"][0][15], serde_json::json!(9));
        assert_eq!(
            value["e"][0][21],
            serde_json::json!([
                [1, 96.0, 112.0],
                [7, 128.0, 160.0],
                [8, 176.0, 208.0],
                [6, 192.0, 224.0],
                [10, 320.0, 352.0]
            ])
        );
        assert_eq!(value["e"][0][22], serde_json::json!(87));
        assert_eq!(value["e"][0][23], serde_json::json!([[1, 87, 2]]));
        assert_eq!(value["e"][0][24], serde_json::Value::Null);
        assert_eq!(value["e"][0][25], serde_json::json!(true));
        // Rally point rides in slot 18 of the producing building's record.
        assert_eq!(value["e"][2][18], serde_json::json!([256.0, 512.0]));
        // Rally plan is appended after the legacy optional slots so earlier compact positions stay stable.
        assert_eq!(
            value["e"][2][27],
            serde_json::json!([[1, 256.0, 512.0], [2, 320.0, 544.0]])
        );
        assert_eq!(value["r"], serde_json::json!([[200, 1498]]));
        assert_eq!(
            value["sm"],
            serde_json::json!([[50, 320.0, 352.0, 2.0, 120]])
        );
        assert_eq!(
            value["ao"],
            serde_json::json!([[
                70,
                1,
                6,
                1,
                384.0,
                416.0,
                90,
                7,
                [45, null, null, null, null, null]
            ]])
        );
        assert_eq!(value["fg"], serde_json::json!([1, 2, 3, 1]));
        assert_eq!(
            value["mb"],
            serde_json::json!([[99, 2, 7, 640.0, 672.0, [[20, 21], [21, 21]], 39]])
        );
        assert_eq!(value["u"], serde_json::json!([4]));
        assert_eq!(value["ev"].as_array().unwrap().len(), 9);
        assert_eq!(
            value["n"],
            serde_json::json!([4, 17, 2, 2, 3, PREDICTION_PROTOCOL_VERSION, 8, 42])
        );
        assert_eq!(
            value["ev"][0][3],
            serde_json::json!([1, 4, 12.0, 24.0, 0.5, 0.75, 3])
        );
        assert_eq!(value["ev"][0][4], serde_json::json!([48.0, 96.0]));
        assert_eq!(value["ev"][1], serde_json::json!([10, 8]));
        assert_eq!(
            value["ev"][5],
            serde_json::json!([9, 9, [256.0, 272.0], [320.0, 352.0], 1.5, 68])
        );
        assert_eq!(
            value["ev"][6],
            serde_json::json!([7, 10, [320.0, 352.0], 3.0, 120])
        );
        assert_eq!(
            value["ev"][7],
            serde_json::json!([11, 1, 288.0, 304.0, 0.25])
        );
        assert_eq!(value["ev"][8], serde_json::json!([8, 336.0, 368.0, 3.0]));
    }

    #[test]
    fn snapshot_codec_seam_defaults_to_messagepack_binary() {
        let snapshot = representative_snapshot();
        assert_eq!(
            default_snapshot_codec().name(),
            SNAPSHOT_CODEC_MESSAGEPACK_COMPACT
        );
        assert_eq!(default_snapshot_codec().version(), SNAPSHOT_CODEC_VERSION);
        assert!(supported_snapshot_codec(
            SNAPSHOT_CODEC_MESSAGEPACK_COMPACT,
            SNAPSHOT_CODEC_VERSION
        ));
        assert!(!supported_snapshot_codec(
            SNAPSHOT_CODEC_COMPACT_JSON,
            SNAPSHOT_CODEC_VERSION
        ));
        let frame = encode_snapshot_frame(&snapshot, default_snapshot_codec()).unwrap();
        match frame {
            SnapshotFrame::Binary(bytes) => {
                assert_eq!(
                    &bytes[..MESSAGEPACK_SNAPSHOT_FRAME_MAGIC.len()],
                    MESSAGEPACK_SNAPSHOT_FRAME_MAGIC.as_slice()
                );
                assert_eq!(
                    bytes[MESSAGEPACK_SNAPSHOT_FRAME_MAGIC.len()],
                    messagepack_frame::MESSAGEPACK_SNAPSHOT_HEADER_VERSION
                );
                let compact_json = serialize_compact_snapshot(&snapshot).unwrap();
                assert!(
                    bytes.len() < compact_json.len(),
                    "MessagePack should be smaller than compact JSON for the representative snapshot"
                );
            }
            SnapshotFrame::Text(_) => panic!("default snapshot codec must be binary"),
        }
    }

    #[test]
    fn compact_json_snapshot_codec_remains_available_for_local_baselines() {
        let snapshot = representative_snapshot();
        let frame = encode_snapshot_frame(&snapshot, SnapshotCodec::CompactJson).unwrap();
        match frame {
            SnapshotFrame::Text(text) => {
                let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(value["t"], "snapshot");
                assert_eq!(value["v"], COMPACT_SNAPSHOT_VERSION);
            }
            SnapshotFrame::Binary(_) => panic!("compact JSON baseline codec must stay text"),
        }
    }

    #[test]
    fn compact_entity_trims_trailing_optional_nulls() {
        let snapshot = Snapshot {
            tick: 1,
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            entities: vec![EntityView::new(
                1,
                1,
                kinds::WORKER,
                10.0,
                20.0,
                40,
                40,
                states::IDLE,
            )],
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            ability_objects: Vec::new(),
            visible_tiles: Vec::new(),
            remembered_buildings: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: Vec::new(),
            net_status: SnapshotNetStatus::default(),
        };

        let compact = serialize_compact_snapshot(&snapshot).unwrap();
        let value: serde_json::Value = serde_json::from_str(&compact).unwrap();
        let entity = value["e"][0].as_array().unwrap();
        assert_eq!(entity.len(), 8);
        assert!(value.get("r").is_none());
        assert!(value.get("ev").is_none());
    }
}
