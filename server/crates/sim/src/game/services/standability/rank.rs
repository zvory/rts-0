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
mod tests {
    use super::*;

    #[test]
    fn entity_spawn_standability_preserves_vehicle_facing_for_terrain_and_units() {
        let mut map = Map {
            width: 12,
            height: 12,
            terrain: vec![crate::protocol::terrain::GRASS; 144],
            ..Default::default()
        };
        let rock = map.index(5, 5);
        map.terrain[rock] = crate::protocol::terrain::ROCK;
        let mut entities = EntityStore::new();
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, 80.0, 80.0)
            .unwrap();
        let candidate = map.tile_center(5, 4);
        let occ = Occupancy::build(&map, &entities);
        assert!(unit_spawn_standable_for_entity(
            &map,
            &occ,
            &entities,
            entities.get(tank).unwrap(),
            candidate.0,
            candidate.1,
        ));
        entities
            .get_mut(tank)
            .unwrap()
            .set_facing(std::f32::consts::FRAC_PI_2);
        assert!(!unit_spawn_standable_for_entity(
            &map,
            &occ,
            &entities,
            entities.get(tank).unwrap(),
            candidate.0,
            candidate.1,
        ));

        map.terrain[rock] = crate::protocol::terrain::GRASS;
        let blocker = map.tile_center(5, 5);
        entities
            .spawn_unit(1, EntityKind::Worker, blocker.0, blocker.1)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);
        assert!(!unit_spawn_standable_for_entity(
            &map,
            &occ,
            &entities,
            entities.get(tank).unwrap(),
            candidate.0,
            candidate.1,
        ));
        entities.get_mut(tank).unwrap().set_facing(0.0);
        assert!(unit_spawn_standable_for_entity(
            &map,
            &occ,
            &entities,
            entities.get(tank).unwrap(),
            candidate.0,
            candidate.1,
        ));
    }
}
