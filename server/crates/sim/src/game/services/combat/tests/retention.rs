use super::*;

fn tank_with_retained_target(
    map: &Map,
    tank_pos: (f32, f32),
    retained_owner: u32,
    retained_pos: (f32, f32),
    fallback_pos: (f32, f32),
) -> (EntityStore, u32, u32, u32) {
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, tank_pos.0, tank_pos.1)
        .expect("tank should spawn");
    let retained = entities
        .spawn_unit(
            retained_owner,
            EntityKind::Worker,
            retained_pos.0,
            retained_pos.1,
        )
        .expect("retained target should spawn");
    let fallback = entities
        .spawn_unit(2, EntityKind::Worker, fallback_pos.0, fallback_pos.1)
        .expect("fallback target should spawn");
    entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_order(Order::move_to(
            map.tile_center(8, 4).0,
            map.tile_center(4, 4).1,
        ));
    entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_target_id(Some(retained));
    (entities, tank, retained, fallback)
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_friendly() {
    let map = open_map(8);
    let (entities, tank, _retained, fallback) =
        tank_with_retained_target(&map, (100.0, 100.0), 1, (150.0, 100.0), (120.0, 130.0));

    assert_eq!(
        resolve_test_target(&map, &entities, &default_team_relations(), tank, 192.0),
        Some(fallback)
    );
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_hidden() {
    let map = open_map(24);
    let (entities, tank, retained, fallback) =
        tank_with_retained_target(&map, (100.0, 100.0), 2, (356.0, 100.0), (130.0, 100.0));
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], &entities, &map);
    let retained_entity = entities
        .get(retained)
        .expect("retained target should exist");
    assert!(
        !fog.is_visible_world(1, retained_entity.pos_x, retained_entity.pos_y),
        "test setup requires the retained target to be hidden"
    );
    let smokes = SmokeCloudStore::new();
    let tank_entity = entities.get(tank).expect("tank should exist");

    let target = resolve_target(
        &map,
        &entities,
        &default_team_relations(),
        &spatial,
        &los,
        &fog,
        &smokes,
        tank,
        tank_entity.owner,
        tank_entity.pos_x,
        tank_entity.pos_y,
        512.0,
        combat_mode(tank_entity),
    );

    assert_eq!(target, Some(fallback));
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_smoke_covered() {
    let map = open_map(12);
    let (entities, tank, retained, fallback) =
        tank_with_retained_target(&map, (100.0, 100.0), 2, (150.0, 100.0), (120.0, 130.0));
    let mut smokes = SmokeCloudStore::new();
    let retained_entity = entities
        .get(retained)
        .expect("retained target should exist");
    smokes
        .spawn(retained_entity.pos_x, retained_entity.pos_y, 1.0, 100, 0)
        .expect("smoke should spawn");
    let los = LineOfSight::with_smoke(&map, &smokes);
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut fog = Fog::new(map.size);
    fog.recompute_with_smoke(&[1, 2], &entities, &map, &smokes);
    let tank_entity = entities.get(tank).expect("tank should exist");

    let target = resolve_target(
        &map,
        &entities,
        &default_team_relations(),
        &spatial,
        &los,
        &fog,
        &smokes,
        tank,
        tank_entity.owner,
        tank_entity.pos_x,
        tank_entity.pos_y,
        192.0,
        combat_mode(tank_entity),
    );

    assert_eq!(target, Some(fallback));
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_not_fireable() {
    let map = map_with_rock_at((3, 4));
    let attacker_pos = map.tile_center(2, 4);
    let blocked_pos = map.tile_center(6, 4);
    let fallback_pos = map.tile_center(2, 5);
    let (entities, tank, retained, fallback) =
        tank_with_retained_target(&map, attacker_pos, 2, blocked_pos, fallback_pos);
    assert_eq!(entities.get(retained).map(|target| target.owner), Some(2));

    assert_eq!(
        resolve_test_target(&map, &entities, &default_team_relations(), tank, 512.0),
        Some(fallback)
    );
}
