use crate::config;
use crate::game::entity::{EntityStore, Order};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;

/// World pixels at which a unit is considered "arrived" at a waypoint / target point.
const ARRIVE_EPS: f32 = 2.0;

/// Separation push factor: how many world pixels a unit is nudged away from an overlapping
/// neighbor per tick. This is a soft collision — units can still path through each other, but
/// they no longer stack perfectly.
const SEPARATION_PUSH: f32 = 0.5;

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

/// Apply a soft separation force so units don't stack perfectly. For each unit, query the
/// spatial index for other units within one tile and push away from them.
pub(crate) fn separation(entities: &mut EntityStore, spatial: &SpatialIndex, map: &Map) {
    let sep_radius = config::TILE_SIZE as f32; // 1 tile
    let ids = entities.ids();
    let mut pushes: Vec<(u32, f32, f32)> = Vec::new();

    for id in &ids {
        let (px, py) = match entities.get(*id) {
            Some(e) if e.is_unit() => (e.pos_x, e.pos_y),
            _ => continue,
        };

        let mut dx = 0.0;
        let mut dy = 0.0;
        let mut count = 0usize;

        for nid in spatial.ids_in_circle_bbox(px, py, sep_radius) {
            if nid == *id {
                continue;
            }
            if let Some(neighbor) = entities.get(nid) {
                if !neighbor.is_unit() {
                    continue;
                }
                let d2 = (neighbor.pos_x - px) * (neighbor.pos_x - px)
                    + (neighbor.pos_y - py) * (neighbor.pos_y - py);
                if d2 < sep_radius * sep_radius && d2 > 0.0 {
                    let dist = d2.sqrt();
                    let ndx = (px - neighbor.pos_x) / dist;
                    let ndy = (py - neighbor.pos_y) / dist;
                    // Weight by inverse distance so closer neighbors push harder.
                    let weight = (sep_radius - dist) / sep_radius;
                    dx += ndx * weight;
                    dy += ndy * weight;
                    count += 1;
                }
            }
        }

        if count > 0 {
            pushes.push((*id, dx * SEPARATION_PUSH, dy * SEPARATION_PUSH));
        }
    }

    for (id, push_x, push_y) in pushes {
        if let Some(e) = entities.get_mut(id) {
            e.pos_x = (e.pos_x + push_x).clamp(0.0, map.world_size_px() - 0.01);
            e.pos_y = (e.pos_y + push_y).clamp(0.0, map.world_size_px() - 0.01);
        }
    }
}
