// Wind Dash: windDash (ability 21, order stage 29), without a return marker.
//! Wire protocol (JSON + binary snapshots over WebSocket). See `docs/design/protocol.md`.
//!
//! Rust is authoritative; update its JavaScript mirror (`client/src/protocol.js`) together.
//!
//! Tag conventions: top-level messages use `"t"`, commands use `"c"`, events use `"e"`.
//! Coordinates are world pixels (floats) unless the field name ends in `Tile`.
use serde::{Deserialize, Serialize};
// Compact v76 appends the fog-gated public chargeActive flag at entity slot 55.
// Dao Adepts: kinds 35–37; public Master rank is EntityView slot 52 (v65); public itemRadianceTier is slot 54 (v70), range 0–6, fog-gated with its carrier.
pub mod chat;
mod client_message;
mod client_net_report;
mod compact_snapshot;
mod contract_metadata;
mod formation_point;
mod lab_replay;
mod lab_scenario;
mod messagepack_frame;
mod observer_analysis;
mod server_message;
pub use chat::{ChatChannel, ChatScope, ChatSendPayload, ChatText};
pub use client_message::ClientMessage;
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
    AttackReveal, AutoBuildSettingsSnapshot, CommandCapabilities, DebugPathPoint, DebugPathView,
    DiagnosticCapabilities, EntityView, Event, GroundDecalDelta, GroundDecalView, InitialCamera,
    LabStartMetadata, LabStartRole, LabVisionMode, MapInfo, MapTile, MatchControlCapabilities,
    MovementPathDiagnosticScope, NoticeSeverity, ObserverViewSelection, OrderPlanMarker,
    PlayerFactionSnapshot, PlayerResourceSnapshot, PlayerScore, PlayerStart,
    RememberedAntiTankGunView, RememberedBuildingView, ReplayStartMetadata, ResourceDelta,
    ResourceNode, RoomCapabilities, RoomTimeCapabilities, RoomTimeSeekState, RoomTimeState,
    ScoutPlaneStateView, SmokeCloudView, Snapshot, SnapshotNetStatus, StartPayload, TankTrailView,
    TeamId, TrenchView, VisibilityCapabilities, DEFAULT_FACTION_ID,
    GAME_CHECKPOINT_CURRENT_VERSION, MAP_DOODAD_TYPE_IDS, MAP_TILE_SIZE_PX,
    MAX_GROUND_DECALS_PER_SNAPSHOT_DELTA,
};
pub use server_message::{
    LivePauseState, LiveResumeCountdown, MatchConclusion, MatchConclusionReason, ServerMessage,
};
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
    ClearObstacleArea {
        units: Vec<u32>,
        target: u32,
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
    Repair {
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_id: Option<u32>,
        ability: String,
        units: Vec<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        x: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        y: Option<f32>,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    VectorField {
        units: Vec<u32>,
        x: f32,
        y: f32,
        direction_x: f32,
        direction_y: f32,
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
    SetAutoBuildSettings {
        paused: bool,
        reserve_steel: u32,
        reserve_oil: u32,
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
    PointTanks {
        units: Vec<u32>,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dao: Option<LabSpawnDao>,
        #[serde(default, skip_serializing_if = "formation_point::is_false")]
        dao_master: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        faction_id: Option<String>,
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
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LabSpawnDao {
    Wind,
    War,
    Magic,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dao: Option<LabSpawnDao>,
    #[serde(default, skip_serializing_if = "formation_point::is_false")]
    pub dao_master: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<String>,
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
mod branch_message_tests;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

// Compact v71: visible Shield ownerState appends maxHp (slot 6) and durationTicks (slot 7).
