use super::*;

const TANK_STATIONARY_RANGE_RAMP_TICKS: u16 = crate::config::TICK_HZ as u16 * 3;

#[test]
fn moving_fire_move_orders_do_not_chase_targets_outside_weapon_range() {
    for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("moving-fire unit should spawn");
        entities
            .spawn_unit(2, EntityKind::Rifleman, 288.0, 100.0)
            .expect("enemy should spawn");
        if let Some(unit) = entities.get_mut(unit_id) {
            unit.set_order(Order::move_to(300.0, 100.0));
            unit.set_path(vec![(300.0, 100.0)]);
            unit.set_path_goal(Some((300.0, 100.0)));
            unit.mark_move_phase(MovePhase::Moving);
        }

        run_combat_tick(&mut entities);

        let unit = entities
            .get(unit_id)
            .expect("moving-fire unit should exist");
        assert_eq!(unit.target_id(), None, "{kind:?} should not acquire");
        assert_eq!(unit.path_goal(), Some((300.0, 100.0)), "{kind:?}");
        assert_eq!(unit.next_waypoint(), Some((300.0, 100.0)), "{kind:?}");
    }
}

#[test]
fn moving_fire_attack_move_chases_enemy_outside_weapon_range() {
    for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("moving-fire unit should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 288.0, 100.0)
            .expect("enemy should spawn");
        if let Some(unit) = entities.get_mut(unit_id) {
            unit.set_order(Order::attack_move_to(400.0, 100.0));
            unit.set_path(vec![(400.0, 100.0)]);
            unit.set_path_goal(Some((400.0, 100.0)));
            unit.mark_move_phase(MovePhase::Moving);
        }

        let map = open_map(20);
        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );

        let unit = entities
            .get(unit_id)
            .expect("moving-fire unit should exist");
        assert_eq!(unit.target_id(), Some(enemy_id), "{kind:?} should acquire");
        assert_ne!(unit.path_goal(), Some((400.0, 100.0)), "{kind:?}");
        assert!(!unit.path_is_empty(), "{kind:?} should chase");
    }
}

#[test]
fn meth_rifleman_move_does_not_chase_targets_outside_weapon_range() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 288.0, 100.0)
        .expect("enemy should spawn");
    if let Some(rifleman) = entities.get_mut(rifleman_id) {
        rifleman.set_order(Order::move_to(400.0, 100.0));
        rifleman.set_path(vec![(400.0, 100.0)]);
        rifleman.set_path_goal(Some((400.0, 100.0)));
        rifleman.mark_move_phase(MovePhase::Moving);
    }

    let map = open_map(20);
    let mut meth_player = player_state(1, false);
    meth_player.upgrades.insert(UpgradeKind::Methamphetamines);
    run_combat_tick_on_map(&mut entities, &[meth_player, player_state(2, false)], &map);

    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), None);
    assert_eq!(rifleman.path_goal(), Some((400.0, 100.0)));
    assert_eq!(rifleman.next_waypoint(), Some((400.0, 100.0)));
}

#[test]
fn meth_rifleman_attack_move_chases_targets_outside_weapon_range() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 288.0, 100.0)
        .expect("enemy should spawn");
    if let Some(rifleman) = entities.get_mut(rifleman_id) {
        rifleman.set_order(Order::attack_move_to(400.0, 100.0));
        rifleman.set_path(vec![(400.0, 100.0)]);
        rifleman.set_path_goal(Some((400.0, 100.0)));
        rifleman.mark_move_phase(MovePhase::Moving);
    }

    let map = open_map(20);
    let mut meth_player = player_state(1, false);
    meth_player.upgrades.insert(UpgradeKind::Methamphetamines);
    run_combat_tick_on_map(&mut entities, &[meth_player, player_state(2, false)], &map);

    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), Some(enemy_id));
    assert_eq!(rifleman.path_goal(), Some((288.0, 100.0)));
    assert!(!rifleman.path_is_empty());
}

