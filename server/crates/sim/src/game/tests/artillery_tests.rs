use super::fixtures::*;
use super::*;

#[test]
fn artillery_point_fire_queue_is_terminal() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(26, 10);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    deploy_artillery_toward(&mut game, artillery, target);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![artillery],
            x: target.0 + 64.0,
            y: target.1,
            queued: true,
        },
    );
    game.tick();

    let entity = game.entities.get(artillery).expect("artillery exists");
    assert!(matches!(entity.order(), Order::ArtilleryPointFire(_)));
    assert!(
        entity.queued_orders().is_empty(),
        "later queued move should not be accepted behind terminal Point Fire"
    );
}

#[test]
fn artillery_firing_from_fog_is_actionable_for_all_enemies() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Shooter".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Counter".into(),
            color: "#000".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Observer".into(),
            color: "#f00".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(20, 20);
    let target = game.map.tile_center(42, 20);
    let counter_pos = game.map.tile_center(4, 4);
    let observer_pos = game.map.tile_center(4, 12);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    let counter = game
        .entities
        .spawn_unit(2, EntityKind::Tank, counter_pos.0, counter_pos.1)
        .expect("counter tank should spawn");
    game.entities
        .spawn_unit(3, EntityKind::Worker, observer_pos.0, observer_pos.1)
        .expect("observer worker should spawn");
    deploy_artillery_toward(&mut game, artillery, target);
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    for viewer in [2, 3] {
        assert!(
            !game.fog.is_visible_world(viewer, pos.0, pos.1),
            "fixture requires artillery to start hidden from player {viewer}"
        );
    }

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
    game.tick();

    for viewer in [2, 3] {
        let view = game
            .snapshot_for(viewer)
            .entities
            .into_iter()
            .find(|entity| entity.id == artillery)
            .expect("firing artillery should be visible to every enemy player");
        assert!(
            !view.vision_only,
            "firing artillery should be actionable live fog for player {viewer}"
        );
    }

    game.enqueue(
        2,
        Command::Attack {
            units: vec![counter],
            target: artillery,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(
        game.entities
            .get(counter)
            .expect("counter should exist")
            .order()
            .attack_target(),
        Some(artillery),
        "enemy units should accept direct attack orders against firing-revealed artillery"
    );
}

#[test]
fn artillery_firing_reveal_does_not_override_smoke_concealment() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(20, 20);
    let target = game.map.tile_center(42, 20);
    let counter_pos = game.map.tile_center(4, 4);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    let counter = game
        .entities
        .spawn_unit(2, EntityKind::Tank, counter_pos.0, counter_pos.1)
        .expect("counter tank should spawn");
    deploy_artillery_toward(&mut game, artillery, target);
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    game.spawn_smoke_cloud_for_test(pos.0, pos.1)
        .expect("smoke should spawn over the artillery");

    assert!(
        !game.fog.is_visible_world(2, pos.0, pos.1),
        "fixture requires smoke to hide the artillery from player 2"
    );

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
    let events = game.tick();

    assert!(
        events.iter().any(|(pid, events)| {
            *pid == 2
                && events
                    .iter()
                    .any(|event| matches!(event, Event::ArtilleryFiring { owner: 1, .. }))
        }),
        "the global firing marker should still confirm the shot was launched"
    );
    assert!(
        !game
            .snapshot_for(2)
            .entities
            .iter()
            .any(|entity| entity.id == artillery),
        "actionable firing reveal must not make a smoke-hidden artillery visible"
    );

    game.enqueue(
        2,
        Command::Attack {
            units: vec![counter],
            target: artillery,
            queued: false,
        },
    );
    game.tick();

    assert_ne!(
        game.entities
            .get(counter)
            .expect("counter should exist")
            .order()
            .attack_target(),
        Some(artillery),
        "smoke-hidden firing artillery should not validate direct attack commands"
    );
}

