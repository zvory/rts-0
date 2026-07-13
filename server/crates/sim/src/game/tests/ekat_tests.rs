use super::fixtures::*;
use super::*;

fn ekat_player() -> PlayerInit {
    PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EKAT_FACTION_ID.to_string(),
        name: "Ekat".into(),
        color: "#fff".into(),
        is_ai: false,
    }
}

fn kriegsia_enemy() -> PlayerInit {
    PlayerInit {
        id: 2,
        team_id: 2,
        faction_id: crate::rules::faction::DEFAULT_FACTION_ID.to_string(),
        name: "Enemy".into(),
        color: "#000".into(),
        is_ai: false,
    }
}

fn enqueue_ekat_dash(game: &mut Game, hero: u32, target: (f32, f32)) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatTeleport,
            units: vec![hero],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
}

fn enqueue_ekat_return(game: &mut Game, hero: u32, target_object_id: Option<u32>) {
    game.enqueue(
        1,
        Command::RecastAbility {
            ability: ability::AbilityKind::EkatTeleport,
            units: vec![hero],
            target_object_id,
            queued: false,
        },
    );
}

fn enqueue_ekat_line_shot(game: &mut Game, hero: u32, target: (f32, f32)) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatLineShot,
            units: vec![hero],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
}

fn enqueue_ekat_anchor(game: &mut Game, hero: u32, target: (f32, f32)) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatMagicAnchor,
            units: vec![hero],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
}

fn enqueue_ekat_consume_golem(game: &mut Game, hero: u32) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatConsumeGolem,
            units: vec![hero],
            x: None,
            y: None,
            queued: false,
        },
    );
}

fn active_return_marker_id(game: &Game, hero: u32) -> Option<u32> {
    game.state
        .ability_runtime
        .active_return_marker(
            1,
            hero,
            ability::AbilityKind::EkatTeleport,
            None,
            game.current_tick(),
        )
        .map(|marker| marker.id.get())
}

fn active_anchor_id(game: &Game, hero: u32) -> Option<u32> {
    game.state
        .ability_runtime
        .active_anchor(
            1,
            hero,
            ability::AbilityKind::EkatMagicAnchor,
            game.current_tick(),
        )
        .map(|anchor| anchor.id.get())
}

fn line_projectiles(game: &Game) -> Vec<ability_projectile::AbilityProjectile> {
    game.state
        .ability_runtime
        .projectiles()
        .filter(|projectile| projectile.ability == ability::AbilityKind::EkatLineShot)
        .cloned()
        .collect()
}

fn refresh_visibility(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
}

#[test]
fn zamok_trains_golem_for_ekat_faction() {
    let players = [ekat_player()];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let zamok = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Zamok)
        .map(|entity| entity.id)
        .expect("Ekat should start with Zamok");

    game.enqueue(
        1,
        Command::Train {
            building: zamok,
            unit: EntityKind::Golem,
        },
    );
    for _ in 0..=config::unit_stats(EntityKind::Golem)
        .expect("Golem stats should exist")
        .build_ticks
    {
        game.tick();
    }

    assert!(
        game.state
            .entities
            .iter()
            .any(|entity| entity.owner == 1 && entity.kind == EntityKind::Golem),
        "Zamok should produce Golems"
    );
    assert_eq!(game.state.players[0].supply_used, 4);
}

#[test]
fn golem_mines_four_worker_loads_near_zamok() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let center = game.state.map.tile_center(10, 10);
    game.state
        .entities
        .spawn_building(1, EntityKind::Zamok, center.0, center.1, true)
        .expect("Zamok should spawn");
    let node = game
        .state
        .entities
        .spawn_node(EntityKind::Steel, center.0 + 96.0, center.1)
        .expect("steel node should spawn");
    let golem = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Golem, center.0 + 128.0, center.1)
        .expect("Golem should spawn");

    game.enqueue(
        1,
        Command::Gather {
            units: vec![golem],
            node,
            queued: false,
        },
    );
    for _ in 0..=config::HARVEST_TICKS + 2 {
        game.tick();
    }

    assert_eq!(game.state.players[0].steel, config::STEEL_LOAD * 4);
}

