use super::fixtures::*;
use super::*;
use crate::game::ability::AbilityKind;
use crate::game::upgrade::{self, UpgradeKind};

fn library_game() -> Game {
    empty_flat_game(&[
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "cultivators".into(),
            name: "Cultivator".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".into(),
            name: "Opponent".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ])
}

#[test]
fn technique_library_research_pays_fixed_prices_and_survives_building_loss() {
    let mut game = library_game();
    game.state.players[0].set_resources(300, 300);
    let library = game
        .state
        .entities
        .spawn_building(1, EntityKind::TechniqueLibrary, 500.0, 500.0, true)
        .unwrap();
    let techniques = [
        UpgradeKind::TechniqueCharge,
        UpgradeKind::TechniqueShield,
        UpgradeKind::TechniqueThrowSword,
    ];
    for kind in techniques {
        let def = upgrade::definition(kind);
        assert_eq!((def.cost_steel, def.cost_oil), (100, 100));
        assert_eq!(def.research_ticks, 600);
        game.enqueue(
            1,
            Command::Research {
                building: library,
                upgrade: kind,
            },
        );
    }
    game.enqueue(
        1,
        Command::Research {
            building: library,
            upgrade: techniques[0],
        },
    );
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(library)
            .unwrap()
            .research_queue()
            .len(),
        3
    );
    for _ in 0..2100 {
        game.tick();
    }
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
    game.state.entities.remove(library);
    for kind in techniques {
        assert!(game.state.players[0].has_upgrade(kind));
    }
}

#[test]
fn technique_library_gates_abilities_for_adepts_and_masters_but_not_blink_or_vector_field() {
    for master in [false, true] {
        for (unit_kind, ability, required) in [
            (EntityKind::WindAdept, AbilityKind::WindDash, None),
            (
                EntityKind::MagicAdept,
                AbilityKind::DiscipleShield,
                Some(UpgradeKind::TechniqueShield),
            ),
            (
                EntityKind::WarAdept,
                AbilityKind::ThrowSword,
                Some(UpgradeKind::TechniqueThrowSword),
            ),
            (EntityKind::MagicAdept, AbilityKind::VectorField, None),
        ] {
            for queued in [false, true] {
                let mut game = library_game();
                let id = game
                    .state
                    .entities
                    .spawn_unit(1, unit_kind, 500.0, 500.0)
                    .unwrap();
                if master {
                    game.state.entities.get_mut(id).unwrap().dao_master = true;
                }
                let cast = if ability == AbilityKind::VectorField {
                    Command::VectorField {
                        units: vec![id],
                        x: 560.0,
                        y: 500.0,
                        direction_x: 0.0,
                        direction_y: -1.0,
                    }
                } else {
                    Command::UseAbility {
                        target_id: None,
                        ability,
                        units: vec![id],
                        x: Some(560.0),
                        y: Some(500.0),
                        queued,
                    }
                };
                game.enqueue(1, cast.clone());
                game.tick();
                if let Some(required) = required {
                    assert!(
                        game.state.ability_runtime.world_objects().next().is_none(),
                        "{ability:?} locked"
                    );
                    assert_eq!(
                        game.state
                            .entities
                            .get(id)
                            .unwrap()
                            .ability_cooldown_ticks(ability),
                        0
                    );
                    game.state.players[0].upgrades.insert(required);
                    game.enqueue(1, cast);
                    game.tick();
                }
                let unit = game.state.entities.get(id).unwrap();
                assert!(
                    unit.ability_cooldown_ticks(ability) > 0
                        || ability == AbilityKind::WindDash && unit.pos_x() > 520.0,
                    "{ability:?} available after research, master={master}, queued={queued}"
                );
            }
        }
    }
}

#[test]
fn technique_library_gates_automatic_charge_for_novice_and_war_ranks() {
    for kind in [EntityKind::Warrior, EntityKind::WarAdept] {
        for master in [false, true] {
            let mut game = library_game();
            let id = game
                .state
                .entities
                .spawn_unit(1, kind, 500.0, 500.0)
                .unwrap();
            if master && kind == EntityKind::WarAdept {
                game.state.entities.get_mut(id).unwrap().dao_master = true;
            }
            let enemy = game
                .state
                .entities
                .spawn_unit(2, EntityKind::Rifleman, 630.0, 500.0)
                .unwrap();
            game.state
                .entities
                .get_mut(id)
                .unwrap()
                .set_order(Order::attack(enemy));
            game.state
                .fog
                .recompute(&[1, 2], &game.state.entities, &game.state.map);
            game.tick();
            assert_eq!(
                game.state
                    .entities
                    .get(id)
                    .unwrap()
                    .ability_cooldown_ticks(AbilityKind::Charge),
                0
            );
            game.state.players[0]
                .upgrades
                .insert(UpgradeKind::TechniqueCharge);
            game.tick();
            assert!(
                game.state
                    .entities
                    .get(id)
                    .unwrap()
                    .ability_cooldown_ticks(AbilityKind::Charge)
                    > 0
            );
        }
    }
}

#[test]
fn technique_library_rejects_retired_blink_research() {
    let mut game = library_game();
    game.state.players[0].set_resources(100, 100);
    let library = game
        .state
        .entities
        .spawn_building(1, EntityKind::TechniqueLibrary, 500.0, 500.0, true)
        .unwrap();
    game.enqueue(
        1,
        Command::Research {
            building: library,
            upgrade: UpgradeKind::TechniqueBlink,
        },
    );
    game.tick();
    assert!(game
        .state
        .entities
        .get(library)
        .unwrap()
        .research_queue()
        .is_empty());
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (100, 100)
    );
}
