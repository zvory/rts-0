use super::*;
use crate::game::entity::{EntityKind, Order, ProdItem, RallyIntent, RallyKind, ScoutPlaneState};
use crate::game::map::{Map, MapMetadata, CURRENT_MAP_VERSION};
use crate::game::{Game, PlayerInit, SimCommand};
use crate::protocol::terrain;

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

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
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
