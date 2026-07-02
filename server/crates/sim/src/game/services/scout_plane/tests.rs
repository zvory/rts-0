use super::*;
use crate::game::ability::AbilityKind;
use crate::game::entity::{EntityKind, Order, ProdItem, RallyIntent, RallyKind, ScoutPlaneState};
use crate::game::map::{Map, MapMetadata, CURRENT_MAP_VERSION};
use crate::game::teams::TeamRelations;
use crate::game::{Game, PlayerInit, SimCommand};
use crate::protocol::terrain;
use std::collections::HashMap;

const EPS: f32 = 0.01;

fn test_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4), (size.saturating_sub(5), size.saturating_sub(5))],
        expansion_sites: Vec::new(),
    }
}

fn test_metadata() -> MapMetadata {
    MapMetadata {
        name: "Scout Plane Test".to_string(),
        schema_version: CURRENT_MAP_VERSION,
        content_hash: "test".to_string(),
    }
}

fn players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".to_string(),
            color: "#000".to_string(),
            is_ai: false,
        },
    ]
}

fn players_with_ai_opponent() -> [PlayerInit; 2] {
    let mut players = players();
    players[1].is_ai = true;
    players
}

fn empty_game(map: Map) -> Game {
    let players = players();
    let mut game = Game::new_direct_start_for_test(
        &players,
        Some((1_000, 1_000)),
        0x5150_7003,
        None,
        Some(map),
        test_metadata(),
    );
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    crate::game::systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.clear_and_rebuild_derived_state_for_test();
    game
}

fn spawn_city_centre(game: &mut Game, owner: u32, x: f32, y: f32) -> u32 {
    game.state
        .entities
        .spawn_building(owner, EntityKind::CityCentre, x, y, true)
        .expect("city centre should spawn")
}

fn spawn_plane(game: &mut Game, owner: u32, x: f32, y: f32) -> u32 {
    game.state
        .entities
        .spawn_unit(owner, EntityKind::ScoutPlane, x, y)
        .expect("scout plane should spawn")
}

fn plane_state(game: &Game, id: u32) -> ScoutPlaneState {
    *game
        .state
        .entities
        .get(id)
        .expect("plane should exist")
        .scout_plane_state()
        .expect("plane state should exist")
}

fn plane_fuel(game: &Game, id: u32) -> u8 {
    plane_state(game, id).fuel_oil
}

fn set_player_oil(game: &mut Game, player_id: u32, oil: u32) {
    let player = game
        .state
        .players
        .iter_mut()
        .find(|player| player.id == player_id)
        .expect("player should exist");
    player.set_resources(player.steel, oil);
}

fn player_oil(game: &Game, player_id: u32) -> u32 {
    game.state
        .players
        .iter()
        .find(|player| player.id == player_id)
        .expect("player should exist")
        .oil
}

