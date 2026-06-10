use super::*;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore, MovePhase, Order, WeaponSetup};
use crate::game::fog::Fog;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::movement::angle_delta;
use crate::game::services::movement::movement_system;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::ScoreState;
use crate::protocol::{terrain, NoticeSeverity};
use crate::rules::combat as combat_rules;
use rand::SeedableRng;

fn rifleman_with_enemy() -> (EntityStore, u32, u32) {
    let mut entities = EntityStore::new();
    let self_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy rifleman should spawn");
    (entities, self_id, enemy_id)
}

fn open_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4), (size - 5, size - 5)],
        expansion_sites: Vec::new(),
    }
}

fn map_with_rock_at(tile: (u32, u32)) -> Map {
    let mut map = open_map(12);
    map.terrain[(tile.1 * map.size + tile.0) as usize] = terrain::ROCK;
    map
}

fn visible_fog(map: &Map, entities: &EntityStore) -> Fog {
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], entities, map);
    fog
}

fn player_state(id: u32, is_ai: bool) -> PlayerState {
    PlayerState {
        id,
        name: format!("Player {id}"),
        color: "#fff".to_string(),
        start_tile: (4, 4),
        steel: 1_000,
        oil: 1_000,
        supply_used: 0,
        supply_cap: 20,
        is_ai,
        score: ScoreState::default(),
        upgrades: Default::default(),
    }
}

fn run_combat_tick(entities: &mut EntityStore) -> HashMap<u32, Vec<Event>> {
    run_combat_tick_with_players(entities, &[player_state(1, false), player_state(2, false)])
}

fn run_combat_tick_with_players(
    entities: &mut EntityStore,
    players: &[PlayerState],
) -> HashMap<u32, Vec<Event>> {
    let map = Map::generate(2, 0x00C0_FFEE);
    run_combat_tick_on_map(entities, players, &map)
}

fn run_combat_tick_on_map(
    entities: &mut EntityStore,
    players: &[PlayerState],
    map: &Map,
) -> HashMap<u32, Vec<Event>> {
    run_combat_tick_on_map_with_seed(entities, players, map, 0)
}

fn run_combat_tick_on_map_with_seed(
    entities: &mut EntityStore,
    players: &[PlayerState],
    map: &Map,
    rng_seed: u64,
) -> HashMap<u32, Vec<Event>> {
    let smokes = SmokeCloudStore::new();
    run_combat_tick_on_map_with_seed_and_smokes(entities, players, map, rng_seed, &smokes)
}

fn run_combat_tick_on_map_with_seed_and_smokes(
    entities: &mut EntityStore,
    players: &[PlayerState],
    map: &Map,
    rng_seed: u64,
    smokes: &SmokeCloudStore,
) -> HashMap<u32, Vec<Event>> {
    let occ = Occupancy::build(map, entities);
    let spatial = SpatialIndex::build(entities, map.size);
    let mut pathing = PathingService::new(256, 64);
    let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 10);
    let mut fog = Fog::new(map.size);
    fog.recompute_with_smoke(&[1, 2], entities, map, smokes);
    let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    let mut rng = SmallRng::seed_from_u64(rng_seed);
    let mut mortar_shells = crate::game::mortar::MortarShellStore::default();
    combat_system(
        map,
        entities,
        players,
        &occ,
        &spatial,
        &mut coordinator,
        &fog,
        smokes,
        &mut mortar_shells,
        &mut rng,
        &mut events,
        10,
    );
    events
}

fn run_movement_tick(entities: &mut EntityStore) {
    let map = Map::generate(2, 0x00C0_FFEE);
    let occ = Occupancy::build(&map, entities);
    let spatial = SpatialIndex::build(entities, map.size);
    movement_system(&map, entities, &mut [], &occ, &spatial, 0);
}

#[allow(clippy::too_many_arguments)]
fn apply_test_damage(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    attacker: u32,
    victim: u32,
    dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    range_px: f32,
) {
    let map = Map::generate(2, 0x00C0_FFEE);
    let fog = Fog::new(map.size);
    let smokes = SmokeCloudStore::new();
    let mut rng = SmallRng::seed_from_u64(0);
    apply_damage(
        &map,
        entities,
        events,
        &fog,
        &smokes,
        &mut rng,
        attacker,
        victim,
        dmg,
        attacker_owner,
        ax,
        ay,
        vx,
        vy,
        range_px,
        0.0,
        10,
    );
}

#[test]
fn idle_army_units_auto_acquire_targets() {
    let (entities, self_id, enemy_id) = rifleman_with_enemy();
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker = entities.get(self_id).expect("attacker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &spatial,
        &los,
        &fog,
        &smokes,
        self_id,
        attacker.owner,
        attacker.pos_x,
        attacker.pos_y,
        128.0,
        combat_mode(attacker),
    );

    assert_eq!(target, Some(enemy_id));
}

#[test]
fn move_orders_ignore_nearby_enemies() {
    let (mut entities, self_id, _) = rifleman_with_enemy();
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker = entities.get_mut(self_id).expect("attacker should exist");
    attacker.set_order(Order::move_to(300.0, 300.0));

    let target = resolve_target(
        &map,
        &entities,
        &spatial,
        &los,
        &fog,
        &smokes,
        self_id,
        1,
        100.0,
        100.0,
        128.0,
        combat_mode(entities.get(self_id).expect("attacker should exist")),
    );

    assert_eq!(target, None);
}

#[test]
fn attack_move_keeps_auto_acquisition() {
    let (mut entities, self_id, enemy_id) = rifleman_with_enemy();
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker = entities.get_mut(self_id).expect("attacker should exist");
    attacker.set_order(Order::attack_move_to(300.0, 300.0));

    let target = resolve_target(
        &map,
        &entities,
        &spatial,
        &los,
        &fog,
        &smokes,
        self_id,
        1,
        100.0,
        100.0,
        128.0,
        combat_mode(entities.get(self_id).expect("attacker should exist")),
    );

    assert_eq!(target, Some(enemy_id));
}

