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

#[test]
fn cultivator_pavilion_research_spends_resources_completes_and_leaves_disciple_unchanged() {
    use crate::game::upgrade::{self, UpgradeKind};

    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(600, 300);
    let pavilion_pos = game.state.map.tile_center(12, 12);
    let pavilion = game
        .state
        .entities
        .spawn_building(
            1,
            EntityKind::ThreeDaoPavilion,
            pavilion_pos.0,
            pavilion_pos.1,
            true,
        )
        .expect("Pavilion should spawn");
    let disciple_pos = game.state.map.tile_center(20, 20);
    let disciple = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, disciple_pos.0, disciple_pos.1)
        .expect("Disciple should spawn");
    let before = game.state.entities.get(disciple).unwrap().clone();
    let daos = [
        UpgradeKind::DaoOfLightness,
        UpgradeKind::DaoOfBarriers,
        UpgradeKind::DaoOfHeaviness,
    ];
    for dao in daos {
        game.enqueue(
            1,
            Command::Research {
                building: pavilion,
                upgrade: dao,
            },
        );
    }
    // Repeated requests must not create a second research or spend resources twice.
    game.enqueue(
        1,
        Command::Research {
            building: pavilion,
            upgrade: daos[0],
        },
    );
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(pavilion)
            .unwrap()
            .research_queue()
            .len(),
        3
    );
    for dao in daos {
        assert_eq!(upgrade::definition(dao).research_ticks, 600);
    }
    // The shared queue pays each item when it becomes active.
    for _ in 0..1800 {
        game.tick();
    }
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
    for dao in daos {
        assert!(game.state.players[0].upgrades.contains(&dao));
    }
    assert!(game
        .state
        .entities
        .get(pavilion)
        .unwrap()
        .research_queue()
        .is_empty());
    let after = game.state.entities.get(disciple).unwrap();
    assert_eq!(
        (after.hp, after.max_hp, after.pos_x(), after.pos_y()),
        (before.hp, before.max_hp, before.pos_x(), before.pos_y())
    );
    game.state.entities.remove(pavilion);
    game.tick();
    for dao in daos {
        assert!(game.state.players[0].upgrades.contains(&dao));
    }
}

#[test]
fn pavilion_prerequisites_and_research_are_faction_gated() {
    use crate::rules::{
        economy,
        faction::{self, UpgradeKind},
    };
    let pavilion = EntityKind::ThreeDaoPavilion;
    let cultivators = faction::CULTIVATORS_FACTION_ID;
    assert!(economy::can_build_for_faction(
        cultivators,
        EntityKind::Worker,
        pavilion
    ));
    assert!(!economy::can_build_for_faction(
        faction::DEFAULT_FACTION_ID,
        EntityKind::Worker,
        pavilion
    ));
    assert!(!economy::build_requirement_met(
        pavilion,
        &[EntityKind::ResourceDepot]
    ));
    assert!(!economy::build_requirement_met(
        pavilion,
        &[EntityKind::Portal]
    ));
    assert!(economy::build_requirement_met(
        pavilion,
        &[EntityKind::ResourceDepot, EntityKind::Portal]
    ));
    for dao in [
        UpgradeKind::DaoOfLightness,
        UpgradeKind::DaoOfBarriers,
        UpgradeKind::DaoOfHeaviness,
    ] {
        assert!(economy::can_research_for_faction(
            cultivators,
            dao,
            pavilion
        ));
        assert!(!economy::can_research_for_faction(
            cultivators,
            dao,
            EntityKind::Portal
        ));
        assert!(!economy::can_research_for_faction(
            faction::DEFAULT_FACTION_ID,
            dao,
            pavilion
        ));
    }
}
