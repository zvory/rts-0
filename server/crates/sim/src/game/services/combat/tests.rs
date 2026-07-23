use super::*;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore, MovePhase, Order, WeaponSetup};
use crate::game::fog::Fog;
use crate::game::mortar;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::movement::{angle_delta, movement_system};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::game::upgrade::UpgradeKind;
use crate::game::{PlayerState, ScoreState};
use crate::protocol::{terrain, NoticeSeverity};
use crate::rules::combat as combat_rules;
use rand::{rngs::SmallRng, SeedableRng};
mod accuracy;
mod anti_tank_acquisition;
mod anti_tank_behavior;
mod coax;
mod entrenchment;
mod fog_visibility;
mod mortar_autocast;
mod moving_fire_policy;
mod range_targeting;
mod retention;
mod support_weapon_attack_move;
mod tank_traps;
mod target_legality;
mod target_priority;
mod weapon_cooldowns;
mod weapon_profiles;
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
        base_sites: Vec::new(),
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
fn resolve_test_target(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    attacker_id: u32,
    acquire_px: f32,
) -> Option<u32> {
    let los = LineOfSight::new(map);
    let spatial = SpatialIndex::build(entities, map.size);
    let fog = visible_fog(map, entities);
    let smokes = SmokeCloudStore::new();
    let attacker = entities.get(attacker_id).expect("attacker should exist");
    resolve_target(
        map,
        entities,
        teams,
        &spatial,
        &los,
        &fog,
        &smokes,
        attacker_id,
        attacker.owner,
        attacker.pos_x,
        attacker.pos_y,
        acquire_px,
        combat_mode(attacker),
    )
}
fn resolve_tank_test_target(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    tank_id: u32,
) -> Option<u32> {
    let los = LineOfSight::new(map);
    let spatial = SpatialIndex::build(entities, map.size);
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2, 3], entities, map);
    let smokes = SmokeCloudStore::new();
    let tank = entities.get(tank_id).expect("tank should exist");
    resolve_target(
        map,
        entities,
        teams,
        &spatial,
        &los,
        &fog,
        &smokes,
        tank_id,
        tank.owner,
        tank.pos_x,
        tank.pos_y,
        192.0,
        combat_mode(tank),
    )
}

fn spawn_tank_priority_target(entities: &mut EntityStore, kind: EntityKind, x: f32) -> Option<u32> {
    if kind == EntityKind::TankTrap {
        entities.spawn_building(2, kind, x, 100.0, true)
    } else {
        entities.spawn_unit(2, kind, x, 100.0)
    }
}

fn player_state(id: u32, is_ai: bool) -> PlayerState {
    PlayerState {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: format!("Player {id}"),
        color: "#fff".to_string(),
        start_tile: (4, 4),
        steel: 1_000,
        oil: 1_000,
        supply_used: 0,
        is_ai,
        score: ScoreState::default(),
        upgrades: Default::default(),
        ability_cooldowns: Default::default(),
    }
}

fn default_team_relations() -> TeamRelations {
    TeamRelations::from_player_teams([(1, 1), (2, 2)])
}
fn team_relations(assignments: &[(u32, u32)]) -> TeamRelations {
    TeamRelations::from_player_teams(assignments.iter().copied())
}

fn run_combat_tick(entities: &mut EntityStore) -> HashMap<u32, Vec<Event>> {
    let mut player = player_state(1, false);
    player.upgrades.insert(UpgradeKind::MortarAutocast);
    run_combat_tick_with_players(entities, &[player, player_state(2, false)])
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
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    let mut coordinator =
        MoveCoordinator::new_with_teams(&mut pathing, map, &occ, 10, teams.clone());
    let mut fog = Fog::new(map.size);
    let player_ids: Vec<u32> = players.iter().map(|player| player.id).collect();
    fog.recompute_with_smoke(&player_ids, entities, map, smokes);
    let mut events: HashMap<u32, Vec<Event>> = player_ids
        .iter()
        .map(|player_id| (*player_id, Vec::new()))
        .collect();

    let mut rng = SmallRng::seed_from_u64(rng_seed);
    let mut mortar_shells = crate::game::mortar::MortarShellStore::default();
    let mut panzerfaust_shots = crate::game::panzerfaust_shot::PanzerfaustShotStore::default();
    let mut firing_reveals = Vec::new();
    let mortar_autocast_researched = |owner| {
        players
            .iter()
            .any(|p| p.id == owner && p.upgrades.contains(&UpgradeKind::MortarAutocast))
    };
    let methamphetamines_researched = |owner| {
        players
            .iter()
            .any(|p| p.id == owner && p.has_upgrade(UpgradeKind::Methamphetamines))
    };
    combat_system(
        map,
        entities,
        &teams,
        &mortar_autocast_researched,
        &methamphetamines_researched,
        &occ,
        &spatial,
        &mut coordinator,
        &fog,
        smokes,
        &mut mortar_shells,
        &mut panzerfaust_shots,
        &mut rng,
        &mut events,
        &mut firing_reveals,
        10,
    );
    events
}

