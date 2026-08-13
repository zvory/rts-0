use super::*;
use crate::game::entity::MovePhase;

#[test]
fn every_road_variant_applies_the_authoritative_movement_speed_multiplier() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0xC0FF_EE01);
    game.state.map.terrain.fill(terrain::GRASS);
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    let variants = [
        terrain::ROAD_BARE,
        terrain::ROAD_HORIZONTAL,
        terrain::ROAD_VERTICAL,
        terrain::ROAD_DIAGONAL_NW_SE,
        terrain::ROAD_DIAGONAL_NE_SW,
    ];

    let grass_start = game.state.map.tile_center(20, 20);
    let grass = spawn_moving_rifleman(&mut game, grass_start);
    let roads: Vec<_> = variants
        .into_iter()
        .enumerate()
        .map(|(index, code)| {
            let tile = (20, 30 + index as u32 * 2);
            let terrain_index = game.state.map.index(tile.0, tile.1);
            game.state.map.terrain[terrain_index] = code;
            let start = game.state.map.tile_center(tile.0, tile.1);
            (spawn_moving_rifleman(&mut game, start), start, code)
        })
        .collect();

    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let player_ids = game.state.player_ids();
    game.state
        .fog
        .recompute(&player_ids, &game.state.entities, &game.state.map);
    game.assert_invariants();
    game.tick();

    let base_speed = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .speed;
    assert_moved_distance(&game, grass, grass_start, base_speed, "grass");
    for (id, start, code) in roads {
        assert_moved_distance(
            &game,
            id,
            start,
            base_speed * crate::rules::terrain::ROAD_MOVEMENT_SPEED_MULTIPLIER,
            &format!("road terrain {code}"),
        );
    }
}

#[test]
fn authored_slow_tiles_reduce_movement_by_a_quarter_and_stack_with_roads() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x5100_7001);
    game.state.map.terrain.fill(terrain::GRASS);
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    let slow_tile = (20, 20);
    let slow_road_tile = (20, 24);
    let road_index = game.state.map.index(slow_road_tile.0, slow_road_tile.1);
    game.state.map.terrain[road_index] = terrain::ROAD_HORIZONTAL;
    game.state.map.slow_movement_tiles = vec![slow_tile, slow_road_tile];
    let slow_start = game.state.map.tile_center(slow_tile.0, slow_tile.1);
    let slow_road_start = game
        .state
        .map
        .tile_center(slow_road_tile.0, slow_road_tile.1);
    let slow = spawn_moving_rifleman(&mut game, slow_start);
    let slow_road = spawn_moving_rifleman(&mut game, slow_road_start);

    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.tick();

    let base_speed = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .speed;
    assert_moved_distance(&game, slow, slow_start, base_speed * 0.75, "slow tile");
    assert_moved_distance(
        &game,
        slow_road,
        slow_road_start,
        base_speed * crate::rules::terrain::ROAD_MOVEMENT_SPEED_MULTIPLIER * 0.75,
        "slow road tile",
    );
}

#[test]
fn local_elevation_grade_slows_uphill_and_boosts_downhill_movement() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0xE1E0_0001);
    game.state.map.terrain.fill(terrain::GRASS);
    game.state.map.elevation.fill(0);
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let level_start = game.state.map.tile_center(20, 16);
    let uphill_start = game.state.map.tile_center(20, 20);
    let downhill_start = game.state.map.tile_center(20, 24);
    for tile_y in [20, 24] {
        let current_index = game.state.map.index(20, tile_y);
        let ahead_index = game.state.map.index(21, tile_y);
        game.state.map.elevation[current_index] = if tile_y == 24 { 1 } else { 0 };
        game.state.map.elevation[ahead_index] = if tile_y == 20 { 1 } else { 0 };
    }
    let level = spawn_moving_rifleman(&mut game, level_start);
    let uphill = spawn_moving_rifleman(&mut game, uphill_start);
    let downhill = spawn_moving_rifleman(&mut game, downhill_start);

    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.tick();

    let base_speed = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .speed;
    assert_moved_distance(&game, level, level_start, base_speed, "level ground");
    assert_moved_distance(
        &game,
        uphill,
        uphill_start,
        base_speed * 0.88,
        "one grade uphill",
    );
    assert_moved_distance(
        &game,
        downhill,
        downhill_start,
        base_speed * 1.06,
        "one grade downhill",
    );
}

fn spawn_moving_rifleman(game: &mut Game, start: (f32, f32)) -> u32 {
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman should spawn");
    let goal = (start.0 + 64.0, start.1);
    let entity = game.state.entities.get_mut(id).expect("spawned rifleman");
    entity.set_order(Order::move_to(goal.0, goal.1));
    entity.set_path(vec![goal]);
    entity.set_path_goal(Some(goal));
    entity.mark_move_phase(MovePhase::Moving);
    id
}

fn assert_moved_distance(game: &Game, id: u32, start: (f32, f32), expected: f32, label: &str) {
    let entity = game.state.entities.get(id).expect("spawned rifleman");
    let moved = ((entity.pos_x - start.0).powi(2) + (entity.pos_y - start.1).powi(2)).sqrt();
    assert!(
        (moved - expected).abs() <= 0.001,
        "{label} moved {moved}px; expected {expected}px"
    );
}