fn tick_n(game: &mut Game, ticks: u32) {
    for _ in 0..ticks {
        game.tick();
    }
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

#[test]
fn orbit_transition_spends_one_movement_budget() {
    let speed = config::SCOUT_PLANE_SPEED_PX_PER_TICK;
    let orbit_radius = config::SCOUT_PLANE_ORBIT_RADIUS_TILES as f32 * config::TILE_SIZE as f32;
    let center = (512.0, 512.0);
    let snapshot = ScoutPlaneSnapshot {
        x: center.0 - orbit_radius - speed,
        y: center.1,
        center,
        phase: 0.0,
        orbiting: false,
    };

    let step = advance_one(snapshot, speed, orbit_radius, 2_048.0);

    assert!(step.orbiting);
    assert!(
        distance((snapshot.x, snapshot.y), (step.x, step.y)) <= speed + EPS,
        "reaching the orbit ring must not also spend a full orbit step"
    );
}

#[test]
fn inside_radius_retarget_uses_nearest_orbit_phase() {
    let speed = config::SCOUT_PLANE_SPEED_PX_PER_TICK;
    let orbit_radius = config::SCOUT_PLANE_ORBIT_RADIUS_TILES as f32 * config::TILE_SIZE as f32;
    let center = (512.0, 512.0);
    let snapshot = ScoutPlaneSnapshot {
        x: center.0 - orbit_radius * 0.5,
        y: center.1,
        center,
        phase: 0.0,
        orbiting: false,
    };

    let step = advance_one(snapshot, speed, orbit_radius, 2_048.0);

    assert!(step.orbiting);
    assert!(
        step.x < snapshot.x,
        "a plane already inside the orbit radius should fly to the nearest ring point"
    );
    assert!((step.phase - std::f32::consts::PI).abs() <= EPS);
}

#[test]
fn scout_plane_stamps_aerial_fog_and_projects_to_owner() {
    let mut game = empty_game(test_map(40));
    spawn_city_centre(&mut game, 1, 64.0, 64.0);
    let plane = spawn_plane(&mut game, 1, 640.0, 640.0);
    let hidden_enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, 704.0, 640.0)
        .expect("enemy should spawn");

    game.tick();

    let snapshot = game.snapshot_for(1);
    let owner_plane = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == plane)
        .expect("owner should see the active Scout Plane");
    assert!(
        owner_plane.scout_plane.is_some(),
        "owner projection should include private fuel/orbit state"
    );
    assert!(
        snapshot
            .entities
            .iter()
            .any(|entity| entity.id == hidden_enemy),
        "Scout Plane sight should reveal nearby enemies"
    );
    let (tx, ty) = game.state.map.tile_of(704.0, 640.0);
    let index = (ty * game.state.map.size + tx) as usize;
    assert_eq!(
        snapshot.visible_tiles.get(index).copied(),
        Some(1),
        "the enemy tile should be visible through aerial Scout Plane sight"
    );
    let full = game.snapshot_full_for(1);
    let full_plane = full
        .entities
        .iter()
        .find(|entity| entity.id == plane)
        .expect("full-world diagnostics should still include hidden plane state");
    assert!(full_plane.scout_plane.is_some());
}

#[test]
fn hidden_scout_plane_does_not_keep_ai_player_alive() {
    let players = players_with_ai_opponent();
    let mut game = Game::new_direct_start_for_test(
        &players,
        Some((1_000, 1_000)),
        0x5150_7004,
        None,
        Some(test_map(40)),
        test_metadata(),
    );
    let ai_units: Vec<u32> = game
        .state
        .entities
        .iter()
        .filter(|entity| entity.owner == 2 && entity.is_unit())
        .map(|entity| entity.id)
        .collect();
    for id in ai_units {
        game.state.entities.remove(id);
    }
    spawn_plane(&mut game, 2, 640.0, 640.0);

    assert!(
        !game.alive_players().contains(&2),
        "hidden non-targetable planes must not satisfy the AI survival-unit predicate"
    );
}

#[test]
fn scout_plane_sight_ignores_terrain_and_building_blockers() {
    let mut map = test_map(40);
    map.terrain[(4 * map.size + 5) as usize] = terrain::ROCK;
    let mut game = empty_game(map);
    let plane_pos = game.state.map.tile_center(3, 4);
    let rock_hidden_pos = game.state.map.tile_center(7, 4);
    let depot_pos = crate::game::services::occupancy::footprint_center(
        &game.state.map,
        EntityKind::Depot,
        9,
        3,
    );
    let building_hidden_pos = game.state.map.tile_center(13, 4);
    spawn_plane(&mut game, 1, plane_pos.0, plane_pos.1);
    let rock_hidden = game
        .state
        .entities
        .spawn_unit(
            2,
            EntityKind::Rifleman,
            rock_hidden_pos.0,
            rock_hidden_pos.1,
        )
        .expect("enemy behind rock should spawn");
    game.state
        .entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("line-of-sight blocker should spawn");
    let building_hidden = game
        .state
        .entities
        .spawn_unit(
            2,
            EntityKind::Rifleman,
            building_hidden_pos.0,
            building_hidden_pos.1,
        )
        .expect("enemy behind building should spawn");

    game.tick();

    let snapshot = game.snapshot_for(1);
    assert!(
        snapshot
            .entities
            .iter()
            .any(|entity| entity.id == rock_hidden),
        "Scout Plane sight should reveal through terrain blockers"
    );
    assert!(
        snapshot
            .entities
            .iter()
            .any(|entity| entity.id == building_hidden),
        "Scout Plane sight should reveal through building blockers"
    );
}