fn test_mortar_scattered_impact(
    entities: &EntityStore,
    teams: &TeamRelations,
    player_ids: &[u32],
    owner: u32,
    attacker: u32,
    target: u32,
    tick: u32,
) -> (f32, f32) {
    let map = Map::generate(2, 0x00C0_FFEE);
    let mut fog = Fog::new(map.size);
    fog.recompute(player_ids, entities, &map);
    let target = entities.get(target).expect("target should exist");
    crate::game::mortar_scatter::scattered_mortar_impact(
        &fog,
        teams,
        owner,
        attacker,
        target.pos_x,
        target.pos_y,
        tick,
    )
}
fn run_movement_tick(entities: &mut EntityStore) {
    let map = Map::generate(2, 0x00C0_FFEE);
    run_movement_tick_on_map(entities, &map, 0);
}
fn run_movement_tick_on_map(entities: &mut EntityStore, map: &Map, tick: u32) {
    let occ = Occupancy::build(map, entities);
    let spatial = SpatialIndex::build(entities, map.size);
    movement_system(map, entities, &mut [], &occ, &spatial, tick);
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
    apply_test_damage_with_teams(
        entities,
        &default_team_relations(),
        events,
        attacker,
        victim,
        dmg,
        attacker_owner,
        ax,
        ay,
        vx,
        vy,
        range_px,
    );
}
#[allow(clippy::too_many_arguments)]
fn apply_test_damage_with_teams(
    entities: &mut EntityStore,
    teams: &TeamRelations,
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
    let weapon_profile = entities
        .get(attacker)
        .and_then(|entity| combat_rules::default_weapon_profile(entity.kind))
        .expect("test attacker should have a default weapon profile");
    let map = Map::generate(2, 0x00C0_FFEE);
    let fog = Fog::new(map.size);
    let smokes = SmokeCloudStore::new();
    let mut rng = SmallRng::seed_from_u64(0);
    let blockers = ShotBlockerIndex::build(&map, entities);
    apply_damage(
        &map,
        entities,
        &blockers,
        teams,
        events,
        &fog,
        &smokes,
        &mut rng,
        attacker,
        victim,
        weapon_profile,
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
        &default_team_relations(),
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
fn allied_riflemen_do_not_auto_acquire_each_other() {
    let (entities, self_id, ally_id) = rifleman_with_enemy();
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker = entities.get(self_id).expect("attacker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &team_relations(&[(1, 7), (2, 7)]),
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
    assert_ne!(target, Some(ally_id));
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
        &default_team_relations(),
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
        &default_team_relations(),
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
fn attack_move_ignores_allies_and_acquires_enemies() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let ally = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("ally should spawn");
    let enemy = entities
        .spawn_unit(3, EntityKind::Rifleman, 150.0, 100.0)
        .expect("enemy should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack_move_to(300.0, 100.0));
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker_entity = entities.get(attacker).expect("attacker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &team_relations(&[(1, 7), (2, 7), (3, 3)]),
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

    assert_eq!(target, Some(enemy));
    assert_ne!(target, Some(ally));
}

#[test]
fn ordered_attackers_do_not_retain_allied_targets() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let ally = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("ally should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(ally));
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker_entity = entities.get(attacker).expect("attacker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &team_relations(&[(1, 7), (2, 7)]),
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

    assert_eq!(target, None);
}

#[test]
fn explicit_attack_retains_visible_enemy_target_over_nearer_candidate() {
    let map = open_map(8);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let explicit_target = entities
        .spawn_unit(2, EntityKind::Worker, 150.0, 100.0)
        .expect("explicit target should spawn");
    let closer_target = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("closer target should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(explicit_target));

    let target = resolve_test_target(&map, &entities, &default_team_relations(), attacker, 192.0);

    assert_eq!(target, Some(explicit_target));
    assert_ne!(target, Some(closer_target));
}

#[test]
fn explicit_attack_drops_dead_target_and_runs_normal_acquisition() {
    let map = open_map(8);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let dead_target = entities
        .spawn_unit(2, EntityKind::Worker, 120.0, 100.0)
        .expect("dead target should spawn");
    let fallback_target = entities
        .spawn_unit(2, EntityKind::Rifleman, 150.0, 100.0)
        .expect("fallback target should spawn");
    assert!(
        entities
            .get_mut(dead_target)
            .expect("dead target should exist")
            .apply_damage(u32::MAX, None),
        "test setup should kill the explicit target",
    );
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(dead_target));

    let target = resolve_test_target(&map, &entities, &default_team_relations(), attacker, 192.0);

    assert_eq!(target, Some(fallback_target));
    assert_ne!(target, Some(dead_target));
}

#[test]
fn infantry_explicit_attack_can_target_visible_enemy_tank_trap() {
    let map = open_map(8);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let trap = entities
        .spawn_building(2, EntityKind::TankTrap, 150.0, 100.0, true)
        .expect("tank trap should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(trap));

    let target = resolve_test_target(&map, &entities, &default_team_relations(), attacker, 192.0);

    assert_eq!(target, Some(trap));
}

#[test]
fn acquisition_against_buildings_ignores_allied_buildings() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let allied_building = entities
        .spawn_building(2, EntityKind::Barracks, 120.0, 100.0, true)
        .expect("allied building should spawn");
    let enemy_building = entities
        .spawn_building(3, EntityKind::Barracks, 150.0, 100.0, true)
        .expect("enemy building should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack_move_to(300.0, 100.0));
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker_entity = entities.get(attacker).expect("attacker should exist");

    let target = resolve_target(
        &map,
        &entities,
        &team_relations(&[(1, 7), (2, 7), (3, 3)]),
        &spatial,
        &los,
        &fog,
        &smokes,
        attacker,
        attacker_entity.owner,
        attacker_entity.pos_x,
        attacker_entity.pos_y,
        192.0,
        combat_mode(attacker_entity),
    );

    assert_eq!(target, Some(enemy_building));
    assert_ne!(target, Some(allied_building));
}

#[test]
fn anti_tank_gun_tank_preference_ignores_allied_tanks() {
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let allied_tank = entities
        .spawn_unit(2, EntityKind::Tank, 120.0, 100.0)
        .expect("allied tank should spawn");
    let enemy_tank = entities
        .spawn_unit(3, EntityKind::Tank, 150.0, 100.0)
        .expect("enemy tank should spawn");
    let map = open_map(8);
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let fog = visible_fog(&map, &entities);
    let smokes = SmokeCloudStore::new();
    let attacker = entities
        .get(anti_tank_gun)
        .expect("anti-tank gun should exist");

    let target = resolve_target(
        &map,
        &entities,
        &team_relations(&[(1, 7), (2, 7), (3, 3)]),
        &spatial,
        &los,
        &fog,
        &smokes,
        anti_tank_gun,
        attacker.owner,
        attacker.pos_x,
        attacker.pos_y,
        192.0,
        combat_mode(attacker),
    );

    assert_eq!(target, Some(enemy_tank));
    assert_ne!(target, Some(allied_tank));
}

#[test]
fn anti_tank_gun_prefers_tank_over_nearer_soft_target() {
    let map = open_map(8);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let nearer_rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("nearer rifleman should spawn");
    let tank = entities
        .spawn_unit(2, EntityKind::Tank, 150.0, 100.0)
        .expect("tank should spawn");

    let target = resolve_test_target(
        &map,
        &entities,
        &default_team_relations(),
        anti_tank_gun,
        192.0,
    );

    assert_eq!(target, Some(tank));
    assert_ne!(target, Some(nearer_rifleman));
}

#[test]
fn small_arms_prefers_unit_over_nearer_building() {
    let map = open_map(8);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let building = entities
        .spawn_building(2, EntityKind::Barracks, 120.0, 100.0, true)
        .expect("building should spawn");
    let unit = entities
        .spawn_unit(2, EntityKind::Worker, 150.0, 100.0)
        .expect("unit should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack_move_to(300.0, 100.0));

    let target = resolve_test_target(&map, &entities, &default_team_relations(), attacker, 192.0);

    assert_eq!(target, Some(unit));
    assert_ne!(target, Some(building));
}

#[test]
fn small_arms_falls_back_to_armored_or_hard_targets() {
    for target_kind in [EntityKind::Tank, EntityKind::Barracks] {
        let map = open_map(8);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let target = if target_kind.is_building() {
            entities.spawn_building(2, target_kind, 150.0, 100.0, true)
        } else {
            entities.spawn_unit(2, target_kind, 150.0, 100.0)
        }
        .expect("fallback target should spawn");
        entities
            .get_mut(attacker)
            .expect("attacker should exist")
            .set_order(Order::attack_move_to(300.0, 100.0));

        assert_eq!(
            resolve_test_target(&map, &entities, &default_team_relations(), attacker, 192.0),
            Some(target),
            "Rifleman should keep {target_kind:?} as a legal fallback target"
        );
    }
}

#[test]
fn own_and_allied_tank_traps_are_not_hostile_targets() {
    let cases = [
        ("own", 1, team_relations(&[(1, 1), (2, 2)])),
        ("allied", 2, team_relations(&[(1, 7), (2, 7)])),
    ];

    for (label, trap_owner, teams) in cases {
        let map = open_map(8);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::ScoutCar, 100.0, 100.0)
            .expect("attacker should spawn");
        let trap = entities
            .spawn_building(trap_owner, EntityKind::TankTrap, 150.0, 100.0, true)
            .expect("tank trap should spawn");
        entities
            .get_mut(attacker)
            .expect("attacker should exist")
            .set_order(Order::attack_move_to(300.0, 100.0));

        let target = resolve_test_target(&map, &entities, &teams, attacker, 192.0);

        assert_eq!(target, None, "{label} Tank Trap should not be acquired");
        assert_ne!(target, Some(trap));
    }
}

#[test]
fn tank_target_priority_uses_threat_role_policy_for_targets_in_weapon_range() {
    use EntityKind::*;

    let map = open_map(12);
    let cases: &[(&[EntityKind], EntityKind)] = &[
        (
            &[Rifleman, MortarTeam, TankTrap, Tank, AntiTankGun],
            AntiTankGun,
        ),
        (&[Rifleman, MortarTeam, TankTrap, Tank], Tank),
        (&[Rifleman, MortarTeam, TankTrap], TankTrap),
        (&[Rifleman, MortarTeam], MortarTeam),
        (&[Rifleman], Rifleman),
    ];

    for (targets, expected_kind) in cases.iter().copied() {
        let mut entities = EntityStore::new();
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let mut expected_id = None;
        for (index, kind) in targets.iter().copied().enumerate() {
            let target_id =
                spawn_tank_priority_target(&mut entities, kind, 120.0 + index as f32 * 10.0)
                    .expect("priority target should spawn");
            if kind == expected_kind {
                expected_id = Some(target_id);
            }
        }
        entities
            .get_mut(tank)
            .expect("tank should exist")
            .set_order(Order::attack_move_to(300.0, 100.0));

        assert_eq!(
            resolve_tank_test_target(&map, &entities, &default_team_relations(), tank),
            expected_id,
            "tank should prefer {expected_kind:?}"
        );
    }
}

#[test]
fn tank_priority_targets_must_be_in_weapon_range() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, 150.0, 100.0)
        .expect("rifleman should spawn");
    let anti_tank_gun = entities
        .spawn_unit(2, EntityKind::AntiTankGun, 288.0, 100.0)
        .expect("anti-tank gun should spawn");
    entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_order(Order::attack_move_to(320.0, 100.0));

    let target = resolve_tank_test_target(&map, &entities, &default_team_relations(), tank);
    assert_eq!(target, Some(rifleman));
    assert_ne!(target, Some(anti_tank_gun));
}

#[test]
fn tank_target_priority_overrides_retained_lower_priority_target() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, 130.0, 100.0)
        .expect("worker should spawn");
    let anti_tank_gun = entities
        .spawn_unit(2, EntityKind::AntiTankGun, 160.0, 100.0)
        .expect("anti-tank gun should spawn");
    if let Some(tank_entity) = entities.get_mut(tank) {
        tank_entity.set_order(Order::move_to(300.0, 100.0));
        tank_entity.set_target_id(Some(worker));
    }

    assert_eq!(
        resolve_tank_test_target(&map, &entities, &default_team_relations(), tank),
        Some(anti_tank_gun)
    );
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
        &default_team_relations(),
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
        &default_team_relations(),
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
        .spawn_unit(1, EntityKind::AntiTankGun, attacker_pos.0, attacker_pos.1)
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
        "smoke-blocked anti-tank gun shots should not emit attack tracers"
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
        .spawn_unit(2, EntityKind::Worker, 120.0, 100.0)
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
fn visible_enemy_damage_alerts_victim_owner_only() {
    let mut entities = EntityStore::new();
    let attacker_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let victim_id = entities
        .spawn_unit(2, EntityKind::Worker, 120.0, 100.0)
        .expect("victim should spawn");
    entities
        .spawn_unit(3, EntityKind::Worker, 700.0, 700.0)
        .expect("victim ally should spawn outside individual fight vision");
    entities
        .get_mut(attacker_id)
        .expect("attacker should exist")
        .set_order(Order::attack(victim_id));
    let mut p1 = player_state(1, false);
    let mut p2 = player_state(2, false);
    let mut p3 = player_state(3, false);
    p1.team_id = 1;
    p2.team_id = 7;
    p3.team_id = 7;

    let events = run_combat_tick_with_players(&mut entities, &[p1, p2, p3]);

    assert!(
        events
            .get(&2)
            .expect("victim owner events should exist")
            .iter()
            .any(|event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack")),
        "victim owner should receive the under-attack notice"
    );
    assert!(
        events
            .get(&3)
            .expect("victim ally events should exist")
            .iter()
            .all(
                |event| !matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack")
            ),
        "victim ally should not receive the teammate's under-attack notice"
    );
    assert!(
        events
            .get(&3)
            .expect("victim ally events should exist")
            .iter()
            .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker_id && *to == victim_id)),
        "victim ally should receive the attack event through teammate current vision"
    );
    assert!(
        events
            .get(&1)
            .expect("attacker events should exist")
            .iter()
            .all(
                |event| !matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack")
            ),
        "attacker team should not receive the victim alert"
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
    let mut panzerfaust_shots = crate::game::panzerfaust_shot::PanzerfaustShotStore::default();
    let mut events = HashMap::from([(1, Vec::new())]);
    let mut firing_reveals = Vec::new();

    let mut rng = SmallRng::seed_from_u64(0);
    let mortar_autocast_researched = |_owner| false;
    let methamphetamines_researched = |_owner| false;
    combat_system(
        &map,
        &mut entities,
        &TeamRelations::from_player_teams([(1, 1)]),
        &mortar_autocast_researched,
        &methamphetamines_researched,
        &occ,
        &spatial,
        &mut coordinator,
        &fog,
        &smokes,
        &mut mortar_shells,
        &mut panzerfaust_shots,
        &mut rng,
        &mut events,
        &mut firing_reveals,
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
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
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
fn tank_attack_move_stops_when_it_reaches_an_enemy() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let _enemy_id = entities
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
    assert_eq!(tank.target_id(), None);
    assert!(
        tank.path_is_empty(),
        "attack-moving tank should stop to engage the enemy"
    );
}

#[test]
fn tank_move_order_fires_without_leaving_move_path() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let _enemy_id = entities
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
    assert_eq!(tank.target_id(), None);
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
            &default_team_relations(),
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
            &default_team_relations(),
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
fn rifleman_attack_move_without_meth_holds_position_while_firing() {
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
        "unupgraded riflemen should still stop while firing"
    );
}

#[test]
fn meth_rifleman_move_order_keeps_path_while_firing_without_charge_state() {
    let mut entities = EntityStore::new();
    let rifleman_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 120.0)
        .expect("enemy should spawn");
    if let Some(rifleman) = entities.get_mut(rifleman_id) {
        rifleman.set_order(Order::move_to(300.0, 100.0));
        rifleman.set_path(vec![(300.0, 100.0)]);
        rifleman.set_path_goal(Some((300.0, 100.0)));
    }
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    let mut meth_player = player_state(1, false);
    meth_player.upgrades.insert(UpgradeKind::Methamphetamines);
    run_combat_tick_with_players(&mut entities, &[meth_player, player_state(2, false)]);

    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), Some(enemy_id));
    assert!(
        !rifleman.path_is_empty(),
        "meth riflemen should keep their movement path while firing"
    );
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp.saturating_sub(5),
        "meth riflemen should fire immediately without a vehicle turret alignment gate"
    );
}