#[test]
fn stone_blocks_attack_move_auto_acquisition() {
    let map = map_with_rock_at((3, 4));
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let enemy_pos = map.tile_center(4, 4);
    let self_id = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    entities
        .get_mut(self_id)
        .expect("attacker should exist")
        .set_order(Order::attack_move_to(300.0, 300.0));
    let attacker = entities.get(self_id).expect("attacker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &spatial,
        &los,
        &fog,
        &smokes,
        self_id,
        attacker.owner,
        attacker.pos_x,
        attacker.pos_y,
        128.0,
        combat_mode(attacker),
    );

    assert_eq!(target, None);
}

#[test]
fn stone_blocks_explicit_attack_damage_until_shot_is_clear() {
    let map = map_with_rock_at((3, 4));
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let enemy_pos = map.tile_center(4, 4);
    let attacker_id = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    if let Some(attacker) = entities.get_mut(attacker_id) {
        attacker.set_weapon_setup(WeaponSetup::Deployed);
        attacker.set_order(Order::attack(enemy_id));
    }
    let before_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        before_hp
    );
    assert!(
        events
            .values()
            .flatten()
            .all(|event| !matches!(event, Event::Attack { .. })),
        "blocked shots should not emit attack tracers"
    );
}

#[test]
fn smoke_blocks_attack_move_auto_acquisition() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let enemy_pos = map.tile_center(6, 4);
    let smoke_pos = map.tile_center(4, 4);
    let self_id = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    let mut smokes = SmokeCloudStore::new();
    smokes
        .spawn(smoke_pos.0, smoke_pos.1, 1.0, 100, 0)
        .expect("smoke should spawn");
    let los = LineOfSight::with_smoke(&map, &smokes);
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut fog = Fog::new(map.size);
    fog.recompute_with_smoke(&[1, 2], &entities, &map, &smokes);
    entities
        .get_mut(self_id)
        .expect("attacker should exist")
        .set_order(Order::attack_move_to(300.0, 300.0));
    let attacker = entities.get(self_id).expect("attacker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &spatial,
        &los,
        &fog,
        &smokes,
        self_id,
        attacker.owner,
        attacker.pos_x,
        attacker.pos_y,
        256.0,
        combat_mode(attacker),
    );

    assert_eq!(target, None);
}

#[test]
fn smoke_blocks_explicit_attack_damage_until_shot_is_clear() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let enemy_pos = map.tile_center(8, 4);
    let smoke_pos = map.tile_center(5, 4);
    let attacker_id = entities
        .spawn_unit(1, EntityKind::AtTeam, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Tank, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    entities
        .get_mut(attacker_id)
        .expect("attacker should exist")
        .set_order(Order::attack(enemy_id));
    let mut smokes = SmokeCloudStore::new();
    smokes
        .spawn(smoke_pos.0, smoke_pos.1, 1.0, 100, 0)
        .expect("smoke should spawn");
    let before_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    let events = run_combat_tick_on_map_with_seed_and_smokes(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
        0,
        &smokes,
    );

    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        before_hp
    );
    assert!(
        events
            .values()
            .flatten()
            .all(|event| !matches!(event, Event::Attack { .. })),
        "smoke-blocked AT gun shots should not emit attack tracers"
    );
}

#[test]
fn units_inside_smoke_drop_retained_targets() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(4, 4);
    let enemy_pos = map.tile_center(5, 4);
    let attacker_id = entities
        .spawn_unit(1, EntityKind::Tank, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Worker, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    if let Some(attacker) = entities.get_mut(attacker_id) {
        attacker.set_order(Order::move_to(300.0, 100.0));
        attacker.set_target_id(Some(enemy_id));
    }
    let mut smokes = SmokeCloudStore::new();
    smokes
        .spawn(attacker_pos.0, attacker_pos.1, 1.0, 100, 0)
        .expect("smoke should spawn");

    run_combat_tick_on_map_with_seed_and_smokes(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
        0,
        &smokes,
    );

    assert_eq!(
        entities
            .get(attacker_id)
            .expect("attacker should exist")
            .target_id(),
        None
    );
}

#[test]
fn visible_damage_emits_positioned_under_attack_alert_to_victim_owner() {
    let mut entities = EntityStore::new();
    let attacker_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let victim_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("victim should spawn");
    entities
        .get_mut(attacker_id)
        .expect("attacker should exist")
        .set_order(Order::attack(victim_id));

    let events = run_combat_tick(&mut entities);
    let victim_events = events
        .get(&2)
        .expect("victim owner should have an event queue");

    assert!(
            victim_events
                .iter()
                .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker_id && *to == victim_id)),
            "victim owner should receive the visible attack event"
        );
    assert!(
            victim_events.iter().any(|event| matches!(
                event,
                Event::Notice {
                    msg,
                    x: Some(x),
                    y: Some(y),
                    severity: NoticeSeverity::Alert,
                } if msg == "alert:under_attack" && (*x - 120.0).abs() < 0.001 && (*y - 100.0).abs() < 0.001
            )),
            "victim owner should receive a positioned under-attack alert"
        );
}

#[test]
fn attack_move_resumes_original_destination_after_target_is_gone() {
    let mut entities = EntityStore::new();
    let attacker_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let attacker = entities
        .get_mut(attacker_id)
        .expect("attacker should exist");
    attacker.set_order(Order::attack_move_to(300.0, 300.0));
    attacker.set_path_goal(Some((270.0, 100.0)));
    attacker.set_path(Vec::new());

    let map = Map::generate(2, 0x00C0_FFEE);
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut pathing = PathingService::new(256, 64);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 0);
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1], &entities, &map);
    let smokes = SmokeCloudStore::new();
    let mut mortar_shells = crate::game::mortar::MortarShellStore::default();
    let mut events = HashMap::from([(1, Vec::new())]);

    let mut rng = SmallRng::seed_from_u64(0);
    combat_system(
        &map,
        &mut entities,
        &[player_state(1, false)],
        &occ,
        &spatial,
        &mut coordinator,
        &fog,
        &smokes,
        &mut mortar_shells,
        &mut rng,
        &mut events,
        10,
    );
    assert_eq!(
        entities
            .get(attacker_id)
            .expect("attacker should exist")
            .path_goal(),
        Some((300.0, 300.0))
    );
}

