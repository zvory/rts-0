use crate::config;
use crate::game::entity::{EntityStore, Order};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::occupancy::Occupancy;

/// World pixels at which a unit is considered "arrived" at a waypoint / target point.
const ARRIVE_EPS: f32 = 2.0;

/// Advance every moving unit along its waypoint path at its speed. Clamps the final landing
/// tile to passable terrain (soft overlap with other units is allowed, so we don't resolve
/// unit-unit collisions here). Arriving at the last waypoint of a plain Move clears the order.
pub(crate) fn movement_system(map: &Map, entities: &mut EntityStore, occ: &Occupancy) {
    for id in entities.ids() {
        // Pull the data we need, then mutate.
        let (speed, mut x, mut y) = {
            let e = match entities.get(id) {
                Some(e) if e.is_unit() && !e.path.is_empty() => e,
                _ => continue,
            };
            let speed = config::unit_stats(e.kind).map(|s| s.speed).unwrap_or(0.0);
            (speed, e.pos_x, e.pos_y)
        };
        if speed <= 0.0 {
            continue;
        }

        let mut budget = speed;
        let mut new_facing = None;
        // Consume waypoints (stored reversed, next = last element) within this tick's budget.
        loop {
            let next = {
                let e = entities.get(id).unwrap();
                e.path.last().copied()
            };
            let Some((wx, wy)) = next else { break };
            let dx = wx - x;
            let dy = wy - y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= ARRIVE_EPS {
                // Reached this waypoint exactly; drop it and continue with the remaining budget.
                if let Some(e) = entities.get_mut(id) {
                    e.path.pop();
                }
                x = wx;
                y = wy;
                continue;
            }
            new_facing = Some(dy.atan2(dx));
            if dist <= budget {
                // We can reach this waypoint this tick.
                x = wx;
                y = wy;
                budget -= dist;
                if let Some(e) = entities.get_mut(id) {
                    e.path.pop();
                }
            } else {
                // Partial step toward the waypoint.
                let nx = x + dx / dist * budget;
                let ny = y + dy / dist * budget;
                // Clamp landing to a passable tile (don't slide into rock/water/buildings).
                if tile_passable_at(occ, map, nx, ny) {
                    x = nx;
                    y = ny;
                }
                break;
            }
        }

        if let Some(e) = entities.get_mut(id) {
            e.pos_x = x.clamp(0.0, map.world_size_px() - 0.01);
            e.pos_y = y.clamp(0.0, map.world_size_px() - 0.01);
            if let Some(f) = new_facing {
                e.facing = f;
            }
            // A plain Move with an empty path has arrived → go idle.
            if e.path.is_empty() {
                if let Order::Move { .. } = e.order {
                    e.order = Order::Idle;
                }
            }
        }
    }
}

/// Whether a world point lands on a passable (terrain + building) tile.
fn tile_passable_at(occ: &Occupancy, map: &Map, x: f32, y: f32) -> bool {
    let (tx, ty) = map.tile_of(x, y);
    occ.passable(tx as i32, ty as i32)
}
