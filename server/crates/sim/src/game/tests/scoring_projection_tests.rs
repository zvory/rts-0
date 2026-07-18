use super::fixtures::*;
use super::*;

#[test]
fn scores_count_starting_entities() {
    let players = human_vs_ai_players();
    let game = Game::new(&players, 0x0515_C0DE);
    let scores = game.scores();
    let human = scores
        .iter()
        .find(|score| score.id == 1)
        .expect("human score should exist");

    assert_eq!(
        human.unit_score,
        config::STARTING_WORKERS * entity_score_value(EntityKind::Worker)
    );
    assert_eq!(
        human.structure_score,
        entity_score_value(EntityKind::CityCentre)
    );
    assert_eq!(human.units_killed, 0);
    assert_eq!(human.units_lost, 0);
    assert_eq!(human.buildings_killed, 0);
    assert_eq!(human.buildings_lost, 0);
}

#[test]
fn scores_record_kills_and_losses_on_death() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0x0515_C0DE);
    let victim_unit = game
        .state
        .entities
        .iter()
        .find(|e| e.owner == 2 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("victim unit should exist");
    let victim_building = game
        .state
        .entities
        .iter()
        .find(|e| e.owner == 2 && e.kind == EntityKind::CityCentre)
        .map(|e| e.id)
        .expect("victim building should exist");
    for id in [victim_unit, victim_building] {
        let entity = game
            .state
            .entities
            .get_mut(id)
            .expect("victim should exist");
        entity.hp = 0;
        entity.set_last_damage_owner(Some(1));
    }

    let mut events: HashMap<u32, Vec<Event>> = game
        .state
        .players
        .iter()
        .map(|p| (p.id, Vec::new()))
        .collect();
    let mut lingering_sight = Vec::new();
    let tick = game.tick_count();
    let teams = game.team_relations();
    services::death::death_system(
        &mut game.state.entities,
        &game.state.fog,
        &game.state.smokes,
        &teams,
        &mut game.state.players,
        &mut lingering_sight,
        &mut events,
        tick,
    );

    let scores = game.scores();
    let attacker = scores
        .iter()
        .find(|score| score.id == 1)
        .expect("attacker score should exist");
    let victim = scores
        .iter()
        .find(|score| score.id == 2)
        .expect("victim score should exist");

    assert_eq!(attacker.units_killed, 1);
    assert_eq!(attacker.buildings_killed, 1);
    assert_eq!(victim.units_lost, 1);
    assert_eq!(victim.buildings_lost, 1);
}

