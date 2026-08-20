use super::fixtures::*;
use super::*;

#[test]
fn artillery_attack_move_uses_team_vision_and_preserves_the_route() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "Gunner".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "Spotter".into(),
            color: "#0f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 9,
            faction_id: "kriegsia".to_string(),
            name: "Target".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let gun_pos = game.state.map.tile_center(10, 10);
    let target_pos = game.state.map.tile_center(30, 10);
    let destination = game.state.map.tile_center(50, 10);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, gun_pos.0, gun_pos.1)
        .expect("artillery should spawn");
    let enemy = game
        .state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("enemy should spawn");
    game.state
        .entities
        .spawn_unit(
            2,
            EntityKind::Worker,
            target_pos.0,
            target_pos.1 + config::TILE_SIZE as f32,
        )
        .expect("allied spotter should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    assert!(!game
        .state
        .fog
        .is_visible_world(1, target_pos.0, target_pos.1));
    assert!(game
        .state
        .fog
        .is_visible_world(2, target_pos.0, target_pos.1));

    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![artillery],
            x: destination.0,
            y: destination.1,
            queued: false,
        },
    );
    let initial_steel = game.state.players[0].steel;
    let mut fired_target = None;
    for _ in 0..=(config::ARTILLERY_SETUP_TICKS as u32 + 8) {
        for (player, events) in game.tick() {
            if player != 1 {
                continue;
            }
            for event in events {
                if let Event::ArtilleryTarget { from, x, y, .. } = event {
                    if from == artillery {
                        fired_target = Some((x, y));
                    }
                }
            }
        }
        if fired_target.is_some() {
            break;
        }
    }

    let fired_target = fired_target.expect("attack-moving artillery should set up and fire");
    assert!(
        (fired_target.0 - target_pos.0).hypot(fired_target.1 - target_pos.1)
            <= config::ARTILLERY_MIN_FIRE_RADIUS_TILES * config::TILE_SIZE as f32,
        "automatic artillery should use the narrowest available dispersion"
    );
    let artillery_entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
    assert!(matches!(artillery_entity.order(), Order::AttackMove(_)));
    assert_eq!(artillery_entity.move_intent(), Some(destination));
    assert_eq!(artillery_entity.target_id(), Some(enemy));
    assert_eq!(
        game.state.players[0].steel,
        initial_steel - config::ARTILLERY_AMMO_COST_STEEL
    );
}

#[test]
fn artillery_attack_move_ignores_neutral_tank_traps_and_acquires_enemies() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let gun_pos = game.state.map.tile_center(10, 10);
    let target_pos = game.state.map.tile_center(25, 10);
    let destination = game.state.map.tile_center(50, 10);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, gun_pos.0, gun_pos.1)
        .expect("artillery should spawn");
    let trap = game
        .state
        .entities
        .spawn_building(2, EntityKind::TankTrap, target_pos.0, target_pos.1, true)
        .expect("Tank Trap should spawn");
    game.state
        .entities
        .spawn_unit(
            1,
            EntityKind::Worker,
            target_pos.0,
            target_pos.1 + config::TILE_SIZE as f32,
        )
        .expect("spotter should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![artillery],
            x: destination.0,
            y: destination.1,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(
        game.state
            .entities
            .get(artillery)
            .and_then(Entity::target_id),
        None,
        "ordinary artillery Attack Move must ignore a visible neutral Tank Trap"
    );
    assert_eq!(
        game.state.entities.get(trap).map(|entity| entity.owner),
        Some(0),
        "the completed Tank Trap should be neutral"
    );

    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("enemy should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    game.tick();

    assert_eq!(
        game.state
            .entities
            .get(artillery)
            .and_then(Entity::target_id),
        Some(enemy),
        "artillery should still acquire a visible enemy under the same Attack Move order"
    );
}

#[test]
fn artillery_attack_move_prioritizes_soft_targets_then_infantry_then_tanks() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let gun_pos = game.state.map.tile_center(10, 10);
    let soft_pos = game.state.map.tile_center(22, 10);
    let infantry_pos = game.state.map.tile_center(26, 10);
    let tank_pos = game.state.map.tile_center(30, 10);
    let destination = game.state.map.tile_center(50, 10);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, gun_pos.0, gun_pos.1)
        .expect("artillery should spawn");
    let anti_tank_gun = game
        .state
        .entities
        .spawn_unit(2, EntityKind::AntiTankGun, soft_pos.0, soft_pos.1)
        .expect("soft support weapon should spawn");
    let infantry = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, infantry_pos.0, infantry_pos.1)
        .expect("enemy infantry should spawn");
    let tank = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, tank_pos.0, tank_pos.1)
        .expect("enemy tank should spawn");
    // Give the tank cluster far more purchase value than either better target. Target class must
    // still win before the deliberately simple dispersion-value score is considered.
    for offset in 1..=3 {
        game.state
            .entities
            .spawn_unit(
                2,
                EntityKind::Tank,
                tank_pos.0,
                tank_pos.1 + offset as f32 * config::TILE_SIZE as f32,
            )
            .expect("tank cluster should spawn");
    }
    game.state
        .entities
        .spawn_unit(
            1,
            EntityKind::Worker,
            infantry_pos.0,
            infantry_pos.1 + config::TILE_SIZE as f32,
        )
        .expect("friendly spotter inside the possible blast area should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![artillery],
            x: destination.0,
            y: destination.1,
            queued: false,
        },
    );
    game.tick();

    let artillery_entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
    assert_eq!(
        artillery_entity.target_id(),
        Some(anti_tank_gun),
        "an exposed soft support weapon should outrank a valuable tank cluster"
    );

    game.state.entities.remove(anti_tank_gun);
    game.rebuild_final_spatial();
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(artillery)
            .and_then(Entity::target_id),
        Some(infantry),
        "infantry should outrank tanks when no exposed soft target remains"
    );

    game.state.entities.remove(infantry);
    game.rebuild_final_spatial();
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(artillery)
            .and_then(Entity::target_id),
        Some(tank),
        "tanks should remain legal fallback targets"
    );
}

