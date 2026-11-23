use super::*;

pub(crate) fn unit_spawn_standable_for_entity(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    entity: &Entity,
    x: f32,
    y: f32,
) -> bool {
    let Some((x, y)) = stored_position(x, y) else {
        return false;
    };
    if !unit_static_standable_with_radius(
        map,
        occ,
        entity.kind,
        x,
        y,
        entity.facing(),
        Some(entity.radius()),
    ) {
        return false;
    }
    let Some(mut body) = unit_body_with_facing(entity.kind, x, y, entity.facing()) else {
        return false;
    };
    if entity.kind.is_disciple() {
        if let UnitBody::Circle(circle) = &mut body {
            circle.radius = entity.radius();
        }
    }
    entities.iter().all(|e| {
        e.hp == 0
            || !e.is_unit()
            || unit_body_for_entity(e).is_none_or(|other| !unit_bodies_intersect(body, other))
    })
}

#[cfg(test)]
#[path = "rank_tests.rs"]
mod tests;