#[test]
fn golem_ordered_attack_deals_four_worker_damage() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let golem = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Golem, pos.0, pos.1)
        .expect("Golem should spawn");
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Worker, pos.0 + 20.0, pos.1)
        .expect("target worker should spawn");
    let target_hp = game.state.entities.get(target).expect("target exists").hp;
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    game.enqueue(
        1,
        Command::Attack {
            units: vec![golem],
            target,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(
        game.state.entities.get(target).expect("target exists").hp,
        target_hp - 16
    );
}

#[test]
fn ekat_consumes_nearby_golem_to_heal_to_full() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    let golem = game
        .state
        .entities
        .spawn_unit(
            1,
            EntityKind::Golem,
            pos.0 + config::TILE_SIZE as f32,
            pos.1,
        )
        .expect("Golem should spawn");
    game.state
        .entities
        .get_mut(hero)
        .expect("hero exists")
        .apply_damage(70, None);

    enqueue_ekat_consume_golem(&mut game, hero);
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    assert_eq!(hero_entity.hp, hero_entity.max_hp);
    assert!(
        game.state.entities.get(golem).is_none(),
        "consumed Golem should be removed permanently"
    );
}

#[test]
fn ekat_consume_requires_nearby_golem() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    let golem = game
        .state
        .entities
        .spawn_unit(
            1,
            EntityKind::Golem,
            pos.0 + config::TILE_SIZE as f32 * 4.0,
            pos.1,
        )
        .expect("Golem should spawn");
    game.state
        .entities
        .get_mut(hero)
        .expect("hero exists")
        .apply_damage(70, None);

    enqueue_ekat_consume_golem(&mut game, hero);
    game.tick();

    assert_eq!(game.state.entities.get(hero).expect("hero exists").hp, 80);
    assert!(
        game.state.entities.get(golem).is_some(),
        "out-of-range Golem should not be consumed"
    );
}

#[test]
fn ekat_dash_moves_up_to_five_tiles_leaves_return_marker_and_starts_cooldown() {
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
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - target.0).abs() < f32::EPSILON);
    assert!((hero_entity.pos_y - target.1).abs() < f32::EPSILON);
    assert_eq!(
        hero_entity.ability_cooldown_ticks(ability::AbilityKind::EkatTeleport),
        config::EKAT_TELEPORT_COOLDOWN_TICKS.saturating_sub(1)
    );
    let marker = game
        .state
        .ability_runtime
        .active_return_marker(1, hero, ability::AbilityKind::EkatTeleport, None, 1)
        .expect("dash should leave a return marker");
    assert!((marker.x - pos.0).abs() < f32::EPSILON);
    assert!((marker.y - pos.1).abs() < f32::EPSILON);
    assert_eq!(
        marker.expires_in(game.current_tick()),
        Some(config::EKAT_RETURN_MARKER_DURATION_TICKS as u16)
    );
}

#[test]
fn ekat_dash_far_target_clamps_to_max_range_without_staging_move() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let far_target = (pos.0 + config::TILE_SIZE as f32 * 20.0, pos.1);
    let expected_x = pos.0 + config::TILE_SIZE as f32 * config::EKAT_TELEPORT_RANGE_TILES as f32;
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, far_target);
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - expected_x).abs() < 0.001);
    assert!((hero_entity.pos_y - pos.1).abs() < 0.001);
    assert!(
        matches!(hero_entity.order(), Order::Idle),
        "out-of-range Dash should resolve immediately instead of staging a movement order"
    );
    assert!(active_return_marker_id(&game, hero).is_some());
}

