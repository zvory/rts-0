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