#[test]
fn meth_rifleman_chase_goal_uses_target_center_without_vehicle_standoff() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 288.0, 100.0)
        .expect("enemy should spawn");
    let map = open_map(20);
    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    let enemy = entities.get(enemy_id).expect("enemy should exist");
    let profile = combat_rules::attack_profile(EntityKind::Rifleman);
    let range_px =
        profile.range_tiles as f32 * config::TILE_SIZE as f32 + rifleman.radius() + RANGE_SLACK;
    let goal = chase_goal_for_target(
        &map,
        &entities,
        rifleman_id,
        (100.0, 100.0),
        (enemy.pos_x, enemy.pos_y),
        range_px,
        dist2(100.0, 100.0, enemy.pos_x, enemy.pos_y).sqrt(),
    );
    assert_eq!(
        goal,
        (enemy.pos_x, enemy.pos_y),
        "meth riflemen should not route direct attacks through vehicle standoff policy"
    );
}

#[test]
fn non_moving_fire_attack_move_still_chases_out_of_range_targets() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 260.0, 100.0)
        .expect("enemy should spawn");
    if let Some(rifleman) = entities.get_mut(rifleman_id) {
        rifleman.set_order(Order::attack_move_to(400.0, 100.0));
        rifleman.set_path(Vec::new());
        rifleman.set_path_goal(Some((400.0, 100.0)));
        rifleman.mark_move_phase(MovePhase::Moving);
    }

    let map = open_map(20);
    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    let enemy = entities.get(enemy_id).expect("enemy should exist");
    assert_eq!(rifleman.target_id(), Some(enemy_id));
    assert_eq!(rifleman.path_goal(), Some((enemy.pos_x, enemy.pos_y)));
}

#[test]
fn attack_move_prefers_in_range_armored_fallback_over_out_of_range_soft_target() {
    let map = open_map(20);
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let in_range_tank = entities
        .spawn_unit(2, EntityKind::Tank, 180.0, 100.0)
        .expect("fallback tank should spawn");
    let out_of_range_worker = entities
        .spawn_unit(2, EntityKind::Worker, 300.0, 100.0)
        .expect("preferred soft target should spawn");
    if let Some(rifleman) = entities.get_mut(rifleman_id) {
        rifleman.set_order(Order::attack_move_to(500.0, 100.0));
        rifleman.set_path(vec![(500.0, 100.0)]);
        rifleman.set_path_goal(Some((500.0, 100.0)));
        rifleman.mark_move_phase(MovePhase::Moving);
    }

    let target = resolve_test_target(
        &map,
        &entities,
        &default_team_relations(),
        rifleman_id,
        256.0,
    );

    assert_eq!(target, Some(in_range_tank));
    assert_ne!(target, Some(out_of_range_worker));
}

#[test]
fn direct_tank_attack_chases_to_standoff_range_instead_of_target_center() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 288.0, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_order(Order::attack(enemy_id));
        tank.set_path(vec![(96.0, 100.0)]);
        tank.set_path_goal(Some((96.0, 100.0)));
        tank.set_last_repath_tick(10);
    }

    let map = open_map(20);
    let old_goal = entities
        .get(tank_id)
        .expect("tank should exist")
        .path_goal()
        .expect("old goal should exist");

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let tank = entities.get(tank_id).expect("tank should exist");
    let enemy = entities.get(enemy_id).expect("enemy should exist");
    let goal = tank.path_goal().expect("tank should keep a chase goal");
    let profile = combat_rules::attack_profile(EntityKind::Tank);
    let range_px =
        profile.range_tiles as f32 * config::TILE_SIZE as f32 + tank.radius() + RANGE_SLACK;
    let goal_to_enemy = dist2(goal.0, goal.1, enemy.pos_x, enemy.pos_y).sqrt();

    assert_ne!(goal, old_goal);
    assert_ne!(goal, (enemy.pos_x, enemy.pos_y));
    assert!(
        goal_to_enemy < range_px,
        "standoff goal should be comfortably inside weapon range"
    );
}