#[test]
fn meth_rifleman_attack_move_stops_when_it_reaches_an_enemy() {
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
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    let mut meth_player = player_state(1, false);
    meth_player.upgrades.insert(UpgradeKind::Methamphetamines);
    run_combat_tick_with_players(&mut entities, &[meth_player, player_state(2, false)]);

    let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
    assert_eq!(rifleman.target_id(), Some(enemy_id));
    assert!(
        rifleman.path_is_empty(),
        "attack-moving meth riflemen should stop to engage the enemy"
    );

    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp.saturating_sub(5),
        "meth riflemen should fire with normal accuracy after stopping"
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
        &default_team_relations(),
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
fn ekat_has_no_default_attack() {
    let mut entities = EntityStore::new();
    let ekat_id = entities
        .spawn_unit(1, EntityKind::Ekat, 100.0, 100.0)
        .expect("Ekat should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("enemy rifleman should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

    entities
        .get_mut(ekat_id)
        .expect("Ekat should exist")
        .set_order(Order::attack(enemy_id));
    run_combat_tick(&mut entities);

    let ekat = entities.get(ekat_id).expect("Ekat should exist");
    assert!(
        !ekat.can_attack(),
        "Ekat should not expose a default weapon"
    );
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp,
        "Ekat should not damage enemies through the default combat service"
    );
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
fn hold_position_does_not_chase_enemy_in_sight() {
    let mut entities = EntityStore::new();
    let holder = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 300.0, 100.0)
        .expect("enemy rifleman should spawn");
    entities
        .get_mut(holder)
        .expect("holder should exist")
        .hold_position();

    run_combat_tick(&mut entities);

    let holder = entities.get(holder).expect("holder should exist");
    assert!(matches!(holder.order(), Order::HoldPosition));
    assert_eq!(holder.target_id(), None);
    assert_eq!(holder.path_goal(), None);
    assert!(holder.path_is_empty());
}

#[test]
fn hold_position_fires_at_enemy_in_weapon_range() {
    let mut entities = EntityStore::new();
    let holder = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 130.0, 100.0)
        .expect("enemy rifleman should spawn");
    entities
        .get_mut(holder)
        .expect("holder should exist")
        .hold_position();
    let enemy_hp = entities.get(enemy).expect("enemy should exist").hp;

    run_combat_tick(&mut entities);

    let holder = entities.get(holder).expect("holder should exist");
    assert!(matches!(holder.order(), Order::HoldPosition));
    assert!(holder.path_is_empty());
    assert!(
        entities.get(enemy).expect("enemy should exist").hp < enemy_hp,
        "held rifleman should fire once enemies enter weapon range"
    );
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
fn idle_anti_tank_gun_does_not_auto_setup() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");

    run_combat_tick(&mut entities);

    assert_eq!(
        entities
            .get(at_id)
            .expect("anti-tank gun should exist")
            .weapon_setup(),
        WeaponSetup::Packed
    );
}