#[test]
fn queued_ekat_abilities_append_future_world_ability_intents() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    {
        let hero_entity = game.state.entities.get_mut(hero).expect("hero exists");
        hero_entity.set_order(Order::move_to(pos.0 + config::TILE_SIZE as f32, pos.1));
        hero_entity.mark_move_phase(crate::game::entity::MovePhase::Moving);
    }

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatTeleport,
            units: vec![hero],
            x: Some(pos.0 + config::TILE_SIZE as f32 * 2.0),
            y: Some(pos.1),
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatLineShot,
            units: vec![hero],
            x: Some(pos.0 + config::TILE_SIZE as f32 * 3.0),
            y: Some(pos.1),
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatMagicAnchor,
            units: vec![hero],
            x: Some(pos.0),
            y: Some(pos.1 + config::TILE_SIZE as f32 * 2.0),
            queued: true,
        },
    );
    game.tick();

    let queued = game
        .state
        .entities
        .get(hero)
        .expect("hero exists")
        .queued_orders();
    assert_eq!(queued.len(), 3);
    assert!(matches!(
        queued[0],
        OrderIntent::WorldAbility(intent)
            if intent.ability == ability::AbilityKind::EkatTeleport
    ));
    assert!(matches!(
        queued[1],
        OrderIntent::WorldAbility(intent)
            if intent.ability == ability::AbilityKind::EkatLineShot
    ));
    assert!(matches!(
        queued[2],
        OrderIntent::WorldAbility(intent)
            if intent.ability == ability::AbilityKind::EkatMagicAnchor
    ));
}

#[test]
fn ekat_dash_rejects_invalid_landing_without_marker_or_cooldown() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(15, 10);
    let blocked_index = game.state.map.index(15, 10);
    game.state.map.terrain[blocked_index] = terrain::ROCK;
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - pos.0).abs() < f32::EPSILON);
    assert_eq!(
        hero_entity.ability_cooldown_ticks(ability::AbilityKind::EkatTeleport),
        0
    );
    assert!(active_return_marker_id(&game, hero).is_none());
}

#[test]
fn ekat_return_cannot_happen_in_same_tick_as_dash() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    enqueue_ekat_return(&mut game, hero, None);
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - target.0).abs() < f32::EPSILON);
    assert!(active_return_marker_id(&game, hero).is_some());
}

#[test]
fn ekat_return_recasts_to_marker_and_consumes_it_after_delay() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    let marker_id = active_return_marker_id(&game, hero).expect("return marker exists");
    enqueue_ekat_return(&mut game, hero, Some(marker_id));
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - pos.0).abs() < f32::EPSILON);
    assert!((hero_entity.pos_y - pos.1).abs() < f32::EPSILON);
    assert!(active_return_marker_id(&game, hero).is_none());
}

#[test]
fn ekat_return_fails_after_marker_expires() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    for _ in 0..config::EKAT_RETURN_MARKER_DURATION_TICKS {
        game.tick();
    }
    enqueue_ekat_return(&mut game, hero, None);
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - target.0).abs() < f32::EPSILON);
    assert!(active_return_marker_id(&game, hero).is_none());
}

#[test]
fn ekat_return_fails_when_marker_destination_is_blocked() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    let marker_id = active_return_marker_id(&game, hero).expect("return marker exists");
    let blocked_index = game.state.map.index(10, 10);
    game.state.map.terrain[blocked_index] = terrain::ROCK;
    enqueue_ekat_return(&mut game, hero, Some(marker_id));
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - target.0).abs() < f32::EPSILON);
    assert_eq!(active_return_marker_id(&game, hero), Some(marker_id));
}

#[test]
fn ekat_return_with_stale_caster_is_panic_free() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    game.state.entities.remove(hero);
    enqueue_ekat_return(&mut game, hero, None);
    game.tick();

    assert!(game.state.entities.get(hero).is_none());
}

#[test]
fn ekat_dash_return_marker_projection_respects_fog() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    game.state
        .entities
        .spawn_unit(2, EntityKind::Worker, target.0 + 500.0, target.1 + 500.0)
        .expect("enemy should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    let marker_id = active_return_marker_id(&game, hero).expect("return marker exists");

    assert!(game
        .snapshot_for(1)
        .ability_objects
        .iter()
        .any(|object| object.id == marker_id));
    assert!(!game
        .snapshot_for(2)
        .ability_objects
        .iter()
        .any(|object| object.id == marker_id));
}

