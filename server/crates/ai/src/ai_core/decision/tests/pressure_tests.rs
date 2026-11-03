use super::*;
use crate::ai_core::profiles::PRESSURE;

#[test]
fn pressure_stops_legacy_rifleman_orders_only_after_panzerfaust_research_completes() {
    let mut owned = vec![
        worker(1, AiEntityState::Idle),
        building(2, EntityKind::ResourceDepot, Some(0)),
        building(3, EntityKind::Barracks, Some(0)),
        building(4, EntityKind::TrainingCentre, Some(0)),
    ];
    owned.extend((5..9).map(|id| combat(id, EntityKind::MachineGunner)));
    let mut obs = observation(
        AiEconomy {
            steel: 200,
            oil: 50,
            supply_used: 10,
            supply_cap: 300,
        },
        owned,
    );
    obs.tick = 1;
    obs.upgrades.push(UpgradeKind::Methamphetamines);
    let before = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(
        before.commands.iter().any(|c| matches!(
            c,
            Command::Train {
                unit: EntityKind::Rifleman,
                ..
            }
        )),
        "Rifleman production must continue while Panzerfausts are locked"
    );
    obs.upgrades.push(UpgradeKind::Panzerfausts);
    obs.owned.push(combat(99, EntityKind::Panzerfaust));
    let after = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(
        !after.commands.iter().any(|c| matches!(
            c,
            Command::Train {
                unit: EntityKind::Rifleman,
                ..
            }
        )),
        "legacy Rifleman training must not compete with Panzerfaust repeat"
    );
    assert!(
        after.pressure_wave_units.contains(&99),
        "Panzerfausts must join the assault staging owner"
    );
}

#[test]
fn pressure_retries_first_wave_research_on_a_replacement_training_centre() {
    let mut obs = observation(
        AiEconomy {
            steel: 1000,
            oil: 1000,
            supply_used: 10,
            supply_cap: 300,
        },
        vec![
            worker(1, AiEntityState::Idle),
            building(2, EntityKind::ResourceDepot, Some(0)),
            building(3, EntityKind::Barracks, Some(0)),
            building(4, EntityKind::TrainingCentre, Some(0)),
            building(5, EntityKind::EngineeringComplex, Some(0)),
            building(6, EntityKind::Factory, Some(0)),
        ],
    );
    obs.tick = 2500;
    let mut memory = AiDecisionMemory::for_profile(&PRESSURE);
    let first = decide(&obs, &PRESSURE, &mut memory);
    assert!(first.commands.iter().any(|c| matches!(
        c,
        Command::Research {
            building: 4,
            upgrade: UpgradeKind::Methamphetamines
        }
    )));
    obs.tick += 9;
    let pending = decide(&obs, &PRESSURE, &mut memory);
    assert!(
        !pending.commands.iter().any(|c| matches!(
            c,
            Command::Research {
                upgrade: UpgradeKind::Methamphetamines,
                ..
            }
        )),
        "live producer research must not be requeued"
    );
    obs.owned.retain(|e| e.id != 4);
    obs.owned
        .push(building(14, EntityKind::TrainingCentre, Some(0)));
    obs.tick += 9;
    let retry = decide(&obs, &PRESSURE, &mut memory);
    assert!(
        retry.commands.iter().any(|c| matches!(
            c,
            Command::Research {
                building: 14,
                upgrade: UpgradeKind::Methamphetamines
            }
        )),
        "lost research must not permanently lock the opening wave"
    );
}

#[test]
fn pressure_hands_nearby_panzerfausts_to_the_single_sdk_controller() {
    let home = 8.5 * config::TILE_SIZE as f32;
    let mut obs = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 10,
            supply_cap: 300,
        },
        vec![
            worker_at(1, AiEntityState::Idle, home, home),
            building_at(2, EntityKind::ResourceDepot, Some(0), home, home),
            combat_at(99, EntityKind::Panzerfaust, home, home),
        ],
    );
    obs.tick = 6000;
    obs.upgrades
        .extend([UpgradeKind::Methamphetamines, UpgradeKind::Panzerfausts]);
    let mut enemy = combat_at(100, EntityKind::Tank, home + 32.0, home);
    enemy.owner = 2;
    obs.visible_enemies.push(enemy);
    let decision = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(!decision
        .commands
        .iter()
        .any(|c| matches!(c, Command::Attack { units, .. } if units.contains(&99))));
    assert!(
        decision.pressure_wave_units.contains(&99),
        "the SDK response owns home threats as well as staging"
    );
}

