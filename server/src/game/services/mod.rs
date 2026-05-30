pub mod combat;
pub mod commands;
pub mod construction;
pub mod death;
pub mod economy;
pub mod move_coordinator;
pub mod movement;
pub mod occupancy;
pub mod pathing;
pub mod production;
pub mod spatial;
pub mod supply;

use crate::game::entity::EntityStore;

/// Squared Euclidean distance.
pub(crate) fn dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

/// Distance (px) at which a worker is "in contact" with an entity for harvest / deposit /
/// construction. Accounts for the target's radius (a 3×3 Industrial Center is ~1.5 tiles wide)
/// so a worker standing just outside a footprint still counts as adjacent. `None` for a missing
/// entity.
pub(crate) fn interact_range(entities: &EntityStore, target: u32) -> Option<f32> {
    let t = entities.get(target)?;
    // Target half-extent + roughly one worker reach (one tile) of slack.
    Some(t.radius() + crate::config::TILE_SIZE as f32)
}