#[test]
fn scout_plane_sight_is_blocked_by_smoke() {
    let mut game = empty_game(test_map(40));
    let plane_pos = game.state.map.tile_center(3, 4);
    let smoke_pos = game.state.map.tile_center(5, 4);
    let hidden_pos = game.state.map.tile_center(8, 4);
    spawn_plane(&mut game, 1, plane_pos.0, plane_pos.1);
    let hidden_enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, hidden_pos.0, hidden_pos.1)
        .expect("smoke-hidden enemy should spawn");
    game.spawn_smoke_cloud_for_test(smoke_pos.0, smoke_pos.1)
        .expect("smoke should spawn");

    game.tick();

    let snapshot = game.snapshot_for(1);
    assert!(
        snapshot
            .entities
            .iter()
            .all(|entity| entity.id != hidden_enemy),
        "smoke between the Scout Plane and target should block aerial vision"
    );
    let (tx, ty) = game.state.map.tile_of(hidden_pos.0, hidden_pos.1);
    let index = (ty * game.state.map.size + tx) as usize;
    assert_eq!(snapshot.visible_tiles.get(index).copied(), Some(0));
}

#[test]
fn scout_plane_upkeep_spends_oil_every_interval_and_keeps_full_reserve() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 160.0, 160.0);
    set_player_oil(&mut game, 1, 10);

    tick_n(
        &mut game,
        (config::SCOUT_PLANE_UPKEEP_INTERVAL_TICKS - 1) as u32,
    );
    assert_eq!(player_oil(&game, 1), 10);
    assert_eq!(
        plane_fuel(&game, plane),
        config::SCOUT_PLANE_FUEL_RESERVE_OIL
    );

    game.tick();

    assert_eq!(player_oil(&game, 1), 9);
    assert_eq!(
        plane_fuel(&game, plane),
        config::SCOUT_PLANE_FUEL_RESERVE_OIL
    );
}

#[test]
fn scout_plane_due_upkeep_does_not_free_refill_depleted_reserve() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 160.0, 160.0);
    if let Some(state) = game
        .state
        .entities
        .get_mut(plane)
        .and_then(|entity| entity.scout_plane_state_mut())
    {
        state.fuel_oil = config::SCOUT_PLANE_FUEL_RESERVE_OIL - 3;
        state.upkeep_ticks_until_due = 1;
    }
    set_player_oil(&mut game, 1, 1);

    game.tick();

    assert_eq!(player_oil(&game, 1), 0);
    assert_eq!(
        plane_fuel(&game, plane),
        config::SCOUT_PLANE_FUEL_RESERVE_OIL - 3,
        "the due upkeep payment should not also refill previously missing reserve"
    );
}

#[test]
fn scout_plane_zero_oil_drains_reserve_and_auto_dismisses() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 160.0, 160.0);
    set_player_oil(&mut game, 1, 0);

    tick_n(&mut game, config::SCOUT_PLANE_UPKEEP_INTERVAL_TICKS as u32);
    assert_eq!(
        plane_fuel(&game, plane),
        config::SCOUT_PLANE_FUEL_RESERVE_OIL - 1
    );
    assert!(game.state.entities.get(plane).is_some());

    tick_n(
        &mut game,
        config::SCOUT_PLANE_UPKEEP_INTERVAL_TICKS as u32
            * (config::SCOUT_PLANE_FUEL_RESERVE_OIL as u32 - 1),
    );

    assert!(
        game.state.entities.get(plane).is_none(),
        "fuel exhaustion should automatically dismiss the Scout Plane"
    );
}

#[test]
fn scout_plane_oil_income_refills_reserve_before_fuel_exhaustion() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 160.0, 160.0);
    set_player_oil(&mut game, 1, 0);

    tick_n(
        &mut game,
        config::SCOUT_PLANE_UPKEEP_INTERVAL_TICKS as u32 * 3,
    );
    assert_eq!(
        plane_fuel(&game, plane),
        config::SCOUT_PLANE_FUEL_RESERVE_OIL - 3
    );

    set_player_oil(&mut game, 1, 3);
    game.tick();

    assert_eq!(player_oil(&game, 1), 0);
    assert_eq!(
        plane_fuel(&game, plane),
        config::SCOUT_PLANE_FUEL_RESERVE_OIL
    );

    tick_n(
        &mut game,
        config::SCOUT_PLANE_UPKEEP_INTERVAL_TICKS as u32 * 5,
    );
    assert!(
        game.state.entities.get(plane).is_some(),
        "refilled reserve should keep the plane alive past the original exhaustion tick"
    );
}

