use super::*;

const TANK_STATIONARY_RANGE_RAMP_TICKS: u16 = crate::config::TICK_HZ as u16 * 3;
const MOVING_FIRE_ROUTE_GOAL: (f32, f32) = (500.0, 100.0);

fn visible_target_x_outside_weapon_range(entities: &EntityStore, attacker_id: u32) -> f32 {
    let attacker = entities
        .get(attacker_id)
        .expect("moving-fire attacker should exist for range setup");
    let profile = effective_attack_profile(attacker);
    let range_px = profile.range_tiles * config::TILE_SIZE as f32 + attacker.radius() + RANGE_SLACK;
    let target_x = attacker.pos_x + range_px + config::TILE_SIZE as f32;
    let sight_px = attacker.sight_tiles() as f32 * config::TILE_SIZE as f32;
    assert!(
        target_x - attacker.pos_x < sight_px,
        "{:?} fixture needs a visible target outside weapon range",
        attacker.kind
    );
    target_x
}

#[test]
fn moving_fire_move_orders_do_not_chase_targets_outside_weapon_range() {
    for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("moving-fire unit should spawn");
        let target_x = visible_target_x_outside_weapon_range(&entities, unit_id);
        entities
            .spawn_unit(2, EntityKind::Rifleman, target_x, 100.0)
            .expect("enemy should spawn");
        if let Some(unit) = entities.get_mut(unit_id) {
            unit.set_order(Order::move_to(
                MOVING_FIRE_ROUTE_GOAL.0,
                MOVING_FIRE_ROUTE_GOAL.1,
            ));
            unit.set_path(vec![MOVING_FIRE_ROUTE_GOAL]);
            unit.set_path_goal(Some(MOVING_FIRE_ROUTE_GOAL));
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
        assert_eq!(unit.target_id(), None, "{kind:?} should not acquire");
        assert_eq!(unit.path_goal(), Some(MOVING_FIRE_ROUTE_GOAL), "{kind:?}");
        assert_eq!(
            unit.next_waypoint(),
            Some(MOVING_FIRE_ROUTE_GOAL),
            "{kind:?}"
        );
    }
}

#[test]
fn moving_fire_attack_move_keeps_commanded_path_past_out_of_range_enemy() {
    for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("moving-fire unit should spawn");
        let target_x = visible_target_x_outside_weapon_range(&entities, unit_id);
        entities
            .spawn_unit(2, EntityKind::Rifleman, target_x, 100.0)
            .expect("enemy should spawn");
        if let Some(unit) = entities.get_mut(unit_id) {
            unit.set_order(Order::attack_move_to(
                MOVING_FIRE_ROUTE_GOAL.0,
                MOVING_FIRE_ROUTE_GOAL.1,
            ));
            unit.set_path(vec![MOVING_FIRE_ROUTE_GOAL]);
            unit.set_path_goal(Some(MOVING_FIRE_ROUTE_GOAL));
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
        assert_eq!(unit.target_id(), None, "{kind:?} should not acquire");
        assert_eq!(unit.path_goal(), Some(MOVING_FIRE_ROUTE_GOAL), "{kind:?}");
        assert!(!unit.path_is_empty(), "{kind:?} should keep moving");
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
fn meth_rifleman_attack_move_ignores_targets_outside_weapon_range() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    entities
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
    assert_eq!(rifleman.target_id(), None);
    assert_eq!(rifleman.path_goal(), Some((400.0, 100.0)));
    assert!(!rifleman.path_is_empty());
}

#[test]
fn non_moving_fire_attack_move_resumes_commanded_path_without_pursuit() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 300.0, 100.0)
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
    assert_eq!(rifleman.target_id(), None);
    assert_eq!(rifleman.path_goal(), Some((400.0, 100.0)));
    assert!(!rifleman.path_is_empty());
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
fn out_of_range_direct_tank_attack_creates_a_pursuit_path() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_x = visible_target_x_outside_weapon_range(&entities, tank_id);
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_x, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_order(Order::attack(enemy_id));
    }

    let map = open_map(20);

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let tank = entities.get(tank_id).expect("tank should exist");
    assert_eq!((tank.pos_x, tank.pos_y), (100.0, 100.0));
    assert!(!tank.path_is_empty());
    assert!(tank.path_goal().is_some());
    assert_eq!(tank.target_id(), None);
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
fn tank_path_pivot_without_translation_resets_stationary_range_to_base() {
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
    assert_range_near(
        tank_range_tiles(&entities, tank_id),
        combat_rules::attack_profile(EntityKind::Tank).range_tiles as f32,
    );
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

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert!(
        events
            .get(&1)
            .expect("tank owner events should exist")
            .iter()
            .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == tank_id && *to == target_id)),
        "stationary tank should fire at a target inside the 14-tile ramped range, even when the shell's infantry dodge roll misses"
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