#[test]
fn ekat_magic_anchor_places_single_visible_ten_second_object() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let first_target = game.state.map.tile_center(13, 10);
    let second_target = game.state.map.tile_center(14, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_anchor(&mut game, hero, first_target);
    game.tick();
    let first_anchor = active_anchor_id(&game, hero).expect("anchor should exist");
    let anchor = game
        .state
        .ability_runtime
        .active_anchor(
            1,
            hero,
            ability::AbilityKind::EkatMagicAnchor,
            game.current_tick(),
        )
        .expect("anchor should remain active");
    assert!((anchor.x - first_target.0).abs() < f32::EPSILON);
    assert_eq!(
        anchor.expires_in(game.current_tick()),
        Some(config::EKAT_MAGIC_ANCHOR_DURATION_TICKS as u16)
    );

    enqueue_ekat_anchor(&mut game, hero, second_target);
    game.tick();
    let replacement_anchor =
        active_anchor_id(&game, hero).expect("replacement anchor should exist");
    assert_ne!(first_anchor, replacement_anchor);
    assert_eq!(
        game.state
            .ability_runtime
            .world_objects()
            .filter(|object| object.kind == ability_runtime::AbilityWorldObjectKind::MagicAnchor)
            .count(),
        1
    );
}

#[test]
fn ekat_magic_anchor_rejects_out_of_range_or_locked_placement() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let far_target = game.state.map.tile_center(40, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_anchor(&mut game, hero, far_target);
    game.tick();
    assert!(active_anchor_id(&game, hero).is_none());

    let lockout_until = game.current_tick().saturating_add(30);
    let retry_target = game.state.map.tile_center(11, 10);
    game.state
        .entities
        .get_mut(hero)
        .expect("hero exists")
        .start_ability_lockout_until(ability::AbilityKind::EkatMagicAnchor, lockout_until);
    enqueue_ekat_anchor(&mut game, hero, retry_target);
    game.tick();
    assert!(active_anchor_id(&game, hero).is_none());
}

#[test]
fn ekat_magic_anchor_natural_expiry_does_not_lock_out_recast() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(12, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_anchor(&mut game, hero, target);
    game.tick();
    for _ in 0..config::EKAT_MAGIC_ANCHOR_DURATION_TICKS {
        game.tick();
    }
    assert!(active_anchor_id(&game, hero).is_none());
    assert!(game
        .state
        .entities
        .get(hero)
        .expect("hero exists")
        .ability_lockout_until_tick(ability::AbilityKind::EkatMagicAnchor, game.current_tick(),)
        .is_none());

    enqueue_ekat_anchor(&mut game, hero, target);
    game.tick();
    assert!(active_anchor_id(&game, hero).is_some());
}

#[test]
fn ekat_magic_anchor_is_not_attackable_by_enemy_weapons() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(12, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    game.state
        .entities
        .spawn_unit(
            2,
            EntityKind::Tank,
            target.0 + config::TILE_SIZE as f32 * 2.0,
            target.1,
        )
        .expect("enemy tank should spawn");
    refresh_visibility(&mut game);

    enqueue_ekat_anchor(&mut game, hero, target);
    game.tick();
    assert!(active_anchor_id(&game, hero).is_some());
    let far = game.state.map.tile_center(40, 40);
    game.state
        .entities
        .get_mut(hero)
        .expect("hero exists")
        .set_position(far.0, far.1);
    refresh_visibility(&mut game);
    for _ in 0..240 {
        game.tick();
    }

    assert!(
        active_anchor_id(&game, hero).is_some(),
        "enemy combat should ignore Magic Anchor instead of damaging it"
    );
    assert!(game
        .state
        .entities
        .get(hero)
        .expect("hero exists")
        .ability_lockout_until_tick(ability::AbilityKind::EkatMagicAnchor, game.current_tick())
        .is_none());
}