#[test]
fn observer_analysis_reports_authoritative_inventory_production_and_losses() {
    let players = human_vs_ai_players();
    let mut game =
        Game::new_for_replay_with_starting_resources(&players, 5_000, 5_000, 0xA11A_0001);
    let city_centre = game
        .state
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::CityCentre)
        .map(|e| e.id)
        .expect("player city centre should exist");
    game.enqueue(
        1,
        Command::Train {
            building: city_centre,
            unit: EntityKind::Worker,
        },
    );
    game.tick();
    game.state.tick = config::TICK_HZ * 10;
    let current_tick = game.tick_count();
    {
        let player = game
            .state
            .players
            .iter_mut()
            .find(|player| player.id == 1)
            .expect("player one should exist");
        player.add_gathered_resources(EntityKind::Steel, 80, current_tick);
        player.add_gathered_resources(EntityKind::Oil, 30, current_tick - config::TICK_HZ * 10);
        player.upgrades.insert(upgrade::UpgradeKind::TankUnlock);
    }

    let victim_unit = game
        .state
        .entities
        .iter()
        .find(|e| e.owner == 2 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("victim unit should exist");
    let entity = game
        .state
        .entities
        .get_mut(victim_unit)
        .expect("victim unit should still exist");
    entity.hp = 0;
    entity.set_last_damage_owner(Some(1));
    let mut events: HashMap<u32, Vec<Event>> = game
        .state
        .players
        .iter()
        .map(|p| (p.id, Vec::new()))
        .collect();
    let mut lingering_sight = Vec::new();
    let tick = game.tick_count();
    let teams = game.team_relations();
    services::death::death_system(
        &mut game.state.entities,
        &game.state.fog,
        &game.state.smokes,
        &teams,
        &mut game.state.players,
        &mut lingering_sight,
        &mut events,
        tick,
    );

    let analysis = game.observer_analysis();
    assert_eq!(analysis.tick, game.tick_count());
    let player_one = analysis
        .players
        .iter()
        .find(|player| player.id == 1)
        .expect("player one analysis should exist");
    assert!(player_one
        .units
        .iter()
        .any(|row| row.kind == "worker" && row.count == config::STARTING_WORKERS));
    assert!(player_one.production.iter().any(|row| {
        row.building_id == city_centre
            && row.building_kind == "city_centre"
            && row.item_kind == "worker"
            && row.item_type == "unit"
            && row.queue_depth == 1
            && row.progress > 0.0
    }));
    assert_eq!(player_one.upgrades, vec!["tank_unlock"]);
    assert_eq!(player_one.resources.lifetime.steel, 80);
    assert_eq!(player_one.resources.lifetime.oil, 30);
    assert_eq!(player_one.resources.last_5s.steel, 80);
    assert_eq!(player_one.resources.last_5s.oil, 0);
    assert_eq!(player_one.resources.last_minute.steel, 80);
    assert_eq!(player_one.resources.last_minute.oil, 30);
    let player_two = analysis
        .players
        .iter()
        .find(|player| player.id == 2)
        .expect("player two analysis should exist");
    assert!(player_two
        .units_lost
        .iter()
        .any(|row| row.kind == "worker" && row.count == 1 && row.steel_value > 0));
    assert_eq!(
        player_two.resources_lost.steel,
        player_two.units_lost[0].steel_value
    );
    assert_eq!(
        player_two.resources_lost.oil,
        player_two.units_lost[0].oil_value
    );
}

#[test]
fn phase4_projection_matches_legacy_snapshot_entities() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let (sx, sy) = game.state.map.tile_center(
        game.state.players[0].start_tile.0,
        game.state.players[0].start_tile.1,
    );
    let attacker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, sx + 64.0, sy)
        .expect("attacker should spawn");
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, sx + 96.0, sy)
        .expect("target should spawn");
    if let Some(e) = game.state.entities.get_mut(attacker) {
        e.set_order(Order::attack(target));
        e.set_target_id(Some(target));
    }
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    assert_eq!(
        game.snapshot_for(1).entities,
        legacy_snapshot_entities(&game, 1, true)
    );
    assert_eq!(
        game.snapshot_full_for(1).entities,
        legacy_snapshot_entities(&game, 1, false)
    );
}

#[test]
fn spectator_snapshot_uses_union_fog_not_full_world() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let active_players = [1, 2];
    game.state
        .fog
        .recompute(&active_players, &game.state.entities, &game.state.map);

    let hidden_pos = (0..game.state.map.size)
        .flat_map(|ty| (0..game.state.map.size).map(move |tx| (tx, ty)))
        .find_map(|(tx, ty)| {
            let (x, y) = game.state.map.tile_center(tx, ty);
            let hidden_from_all = active_players
                .iter()
                .all(|player| !game.state.fog.is_visible_world(*player, x, y));
            hidden_from_all.then_some((x, y))
        })
        .expect("map should contain a tile outside both players' opening fog");
    let hidden = game
        .state
        .entities
        .spawn_unit(99, EntityKind::Rifleman, hidden_pos.0, hidden_pos.1)
        .expect("hidden unit should spawn");
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&active_players, &game.state.entities, &game.state.map);

    let snapshot = game.snapshot_for_spectator(&active_players);

    assert!(snapshot.entities.iter().any(|e| e.owner == 1));
    assert!(snapshot.entities.iter().any(|e| e.owner == 2));
    assert!(!snapshot.entities.iter().any(|e| e.id == hidden));
    assert_eq!(snapshot.player_resources.len(), 2);
}

