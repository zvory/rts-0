use super::*;

#[test]
fn uses_rotation_clear_exit_and_starts_moving() {
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

        for _ in 0..15 {
            game.tick();
        }

        let moved = game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 1 && entity.kind == unit && entity.hp > 0)
            .expect("produced vehicle should remain alive");
        let distance = ((moved.pos_x - start.0).powi(2) + (moved.pos_y - start.1).powi(2)).sqrt();
        assert!(
            distance > 1.0,
            "{unit} should immediately make progress toward the rally, moved {distance:.3}px"
        );
    }
}