#[test]
fn ekat_magic_anchor_pull_field_slows_away_and_boosts_toward_movement() {
    let away_delta = ekat_anchor_move_delta(-6.0);
    let toward_delta = ekat_anchor_move_delta(14.0);

    assert!(
        away_delta < config::unit_stats(EntityKind::Ekat).unwrap().speed,
        "moving away from the anchor should be slowed, delta={away_delta}"
    );
    assert!(
        toward_delta > config::unit_stats(EntityKind::Ekat).unwrap().speed,
        "moving toward the anchor should be boosted, delta={toward_delta}"
    );
    assert!(
        toward_delta > away_delta,
        "toward movement should cover more distance than away movement"
    );
}

#[test]
fn ekat_magic_anchor_pulls_stationary_units_with_weight_resistance() {
    let players = [
        ekat_player(),
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Enemy".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let hero_pos = game.state.map.tile_center(8, 10);
    let anchor_target = game.state.map.tile_center(12, 10);
    let infantry_start = game.state.map.tile_center(10, 10);
    let tank_start = game.state.map.tile_center(10, 12);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, hero_pos.0, hero_pos.1)
        .expect("hero should spawn");
    let infantry = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, infantry_start.0, infantry_start.1)
        .expect("infantry should spawn");
    let tank = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, tank_start.0, tank_start.1)
        .expect("tank should spawn");

    enqueue_ekat_anchor(&mut game, hero, anchor_target);
    game.tick();
    game.tick();

    let infantry_delta = entity_distance_to(&game, infantry, infantry_start);
    let tank_delta = entity_distance_to(&game, tank, tank_start);

    assert!(
        infantry_delta > 0.01,
        "stationary infantry should be pulled by Magic Anchor, delta={infantry_delta}"
    );
    assert!(
        tank_delta > 0.01,
        "stationary tanks should still be pulled by Magic Anchor, delta={tank_delta}"
    );
    assert!(
        tank_delta < infantry_delta,
        "heavy units should receive less pull than infantry, infantry={infantry_delta}, tank={tank_delta}"
    );
}

fn ekat_anchor_move_delta(destination_tiles_from_start: f32) -> f32 {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let start = game.state.map.tile_center(10, 10);
    let anchor_target = game.state.map.tile_center(12, 10);
    let destination = (
        start.0 + destination_tiles_from_start * config::TILE_SIZE as f32,
        start.1,
    );
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, start.0, start.1)
        .expect("hero should spawn");

    enqueue_ekat_anchor(&mut game, hero, anchor_target);
    game.tick();
    game.enqueue(
        1,
        Command::Move {
            units: vec![hero],
            x: destination.0,
            y: destination.1,
            queued: false,
        },
    );
    game.tick();

    let hero_entity = game.state.entities.get(hero).expect("hero exists");
    (hero_entity.pos_x - start.0).abs()
}

#[test]
fn ekat_magic_anchor_projection_respects_fog_and_owner_lockout() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(12, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    game.state
        .entities
        .spawn_unit(2, EntityKind::Worker, target.0 + 500.0, target.1 + 500.0)
        .expect("enemy should spawn");
    refresh_visibility(&mut game);

    enqueue_ekat_anchor(&mut game, hero, target);
    game.tick();
    let anchor_id = active_anchor_id(&game, hero).expect("anchor exists");
    let owner_snapshot = game.snapshot_for(1);
    let hero_view = owner_snapshot
        .entities
        .iter()
        .find(|entity| entity.id == hero)
        .expect("hero projects");
    let anchor_affordance = hero_view
        .abilities
        .iter()
        .find(|ability| ability.ability == crate::protocol::abilities::EKAT_MAGIC_ANCHOR)
        .expect("anchor affordance projects");
    assert_eq!(anchor_affordance.active_object_id, Some(anchor_id));
    assert!(anchor_affordance.expires_in.is_some());
    assert!(owner_snapshot
        .ability_objects
        .iter()
        .any(|object| object.id == anchor_id && object.owner_state.is_some()));
    assert!(!game
        .snapshot_for(2)
        .ability_objects
        .iter()
        .any(|object| object.id == anchor_id));

    let lockout_until = game.current_tick().saturating_add(100);
    game.state
        .entities
        .get_mut(hero)
        .expect("hero exists")
        .start_ability_lockout_until(ability::AbilityKind::EkatMagicAnchor, lockout_until);
    let owner_snapshot = game.snapshot_for(1);
    let hero_view = owner_snapshot
        .entities
        .iter()
        .find(|entity| entity.id == hero)
        .expect("hero projects");
    assert!(hero_view.abilities.iter().any(|ability| ability.ability
        == crate::protocol::abilities::EKAT_MAGIC_ANCHOR
        && ability.lockout_until_tick.is_some()));
}

