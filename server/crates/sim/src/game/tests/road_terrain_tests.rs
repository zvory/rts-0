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
