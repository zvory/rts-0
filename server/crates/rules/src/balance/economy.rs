//! Economy, resource-node, and movement-fuel balance constants.

use super::{TICK_HZ, TILE_SIZE};

pub const TANK_OIL_COST_PER_PX: f32 = 20.0 / (96.0 * TILE_SIZE as f32);
pub const SCOUT_CAR_OIL_COST_PER_PX: f32 = 5.0 / (96.0 * TILE_SIZE as f32);
pub const TANK_OIL_STARVED_PAUSE_TICKS: u16 = TICK_HZ as u16;

pub const STARTING_STEEL: u32 = 75;
pub const STARTING_OIL: u32 = 0;
pub const STARTING_WORKERS: u32 = 6;

pub const STEEL_LOAD: u32 = 2;
pub const OIL_LOAD: u32 = 2;
pub const HARVEST_TICKS: u32 = 40;
pub const STEEL_PATCH_AMOUNT: u32 = 625;
// Twelve steel patches and three oil patches yield a 2.599:1 Steel/Oil base ratio, the nearest
// whole-unit oil capacity to the 2.6:1 target.
pub const OIL_GEYSER_AMOUNT: u32 = 962;
pub const STEEL_PATCHES_PER_BASE: u32 = 12;
pub const OIL_PATCHES_PER_BASE: u32 = 3;

pub const CC_RESOURCE_MIN_DIST_TILES: f32 = 3.5;
pub const CC_RESOURCE_MAX_DIST_TILES: f32 = 7.0;
// Two-tile mining buffer beyond authored/base resource cluster placement.
pub const MINING_CC_RANGE_TILES: f32 = 9.0;
pub const STEEL_BLOCK_DIST_TILES: f32 = 4.0;
pub const OIL_DIST_TILES: f32 = 6.0;