#[test]
fn pressure_has_no_legacy_home_reservation_for_engaged_mobile_units() {
    let home = 8.5 * config::TILE_SIZE as f32;
    for kind in [EntityKind::Panzerfaust, EntityKind::Rifleman] {
        let mut defender = combat_at(99, kind, home, home);
        defender.state = AiEntityState::Attack;
        defender.target_id = Some(100);
        defender.free_for_combat = false;
        let mut obs = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 10,
                supply_cap: 300,
            },
            vec![
                worker_at(1, AiEntityState::Idle, home, home),
                building_at(2, EntityKind::ResourceDepot, Some(0), home, home),
                defender,
            ],
        );
        obs.tick = 6009;
        obs.upgrades
            .extend([UpgradeKind::Methamphetamines, UpgradeKind::Panzerfausts]);
        let mut enemy = combat_at(100, EntityKind::Tank, home + 32.0, home);
        enemy.owner = 2;
        obs.visible_enemies.push(enemy);
        let mut memory = AiDecisionMemory::for_profile(&PRESSURE);
        let defending = decide(&obs, &PRESSURE, &mut memory);
        assert!(
            defending.pressure_wave_units.contains(&99),
            "engaged {kind:?} belongs to the SDK controller"
        );
        obs.owned.iter_mut().filter(|e| e.id == 99).for_each(|e| {
            e.x = 1300.0;
            e.y = 1300.0;
        });
        obs.tick += 9;
        let away = decide(&obs, &PRESSURE, &mut memory);
        assert!(
            away.pressure_wave_units.contains(&99),
            "a fight outside home must not permanently reserve staging recruits"
        );
        obs.owned.iter_mut().filter(|e| e.id == 99).for_each(|e| {
            e.x = home;
            e.y = home;
        });
        obs.visible_enemies.clear();
        obs.tick += 9;
        let released = decide(&obs, &PRESSURE, &mut memory);
        assert!(
            released.pressure_wave_units.contains(&99),
            "staging resumes when the home threat is gone"
        );
    }
}

fn pressure_economy_observation(depots: usize, factories: usize) -> AiObservation {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        worker_at(1, AiEntityState::Idle, 8.5 * ts, 8.5 * ts),
        building(10, EntityKind::Barracks, Some(0)),
        building(11, EntityKind::TrainingCentre, Some(0)),
        building(12, EntityKind::EngineeringComplex, Some(0)),
    ];
    owned.extend((0..depots).map(|i| {
        building_at(
            20 + i as u32,
            EntityKind::ResourceDepot,
            Some(0),
            (8.5 + i as f32 * 10.0) * ts,
            8.5 * ts,
        )
    }));
    owned.extend((0..factories).map(|i| building(100 + i as u32, EntityKind::Factory, Some(0))));
    owned.extend((200..204).map(|id| combat(id, EntityKind::MachineGunner)));
    let mut obs = with_expansion_resources(observation(
        AiEconomy {
            steel: 1500,
            oil: 500,
            supply_used: 60,
            supply_cap: 300,
        },
        owned,
    ));
    obs.tick = 9000;
    obs.upgrades.extend([
        UpgradeKind::Methamphetamines,
        UpgradeKind::Panzerfausts,
        UpgradeKind::TankUnlock,
    ]);
    obs
}

