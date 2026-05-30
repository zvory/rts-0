pub mod combat;
pub mod commands;
pub mod construction;
pub mod death;
pub mod economy;
pub mod movement;
pub mod occupancy;
pub mod production;
pub mod supply;

use crate::game::entity::EntityStore;
use crate::game::map::Map;
use crate::game::pathfinding::{self, Passability};

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

/// Recompute an A* path for a unit toward a world point and store it.
pub(crate) fn repath<P: Passability>(
    map: &Map,
    entities: &mut EntityStore,
    occ: &P,
    id: u32,
    gx: f32,
    gy: f32,
) {
    let (sx, sy) = match entities.get(id) {
        Some(e) => map.tile_of(e.pos_x, e.pos_y),
        None => return,
    };
    let (gtx, gty) = map.tile_of(gx, gy);
    let path = pathfinding::find_path(occ, sx as i32, sy as i32, gtx as i32, gty as i32);
    let mut waypoints = pathfinding::to_world_waypoints(&path);
    if !waypoints.is_empty() {
        waypoints[0] = (gx, gy);
    } else {
        // Best-effort straight-line waypoint so the worker still nudges toward the goal.
        waypoints = vec![(gx, gy)];
    }
    if let Some(e) = entities.get_mut(id) {
        e.path = waypoints;
    }
}