#[test]
fn spectator_player_resources_follow_selected_players() {
    let players = human_vs_ai_players();
    let game = Game::new(&players, 0x0515_C0DE);

    let snapshot = game.snapshot_for_spectator(&[2]);

    assert_eq!(
        snapshot
            .player_resources
            .iter()
            .map(|resources| resources.id)
            .collect::<Vec<_>>(),
        vec![2]
    );
}

#[test]
fn spectator_apm_counts_one_multi_unit_command_as_one_action() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0x0515_C0DE);
    let workers = game
        .state
        .entities
        .iter()
        .filter(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
        .map(|entity| entity.id)
        .collect::<Vec<_>>();
    let destination = game.state.map.tile_center(12, 12);

    game.enqueue(
        1,
        Command::Move {
            units: workers,
            x: destination.0,
            y: destination.1,
            queued: false,
        },
    );
    game.tick();

    let snapshot = game.snapshot_for_spectator(&[1]);
    assert_eq!(snapshot.player_resources[0].apm, 6);
}

#[test]
fn death_vision_lingers_as_normal_vision_for_five_seconds() {
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
    let mut game = Game::new_for_replay(&players, 0xD3AD_5151);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let rifle_pos = game.state.map.tile_center(2, 2);
    let rifle = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, rifle_pos.0, rifle_pos.1)
        .expect("rifleman should spawn");
    let second_rifle_pos = game.state.map.tile_center(2, 3);
    let second_rifle = game
        .state
        .entities
        .spawn_unit(
            1,
            EntityKind::Rifleman,
            second_rifle_pos.0,
            second_rifle_pos.1,
        )
        .expect("second rifleman should spawn");
    let spotter_pos = game.state.map.tile_center(20, 20);
    let spotter = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, spotter_pos.0, spotter_pos.1)
        .expect("spotter should spawn");
    let enemy_pos = game.state.map.tile_center(22, 20);
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    let enemy_depot_pos = game.state.map.tile_center(24, 21);
    let enemy_depot = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::Depot,
            enemy_depot_pos.0,
            enemy_depot_pos.1,
            true,
        )
        .expect("enemy depot should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    assert!(game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1));

    game.state
        .entities
        .get_mut(spotter)
        .expect("spotter should exist")
        .hp = 0;
    game.tick();

    assert!(!game.state.entities.contains(spotter));
    assert!(
        game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
        "death vision should become ordinary live fog while the linger lasts"
    );
    let first_linger = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|e| e.id == enemy)
        .expect("enemy should remain visible through lingering death vision");
    assert!(!first_linger.vision_only);
    let first_building_linger = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|e| e.id == enemy_depot)
        .expect("enemy building should remain visible through lingering death vision");
    assert!(!first_building_linger.vision_only);

    let enemy_goal = game.state.map.tile_center(24, 20);
    game.enqueue(
        1,
        Command::Attack {
            units: vec![rifle],
            target: enemy,
            queued: false,
        },
    );
    game.enqueue(
        1,
        Command::Attack {
            units: vec![second_rifle],
            target: enemy_depot,
            queued: true,
        },
    );
    game.enqueue(
        2,
        Command::Move {
            units: vec![enemy],
            x: enemy_goal.0,
            y: enemy_goal.1,
            queued: false,
        },
    );
    game.tick();

    let rifle_entity = game
        .state
        .entities
        .get(rifle)
        .expect("rifle should remain alive");
    assert_eq!(
        rifle_entity.order().attack_target(),
        Some(enemy),
        "death-vision enemy units should be accepted as direct attack targets"
    );
    let second_rifle_entity = game
        .state
        .entities
        .get(second_rifle)
        .expect("second rifle should remain alive");
    assert_eq!(
        second_rifle_entity.order().attack_target(),
        Some(enemy_depot),
        "queued death-vision enemy buildings should promote as direct attack targets"
    );
    let moved_enemy = game
        .state
        .entities
        .get(enemy)
        .expect("enemy should remain alive");
    let moving_linger = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|e| e.id == enemy)
        .expect("moving enemy should still be visible during lingering death vision");
    assert!(!moving_linger.vision_only);
    assert!((moving_linger.x - moved_enemy.pos_x).abs() < 0.001);
    assert!((moving_linger.y - moved_enemy.pos_y).abs() < 0.001);

    while game.tick_count() <= config::TICK_HZ * 5 {
        game.tick();
    }
    advance_to_fog_refresh(&mut game);
    let expired_snapshot = game.snapshot_for(1);
    assert!(
        expired_snapshot.entities.iter().all(|e| e.id != enemy),
        "lingering death vision should expire after five seconds"
    );
    assert!(
        expired_snapshot
            .remembered_buildings
            .iter()
            .any(|view| view.id == enemy_depot),
        "death vision is normal vision and should refresh remembered enemy buildings"
    );
}

