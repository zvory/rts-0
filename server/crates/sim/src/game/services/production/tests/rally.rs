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
    let blocker = entities
        .spawn_unit(1, EntityKind::Worker, blocked.0, blocked.1)
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
        .find(|entity| {
            entity.id != blocker
                && entity.owner == 1
                && entity.kind == EntityKind::Worker
                && entity.hp > 0
        })
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

#[test]
fn simultaneous_production_reserves_distinct_rally_tiles() {
    let map = flat_map(40);
    let mut entities = EntityStore::new();
    let left = spawn_building_training(
        &map,
        &mut entities,
        5,
        5,
        EntityKind::CityCentre,
        EntityKind::Worker,
    );
    let right = spawn_building_training(
        &map,
        &mut entities,
        28,
        5,
        EntityKind::CityCentre,
        EntityKind::Worker,
    );
    let rally = map.tile_center(20, 20);
    for producer in [left, right] {
        entities
            .get_mut(producer)
            .expect("city centre")
            .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
    }
    let mut players = vec![player(1)];

    tick_production(&map, &mut entities, &mut players);

    let goals = entities
        .iter()
        .filter(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
        .filter_map(|entity| entity.path_goal())
        .collect::<Vec<_>>();
    assert_eq!(goals.len(), 2, "both workers should receive rally orders");
    assert_ne!(
        goals[0], goals[1],
        "an earlier produced unit's pending destination should reserve its tile"
    );
    assert!(
        goals.contains(&rally),
        "the first produced worker should keep the exact free rally tile"
    );
}

#[test]
fn produced_unit_uses_nearest_reachable_free_rally_tile() {
    let mut map = flat_map(32);
    for ty in 0..map.size {
        let index = map.index(16, ty);
        map.terrain[index] = terrain::ROCK;
    }
    let mut entities = EntityStore::new();
    let city_centre = spawn_building_training(
        &map,
        &mut entities,
        6,
        10,
        EntityKind::CityCentre,
        EntityKind::Worker,
    );
    let rally = map.tile_center(24, 10);
    entities
        .get_mut(city_centre)
        .expect("city centre")
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
    let mut players = vec![player(1)];

    tick_production(&map, &mut entities, &mut players);

    let goal = entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
        .and_then(|entity| entity.path_goal())
        .expect("worker should receive a reachable rally goal");
    assert!(
        goal.0 < 16.0 * crate::config::TILE_SIZE as f32,
        "the rally goal must stay on the worker's side of the impassable wall, got {goal:?}"
    );
}

#[test]
fn hidden_enemy_does_not_change_produced_unit_rally_goal() {
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
    let rally = map.tile_center(24, 24);
    let enemy = entities
        .spawn_unit(2, EntityKind::Worker, rally.0, rally.1)
        .expect("hidden enemy should spawn");
    entities
        .get_mut(city_centre)
        .expect("city centre")
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
    let mut players = vec![player(1), player(2)];

    tick_production(&map, &mut entities, &mut players);

    let produced = entities
        .iter()
        .find(|entity| entity.id != enemy && entity.owner == 1 && entity.kind == EntityKind::Worker)
        .expect("worker should be produced");
    assert_eq!(
        produced.path_goal(),
        Some(rally),
        "hidden enemy occupancy must not leak through the produced unit's rally order"
    );
}

#[test]
fn visible_enemy_blocks_produced_unit_rally_goal() {
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
    let rally = map.tile_center(24, 24);
    entities
        .spawn_unit(2, EntityKind::Worker, rally.0, rally.1)
        .expect("visible enemy should spawn");
    let observer = map.tile_center(24, 20);
    entities
        .spawn_unit(1, EntityKind::Rifleman, observer.0, observer.1)
        .expect("friendly observer should spawn");
    entities
        .get_mut(city_centre)
        .expect("city centre")
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
    let mut players = vec![player(1), player(2)];
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], &entities, &map);

    tick_production_with_fog(&map, &mut entities, &mut players, &fog);

    let produced = entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
        .expect("worker should be produced");
    assert_ne!(
        produced.path_goal(),
        Some(rally),
        "visible enemy occupancy should reserve the selected rally tile"
    );
}

#[test]
fn enemy_future_goal_does_not_reserve_rally_tile() {
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
    let rally = map.tile_center(24, 24);
    let enemy_pos = map.tile_center(22, 24);
    let enemy = entities
        .spawn_unit(2, EntityKind::Worker, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    {
        let enemy = entities.get_mut(enemy).expect("enemy");
        enemy.replace_active_order(Order::move_to(rally.0, rally.1));
        enemy.set_path_goal(Some(rally));
        enemy.mark_move_phase(crate::game::entity::MovePhase::AwaitingPath);
    }
    let observer = map.tile_center(22, 20);
    entities
        .spawn_unit(1, EntityKind::Rifleman, observer.0, observer.1)
        .expect("friendly observer should spawn");
    entities
        .get_mut(city_centre)
        .expect("city centre")
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
    let mut players = vec![player(1), player(2)];
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], &entities, &map);

    tick_production_with_fog(&map, &mut entities, &mut players, &fog);

    let produced = entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
        .expect("worker should be produced");
    assert_eq!(
        produced.path_goal(),
        Some(rally),
        "an enemy's private future destination must not influence the rally order"
    );
}
