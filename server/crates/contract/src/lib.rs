//! Semantic message contracts shared across simulation, replay, protocol, and server boundaries.
//!
//! These DTOs describe game state and events independent of WebSocket envelopes or compact
//! transport encoding.

use serde::{Deserialize, Serialize};

pub type TeamId = u32;
pub const DEFAULT_FACTION_ID: &str = "kriegsia";
/// Current version of the embedded authoritative `rts.gameCheckpoint` payload.
pub const GAME_CHECKPOINT_CURRENT_VERSION: u32 = 2;
/// Maximum raw submitted ids in an ordinary multi-unit command.
pub const MAX_UNITS_PER_COMMAND: usize = 256;
/// Maximum raw submitted ids in a Lab command that bypasses ordinary command limits.
pub const LAB_MAX_UNITS_PER_COMMAND: usize = 4_096;
/// Maximum durable ground-mark records repeated in one recipient-scoped snapshot delta.
pub const MAX_GROUND_DECALS_PER_SNAPSHOT_DELTA: usize = 64;

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
    /// Authoritative per-connection observer perspective for privileged spectators. Active
    /// players do not receive this because their view is fixed by their owned seat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observer_view: Option<ObserverViewSelection>,
    pub tick: u32,
    pub map: MapInfo,
    pub players: Vec<PlayerStart>,
}

/// A read-only observer's selected perspective. This is a server-to-client reflection of the
/// authoritative view, and shares the wire shape of a vision-selection request.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ObserverViewSelection {
    All,
    Player { player_id: u32 },
    Players { player_ids: Vec<u32> },
    Omniscient,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoomTimeSeekState {
    pub id: u32,
    pub controller_id: u32,
    pub from_tick: u32,
    pub target_tick: u32,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seek: Option<RoomTimeSeekState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapInfo {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    /// Row-major terrain codes, length = width * height.
    pub terrain: Vec<u8>,
    /// Row-major static elevation levels, length = width * height.
    ///
    /// The server owns and distributes elevation. The simulation consumes local direction for
    /// movement speed and absolute level for a capped sight bonus; elevation does not occlude
    /// visibility or alter combat line of sight.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elevation: Vec<u8>,
    /// Optional authored presentation conditions for static elevation lighting. Flat maps omit
    /// this field and retain the renderer's unlit terrain path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sun: Option<MapSun>,
    /// Positions of all neutral resource nodes (steel/oil). Included so the
    /// client can render them on the minimap before fog-of-war reveals them.
    pub resources: Vec<ResourceNode>,
    /// Static authored map objects sent once at match start. Tree records also provide
    /// authoritative trunk collision/pathing inputs. Entity-backed authored objects such as Tank
    /// Traps are excluded from this client-visible projection and arrive through fog-filtered
    /// snapshots instead.
    #[serde(default)]
    pub doodads: Vec<MapDoodad>,
    /// Sparse tile overlay whose unit occupants are concealed from enemies until revealed.
    #[serde(default)]
    pub concealment_tiles: Vec<MapTile>,
    /// Sparse tile overlay blocked only for vehicle-body movement.
    #[serde(default)]
    pub no_vehicle_tiles: Vec<MapTile>,
    /// Sparse tile overlay that rejects building footprints.
    #[serde(default)]
    pub no_building_tiles: Vec<MapTile>,
    /// Sparse tile overlay where infantry cannot dig or occupy trenches.
    #[serde(default)]
    pub no_entrenchment_tiles: Vec<MapTile>,
    /// Sparse tile overlay that reduces incoming damage to occupants by 25%.
    #[serde(default)]
    pub damage_reduction_tiles: Vec<MapTile>,
    /// Sparse tile overlay that reduces occupant movement speed by 25%.
    #[serde(default)]
    pub slow_movement_tiles: Vec<MapTile>,
}

/// Authored static sunlight used only to render elevated terrain.
///
/// Azimuth follows compass degrees in map space: 0 is north (negative tile Y), 90 is east.
/// Elevation is degrees above the horizon. Warmth is a bounded 0-100 presentation grade.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapSun {
    pub azimuth_degrees: u16,
    pub elevation_degrees: u8,
    pub warmth: u8,
}

/// A canonical map tile coordinate used by sparse authored overlays.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapTile {
    pub x: u32,
    pub y: u32,
}

/// A server-validated map object authored in integer world pixels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapDoodad {
    pub id: u32,
    pub type_id: String,
    pub x: u32,
    pub y: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapDoodadClass {
    Tree,
    Wildflower,
    TankTrap,
}

pub const MAX_MAP_DOODADS: usize = 4_096;
pub const MAP_TILE_SIZE_PX: u32 = 32;
pub const MAP_DOODAD_TYPE_IDS: [&str; 7] = [
    "tree.oak",
    "tree.pine",
    "tree.spruce",
    "tree.alder",
    "wildflower.single",
    "wildflower.cluster",
    "unit.tank_trap",
];

