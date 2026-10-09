use super::*;
use crate::ai_core::profiles::{JEFFS_AI, JEFFS_AI_PRE_TANK_MICRO};

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
    // The two opening Machine Gunners hold home; the natural waits for them.
    units.extend((20..22).map(|id| combat_at(id, EntityKind::MachineGunner, 10.0 * ts, 10.0 * ts)));
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
        None,
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
        None,
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
fn successful_expansion_attempt_does_not_time_out_after_later_depot_loss() {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    memory.expansion_security.note_build_attempt(obs.tick, 1);
    let site = memory.expansion_security.site.unwrap();
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    obs.owned.push(building_at(
        99,
        EntityKind::ResourceDepot,
        Some(0),
        center.0,
        center.1,
    ));
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );

    obs.owned.retain(|entity| entity.id != 99);
    obs.tick += config::TICK_HZ * 3;
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );

    assert_eq!(memory.expansion_security.site, Some(site));
    assert_eq!(memory.expansion_security.retry_builder(), None);
}

#[test]
fn jeff_rejects_depot_sites_whose_footprint_touches_a_resource_body() {
    use crate::ai_core::decision::jeff::resource_body_blocks_site;
    // Classic's natural had a Steel center 0.2px below the Depot footprint: outside the
    // footprint tiles, but the server rejects any footprint the node body touches.
    let ts = config::TILE_SIZE as f32;
    let (tx, ty) = (40, 40);
    let mut obs = security_observation();
    obs.resources.clear();
    obs.resources.push(resource(
        400,
        EntityKind::Steel,
        tx as f32 * ts + 6.0,
        (ty + 3) as f32 * ts + 0.2,
    ));
    assert!(resource_body_blocks_site(
        &obs,
        EntityKind::ResourceDepot,
        tx,
        ty
    ));
    assert!(!resource_body_blocks_site(
        &obs,
        EntityKind::SteelMine,
        tx,
        ty
    ));

    obs.resources.clear();
    obs.resources.push(resource(
        401,
        EntityKind::Steel,
        tx as f32 * ts - ts * 0.5 - 1.0,
        (ty + 1) as f32 * ts,
    ));
    assert!(!resource_body_blocks_site(
        &obs,
        EntityKind::ResourceDepot,
        tx,
        ty
    ));
}

#[test]
fn walking_builder_keeps_the_site_until_the_order_is_dropped() {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let prepare = |obs: &AiObservation, memory: &mut AiDecisionMemory| {
        expansion_security::prepare(
            obs,
            &AiFacts::from_observation(obs),
            &JEFFS_AI,
            memory,
            None,
            &mut |_, _, _| true,
        );
    };
    prepare(&obs, &mut memory);
    let site = memory.expansion_security.site.unwrap();
    memory.expansion_security.note_build_attempt(obs.tick, 1);

    obs.pending_builds = vec![crate::ai_core::observation::AiBuildIntent::to_site(
        1,
        EntityKind::ResourceDepot,
        site.0,
        site.1,
    )];
    obs.tick += config::TICK_HZ * 10;
    prepare(&obs, &mut memory);
    assert_eq!(memory.expansion_security.site, Some(site));
    assert_eq!(memory.expansion_security.retry_builder(), None);

    // The first dropped order keeps the site: the builder is usually just shot on the way.
    obs.pending_builds.clear();
    obs.tick += 1;
    prepare(&obs, &mut memory);
    assert_eq!(memory.expansion_security.site, Some(site));
    assert_eq!(memory.expansion_security.retry_builder(), Some(1));

    // A second drop at the same site gives it up.
    memory.expansion_security.note_build_attempt(obs.tick, 1);
    obs.tick += config::TICK_HZ * 10;
    prepare(&obs, &mut memory);
    assert_ne!(memory.expansion_security.site, Some(site));
    assert_eq!(memory.expansion_security.retry_builder(), Some(1));
}