#[test]
fn ekat_magic_anchor_with_stale_caster_is_panic_free() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(12, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_anchor(&mut game, hero, target);
    game.tick();
    assert!(active_anchor_id(&game, hero).is_some());
    game.state.entities.remove(hero);
    game.tick();
    assert!(game.state.ability_runtime.world_objects().next().is_none());
}

#[test]
fn ekat_line_shot_spawns_moving_projectile_and_starts_cooldown_without_immediate_damage() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 6.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    let enemy = game
        .state
        .entities
        .spawn_unit(
            2,
            EntityKind::Rifleman,
            pos.0 + config::TILE_SIZE as f32 * 3.0,
            pos.1,
        )
        .expect("enemy should spawn");
    let ally = game
        .state
        .entities
        .spawn_unit(
            1,
            EntityKind::Rifleman,
            pos.0 + config::TILE_SIZE as f32 * 4.0,
            pos.1,
        )
        .expect("ally should spawn");

    enqueue_ekat_line_shot(&mut game, hero, target);
    game.tick();

    let projectile = game
        .state
        .ability_runtime
        .world_objects()
        .find(|object| object.ability == ability::AbilityKind::EkatLineShot)
        .expect("line shot should spawn a projected ability object");
    assert_eq!(
        projectile.kind,
        ability_runtime::AbilityWorldObjectKind::LineProjectile
    );
    assert!(
        projectile.x > pos.0 && projectile.x < target.0,
        "projectile should move out from Ekat instead of applying instant full-line damage"
    );
    assert_eq!(
        line_projectiles(&game).len(),
        1,
        "line shot without an anchor should keep the Phase 7 single-origin behavior"
    );
    assert_eq!(
        game.state.entities.get(enemy).expect("enemy exists").hp,
        game.state.entities.get(enemy).expect("enemy exists").max_hp
    );
    assert_eq!(game.state.entities.get(ally).expect("ally exists").hp, 45);
    assert_eq!(
        game.state
            .entities
            .get(hero)
            .expect("hero exists")
            .ability_cooldown_ticks(ability::AbilityKind::EkatLineShot),
        config::EKAT_LINE_SHOT_COOLDOWN_TICKS.saturating_sub(1)
    );
}

#[test]
fn ekat_line_shot_with_active_anchor_launches_from_both_origins_once() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let hero_pos = game.state.map.tile_center(10, 10);
    let anchor_pos = game.state.map.tile_center(10, 13);
    let target = game.state.map.tile_center(17, 10);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, hero_pos.0, hero_pos.1)
        .expect("hero should spawn");

    enqueue_ekat_anchor(&mut game, hero, anchor_pos);
    game.tick();
    let anchor_id = active_anchor_id(&game, hero).expect("anchor should exist");

    enqueue_ekat_line_shot(&mut game, hero, target);
    game.tick();

    let projectiles = line_projectiles(&game);
    assert_eq!(projectiles.len(), 2);
    assert!(
        projectiles
            .iter()
            .any(|projectile| projectile.source_object_id.is_none()),
        "hero-origin projectile should launch from Ekat"
    );
    assert!(
        projectiles
            .iter()
            .any(|projectile| projectile.source_object_id == Some(anchor_id)),
        "anchor-origin projectile should carry the source anchor id"
    );
    assert_eq!(
        game.state
            .entities
            .get(hero)
            .expect("hero exists")
            .ability_cooldown_ticks(ability::AbilityKind::EkatLineShot),
        config::EKAT_LINE_SHOT_COOLDOWN_TICKS.saturating_sub(1),
        "dual-origin launch should start one line-shot cooldown"
    );
}