#[test]
fn attack_move_resumes_after_firing_cleared_path_before_arrival() {
    let mut entities = EntityStore::new();
    let attacker_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    if let Some(attacker) = entities.get_mut(attacker_id) {
        attacker.set_order(Order::attack_move_to(300.0, 100.0));
        attacker.set_path_goal(Some((300.0, 100.0)));
        attacker.set_path(Vec::new());
        attacker.mark_move_phase(MovePhase::Moving);
    }

    let map = open_map(16);
    run_combat_tick_on_map(&mut entities, &[player_state(1, false)], &map);

    let attacker = entities.get(attacker_id).expect("attacker should exist");
    assert_eq!(attacker.path_goal(), Some((300.0, 100.0)));
    assert_eq!(attacker.move_phase(), Some(MovePhase::Moving));
    assert!(
        !attacker.path_is_empty(),
        "attack-move should resume toward its original destination after firing cleared its path"
    );
}

#[test]
fn tank_keeps_moving_path_while_firing() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_facing(0.0);
        tank.set_weapon_facing(0.0);
        tank.set_order(Order::attack_move_to(300.0, 100.0));
        tank.set_path(vec![(300.0, 100.0)]);
        tank.set_path_goal(Some((300.0, 100.0)));
    }

    run_combat_tick(&mut entities);

    let tank = entities.get(tank_id).expect("tank should exist");
    assert_eq!(tank.target_id(), Some(enemy_id));
    assert!(
        !tank.path_is_empty(),
        "tank should keep its movement path while firing"
    );
    assert_eq!(tank.next_waypoint(), Some((300.0, 100.0)));
}

#[test]
fn tank_move_order_fires_without_leaving_move_path() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_facing(0.0);
        tank.set_weapon_facing(0.0);
        tank.set_order(Order::move_to(300.0, 100.0));
        tank.set_path(vec![(300.0, 100.0)]);
        tank.set_path_goal(Some((300.0, 100.0)));
    }

    run_combat_tick(&mut entities);

    let tank = entities.get(tank_id).expect("tank should exist");
    assert_eq!(tank.target_id(), Some(enemy_id));
    assert!(
        tank.attack_cd() > 0,
        "aligned moving tank turret should fire"
    );
    assert!(
        !tank.path_is_empty(),
        "moving tank should keep its movement path while firing"
    );
    assert_eq!(tank.next_waypoint(), Some((300.0, 100.0)));
}

#[test]
fn tank_move_order_does_not_chase_targets_outside_weapon_range() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 300.0, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_order(Order::move_to(300.0, 100.0));
        tank.set_path(vec![(300.0, 100.0)]);
        tank.set_path_goal(Some((300.0, 100.0)));
    }

    run_combat_tick(&mut entities);

    let tank = entities.get(tank_id).expect("tank should exist");
    assert_eq!(tank.target_id(), None);
    assert_eq!(tank.path_goal(), Some((300.0, 100.0)));
    assert_eq!(tank.next_waypoint(), Some((300.0, 100.0)));
}

#[test]
fn shoot_while_moving_units_keep_existing_valid_target() {
    for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        let retained_target_id = entities
            .spawn_unit(2, EntityKind::Worker, 150.0, 100.0)
            .expect("retained target should spawn");
        entities
            .spawn_unit(2, EntityKind::Worker, 120.0, 130.0)
            .expect("closer target should spawn");
        if let Some(attacker) = entities.get_mut(attacker_id) {
            attacker.set_order(Order::move_to(300.0, 100.0));
            attacker.set_target_id(Some(retained_target_id));
        }

        let map = open_map(8);
        let los = LineOfSight::new(&map);
        let spatial = SpatialIndex::build(&entities, map.size);
        let fog = visible_fog(&map, &entities);
        let smokes = SmokeCloudStore::new();
        let attacker = entities
            .get(attacker_id)
            .expect("attacker should still exist");

        let target = resolve_target(
            &map,
            &entities,
            &spatial,
            &los,
            &fog,
            &smokes,
            attacker_id,
            attacker.owner,
            attacker.pos_x,
            attacker.pos_y,
            192.0,
            combat_mode(attacker),
        );

        assert_eq!(
            target,
            Some(retained_target_id),
            "{kind} should stay focused"
        );
    }
}

#[test]
fn shoot_while_moving_units_reacquire_when_existing_target_is_dead() {
    for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        let dead_target_id = entities
            .spawn_unit(2, EntityKind::Worker, 150.0, 100.0)
            .expect("dead target should spawn");
        let new_target_id = entities
            .spawn_unit(2, EntityKind::Worker, 120.0, 130.0)
            .expect("new target should spawn");
        if let Some(dead_target) = entities.get_mut(dead_target_id) {
            dead_target.hp = 0;
        }
        if let Some(attacker) = entities.get_mut(attacker_id) {
            attacker.set_order(Order::move_to(300.0, 100.0));
            attacker.set_target_id(Some(dead_target_id));
        }

        let map = open_map(8);
        let los = LineOfSight::new(&map);
        let spatial = SpatialIndex::build(&entities, map.size);
        let fog = visible_fog(&map, &entities);
        let smokes = SmokeCloudStore::new();
        let attacker = entities
            .get(attacker_id)
            .expect("attacker should still exist");

        let target = resolve_target(
            &map,
            &entities,
            &spatial,
            &los,
            &fog,
            &smokes,
            attacker_id,
            attacker.owner,
            attacker.pos_x,
            attacker.pos_y,
            192.0,
            combat_mode(attacker),
        );

        assert_eq!(target, Some(new_target_id), "{kind} should reacquire");
    }
}

