use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;

/// Advance each building's front production item; on completion spawn the unit adjacent to the
/// building and remove the item from the queue. Supply was already reserved on enqueue, so
/// spawning does not re-charge it. Cost was charged at enqueue too.
pub(crate) fn production_system(
    map: &Map,
    entities: &mut EntityStore,
    _players: &mut [crate::game::PlayerState],
    _events: &mut std::collections::HashMap<u32, Vec<crate::protocol::Event>>,
) {
    for id in entities.ids() {
        // Is this a finished building with a non-empty queue?
        let (owner, kind, completed_unit) = {
            let b = match entities.get_mut(id) {
                Some(b) if b.is_building() && !b.under_construction && !b.prod_queue.is_empty() => {
                    b
                }
                _ => continue,
            };
            let front = &mut b.prod_queue[0];
            front.progress += 1;
            if front.progress >= front.total {
                let unit = b.prod_queue.remove(0).unit;
                (b.owner, b.kind, Some(unit))
            } else {
                (b.owner, b.kind, None)
            }
        };

        if let Some(unit) = completed_unit {
            // Spawn adjacent to the building footprint.
            let (bx, by) = match entities.get(id) {
                Some(b) => (b.pos_x, b.pos_y),
                None => continue,
            };
            let (sx, sy) = spawn_point_near(map, kind, bx, by);
            entities.spawn_unit(owner, unit, sx, sy);
        }
    }
}

/// A reasonable spawn point just outside a building's footprint toward the map below it.
fn spawn_point_near(map: &Map, building_kind: EntityKind, bx: f32, by: f32) -> (f32, f32) {
    let ts = config::TILE_SIZE as f32;
    let half = config::building_stats(building_kind)
        .map(|s| (s.foot_h as f32 * ts) * 0.5)
        .unwrap_or(ts);
    // Prefer spawning below the building; clamp into the world.
    let max = map.world_size_px() - 1.0;
    let x = bx.clamp(0.0, max);
    let y = (by + half + ts * 0.5).clamp(0.0, max);
    (x, y)
}