#[test]
fn ekat_line_shot_ignores_expired_anchors() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut expired_game = empty_flat_game(&players);
    let pos = expired_game.state.map.tile_center(10, 10);
    let anchor_pos = expired_game.state.map.tile_center(12, 10);
    let target = expired_game.state.map.tile_center(17, 10);
    let expired_hero = expired_game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_anchor(&mut expired_game, expired_hero, anchor_pos);
    expired_game.tick();
    for _ in 0..config::EKAT_MAGIC_ANCHOR_DURATION_TICKS {
        expired_game.tick();
    }
    assert!(active_anchor_id(&expired_game, expired_hero).is_none());
    enqueue_ekat_line_shot(&mut expired_game, expired_hero, target);
    expired_game.tick();
    assert_eq!(line_projectiles(&expired_game).len(), 1);
}

#[test]
fn ekat_anchor_line_projectiles_have_independent_hit_dedupe_and_return_to_ekat() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let hero_pos = game.state.map.tile_center(10, 10);
    let anchor_pos = game.state.map.tile_center(10, 12);
    let target = game.state.map.tile_center(16, 10);
    let dash_target = game.state.map.tile_center(10, 15);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, hero_pos.0, hero_pos.1)
        .expect("hero should spawn");
    let enemy = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::Depot,
            game.state.map.tile_center(13, 10).0,
            game.state.map.tile_center(13, 10).1,
            true,
        )
        .expect("enemy should spawn");
    let enemy_max_hp = game.state.entities.get(enemy).expect("enemy exists").max_hp;

    enqueue_ekat_anchor(&mut game, hero, anchor_pos);
    game.tick();
    enqueue_ekat_line_shot(&mut game, hero, target);
    for _ in 0..23 {
        game.tick();
    }
    assert!(
        game.state.entities.get(enemy).expect("enemy exists").hp
            <= enemy_max_hp.saturating_sub(config::EKAT_LINE_SHOT_DAMAGE * 2),
        "the same target may be hit by both simultaneous origin projectiles"
    );

    enqueue_ekat_dash(&mut game, hero, dash_target);
    game.tick();
    for _ in 0..18 {
        game.tick();
    }

    assert!(
        line_projectiles(&game)
            .iter()
            .any(|projectile| projectile.y > hero_pos.1),
        "returning anchor and hero projectiles should steer toward Ekat's current position"
    );
}

#[test]
fn ekat_anchor_origin_projectile_clamps_from_anchor_and_stays_fog_filtered() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let hero_pos = game.state.map.tile_center(10, 10);
    let anchor_pos = game.state.map.tile_center(10, 15);
    let far_target = game.state.map.tile_center(40, 15);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, hero_pos.0, hero_pos.1)
        .expect("hero should spawn");
    game.state
        .entities
        .spawn_unit(
            2,
            EntityKind::Worker,
            far_target.0 + 400.0,
            far_target.1 + 400.0,
        )
        .expect("distant enemy should spawn");
    refresh_visibility(&mut game);

    enqueue_ekat_anchor(&mut game, hero, anchor_pos);
    game.tick();
    let anchor_id = active_anchor_id(&game, hero).expect("anchor exists");
    enqueue_ekat_line_shot(&mut game, hero, far_target);
    game.tick();

    let projectiles = line_projectiles(&game);
    let anchor_projectile = projectiles
        .iter()
        .find(|projectile| projectile.source_object_id == Some(anchor_id))
        .expect("anchor projectile exists");
    let max_range = config::EKAT_LINE_SHOT_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let endpoint_distance = ((anchor_projectile.endpoint.0 - anchor_pos.0).powi(2)
        + (anchor_projectile.endpoint.1 - anchor_pos.1).powi(2))
    .sqrt();
    assert!(
        endpoint_distance <= max_range + 0.01,
        "anchor-origin projectile should clamp endpoint from the anchor origin"
    );
    assert!(game
        .snapshot_for(1)
        .ability_objects
        .iter()
        .any(|object| object.id == anchor_projectile.id.get()));
    assert!(
        game.snapshot_for(2).ability_objects.is_empty(),
        "hidden anchor-origin projectiles should not leak through enemy snapshots"
    );
}