#[test]
fn artillery_target_is_owner_only_and_enemy_events_require_current_vision() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(26, 10);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    game.entities
        .spawn_unit(
            2,
            EntityKind::Worker,
            pos.0 + config::TILE_SIZE as f32,
            pos.1,
        )
        .expect("enemy gun spotter should spawn");
    game.entities
        .spawn_unit(2, EntityKind::Worker, target.0, target.1)
        .expect("enemy impact spotter should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    deploy_artillery_toward(&mut game, artillery, target);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );

    let mut owner_saw_target = false;
    let mut enemy_saw_target = false;
    let mut enemy_saw_artillery_reveal = false;
    let mut owner_saw_impact = false;
    let mut enemy_saw_impact = false;
    for _ in 0..(config::ARTILLERY_SETUP_TICKS as u32 + config::ARTILLERY_SHELL_DELAY_TICKS + 8) {
        for (pid, events) in game.tick() {
            for event in events {
                match event {
                    Event::ArtilleryTarget { .. } if pid == 1 => owner_saw_target = true,
                    Event::ArtilleryTarget { .. } if pid == 2 => enemy_saw_target = true,
                    Event::Attack {
                        from,
                        reveal: Some(reveal),
                        ..
                    } if pid == 2 && from == artillery && reveal.kind == kinds::ARTILLERY => {
                        enemy_saw_artillery_reveal = true
                    }
                    Event::ArtilleryImpact { .. } if pid == 1 => owner_saw_impact = true,
                    Event::ArtilleryImpact { .. } if pid == 2 => enemy_saw_impact = true,
                    _ => {}
                }
            }
        }
    }

    assert!(
        owner_saw_target,
        "firing player should see pre-impact target marker"
    );
    assert!(
        !enemy_saw_target,
        "enemy should never receive pre-impact artillery target marker"
    );
    assert!(enemy_saw_artillery_reveal);
    assert!(owner_saw_impact, "owner should see delayed impact");
    assert!(
        enemy_saw_impact,
        "enemy should see delayed impact only with current vision at the impact"
    );
    assert!(
        game.players[0].steel <= initial_steel - config::ARTILLERY_AMMO_COST_STEEL,
        "at least one fired shell should spend steel at fire time"
    );
}

#[test]
fn packed_artillery_point_fire_does_not_auto_setup_or_fire() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(26, 10);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
    let events = game.tick();

    let entity = game.entities.get(artillery).expect("artillery exists");
    assert!(matches!(entity.weapon_setup(), WeaponSetup::Packed));
    assert!(!matches!(entity.order(), Order::ArtilleryPointFire(_)));
    assert_eq!(game.players[0].steel, initial_steel);
    assert!(
        events
            .iter()
            .flat_map(|(_, events)| events)
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "packed point fire should not emit a target marker"
    );
}

#[test]
fn manually_deployed_artillery_can_point_fire() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let setup_target = game.map.tile_center(18, 10);
    let fire_target = game.map.tile_center(26, 10);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");

    game.enqueue(
        1,
        Command::SetupAntiTankGuns {
            units: vec![artillery],
            x: setup_target.0,
            y: setup_target.1,
            queued: false,
        },
    );
    for _ in 0..=config::ARTILLERY_SETUP_TICKS {
        game.tick();
    }
    assert!(matches!(
        game.entities
            .get(artillery)
            .expect("artillery exists")
            .weapon_setup(),
        WeaponSetup::Deployed
    ));

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(fire_target.0),
            y: Some(fire_target.1),
            queued: false,
        },
    );
    let events = game.tick();

    assert_eq!(
        game.players[0].steel,
        initial_steel - config::ARTILLERY_AMMO_COST_STEEL
    );
    assert!(
        events.iter().any(|(pid, events)| {
            *pid == 1
                && events
                    .iter()
                    .any(|event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery))
        }),
        "manual setup should allow artillery point fire and identify the firing gun"
    );
}

