use super::fixtures::*;
use super::*;

fn mortar_launch_impact(
    events: &[(u32, Vec<Event>)],
    player_id: u32,
    mortar: u32,
) -> Option<(f32, f32)> {
    events
        .iter()
        .find(|(id, _)| *id == player_id)
        .and_then(|(_, player_events)| {
            player_events.iter().find_map(|event| match event {
                Event::MortarLaunch {
                    from, to_x, to_y, ..
                } if *from == mortar => Some((*to_x, *to_y)),
                _ => None,
            })
        })
}

fn smoke_projection_fixture() -> (Game, u32, u32, u32, (f32, f32)) {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = Game::new_for_replay(&players, 0x5EED_5000);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    let observer_pos = game.state.map.tile_center(4, 4);
    let smoke_pos = game.state.map.tile_center(7, 4);
    let friendly_pos = game.state.map.tile_center(8, 4);
    let observer = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, observer_pos.0, observer_pos.1)
        .expect("observer should spawn");
    let friendly = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, friendly_pos.0, friendly_pos.1)
        .expect("friendly should spawn");
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, smoke_pos.0, smoke_pos.1)
        .expect("enemy should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.state
        .smokes
        .spawn(
            smoke_pos.0,
            smoke_pos.1,
            config::SMOKE_CLOUD_RADIUS_TILES,
            config::SMOKE_CLOUD_DURATION_TICKS,
            game.state.tick,
        )
        .expect("smoke should spawn");
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute_with_smoke(
        &ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );
    (game, observer, friendly, enemy, smoke_pos)
}

fn team_fog_fixture() -> (Game, u32, u32, u32, (f32, f32)) {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#bbb".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let p1_base = game.state.map.tile_center(2, 2);
    let p2_base = game.state.map.tile_center(5, 2);
    let p3_base = game.state.map.tile_center(55, 55);
    game.state
        .entities
        .spawn_building(1, EntityKind::CityCentre, p1_base.0, p1_base.1, true)
        .expect("p1 city centre should spawn");
    game.state
        .entities
        .spawn_building(2, EntityKind::CityCentre, p2_base.0, p2_base.1, true)
        .expect("p2 city centre should spawn");
    game.state
        .entities
        .spawn_building(3, EntityKind::CityCentre, p3_base.0, p3_base.1, true)
        .expect("p3 city centre should spawn");

    let spotter_pos = game.state.map.tile_center(28, 30);
    let enemy_pos = game.state.map.tile_center(30, 30);
    let hidden_enemy_pos = game.state.map.tile_center(55, 50);
    let spotter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Worker, spotter_pos.0, spotter_pos.1)
        .expect("ally spotter should spawn");
    let visible_enemy = game
        .state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("visible enemy should spawn");
    let hidden_enemy = game
        .state
        .entities
        .spawn_unit(
            3,
            EntityKind::Rifleman,
            hidden_enemy_pos.0,
            hidden_enemy_pos.1,
        )
        .expect("hidden enemy should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute_with_smoke(
        &ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );
    (game, spotter, visible_enemy, hidden_enemy, enemy_pos)
}

#[test]
fn snapshot_shares_living_teammate_current_vision() {
    let (game, _spotter, visible_enemy, hidden_enemy, enemy_pos) = team_fog_fixture();
    assert!(
        !game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
        "fixture should keep the enemy outside player 1's own raw fog"
    );
    assert!(
        game.state.fog.is_visible_world(2, enemy_pos.0, enemy_pos.1),
        "fixture should put the enemy inside player 2's raw fog"
    );

    let snapshot = game.snapshot_for(1);

    assert!(
        snapshot
            .entities
            .iter()
            .any(|entity| entity.id == visible_enemy),
        "ally current sight should reveal the enemy in player 1's snapshot"
    );
    assert!(
        snapshot
            .entities
            .iter()
            .all(|entity| entity.id != hidden_enemy),
        "enemies outside every living teammate's current sight should stay hidden"
    );
    let visible_index = ((enemy_pos.1 / config::TILE_SIZE as f32).floor() as u32
        * game.state.map.size
        + (enemy_pos.0 / config::TILE_SIZE as f32).floor() as u32) as usize;
    assert_eq!(
        snapshot.visible_tiles[visible_index], 1,
        "visibleTiles should include the living teammate's current sight"
    );
    assert_eq!(snapshot.player_resources.len(), 0);
    assert!(
        snapshot.steel
            == game
                .state
                .players
                .iter()
                .find(|player| player.id == 1)
                .expect("player 1 should exist")
                .steel,
        "recipient economy remains local-player-only"
    );
}

