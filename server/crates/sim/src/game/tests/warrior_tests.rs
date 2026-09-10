use super::fixtures::*;
use super::*;

fn cultivator_player() -> PlayerInit {
    PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::CULTIVATORS_FACTION_ID.to_string(),
        name: "Cultivator".into(),
        color: "#fff".into(),
        is_ai: false,
    }
}

#[test]
fn cultivator_portal_trains_warrior_and_reserves_two_supply() {
    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(100, 0);
    let position = game.state.map.tile_center(12, 12);
    let portal = game
        .state
        .entities
        .spawn_building(1, EntityKind::Portal, position.0, position.1, true)
        .expect("Portal should spawn");

    game.enqueue(
        1,
        Command::Train {
            building: portal,
            unit: EntityKind::Warrior,
        },
    );
    game.tick();

    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].supply_used, 2);
    assert_eq!(
        game.state
            .entities
            .get(portal)
            .map(|producer| producer.prod_queue().len()),
        Some(1)
    );

    for _ in 0..config::unit_stats(EntityKind::Warrior)
        .expect("Warrior stats should exist")
        .build_ticks
    {
        game.tick();
    }

    let warrior = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Warrior)
        .expect("Portal should produce a Warrior");
    assert_eq!((warrior.hp, warrior.max_hp), (135, 135));
    assert_eq!(warrior.radius(), 13.5);
}
