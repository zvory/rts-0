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
    game.state.players[0].set_resources(50, 0);
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
    assert_eq!((warrior.hp, warrior.max_hp), (58, 58));
    assert_eq!(warrior.radius(), 17.0);
    assert_eq!(warrior.movement_speed(), 2.0);
}

#[test]
fn cultivator_portal_trains_researched_adepts_for_60_oil_without_novices() {
    use crate::game::entity::{Order, RallyIntent, RallyKind};
    use crate::game::upgrade::UpgradeKind;

    for (kind, required) in [
        (EntityKind::WindAdept, UpgradeKind::DaoWind),
        (EntityKind::WarAdept, UpgradeKind::DaoOfHeaviness),
        (EntityKind::MagicAdept, UpgradeKind::DaoOfMagic),
    ] {
        let mut game = empty_flat_game(&[cultivator_player()]);
        game.state.players[0].set_resources(150, 59);
        let portal = game
            .state
            .entities
            .spawn_building(1, EntityKind::Portal, 624.0, 624.0, true)
            .unwrap();
        game.state
            .entities
            .get_mut(portal)
            .unwrap()
            .set_rally_point(Some(RallyIntent::new(RallyKind::Move, 950.0, 624.0)));
        // Other Dao research must not unlock this specialization.
        for other in [
            UpgradeKind::DaoWind,
            UpgradeKind::DaoOfHeaviness,
            UpgradeKind::DaoOfMagic,
        ] {
            if other != required {
                game.state.players[0].upgrades.insert(other);
            }
        }
        game.enqueue(
            1,
            Command::Train {
                building: portal,
                unit: kind,
            },
        );
        game.tick();
        assert!(game
            .state
            .entities
            .get(portal)
            .unwrap()
            .prod_queue()
            .is_empty());
        game.state.players[0].upgrades.insert(required);
        game.enqueue(
            1,
            Command::Train {
                building: portal,
                unit: kind,
            },
        );
        game.tick();
        let queued = &game.state.entities.get(portal).unwrap().prod_queue()[0];
        assert!(!queued.paid);
        assert_eq!(queued.progress, 0);
        assert_eq!(game.state.players[0].oil, 59);
        assert_eq!(game.state.players[0].supply_used, 0);
        game.state.players[0].set_resources(149, 60);
        game.tick();
        assert!(!game.state.entities.get(portal).unwrap().prod_queue()[0].paid);
        assert_eq!(game.state.players[0].oil, 60);
        game.state.players[0].set_resources(150, 60);
        game.tick();
        assert_eq!(game.state.players[0].steel, 0);
        assert_eq!(game.state.players[0].oil, 0);
        assert_eq!(game.state.players[0].supply_used, 3);
        let progress = game.state.entities.get(portal).unwrap().prod_queue()[0].progress;
        assert_eq!(
            game.state.entities.get(portal).unwrap().prod_queue()[0].total,
            300
        );
        let mut restored = Game::restore_checkpoint_payload_text_for_test(
            &game.checkpoint_payload_text_for_test().unwrap(),
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        for _ in progress..299 {
            game.tick();
            restored.tick();
        }
        assert!(!game.state.entities.iter().any(|e| e.kind.is_disciple()));
        game.tick();
        restored.tick();
        let adept = game.state.entities.iter().find(|e| e.kind == kind).unwrap();
        assert_eq!(adept.dao(), Some(required));
        assert_eq!(adept.insight, 0);
        assert_eq!(adept.insight_cap(), 200);
        assert_eq!(adept.hp, adept.max_hp);
        assert!(matches!(adept.order(), Order::AttackMove(_)));
        assert_eq!(game.state.players[0].supply_used, 3);
        assert_eq!(
            game.replay_end_state_hash().unwrap(),
            restored.replay_end_state_hash().unwrap()
        );
    }
}

#[test]
fn cultivator_portal_adept_cancellation_refunds_60_oil() {
    use crate::game::upgrade::UpgradeKind;
    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(150, 60);
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfMagic);
    let portal = game
        .state
        .entities
        .spawn_building(1, EntityKind::Portal, 624.0, 624.0, true)
        .unwrap();
    game.enqueue(
        1,
        Command::Train {
            building: portal,
            unit: EntityKind::MagicAdept,
        },
    );
    game.tick();
    assert_eq!(game.state.players[0].oil, 0);
    game.enqueue(
        1,
        Command::Cancel {
            building: portal,
            construction: false,
        },
    );
    game.tick();
    assert!(game
        .state
        .entities
        .get(portal)
        .unwrap()
        .prod_queue()
        .is_empty());
    assert_eq!(game.state.players[0].oil, 60);
    assert_eq!(game.state.players[0].steel, 150);
    assert_eq!(game.state.players[0].supply_used, 0);
}

#[test]
fn cultivator_portal_repeat_adept_waits_for_research_and_charges_each_output() {
    use crate::game::upgrade::UpgradeKind;
    let mut game = empty_flat_game(&[cultivator_player()]);
    game.state.players[0].set_resources(300, 120);
    let portal = game
        .state
        .entities
        .spawn_building(1, EntityKind::Portal, 624.0, 624.0, true)
        .unwrap();
    game.enqueue(
        1,
        Command::AdjustProductionRepeat {
            buildings: vec![portal],
            unit: EntityKind::MagicAdept,
            delta: 1,
        },
    );
    game.tick();
    assert!(game
        .state
        .entities
        .get(portal)
        .unwrap()
        .prod_queue()
        .is_empty());
    assert_eq!(game.state.players[0].oil, 120);
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfMagic);
    for _ in 0..605 {
        game.tick();
    }
    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::MagicAdept)
            .count(),
        2
    );
    assert_eq!(game.state.players[0].oil, 0);
    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].supply_used, 6);
}