#[test]
fn manual_dismiss_removes_plane_and_stops_upkeep() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 160.0, 160.0);
    set_player_oil(&mut game, 1, 10);

    game.enqueue(
        1,
        SimCommand::UseAbility {
            ability: AbilityKind::DismissScoutPlane,
            units: vec![plane],
            x: None,
            y: None,
            queued: false,
        },
    );
    game.tick();
    tick_n(&mut game, config::SCOUT_PLANE_UPKEEP_INTERVAL_TICKS as u32);

    assert!(game.state.entities.get(plane).is_none());
    assert_eq!(
        player_oil(&game, 1),
        10,
        "dismissed planes should not continue charging upkeep"
    );
}

#[test]
fn runtime_duplicate_cleanup_keeps_one_active_plane_per_owner() {
    let mut game = empty_game(test_map(40));
    let first = spawn_plane(&mut game, 1, 160.0, 160.0);
    let second = spawn_plane(&mut game, 1, 192.0, 160.0);

    game.tick();

    assert!(game.state.entities.get(first).is_some());
    assert!(game.state.entities.get(second).is_none());
    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::ScoutPlane && entity.owner == 1)
            .count(),
        1
    );
}

#[test]
fn scout_plane_projection_is_fog_safe_for_owner_enemy_and_spectator() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 160.0, 160.0);

    game.tick();

    let owner = game.snapshot_for(1);
    assert!(
        owner
            .entities
            .iter()
            .any(|entity| entity.id == plane && entity.scout_plane.is_some()),
        "owner should see the plane with private fuel/orbit details"
    );
    let blind_enemy = game.snapshot_for(2);
    assert!(
        blind_enemy.entities.iter().all(|entity| entity.id != plane),
        "enemies without current vision should not receive the plane"
    );

    game.state
        .entities
        .spawn_unit(2, EntityKind::Worker, 176.0, 160.0)
        .expect("enemy spotter should spawn");
    game.tick();

    let enemy = game.snapshot_for(2);
    let enemy_plane = enemy
        .entities
        .iter()
        .find(|entity| entity.id == plane)
        .expect("enemy should see the plane only while it is currently visible");
    assert_eq!(
        enemy_plane.scout_plane, None,
        "enemy plane projection must omit private fuel/orbit state"
    );

    let spectator = game.snapshot_for_spectator(&[1]);
    let spectator_plane = spectator
        .entities
        .iter()
        .find(|entity| entity.id == plane)
        .expect("selected-owner spectator view should include the visible plane");
    assert_eq!(
        spectator_plane.scout_plane, None,
        "selected spectator projections should not expose owner-private plane state"
    );

    let full = game.snapshot_full_for(2);
    assert!(
        full.entities
            .iter()
            .any(|entity| entity.id == plane && entity.scout_plane.is_some()),
        "full-world diagnostics should retain private plane state"
    );
}

#[test]
fn spawned_plane_flies_directly_over_blockers_and_establishes_orbit() {
    let mut map = test_map(32);
    for tx in 4..20 {
        let idx = (8 * map.size + tx) as usize;
        map.terrain[idx] = terrain::WATER;
    }
    let mut game = empty_game(map);
    spawn_city_centre(&mut game, 1, 96.0, 96.0);
    game.state
        .entities
        .spawn_building(2, EntityKind::TankTrap, 192.0, 128.0, true)
        .expect("tank trap should spawn");
    let plane = spawn_plane(&mut game, 1, 64.0, 128.0);
    assert!(retarget(
        &game.state.map,
        &mut game.state.entities,
        plane,
        448.0,
        128.0,
        true,
    ));

    game.tick();
    let after_one = game.state.entities.get(plane).expect("plane");
    assert!((after_one.pos_x - 66.0).abs() <= EPS);
    assert!((after_one.pos_y - 128.0).abs() <= EPS);

    for _ in 0..140 {
        game.tick();
    }

    let plane_entity = game.state.entities.get(plane).expect("plane");
    let state = plane_entity.scout_plane_state().expect("state");
    let orbit_radius = config::SCOUT_PLANE_ORBIT_RADIUS_TILES as f32 * config::TILE_SIZE as f32;
    assert!(state.orbiting, "plane should have reached its orbit area");
    assert!(
        (distance((plane_entity.pos_x, plane_entity.pos_y), state.orbit_center) - orbit_radius)
            .abs()
            <= config::SCOUT_PLANE_SPEED_PX_PER_TICK + EPS,
        "plane should settle on the approved orbit radius"
    );
}