#[test]
fn anti_tank_gun_turns_slowly_before_firing() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
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
        at.facing().abs() <= ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
        "anti-tank gun should only slew by its turn-rate cap, got {:.4}",
        at.facing()
    );
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp,
        "anti-tank gun should not fire until its barrel is aligned"
    );
}

#[test]
fn mortar_turns_fast_before_auto_firing() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 300.0, 300.0)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 428.6, 146.8)
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }
    run_combat_tick(&mut entities);
    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert!(angle_delta(mortar.facing(), -mortar::TURN_RATE_RAD_PER_TICK).abs() <= 0.001);
    assert_eq!(mortar.attack_cd(), 0);
    for _ in 0..2 {
        run_combat_tick(&mut entities);
    }
    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert!(
        angle_delta(mortar.facing(), -50_f32.to_radians()).abs()
            <= mortar::FIRE_TOLERANCE_RAD + 0.001
    );
    assert!(mortar.attack_cd() > 0);
}

#[test]
fn movement_delta_clears_when_target_stops_to_fire() {
    let mut entities = EntityStore::new();
    let target_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("target should spawn");
    let enemy_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 235.0, 100.0)
        .expect("enemy should spawn");
    if let Some(target) = entities.get_mut(target_id) {
        target.set_order(Order::attack_move_to(500.0, 100.0));
        target.set_path(vec![(500.0, 100.0)]);
        target.mark_move_phase(MovePhase::Moving);
        target.set_movement_delta(1.6, 0.0);
    }
    if let Some(enemy) = entities.get_mut(enemy_id) {
        enemy.set_attack_cd(10);
    }

    run_combat_tick_with_players(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
    );

    let target = entities.get(target_id).expect("target should exist");
    assert!(
        target.path_is_empty(),
        "target should clear its path when stopping to fire"
    );
    assert_eq!(
        target.movement_delta(),
        (0.0, 0.0),
        "target should not retain stale movement delta after combat clears its path"
    );
}