#[test]
fn tank_chases_to_standoff_range_instead_of_target_center() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 280.0, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_order(Order::attack_move_to(400.0, 100.0));
        tank.set_path(Vec::new());
        tank.set_path_goal(Some((400.0, 100.0)));
    }

    let map = open_map(20);
    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let tank = entities.get(tank_id).expect("tank should exist");
    let enemy = entities.get(enemy_id).expect("enemy should exist");
    let goal = tank.path_goal().expect("tank should request a chase path");
    let profile = combat_rules::attack_profile(EntityKind::Tank);
    let range_px =
        profile.range_tiles as f32 * config::TILE_SIZE as f32 + tank.radius() + RANGE_SLACK;
    let goal_to_enemy = dist2(goal.0, goal.1, enemy.pos_x, enemy.pos_y).sqrt();

    assert_ne!(goal, (enemy.pos_x, enemy.pos_y));
    assert!(
        goal_to_enemy < range_px,
        "standoff goal should be comfortably inside weapon range"
    );
}

#[test]
fn tank_chase_refreshes_stale_standoff_goal() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 288.0, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_order(Order::attack_move_to(500.0, 100.0));
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

    assert_ne!(goal, old_goal);
    assert!(
        goal.0 < enemy.pos_x,
        "tank should route to the near side of the target, not the target center"
    );
}

#[test]
fn rifleman_attack_move_without_charge_holds_position_while_firing() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    if let Some(rifleman) = entities.get_mut(rifleman_id) {
        rifleman.set_order(Order::attack_move_to(300.0, 100.0));
        rifleman.set_path(vec![(300.0, 100.0)]);
        rifleman.set_path_goal(Some((300.0, 100.0)));
    }

    run_combat_tick(&mut entities);

    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), Some(enemy_id));
    assert!(
        rifleman.path_is_empty(),
        "non-charged riflemen should still stop while firing"
    );
}

#[test]
fn charged_rifleman_move_order_keeps_path_while_firing() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    if let Some(rifleman) = entities.get_mut(rifleman_id) {
        rifleman.set_order(Order::move_to(300.0, 100.0));
        rifleman.set_path(vec![(300.0, 100.0)]);
        rifleman.set_path_goal(Some((300.0, 100.0)));
        rifleman.start_charge(config::RIFLEMAN_CHARGE_TICKS);
    }

    run_combat_tick(&mut entities);

    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), Some(enemy_id));
    assert!(
        !rifleman.path_is_empty(),
        "charged riflemen should keep their movement path while firing"
    );
}

#[test]
fn moving_charged_rifleman_does_not_take_miss_penalty() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    if let Some(rifleman) = entities.get_mut(rifleman_id) {
        rifleman.set_order(Order::move_to(300.0, 100.0));
        rifleman.set_path(vec![(300.0, 100.0)]);
        rifleman.set_path_goal(Some((300.0, 100.0)));
        rifleman.start_charge(config::RIFLEMAN_CHARGE_TICKS);
    }
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    run_combat_tick(&mut entities);

    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp.saturating_sub(5),
        "moving charged riflemen should fire with normal accuracy"
    );
}

#[test]
fn idle_workers_do_not_auto_acquire_targets() {
    let mut entities = EntityStore::new();
    let worker_id = entities
        .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
        .expect("worker should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy rifleman should spawn");
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let worker = entities.get(worker_id).expect("worker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &spatial,
        &los,
        &fog,
        &smokes,
        worker_id,
        worker.owner,
        worker.pos_x,
        worker.pos_y,
        128.0,
        combat_mode(worker),
    );

    assert_eq!(target, None);
}

#[test]
fn direct_hits_record_damage_signal_on_victim() {
    let mut entities = EntityStore::new();
    let worker_id = entities
        .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
        .expect("worker should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
        .expect("enemy rifleman should spawn");

    run_combat_tick(&mut entities);

    let worker = entities.get(worker_id).expect("worker should exist");
    assert!(
        worker.hp < worker.max_hp,
        "worker should have taken direct damage"
    );
    let pos = worker
        .last_damage_pos()
        .expect("victim should record attacker position");
    assert!(
        pos.0 < worker.pos_x,
        "recorded attacker position should be on the attacker's side"
    );
    assert!(
        worker.last_damage_tick().is_some(),
        "victim should record damage tick for diagnostics"
    );
}

#[test]
fn combat_no_longer_issues_retreat_orders() {
    let mut entities = EntityStore::new();
    let worker_id = entities
        .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
        .expect("worker should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
        .expect("enemy rifleman should spawn");

    run_combat_tick(&mut entities);

    let worker = entities.get(worker_id).expect("worker should exist");
    assert!(
        matches!(worker.order(), Order::Idle),
        "combat must not mutate orders"
    );
    assert_eq!(worker.path_goal(), None, "combat must not issue path goals");
}

#[test]
fn direct_hits_do_not_pull_workers_off_active_construction() {
    let mut entities = EntityStore::new();
    let worker_id = entities
        .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
        .expect("worker should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
        .expect("enemy rifleman should spawn");
    let site = entities
        .spawn_building(1, EntityKind::Depot, 160.0, 100.0, false)
        .expect("scaffold should spawn");
    if let Some(worker) = entities.get_mut(worker_id) {
        worker.set_order(Order::build(EntityKind::Depot, 4, 4));
        worker.mark_build_phase(BuildPhase::Constructing { site });
    }

    run_combat_tick(&mut entities);

    let worker = entities.get(worker_id).expect("worker should exist");
    assert!(
        matches!(worker.build_phase(), Some(BuildPhase::Constructing { .. })),
        "active builders remain latched so scaffolds are not stranded"
    );
}

#[test]
fn idle_machine_gunner_deploys_after_stationary_delay() {
    let mut entities = EntityStore::new();
    let mg_id = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");

    run_combat_tick(&mut entities);
    assert!(matches!(
        entities.get(mg_id).expect("mg should exist").weapon_setup(),
        WeaponSetup::SettingUp { .. }
    ));

    for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
        run_combat_tick(&mut entities);
    }

    assert_eq!(
        entities.get(mg_id).expect("mg should exist").weapon_setup(),
        WeaponSetup::Deployed
    );
}

#[test]
fn idle_machine_gunner_does_not_chase_distant_enemies() {
    let mut entities = EntityStore::new();
    let mg_id = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 330.0, 100.0)
        .expect("enemy should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    run_combat_tick(&mut entities);

    let mg = entities.get(mg_id).expect("mg should exist");
    assert_eq!(mg.target_id(), None);
    assert!(mg.path_is_empty(), "idle machine gunner should not chase");
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp,
        "distant enemies should not be attacked or chased"
    );
}

#[test]
fn machine_gunner_waits_to_deploy_before_first_shot() {
    let mut entities = EntityStore::new();
    entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    run_combat_tick(&mut entities);
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp
    );

    for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
        run_combat_tick(&mut entities);
    }

    assert!(
        entities.get(enemy_id).expect("enemy should exist").hp < enemy_hp,
        "machine gunner should fire once deployment completes"
    );
}

