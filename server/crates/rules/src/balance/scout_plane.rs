//! Hidden Scout Plane contract constants.
//!
//! Phase 2 makes these values mirrorable before normal production exposure. Hidden server runtime,
//! upkeep, and fog stamping land before later command-card and normal production exposure.

use super::TICK_HZ;

pub const SCOUT_PLANE_BUILD_TICKS: u32 = TICK_HZ * 20;
pub const SCOUT_PLANE_HP: u32 = 40;
pub const SCOUT_PLANE_SIGHT_TILES: u32 = 12;
pub const SCOUT_PLANE_SPEED_PX_PER_TICK: f32 = 2.0;
pub const SCOUT_PLANE_COST_STEEL: u32 = 50;
pub const SCOUT_PLANE_COST_OIL: u32 = 50;
pub const SCOUT_PLANE_SUPPLY: u32 = 0;
pub const SCOUT_PLANE_ORBIT_RADIUS_TILES: u32 = 4;
pub const SCOUT_PLANE_UPKEEP_OIL: u8 = 1;
pub const SCOUT_PLANE_UPKEEP_INTERVAL_TICKS: u16 = 20;
pub const SCOUT_PLANE_FUEL_RESERVE_OIL: u8 = 8;

// Client render/selection body only. The authoritative movement/collision body stays zero-radius
// in `defs.rs` until aerial movement owns its own non-blocking runtime path.
pub const SCOUT_PLANE_BODY_LENGTH_PX: f32 = 48.0;
pub const SCOUT_PLANE_BODY_WIDTH_PX: f32 = 34.0;
pub const SCOUT_PLANE_BODY_CLEARANCE_PX: f32 = 0.0;