#[test]
fn mortar_autocast_skips_shot_that_would_hit_owned_unit() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        test_mortar_scattered_impact(&entities, &teams, &[1, 2], 1, mortar_id, enemy_id, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.attack_cd(),
        0,
        "autocast mortar should hold fire when the scattered impact would hit an owned unit"
    );
}

#[test]
fn mortar_autocast_skips_shot_that_would_hit_allied_unit() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let enemy_id = entities
        .spawn_unit(3, EntityKind::Rifleman, 220.0, 100.0)
        .expect("enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 7), (2, 7), (3, 3)]);
    let (impact_x, impact_y) =
        test_mortar_scattered_impact(&entities, &teams, &[1, 2, 3], 1, mortar_id, enemy_id, 10);
    entities
        .spawn_unit(2, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("allied unit should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }
    let mut p1 = player_state(1, false);
    let mut p2 = player_state(2, false);
    let mut p3 = player_state(3, false);
    p1.team_id = 7;
    p2.team_id = 7;
    p3.team_id = 3;

    run_combat_tick_with_players(&mut entities, &[p1, p2, p3]);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.attack_cd(),
        0,
        "autocast mortar should hold fire when the scattered impact would hit an allied unit"
    );
}

#[test]
fn mortar_autocast_skips_shot_that_would_hit_owned_building() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        test_mortar_scattered_impact(&entities, &teams, &[1, 2], 1, mortar_id, enemy_id, 10);
    entities
        .spawn_building(1, EntityKind::Depot, impact_x, impact_y + 40.0, true)
        .expect("depot should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.attack_cd(),
        0,
        "autocast mortar should hold fire when the scattered impact would hit an owned building"
    );
}