#[test]
fn pressure_uses_surplus_for_fourth_and_fifth_without_saving_instead_of_training_tanks() {
    for depots in [3, 4] {
        let mut obs = pressure_economy_observation(depots, 2);
        obs.economy.steel = 425;
        obs.economy.oil = 175;
        let decision = decide(
            &obs,
            &PRESSURE,
            &mut AiDecisionMemory::for_profile(&PRESSURE),
        );
        assert!(
            decision.commands.iter().any(|command| matches!(
                command,
                Command::Train {
                    unit: EntityKind::Tank,
                    ..
                }
            )),
            "Pressure must keep funding Tanks at {depots} bases when a Depot is unaffordable"
        );
        obs.economy.steel = 1000;
        obs.economy.oil = 300;
        let decision = decide(
            &obs,
            &PRESSURE,
            &mut AiDecisionMemory::for_profile(&PRESSURE),
        );
        assert!(
            decision.commands.iter().any(|command| matches!(
                command,
                Command::Build {
                    building: EntityKind::ResourceDepot,
                    ..
                }
            )),
            "surplus must buy the next Depot from {depots} bases"
        );
        assert!(
            decision.commands.iter().any(|command| matches!(
                command,
                Command::Train {
                    unit: EntityKind::Tank,
                    ..
                }
            )),
            "a Depot purchase must still allow a Tank from the remaining resources"
        );
    }
    let obs = pressure_economy_observation(5, 2);
    let decision = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(!decision.commands.iter().any(|command| matches!(
        command,
        Command::Build {
            building: EntityKind::ResourceDepot,
            ..
        }
    )));
}

#[test]
fn pressure_protects_pending_depot_cost_but_keeps_tank_orders_above_it() {
    let mut obs = pressure_economy_observation(3, 2);
    obs.owned[0].state = AiEntityState::Build;
    obs.pending_builds
        .push(AiBuildIntent::to_site(1, EntityKind::ResourceDepot, 30, 30));
    for (steel, oil, should_train) in [(799, 500, false), (1500, 244, false), (800, 245, true)] {
        obs.economy.steel = steel;
        obs.economy.oil = oil;
        let decision = decide(
            &obs,
            &PRESSURE,
            &mut AiDecisionMemory::for_profile(&PRESSURE),
        );
        assert_eq!(
            decision.commands.iter().any(|command| matches!(
                command,
                Command::Train {
                    unit: EntityKind::Tank,
                    ..
                }
            )),
            should_train
        );
        assert!(!decision.commands.iter().any(|command| matches!(
            command,
            Command::Build {
                building: EntityKind::ResourceDepot,
                ..
            }
        )));
    }
}

#[test]
fn pressure_caps_factories_at_four_with_five_bases_and_float() {
    for (depots, factories, expected) in [
        (2, 1, false),
        (3, 1, true),
        (3, 2, false),
        (4, 2, false),
        (5, 2, true),
        (5, 3, true),
        (5, 4, false),
        (5, 9, false),
    ] {
        let obs = pressure_economy_observation(depots, factories);
        assert_eq!(
            should_build_extra_factory(&obs, &AiFacts::from_observation(&obs), &PRESSURE, 0),
            expected
        );
    }
    // A Works already planned this decision consumes the fourth slot too.
    let obs = pressure_economy_observation(5, 3);
    assert!(!should_build_extra_factory(
        &obs,
        &AiFacts::from_observation(&obs),
        &PRESSURE,
        1
    ));
    // Foundations count toward the cap before they complete.
    let mut obs = pressure_economy_observation(5, 4);
    obs.owned
        .iter_mut()
        .find(|e| e.id == 103)
        .unwrap()
        .is_complete = false;
    assert!(!should_build_extra_factory(
        &obs,
        &AiFacts::from_observation(&obs),
        &PRESSURE,
        0
    ));
    for (steel, oil) in [(500, 326), (501, 325)] {
        let mut obs = pressure_economy_observation(5, 2);
        obs.economy.steel = steel;
        obs.economy.oil = oil;
        assert!(!should_build_extra_factory(
            &obs,
            &AiFacts::from_observation(&obs),
            &PRESSURE,
            0
        ));
    }
    let obs = pressure_economy_observation(5, 2);
    assert!(!should_build_extra_factory(
        &obs,
        &AiFacts::from_observation(&obs),
        &crate::ai_core::profiles::AI_2_1,
        0
    ));
}

