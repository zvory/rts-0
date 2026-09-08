use super::*;
use crate::ai_core::profiles::JEFFS_AI;

fn security_observation() -> AiObservation {
    let ts = config::TILE_SIZE as f32;
    let mut units = vec![
        worker(1, AiEntityState::Idle),
        building_at(2, EntityKind::ResourceDepot, Some(0), 8.5 * ts, 8.5 * ts),
        building(3, EntityKind::Barracks, Some(0)),
        building(4, EntityKind::TrainingCentre, Some(0)),
        building(5, EntityKind::EngineeringComplex, Some(0)),
        building(6, EntityKind::Factory, Some(1)),
    ];
    units.last_mut().unwrap().production_kind = Some(EntityKind::Tank);
    units.extend((10..16).map(|id| combat_at(id, EntityKind::Rifleman, 14.0 * ts, 14.0 * ts)));
    let mut obs = observation(
        AiEconomy {
            steel: 40,
            oil: 0,
            supply_used: 20,
            supply_cap: 80,
        },
        units,
    );
    obs.resources
        .push(resource(300, EntityKind::Steel, 28.0 * ts, 28.0 * ts));
    obs.resources
        .push(resource(301, EntityKind::Oil, 30.0 * ts, 28.0 * ts));
    obs.upgrades.push(UpgradeKind::TankUnlock);
    obs
}

#[test]
fn expansion_security_dispatches_before_affordability_or_build_intent() {
    let obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let decision = decide(&obs, &JEFFS_AI, &mut memory);
    assert!(memory.expansion_security.site.is_some());
    assert_eq!(memory.expansion_security.riflemen, vec![14, 15]);
    assert!(obs.pending_builds.is_empty());
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::ResourceDepot
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    let text = format!("{:?}", decision.commands);
    assert!(
        text.contains("AttackMove"),
        "advance orders missing: {text}"
    );
}

#[test]
fn expansion_footprint_is_predicted_before_the_expansion_opening() {
    let mut obs = security_observation();
    obs.map.width = 166;
    obs.map.height = 166;
    obs.own_start_tile = (157, 47);
    obs.owned
        .retain(|entity| entity.kind != EntityKind::Factory);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        &mut |_, _, _| true,
    );
    assert!(memory.expansion_security.site.is_some());
    assert!(memory.expansion_security.riflemen.is_empty());
}

#[test]
fn predicted_expansion_footprint_evicts_friendly_combat_units() {
    let mut obs = security_observation();
    obs.map.width = 166;
    obs.map.height = 166;
    obs.own_start_tile = (157, 47);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        &mut |_, _, _| true,
    );
    let center = building_center(
        memory.expansion_security.site.unwrap(),
        EntityKind::ResourceDepot,
        obs.map.tile_size,
    )
    .unwrap();
    obs.owned
        .push(combat_at(99, EntityKind::Rifleman, center.0, center.1));
    let facts = AiFacts::from_observation(&obs);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let blockers = expansion_security::clear_reserved_footprint(&obs, &memory, &mut actions);
    assert_eq!(blockers, vec![99]);
    assert!(actions
        .into_commands()
        .iter()
        .any(|command| matches!(command, Command::Move { units, .. } if units == &[99])));
}

#[test]
fn expansion_security_requires_arrival_and_uncontested_dwell() {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        &mut |_, _, _| true,
    );
    let points = expansion_security::positions(&obs, None, &memory.expansion_security);
    assert_eq!(points.len(), 2);
    assert!(
        geometry::dist2(points[0].0, points[0].1, points[1].0, points[1].1).sqrt()
            >= 2.75 * config::TILE_SIZE as f32
    );
    let step = |obs: &AiObservation, memory: &mut AiDecisionMemory| {
        let facts = AiFacts::from_observation(obs);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
        expansion_security::update_and_stage(obs, None, memory, &mut actions)
    };
    assert!(!step(&obs, &mut memory));
    for (index, id) in memory.expansion_security.riflemen.iter().enumerate() {
        let unit = obs.owned.iter_mut().find(|unit| unit.id == *id).unwrap();
        unit.x = (16.0 + index as f32 * 3.0) * config::TILE_SIZE as f32;
        unit.y = 16.0 * config::TILE_SIZE as f32;
    }
    assert!(memory.expansion_security.riflemen.iter().all(|id| {
        let unit = obs.owned.iter().find(|unit| unit.id == *id).unwrap();
        let site = memory.expansion_security.site.unwrap();
        let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
        geometry::dist2(unit.x, unit.y, center.0, center.1).sqrt()
            < geometry::dist2(
                config::TILE_SIZE as f32 * 8.5,
                config::TILE_SIZE as f32 * 8.5,
                center.0,
                center.1,
            )
    }));
    obs.tick += config::TICK_HZ * 3;
    assert!(
        !step(&obs, &mut memory),
        "guards merely partway to the expansion must not start the secure dwell"
    );
    for (id, point) in memory.expansion_security.riflemen.iter().zip(&points) {
        let unit = obs.owned.iter_mut().find(|unit| unit.id == *id).unwrap();
        unit.x = point.0;
        unit.y = point.1;
    }
    assert!(!step(&obs, &mut memory));
    obs.tick += config::TICK_HZ * 3;
    assert!(step(&obs, &mut memory));
    let mut enemy = combat_at(900, EntityKind::Tank, points[0].0, points[0].1);
    enemy.owner = 2;
    obs.visible_enemies.push(enemy);
    assert!(!step(&obs, &mut memory));
    obs.visible_enemies.clear();
    obs.tick += 9;
    assert!(
        !step(&obs, &mut memory),
        "lost contact must not bypass the dwell"
    );
}