#[test]
fn mortar_autocast_fires_when_scattered_impact_is_clear_of_owned_entities() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 300.0, 100.0)
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert!(
        mortar.attack_cd() > 0,
        "autocast mortar should fire when no owned entity is inside the scattered impact"
    );
}

#[test]
fn mortar_autocast_fires_over_blocking_terrain_with_spotter_vision() {
    let map = map_with_rock_at((4, 3));
    let mortar_pos = map.tile_center(2, 3);
    let target_pos = map.tile_center(8, 3);
    let spotter_pos = map.tile_center(8, 6);
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("enemy should spawn");
    entities
        .spawn_unit(1, EntityKind::Rifleman, spotter_pos.0, spotter_pos.1)
        .expect("spotter should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    let mut player = player_state(1, false);
    player.upgrades.insert(UpgradeKind::MortarAutocast);
    run_combat_tick_on_map(&mut entities, &[player, player_state(2, false)], &map);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert!(
        mortar.attack_cd() > 0,
        "mortar autocast should fire indirectly at owner-visible targets behind LOS blockers"
    );
}

#[test]
fn mortar_autocast_does_not_fire_at_hidden_target_behind_blocking_terrain() {
    let map = map_with_rock_at((4, 3));
    let mortar_pos = map.tile_center(2, 3);
    let target_pos = map.tile_center(8, 3);
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.attack_cd(),
        0,
        "mortar autocast should still require the target to be visible to the owner"
    );
}

#[test]
fn mortar_autocast_disabled_holds_fire_without_blocking_manual_state() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.autocast_enabled(AbilityKind::MortarFire),
        Some(false),
        "mortar autocast should default to disabled"
    );
    assert_eq!(
        mortar.attack_cd(),
        0,
        "disabled autocast mortar should hold fire against visible in-range targets"
    );
}