#[test]
fn defeated_teammate_no_longer_contributes_current_vision() {
    let (mut game, _spotter, visible_enemy, _hidden_enemy, _enemy_pos) = team_fog_fixture();

    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .any(|entity| entity.id == visible_enemy),
        "precondition: teammate sight reveals the enemy"
    );

    game.eliminate(2);

    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .all(|entity| entity.id != visible_enemy),
        "eliminated teammate sight should stop contributing to team current vision"
    );
    assert!(
        game.snapshot_for(2)
            .entities
            .iter()
            .all(|entity| entity.id != visible_enemy),
        "defeated player should receive surviving teammate vision but not their own stale vision"
    );
}

#[test]
fn team_current_vision_keeps_smoke_blocking() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#bbb".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let p1_base = game.state.map.tile_center(2, 2);
    let p2_base = game.state.map.tile_center(4, 2);
    let p3_base = game.state.map.tile_center(50, 50);
    game.state
        .entities
        .spawn_building(1, EntityKind::CityCentre, p1_base.0, p1_base.1, true)
        .expect("p1 city centre should spawn");
    game.state
        .entities
        .spawn_building(2, EntityKind::CityCentre, p2_base.0, p2_base.1, true)
        .expect("p2 city centre should spawn");
    game.state
        .entities
        .spawn_building(3, EntityKind::CityCentre, p3_base.0, p3_base.1, true)
        .expect("p3 city centre should spawn");
    let spotter_pos = game.state.map.tile_center(4, 4);
    let smoke_pos = game.state.map.tile_center(7, 4);
    let enemy_pos = game.state.map.tile_center(7, 4);
    game.state
        .entities
        .spawn_unit(2, EntityKind::Worker, spotter_pos.0, spotter_pos.1)
        .expect("ally spotter should spawn");
    let enemy = game
        .state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    game.state
        .smokes
        .spawn(
            smoke_pos.0,
            smoke_pos.1,
            config::SMOKE_CLOUD_RADIUS_TILES,
            config::SMOKE_CLOUD_DURATION_TICKS,
            game.state.tick,
        )
        .expect("smoke should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute_with_smoke(
        &ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );

    let snapshot = game.snapshot_for(1);

    assert!(
        snapshot.entities.iter().all(|entity| entity.id != enemy),
        "team current vision must not reveal enemies hidden inside smoke"
    );
}