#[test]
fn pressure_tank_repeat_starts_at_five_and_configures_replacements_once() {
    let mut obs = pressure_economy_observation(4, 2);
    obs.resources.clear();
    let mut memory = AiDecisionMemory::for_profile(&PRESSURE);
    let before = decide(&obs, &PRESSURE, &mut memory);
    assert!(!before.commands.iter().any(|c| matches!(
        c,
        Command::AdjustProductionRepeat {
            unit: EntityKind::Tank,
            ..
        }
    )));
    obs.owned
        .push(building(24, EntityKind::ResourceDepot, Some(0)));
    let configured = decide(&obs, &PRESSURE, &mut memory);
    assert_eq!(
        configured
            .commands
            .iter()
            .filter(|c| matches!(
                c,
                Command::AdjustProductionRepeat {
                    unit: EntityKind::Tank,
                    delta: 1,
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(!configured.commands.iter().any(|c| matches!(
        c,
        Command::Train {
            unit: EntityKind::Tank,
            ..
        }
    )));
    let again = decide(&obs, &PRESSURE, &mut memory);
    assert!(!again.commands.iter().any(|c| matches!(
        c,
        Command::AdjustProductionRepeat {
            unit: EntityKind::Tank,
            ..
        }
    )));
    obs.owned.retain(|e| e.id != 100 && e.id != 24);
    let mut replacement = building(102, EntityKind::Factory, Some(0));
    replacement.is_complete = false;
    obs.owned.push(replacement);
    let unfinished = decide(&obs, &PRESSURE, &mut memory);
    assert!(!unfinished.commands.iter().any(|c| matches!(
        c,
        Command::AdjustProductionRepeat {
            unit: EntityKind::Tank,
            ..
        }
    )));
    obs.owned
        .iter_mut()
        .find(|e| e.id == 102)
        .unwrap()
        .is_complete = true;
    let replaced = decide(&obs, &PRESSURE, &mut memory);
    assert!(replaced.commands.iter().any(|c| matches!(c,
        Command::AdjustProductionRepeat { buildings, unit: EntityKind::Tank, delta: 1 } if buildings==&vec![102])));
    assert!(!replaced.commands.iter().any(|c| matches!(
        c,
        Command::Train {
            unit: EntityKind::Tank,
            ..
        }
    )));
}

#[test]
fn pressure_repeat_memory_does_not_disable_tanks_after_changing_profiles() {
    let obs = pressure_economy_observation(5, 2);
    let mut memory = AiDecisionMemory::for_profile(&PRESSURE);
    decide(&obs, &PRESSURE, &mut memory);
    let mut other = obs.clone();
    other.resources.clear();
    let decision = decide(&other, &crate::ai_core::profiles::AI_2_1, &mut memory);
    assert!(decision.commands.iter().any(|c| matches!(
        c,
        Command::Train {
            unit: EntityKind::Tank,
            ..
        }
    )));
}

#[test]
fn pressure_missing_sites_do_not_freeze_tank_production() {
    let mut obs = pressure_economy_observation(3, 2);
    obs.resources.clear();
    let decision = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(decision.commands.iter().any(|c| matches!(
        c,
        Command::Train {
            unit: EntityKind::Tank,
            ..
        }
    )));
}

#[test]
fn pressure_ranked_sites_skip_occupied_or_exhausted_clusters_and_original_starts() {
    use crate::sdk::{AiBaseSite, AiBaseSiteId, AiFrame, WorldQueries};
    use rts_sim::{
        game::{Game, PlayerInit},
        protocol::{EntityView, ResourceNode},
    };
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("P{id}"),
        color: "#fff".into(),
        is_ai: true,
    });
    let game = Game::new_without_ai_controllers(&players, 7);
    let mut start = game.start_payload();
    start.map.terrain.fill(0);
    start.map.no_building_tiles.clear();
    let ts = start.map.tile_size as f32;
    start.map.resources = [(1000, 20.5, 30.5), (1001, 50.5, 30.5)]
        .map(|(id, x, y)| ResourceNode {
            id,
            kind: "steel".into(),
            x: x * ts,
            y: y * ts,
        })
        .to_vec();
    let mut sites = [
        AiBaseSite {
            id: AiBaseSiteId::Start(1),
            position: (9.5 * ts, 9.5 * ts),
            resource_ids: vec![],
            ground_distances_from_starts: Default::default(),
        },
        AiBaseSite {
            id: AiBaseSiteId::Start(2),
            position: (15.5 * ts, 25.5 * ts),
            resource_ids: vec![],
            ground_distances_from_starts: Default::default(),
        },
        AiBaseSite {
            id: AiBaseSiteId::ResourceCluster(0),
            position: (20.5 * ts, 30.5 * ts),
            resource_ids: vec![1000],
            ground_distances_from_starts: Default::default(),
        },
        AiBaseSite {
            id: AiBaseSiteId::ResourceCluster(1),
            position: (50.5 * ts, 30.5 * ts),
            resource_ids: vec![1001],
            ground_distances_from_starts: Default::default(),
        },
    ];
    for site in &mut sites {
        let (dx, dy) = (site.position.0 - 9.5 * ts, site.position.1 - 9.5 * ts);
        site.ground_distances_from_starts
            .insert(1, (dx * dx + dy * dy).sqrt());
    }
    let mut snapshot = game.snapshot_for(1);
    snapshot.entities.clear();
    let choose =
        |snapshot: &rts_sim::protocol::Snapshot, blocked_near: bool, exhausted_near: bool| {
            let frame = AiFrame::from_host(&start, snapshot, 1, [], Some(&[1, 2]))
                .unwrap()
                .with_base_sites(&sites);
            let mut obs = AiObservation::from_frame(&frame).unwrap();
            if exhausted_near {
                obs.resources
                    .iter_mut()
                    .find(|r| r.id == 1000)
                    .unwrap()
                    .remaining = 0;
            }
            let mut plan = expansion::ExpansionPlan {
                policy: PRESSURE.production_expansion,
                should_save: true,
                blocks_tech_path: false,
                blockers: vec![],
            };
            pressure_economy::plan_site(
                &obs,
                &PRESSURE,
                Some(&WorldQueries::new(&frame)),
                &mut plan,
                &mut |_, x, _| !blocked_near || x > 35,
            )
        };
    let near = choose(&snapshot, false, false).unwrap();
    assert!(near.0 < 35);
    assert!(choose(&snapshot, true, false).unwrap().0 > 35);
    assert!(choose(&snapshot, false, true).unwrap().0 > 35);
    snapshot.entities.push(EntityView::new(
        100,
        2,
        "resource_depot",
        20.5 * ts,
        30.5 * ts,
        180,
        180,
        "idle",
    ));
    assert!(choose(&snapshot, false, false).unwrap().0 > 35);
    snapshot.entities.push(EntityView::new(
        101,
        1,
        "resource_depot",
        50.5 * ts,
        30.5 * ts,
        180,
        180,
        "idle",
    ));
    assert!(choose(&snapshot, false, false).is_none());
}

#[test]
fn pressure_three_base_tank_opening_keeps_its_second_works_before_later_expansion() {
    let obs = pressure_economy_observation(3, 1);
    let decision = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(decision.commands.iter().any(|c| matches!(
        c,
        Command::Build {
            building: EntityKind::Factory,
            ..
        }
    )));
    assert!(!decision.commands.iter().any(|c| matches!(
        c,
        Command::Build {
            building: EntityKind::ResourceDepot,
            ..
        }
    )));
    assert!(decision.commands.iter().any(|c| matches!(
        c,
        Command::Train {
            unit: EntityKind::Tank,
            ..
        }
    )));
}

#[test]
fn pressure_waits_for_the_fifth_depot_to_complete_before_factory_growth_and_repeat() {
    let mut obs = pressure_economy_observation(5, 2);
    obs.owned
        .iter_mut()
        .find(|e| e.id == 24)
        .unwrap()
        .is_complete = false;
    let decision = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(!decision.commands.iter().any(|c| matches!(
        c,
        Command::Build {
            building: EntityKind::Factory,
            ..
        }
    )));
    assert!(!decision.commands.iter().any(|c| matches!(
        c,
        Command::AdjustProductionRepeat {
            unit: EntityKind::Tank,
            ..
        }
    )));
    assert!(decision.commands.iter().any(|c| matches!(
        c,
        Command::Train {
            unit: EntityKind::Tank,
            ..
        }
    )));
}