#[test]
fn city_centre_completion_launches_to_first_rally_and_survives_launcher_death() {
    let mut game = empty_game(test_map(40));
    let city = spawn_city_centre(&mut game, 1, 160.0, 160.0);
    let rally = (352.0, 160.0);
    game.state
        .entities
        .get_mut(city)
        .expect("city centre")
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
    game.state
        .entities
        .get_mut(city)
        .expect("city centre")
        .push_production(ProdItem {
            unit: EntityKind::ScoutPlane,
            progress: 0,
            total: 1,
        });

    game.tick();

    let plane = game
        .state
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::ScoutPlane)
        .map(|entity| entity.id)
        .expect("completed production should launch a Scout Plane");
    let plane_entity = game.state.entities.get(plane).expect("plane");
    assert_eq!((plane_entity.pos_x, plane_entity.pos_y), (160.0, 160.0));
    assert_eq!(plane_state(&game, plane).orbit_center, rally);

    game.state.entities.remove(city);
    game.tick();

    assert!(
        game.state.entities.get(plane).is_some(),
        "launched Scout Plane should persist after its City Centre is destroyed"
    );
}

#[test]
fn city_centre_completion_without_rally_orbits_above_launcher() {
    let mut game = empty_game(test_map(40));
    let city = spawn_city_centre(&mut game, 1, 160.0, 160.0);
    game.state
        .entities
        .get_mut(city)
        .expect("city centre")
        .push_production(ProdItem {
            unit: EntityKind::ScoutPlane,
            progress: 0,
            total: 1,
        });

    game.tick();

    let plane = game
        .state
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::ScoutPlane)
        .map(|entity| entity.id)
        .expect("completed production should launch a Scout Plane");
    let state = plane_state(&game, plane);
    assert_eq!(state.orbit_center, (160.0, 160.0));

    game.tick();

    let state = plane_state(&game, plane);
    assert!(
        state.orbiting,
        "a no-rally launch should establish the City Centre orbit"
    );
}

#[test]
fn destroyed_city_centre_before_completion_does_not_launch_plane() {
    let mut game = empty_game(test_map(40));
    let city = spawn_city_centre(&mut game, 1, 160.0, 160.0);
    game.state
        .entities
        .get_mut(city)
        .expect("city centre")
        .push_production(ProdItem {
            unit: EntityKind::ScoutPlane,
            progress: 0,
            total: 30,
        });

    game.state.entities.remove(city);
    game.tick();

    assert!(
        game.state
            .entities
            .iter()
            .all(|entity| entity.kind != EntityKind::ScoutPlane),
        "destroyed production building should follow existing interruption behavior"
    );
}

