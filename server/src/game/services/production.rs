use crate::game::entity::EntityStore;
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;

/// Advance each building's front production item; on completion spawn the unit adjacent to the
/// building and remove the item from the queue. Supply was already reserved on enqueue, so
/// spawning does not re-charge it. Cost was charged at enqueue too.
pub(crate) fn production_system(
    _map: &Map,
    entities: &mut EntityStore,
    coordinator: &MoveCoordinator<'_>,
    _events: &mut std::collections::HashMap<u32, Vec<crate::protocol::Event>>,
) {
    for id in entities.ids() {
        // Is this a finished building with a non-empty queue?
        let (owner, kind, completed_unit) = {
            let b = match entities.get_mut(id) {
                Some(b)
                    if b.hp > 0
                        && b.is_building()
                        && !b.under_construction()
                        && !b.prod_queue().is_empty() =>
                {
                    b
                }
                _ => continue,
            };
            let Some(queue) = b.prod_queue_mut() else {
                continue;
            };
            let front = &mut queue[0];
            front.progress += 1;
            if front.progress >= front.total {
                let unit = queue.remove(0).unit;
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
            let (sx, sy) = coordinator.find_spawn_point(entities, kind, unit, bx, by);
            entities.spawn_unit(owner, unit, sx, sy);
        }
    }
}
