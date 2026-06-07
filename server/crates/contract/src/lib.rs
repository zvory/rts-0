//! Semantic message contracts shared across simulation, replay, protocol, and server boundaries.
//!
//! These DTOs describe game state and events independent of WebSocket envelopes or compact
//! transport encoding.

use serde::{Deserialize, Serialize};

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartPayload {
    pub player_id: u32,
    #[serde(default)]
    pub spectator: bool,
    pub tick: u32,
    pub map: MapInfo,
    pub players: Vec<PlayerStart>,
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
    pub name: String,
    pub color: String,
    pub start_tile_x: u32,
    pub start_tile_y: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerScore {
    pub id: u32,
    pub name: String,
    pub color: String,
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
    pub steel: u32,
    pub oil: u32,
    pub supply_used: u32,
    pub supply_cap: u32,
    pub entities: Vec<EntityView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_deltas: Vec<ResourceDelta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub smokes: Vec<SmokeCloudView>,
    pub events: Vec<Event>,
    /// Per-player resources for all players. Populated only in spectator/replay modes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_resources: Vec<PlayerResourceSnapshot>,
    /// Per-recipient server/network diagnostics for the current match.
    pub net_status: SnapshotNetStatus,
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
}

/// Resources for one player, included in no-fog snapshots so replay viewers see all players.
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
    pub prod_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_progress: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_queue: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_progress: Option<f32>,

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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub oil_used: Option<f32>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order_plan: Vec<OrderPlanMarker>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_cooldown_left: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abilities: Vec<AbilityCooldownView>,

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
            prod_kind: None,
            prod_progress: None,
            prod_queue: None,
            build_progress: None,
            latched_node: None,
            remaining: None,
            target_id: None,
            setup_state: None,
            setup_facing: None,
            rally: None,
            oil_used: None,
            order_plan: Vec::new(),
            charge_cooldown_left: None,
            abilities: Vec::new(),
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