#[test]
fn artillery_attack_move_targets_buildings_only_after_units_are_gone() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let gun_pos = game.state.map.tile_center(10, 10);
    let building_pos = game.state.map.tile_center(24, 10);
    let unit_pos = game.state.map.tile_center(26, 10);
    let destination = game.state.map.tile_center(50, 10);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, gun_pos.0, gun_pos.1)
        .expect("artillery should spawn");
    let building = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::ResourceDepot,
            building_pos.0,
            building_pos.1,
            true,
        )
        .expect("building should spawn");
    let unit = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Worker, unit_pos.0, unit_pos.1)
        .expect("unit should spawn");
    game.state
        .entities
        .spawn_unit(1, EntityKind::Worker, unit_pos.0, unit_pos.1 + 32.0)
        .expect("spotter should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![artillery],
            x: destination.0,
            y: destination.1,
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(artillery)
            .and_then(Entity::target_id),
        Some(unit)
    );

    game.state.entities.remove(unit);
    game.rebuild_final_spatial();
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(artillery)
            .and_then(Entity::target_id),
        Some(building),
        "a visible building should become eligible once no visible unit remains"
    );
}

#[test]
fn deployed_artillery_attack_move_fires_in_field_without_tearing_down() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let gun_pos = game.state.map.tile_center(10, 10);
    let target_pos = game.state.map.tile_center(24, 10);
    let destination = game.state.map.tile_center(40, 10);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, gun_pos.0, gun_pos.1)
        .expect("artillery should spawn");
    game.state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("enemy should spawn");
    game.state
        .entities
        .spawn_unit(1, EntityKind::Worker, target_pos.0, target_pos.1 + 32.0)
        .expect("spotter should spawn");
    deploy_artillery_toward(&mut game, artillery, target_pos);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![artillery],
            x: destination.0,
            y: destination.1,
            queued: false,
        },
    );
    let events = game.tick();

    let artillery_entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
    assert_eq!(artillery_entity.weapon_setup(), WeaponSetup::Deployed);
    assert_eq!(
        artillery_entity.attack_cd(),
        config::ARTILLERY_RELOAD_TICKS,
        "autonomous fire should retain the same effective reload as manual fire"
    );
    assert!(artillery_entity.path_is_empty());
    assert!(matches!(artillery_entity.order(), Order::AttackMove(_)));
    assert!(events.iter().any(|(player, events)| {
        *player == 1
            && events.iter().any(
                |event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery),
            )
    }));
}

#[test]
fn artillery_attack_move_tears_down_and_resumes_after_targets_disappear() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let gun_pos = game.state.map.tile_center(10, 10);
    let target_pos = game.state.map.tile_center(24, 10);
    let destination = game.state.map.tile_center(40, 10);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, gun_pos.0, gun_pos.1)
        .expect("artillery should spawn");
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("enemy should spawn");
    game.state
        .entities
        .spawn_unit(1, EntityKind::Worker, target_pos.0, target_pos.1 + 32.0)
        .expect("spotter should spawn");
    deploy_artillery_toward(&mut game, artillery, target_pos);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![artillery],
            x: destination.0,
            y: destination.1,
            queued: false,
        },
    );
    game.tick();
    game.state.entities.remove(enemy);
    game.rebuild_final_spatial();

    for _ in 0..config::TICK_HZ {
        game.tick();
    }
    let artillery_entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
    assert!(matches!(
        artillery_entity.weapon_setup(),
        WeaponSetup::TearingDown { .. }
    ));
    assert!(!artillery_entity.path_is_empty());
    assert!(matches!(artillery_entity.order(), Order::AttackMove(_)));

    let stopped_x = artillery_entity.pos_x;
    for _ in 0..=(config::ARTILLERY_SETUP_TICKS as u32 + 8) {
        game.tick();
    }
    assert!(
        game.state
            .entities
            .get(artillery)
            .expect("artillery exists")
            .pos_x
            > stopped_x,
        "the packed artillery should continue toward its attack-move destination"
    );
}

