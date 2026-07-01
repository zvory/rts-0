use super::fixtures::*;
use super::*;

#[test]
fn replay_keyframe_clone_preserves_ability_runtime_state() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let caster = game.state.entities
        .spawn_unit(1, EntityKind::Rifleman, 128.0, 128.0)
        .expect("caster should spawn");
    let object_id = game.state.ability_runtime
        .spawn_world_object(ability_runtime::AbilityWorldObjectSpec {
            owner: 1,
            caster_id: caster,
            ability: ability::AbilityKind::EkatTeleport,
            kind: ability_runtime::AbilityWorldObjectKind::ReturnMarker,
            x: 128.0,
            y: 128.0,
            created_tick: 0,
            expires_tick: 30,
            payload: ability_runtime::AbilityObjectPayload::DashReturn {
                earliest_return_tick: 1,
            },
        })
        .expect("ability object should spawn");

    let projectile_id = game.state.ability_runtime
        .spawn_projectile(ability_projectile::AbilityProjectileSpec {
            owner: 1,
            caster_id: caster,
            source_object_id: None,
            ability: ability::AbilityKind::EkatLineShot,
            origin: (128.0, 128.0),
            endpoint: (192.0, 128.0),
            return_target: ability_projectile::AbilityProjectileReturnTarget::FixedPoint {
                x: 128.0,
                y: 128.0,
            },
            speed_px_per_tick: 16.0,
            width_px: 4.0,
            damage: 0,
            created_tick: 0,
            expires_tick: 30,
        })
        .expect("ability projectile should spawn");

    let mut clone = game.clone_for_replay_keyframe();

    assert_eq!(
        clone.state.ability_runtime
            .world_objects()
            .map(|object| object.id.get())
            .collect::<Vec<_>>(),
        vec![object_id, projectile_id]
    );
    assert_eq!(
        clone.state.ability_runtime
            .projectiles()
            .map(|projectile| projectile.id.get())
            .collect::<Vec<_>>(),
        vec![projectile_id]
    );

    clone.tick();

    let projectile_object = clone.state.ability_runtime
        .world_objects()
        .find(|object| object.id.get() == projectile_id)
        .expect("cloned projectile visual should still exist after ticking");
    assert!(
        projectile_object.x > 128.0,
        "cloned keyframe projectile should continue advancing after replay restore"
    );
}

#[test]
fn game_tick_cleans_up_expired_ability_runtime_state() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let caster = game.state.entities
        .spawn_unit(1, EntityKind::Rifleman, 128.0, 128.0)
        .expect("caster should spawn");
    game.state.ability_runtime
        .spawn_world_object(ability_runtime::AbilityWorldObjectSpec {
            owner: 1,
            caster_id: caster,
            ability: ability::AbilityKind::EkatTeleport,
            kind: ability_runtime::AbilityWorldObjectKind::ReturnMarker,
            x: 128.0,
            y: 128.0,
            created_tick: 0,
            expires_tick: 1,
            payload: ability_runtime::AbilityObjectPayload::None,
        })
        .expect("ability object should spawn");

    game.tick();

    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
}

#[test]
fn snapshot_projects_abilities_from_owner_faction_catalog() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EMPTY_FIXTURE_FACTION_ID.to_string(),
        name: "Fixture".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let scout = game.state.entities
        .spawn_unit(1, EntityKind::ScoutCar, pos.0, pos.1)
        .expect("scout should spawn");
    game.rebuild_final_spatial();
    game.state.fog.recompute(&[1], &game.state.entities, &game.state.map);

    let snapshot = game.snapshot_for(1);
    let scout_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == scout)
        .expect("scout should project");

    assert!(
        scout_view.abilities.is_empty(),
        "fixture faction scout cars should not inherit Kriegsia Smoke affordances"
    );
}

#[test]
fn ekat_start_projects_hero_zamok_and_abilities() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EKAT_FACTION_ID.to_string(),
        name: "Ekat".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let game = Game::new_for_replay(&players, 0x1234_5678);

    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].oil, 0);
    assert!(game.state.entities
        .iter()
        .any(|entity| entity.owner == 1 && entity.kind == EntityKind::Zamok));
    let hero = game.state.entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Ekat)
        .expect("Ekat should start with her hero");
    assert_eq!(hero.hp, 150);

    let snapshot = game.snapshot_for(1);
    let hero_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == hero.id)
        .expect("hero should project");
    let ability_ids: Vec<_> = hero_view
        .abilities
        .iter()
        .map(|ability| ability.ability.as_str())
        .collect();
    assert_eq!(
        ability_ids,
        vec![
            crate::protocol::abilities::EKAT_TELEPORT,
            crate::protocol::abilities::EKAT_LINE_SHOT,
            crate::protocol::abilities::EKAT_MAGIC_ANCHOR,
            crate::protocol::abilities::EKAT_CONSUME_GOLEM,
        ]
    );
}

#[test]
fn ekat_does_not_passively_regenerate() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EKAT_FACTION_ID.to_string(),
        name: "Ekat".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let hero = game.state.entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    game.state.entities
        .get_mut(hero)
        .expect("hero exists")
        .apply_damage(50, None);

    for _ in 0..config::TICK_HZ {
        game.tick();
    }

    assert_eq!(game.state.entities.get(hero).expect("hero exists").hp, 100);
}