#[test]
fn manual_mortar_fire_impacts_without_toast_notice() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(14, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    game.state
        .entities
        .get_mut(mortar)
        .expect("mortar should exist")
        .set_weapon_setup(WeaponSetup::Deployed);
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    let accepted_events = game.tick();
    let impact_pos = mortar_launch_impact(&accepted_events, 1, mortar)
        .expect("owner should receive mortar launch impact");
    let owner_events = accepted_events
        .iter()
        .find(|(player_id, _)| *player_id == 1)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        owner_events.iter().any(|event| matches!(
            event,
            Event::MortarLaunch { from, to_x, to_y, delay_ticks, .. }
                if *from == mortar
                    && (*to_x - impact_pos.0).abs() < 0.001
                    && (*to_y - impact_pos.1).abs() < 0.001
                    && *delay_ticks == config::MORTAR_SHELL_DELAY_TICKS
        )),
        "accepted mortar command should emit a launch marker with impact timing: {owner_events:?}"
    );
    let enemy_events = accepted_events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        enemy_events
            .iter()
            .all(|event| !matches!(event, Event::MortarLaunch { .. })),
        "manual mortar fire should not reveal launch preview markers to enemies: {enemy_events:?}"
    );
    assert!(
        owner_events
            .iter()
            .all(|event| !matches!(event, Event::Notice { msg, .. } if msg == "Mortar fire")),
        "accepted mortar command should use impact feedback instead of a toast notice: {owner_events:?}"
    );
    game.state
        .entities
        .get_mut(target)
        .expect("target should still exist")
        .set_position(impact_pos.0, impact_pos.1);
    game.state
        .entities
        .get_mut(target)
        .expect("target should still exist")
        .hold_position();
    let hp_before_impact = game
        .state
        .entities
        .get(target)
        .expect("target should still exist")
        .hp;

    let mut impact_events = Vec::new();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        impact_events = game.tick();
    }

    assert!(
        game.state
            .entities
            .get(target)
            .is_none_or(|target_after| target_after.hp < hp_before_impact),
        "manual mortar fire should damage or kill units at the targeted impact point"
    );
    let owner_events = impact_events
        .iter()
        .find(|(player_id, _)| *player_id == 1)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        owner_events
            .iter()
            .any(|event| matches!(event, Event::MortarImpact { x, y, .. }
                if (*x - impact_pos.0).abs() < 0.001 && (*y - impact_pos.1).abs() < 0.001)),
        "delayed mortar impact should emit a visible impact marker: {owner_events:?}"
    );
}

#[test]
fn set_autocast_command_enables_mortar_autocast_from_default_off() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");

    assert_eq!(
        game.state
            .entities
            .get(mortar)
            .expect("mortar should exist")
            .autocast_enabled(ability::AbilityKind::MortarFire),
        Some(false),
        "mortar autocast should start disabled"
    );
    game.state.players[0]
        .upgrades
        .insert(upgrade::UpgradeKind::MortarAutocast);

    game.enqueue(
        1,
        Command::SetAutocast {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            enabled: true,
        },
    );
    for _ in 0..10 {
        game.tick();
    }

    assert_eq!(
        game.state
            .entities
            .get(mortar)
            .expect("mortar should exist")
            .autocast_enabled(ability::AbilityKind::MortarFire),
        Some(true),
        "setAutocast should enable mortar autofire"
    );
}

#[test]
fn visible_autocast_mortar_launch_is_sent_to_enemy() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(14, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    {
        let mortar_entity = game
            .state
            .entities
            .get_mut(mortar)
            .expect("mortar should exist");
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
        mortar_entity.set_emplacement_facing(Some(0.0));
        mortar_entity.set_autocast_enabled(ability::AbilityKind::MortarFire, true);
    }
    game.state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    game.state.players[0]
        .upgrades
        .insert(upgrade::UpgradeKind::MortarAutocast);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    assert!(
        game.state
            .fog
            .is_visible_world(2, mortar_pos.0, mortar_pos.1),
        "test setup requires the enemy to see the autocasting mortar"
    );

    let events = game.tick();
    let enemy_events = events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);

    assert!(
        enemy_events.iter().any(|event| matches!(
            event,
            Event::MortarLaunch { from, delay_ticks, .. }
                if *from == mortar
                    && *delay_ticks == config::MORTAR_SHELL_DELAY_TICKS
        )),
        "visible autocast mortar fire should show enemy launch preview markers: {enemy_events:?}"
    );
}