#[test]
fn idle_at_team_does_not_auto_setup() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("at team should spawn");

    run_combat_tick(&mut entities);

    assert_eq!(
        entities
            .get(at_id)
            .expect("at team should exist")
            .weapon_setup(),
        WeaponSetup::Packed
    );
}

#[test]
fn packed_at_team_fires_with_shorter_range_and_reduced_damage() {
    let mut entities = EntityStore::new();
    entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("at team should spawn");
    let tank_id = entities
        .spawn_unit(2, EntityKind::Tank, 220.0, 100.0)
        .expect("enemy tank should spawn");
    entities
        .get_mut(tank_id)
        .expect("tank should exist")
        .set_facing(std::f32::consts::PI);
    let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

    run_combat_tick(&mut entities);

    assert_eq!(
        entities.get(tank_id).expect("enemy should exist").hp,
        enemy_hp - 45,
        "packed AT gun should deal 75% of its deployed 60 damage"
    );
}

#[test]
fn deployed_at_team_fires_at_long_range() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("at team should spawn");
    let tank_id = entities
        .spawn_unit(2, EntityKind::Tank, 310.0, 100.0)
        .expect("enemy tank should spawn");
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }
    entities
        .get_mut(tank_id)
        .expect("tank should exist")
        .set_facing(std::f32::consts::PI);
    let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

    let events = run_combat_tick(&mut entities);

    assert!(
        entities.get(tank_id).expect("enemy should exist").hp < enemy_hp,
        "deployed AT team should fire at range 7"
    );
    assert!(
        events.get(&2).is_some_and(|events| events.iter().any(|event| {
            matches!(
                event,
                Event::Attack {
                    from,
                    to,
                    reveal: Some(reveal),
                    to_pos: Some(to_pos),
                } if *from == at_id
                    && *to == tank_id
                    && reveal.kind == crate::protocol::kind_to_wire(EntityKind::AtTeam)
                    && reveal.setup_state.as_deref() == Some(WeaponSetup::Deployed.to_protocol_str())
                    && *to_pos == [310.0, 100.0]
            )
        })),
        "AT attack event should carry shooter reveal and target position for visual feedback"
    );
}

#[test]
fn deployed_at_team_does_not_auto_acquire_targets_hidden_by_fog() {
    let map = open_map(24);
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("at team should spawn");
    let tank_id = entities
        .spawn_unit(2, EntityKind::Tank, 356.0, 100.0)
        .expect("enemy tank should spawn");
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }
    entities
        .get_mut(tank_id)
        .expect("tank should exist")
        .set_facing(std::f32::consts::PI);

    let mut fog = Fog::new(map.size);
    fog.recompute(&[1], &entities, &map);
    assert!(
        !fog.is_visible_world(
            1,
            entities.get(tank_id).expect("tank should exist").pos_x,
            entities.get(tank_id).expect("tank should exist").pos_y,
        ),
        "test setup requires the tank to be outside the AT owner's sight"
    );
    let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities.get(tank_id).expect("enemy should exist").hp,
        enemy_hp,
        "deployed AT guns must not fire at targets hidden by fog"
    );
    assert_eq!(
        entities
            .get(at_id)
            .expect("at team should exist")
            .target_id(),
        None,
        "hidden targets should not be retained as combat targets"
    );
    assert!(
        events
            .values()
            .flatten()
            .all(|event| !matches!(event, Event::Attack { .. })),
        "hidden-target suppression should not emit attack tracers"
    );
}

#[test]
fn at_team_turns_slowly_before_firing() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("at team should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Tank, 100.0, 20.0)
        .expect("enemy tank should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
    if let Some(at) = entities.get_mut(at_id) {
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
        at.set_weapon_setup(WeaponSetup::Deployed);
    }

    run_combat_tick(&mut entities);

    let at = entities.get(at_id).expect("at should exist");
    assert!(
        at.facing().abs() <= AT_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
        "AT gun should only slew by its turn-rate cap, got {:.4}",
        at.facing()
    );
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp,
        "AT gun should not fire until its barrel is aligned"
    );
}

#[test]
fn deployed_at_team_clamps_to_field_edge_and_does_not_fire_outside_arc() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("at team should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Tank, 100.0, 180.0)
        .expect("enemy tank should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    for _ in 0..20 {
        run_combat_tick(&mut entities);
    }

    let at = entities.get(at_id).expect("at should exist");
    let edge = config::AT_GUN_FIELD_OF_FIRE_RAD * 0.5;
    assert!(
        (at.facing() - edge).abs() <= AT_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
        "AT gun should clamp to the nearest arc edge, got {:.4}",
        at.facing()
    );
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp,
        "AT gun should not fire outside its deployed field of fire"
    );
}