#[test]
fn allied_death_vision_allows_teammate_attacks_and_auto_acquisition() {
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
            color: "#ddd".into(),
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
    let mut game = Game::new_for_replay(&players, 0xA11E_D515);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let player_one_base = game.state.map.tile_center(2, 8);
    game.state
        .entities
        .spawn_building(
            1,
            EntityKind::CityCentre,
            player_one_base.0,
            player_one_base.1,
            true,
        )
        .expect("player one base should spawn");
    let player_two_base = game.state.map.tile_center(2, 25);
    game.state
        .entities
        .spawn_building(
            2,
            EntityKind::CityCentre,
            player_two_base.0,
            player_two_base.1,
            true,
        )
        .expect("player two base should spawn");
    let enemy_base = game.state.map.tile_center(28, 28);
    game.state
        .entities
        .spawn_building(3, EntityKind::CityCentre, enemy_base.0, enemy_base.1, true)
        .expect("enemy base should spawn");

    let rifle_pos = game.state.map.tile_center(2, 2);
    let rifle = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, rifle_pos.0, rifle_pos.1)
        .expect("rifleman should spawn");
    let second_rifle_pos = game.state.map.tile_center(3, 2);
    let second_rifle = game
        .state
        .entities
        .spawn_unit(
            1,
            EntityKind::Rifleman,
            second_rifle_pos.0,
            second_rifle_pos.1,
        )
        .expect("second rifleman should spawn");
    let mortar_pos = game.state.map.tile_center(4, 2);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
        mortar_entity.set_emplacement_facing(Some(0.0));
    }
    let spotter_pos = game.state.map.tile_center(15, 2);
    let spotter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Worker, spotter_pos.0, spotter_pos.1)
        .expect("allied spotter should spawn");
    let enemy_pos = game.state.map.tile_center(16, 2);
    let enemy = game
        .state
        .entities
        .spawn_unit(3, EntityKind::Tank, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    assert!(!game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1));
    assert!(game.state.fog.is_visible_world(2, enemy_pos.0, enemy_pos.1));

    game.state
        .entities
        .get_mut(spotter)
        .expect("spotter should exist")
        .hp = 0;
    game.tick();
    advance_to_fog_refresh(&mut game);

    let allied_linger = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|entity| entity.id == enemy)
        .expect("teammate death vision should be shared into player one's snapshot");
    assert!(!allied_linger.vision_only);
    assert!(
        game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
        "teammate death vision should be stamped into player one's live fog"
    );

    game.enqueue(
        1,
        Command::Attack {
            units: vec![rifle],
            target: enemy,
            queued: false,
        },
    );
    game.enqueue(
        1,
        Command::Attack {
            units: vec![second_rifle],
            target: enemy,
            queued: true,
        },
    );
    game.tick();

    let rifle_entity = game
        .state
        .entities
        .get(rifle)
        .expect("rifle should remain alive");
    assert_eq!(
        rifle_entity.order().attack_target(),
        Some(enemy),
        "direct attack should validate against team-shared death vision"
    );
    let second_rifle_entity = game
        .state
        .entities
        .get(second_rifle)
        .expect("second rifle should remain alive");
    assert_eq!(
        second_rifle_entity.order().attack_target(),
        Some(enemy),
        "queued attack promotion should validate against team-shared death vision"
    );
    let mortar_entity = game
        .state
        .entities
        .get(mortar)
        .expect("mortar should remain alive");
    assert_eq!(
        mortar_entity.target_id(),
        Some(enemy),
        "allied death vision should drive teammate auto-acquisition"
    );
}
