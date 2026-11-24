//! Nearest-first branch-and-bound search on a one-pixel landing lattice, plus the exact click.
//! The cast range bounds the search; there is no extra snap-distance limit.
use super::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub(crate) fn nearest_blink_landing(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    caster: &Entity,
    requested: (f32, f32),
    range: f32,
) -> Option<(f32, f32)> {
    let blockers = UnitBlockers::new(entities);
    let origin = (caster.pos_x(), caster.pos_y());
    let legal = |pos: (f32, f32)| {
        (pos.0 - origin.0).det_hypot(pos.1 - origin.1) <= range + 0.0001
            && unit_static_standable_with_radius(
                map,
                occ,
                caster.kind,
                pos.0,
                pos.1,
                caster.facing(),
                Some(caster.radius()),
            )
            && blockers.placement_clear(caster, pos)
    };
    if legal(requested) {
        return Some(requested);
    }
    let bounds = (
        ((origin.0 - range).max(0.0) - requested.0).ceil() as i32,
        ((origin.1 - range).max(0.0) - requested.1).ceil() as i32,
        ((origin.0 + range).min(map.world_width_px()) - requested.0).floor() as i32,
        ((origin.1 + range).min(map.world_height_px()) - requested.1).floor() as i32,
    );
    let mut queue = BinaryHeap::new();
    enqueue(&mut queue, bounds);
    while let Some(Reverse((_, (x0, y0, x1, y1)))) = queue.pop() {
        let closest = (
            origin
                .0
                .clamp(requested.0 + x0 as f32, requested.0 + x1 as f32),
            origin
                .1
                .clamp(requested.1 + y0 as f32, requested.1 + y1 as f32),
        );
        if (closest.0 - origin.0).det_hypot(closest.1 - origin.1) > range {
            continue;
        }
        if x0 == x1 && y0 == y1 {
            let pos = (requested.0 + x0 as f32, requested.1 + y0 as f32);
            if legal(pos) {
                return Some(pos);
            }
            continue;
        }
        let center = (
            requested.0 + (x0 + x1) as f32 * 0.5,
            requested.1 + (y0 + y1) as f32 * 0.5,
        );
        let radius = ((x1 - x0) as f32).det_hypot((y1 - y0) as f32) * 0.5 + 0.002;
        // An entire region inside an occupied body cannot contain a legal landing.
        if let Some(body) =
            super::unit_blockers::body_at(caster, center, caster.facing(), caster.radius())
        {
            if entities.iter().any(|other| {
                other.id != caster.id
                    && other.hp > 0
                    && other.is_unit()
                    && unit_body_for_entity(other)
                        .and_then(|b| unit_body_overlap(body, b))
                        .is_some_and(|overlap| overlap.depth > radius)
            }) {
                continue;
            }
        }
        let tx0 = ((requested.0 + x0 as f32) / config::TILE_SIZE as f32).floor() as i32;
        let tx1 = ((requested.0 + x1 as f32) / config::TILE_SIZE as f32).floor() as i32;
        let ty0 = ((requested.1 + y0 as f32) / config::TILE_SIZE as f32).floor() as i32;
        let ty1 = ((requested.1 + y1 as f32) / config::TILE_SIZE as f32).floor() as i32;
        if tx0 == tx1
            && ty0 == ty1
            && (!map.in_bounds(tx0, ty0)
                || !map.is_passable(tx0, ty0)
                || !occ.passable_for_kind(tx0, ty0, caster.kind))
        {
            continue;
        }
        if x1 - x0 >= y1 - y0 {
            let mid = x0 + (x1 - x0) / 2;
            enqueue(&mut queue, (x0, y0, mid, y1));
            enqueue(&mut queue, (mid + 1, y0, x1, y1));
        } else {
            let mid = y0 + (y1 - y0) / 2;
            enqueue(&mut queue, (x0, y0, x1, mid));
            enqueue(&mut queue, (x0, mid + 1, x1, y1));
        }
    }
    None
}

type SearchQueue = BinaryHeap<Reverse<(i64, (i32, i32, i32, i32))>>;
fn enqueue(queue: &mut SearchQueue, bounds: (i32, i32, i32, i32)) {
    let (x0, y0, x1, y1) = bounds;
    if x0 > x1 || y0 > y1 {
        return;
    }
    let x = i64::from(0.clamp(x0, x1));
    let y = i64::from(0.clamp(y0, y1));
    queue.push(Reverse((x * x + y * y, bounds)));
}

#[cfg(test)]
#[path = "blink_tests.rs"]
mod tests;