#[test]
fn at_team_redeploy_rotates_during_teardown_and_setup() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("at team should spawn");
    let target = std::f32::consts::FRAC_PI_2;
    if let Some(at) = entities.get_mut(at_id) {
        at.set_emplacement_facing(Some(0.0));
        at.set_pending_redeploy_facing(Some(target));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
        at.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
            ticks: config::AT_TEAM_SETUP_TICKS,
        });
    }

    run_combat_tick(&mut entities);

    let facing_after_one_tick = entities.get(at_id).expect("at should exist").facing();
    assert!(
        facing_after_one_tick > 0.0
            && facing_after_one_tick <= AT_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
        "AT gun should start rotating toward its redeploy facing immediately, got {:.4}",
        facing_after_one_tick
    );

    for _ in 0..(config::AT_TEAM_SETUP_TICKS as usize * 2) {
        run_combat_tick(&mut entities);
    }

    let at = entities.get(at_id).expect("at should exist");
    assert_eq!(at.weapon_setup(), WeaponSetup::Deployed);
    assert!(
        (at.facing() - target).abs() <= AT_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
        "AT gun should finish redeploy facing the requested direction, got {:.4}",
        at.facing()
    );
}

#[test]
fn packed_at_team_rotates_before_setup_animation_begins() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("at team should spawn");
    let target = std::f32::consts::FRAC_PI_2;
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Packed);
        at.set_emplacement_facing(Some(target));
        at.set_desired_weapon_facing(target);
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    run_combat_tick(&mut entities);

    let at = entities.get(at_id).expect("at should exist");
    assert_eq!(
        at.weapon_setup(),
        WeaponSetup::Packed,
        "AT gun should stay packed until it has rotated into setup tolerance"
    );
    assert!(
        at.facing() > 0.0 && at.facing() <= AT_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
        "AT gun should begin rotating while still packed, got {:.4}",
        at.facing()
    );

    let mut saw_setting_up = false;
    for _ in 0..200 {
        run_combat_tick(&mut entities);
        let at = entities.get(at_id).expect("at should exist");
        if matches!(
            at.weapon_setup(),
            WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
        ) {
            saw_setting_up = true;
            assert!(
                angle_delta(at.facing(), target).abs() <= AT_GUN_FIRE_TOLERANCE_RAD + 0.001,
                "setup animation should begin only after the AT gun is aligned, got {:.4}",
                at.facing()
            );
            break;
        }
    }
    assert!(
        saw_setting_up,
        "AT gun should eventually start setup once it rotates into tolerance"
    );
}

#[test]
fn deployed_machine_gunner_can_fire_immediately() {
    let mut entities = EntityStore::new();
    let mg_id = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");

    run_combat_tick(&mut entities);
    for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
        run_combat_tick(&mut entities);
    }
    assert_eq!(
        entities.get(mg_id).expect("mg should exist").weapon_setup(),
        WeaponSetup::Deployed
    );

    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    run_combat_tick(&mut entities);

    assert!(
        entities.get(enemy_id).expect("enemy should exist").hp < enemy_hp,
        "deployed machine gunner should not wait for another setup cycle"
    );
}

#[test]
fn machine_gunner_tears_down_before_moving() {
    let mut entities = EntityStore::new();
    let mg_id = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");
    let start_x = entities.get(mg_id).expect("mg should exist").pos_x;

    {
        let mg = entities.get_mut(mg_id).expect("mg should exist");
        mg.set_weapon_setup(WeaponSetup::TearingDown {
            ticks: config::MACHINE_GUNNER_SETUP_TICKS,
        });
        mg.set_order(Order::move_to(120.0, 100.0));
        mg.set_path(vec![(120.0, 100.0)]);
        mg.set_path_goal(Some((120.0, 100.0)));
    }

    run_movement_tick(&mut entities);
    assert_eq!(entities.get(mg_id).expect("mg should exist").pos_x, start_x);

    for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
        run_combat_tick(&mut entities);
    }
    assert_eq!(
        entities.get(mg_id).expect("mg should exist").weapon_setup(),
        WeaponSetup::Packed
    );

    run_movement_tick(&mut entities);
    assert!(
        entities.get(mg_id).expect("mg should exist").pos_x > start_x,
        "machine gunner should move after teardown completes"
    );
}

#[test]
fn tank_combat_keeps_body_stable_and_rotates_turret() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 140.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_facing(0.0);
        tank.set_weapon_facing(0.0);
        tank.set_order(Order::attack(enemy_id));
    }

    run_combat_tick(&mut entities);

    let tank = entities.get(tank_id).expect("tank should exist");
    assert_eq!(
        tank.facing(),
        0.0,
        "tank combat should not rotate the hull once turret state exists"
    );
    assert!(
        tank.weapon_facing().unwrap_or(0.0) > 0.0
            && tank.weapon_facing().unwrap_or(0.0) <= TANK_TURRET_TURN_RATE_RAD_PER_TICK + 0.0001,
        "tank turret should rotate gradually toward target, got {:.4}",
        tank.weapon_facing().unwrap_or(0.0)
    );
    assert_eq!(
        tank.attack_cd(),
        0,
        "misaligned turret should not fire on the same tick it starts turning"
    );
}

#[test]
fn tank_cannot_fire_until_turret_aligned() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_facing(std::f32::consts::FRAC_PI_2);
        tank.set_weapon_facing(std::f32::consts::FRAC_PI_2);
        tank.set_order(Order::attack(enemy_id));
    }
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    for _ in 0..10 {
        run_combat_tick(&mut entities);
    }
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp,
        "tank should not damage the target while turret aim is outside tolerance"
    );
    assert_eq!(
        entities
            .get(tank_id)
            .expect("tank should exist")
            .attack_cd(),
        0,
        "tank cooldown should remain ready while firing is gated by turret alignment"
    );
    assert_eq!(
        entities.get(tank_id).expect("tank should exist").facing(),
        std::f32::consts::FRAC_PI_2,
        "turret aiming must not rotate the hull"
    );

    let mut fired = false;
    for _ in 0..80 {
        run_combat_tick(&mut entities);
        if entities
            .get(tank_id)
            .expect("tank should exist")
            .attack_cd()
            > 0
        {
            fired = true;
            break;
        }
    }

    assert!(
        fired,
        "tank should fire once its turret rotates inside tolerance"
    );
}