#[test]
fn move_commands_retarget_and_queue_orbit_centers() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 96.0, 96.0);
    let rifleman = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");

    game.enqueue(
        1,
        SimCommand::Move {
            units: vec![plane, rifleman],
            x: 320.0,
            y: 320.0,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(plane_state(&game, plane).orbit_center, (320.0, 320.0));
    assert!(
        matches!(
            game.state.entities.get(plane).expect("plane").order(),
            Order::Idle
        ),
        "plane retarget should not enter the ground pathing order stack"
    );
    assert!(
        matches!(
            game.state.entities.get(rifleman).expect("rifleman").order(),
            Order::Move(_)
        ),
        "ground units in a mixed selection should still receive normal move orders"
    );

    game.enqueue(
        1,
        SimCommand::Move {
            units: vec![plane],
            x: 384.0,
            y: 320.0,
            queued: true,
        },
    );
    game.tick();

    let plane_entity = game.state.entities.get(plane).expect("plane");
    assert_eq!(plane_entity.queued_orders().len(), 1);
    assert_eq!(plane_state(&game, plane).orbit_center, (320.0, 320.0));

    for _ in 0..130 {
        game.tick();
    }

    assert_eq!(
        game.state
            .entities
            .get(plane)
            .unwrap()
            .queued_orders()
            .len(),
        0
    );
    assert_eq!(plane_state(&game, plane).orbit_center, (384.0, 320.0));
}

#[test]
fn combat_and_hold_commands_do_not_apply_to_plane() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 96.0, 96.0);
    let rifleman = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 110.0, 96.0)
        .expect("rifleman should spawn");
    let original_center = plane_state(&game, plane).orbit_center;

    game.enqueue(
        1,
        SimCommand::AttackMove {
            units: vec![plane],
            x: 320.0,
            y: 320.0,
            queued: false,
        },
    );
    game.enqueue(1, SimCommand::HoldPosition { units: vec![plane] });
    game.enqueue(1, SimCommand::Stop { units: vec![plane] });
    game.tick();

    let plane_entity = game.state.entities.get(plane).expect("plane");
    assert_eq!(
        plane_entity.scout_plane_state().unwrap().orbit_center,
        original_center
    );
    assert!(
        matches!(plane_entity.order(), Order::Idle),
        "non-plane hold-position semantics should be filtered"
    );

    game.enqueue(
        1,
        SimCommand::Attack {
            units: vec![rifleman],
            target: plane,
            queued: false,
        },
    );
    game.tick();

    assert!(
        !matches!(
            game.state.entities.get(rifleman).expect("rifleman").order(),
            Order::Attack(_)
        ),
        "Scout Planes should not be legal explicit attack targets"
    );
}

#[test]
fn artillery_area_damage_ignores_hidden_scout_plane() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 160.0, 160.0);
    let rifleman = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 160.0, 160.0)
        .expect("rifleman should spawn");
    let plane_hp = game.state.entities.get(plane).expect("plane").hp;
    let rifleman_hp = game.state.entities.get(rifleman).expect("rifleman").hp;
    let teams = TeamRelations::from_player_teams(
        game.state
            .players
            .iter()
            .map(|player| (player.id, player.team_id)),
    );
    let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

    game.state.artillery_shells.schedule(2, 0, 160.0, 160.0, 0);
    game.state.artillery_shells.resolve_due(
        &mut game.state.entities,
        &teams,
        &game.state.fog,
        &mut events,
        config::ARTILLERY_SHELL_DELAY_TICKS,
    );

    assert_eq!(game.state.entities.get(plane).expect("plane").hp, plane_hp);
    assert!(
        game.state
            .entities
            .get(rifleman)
            .is_none_or(|entity| entity.hp < rifleman_hp),
        "the same shell should still damage ordinary targetable units"
    );
}

#[test]
fn checkpoint_round_trips_scout_plane_runtime_state() {
    let mut game = empty_game(test_map(40));
    let plane = spawn_plane(&mut game, 1, 96.0, 96.0);
    assert!(retarget(
        &game.state.map,
        &mut game.state.entities,
        plane,
        320.0,
        224.0,
        true,
    ));
    if let Some(state) = game
        .state
        .entities
        .get_mut(plane)
        .and_then(|entity| entity.scout_plane_state_mut())
    {
        state.orbit_phase = 1.25;
        state.orbiting = true;
        state.fuel_oil = 6;
    }

    let payload = game
        .checkpoint_payload_text_for_test()
        .expect("checkpoint should export");
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .expect("checkpoint should restore");

    assert_eq!(plane_state(&restored, plane), plane_state(&game, plane));
}

#[test]
fn ground_collision_ignores_scout_plane_body() {
    let mut game = empty_game(test_map(24));
    let plane = spawn_plane(&mut game, 1, 160.0, 160.0);
    let rifleman = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 160.0, 160.0)
        .expect("rifleman should spawn");

    game.tick();

    assert!(game.state.entities.get(plane).is_some());
    let rifleman_entity = game.state.entities.get(rifleman).expect("rifleman");
    assert!(
        (rifleman_entity.pos_x - 160.0).abs() <= EPS
            && (rifleman_entity.pos_y - 160.0).abs() <= EPS,
        "ground unit should not be pushed away by a Scout Plane"
    );
}