#[test]
fn contested_expansion_guards_are_available_to_local_defense() {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        &mut |_, _, _| true,
    );
    let site = memory.expansion_security.site.unwrap();
    let points = expansion_security::positions(&obs, None, &memory.expansion_security);
    for (id, point) in memory.expansion_security.riflemen.iter().zip(&points) {
        let unit = obs.owned.iter_mut().find(|unit| unit.id == *id).unwrap();
        unit.x = point.0;
        unit.y = point.1;
    }
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    obs.owned.push(building_at(
        50,
        EntityKind::ResourceDepot,
        Some(0),
        center.0,
        center.1,
    ));
    let mut enemy = combat_at(900, EntityKind::Rifleman, points[0].0, points[0].1);
    enemy.owner = 2;
    obs.visible_enemies.push(enemy);

    let decision = decide(&obs, &JEFFS_AI, &mut memory);
    assert!(decision.commands.iter().any(|command| matches!(
        command,
        Command::Attack { units, target, .. }
            if *target == 900 && units.iter().any(|id| memory.expansion_security.riflemen.contains(id))
    )), "{:?}", decision.commands);
}

#[test]
fn expansion_security_retains_party_and_replaces_casualties_without_taking_home_four() {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let prepare = |obs: &AiObservation, memory: &mut AiDecisionMemory| {
        expansion_security::prepare(
            obs,
            &AiFacts::from_observation(obs),
            &JEFFS_AI,
            memory,
            &mut |_, _, _| true,
        )
    };
    prepare(&obs, &mut memory);
    let site = memory.expansion_security.site;
    obs.owned.push(combat(16, EntityKind::Rifleman));
    prepare(&obs, &mut memory);
    assert_eq!(memory.expansion_security.riflemen, vec![14, 15]);
    obs.owned.retain(|unit| unit.id != 14);
    prepare(&obs, &mut memory);
    assert_eq!(memory.expansion_security.riflemen, vec![15, 16]);
    assert_eq!(memory.expansion_security.site, site);
}

#[test]
fn expansion_security_waits_safely_when_only_one_guard_is_available() {
    let mut obs = security_observation();
    obs.owned.retain(|unit| unit.id != 15);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let decision = decide(&obs, &JEFFS_AI, &mut memory);

    assert_eq!(memory.expansion_security.riflemen, vec![14]);
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::ResourceDepot
    }));
}

#[test]
fn expansion_security_does_not_start_before_first_tank_commitment() {
    let mut obs = security_observation();
    obs.owned
        .iter_mut()
        .find(|unit| unit.kind == EntityKind::Factory)
        .unwrap()
        .production_kind = None;
    assert!(!expansion_security::expansion_is_next(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI
    ));
}

#[test]
fn expansion_security_allows_a_replacement_when_the_first_tank_is_lost() {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    decide(&obs, &JEFFS_AI, &mut memory);
    assert!(memory.expansion_security.site.is_some());

    let factory = obs
        .owned
        .iter_mut()
        .find(|unit| unit.kind == EntityKind::Factory)
        .unwrap();
    factory.production_kind = None;
    factory.production_queue_len = Some(0);
    obs.economy.steel = 2000;
    obs.economy.oil = 2000;

    let decision = decide(&obs, &JEFFS_AI, &mut memory);
    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
}