#[test]
fn tank_can_fire_outside_hull_facing_once_turret_aligned() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy should spawn");
    if let Some(tank) = entities.get_mut(tank_id) {
        tank.set_facing(std::f32::consts::PI);
        tank.set_weapon_facing(0.08);
        tank.set_order(Order::attack(enemy_id));
    }
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    run_combat_tick(&mut entities);

    let tank = entities.get(tank_id).expect("tank should exist");
    assert_eq!(
        tank.facing(),
        std::f32::consts::PI,
        "hull may remain pointed away from the target"
    );
    assert!(
        tank.attack_cd() > 0,
        "aligned turret should allow firing even when hull faces away"
    );
    assert!(
        entities.get(enemy_id).expect("enemy should exist").hp < enemy_hp,
        "target should take tank damage once turret is aligned"
    );
}

#[test]
fn tank_front_and_rear_hits_take_different_damage() {
    fn tank_hp_after_at_hit(attacker_pos: (f32, f32)) -> u32 {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::AtTeam, attacker_pos.0, attacker_pos.1)
            .expect("attacker should spawn");
        let victim = entities
            .spawn_unit(2, EntityKind::Tank, 100.0, 100.0)
            .expect("victim tank should spawn");
        entities
            .get_mut(victim)
            .expect("victim tank should exist")
            .set_facing(0.0);
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        events.insert(1, Vec::new());
        events.insert(2, Vec::new());

        apply_test_damage(
            &mut entities,
            &mut events,
            attacker,
            victim,
            48,
            1,
            attacker_pos.0,
            attacker_pos.1,
            100.0,
            100.0,
            128.0,
        );

        entities.get(victim).expect("victim tank should exist").hp
    }

    let front_hp = tank_hp_after_at_hit((140.0, 100.0));
    let rear_hp = tank_hp_after_at_hit((60.0, 100.0));

    assert_eq!(front_hp, 244);
    assert_eq!(rear_hp, 208);
    assert!(
        front_hp > rear_hp,
        "rear AT hits should deal more damage than front hits"
    );
}

#[test]
fn shots_overpenetrate_past_non_blocking_primary_target() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let primary = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("primary target should spawn");
    let secondary = entities
        .spawn_unit(2, EntityKind::Worker, 165.0, 100.0)
        .expect("secondary target should spawn");
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        primary,
        10,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(entities.get(primary).expect("primary should exist").hp, 35);
    let secondary = entities.get(secondary).expect("secondary should exist");
    assert_eq!(secondary.hp, 35);
    assert!(
        matches!(secondary.order(), Order::Idle),
        "overpenetration damage must not mutate worker orders"
    );
}

#[test]
fn missed_primary_shot_still_emits_attack_event() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("attacker should spawn");
    let victim = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("victim should spawn");
    let victim_hp = entities.get(victim).expect("victim should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        victim,
        48,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(
        entities.get(victim).expect("victim should exist").hp,
        victim_hp,
        "seeded AT shot should miss the infantry target"
    );
    assert!(
            events
                .get(&1)
                .expect("attacker owner events should exist")
                .iter()
                .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == victim)),
            "missed shots should still emit attack feedback for gun audio"
        );
    assert!(
        events
            .get(&2)
            .expect("victim owner events should exist")
            .iter()
            .all(
                |event| !matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack")
            ),
        "misses should not emit under-attack damage alerts"
    );
}

#[test]
fn at_team_seeded_shot_hits_scout_car_without_miss_roll() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
        .expect("attacker should spawn");
    let victim = entities
        .spawn_unit(2, EntityKind::ScoutCar, 140.0, 100.0)
        .expect("victim should spawn");
    let victim_hp = entities.get(victim).expect("victim should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        victim,
        48,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(
        entities.get(victim).expect("victim should exist").hp,
        victim_hp.saturating_sub(48),
        "seeded AT shot should not miss the scout car target"
    );
}

#[test]
fn shots_do_not_continue_into_resource_nodes() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let primary = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("primary target should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, 165.0, 100.0)
        .expect("resource node should spawn");
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        primary,
        10,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert_eq!(entities.get(primary).expect("primary should exist").hp, 35);
    assert_eq!(
        entities.get(node).expect("node should exist").remaining(),
        Some(1500)
    );
    assert_eq!(entities.get(node).expect("node should exist").hp, 1);
}

#[test]
fn tank_behind_primary_target_blocks_overpenetration() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let primary = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("primary target should spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::Tank, 165.0, 100.0)
        .expect("blocking tank should spawn");
    let behind = entities
        .spawn_unit(2, EntityKind::Worker, 190.0, 100.0)
        .expect("unit behind blocker should spawn");
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let behind_hp_before = entities.get(behind).expect("behind should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        primary,
        20,
        1,
        100.0,
        100.0,
        140.0,
        100.0,
        128.0,
    );

    assert!(
        entities.get(blocker).expect("blocker should exist").hp < blocker_hp_before,
        "tank behind the primary target should take overpenetration damage"
    );
    assert_eq!(
        entities.get(behind).expect("behind should exist").hp,
        behind_hp_before,
        "overpenetration should stop at the tank"
    );
}