pub fn classify_map_doodad(type_id: &str) -> Option<MapDoodadClass> {
    match type_id {
        "tree.oak" | "tree.pine" | "tree.spruce" | "tree.alder" => Some(MapDoodadClass::Tree),
        "wildflower.single" | "wildflower.cluster" => Some(MapDoodadClass::Wildflower),
        "unit.tank_trap" => Some(MapDoodadClass::TankTrap),
        _ => None,
    }
}

/// Validate a canonical doodad list at already overflow-checked world dimensions.
pub fn validate_map_doodads(
    doodads: &[MapDoodad],
    world_width_px: u32,
    world_height_px: u32,
) -> Result<(), String> {
    if doodads.len() > MAX_MAP_DOODADS {
        return Err(format!(
            "doodads must contain at most {MAX_MAP_DOODADS} entries"
        ));
    }
    let mut previous_id = 0;
    for (index, doodad) in doodads.iter().enumerate() {
        if doodad.id == 0 {
            return Err(format!("doodads[{index}].id must be nonzero"));
        }
        if doodad.id <= previous_id {
            return Err(format!(
                "doodads must be ordered by ascending unique id; id {} at index {index} is not canonical",
                doodad.id
            ));
        }
        previous_id = doodad.id;
        if doodad.x >= world_width_px || doodad.y >= world_height_px {
            return Err(format!(
                "doodads[{index}] position ({},{}) is outside the {world_width_px}x{world_height_px}px map",
                doodad.x, doodad.y
            ));
        }
        let Some(class) = classify_map_doodad(&doodad.type_id) else {
            return Err(format!(
                "doodads[{index}].typeId {:?} is not in the server catalog",
                doodad.type_id
            ));
        };
        match (class, doodad.color.as_deref()) {
            (MapDoodadClass::Tree | MapDoodadClass::TankTrap, Some(_)) => {
                return Err(format!(
                    "doodads[{index}].color is only allowed for wildflowers"
                ))
            }
            (MapDoodadClass::Wildflower, Some(color)) if !canonical_hex_color(color) => {
                return Err(format!(
                    "doodads[{index}].color must use canonical lowercase #rrggbb"
                ))
            }
            _ => {}
        }
        if class == MapDoodadClass::TankTrap
            && (doodad.x % MAP_TILE_SIZE_PX != MAP_TILE_SIZE_PX / 2
                || doodad.y % MAP_TILE_SIZE_PX != MAP_TILE_SIZE_PX / 2)
        {
            return Err(format!(
                "doodads[{index}] tank trap must be centered on a map tile"
            ));
        }
    }
    Ok(())
}

