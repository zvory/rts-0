use super::*;

#[test]
fn produced_unit_rallies_to_nearest_free_tile() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let city_centre = spawn_building_training(
        &map,
        &mut entities,
        10,
        10,
        EntityKind::CityCentre,
        EntityKind::Worker,
    );
    let blocked = map.tile_center(18, 10);
    entities
        .spawn_unit(2, EntityKind::Worker, blocked.0, blocked.1)
        .expect("rally destination blocker should spawn");
    let rally = (blocked.0 + 14.0, blocked.1);
    entities
        .get_mut(city_centre)
        .expect("city centre")
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
    let mut players = vec![player(1)];

    tick_production(&map, &mut entities, &mut players);

    let worker = entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker && entity.hp > 0)
        .expect("worker should spawn");
    assert_eq!(worker.path_goal(), Some(map.tile_center(19, 10)));
    assert_eq!(
        entities
            .get(city_centre)
            .expect("city centre")
            .rally_point(),
        Some(rally),
        "resolving this unit's destination must not move the producer's rally marker"
    );
}