#[test]
fn artillery_point_fire_queue_is_terminal() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(38, 10);
    let artillery = game
        .state
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

    let entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
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
    let pos = game.state.map.tile_center(20, 20);
    let target = game.state.map.tile_center(47, 20);
    let counter_pos = game.state.map.tile_center(4, 4);
    let observer_pos = game.state.map.tile_center(4, 12);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    let counter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, counter_pos.0, counter_pos.1)
        .expect("counter tank should spawn");
    game.state
        .entities
        .spawn_unit(3, EntityKind::Worker, observer_pos.0, observer_pos.1)
        .expect("observer worker should spawn");
    deploy_artillery_toward(&mut game, artillery, target);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    for viewer in [2, 3] {
        assert!(
            !game.state.fog.is_visible_world(viewer, pos.0, pos.1),
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
    advance_to_fog_refresh(&mut game);

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
        game.state
            .entities
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
    let pos = game.state.map.tile_center(20, 20);
    let target = game.state.map.tile_center(47, 20);
    let counter_pos = game.state.map.tile_center(4, 4);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    let counter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, counter_pos.0, counter_pos.1)
        .expect("counter tank should spawn");
    deploy_artillery_toward(&mut game, artillery, target);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.spawn_smoke_cloud_for_test(pos.0, pos.1)
        .expect("smoke should spawn over the artillery");

    assert!(
        !game.state.fog.is_visible_world(2, pos.0, pos.1),
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
        game.state
            .entities
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
    let initial_steel = game.state.players[0].steel;
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(38, 10);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    game.state
        .entities
        .spawn_unit(
            2,
            EntityKind::Worker,
            pos.0 + config::TILE_SIZE as f32,
            pos.1,
        )
        .expect("enemy gun spotter should spawn");
    game.state
        .entities
        .spawn_unit(2, EntityKind::Worker, target.0, target.1)
        .expect("enemy impact spotter should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
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
        game.state.players[0].steel <= initial_steel - config::ARTILLERY_AMMO_COST_STEEL,
        "at least one fired shell should spend steel at fire time"
    );
}

#[test]
fn packed_artillery_point_fire_auto_sets_up_before_firing() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.state.players[0].steel;
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(38, 10);
    let artillery = game
        .state
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

    let entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
    assert!(matches!(
        entity.weapon_setup(),
        WeaponSetup::Packed | WeaponSetup::SettingUp { .. }
    ));
    assert!(matches!(entity.order(), Order::ArtilleryPointFire(_)));
    assert_eq!(game.state.players[0].steel, initial_steel);
    assert!(
        events
            .iter()
            .flat_map(|(_, events)| events)
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "packed point fire should not emit a target marker before deployment"
    );

    let mut owner_saw_target = false;
    for _ in 0..=(config::ARTILLERY_SETUP_TICKS as u32 + 4) {
        for (pid, events) in game.tick() {
            owner_saw_target |= pid == 1
                && events
                    .iter()
                    .any(|event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery));
        }
    }
    assert!(owner_saw_target, "auto-setup should eventually fire");
    assert!(
        game.state.players[0].steel <= initial_steel - config::ARTILLERY_AMMO_COST_STEEL,
        "auto-setup point fire should spend ammo only once the gun is deployed"
    );
}