#[test]
fn manual_mortar_fire_impacts_after_shooter_dies_before_impact() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(14, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    game.state
        .entities
        .get_mut(mortar)
        .expect("mortar should exist")
        .set_weapon_setup(WeaponSetup::Deployed);
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, target_pos.0, target_pos.1)
        .expect("target should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    let launch_events = game.tick();
    let impact_pos = mortar_launch_impact(&launch_events, 1, mortar)
        .expect("owner should receive mortar launch impact");
    assert!(
        launch_events.iter().any(|(player_id, events)| {
            *player_id == 1
                && events.iter().any(
                    |event| matches!(event, Event::MortarLaunch { from, .. } if *from == mortar),
                )
        }),
        "accepted mortar command should emit a launch before shooter death: {launch_events:?}"
    );

    game.state
        .entities
        .get_mut(target)
        .expect("target should still exist")
        .set_position(impact_pos.0, impact_pos.1);
    let hp_before_impact = game
        .state
        .entities
        .get(target)
        .expect("target should still exist")
        .hp;
    game.state
        .entities
        .get_mut(mortar)
        .expect("mortar should still exist after launch")
        .apply_damage(u32::MAX, None);
    game.tick();
    assert!(
        !game.state.entities.contains(mortar),
        "mortar should be removed before the delayed shell impact"
    );

    let mut impact_seen = false;
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        let events = game.tick();
        impact_seen |= events.iter().any(|(player_id, events)| {
            *player_id == 1
                && events.iter().any(|event| {
                    matches!(
                        event,
                        Event::MortarImpact { x, y, .. }
                            if (*x - impact_pos.0).abs() < 0.001
                                && (*y - impact_pos.1).abs() < 0.001
                    )
                })
        });
    }

    let hp_after_impact = game
        .state
        .entities
        .get(target)
        .expect("tank should survive")
        .hp;
    assert!(
        hp_after_impact < hp_before_impact,
        "mortar shell should still damage after shooter death, before={hp_before_impact}, after={hp_after_impact}"
    );
    assert!(
        impact_seen,
        "mortar shell should emit an impact marker even after shooter death"
    );
}

#[test]
fn manual_mortar_fire_turns_briefly_before_launching() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(8, 2);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_facing(0.0);
        mortar_entity.set_weapon_facing(0.0);
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    game.state
        .entities
        .get_mut(target)
        .expect("target should exist")
        .hold_position();
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );

    game.tick();
    let mortar_entity = game
        .state
        .entities
        .get(mortar)
        .expect("mortar should exist");
    assert_eq!(
        mortar_entity.ability_cooldown_ticks(ability::AbilityKind::MortarFire),
        0,
        "manual mortar fire should wait for the brief facing slew before launching"
    );
    assert!(
        (mortar_entity.facing() + mortar::TURN_RATE_RAD_PER_TICK).abs() <= 0.001,
        "mortar should rotate one fast step toward the manual target, got {:.4}",
        mortar_entity.facing()
    );

    let mut impact_pos = None;
    for _ in 0..2 {
        let events = game.tick();
        impact_pos = impact_pos.or_else(|| mortar_launch_impact(&events, 1, mortar));
    }
    let impact_pos = impact_pos.expect("owner should receive mortar launch impact");
    let mortar_entity = game
        .state
        .entities
        .get(mortar)
        .expect("mortar should exist");
    assert!(
        mortar_entity.ability_cooldown_ticks(ability::AbilityKind::MortarFire) > 0,
        "manual mortar fire should launch once the fast facing slew completes"
    );
    assert!(
        (mortar_entity.facing() + std::f32::consts::FRAC_PI_2).abs()
            <= mortar::FIRE_TOLERANCE_RAD + 0.001,
        "mortar should finish facing the manual target, got {:.4}",
        mortar_entity.facing()
    );
    let hp_before_impact = game
        .state
        .entities
        .get(target)
        .expect("target should still exist")
        .hp;
    game.state
        .entities
        .get_mut(target)
        .expect("target should still exist")
        .set_position(impact_pos.0, impact_pos.1);

    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    assert!(
        game.state
            .entities
            .get(target)
            .is_none_or(|target_after| target_after.hp < hp_before_impact),
        "manual mortar fire should damage or kill units after the delayed impact"
    );
}

#[test]
fn manual_mortar_fire_damages_friendly_units_at_enemy_rate() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(14, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let friendly = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MachineGunner, target_pos.0, target_pos.1)
        .expect("friendly should spawn");
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, target_pos.0, target_pos.1)
        .expect("enemy should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    let launch_events = game.tick();
    let impact_pos = mortar_launch_impact(&launch_events, 1, mortar)
        .expect("owner should receive mortar launch impact");
    game.state
        .entities
        .get_mut(friendly)
        .expect("friendly should still exist")
        .set_position(impact_pos.0, impact_pos.1);
    game.state
        .entities
        .get_mut(enemy)
        .expect("enemy should still exist")
        .set_position(impact_pos.0, impact_pos.1);
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    assert!(
        !game.state.entities.contains(friendly),
        "friendly machine gunner should take the same lethal inner-radius hit as an enemy"
    );
    assert!(
        !game.state.entities.contains(enemy),
        "enemy machine gunner should take the matching lethal inner-radius hit"
    );
}