#[test]
fn mortar_autocast_requires_research_even_if_entity_flag_is_enabled() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick_with_players(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
    );

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.attack_cd(),
        0,
        "mortar autocast should not fire before Mortar Autocast research completes"
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
            .spawn_unit(1, EntityKind::AntiTankGun, attacker_pos.0, attacker_pos.1)
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
    assert_eq!(rear_hp, 210);
    assert!(
        front_hp > rear_hp,
        "rear anti-tank hits should deal more damage than front hits"
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
    let secondary_id = secondary;
    let secondary = entities.get(secondary_id).expect("secondary should exist");
    assert_eq!(secondary.hp, 35);
    assert!(
        matches!(secondary.order(), Order::Idle),
        "overpenetration damage must not mutate worker orders"
    );
    let attacker_events = events.get(&1).expect("attacker owner events should exist");
    assert!(
        attacker_events
            .iter()
            .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == primary)),
        "primary shot should still emit attack feedback"
    );
    assert!(
        attacker_events
            .iter()
            .any(|event| matches!(event, Event::Overpenetration { to } if *to == secondary_id)),
        "secondary hit should emit overpenetration feedback"
    );
    assert!(
        attacker_events
            .iter()
            .all(|event| !matches!(event, Event::Attack { to, .. } if *to == secondary_id)),
        "secondary overpenetration hit must not emit an attack event"
    );
}

#[test]
fn overpenetration_does_not_damage_allied_entity_behind_enemy() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let primary = entities
        .spawn_unit(3, EntityKind::Rifleman, 140.0, 100.0)
        .expect("enemy primary target should spawn");
    let ally_behind = entities
        .spawn_unit(2, EntityKind::Worker, 165.0, 100.0)
        .expect("allied unit should spawn");
    let ally_hp_before = entities.get(ally_behind).expect("ally should exist").hp;
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());
    events.insert(3, Vec::new());
    let teams = team_relations(&[(1, 7), (2, 7), (3, 3)]);

    apply_test_damage_with_teams(
        &mut entities,
        &teams,
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
        entities.get(primary).expect("primary should exist").hp < 40,
        "enemy primary should take the direct hit"
    );
    assert_eq!(
        entities.get(ally_behind).expect("ally should exist").hp,
        ally_hp_before,
        "overpenetration must not damage allied entities behind an enemy"
    );
}

#[test]
fn allied_damage_does_not_update_last_damage_signal() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let ally = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("ally should spawn");
    let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
    events.insert(1, Vec::new());
    events.insert(2, Vec::new());
    let teams = team_relations(&[(1, 7), (2, 7)]);

    apply_test_damage_with_teams(
        &mut entities,
        &teams,
        &mut events,
        attacker,
        ally,
        10,
        1,
        100.0,
        100.0,
        120.0,
        100.0,
        128.0,
    );

    let ally = entities.get(ally).expect("ally should exist");
    assert!(
        ally.hp < ally.max_hp,
        "test applies raw damage to prove attribution is independent of health loss"
    );
    assert_eq!(ally.last_damage_owner(), None);
    assert_eq!(ally.last_damage_pos(), None);
    assert_eq!(ally.last_damage_tick(), None);
}

#[test]
fn missed_primary_shot_still_emits_attack_event() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
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
        "seeded anti-tank shot should miss the infantry target"
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
fn anti_tank_gun_seeded_shot_hits_scout_car_without_miss_roll() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
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
        "seeded anti-tank shot should not miss the scout car target"
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
        Some(config::STEEL_PATCH_AMOUNT)
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
fn tank_trap_between_attacker_and_target_does_not_block_the_shot() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack_move_to(300.0, 100.0));
    let blocker = entities
        .spawn_building(2, EntityKind::TankTrap, 160.0, 100.0, true)
        .expect("tank trap should spawn");
    let intended = entities
        .spawn_unit(2, EntityKind::Worker, 230.0, 100.0)
        .expect("intended target should spawn");
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let intended_hp_before = entities.get(intended).expect("intended should exist").hp;
    let map = open_map(12);

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities
            .get(attacker)
            .expect("attacker should exist")
            .target_id(),
        Some(intended),
        "attack-move should prefer the enemy worker over the closer tank trap"
    );
    assert_eq!(
        entities.get(blocker).expect("blocker should exist").hp,
        blocker_hp_before,
        "tank traps should not take damage while a unit behind them is targeted"
    );
    assert!(
        entities.get(intended).expect("intended should exist").hp < intended_hp_before,
        "target behind the tank trap should take the shot damage"
    );
    assert!(
        events
            .get(&1)
            .expect("attacker owner events should exist")
            .iter()
            .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == intended)),
        "attack event should point at the intended target"
    );
}