#[test]
fn expansion_security_spends_only_resources_above_the_depot_reserve() {
    let mut obs = security_observation();
    obs.economy.steel = 2000;
    obs.economy.oil = 2000;
    obs.owned.push(combat(20, EntityKind::Tank));
    let factory = obs
        .owned
        .iter_mut()
        .find(|unit| unit.kind == EntityKind::Factory)
        .unwrap();
    factory.production_kind = None;
    factory.production_queue_len = Some(0);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let first = decide(&obs, &JEFFS_AI, &mut memory);
    assert!(
        first.intents.contains(&AiIntent::Train {
            kind: EntityKind::Tank
        }),
        "surplus resources should remain available for Tanks: {:?}",
        first.intents
    );
    assert!(!first.intents.contains(&AiIntent::Build {
        kind: EntityKind::ResourceDepot
    }));
    let site = memory.expansion_security.site.unwrap();
    let points = expansion_security::positions(&obs, None, &memory.expansion_security);
    for (id, point) in memory.expansion_security.riflemen.iter().zip(&points) {
        let unit = obs.owned.iter_mut().find(|unit| unit.id == *id).unwrap();
        unit.x = point.0;
        unit.y = point.1;
    }
    let second = decide(&obs, &JEFFS_AI, &mut memory);
    assert!(!second.intents.contains(&AiIntent::Build {
        kind: EntityKind::ResourceDepot
    }));
    obs.tick += config::TICK_HZ * 3;
    let secured = decide(&obs, &JEFFS_AI, &mut memory);
    assert!(
        secured.commands.iter().any(|command| matches!(command,
        Command::Build { building: EntityKind::ResourceDepot, tile_x, tile_y, .. }
            if (*tile_x, *tile_y) == site)),
        "{:?}",
        secured.commands
    );
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    obs.owned.push(building_at(
        21,
        EntityKind::ResourceDepot,
        Some(0),
        center.0,
        center.1,
    ));
    decide(&obs, &JEFFS_AI, &mut memory);
    assert_eq!(memory.expansion_security.riflemen, vec![14, 15]);
}

#[test]
fn expansion_security_has_spaced_reachable_posts_on_river_and_crossroads() {
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, PlayerInit};
    let players: Vec<_> = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        })
        .collect();
    for name in ["Schone Tage", "The River", "1v1", "Crossroads"] {
        let map = Map::load_for_players(name, &[(1, 1), (2, 2)], 0x1234_5678).unwrap();
        let game = Game::new_with_random_ai_profiles_and_map_metadata(
            &players,
            0x1234_5678,
            map,
            Map::metadata_for_name(name).unwrap(),
        );
        let start = game.start_payload();
        let analysis = AiMapAnalysis::analyze(&start);
        for player in [1, 2] {
            let obs = AiObservation::from_snapshot_with_alive(
                &start,
                &game.snapshot_for(player),
                player,
                [],
                None,
            )
            .unwrap();
            let site = expansion::expansion_resource_depot_site(
                &obs,
                JEFFS_AI.expansion.unwrap(),
                EntityKind::ResourceDepot,
                JEFFS_AI.id,
                &mut |_, _, _| true,
            )
            .unwrap();
            let mut security = expansion_security::ExpansionSecurity::default();
            security.site = Some(site);
            let points = expansion_security::positions(&obs, Some(&analysis), &security);
            assert_eq!(points.len(), 2, "{name} player {player} site {site:?}");
            let center =
                building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
            let enemy = obs
                .players
                .iter()
                .find(|candidate| obs.is_enemy_player(candidate.id))
                .unwrap();
            let enemy_center = tile_center(enemy.start_tile, obs.map.tile_size);
            let crossroads_direction = defense::crossroads_wall_aware_approach_direction(&obs);
            if name == "Crossroads" {
                assert!(crossroads_direction.is_some(), "player {player}");
            } else {
                assert!(crossroads_direction.is_none(), "{name} player {player}");
            }
            let direction = crossroads_direction
                .or_else(|| normalized_direction(center, enemy_center))
                .unwrap();
            let stats = config::building_stats(EntityKind::ResourceDepot).unwrap();
            let ts = obs.map.tile_size as f32;
            for point in &points {
                assert!(
                    (point.0 - center.0).abs() > (stats.foot_w as f32 * 0.5 + 0.5) * ts
                        || (point.1 - center.1).abs() > (stats.foot_h as f32 * 0.5 + 0.5) * ts,
                    "guard blocks depot on {name} player {player}: {point:?}"
                );
                let forward =
                    (point.0 - center.0) * direction.0 + (point.1 - center.1) * direction.1;
                assert!(
                    forward > 0.0,
                    "guard faces away from the approach on {name} player {player}: {point:?}"
                );
            }
            assert!(
                geometry::dist2(points[0].0, points[0].1, points[1].0, points[1].1).sqrt()
                    >= 2.75 * obs.map.tile_size as f32
            );
        }
    }
}