#[test]
fn ekat_line_shot_hits_enemies_on_outbound_and_return_legs() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 6.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    let enemy = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::Depot,
            pos.0 + config::TILE_SIZE as f32 * 5.0,
            pos.1,
            true,
        )
        .expect("enemy should spawn");
    let enemy_max_hp = game.state.entities.get(enemy).expect("enemy exists").max_hp;

    enqueue_ekat_line_shot(&mut game, hero, target);
    for _ in 0..23 {
        game.tick();
    }
    let after_outbound = game.state.entities.get(enemy).expect("enemy exists").hp;
    assert!(
        after_outbound <= enemy_max_hp.saturating_sub(config::EKAT_LINE_SHOT_DAMAGE),
        "outbound leg should damage the enemy"
    );
    for _ in 0..25 {
        game.tick();
    }
    assert!(
        game.state.entities.get(enemy).expect("enemy exists").hp
            <= after_outbound.saturating_sub(config::EKAT_LINE_SHOT_DAMAGE),
        "return leg should damage the enemy again"
    );
}

#[test]
fn ekat_line_shot_endpoint_clamps_to_range() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let far_target = (pos.0 + config::TILE_SIZE as f32 * 20.0, pos.1);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    let inside_range_enemy = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::Depot,
            pos.0 + config::TILE_SIZE as f32 * 5.5,
            pos.1,
            true,
        )
        .expect("inside range enemy should spawn");
    let beyond_range_enemy = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::Depot,
            pos.0 + config::TILE_SIZE as f32 * 8.0,
            pos.1,
            true,
        )
        .expect("beyond range enemy should spawn");
    let inside_max_hp = game
        .state
        .entities
        .get(inside_range_enemy)
        .expect("inside range enemy exists")
        .max_hp;
    let beyond_max_hp = game
        .state
        .entities
        .get(beyond_range_enemy)
        .expect("beyond range enemy exists")
        .max_hp;

    enqueue_ekat_line_shot(&mut game, hero, far_target);
    for _ in 0..23 {
        game.tick();
    }

    assert!(
        game.state
            .entities
            .get(inside_range_enemy)
            .expect("inside range enemy exists")
            .hp
            <= inside_max_hp.saturating_sub(config::EKAT_LINE_SHOT_DAMAGE),
        "clamped endpoint should allow targets inside range to be hit"
    );
    assert_eq!(
        game.state
            .entities
            .get(beyond_range_enemy)
            .expect("beyond range enemy exists")
            .hp,
        beyond_max_hp
    );
}

#[test]
fn ekat_line_shot_return_tracks_ekats_current_position_after_dash() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 6.0, pos.1);
    let dash_target = (pos.0, pos.1 + config::TILE_SIZE as f32 * 5.0);
    let hero = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_line_shot(&mut game, hero, target);
    game.tick();
    enqueue_ekat_dash(&mut game, hero, dash_target);
    game.tick();
    for _ in 0..24 {
        game.tick();
    }

    let projectile = game
        .state
        .ability_runtime
        .world_objects()
        .find(|object| object.ability == ability::AbilityKind::EkatLineShot)
        .expect("line shot should still be returning");
    assert!(
        projectile.y > pos.1,
        "returning projectile should bend toward Ekat's dashed position"
    );
}