fn canonical_hex_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod map_doodad_tests {
    use super::*;

    fn flower(id: u32) -> MapDoodad {
        MapDoodad {
            id,
            type_id: "wildflower.single".to_string(),
            x: 12,
            y: 34,
            color: Some("#e05a91".to_string()),
        }
    }

    #[test]
    fn map_doodad_validator_enforces_count_and_canonical_ids() {
        let too_many = (1..=(MAX_MAP_DOODADS as u32 + 1))
            .map(flower)
            .collect::<Vec<_>>();
        assert!(validate_map_doodads(&too_many, 1_024, 1_024)
            .expect_err("count cap")
            .contains("at most"));
        assert!(validate_map_doodads(&[flower(2), flower(1)], 1_024, 1_024)
            .expect_err("canonical order")
            .contains("ascending unique id"));
        assert!(validate_map_doodads(&[flower(1), flower(1)], 1_024, 1_024)
            .expect_err("duplicate ids")
            .contains("ascending unique id"));
    }

    #[test]
    fn map_doodad_validator_enforces_rectangular_world_bounds() {
        let mut doodad = flower(1);
        doodad.x = 639;
        doodad.y = 319;
        validate_map_doodads(&[doodad.clone()], 640, 320).expect("last pixel is in bounds");

        doodad.x = 640;
        let error = validate_map_doodads(&[doodad.clone()], 640, 320)
            .expect_err("width bound must be independent");
        assert!(error.contains("640x320px map"), "error was: {error}");

        doodad.x = 639;
        doodad.y = 320;
        let error = validate_map_doodads(&[doodad], 640, 320)
            .expect_err("height bound must be independent");
        assert!(error.contains("640x320px map"), "error was: {error}");
    }

    #[test]
    fn map_doodad_catalog_is_unique_and_classifies_every_species_variant() {
        for (index, type_id) in MAP_DOODAD_TYPE_IDS.iter().enumerate() {
            assert!(
                !MAP_DOODAD_TYPE_IDS[..index].contains(type_id),
                "duplicate doodad type id {type_id}"
            );
            let class = classify_map_doodad(type_id).expect("catalog id classifies");
            if type_id.starts_with("tree.") {
                assert_eq!(class, MapDoodadClass::Tree);
            } else if *type_id == "unit.tank_trap" {
                assert_eq!(class, MapDoodadClass::TankTrap);
            } else {
                assert_eq!(class, MapDoodadClass::Wildflower);
            }
        }
    }
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
    /// Recipient-scoped cursor for reliable, request-driven ground decal reconciliation.
    #[serde(default)]
    pub ground_decal_revision: u32,
    /// Bounded, fog-scoped tail of decals discovered after `after_revision`. Repeated snapshots
    /// make this fast path safe when latest-only snapshot delivery replaces an older frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_decal_delta: Option<GroundDecalDelta>,
    /// Coarse world combat area shared identically with every recipient for directional ambience.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_combat_position: Option<[f32; 2]>,
    pub steel: u32,
    pub oil: u32,
    pub supply_used: u32,
    pub supply_cap: u32,
    /// Authoritative settings for the player whose private resources this snapshot projects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_build: Option<AutoBuildSettingsSnapshot>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_tiles: Vec<u8>,
    /// Row-major authoritative cumulative exploration grid for this recipient.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explored_tiles: Vec<u8>,
    /// Recipient-only stale enemy building intel. These records are last-seen memory, not live
    /// entities: clients may render them as non-interactive fog silhouettes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remembered_buildings: Vec<RememberedBuildingView>,
    /// Recipient-only last-observed deployed enemy Anti-Tank Gun firing arcs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remembered_anti_tank_guns: Vec<RememberedAntiTankGunView>,
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upgrades: Vec<String>,
    /// Per-player resources for the projected observer players.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_resources: Vec<PlayerResourceSnapshot>,
    /// Per-recipient server/network diagnostics for the current match.
    pub net_status: SnapshotNetStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutoBuildSettingsSnapshot {
    pub paused: bool,
    pub reserve_steel: u32,
    pub reserve_oil: u32,
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
pub struct RememberedAntiTankGunView {
    pub id: u32,
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub facing: f32,
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

/// One durable authoritative ground mark. Visual randomness is fixed by `seed`, so clients can
/// rebuild the same decal texture after reconnects and replay seeks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroundDecalView {
    pub id: u32,
    pub decal_class: String,
    pub source_kind: String,
    pub x: f32,
    pub y: f32,
    pub owner: u32,
    pub seed: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_facing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_tiles: Option<f32>,
}

/// One immutable, spatially bounded tank-trail chunk. Coordinates are absolute quarter-pixels;
/// the third component is a signed i16 heading encoded in an i32 protocol slot, mapping
/// `[-32767, 32767]` to `[-pi, pi]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TankTrailView {
    pub id: u32,
    pub poses: Vec<[i32; 3]>,
}

/// A complete perspective-scoped decal range `(after_revision, snapshot revision]`. Clients may
/// advance their retained cursor only when they already cover `after_revision`; otherwise the
/// rows are still safe to present and the reliable repair path fills the gap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroundDecalDelta {
    pub after_revision: u32,
    pub decals: Vec<GroundDecalView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tank_trails: Vec<TankTrailView>,
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
    /// Rolling command-envelope rate over the last ten simulated seconds.
    pub apm: u32,
    /// Completed research for this projected player.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upgrades: Vec<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_recharge_left: Option<u16>,
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
    /// 0 = neutral (resource nodes and completed field obstacles), otherwise the owning player id.
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub max_hp: u32,
    pub state: String,

    /// Enemy units killed by this unit. Omitted for buildings and resource nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units_killed: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_facing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_range_tiles: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panzerfaust_loaded: Option<bool>,
    /// Normalized 0..1 progress while a visible Panzerfaust is winding up its loaded shot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panzerfaust_windup_progress: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_upgrade: Option<String>,
    /// Owner/allies only: ordered upgrade ids in this building's research queue.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prod_upgrade_queue: Vec<String>,
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

    /// Whether a visible completed Pump Jack can currently extract oil.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extractor_active: Option<bool>,

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
            units_killed: None,
            facing: None,
            weapon_facing: None,
            weapon_range_tiles: None,
            panzerfaust_loaded: None,
            panzerfaust_windup_progress: None,
            prod_kind: None,
            prod_upgrade: None,
            prod_upgrade_queue: Vec::new(),
            prod_progress: None,
            prod_queue: None,
            prod_repeat_kinds: Vec::new(),
            prod_scout_plane_queued: false,
            prod_waiting: false,
            extractor_active: None,
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