#[test]
fn crossroads_expansion_tank_uses_the_same_wall_aware_approach_as_its_rifles() {
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, PlayerInit};

    let players: Vec<_> = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        })
        .collect();
    let map = Map::load_for_players("Crossroads", &[(1, 1), (2, 2)], 0x1234_5678).unwrap();
    let game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        0x1234_5678,
        map,
        Map::metadata_for_name("Crossroads").unwrap(),
    );
    let start = game.start_payload();
    let analysis = AiMapAnalysis::analyze(&start);

    for player in [1, 2] {
        let mut obs = AiObservation::from_snapshot_with_alive(
            &start,
            &game.snapshot_for(player),
            player,
            [],
            None,
        )
        .unwrap();
        let site = expansion::expansion_resource_depot_site(
            &obs,
            JEFFS_AI.expansion.unwrap(),
            EntityKind::ResourceDepot,
            JEFFS_AI.id,
            &mut |_, _, _| true,
        )
        .unwrap();
        let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
        obs.owned.push(building_at(
            900,
            EntityKind::ResourceDepot,
            Some(0),
            center.0,
            center.1,
        ));
        let mut security = expansion_security::ExpansionSecurity::default();
        security.site = Some(site);
        let rifles = expansion_security::positions(&obs, Some(&analysis), &security);
        let tank = expansion_security::tank_staging_center(&obs, Some(&analysis)).unwrap();
        let direction = defense::crossroads_wall_aware_approach_direction(&obs).unwrap();
        let projection = |point: (f32, f32)| {
            (point.0 - center.0) * direction.0 + (point.1 - center.1) * direction.1
        };

        assert_eq!(rifles.len(), 2, "player {player} site {site:?}");
        assert!(projection(tank) > 0.0, "player {player} tank {tank:?}");
        assert!(
            rifles
                .iter()
                .all(|point| projection(*point) > projection(tank)),
            "player {player} center {center:?} tank {tank:?} tank_forward={} rifles {rifles:?} rifle_forward={:?}",
            projection(tank),
            rifles.iter().map(|point| projection(*point)).collect::<Vec<_>>()
        );
    }
}

#[test]
fn completed_expansion_stages_tanks_between_the_depot_and_rifle_screen() {
    let mut obs = security_observation();
    assert!(expansion_security::tank_staging_center(&obs, None).is_none());

    let ts = obs.map.tile_size as f32;
    let site = (23, 23);
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    let mut depot = building_at(50, EntityKind::ResourceDepot, Some(0), center.0, center.1);
    depot.is_complete = false;
    obs.owned.push(depot);

    assert!(expansion_security::tank_staging_center(&obs, None).is_none());
    obs.owned.last_mut().unwrap().is_complete = true;

    let tank = expansion_security::tank_staging_center(&obs, None).unwrap();
    let enemy = obs
        .players
        .iter()
        .find(|player| obs.is_enemy_player(player.id))
        .unwrap();
    let enemy_center = tile_center(enemy.start_tile, obs.map.tile_size);
    let direction = normalized_direction(center, enemy_center).unwrap();
    let projection =
        |point: (f32, f32)| (point.0 - center.0) * direction.0 + (point.1 - center.1) * direction.1;
    let mut security = expansion_security::ExpansionSecurity::default();
    security.site = Some(site);
    let rifles = expansion_security::positions(&obs, None, &security);

    assert_eq!(rifles.len(), 2);
    assert!(projection(tank) > 0.0);
    assert!(rifles
        .iter()
        .all(|point| projection(*point) > projection(tank)));
    let stats = config::building_stats(EntityKind::ResourceDepot).unwrap();
    let rect = (
        center.0 - stats.foot_w as f32 * ts * 0.5,
        center.1 - stats.foot_h as f32 * ts * 0.5,
        center.0 + stats.foot_w as f32 * ts * 0.5,
        center.1 + stats.foot_h as f32 * ts * 0.5,
    );
    assert!(!crate::sdk::unit_circle_touches_rect(
        tank,
        rts_rules::balance::unit_placement_radius(EntityKind::Tank),
        rect,
    ));
}