#[test]
fn manual_mortar_fire_has_no_armor_piercing_in_either_splash_radius() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(14, 8);
    let outer_pos = game.state.map.tile_center(15, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let armored_inner = game
        .state
        .entities
        .spawn_building(2, EntityKind::TankTrap, target_pos.0, target_pos.1, true)
        .expect("armored target should spawn");
    let armored_outer = game
        .state
        .entities
        .spawn_building(2, EntityKind::TankTrap, outer_pos.0, outer_pos.1, true)
        .expect("outer armored target should spawn");
    let armored_inner_hp_before = game
        .state
        .entities
        .get(armored_inner)
        .expect("armored target exists")
        .hp;
    let armored_outer_hp_before = game
        .state
        .entities
        .get(armored_outer)
        .expect("outer armored target exists")
        .hp;
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    let launch_events = game.tick();
    let impact_pos = mortar_launch_impact(&launch_events, 1, mortar)
        .expect("owner should receive mortar launch impact");
    game.state
        .entities
        .get_mut(armored_inner)
        .expect("armored target should exist")
        .set_position(impact_pos.0, impact_pos.1);
    game.state
        .entities
        .get_mut(armored_outer)
        .expect("outer armored target should exist")
        .set_position(impact_pos.0, impact_pos.1 + config::TILE_SIZE as f32 * 1.25);
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    let armored_inner_hp_after = game
        .state
        .entities
        .get(armored_inner)
        .expect("armored target should survive")
        .hp;
    let armored_outer_hp_after = game
        .state
        .entities
        .get(armored_outer)
        .expect("outer armored target should survive")
        .hp;
    assert_eq!(
        armored_inner_hp_before - armored_inner_hp_after,
        config::MORTAR_INNER_DAMAGE / 4,
        "inner mortar splash should receive the standard non-piercing armor reduction"
    );
    assert_eq!(
        armored_outer_hp_before - armored_outer_hp_after,
        config::MORTAR_OUTER_DAMAGE / 4,
        "outer mortar splash should receive the standard non-piercing armor reduction"
    );
}

#[test]
fn manual_mortar_fire_damages_allied_units_without_kill_credit() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#aaa".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(14, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let ally = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, target_pos.0, target_pos.1)
        .expect("ally should spawn");
    game.state
        .entities
        .get_mut(ally)
        .expect("ally should exist")
        .set_last_damage_owner(Some(3));
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    let launch_events = game.tick();
    let impact_pos = mortar_launch_impact(&launch_events, 1, mortar)
        .expect("owner should receive mortar launch impact");
    game.state
        .entities
        .get_mut(ally)
        .expect("ally should still exist")
        .set_position(impact_pos.0, impact_pos.1);
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    assert!(
        !game.state.entities.contains(ally),
        "same-team machine gunner should take lethal mortar splash"
    );
    let scores = game.scores();
    let attacker = scores.iter().find(|score| score.id == 1).unwrap();
    let ally_owner = scores.iter().find(|score| score.id == 2).unwrap();
    let stale_enemy = scores.iter().find(|score| score.id == 3).unwrap();
    assert_eq!(
        attacker.units_killed, 0,
        "same-team mortar splash must not award kill credit"
    );
    assert_eq!(
        stale_enemy.units_killed, 0,
        "same-team lethal splash must clear stale enemy kill credit"
    );
    assert_eq!(ally_owner.units_lost, 1);
}

#[test]
fn manual_mortar_fire_damages_friendly_buildings() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(14, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let depot = game
        .state
        .entities
        .spawn_building(1, EntityKind::Depot, target_pos.0, target_pos.1, true)
        .expect("depot should spawn");
    let hp_before = game.state.entities.get(depot).expect("depot exists").hp;
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    game.tick();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    let hp_after = game.state.entities.get(depot).expect("depot survives").hp;
    assert!(
        hp_after < hp_before,
        "friendly depot should take mortar impact damage, before={hp_before}, after={hp_after}"
    );
}