#[test]
fn expansion_reserve_expires_and_rearms_after_the_expansion_cycle() {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    let reserve = |obs: &AiObservation, memory: &mut AiDecisionMemory| {
        expansion_security::reserve_expansion(
            obs,
            &AiFacts::from_observation(obs),
            &JEFFS_AI,
            memory,
        )
    };
    assert!(reserve(&obs, &mut memory));
    obs.tick += config::TICK_HZ * 60 - 1;
    assert!(reserve(&obs, &mut memory));
    obs.tick += 1;
    assert!(!reserve(&obs, &mut memory));

    let ts = config::TILE_SIZE as f32;
    obs.owned.push(building_at(
        98,
        EntityKind::ResourceDepot,
        Some(0),
        40.5 * ts,
        40.5 * ts,
    ));
    assert!(!reserve(&obs, &mut memory));
    obs.owned.retain(|entity| entity.id != 98);
    assert!(reserve(&obs, &mut memory));
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
        None,
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
        expansion_security::update_and_stage(obs, None, memory, &mut actions, false)
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
fn guards_stuck_just_short_of_their_posts_secure_the_site_after_a_longer_dwell() {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    let points = expansion_security::positions(&obs, None, &memory.expansion_security);
    let step = |obs: &AiObservation, memory: &mut AiDecisionMemory| {
        let facts = AiFacts::from_observation(obs);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
        expansion_security::update_and_stage(obs, None, memory, &mut actions, false)
    };
    // Each guard stands a tile short of its post, as one pressed against steel patches does.
    let site = memory.expansion_security.site.unwrap();
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    for (id, point) in memory.expansion_security.riflemen.iter().zip(&points) {
        let away = geometry::normalized_direction(center, *point).unwrap();
        let unit = obs.owned.iter_mut().find(|unit| unit.id == *id).unwrap();
        unit.x = point.0 + away.0 * config::TILE_SIZE as f32;
        unit.y = point.1 + away.1 * config::TILE_SIZE as f32;
    }
    assert!(!step(&obs, &mut memory));
    obs.tick += config::TICK_HZ * 3;
    assert!(
        !step(&obs, &mut memory),
        "short of the post the usual dwell is not enough"
    );
    obs.tick += config::TICK_HZ * 12;
    assert!(
        step(&obs, &mut memory),
        "a longer dwell near the posts secures the site"
    );

    // Contact near the site restarts the wait.
    let mut enemy = combat_at(900, EntityKind::Rifleman, points[0].0, points[0].1);
    enemy.owner = 2;
    obs.visible_enemies.push(enemy);
    assert!(!step(&obs, &mut memory));
    obs.visible_enemies.clear();
    obs.tick += config::TICK_HZ * 3;
    assert!(!step(&obs, &mut memory));

    // Farther off than that, the guards are not in place at all.
    for (id, point) in memory.expansion_security.riflemen.iter().zip(&points) {
        let away = geometry::normalized_direction(center, *point).unwrap();
        let unit = obs.owned.iter_mut().find(|unit| unit.id == *id).unwrap();
        unit.x = point.0 + away.0 * 2.0 * config::TILE_SIZE as f32;
        unit.y = point.1 + away.1 * 2.0 * config::TILE_SIZE as f32;
    }
    assert!(!step(&obs, &mut memory));
    obs.tick += config::TICK_HZ * 30;
    assert!(!step(&obs, &mut memory));
}

/// The guards on a contested natural are left to local defense: the frozen Jeff sends them at the
/// raider, and the live Jeff's raid response takes them in.
#[test]
fn contested_expansion_guards_are_available_to_local_defense() {
    for profile in [&JEFFS_AI_PRE_TANK_MICRO, &JEFFS_AI] {
        contested_expansion_guards_answer_the_raid(profile);
    }
}

fn contested_expansion_guards_answer_the_raid(profile: &'static AiProfile) {
    let mut obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(profile);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        profile,
        &mut memory,
        None,
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

    let decision = decide(&obs, profile, &mut memory);
    if profile.id == JEFFS_AI_ID {
        let answering: BTreeSet<u32> = memory.raid_defense.reserved().collect();
        assert!(
            memory
                .expansion_security
                .riflemen
                .iter()
                .all(|id| answering.contains(id)),
            "{:?}",
            decision.commands
        );
        return;
    }
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
            None,
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
    for name in ["Schone Tage", "The River", "Classic", "Crossroads"] {
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
                Some(&analysis),
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
fn crossroads_natural_is_the_base_behind_the_main_not_the_one_toward_the_enemy() {
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
        let obs = AiObservation::from_snapshot_with_alive(
            &start,
            &game.snapshot_for(player),
            player,
            [],
            None,
        )
        .unwrap();
        let enemy = if player == 1 { 2 } else { 1 };
        let policy = JEFFS_AI.expansion.unwrap();
        let natural = expansion::expansion_resource_depot_site(
            &obs,
            policy,
            EntityKind::ResourceDepot,
            JEFFS_AI.id,
            Some(&analysis),
            &mut |_, _, _| true,
        )
        .unwrap();
        // The closest base in a straight line sits out on the enemy's side of the main.
        let straight_line = expansion::generic_expansion_depot_site(
            &obs,
            policy,
            EntityKind::ResourceDepot,
            &expansion::expansion_candidate_resources(&obs),
            &mut |_, _, _| true,
        )
        .unwrap();
        let walk = |site: (u32, u32), from: u32| {
            let center =
                building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
            let ts = obs.map.tile_size as f32;
            analysis
                .ground_distance_from_start(from, ((center.0 / ts) as u32, (center.1 / ts) as u32))
                .unwrap()
        };
        assert!(
            walk(natural, enemy) > walk(straight_line, enemy) + 30.0,
            "player {player}: natural {natural:?} vs {straight_line:?}"
        );
        assert!(
            walk(natural, player) <= walk(straight_line, player),
            "player {player}: natural {natural:?} vs {straight_line:?}"
        );
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
            Some(&analysis),
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
    // A push of two Tanks and a Scout Car is out on the map.
    for (id, kind) in [
        (53, EntityKind::Tank),
        (54, EntityKind::Tank),
        (55, EntityKind::ScoutCar),
    ] {
        obs.owned
            .push(combat_at(id, kind, (40 + id) as f32 * ts, 40.0 * ts));
    }
    let target = expansion_security::tank_staging_center(&obs, None).unwrap();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.wave_launched = true;
    memory.containment.active_tanks = [53, 54].into_iter().collect();
    memory.containment.active_scout = Some(55);
    memory.containment.launch_tanks = 2;
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
    for name in ["The River", "Schone Tage", "Classic", "Crossroads"] {
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
                Some(&AiMapAnalysis::analyze(&start)),
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

/// Secure the natural in the standard fixture and return it with the site.
fn secured_natural() -> (AiObservation, AiDecisionMemory, (u32, u32)) {
    let obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    let site = memory.expansion_security.site.expect("natural site");
    (obs, memory, site)
}

#[test]
fn the_natural_is_only_ordered_with_its_full_cost_banked_and_a_quiet_route() {
    let (mut obs, memory, site) = secured_natural();
    let ready =
        |obs: &AiObservation| expansion_security::natural_attempt_ready(obs, None, &memory, true);
    obs.economy.steel = 450;
    obs.economy.oil = 99;
    assert!(!ready(&obs), "one Oil short");
    obs.economy.oil = 100;
    assert!(ready(&obs));
    assert!(
        !expansion_security::natural_attempt_ready(&obs, None, &memory, false),
        "unsecured"
    );

    // An enemy next to the builder keeps it home.
    let builder = obs
        .owned
        .iter()
        .find(|unit| unit.kind == EntityKind::Worker)
        .unwrap();
    let mut raider = combat_at(900, EntityKind::Rifleman, builder.x + 64.0, builder.y);
    raider.owner = 2;
    obs.visible_enemies.push(raider);
    assert!(!ready(&obs));

    // So does an order already on its way to the site.
    obs.visible_enemies.clear();
    obs.pending_builds = vec![crate::ai_core::observation::AiBuildIntent::to_site(
        1,
        EntityKind::ResourceDepot,
        site.0,
        site.1,
    )];
    assert!(!ready(&obs));
}

#[test]
fn a_dropped_natural_order_waits_out_the_cooldown_before_the_next_try() {
    let (mut obs, mut memory, site) = secured_natural();
    obs.economy.steel = 1000;
    obs.economy.oil = 1000;
    memory.expansion_security.note_build_attempt(obs.tick, 1);
    obs.tick += config::TICK_HZ * 4;
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    assert_eq!(
        memory.expansion_security.site,
        Some(site),
        "kept after one drop"
    );
    assert!(!expansion_security::natural_attempt_ready(
        &obs, None, &memory, true
    ));
    obs.tick += config::TICK_HZ * 20;
    assert!(expansion_security::natural_attempt_ready(
        &obs, None, &memory, true
    ));
}

#[test]
fn a_pending_natural_order_keeps_its_oil_from_the_next_tank() {
    let (mut obs, mut memory, site) = secured_natural();
    // Start the natural's bounded reserve, then let it expire, so only the pending order can be
    // what holds the Depot's cost back.
    decide(&obs, &JEFFS_AI, &mut memory);
    obs.tick += config::TICK_HZ * 61;
    let factory = obs
        .owned
        .iter_mut()
        .find(|unit| unit.kind == EntityKind::Factory)
        .unwrap();
    factory.production_kind = None;
    factory.production_queue_len = Some(0);
    // Enough for a Tank (425/175), but not once the Depot's 450/100 is set aside.
    obs.economy.steel = 700;
    obs.economy.oil = 250;
    let tank = AiIntent::Train {
        kind: EntityKind::Tank,
    };
    let mut control = memory.clone();
    assert!(
        decide(&obs, &JEFFS_AI, &mut control)
            .intents
            .contains(&tank),
        "with nothing pending the Tank is trained"
    );
    obs.pending_builds = vec![crate::ai_core::observation::AiBuildIntent::to_site(
        1,
        EntityKind::ResourceDepot,
        site.0,
        site.1,
    )];
    assert!(!decide(&obs, &JEFFS_AI, &mut memory).intents.contains(&tank));
}

/// The site of the natural, with Tanks at `(id, x tile, y tile)` and an enemy unit of `kind` on it.
fn natural_held_by(
    kind: EntityKind,
    tanks: &[(u32, f32, f32)],
) -> (AiObservation, AiDecisionMemory) {
    let ts = config::TILE_SIZE as f32;
    let mut obs = security_observation();
    for (id, x, y) in tanks {
        obs.owned
            .push(combat_at(*id, EntityKind::Tank, x * ts, y * ts));
    }
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    let site = memory.expansion_security.site.unwrap();
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    let mut enemy = combat_at(900, kind, center.0 + 2.0 * ts, center.1);
    enemy.owner = 2;
    obs.visible_enemies.push(enemy);
    (obs, memory)
}

#[test]
fn enemy_infantry_on_the_natural_is_cleared_by_the_two_nearest_tanks() {
    let (obs, mut memory) = natural_held_by(
        EntityKind::Rifleman,
        &[(60, 20.0, 20.0), (61, 18.0, 18.0), (62, 9.0, 9.0)],
    );
    let decision = decide(&obs, &JEFFS_AI, &mut memory);
    assert_eq!(memory.expansion_security.contested_tick, Some(obs.tick));
    assert_eq!(memory.expansion_security.clearers, vec![60, 61]);
    for tank in [60, 61] {
        assert!(
            decision.commands.iter().any(|command| matches!(
                command,
                Command::AttackMove { units, .. } if units == &[tank]
            )),
            "Tank {tank} is sent: {:?}",
            decision.commands
        );
    }
    // No Depot money is held back while the site is held.
    assert!(!expansion_security::reserve_expansion(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
    ));
}

#[test]
fn enemy_armor_on_the_natural_is_left_to_the_tank_fight() {
    let (obs, mut memory) = natural_held_by(EntityKind::Tank, &[(60, 20.0, 20.0)]);
    decide(&obs, &JEFFS_AI, &mut memory);
    assert!(memory.expansion_security.clearers.is_empty());
}
