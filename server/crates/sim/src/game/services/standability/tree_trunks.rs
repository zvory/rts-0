use super::*;

pub(super) fn body_clear(occupancy: &Occupancy<'_>, body: UnitBody) -> bool {
    let radius = crate::game::map::doodads::TREE_TRUNK_RADIUS_PX;
    let ts = config::TILE_SIZE as f32;
    let aabb = body.aabb();
    let bounds = (
        ((aabb.min_x - radius) / ts).floor() as i32,
        ((aabb.min_y - radius) / ts).floor() as i32,
        ((aabb.max_x + radius) / ts).floor() as i32,
        ((aabb.max_y + radius) / ts).floor() as i32,
    );
    occupancy
        .tree_trunks_in_tile_rect(bounds.0, bounds.1, bounds.2, bounds.3)
        .all(|(x, y)| !unit_body_intersects_circle(body, CircleBody { x, y, radius }))
}

pub(in crate::game::services) fn segment_clear(
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
) -> bool {
    if movement_body_class(kind) == MovementBodyClass::InfantryLike {
        return true;
    }
    let Some(body_radius) = unit_body(kind, from.0, from.1).map(UnitBody::bounding_radius) else {
        return false;
    };
    let trunk_radius = crate::game::map::doodads::TREE_TRUNK_RADIUS_PX;
    let sweep_radius = body_radius + trunk_radius;
    let ts = config::TILE_SIZE as f32;
    let bounds = (
        ((from.0.min(to.0) - sweep_radius) / ts).floor() as i32,
        ((from.1.min(to.1) - sweep_radius) / ts).floor() as i32,
        ((from.0.max(to.0) + sweep_radius) / ts).floor() as i32,
        ((from.1.max(to.1) + sweep_radius) / ts).floor() as i32,
    );
    occupancy
        .tree_trunks_in_tile_rect(bounds.0, bounds.1, bounds.2, bounds.3)
        .all(|trunk| point_segment_distance_sq(trunk, from, to) > sweep_radius * sweep_radius)
}

fn point_segment_distance_sq(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let delta = (end.0 - start.0, end.1 - start.1);
    let length_sq = delta.0 * delta.0 + delta.1 * delta.1;
    if length_sq <= f32::EPSILON {
        let offset = (point.0 - start.0, point.1 - start.1);
        return offset.0 * offset.0 + offset.1 * offset.1;
    }
    let t = (((point.0 - start.0) * delta.0 + (point.1 - start.1) * delta.1) / length_sq)
        .clamp(0.0, 1.0);
    let closest = (start.0 + delta.0 * t, start.1 + delta.1 * t);
    let offset = (point.0 - closest.0, point.1 - closest.1);
    offset.0 * offset.0 + offset.1 * offset.1
}