#[test]
fn pump_jack_between_attacker_and_target_does_not_block_the_shot() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let blocker = entities
        .spawn_building(2, EntityKind::PumpJack, 160.0, 100.0, true)
        .expect("pump jack should spawn");
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
        entities.get(blocker).expect("blocker should exist").hp,
        blocker_hp_before,
        "Pump Jacks should not take damage while a unit behind them is targeted"
    );
    assert!(
        entities.get(intended).expect("intended should exist").hp < intended_hp_before,
        "target behind the Pump Jack should take the shot damage"
    );
    assert!(
        events
            .get(&1)
            .expect("attacker owner events should exist")
            .iter()
            .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == intended)),
        "attack event should point at the intended target"
    );
}

#[test]
fn infantry_like_auto_acquisition_ignores_enemy_tank_traps() {
    for kind in [
        EntityKind::Worker,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
    ] {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        entities
            .get_mut(attacker)
            .expect("attacker should exist")
            .set_order(Order::attack_move_to(300.0, 100.0));
        let trap = entities
            .spawn_building(2, EntityKind::TankTrap, 150.0, 100.0, true)
            .expect("tank trap should spawn");
        let trap_hp_before = entities.get(trap).expect("trap should exist").hp;

        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &open_map(12),
        );

        assert_eq!(
            entities
                .get(attacker)
                .expect("attacker should exist")
                .target_id(),
            None,
            "{kind:?} should not auto-acquire enemy Tank Traps"
        );
        assert_eq!(
            entities.get(trap).expect("trap should exist").hp,
            trap_hp_before,
            "{kind:?} should not damage enemy Tank Traps without a direct order"
        );
    }
}

#[test]
fn vehicle_body_auto_acquisition_keeps_enemy_tank_traps_targetable() {
    for kind in [EntityKind::ScoutCar, EntityKind::Tank] {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        entities
            .get_mut(attacker)
            .expect("attacker should exist")
            .set_order(Order::attack_move_to(300.0, 100.0));
        let trap = entities
            .spawn_building(2, EntityKind::TankTrap, 150.0, 100.0, true)
            .expect("tank trap should spawn");

        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &open_map(12),
        );

        assert_eq!(
            entities
                .get(attacker)
                .expect("attacker should exist")
                .target_id(),
            Some(trap),
            "{kind:?} should still auto-acquire enemy Tank Traps"
        );
    }
}

#[test]
fn friendly_building_between_attacker_and_target_leaves_direct_attacker_stationary() {
    let map = open_map(12);
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
    let spotter = map.tile_center(7, 5);
    entities
        .spawn_unit(1, EntityKind::Worker, spotter.0, spotter.1)
        .expect("spotter should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(intended));
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let intended_hp_before = entities.get(intended).expect("intended should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let attacker_entity = entities.get(attacker).expect("attacker should exist");
    assert_eq!(attacker_entity.target_id(), None);
    assert!(attacker_entity.path_is_empty());
    assert_eq!(attacker_entity.path_goal(), None);
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
fn enemy_building_between_attacker_and_target_leaves_direct_attacker_stationary() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let blocker = entities
        .spawn_building(2, EntityKind::Depot, 160.0, 100.0, true)
        .expect("enemy blocker should spawn");
    let intended = entities
        .spawn_unit(2, EntityKind::Worker, 230.0, 100.0)
        .expect("intended target should spawn");
    let spotter = map.tile_center(7, 5);
    entities
        .spawn_unit(1, EntityKind::Worker, spotter.0, spotter.1)
        .expect("spotter should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(intended));
    let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
    let intended_hp_before = entities.get(intended).expect("intended should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let attacker_entity = entities.get(attacker).expect("attacker should exist");
    assert_eq!(attacker_entity.target_id(), None);
    assert!(attacker_entity.path_is_empty());
    assert_eq!(attacker_entity.path_goal(), None);
    assert_eq!(
        attacker_entity.attack_cd(),
        0,
        "blocked direct attacks must not reset cooldown"
    );
    assert_eq!(
        entities.get(blocker).expect("blocker should exist").hp,
        blocker_hp_before,
        "direct attacks should not damage an intervening enemy building"
    );
    assert_eq!(
        entities.get(intended).expect("intended should exist").hp,
        intended_hp_before,
        "targets behind enemy buildings should not be damaged until hittable"
    );
    assert!(
        events
            .values()
            .flatten()
            .all(|event| !matches!(event, Event::Attack { from, .. } if *from == attacker)),
        "blocked direct attacks should not emit attack events"
    );
}

#[test]
fn direct_attack_on_out_of_range_building_creates_a_pursuit_path() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("attacker should spawn");
    let target = entities
        .spawn_building(2, EntityKind::Depot, 300.0, 100.0, true)
        .expect("target building should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(target));
    let map = open_map(16);

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let attacker_entity = entities.get(attacker).expect("attacker should exist");
    assert_eq!(attacker_entity.target_id(), None);
    assert!(attacker_entity.path_goal().is_some());
    assert!(!attacker_entity.path_is_empty());
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
        blocker_entity.set_weapon_cooldown(combat_rules::WeaponKind::TankCoax, 99);
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
    assert_eq!(attacker_entity.target_id(), None);
    assert!(attacker_entity.path_is_empty());
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
        &default_team_relations(),
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
