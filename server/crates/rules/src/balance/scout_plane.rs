//! Scout Plane contract constants.

use super::TICK_HZ;

pub const SCOUT_PLANE_HP: u32 = 40;
pub const SCOUT_PLANE_SIGHT_TILES: u32 = 15;
pub const SCOUT_PLANE_SPEED_PX_PER_TICK: f32 = 2.0;
pub const SCOUT_PLANE_COST_STEEL: u32 = 50;
pub const SCOUT_PLANE_COST_OIL: u32 = 75;
pub const SCOUT_PLANE_SUPPLY: u32 = 0;
pub const SCOUT_PLANE_ORBIT_RADIUS_TILES: u32 = 4;
pub const SCOUT_PLANE_LIFETIME_TICKS: u16 = (TICK_HZ * 20) as u16;
pub const SCOUT_PLANE_ABILITY_COOLDOWN_TICKS: u16 = (TICK_HZ * 30) as u16;

// Client render/selection body only. The authoritative movement/collision body stays zero-radius
// in `defs.rs` until aerial movement owns its own non-blocking runtime path.
pub const SCOUT_PLANE_BODY_LENGTH_PX: f32 = 48.0;
pub const SCOUT_PLANE_BODY_WIDTH_PX: f32 = 34.0;
pub const SCOUT_PLANE_BODY_CLEARANCE_PX: f32 = 0.0;