#[test]
fn surplus_tank_moves_forward_while_reserved_tank_stays_home() {
    let mut obs = security_observation();
    let ts = obs.map.tile_size as f32;
    let center = building_center((23, 23), EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    let depot = building_at(50, EntityKind::ResourceDepot, Some(0), center.0, center.1);
    obs.owned.push(depot);
    obs.owned
        .push(combat_at(51, EntityKind::Tank, 10.0 * ts, 10.0 * ts));
    obs.owned
        .push(combat_at(52, EntityKind::Tank, 11.0 * ts, 10.0 * ts));
    let target = expansion_security::tank_staging_center(&obs, None).unwrap();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment_wave_launched = true;
    memory.home_defensive_tank = Some(51);
    memory.home_defensive_tank_assigned_once = true;

    let decision = decide(&obs, &JEFFS_AI, &mut memory);
    assert!(decision.commands.iter().any(|command| matches!(
        command,
        Command::Move { units, x, y, .. }
            if units.contains(&52)
                && geometry::dist2(*x, *y, target.0, target.1) < 1.0
    )));
    assert!(!decision.commands.iter().any(|command| matches!(
        command,
        Command::Move { units, x, y, .. }
            if units.contains(&51)
                && geometry::dist2(*x, *y, target.0, target.1) < 1.0
    )));
}

#[test]
#[cfg(not(debug_assertions))]
fn expansion_security_live_opening_reaches_second_base() {
    if crate::skip_unless_full_ai("expansion_security_live_opening_reaches_second_base") {
        return;
    }
    use crate::live::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, PlayerInit};
    let players: Vec<_> = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        })
        .collect();
    for name in ["The River", "Schone Tage", "1v1", "Crossroads"] {
        let map = Map::load_for_players(name, &[(1, 1), (2, 2)], 0x1234_5678).unwrap();
        let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
            &players,
            0x1234_5678,
            map,
            Map::metadata_for_name(name).unwrap(),
        );
        let start = game.start_payload();
        let mut controllers = [AiController::with_profile_id(1, JEFFS_AI_ID)];
        let mut expanded = false;
        let mut last = None;
        for tick in 0..15000 {
            CanonicalAiTickDriver::run(
                &mut game,
                &mut controllers,
                AiAlivePolicy::StartingPrimaryBase,
            );
            game.tick();
            if tick % 30 != 0 {
                continue;
            }
            let obs =
                AiObservation::from_snapshot_with_alive(&start, &game.snapshot_for(1), 1, [], None)
                    .unwrap();
            if obs
                .owned
                .iter()
                .filter(|unit| unit.kind == EntityKind::ResourceDepot)
                .count()
                >= 2
            {
                expanded = true;
                break;
            }
            last = Some(obs);
        }
        if !expanded {
            let obs = last.unwrap();
            let site = expansion::expansion_resource_depot_site(
                &obs,
                JEFFS_AI.expansion.unwrap(),
                EntityKind::ResourceDepot,
                JEFFS_AI.id,
                &mut |_, _, _| true,
            );
            let blocked = site.map(|site| {
                let stats = config::building_stats(EntityKind::ResourceDepot).unwrap();
                let ts = obs.map.tile_size as f32;
                let rect = (
                    site.0 as f32 * ts,
                    site.1 as f32 * ts,
                    site.0.saturating_add(stats.foot_w) as f32 * ts,
                    site.1.saturating_add(stats.foot_h) as f32 * ts,
                );
                obs.owned
                    .iter()
                    .filter(|unit| {
                        unit.kind.is_unit()
                            && crate::sdk::unit_circle_touches_rect(
                                (unit.x, unit.y),
                                rts_rules::balance::unit_placement_radius(unit.kind),
                                rect,
                            )
                    })
                    .map(|unit| (unit.id, unit.kind, unit.x, unit.y))
                    .collect::<Vec<_>>()
            });
            let units: Vec<_> = obs
                .owned
                .iter()
                .filter(|unit| !matches!(unit.kind, EntityKind::SteelMine | EntityKind::PumpJack))
                .collect();
            let security = &controllers[0].decision_memory().expansion_security;
            let center = security.site.and_then(|site| {
                building_center(site, EntityKind::ResourceDepot, obs.map.tile_size)
            });
            let party: Vec<_> = security
                .riflemen
                .iter()
                .filter_map(|id| {
                    let unit = obs.owned.iter().find(|unit| unit.id == *id)?;
                    Some((
                        unit.id,
                        unit.x,
                        unit.y,
                        unit.state,
                        center.map(|center| {
                            geometry::dist2(unit.x, unit.y, center.0, center.1).sqrt()
                                / obs.map.tile_size as f32
                        }),
                    ))
                })
                .collect();
            panic!(
                "{name}: expansion stalled: site={site:?} blockers={blocked:?} party={party:?} posts={:?} enemies={:?} units={units:?}, trace={:?}",
                expansion_security::positions(&obs, None, security),
                obs.visible_enemies,
                controllers[0].latest_decision_trace()
            );
        }
    }
}
