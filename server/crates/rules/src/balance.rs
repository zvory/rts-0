//! Stable public balance export surface.
//!
//! Focused child modules own the underlying constants and helpers by domain. Downstream crates
//! should keep importing through `rts_rules::balance::*` unless a later contract change explicitly
//! narrows this public surface.

mod abilities;
mod bodies;
mod economy;
mod map;
mod stats;
mod supply;
mod support_weapons;
mod timing;
mod upgrades;

pub use abilities::*;
pub use bodies::*;
pub use economy::*;
pub use map::*;
pub use stats::*;
pub use supply::*;
pub use support_weapons::*;
pub use timing::*;
pub use upgrades::*;

// --- Sim movement compatibility ---------------------------------------------
// These movement/arrival recovery constants are consumed only by `rts-sim` movement services.
// They intentionally remain on the historical `rts_rules::balance::*` surface for this split so
// downstream imports do not break. A later design/API migration can move ownership beside the sim
// movement service once compatibility re-exports are no longer required.
pub const STUCK_EPS_PX: f32 = 2.0;
pub const STUCK_ARRIVAL_TICKS: u16 = 15;
pub const TOLERANT_ARRIVAL_RADIUS_PX: f32 = 2.0 * TILE_SIZE as f32;

pub const SIDESTEP_TRIGGER_TICKS: u16 = 15;
pub const SIDESTEP_DISTANCE_PX: f32 = TILE_SIZE as f32;
pub const SIDESTEP_COOLDOWN_TICKS: u16 = 30;
pub const STATIC_BLOCKED_REPATH_TICKS: u16 = TICK_HZ as u16;

pub const ARRIVE_RADIUS_INTERMEDIATE_PX: f32 = TILE_SIZE as f32 * 0.5;
pub const VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX: f32 = TILE_SIZE as f32 * 0.75;
pub const SCOUT_CAR_FINAL_GOAL_TOLERANCE_PX: f32 = TILE_SIZE as f32 * 0.375;
pub const SCOUT_CAR_STUCK_RECOVERY_TRIGGER_TICKS: u16 = STUCK_ARRIVAL_TICKS;
pub const SCOUT_CAR_REVERSE_RECOVERY_DISTANCE_PX: f32 = TILE_SIZE as f32 * 2.0;
pub const SCOUT_CAR_RECOVERY_COOLDOWN_TICKS: u16 = TICK_HZ as u16;
