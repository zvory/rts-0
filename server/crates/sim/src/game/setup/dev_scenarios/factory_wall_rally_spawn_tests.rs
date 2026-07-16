use super::test_support::speed_scaled_escape_deadline_ticks;
use super::*;

#[test]
fn uses_rotation_clear_exit_and_escapes_wall() {
    for unit in [
        EntityKind::ScoutCar,
        EntityKind::Tank,
        EntityKind::CommandCar,
    ] {
        let setup = Game::new_factory_wall_rally_spawn_scenario(unit, 1, 0x5150_0011)
            .expect("scenario setup should succeed");
        let (_, _, _, trapped_spawn, rotation_clear_spawn, _) = factory_wall_rally_spawn_map();
        let rally = setup.goal;
        let mut game = setup.game;

        game.tick();

        let spawned = game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 1 && entity.kind == unit && entity.hp > 0)
            .unwrap_or_else(|| panic!("{unit} should be produced on the first tick"));
        assert_ne!(
            (spawned.pos_x, spawned.pos_y),
            trapped_spawn,
            "{unit} must not use the replay's wall-hemmed spawn point"
        );
        assert!(
            spawned.pos_y >= rotation_clear_spawn.1,
            "{unit} should spawn at least one full row below the wall for a full turn, got ({:.1}, {:.1})",
            spawned.pos_x,
            spawned.pos_y,
        );
        let expected_facing = (rally.1 - spawned.pos_y).atan2(rally.0 - spawned.pos_x);
        let facing_delta = (spawned.facing() - expected_facing + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        assert!(
            facing_delta.abs() <= 0.001,
            "{unit} should spawn facing its rally point"
        );
        let start = (spawned.pos_x, spawned.pos_y);
        let rally_delta = (rally.0 - start.0, rally.1 - start.1);
        let rally_distance = (rally_delta.0.powi(2) + rally_delta.1.powi(2)).sqrt();
        let rally_direction = (
            rally_delta.0 / rally_distance,
            rally_delta.1 / rally_distance,
        );
        let escape_distance_px = config::TILE_SIZE as f32;
        let deadline_ticks = speed_scaled_escape_deadline_ticks(unit, escape_distance_px, 4);
        let mut escaped = false;
        let mut progress_px = 0.0;

        for _ in 0..deadline_ticks {
            game.tick();
            let moved = game
                .state
                .entities
                .iter()
                .find(|entity| entity.owner == 1 && entity.kind == unit && entity.hp > 0)
                .expect("produced vehicle should remain alive");
            let occupancy =
                services::occupancy::Occupancy::build(&game.state.map, &game.state.entities);
            assert!(
                services::standability::unit_static_standable_with_facing(
                    &game.state.map,
                    &occupancy,
                    unit,
                    moved.pos_x,
                    moved.pos_y,
                    moved.facing(),
                ),
                "{unit} should not clip the factory or terrain wall while escaping"
            );
            progress_px = (moved.pos_x - start.0) * rally_direction.0
                + (moved.pos_y - start.1) * rally_direction.1;
            if progress_px >= escape_distance_px {
                escaped = true;
                break;
            }
        }

        assert!(
            escaped,
            "{unit} should move one tile away from the wall toward its rally within {deadline_ticks} speed-scaled ticks, progressed {progress_px:.3}px"
        );
    }
}