#[test]
fn artillery_point_fire_inside_minimum_range_does_not_spend_steel() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let min_px = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let too_close = (pos.0 + min_px - 8.0, pos.1);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    deploy_artillery_toward(&mut game, artillery, too_close);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(too_close.0),
            y: Some(too_close.1),
            queued: false,
        },
    );
    let events = game.tick();

    assert_eq!(game.players[0].steel, initial_steel);
    assert!(
        events
            .iter()
            .flat_map(|(_, events)| events)
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "minimum-range rejection should not fire or create a target marker"
    );
}

#[test]
fn artillery_shell_inside_building_footprint_deals_full_inner_ap_damage() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("depot should spawn");
    let before = game.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0, 160.0);

    let after = game.entities.get(depot).expect("depot survives").hp;
    let expected = combat::effective_damage(
        EntityKind::Artillery,
        EntityKind::Depot,
        config::ARTILLERY_INNER_DAMAGE,
        Some(TerrainKind::Open),
    );
    assert_eq!(before - after, expected);
}

#[test]
fn artillery_shell_outside_building_uses_footprint_distance_falloff() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("depot should spawn");
    let stats = config::building_stats(EntityKind::Depot).expect("depot stats");
    let ts = config::TILE_SIZE as f32;
    let half_w = stats.foot_w as f32 * ts * 0.5;
    let inner = config::ARTILLERY_INNER_RADIUS_TILES * ts;
    let outer = config::ARTILLERY_OUTER_RADIUS_TILES * ts;
    let gap = inner + (outer - inner) * 0.5;
    let before = game.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0 + half_w + gap, 160.0);

    let after = game.entities.get(depot).expect("depot survives").hp;
    let expected = {
        let t = ((gap - inner) / (outer - inner)).clamp(0.0, 1.0);
        let base = (config::ARTILLERY_INNER_DAMAGE as f32
            + (config::ARTILLERY_OUTER_MIN_DAMAGE as f32 - config::ARTILLERY_INNER_DAMAGE as f32)
                * t)
            .round() as u32;
        combat::effective_damage(
            EntityKind::Rifleman,
            EntityKind::Depot,
            base,
            Some(TerrainKind::Open),
        )
    };
    assert_eq!(before - after, expected);
}

#[test]
fn artillery_shell_damages_allied_entities_without_last_damage_attribution() {
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
    ];
    let mut game = empty_flat_game(&players);
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("allied depot should spawn");
    let before = game.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0, 160.0);

    let depot = game.entities.get(depot).expect("depot survives");
    assert!(
        depot.hp < before,
        "same-team depot should take artillery splash damage"
    );
    assert_eq!(depot.last_damage_owner(), None);
    assert_eq!(depot.last_damage_pos(), None);
    assert_eq!(depot.last_damage_tick(), None);
}

fn resolve_test_artillery_shell(game: &mut Game, x: f32, y: f32) {
    let mut events = HashMap::new();
    events.insert(1, Vec::new());
    let teams = teams::TeamRelations::from_player_teams(
        game.players
            .iter()
            .map(|player| (player.id, player.team_id)),
    );
    game.artillery_shells.schedule(1, 1, x, y, game.tick);
    game.artillery_shells.resolve_due(
        &mut game.entities,
        &teams,
        &game.fog,
        &mut events,
        game.tick + config::ARTILLERY_SHELL_DELAY_TICKS,
    );
}

fn deploy_artillery_toward(game: &mut Game, artillery: u32, target: (f32, f32)) {
    let entity = game
        .entities
        .get_mut(artillery)
        .expect("artillery should exist");
    let facing = (target.1 - entity.pos_y).atan2(target.0 - entity.pos_x);
    entity.set_weapon_setup(WeaponSetup::Deployed);
    entity.set_emplacement_facing(Some(facing));
    entity.set_desired_weapon_facing(facing);
}
