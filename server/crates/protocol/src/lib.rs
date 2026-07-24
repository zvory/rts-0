//! Wire protocol (JSON + binary snapshots over WebSocket). See `docs/design/protocol.md`.
//!
//! Rust is authoritative; update its JavaScript mirror (`client/src/protocol.js`) together.
//!
//! Tag conventions: top-level messages use `"t"`, commands use `"c"`, events use `"e"`.
//! Coordinates are world pixels (floats) unless the field name ends in `Tile`.
use serde::{Deserialize, Serialize};

mod client_net_report;
mod compact_snapshot;
mod contract_metadata;
mod formation_point;
mod lab_replay;
mod lab_scenario;
mod messagepack_frame;
mod observer_analysis;
mod server_message;
pub use client_net_report::{
    ClientFramePhaseReport, ClientNetReport, ClientRenderCounterReport, CommandLifecycleExemplar,
};
#[cfg(test)]
use contract_metadata::kind_code;
pub use contract_metadata::{
    abilities, ability_code, ability_object_kinds, kinds, lobby_kinds, notices, protocol_contract,
    states, terrain, upgrade_code, upgrades, weapons, CompactSlotSchemas, ProtocolCompactCodes,
    ProtocolContract, ProtocolMessageTags, ProtocolVocabularies, SlotField, SnapshotCodecContract,
    COMPACT_SNAPSHOT_VERSION, COMPACT_UNKNOWN_CODE, PREDICTION_PROTOCOL_VERSION,
    SNAPSHOT_CODEC_COMPACT_JSON, SNAPSHOT_CODEC_MESSAGEPACK_COMPACT, SNAPSHOT_CODEC_VERSION,
    SNAPSHOT_FRAME_KIND_BINARY, SNAPSHOT_FRAME_KIND_TEXT,
};
use formation_point::is_false;
pub use formation_point::{FormationPoint, MAX_FORMATION_POINTS};
pub use lab_replay::*;
pub use lab_scenario::*;
pub use messagepack_frame::MESSAGEPACK_SNAPSHOT_FRAME_MAGIC;
pub use observer_analysis::*;
pub use rts_contract::{
    AbilityCooldownView, AbilityObjectOwnerStateView, AbilityObjectView, ActionCapabilities,
    AttackReveal, CommandCapabilities, DebugPathPoint, DebugPathView, DiagnosticCapabilities,
    EntityView, Event, InitialCamera, LabStartMetadata, LabStartRole, LabVisionMode, MapInfo,
    MatchControlCapabilities, MovementPathDiagnosticScope, NoticeSeverity, ObserverViewSelection,
    OrderPlanMarker, PlayerResourceSnapshot, PlayerScore, PlayerStart, RememberedAntiTankGunView,
    RememberedBuildingView, ReplayStartMetadata, ResourceDelta, ResourceNode, RoomCapabilities,
    RoomTimeCapabilities, RoomTimeState, ScoutPlaneStateView, SmokeCloudView, Snapshot,
    SnapshotNetStatus, StartPayload, TeamId, TrenchView, VisibilityCapabilities,
    DEFAULT_FACTION_ID,
};
pub use server_message::ServerMessage;

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
    /// Update this connection's display name while it is still in a lobby.
    SetName { name: String },
    /// Toggle ready state in the lobby.
    Ready { ready: bool },
    /// Confirm that this client's renderer finished warming for the active countdown.
    MatchLoadReady {
        #[serde(rename = "countdownId")]
        countdown_id: u32,
    },
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
    /// Pause a live match. Honored only from live pause-capable players or spectators with pauses
    /// remaining.
    PauseGame,
    /// Unpause a paused live match. Honored only from live pause-capable players or spectators.
    UnpauseGame,
    /// Leave replay playback and return the room to a clean lobby.
    ReturnToLobby,
    /// Latency probe.
    Ping { ts: f64 },
    /// Client-observed network/render health aggregate for server logs.
    NetReport { report: Box<ClientNetReport> },
    /// Throttled notice that the connected browser received human input. This is distinct from
    /// automatic heartbeat and diagnostics traffic so the server can expire abandoned sessions.
    Activity,
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
        op: Box<LabClientOp>,
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
    FormationMove {
        units: Vec<u32>,
        points: Vec<FormationPoint>,
        #[serde(default, skip_serializing_if = "is_false")]
        attack_move: bool,
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
        tank_trap_cluster: bool,
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
    ArtilleryFire {
        units: Vec<u32>,
        x: f32,
        y: f32,
        radius_tiles: f32,
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
    AdjustProductionRepeat {
        buildings: Vec<u32>,
        unit: String,
        delta: i8,
    },
    Research {
        building: u32,
        upgrade: String,
    },
    Cancel {
        building: u32,
        /// Distinguish a construction-cancel intent from the legacy production-cancel action.
        /// This prevents a delayed construction click from cancelling production after the
        /// building completes before the command reaches the simulation.
        #[serde(default, skip_serializing_if = "is_false")]
        construction: bool,
    },
    Stop {
        units: Vec<u32>,
    },
    HoldPosition {
        units: Vec<u32>,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
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

pub type VisionSelectionRequest = ObserverViewSelection;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LabClientOp {
    ExportMap,
    ExportScenario {
        #[serde(default)]
        name: Option<String>,
    },
    ImportScenario {
        scenario: Box<LabScenarioPayload>,
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
    SpawnEntities {
        spawns: Vec<LabSpawnEntitySpec>,
    },
    DeleteEntity {
        entity_id: u32,
    },
    DeleteEntities {
        entity_ids: Vec<u32>,
    },
    MoveEntity {
        entity_id: u32,
        x: f32,
        y: f32,
    },
    ApplyUpdates {
        updates: Vec<LabUpdateSpec>,
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabSpawnEntitySpec {
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub completed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LabUpdateSpec {
    Move {
        entity_id: u32,
        x: f32,
        y: f32,
    },
    Reassign {
        entity_id: u32,
        owner: u32,
    },
    Resources {
        player_id: u32,
        steel: u32,
        oil: u32,
    },
    Research {
        player_id: u32,
        upgrade: String,
        #[serde(default = "default_true")]
        completed: bool,
    },
    GodMode {
        player_id: u32,
        #[serde(default = "default_true")]
        enabled: bool,
    },
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabMapDraft {
    pub name: String,
    pub size: u32,
    pub terrain: Vec<u8>,
    pub starts: Vec<LabMapTile>,
    pub base_sites: Vec<LabMapTile>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabMapTile {
    pub x: u32,
    pub y: u32,
}

// Server -> Client

/// A lobby map catalog row. `name` is the stable selector key; `description` is display text;
/// `min_players`/`max_players` describe authored active-player layout bounds.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableMap {
    pub name: String,
    pub description: String,
    pub min_players: u32,
    pub max_players: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LobbyKind {
    Normal,
    Replay,
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
    pub failed_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<serde_json::Value>,
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
    /// Canonical live AI profile id for computer opponents. Omitted for human players and
    /// spectators.
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

impl std::fmt::Display for SnapshotEncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotPayloadDiagnostics {
    pub bytes: u32,
    pub sections: Vec<SnapshotPayloadSectionDiagnostics>,
    pub entity_kinds: Vec<SnapshotPayloadEntityKindDiagnostics>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPayloadSectionDiagnostics {
    pub section: &'static str,
    pub count: u32,
    pub bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPayloadEntityKindDiagnostics {
    pub kind: String,
    pub count: u32,
    pub approx_bytes: u32,
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

pub fn encode_snapshot_frame_with_diagnostics(
    snapshot: &Snapshot,
    codec: SnapshotCodec,
) -> Result<(SnapshotFrame, SnapshotPayloadDiagnostics), SnapshotEncodeError> {
    match codec {
        SnapshotCodec::CompactJson => {
            let (text, diagnostics) =
                compact_snapshot::serialize_compact_snapshot_with_diagnostics(snapshot)?;
            Ok((SnapshotFrame::Text(text), diagnostics))
        }
        SnapshotCodec::MessagePackCompact => {
            let (bytes, diagnostics) =
                compact_snapshot::serialize_messagepack_compact_snapshot_with_diagnostics(
                    snapshot,
                )?;
            Ok((SnapshotFrame::Binary(bytes), diagnostics))
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
mod command_tests;

#[cfg(test)]
mod client_message_tests;

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod tests {
    use super::*;

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
                "predictedSnapshotLateFramePctX100":6667,
                "predictionActiveLateFrameCount":5,
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
                "frameRafDispatchMaxMs":17,
                "frameRafDispatchP95Ms":8,
                "frameUnattributedMaxMs":19,
                "frameUnattributedP95Ms":12,
                "slowFrameCount":2,
                "frameWorkBudgetMissCount":7,
                "presentBudgetMissCount":3,
                "worstFramePhase":"match.renderer",
                "worstFramePhaseMs":22,
                "rendererMaxMs":20,
                "rendererP95Ms":16,
                "rendererUpdateMaxMs":17,
                "rendererUpdateP95Ms":12,
                "rendererPresentMaxMs":6,
                "rendererPresentP95Ms":4,
                "topRendererPhase":"renderer.units",
                "topRendererPhaseMs":15,
                "topRenderDiagnosticGroup":"renderer.pixi.displayObject",
                "topRenderDiagnosticGroupCount":18,
                "clientFramePhases":[{"label":"match.renderer","count":12,"maxMs":20,"p95Ms":16}],
                "rendererFramePhases":[{"label":"renderer.units","count":12,"maxMs":15,"p95Ms":12}],
                "renderDiagnosticCounters":[{"label":"renderer.pixi.displayObject","samples":18,"frames":9,"total":18,"maxFrame":4}],
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
                "commandIssueToSocketSendAcceptedLatestMs":2,
                "commandIssueToSocketSendAcceptedMaxMs":3,
                "commandIssueToSocketSendAcceptedP95Ms":2,
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
                "commandFamilyMove":2,
                "commandFamilyAttackMove":1,
                "commandFamilyBuild":1,
                "commandFamilyTrain":0,
                "commandFamilyOther":0,
                "commandLifecycleExemplars":[{"clientSeq":9,"family":"move","issuedElapsedMs":125,"stage":"issueToSimAck","stageMs":180}],
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
                assert_eq!(report.predicted_snapshot_late_frame_pct_x100, 6_667);
                assert_eq!(report.prediction_active_late_frame_count, 5);
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
                assert_eq!(report.frame_raf_dispatch_max_ms, 17);
                assert_eq!(report.frame_unattributed_p95_ms, 12);
                assert_eq!(report.frame_work_budget_miss_count, 7);
                assert_eq!(report.present_budget_miss_count, 3);
                assert_eq!(report.worst_frame_phase, "match.renderer");
                assert_eq!(report.renderer_update_max_ms, 17);
                assert_eq!(report.renderer_update_p95_ms, 12);
                assert_eq!(report.renderer_present_max_ms, 6);
                assert_eq!(report.renderer_present_p95_ms, 4);
                assert_eq!(report.top_renderer_phase, "renderer.units");
                assert_eq!(
                    report.top_render_diagnostic_group,
                    "renderer.pixi.displayObject"
                );
                assert_eq!(report.client_frame_phases.len(), 1);
                assert_eq!(report.client_frame_phases[0].label, "match.renderer");
                assert_eq!(report.renderer_frame_phases[0].max_ms, 15);
                assert_eq!(report.render_diagnostic_counters[0].total, 18);
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
                assert_eq!(report.command_issue_to_socket_send_accepted_max_ms, 3);
                assert_eq!(report.command_issue_to_sim_ack_max_ms, 180);
                assert_eq!(report.oldest_pending_command_age_ms, 250);
                assert_eq!(report.max_pending_command_count, 5);
                assert_eq!(report.command_family_move, 2);
                assert_eq!(report.command_family_attack_move, 1);
                assert_eq!(report.command_lifecycle_exemplars.len(), 1);
                assert_eq!(report.command_lifecycle_exemplars[0].client_seq, 9);
                assert_eq!(report.command_lifecycle_exemplars[0].family, "move");
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
                assert_eq!(report.frame_raf_dispatch_max_ms, 0);
                assert_eq!(report.frame_unattributed_p95_ms, 0);
                assert_eq!(report.worst_frame_phase, "");
                assert_eq!(report.renderer_p95_ms, 0);
                assert_eq!(report.frame_work_budget_miss_count, 0);
                assert_eq!(report.present_budget_miss_count, 0);
                assert_eq!(report.renderer_update_max_ms, 0);
                assert_eq!(report.renderer_update_p95_ms, 0);
                assert_eq!(report.renderer_present_max_ms, 0);
                assert_eq!(report.renderer_present_p95_ms, 0);
                assert_eq!(report.top_renderer_phase, "");
                assert!(report.client_frame_phases.is_empty());
                assert!(report.renderer_frame_phases.is_empty());
                assert!(report.render_diagnostic_counters.is_empty());
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
    fn client_net_report_bounds_command_lifecycle_exemplars() {
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
                "headOfLineCount":0,
                "commandLifecycleExemplars":[
                    {"clientSeq":1,"family":"move","issuedElapsedMs":1,"stage":"issueToSimAck","stageMs":1},
                    {"clientSeq":2,"family":"move","issuedElapsedMs":2,"stage":"issueToSimAck","stageMs":2},
                    {"clientSeq":3,"family":"move","issuedElapsedMs":3,"stage":"issueToSimAck","stageMs":3},
                    {"clientSeq":4,"family":"move","issuedElapsedMs":4,"stage":"issueToSimAck","stageMs":4},
                    {"clientSeq":5,"family":"move","issuedElapsedMs":5,"stage":"issueToSimAck","stageMs":5},
                    {"clientSeq":6,"family":"move","issuedElapsedMs":6,"stage":"issueToSimAck","stageMs":6}
                ]
            }
        }"#;
        let msg: ClientMessage = serde_json::from_str(raw).unwrap();
        match msg {
            ClientMessage::NetReport { report } => {
                assert_eq!(report.command_lifecycle_exemplars.len(), 5);
                assert_eq!(report.command_lifecycle_exemplars[0].client_seq, 1);
                assert_eq!(report.command_lifecycle_exemplars[4].client_seq, 5);
            }
            other => panic!("expected net report, got {other:?}"),
        }
    }

    #[test]
    fn client_net_report_bounds_frame_context_arrays() {
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
                "headOfLineCount":0,
                "clientFramePhases":[
                    {"label":"match.renderer","count":1,"maxMs":1,"p95Ms":1},
                    {"label":"match.hud","count":2,"maxMs":2,"p95Ms":2},
                    {"label":"match.minimap","count":3,"maxMs":3,"p95Ms":3},
                    {"label":"match.input","count":4,"maxMs":4,"p95Ms":4},
                    {"label":"frame.unattributed","count":5,"maxMs":5,"p95Ms":5},
                    {"label":"frame.rafDispatch","count":6,"maxMs":6,"p95Ms":6}
                ],
                "renderDiagnosticCounters":[
                    {"label":"renderer.pixi.displayObject","samples":1,"frames":1,"total":1,"maxFrame":1},
                    {"label":"renderer.rig.redraw","samples":2,"frames":2,"total":2,"maxFrame":2},
                    {"label":"renderer.graphics.clear","samples":3,"frames":3,"total":3,"maxFrame":3},
                    {"label":"minimap.invalidate","samples":4,"frames":4,"total":4,"maxFrame":4},
                    {"label":"hud.dirty","samples":5,"frames":5,"total":5,"maxFrame":5},
                    {"label":"observer.dirty","samples":6,"frames":6,"total":6,"maxFrame":6}
                ]
            }
        }"#;
        let msg: ClientMessage = serde_json::from_str(raw).unwrap();
        match msg {
            ClientMessage::NetReport { report } => {
                assert_eq!(report.client_frame_phases.len(), 5);
                assert_eq!(report.client_frame_phases[4].label, "frame.unattributed");
                assert_eq!(report.render_diagnostic_counters.len(), 5);
                assert_eq!(report.render_diagnostic_counters[4].label, "hud.dirty");
            }
            other => panic!("expected net report, got {other:?}"),
        }
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
            charge_recharge_left: Some(45),
        }];
        worker.vision_only = true;
        worker.occupied_trench_id = Some(80);
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
        center.prod_upgrade_queue = vec![upgrades::ANTI_TANK_GUN_UNLOCK.to_string()];
        center.prod_repeat_kinds = vec![kinds::WORKER.to_string(), kinds::SCOUT_CAR.to_string()];
        center.prod_scout_plane_queued = true;
        center.prod_waiting = true;
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
            world_combat_position: Some([1024.0, 2048.0]),
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
            trenches: vec![TrenchView {
                id: 80,
                x: 448.0,
                y: 480.0,
                radius_tiles: 0.375,
            }],
            visible_tiles: vec![1, 1, 0, 0, 0, 1],
            explored_tiles: vec![1, 1, 1, 0, 0, 1],
            remembered_buildings: vec![RememberedBuildingView {
                id: 99,
                owner: 2,
                kind: kinds::DEPOT.to_string(),
                x: 640.0,
                y: 672.0,
                footprint: vec![[20, 21], [21, 21]],
                observed_tick: 39,
            }],
            remembered_anti_tank_guns: vec![RememberedAntiTankGunView {
                id: 101,
                owner: 2,
                x: 704.0,
                y: 736.0,
                facing: 0.75,
                observed_tick: 40,
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
                    weapon_kind: Some(weapons::ANTI_TANK_GUN.to_string()),
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
                Event::PanzerfaustLaunch {
                    from: 11,
                    from_x: 360.0,
                    from_y: 384.0,
                    to_x: 416.0,
                    to_y: 384.0,
                    delay_ticks: 15,
                },
                Event::PanzerfaustImpact { x: 416.0, y: 384.0 },
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
        assert_eq!(value["wc"], serde_json::json!([1024.0, 2048.0]));
        assert_eq!(value["e"].as_array().unwrap().len(), 3);
        assert_eq!(
            value["ma"][0],
            serde_json::json!([101, 2, 704.0, 736.0, 0.75, 40])
        );
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
        assert_eq!(value["e"][0][23][0][8], serde_json::json!(45));
        assert_eq!(value["e"][0][24], serde_json::Value::Null);
        assert_eq!(value["e"][0][25], serde_json::json!(true));
        assert_eq!(value["e"][0][32], serde_json::json!(80));
        assert_eq!(value["e"][2][18], serde_json::json!([256.0, 512.0]));
        // Rally plan is appended after the legacy optional slots so earlier compact positions stay stable.
        assert_eq!(
            value["e"][2][27],
            serde_json::json!([[1, 256.0, 512.0], [2, 320.0, 544.0]])
        );
        assert_eq!(value["e"][2][34], serde_json::json!(true));
        let repeat_codes = [kind_code(kinds::WORKER), kind_code(kinds::SCOUT_CAR)];
        assert_eq!(value["e"][2][36], serde_json::json!(repeat_codes));
        assert_eq!(
            value["e"][2][40],
            serde_json::json!([upgrade_code(upgrades::ANTI_TANK_GUN_UNLOCK)])
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
        assert_eq!(value["tr"], serde_json::json!([[80, 448.0, 480.0, 0.375]]));
        assert_eq!(value["fg"], serde_json::json!([1, 2, 3, 1]));
        assert_eq!(value["eg"], serde_json::json!([1, 3, 2, 1]));
        assert_eq!(
            value["mb"],
            serde_json::json!([[99, 2, 7, 640.0, 672.0, [[20, 21], [21, 21]], 39]])
        );
        assert_eq!(value["u"], serde_json::json!([4]));
        assert_eq!(value["ev"].as_array().unwrap().len(), 11);
        assert_eq!(
            value["n"],
            serde_json::json!([4, 17, 2, 2, 3, PREDICTION_PROTOCOL_VERSION, 8, 42])
        );
        assert_eq!(
            value["ev"][0][3],
            serde_json::json!([1, 4, 12.0, 24.0, 0.5, 0.75, 3])
        );
        assert_eq!(value["ev"][0][4], serde_json::json!([48.0, 96.0]));
        assert_eq!(value["ev"][0][5], serde_json::json!(6));
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
        assert_eq!(
            value["ev"][9],
            serde_json::json!([12, 11, [360.0, 384.0], [416.0, 384.0], 15])
        );
        assert_eq!(value["ev"][10], serde_json::json!([13, 416.0, 384.0]));
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
    fn compact_snapshot_diagnostics_reports_bounded_payload_composition() {
        let snapshot = representative_snapshot();
        let (frame, diagnostics) =
            encode_snapshot_frame_with_diagnostics(&snapshot, default_snapshot_codec()).unwrap();
        let SnapshotFrame::Binary(bytes) = frame else {
            panic!("default snapshot codec must be binary");
        };
        let SnapshotFrame::Binary(plain_bytes) =
            encode_snapshot_frame(&snapshot, default_snapshot_codec()).unwrap()
        else {
            panic!("default snapshot codec must be binary");
        };

        assert_eq!(bytes, plain_bytes);
        assert_eq!(diagnostics.bytes, bytes.len() as u32);
        let section = |name: &str| {
            diagnostics
                .sections
                .iter()
                .find(|section| section.section == name)
                .unwrap_or_else(|| panic!("missing section {name}"))
        };
        assert_eq!(section("entities").count, 3);
        assert!(section("entities").bytes > 0);
        assert_eq!(section("visibility").count, 9);
        assert!(section("visibility").bytes > 0);
        assert_eq!(section("resourceDeltas").count, 1);
        assert_eq!(section("events").count, 11);
        assert_eq!(section("smokes").count, 1);
        assert_eq!(section("abilityObjects").count, 1);
        assert_eq!(section("trenches").count, 1);
        assert_eq!(section("playerStatus").count, 2);
        assert_eq!(section("netStatus").count, 1);
        assert!(section("other").bytes > 0);

        let worker = diagnostics
            .entity_kinds
            .iter()
            .find(|kind| kind.kind == kinds::WORKER)
            .expect("worker kind should be summarized");
        assert_eq!(worker.count, 1);
        assert!(worker.approx_bytes > 0);
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
}
