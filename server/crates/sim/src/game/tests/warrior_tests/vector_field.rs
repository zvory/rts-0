use super::*;

#[test]
fn vector_field_cast_captures_adept_or_master_size() {
    use crate::game::ability_runtime::AbilityObjectPayload;
    for (master, expected) in [(false, 4.0), (true, 5.0)] {
        let mut game = empty_flat_game(&[cultivator_player()]);
        let caster = game
            .state
            .entities
            .spawn_unit(1, EntityKind::MagicAdept, 400.0, 400.0)
            .unwrap();
        game.state.entities.get_mut(caster).unwrap().dao_master = master;
        game.enqueue(
            1,
            Command::VectorField {
                units: vec![caster],
                x: 480.0,
                y: 400.0,
                direction_x: 0.0,
                direction_y: -10.0,
            },
        );
        game.tick();
        let object = *game.state.ability_runtime.world_objects().next().unwrap();
        assert_eq!(
            object.payload,
            AbilityObjectPayload::VectorField {
                direction: [0.0, -1.0],
                size_tiles: expected
            }
        );
        game.state.entities.get_mut(caster).unwrap().dao_master = !master;
        assert_eq!(
            game.state
                .ability_runtime
                .world_objects()
                .next()
                .unwrap()
                .payload,
            object.payload
        );
    }
}

#[test]
fn vector_field_preserves_movement_and_enforces_rank_cooldown() {
    use crate::game::ability::AbilityKind;

    for (master, seconds) in [(false, 30), (true, 20)] {
        let mut game = empty_flat_game(&[cultivator_player()]);
        let caster = game
            .state
            .entities
            .spawn_unit(1, EntityKind::MagicAdept, 400.0, 400.0)
            .unwrap();
        game.state.entities.get_mut(caster).unwrap().dao_master = master;
        game.enqueue(
            1,
            Command::AttackMove {
                units: vec![caster],
                x: 800.0,
                y: 400.0,
                queued: false,
            },
        );
        game.tick();
        game.enqueue(
            1,
            Command::Move {
                units: vec![caster],
                x: 900.0,
                y: 400.0,
                queued: true,
            },
        );
        game.tick();
        let unit = game.state.entities.get(caster).unwrap();
        let before_x = unit.pos_x();
        let order = unit.order();
        let queue = unit.queued_orders().to_vec();

        let cast = Command::VectorField {
            units: vec![caster],
            x: 480.0,
            y: 400.0,
            direction_x: 0.0,
            direction_y: -1.0,
        };
        game.enqueue(1, cast.clone());
        game.tick();
        let unit = game.state.entities.get(caster).unwrap();
        assert_eq!(unit.order(), order);
        assert_eq!(unit.queued_orders(), queue);
        assert!(unit.pos_x() > before_x, "casting must not pause movement");
        let cooldown = (seconds * config::TICK_HZ) as u16;
        assert_eq!(
            unit.ability_cooldown_ticks(AbilityKind::VectorField),
            cooldown - 1
        );
        let field_id = game
            .state
            .ability_runtime
            .world_objects()
            .next()
            .unwrap()
            .id;

        game.enqueue(1, cast);
        game.tick();
        assert_eq!(
            game.state
                .ability_runtime
                .world_objects()
                .next()
                .unwrap()
                .id,
            field_id
        );

        for _ in 0..cooldown - 3 {
            game.tick();
        }
        let unit = game.state.entities.get(caster).unwrap();
        assert_eq!(unit.ability_cooldown_ticks(AbilityKind::VectorField), 1);
        let recast = Command::VectorField {
            units: vec![caster],
            x: unit.pos_x(),
            y: unit.pos_y(),
            direction_x: 1.0,
            direction_y: 0.0,
        };
        game.enqueue(1, recast.clone());
        game.tick();
        assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
        game.enqueue(1, recast);
        game.tick();
        assert_eq!(game.state.ability_runtime.world_objects().count(), 1);
    }
}