#[test]
fn stationary_tank_range_linearly_ramps_to_fourteen_tiles() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let base_range = combat_rules::attack_profile(EntityKind::Tank).range_tiles as f32;

    assert_range_near(tank_range_tiles(&entities, tank_id), base_range);

    for _ in 0..(TANK_STATIONARY_RANGE_RAMP_TICKS / 2) {
        run_combat_tick(&mut entities);
    }
    assert_range_near(
        tank_range_tiles(&entities, tank_id),
        (base_range + 14.0) * 0.5,
    );

    for _ in 0..(TANK_STATIONARY_RANGE_RAMP_TICKS / 2) {
        run_combat_tick(&mut entities);
    }
    assert_range_near(tank_range_tiles(&entities, tank_id), 14.0);

    for _ in 0..10 {
        run_combat_tick(&mut entities);
    }
    assert_range_near(tank_range_tiles(&entities, tank_id), 14.0);
}

#[test]
fn tank_path_translation_resets_stationary_range_to_base() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    fully_charge_tank_range(&mut entities);
    assert_range_near(tank_range_tiles(&entities, tank_id), 14.0);

    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_order(Order::move_to(180.0, 100.0));
        tank.set_path(vec![(180.0, 100.0)]);
        tank.set_path_goal(Some((180.0, 100.0)));
        tank.mark_move_phase(MovePhase::Moving);
    }
    run_open_movement_tick(&mut entities);

    assert_range_near(
        tank_range_tiles(&entities, tank_id),
        combat_rules::attack_profile(EntityKind::Tank).range_tiles as f32,
    );
}

#[test]
fn tank_path_pivot_without_translation_preserves_stationary_range() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    fully_charge_tank_range(&mut entities);
    assert_range_near(tank_range_tiles(&entities, tank_id), 14.0);

    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_facing(std::f32::consts::PI);
        tank.set_order(Order::move_to(300.0, 100.0));
        tank.set_path(vec![(300.0, 100.0)]);
        tank.set_path_goal(Some((300.0, 100.0)));
        tank.mark_move_phase(MovePhase::Moving);
    }
    run_open_movement_tick(&mut entities);

    let tank = entities.get(tank_id).expect("tank should exist");
    assert_eq!(
        tank.movement_delta(),
        (0.0, 0.0),
        "high-error tank pivot should not translate on the reset tick"
    );
    assert_range_near(tank_range_tiles(&entities, tank_id), 14.0);
}

#[test]
fn fully_stationary_tank_can_fire_at_extended_range() {
    let map = open_map(24);
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    fully_charge_tank_range(&mut entities);

    let target_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 484.0, 100.0)
        .expect("target should spawn");
    entities
        .spawn_unit(1, EntityKind::Worker, 460.0, 100.0)
        .expect("spotter should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_order(Order::attack(target_id));
        tank.set_weapon_facing(0.0);
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let target = entities.get(target_id).expect("target should still exist");
    assert_eq!(
        target.hp, 0,
        "stationary tank should fire at a target inside the 14-tile ramped range"
    );
}

#[test]
fn moving_range_tank_does_not_fire_at_extended_range_before_ramp() {
    let map = open_map(24);
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let target_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 484.0, 100.0)
        .expect("target should spawn");
    entities
        .spawn_unit(1, EntityKind::Worker, 460.0, 100.0)
        .expect("spotter should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_order(Order::attack(target_id));
        tank.set_weapon_facing(0.0);
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let target = entities.get(target_id).expect("target should exist");
    assert_eq!(
        target.hp, 45,
        "base-range tank should not fire at a target that only the stationary ramp can reach"
    );
}

fn fully_charge_tank_range(entities: &mut EntityStore) {
    for _ in 0..TANK_STATIONARY_RANGE_RAMP_TICKS {
        run_combat_tick(entities);
    }
}

fn tank_range_tiles(entities: &EntityStore, tank_id: u32) -> f32 {
    let tank = entities.get(tank_id).expect("tank should exist");
    effective_attack_profile(tank).range_tiles
}

fn run_open_movement_tick(entities: &mut EntityStore) {
    let map = open_map(24);
    let occ = Occupancy::build(&map, entities);
    let spatial = SpatialIndex::build(entities, map.size);
    movement_system(&map, entities, &mut [], &occ, &spatial, 0);
}

fn assert_range_near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.001,
        "expected range {expected}, got {actual}"
    );
}
