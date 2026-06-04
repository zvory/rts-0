//! Simulation constants and compatibility stats helpers. See `DESIGN.md` §5.
//!
//! Kind-specific balance lives in `rules::defs`. `client/src/config.js` mirrors the subset
//! the UI / rendering / fog overlay needs (costs, supply, sight, sizes, colors). Keep both
//! in sync; when you change a number here that the UI shows, change it there too.

use crate::game::entity::EntityKind;
use crate::rules::defs;

// --- Timing -----------------------------------------------------------------
pub const TICK_HZ: u32 = 30;
pub const TICK_MS: u64 = 1000 / TICK_HZ as u64;

// --- Map --------------------------------------------------------------------
pub const TILE_SIZE: u32 = 32;

// --- Tolerant arrival -------------------------------------------------------
/// Movement below this many world pixels per tick counts as "no progress" for stuck detection.
pub const STUCK_EPS_PX: f32 = 2.0;
/// Consecutive ticks of no progress before a near-goal unit is considered arrived (~0.5 s at 30 Hz).
pub const STUCK_ARRIVAL_TICKS: u16 = 15;
/// A stuck unit within this radius of its `path_goal` is forcibly marked Arrived.
pub const TOLERANT_ARRIVAL_RADIUS_PX: f32 = 2.0 * TILE_SIZE as f32;

// --- Sidestep (mid-path unstick) --------------------------------------------
/// Consecutive stuck ticks before a mid-path unit injects a sidestep detour (~0.5 s at 30 Hz).
pub const SIDESTEP_TRIGGER_TICKS: u16 = 15;
/// Perpendicular detour distance in world pixels (~1 tile).
pub const SIDESTEP_DISTANCE_PX: f32 = TILE_SIZE as f32;
/// Ticks after a sidestep during which another sidestep is suppressed (~1 s at 30 Hz).
pub const SIDESTEP_COOLDOWN_TICKS: u16 = 30;
/// Consecutive ticks blocked by a static obstacle before queuing a repath (~1 s at 30 Hz).
pub const STATIC_BLOCKED_REPATH_TICKS: u16 = TICK_HZ as u16;

/// Radius within which an *intermediate* waypoint is considered reached. Tile centers are routing
/// hints; brushing within half a tile satisfies the route. Must be ≥ largest unit radius so two
/// units contesting the same tile center cannot both lock onto it simultaneously.
pub const ARRIVE_RADIUS_INTERMEDIATE_PX: f32 = TILE_SIZE as f32 * 0.5; // 16 px

/// Machine-gunner setup/teardown time. One second at the simulation tick rate.
pub const MACHINE_GUNNER_SETUP_TICKS: u16 = TICK_HZ as u16;

/// Experimental: tanks burn this much oil per world pixel of movement. Calibrated so a tank
/// driving from one edge of the 96-tile map to the other (3072 px) burns ~10 oil. When a
/// player has zero oil their tanks pause movement before retrying.
pub const TANK_OIL_COST_PER_PX: f32 = 10.0 / (96.0 * TILE_SIZE as f32);
/// Ticks a moving tank waits after an oil-starved movement attempt before checking fuel again.
pub const TANK_OIL_STARVED_PAUSE_TICKS: u16 = TICK_HZ as u16;

// --- Economy ----------------------------------------------------------------
pub const STARTING_STEEL: u32 = 75;
pub const STARTING_OIL: u32 = 0;
pub const STARTING_WORKERS: u32 = 4;
pub const QUICKSTART_STEEL: u32 = 99_999;
pub const QUICKSTART_OIL: u32 = 99_999;

pub const STEEL_LOAD: u32 = 2;
pub const OIL_LOAD: u32 = 2;
pub const HARVEST_TICKS: u32 = 40;
pub const STEEL_PATCH_AMOUNT: u32 = 1500;
pub const OIL_GEYSER_AMOUNT: u32 = 5000;
pub const STEEL_PATCHES_PER_BASE: u32 = 18;
pub const OIL_PATCHES_PER_BASE: u32 = 3;

/// Minimum distance (in tiles) from a City Centre's center to any starting resource node.
/// Prevents resources from spawning inside or too close to the building footprint.
pub const CC_RESOURCE_MIN_DIST_TILES: f32 = 3.5;

/// Maximum distance (in tiles) from a City Centre's center to any starting resource node.
/// Ensures no player is advantaged by resources being too far away.
pub const CC_RESOURCE_MAX_DIST_TILES: f32 = 7.0;

/// Maximum distance (in tiles) from a completed City Centre's center to a resource node for
/// workers to mine that node. Matches the starting resource layout bound so every main patch is
/// usable from the starting City Centre.
pub const MINING_CC_RANGE_TILES: f32 = CC_RESOURCE_MAX_DIST_TILES;

/// Distance (in tiles) from the City Centre to the center of the steel patch block.
pub const STEEL_BLOCK_DIST_TILES: f32 = 5.0;

/// Distance (in tiles) from the City Centre to the starting oil geyser.
pub const OIL_DIST_TILES: f32 = 6.0;

// --- Supply -----------------------------------------------------------------
pub const CITY_CENTRE_SUPPLY: u32 = 10;
pub const DEPOT_SUPPLY: u32 = 8;
pub const SUPPLY_CAP_MAX: u32 = 200;

// --- Vehicle bodies ----------------------------------------------------------
// Tank static legality uses the oriented hull instead of the legacy circular
// fallback radius. Dimensions are world px and include a small collision margin.
pub const TANK_BODY_LENGTH_PX: f32 = 42.0;
pub const TANK_BODY_WIDTH_PX: f32 = 24.0;
pub const TANK_BODY_CLEARANCE_PX: f32 = 1.5;

// --- Stats ------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct UnitStats {
    pub hp: u32,
    pub dmg: u32,
    pub range_tiles: u32,
    pub cooldown: u32, // ticks between attacks
    pub speed: f32,    // world px per tick
    pub sight_tiles: u32,
    pub cost_steel: u32,
    pub cost_oil: u32,
    pub supply: u32,
    pub build_ticks: u32,
    pub radius: f32, // collision / render radius in world px
}

impl UnitStats {
    /// Tile clearance radius for pathfinding: how many tiles around the center must be open.
    /// Units below half a tile of radius, including the v1 tank, are point-sized for coarse A*.
    pub fn radius_tiles(&self) -> u32 {
        (self.radius / TILE_SIZE as f32).round() as u32
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuildingStats {
    pub hp: u32,
    pub sight_tiles: u32,
    pub cost_steel: u32,
    pub cost_oil: u32,
    pub foot_w: u32, // footprint in tiles
    pub foot_h: u32,
    pub build_ticks: u32,
    pub provides_supply: u32,
    // dmg == 0 means the building does not attack.
    pub dmg: u32,
    pub range_tiles: u32,
    pub cooldown: u32,
}

/// Stats for a unit kind, or `None` if `kind` is not a unit.
pub fn unit_stats(kind: EntityKind) -> Option<UnitStats> {
    defs::unit_def(kind).map(|d| d.stats)
}

/// Stats for a building kind, or `None` if `kind` is not a building.
pub fn building_stats(kind: EntityKind) -> Option<BuildingStats> {
    defs::building_def(kind).map(|d| d.stats)
}
