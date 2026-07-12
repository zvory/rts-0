//! Tick services used by [`crate::game::systems`].
//!
//! Services own simulation mutations below the public [`crate::game::Game`] seam: command
//! application, movement, combat, economy, production, construction, death cleanup, and the
//! derived spatial/occupancy/geometry queries those phases share. The tick orchestrator wires
//! these services together; individual services should stay focused on one phase or one reusable
//! query surface.

pub mod ability_orders;
pub mod combat;
pub mod commands;
pub mod construction;
pub mod death;
pub mod economy;
pub mod entrenchment;
pub mod geometry;
pub mod line_of_sight;
pub mod move_coordinator;
pub mod movement;
pub mod occupancy;
pub mod order_execution;
pub mod order_planner;
pub mod order_queue;
pub mod pathing;
pub mod production;
pub mod production_queue;
pub(in crate::game) mod scout_plane;
pub mod spatial;
pub mod standability;
pub mod supply;
pub mod world_query;

use crate::game::entity::EntityKind;

/// Squared Euclidean distance.
pub(crate) fn dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

/// Arrival distance for a worker walking to a building footprint of `kind` (footprint
/// half-extent + one tile of slack). Used for build-arrival checks before any entity for
/// the building exists.
pub(crate) fn interact_range_for_kind(kind: EntityKind) -> f32 {
    let ts = crate::config::TILE_SIZE as f32;
    let half = crate::config::building_stats(kind)
        .map(|s| (s.foot_w.max(s.foot_h) as f32) * ts * 0.5)
        .unwrap_or(ts * 0.5);
    half + ts
}
