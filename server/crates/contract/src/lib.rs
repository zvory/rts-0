//! Semantic message contracts shared across simulation, replay, protocol, and server boundaries.
//!
//! These DTOs describe game state and events independent of WebSocket envelopes or compact
//! transport encoding.

use serde::{Deserialize, Serialize};

pub type TeamId = u32;
pub const DEFAULT_FACTION_ID: &str = "kriegsia";
/// Maximum raw submitted ids in an ordinary multi-unit command.
pub const MAX_UNITS_PER_COMMAND: usize = 256;
/// Maximum raw submitted ids in a Lab command that bypasses ordinary command limits.
pub const LAB_MAX_UNITS_PER_COMMAND: usize = 4_096;

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartPayload {
    pub player_id: u32,
    #[serde(default)]
    pub spectator: bool,
    /// Build id of the server/client bundle that produced this live start payload. Prediction is
    /// enabled only when this matches the browser bundle id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_build_id: Option<String>,
    /// Prediction protocol version supported by this live match. Omitted for spectators/replays.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub prediction_version: u32,
    /// Room-scoped live match correlation id used only for diagnostics/log joins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "RoomCapabilities::is_empty")]
    pub capabilities: RoomCapabilities,
    #[serde(default, skip_serializing_if = "DiagnosticCapabilities::is_empty")]
    pub diagnostics: DiagnosticCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay: Option<ReplayStartMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lab: Option<LabStartMetadata>,
    pub tick: u32,
    pub map: MapInfo,
    pub players: Vec<PlayerStart>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoomCapabilities {
    #[serde(default, skip_serializing_if = "RoomTimeCapabilities::is_empty")]
    pub room_time: RoomTimeCapabilities,
    #[serde(default, skip_serializing_if = "MatchControlCapabilities::is_empty")]
    pub match_controls: MatchControlCapabilities,
    #[serde(default, skip_serializing_if = "VisibilityCapabilities::is_empty")]
    pub visibility: VisibilityCapabilities,
    #[serde(default, skip_serializing_if = "CommandCapabilities::is_empty")]
    pub commands: CommandCapabilities,
    #[serde(default, skip_serializing_if = "ActionCapabilities::is_empty")]
    pub actions: ActionCapabilities,
}

impl RoomCapabilities {
    pub fn is_empty(&self) -> bool {
        self.room_time.is_empty()
            && self.match_controls.is_empty()
            && self.visibility.is_empty()
            && self.commands.is_empty()
            && self.actions.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoomTimeCapabilities {
    #[serde(default, skip_serializing_if = "is_false")]
    pub available: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub set_speed: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pause: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub step: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub seek_relative: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub seek_absolute: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub timeline: bool,
}

impl RoomTimeCapabilities {
    pub fn is_empty(&self) -> bool {
        !self.available
            && !self.set_speed
            && !self.pause
            && !self.step
            && !self.seek_relative
            && !self.seek_absolute
            && !self.timeline
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchControlCapabilities {
    #[serde(default, skip_serializing_if = "is_false")]
    pub pause: bool,
}

impl MatchControlCapabilities {
    pub fn is_empty(&self) -> bool {
        !self.pause
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisibilityCapabilities {
    #[serde(default, skip_serializing_if = "is_false")]
    pub vision_selection: bool,
}

impl VisibilityCapabilities {
    pub fn is_empty(&self) -> bool {
        !self.vision_selection
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandCapabilities {
    #[serde(default, skip_serializing_if = "is_false")]
    pub gameplay: bool,
}

impl CommandCapabilities {
    pub fn is_empty(&self) -> bool {
        !self.gameplay
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionCapabilities {
    #[serde(default, skip_serializing_if = "is_false")]
    pub branch_from_tick: bool,
}

impl ActionCapabilities {
    pub fn is_empty(&self) -> bool {
        !self.branch_from_tick
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCapabilities {
    #[serde(default, skip_serializing_if = "MovementPathDiagnosticScope::is_none")]
    pub movement_paths: MovementPathDiagnosticScope,
    #[serde(default, skip_serializing_if = "is_false")]
    pub observer_analysis: bool,
}

impl DiagnosticCapabilities {
    pub fn is_empty(&self) -> bool {
        self.movement_paths.is_none() && !self.observer_analysis
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MovementPathDiagnosticScope {
    #[default]
    None,
    OwnerOnly,
    All,
}

impl MovementPathDiagnosticScope {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InitialCamera {
    pub center_x: u32,
    pub center_y: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabStartMetadata {
    pub room: String,
    pub operator_id: u32,
    pub role: LabStartRole,
    pub vision: LabVisionMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub god_mode_players: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_camera: Option<InitialCamera>,
    pub dirty: bool,
    pub operation_count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LabStartRole {
    Operator,
    ReadOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LabVisionMode {
    All,
    Team { team_id: TeamId },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayStartMetadata {
    pub artifact_schema_version: u32,
    pub server_build_sha: String,
    pub map_name: String,
    pub map_schema_version: u32,
    pub map_content_hash: String,
    pub seed: u32,
    pub duration_ticks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoomTimeState {
    pub current_tick: u32,
    pub duration_ticks: u32,
    pub keyframe_ticks: Vec<u32>,
    pub speed: f32,
    pub paused: bool,
    pub ended: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapInfo {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    /// Row-major terrain codes, length = width * height.
    pub terrain: Vec<u8>,
    /// Positions of all neutral resource nodes (steel/oil). Included so the
    /// client can render them on the minimap before fog-of-war reveals them.
    pub resources: Vec<ResourceNode>,
}

/// A static resource node position included in the start payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceNode {
    pub id: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStart {
    pub id: u32,
    #[serde(default)]
    pub team_id: TeamId,
    pub faction_id: String,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub is_ai: bool,
    pub start_tile_x: u32,
    pub start_tile_y: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerScore {
    pub id: u32,
    #[serde(default)]
    pub team_id: TeamId,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub apm: u32,
    pub unit_score: u32,
    pub structure_score: u32,
    pub units_killed: u32,
    pub units_lost: u32,
    pub buildings_killed: u32,
    pub buildings_lost: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tick: u32,
    /// Coarse world combat area shared identically with every recipient for directional ambience.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_combat_position: Option<[f32; 2]>,
    pub steel: u32,
    pub oil: u32,
    pub supply_used: u32,
    pub supply_cap: u32,
    pub entities: Vec<EntityView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_deltas: Vec<ResourceDelta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub smokes: Vec<SmokeCloudView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ability_objects: Vec<AbilityObjectView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trenches: Vec<TrenchView>,
    /// Row-major current visibility grid for this recipient, one byte per map tile.
    /// Populated only for fog-filtered snapshots; clients keep explored history locally.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_tiles: Vec<u8>,
    /// Recipient-only stale enemy building intel. These records are last-seen memory, not live
    /// entities: clients may render them as non-interactive fog silhouettes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remembered_buildings: Vec<RememberedBuildingView>,
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upgrades: Vec<String>,
    /// Per-player resources for the projected observer players.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_resources: Vec<PlayerResourceSnapshot>,
    /// Per-recipient server/network diagnostics for the current match.
    pub net_status: SnapshotNetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AbilityObjectView {
    pub id: u32,
    pub owner: u32,
    pub ability: String,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_caster_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_state: Option<AbilityObjectOwnerStateView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AbilityObjectOwnerStateView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest_return_tick: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hp: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destroyed_lockout_ticks: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_traveled: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticks_out: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RememberedBuildingView {
    pub id: u32,
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub footprint: Vec<[u32; 2]>,
    pub observed_tick: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SmokeCloudView {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub radius_tiles: f32,
    pub expires_in: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrenchView {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub radius_tiles: f32,
}

/// Server-side transport and scheduling health attached to every snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotNetStatus {
    pub server_lag_ms: u16,
    pub tick_ms: u16,
    #[serde(default, skip_serializing_if = "is_false")]
    pub slow_tick: bool,
    pub slow_tick_count: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub head_of_line: bool,
    pub head_of_line_count: u32,
    /// Live player prediction/reconciliation protocol version. `0` means no prediction ACK is
    /// available for this recipient, which is used for spectators and replay viewers.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub prediction_version: u32,
    /// Highest contiguous client-local gameplay command sequence consumed by the authoritative
    /// simulation tick stream for this live player.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub last_sim_consumed_client_seq: u32,
    /// Authoritative tick that consumed `last_sim_consumed_client_seq`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sim_consumed_client_tick: Option<u32>,
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

/// Resources for one projected player, included in observer snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerResourceSnapshot {
    pub id: u32,
    pub steel: u32,
    pub oil: u32,
    pub supply_used: u32,
    pub supply_cap: u32,
}

/// Dynamic resource state the client is currently allowed to know.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDelta {
    pub id: u32,
    pub remaining: u32,
}

/// Owner-only visual stage for a selected unit's current + queued order plan. Stages carry only
/// safe world points and order flavor, never target ids.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrderPlanMarker {
    pub kind: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DebugPathPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DebugPathView {
    /// Remaining movement waypoints in traversal order. The first entry is the currently targeted
    /// waypoint; long paths are truncated for transport.
    pub waypoints: Vec<DebugPathPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<DebugPathPoint>,
    pub last_repath_tick: u32,
    pub stuck_ticks: u16,
    pub static_blocked_ticks: u16,
    pub total_waypoints: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AbilityCooldownView {
    pub ability: String,
    pub cooldown_left: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_uses: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autocast_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_object_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_tick: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lockout_until_tick: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u16>,
}

/// Owner/spectator-only Scout Plane state. Enemy snapshots that can see the plane still omit this
/// private state so orbit intent does not leak through fog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScoutPlaneStateView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orbit_center: Option<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_command_car: Option<u32>,
}

/// One entity as seen by one player. Optional fields are omitted when not applicable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityView {
    pub id: u32,
    /// 0 = neutral (resource nodes), otherwise the owning player id.
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub max_hp: u32,
    pub state: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_facing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_range_tiles: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panzerfaust_loaded: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_upgrade: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_progress: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_queue: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prod_repeat_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub prod_scout_plane_queued: bool,
    /// Owner/allies only: the front manual unit or research item has not paid yet.
    #[serde(default, skip_serializing_if = "is_false")]
    pub prod_waiting: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_progress: Option<f32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub build_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deconstruct_progress: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub latched_node: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_facing: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rally: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rally_plan: Vec<OrderPlanMarker>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub oil_used: Option<f32>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order_plan: Vec<OrderPlanMarker>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_cooldown_left: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abilities: Vec<AbilityCooldownView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakthrough_ticks: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakthrough_aura_ticks: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occupied_trench_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scout_plane: Option<ScoutPlaneStateView>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub vision_only: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_path: Option<DebugPathView>,
}

impl EntityView {
    /// Minimal constructor; fill optional fields afterward.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u32,
        owner: u32,
        kind: &str,
        x: f32,
        y: f32,
        hp: u32,
        max_hp: u32,
        state: &str,
    ) -> Self {
        EntityView {
            id,
            owner,
            kind: kind.to_string(),
            x,
            y,
            hp,
            max_hp,
            state: state.to_string(),
            facing: None,
            weapon_facing: None,
            weapon_range_tiles: None,
            panzerfaust_loaded: None,
            prod_kind: None,
            prod_upgrade: None,
            prod_progress: None,
            prod_queue: None,
            prod_repeat_kinds: Vec::new(),
            prod_scout_plane_queued: false,
            prod_waiting: false,
            build_progress: None,
            build_active: false,
            deconstruct_progress: None,
            latched_node: None,
            remaining: None,
            target_id: None,
            setup_state: None,
            setup_facing: None,
            rally: None,
            rally_plan: Vec::new(),
            oil_used: None,
            order_plan: Vec::new(),
            charge_cooldown_left: None,
            abilities: Vec::new(),
            breakthrough_ticks: None,
            breakthrough_aura_ticks: None,
            occupied_trench_id: None,
            scout_plane: None,
            vision_only: false,
            debug_path: None,
        }
    }
}

/// Minimal shooter view attached to selected attack events so the client can show a short-lived
/// fog reveal without adding a normal fog-visible snapshot entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AttackReveal {
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_facing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_state: Option<String>,
}

/// Transient, single-snapshot visual feedback. Clients must not rely on delivery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "e", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    Attack {
        from: u32,
        to: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reveal: Option<AttackReveal>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to_pos: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weapon_kind: Option<String>,
    },
    Overpenetration {
        to: u32,
    },
    Miss {
        to: u32,
    },
    Death {
        id: u32,
        x: f32,
        y: f32,
        kind: String,
    },
    Build {
        id: u32,
        kind: String,
    },
    SmokeLaunch {
        from_x: f32,
        from_y: f32,
        to_x: f32,
        to_y: f32,
        delay_ticks: u32,
    },
    MortarLaunch {
        from: u32,
        from_x: f32,
        from_y: f32,
        to_x: f32,
        to_y: f32,
        radius_tiles: f32,
        delay_ticks: u32,
    },
    MortarImpact {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<u32>,
        x: f32,
        y: f32,
        radius_tiles: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reveal: Option<AttackReveal>,
    },
    ArtilleryTarget {
        from: u32,
        x: f32,
        y: f32,
        radius_tiles: f32,
        delay_ticks: u32,
    },
    ArtilleryFiring {
        owner: u32,
        x: f32,
        y: f32,
        facing: f32,
    },
    ArtilleryImpact {
        x: f32,
        y: f32,
        radius_tiles: f32,
    },
    PanzerfaustLaunch {
        from: u32,
        from_x: f32,
        from_y: f32,
        to_x: f32,
        to_y: f32,
        delay_ticks: u32,
    },
    PanzerfaustImpact {
        x: f32,
        y: f32,
    },
    Notice {
        msg: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f32>,
        #[serde(default, skip_serializing_if = "NoticeSeverity::is_info")]
        severity: NoticeSeverity,
    },
}

/// Notice urgency. Alerts are allowed to cut through the mix and drive minimap pings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum NoticeSeverity {
    #[default]
    Info,
    Warn,
    Alert,
}

impl NoticeSeverity {
    pub fn is_info(&self) -> bool {
        matches!(self, NoticeSeverity::Info)
    }
}