#[test]
fn manually_deployed_artillery_can_point_fire() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.state.players[0].steel;
    let pos = game.state.map.tile_center(10, 10);
    let setup_target = game.state.map.tile_center(18, 10);
    let fire_target = game.state.map.tile_center(38, 10);
    let artillery = game
        .state
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
        game.state
            .entities
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
        game.state.players[0].steel,
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
fn artillery_point_fire_inside_minimum_range_repositions_and_fires_at_clicked_point() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.state.players[0].steel;
    let pos = game.state.map.tile_center(10, 10);
    let min_px = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let too_close = (pos.0 + min_px - 8.0, pos.1);
    let artillery = game
        .state
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
    let entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
    assert!(matches!(entity.order(), Order::Ability(_)));
    assert_eq!(game.state.players[0].steel, initial_steel);
    assert!(
        events
            .iter()
            .flat_map(|(_, events)| events)
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "repositioning artillery should not fire before it reaches a legal firing position"
    );

    let mut fired = false;
    for _ in 0..1200 {
        for (pid, events) in game.tick() {
            fired |= pid == 1
                && events
                    .iter()
                    .any(|event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery));
        }
        if fired {
            break;
        }
    }
    let entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
    assert!(
        (entity.pos_x - pos.0).hypot(entity.pos_y - pos.1) > 1.0,
        "artillery should move out of its minimum range before setting up"
    );
    assert!(
        fired,
        "artillery should eventually fire at the clicked point; pos=({}, {}), order={:?}, setup={:?}, queued={:?}",
        entity.pos_x,
        entity.pos_y,
        entity.order(),
        entity.weapon_setup(),
        entity.queued_orders(),
    );
    let Order::ArtilleryPointFire(order) = entity.order() else {
        panic!("artillery should retain its repeating point-fire order");
    };
    assert!((order.intent.x - too_close.0).abs() < 0.001);
    assert!((order.intent.y - too_close.1).abs() < 0.001);
    assert!(game.state.players[0].steel <= initial_steel - config::ARTILLERY_AMMO_COST_STEEL);
}

#[test]
fn artillery_point_fire_beyond_maximum_range_moves_sets_up_and_fires_at_clicked_point() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(55, 10);
    let artillery = game
        .state
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
    game.tick();
    assert!(matches!(
        game.state
            .entities
            .get(artillery)
            .expect("artillery exists")
            .order(),
        Order::Ability(_)
    ));

    let mut fired = false;
    for _ in 0..1600 {
        for (pid, events) in game.tick() {
            fired |= pid == 1
                && events
                    .iter()
                    .any(|event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery));
        }
        if fired {
            break;
        }
    }

    let entity = game
        .state
        .entities
        .get(artillery)
        .expect("artillery exists");
    let distance_to_target = (entity.pos_x - target.0).hypot(entity.pos_y - target.1);
    let min_range = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let max_range = config::ARTILLERY_MAX_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    assert!(distance_to_target >= min_range && distance_to_target <= max_range);
    assert!(matches!(entity.weapon_setup(), WeaponSetup::Deployed));
    let Order::ArtilleryPointFire(order) = entity.order() else {
        panic!("artillery should retain its repeating point-fire order");
    };
    assert!((order.intent.x - target.0).abs() < 0.001);
    assert!((order.intent.y - target.1).abs() < 0.001);
    assert!(
        fired,
        "artillery should eventually fire at the clicked point"
    );
}

#[test]
fn artillery_shell_inside_building_footprint_deals_full_inner_ap_damage() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let depot = game
        .state
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("depot should spawn");
    let before = game.state.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0, 160.0);

    let after = game.state.entities.get(depot).expect("depot survives").hp;
    assert_eq!(
        before - after,
        config::ARTILLERY_INNER_DAMAGE,
        "inner artillery splash should bypass armored-building damage reduction"
    );
}

#[test]
fn artillery_shell_outside_building_uses_footprint_distance_falloff() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let depot = game
        .state
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("depot should spawn");
    let stats = config::building_stats(EntityKind::Depot).expect("depot stats");
    let ts = config::TILE_SIZE as f32;
    let half_w = stats.foot_w as f32 * ts * 0.5;
    let inner = config::ARTILLERY_INNER_RADIUS_TILES * ts;
    let outer = config::ARTILLERY_OUTER_RADIUS_TILES * ts;
    let gap = inner + (outer - inner) * 0.5;
    let before = game.state.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0 + half_w + gap, 160.0);

    let after = game.state.entities.get(depot).expect("depot survives").hp;
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
        .state
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("allied depot should spawn");
    let before = game.state.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0, 160.0);

    let depot = game.state.entities.get(depot).expect("depot survives");
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
        game.state
            .players
            .iter()
            .map(|player| (player.id, player.team_id)),
    );
    game.state
        .artillery_shells
        .schedule(1, 1, x, y, game.state.tick);
    game.state.artillery_shells.resolve_due(
        &game.state.map,
        &mut game.state.entities,
        &teams,
        &game.state.fog,
        &mut events,
        game.state.tick + config::ARTILLERY_SHELL_DELAY_TICKS,
        |_, _| {},
    );
}

fn deploy_artillery_toward(game: &mut Game, artillery: u32, target: (f32, f32)) {
    let entity = game
        .state
        .entities
        .get_mut(artillery)
        .expect("artillery should exist");
    let facing = (target.1 - entity.pos_y).atan2(target.0 - entity.pos_x);
    entity.set_weapon_setup(WeaponSetup::Deployed);
    entity.set_emplacement_facing(Some(facing));
    entity.set_desired_weapon_facing(facing);
}
