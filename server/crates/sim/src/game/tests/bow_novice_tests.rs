use super::fixtures::*;
use super::*;
use crate::game::ability::AbilityKind;

fn cultivator() -> PlayerInit {
    PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::CULTIVATORS_FACTION_ID.into(),
        name: "Cultivator".into(),
        color: "#fff".into(),
        is_ai: false,
    }
}

#[test]
fn bow_novice_portal_production_needs_no_research_and_reserves_two_supply() {
    let mut game = empty_flat_game(&[cultivator()]);
    game.state.players[0].set_resources(50, 0);
    let portal = game
        .state
        .entities
        .spawn_building(1, EntityKind::Portal, 624.0, 624.0, true)
        .unwrap();
    game.enqueue(
        1,
        Command::Train {
            building: portal,
            unit: EntityKind::BowNovice,
        },
    );
    game.tick();
    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].supply_used, 2);
    assert!(game.state.players[0].upgrades.is_empty());
    assert_eq!(
        game.state.entities.get(portal).unwrap().prod_queue()[0].total,
        300
    );
    for _ in 0..300 {
        game.tick();
    }
    let bow = game
        .state
        .entities
        .iter()
        .find(|e| e.kind == EntityKind::BowNovice)
        .unwrap();
    assert_eq!((bow.hp, bow.max_hp), (48, 48));
    assert_eq!((bow.radius(), bow.movement_speed()), (17.0, 2.0));
}

#[test]
fn bow_novice_combat_insight_evolves_to_every_chosen_dao_and_checkpoints() {
    for (choice, kind) in [
        (AbilityKind::EvolveWind, EntityKind::WindAdept),
        (AbilityKind::EvolveWar, EntityKind::WarAdept),
        (AbilityKind::EvolveMagic, EntityKind::MagicAdept),
    ] {
        let mut game = empty_flat_game(&[cultivator()]);
        let id = game
            .state
            .entities
            .spawn_unit(1, EntityKind::BowNovice, 624.0, 624.0)
            .unwrap();
        game.tick(); // rebuild supply before exporting the checkpoint
        let bow = game.state.entities.get_mut(id).unwrap();
        assert!(bow.choose_dao(choice));
        bow.gain_insight(59, EntityKind::Rifleman);
        bow.gain_insight(3, EntityKind::Depot);
        assert_eq!(bow.insight, 59);
        assert_eq!(
            bow.carried_item_capacity(),
            Some(config::CULTIVATOR_NOVICE_ITEM_CAPACITY)
        );
        bow.set_attack_cd(20);
        bow.apply_damage(bow.hp - 1, None);
        // A pending choice on a Bow Novice is a valid checkpoint, not only a valid entity.
        let checkpoint = game.checkpoint_payload_text_for_test().unwrap();
        game = Game::restore_checkpoint_payload_text_for_test(
            &checkpoint,
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        game.state
            .entities
            .get_mut(id)
            .unwrap()
            .gain_insight(1, EntityKind::Depot);
        game.tick();
        let evolved = game.state.entities.get(id).unwrap();
        assert_eq!(evolved.kind, kind);
        assert_eq!(evolved.hp, evolved.max_hp);
        assert_eq!(evolved.insight, 0);
        assert!(evolved.attack_cd() > 0, "evolution must preserve reload");
    }
}