#[test]
fn tank_between_attacker_and_target_blocks_the_shot() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
        .expect("blocking tank should spawn");
    let intended = entities
        .spawn_unit(2, EntityKind::Worker, 190.0, 100.0)
        .expect("intended target should spawn");
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let intended_hp_before = entities.get(intended).expect("intended should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        intended,
        20,
        1,
        100.0,
        100.0,
        190.0,
        100.0,
        128.0,
    );

    assert_eq!(
        entities.get(intended).expect("intended should exist").hp,
        intended_hp_before,
        "target behind the blocking tank should not be damaged"
    );
    assert!(
        entities.get(blocker).expect("blocker should exist").hp < blocker_hp_before,
        "blocking tank should take the shot damage"
    );
    assert!(
            events
                .get(&1)
                .expect("attacker owner events should exist")
                .iter()
                .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == blocker)),
            "attack event should point at the blocking tank"
        );
}

#[test]
fn building_between_attacker_and_target_blocks_the_shot() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let blocker = entities
        .spawn_building(2, EntityKind::Depot, 160.0, 100.0, true)
        .expect("blocking building should spawn");
    let intended = entities
        .spawn_unit(2, EntityKind::Worker, 230.0, 100.0)
        .expect("intended target should spawn");
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let intended_hp_before = entities.get(intended).expect("intended should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());

    apply_test_damage(
        &mut entities,
        &mut events,
        attacker,
        intended,
        20,
        1,
        100.0,
        100.0,
        230.0,
        100.0,
        128.0,
    );

    assert_eq!(
        entities.get(intended).expect("intended should exist").hp,
        intended_hp_before,
        "target behind the blocking building should not be damaged"
    );
    assert!(
        entities.get(blocker).expect("blocker should exist").hp < blocker_hp_before,
        "blocking building should take the shot damage"
    );
}

#[test]
fn friendly_building_between_attacker_and_target_prevents_firing() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let blocker = entities
        .spawn_building(1, EntityKind::Depot, 160.0, 100.0, true)
        .expect("friendly blocker should spawn");
    let intended = entities
        .spawn_unit(2, EntityKind::Worker, 230.0, 100.0)
        .expect("intended target should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(intended));
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let intended_hp_before = entities.get(intended).expect("intended should exist").hp;
    let map = open_map(12);

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let attacker_entity = entities.get(attacker).expect("attacker should exist");
    assert_eq!(attacker_entity.target_id(), Some(intended));
    assert_eq!(
        attacker_entity.attack_cd(),
        0,
        "blocked shots must not reset cooldown"
    );
    assert_eq!(
        entities.get(blocker).expect("blocker should exist").hp,
        blocker_hp_before,
        "friendly buildings must not take blocked-shot damage"
    );
    assert_eq!(
        entities.get(intended).expect("intended should exist").hp,
        intended_hp_before,
        "targets behind friendly buildings should not be damaged"
    );
    assert!(
        events
            .values()
            .flatten()
            .all(|event| !matches!(event, Event::Attack { .. })),
        "blocked friendly-cover shots should not emit attack events"
    );
}

#[test]
fn friendly_tank_between_attacker_and_target_prevents_firing() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let blocker = entities
        .spawn_unit(1, EntityKind::Tank, 140.0, 100.0)
        .expect("friendly blocker should spawn");
    let intended = entities
        .spawn_unit(2, EntityKind::Worker, 190.0, 100.0)
        .expect("intended target should spawn");
    if let Some(attacker_entity) = entities.get_mut(attacker) {
        attacker_entity.set_order(Order::attack(intended));
    }
    if let Some(blocker_entity) = entities.get_mut(blocker) {
        blocker_entity.set_attack_cd(99);
    }
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let intended_hp_before = entities.get(intended).expect("intended should exist").hp;
    let map = open_map(12);

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let attacker_entity = entities.get(attacker).expect("attacker should exist");
    assert_eq!(attacker_entity.target_id(), Some(intended));
    assert_eq!(
        attacker_entity.attack_cd(),
        0,
        "blocked shots must not reset cooldown"
    );
    assert_eq!(
        entities.get(blocker).expect("blocker should exist").hp,
        blocker_hp_before,
        "friendly tanks must not take blocked-shot damage"
    );
    assert_eq!(
        entities.get(intended).expect("intended should exist").hp,
        intended_hp_before,
        "targets behind friendly tanks should not be damaged"
    );
    assert!(
        events
            .values()
            .flatten()
            .all(|event| !matches!(event, Event::Attack { from, .. } if *from == attacker)),
        "the blocked attacker should not emit attack events"
    );
}

#[test]
fn friendly_non_tank_units_do_not_block_firing() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let blocker = entities
        .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
        .expect("friendly worker should spawn");
    let intended = entities
        .spawn_unit(2, EntityKind::Worker, 190.0, 100.0)
        .expect("intended target should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(intended));
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let intended_hp_before = entities.get(intended).expect("intended should exist").hp;
    let map = open_map(12);

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities.get(blocker).expect("blocker should exist").hp,
        blocker_hp_before,
        "friendly soft units must not take damage"
    );
    assert!(
        entities.get(intended).expect("intended should exist").hp < intended_hp_before,
        "friendly soft units should not block the shot"
    );
}

#[test]
fn attack_move_prefers_clear_target_over_target_behind_friendly_tank() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    entities
        .spawn_unit(1, EntityKind::Tank, 135.0, 100.0)
        .expect("friendly blocker should spawn");
    let blocked_target = entities
        .spawn_unit(2, EntityKind::Worker, 170.0, 100.0)
        .expect("blocked target should spawn");
    let clear_target = entities
        .spawn_unit(2, EntityKind::Worker, 100.0, 180.0)
        .expect("clear target should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack_move_to(220.0, 100.0));
    let map = open_map(12);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker_entity = entities.get(attacker).expect("attacker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &spatial,
        &los,
        &fog,
        &smokes,
        attacker,
        attacker_entity.owner,
        attacker_entity.pos_x,
        attacker_entity.pos_y,
        128.0,
        combat_mode(attacker_entity),
    );

    assert_eq!(target, Some(clear_target));
    assert_ne!(target, Some(blocked_target));
}