#[test]
fn snapshot_projects_visible_smoke_but_hides_enemy_inside_it() {
    let (game, _observer, _friendly, enemy, _smoke_pos) = smoke_projection_fixture();

    let snapshot = game.snapshot_for(1);

    assert_eq!(snapshot.smokes.len(), 1);
    assert!(
        snapshot.entities.iter().all(|entity| entity.id != enemy),
        "enemy inside smoke should be withheld from the opposing player snapshot"
    );
}

#[test]
fn snapshot_visibility_grid_fogs_tiles_behind_smoke() {
    let (game, _observer, _friendly, _enemy, _smoke_pos) = smoke_projection_fixture();

    let snapshot = game.snapshot_for(1);
    let index = |tx: u32, ty: u32| (ty * game.state.map.size + tx) as usize;

    assert_eq!(snapshot.visible_tiles[index(7, 4)], 1);
    assert_eq!(
        snapshot.visible_tiles[index(11, 4)],
        0,
        "tile behind smoke should be fogged in the server-provided visibility grid"
    );
}

#[test]
fn snapshot_keeps_friendly_unit_inside_smoke_visible_to_owner() {
    let (game, _observer, friendly, _enemy, _smoke_pos) = smoke_projection_fixture();

    let snapshot = game.snapshot_for(1);

    assert!(
        snapshot.entities.iter().any(|entity| entity.id == friendly),
        "friendly unit inside smoke should remain owner-visible"
    );
}

#[test]
fn snapshot_keeps_smoke_visible_to_owner_with_unit_inside() {
    let (mut game, _observer, friendly, _enemy, smoke_pos) = smoke_projection_fixture();

    if let Some(unit) = game.state.entities.get_mut(friendly) {
        unit.pos_x = smoke_pos.0;
        unit.pos_y = smoke_pos.1;
    }
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute_with_smoke(
        &ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );

    let snapshot = game.snapshot_for(1);

    assert_eq!(
        snapshot.smokes.len(),
        1,
        "owner should still receive the smoke cloud when their unit is inside it"
    );
}

#[test]
fn smoke_expiration_restores_fog_projection() {
    let (mut game, _observer, _friendly, enemy, _smoke_pos) = smoke_projection_fixture();

    for _ in 0..=config::SMOKE_CLOUD_DURATION_TICKS {
        game.tick();
    }
    let snapshot = game.snapshot_for(1);

    assert!(snapshot.smokes.is_empty());
    assert!(
        snapshot.entities.iter().any(|entity| entity.id == enemy),
        "enemy should become visible again once smoke expires"
    );
}

#[test]
fn smoke_queued_order_skipped_when_caster_dies() {
    let (mut game, scout, target, _) = smoke_command_fixture();
    use crate::game::ability::AbilityKind;
    use crate::game::command::SimCommand;
    // Queue a smoke command (unit already at range per fixture)
    game.enqueue(
        1,
        SimCommand::UseAbility {
            ability: AbilityKind::Smoke,
            units: vec![scout],
            x: Some(target.0),
            y: Some(target.1),
            queued: true,
        },
    );
    // Kill the scout car
    if let Some(e) = game.state.entities.get_mut(scout) {
        e.hp = 0;
    }
    // Tick — death system runs, then order queue promotion
    game.tick();
    assert_eq!(
        game.state.smokes.iter().count(),
        0,
        "dead scout car should not launch queued smoke"
    );
}

#[test]
fn smoke_nonfinite_target_coordinates_are_rejected() {
    let (mut game, scout, _target, _) = smoke_command_fixture();
    use crate::game::ability::AbilityKind;
    use crate::game::command::SimCommand;
    game.enqueue(
        1,
        SimCommand::UseAbility {
            ability: AbilityKind::Smoke,
            units: vec![scout],
            x: Some(f32::NAN),
            y: Some(f32::INFINITY),
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.state.smokes.iter().count(),
        0,
        "non-finite coordinates should be rejected"
    );
}
