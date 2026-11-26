use super::fixtures::*;
use super::*;
use crate::game::ability::AbilityKind;
use crate::game::entity::BuildPhase;

fn setup(faction: &str, steel: u32, oil: u32) -> (Game, u32) {
    let mut game = empty_flat_game(&[PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: faction.into(),
        name: "Builder".into(),
        color: "#fff".into(),
        is_ai: false,
    }]);
    game.state.players[0].set_resources(steel, oil);
    let (x, y) = game.state.map.tile_center(9, 10);
    let worker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, x, y)
        .unwrap();
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    game.enqueue(
        1,
        Command::Build {
            units: vec![worker],
            building: EntityKind::Shop,
            tile_x: 10,
            tile_y: 10,
            queued: false,
        },
    );
    (game, worker)
}

fn start(game: &mut Game, worker: u32) -> u32 {
    for _ in 0..120 {
        game.tick();
        if let Some(BuildPhase::Constructing { site }) =
            game.state.entities.get(worker).unwrap().build_phase()
        {
            return site;
        }
    }
    panic!("Serf must start the Shop");
}

#[test]
fn shop_costs_both_resources_and_finishes_after_600_work_ticks() {
    let (mut game, worker) = setup("cultivators", 100, 100);
    let site = start(&mut game, worker);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
    let building = game.state.entities.get(site).unwrap();
    assert_eq!(building.max_hp, 200);
    assert_eq!(building.construction.as_ref().unwrap().total, 600);
    let progress = building.construction.as_ref().unwrap().progress;
    for _ in progress..599 {
        game.tick();
    }
    assert!(game.state.entities.get(site).unwrap().under_construction());
    game.tick();
    let building = game.state.entities.get(site).unwrap();
    assert!(!building.under_construction());
    assert_eq!(building.hp, 200);
    assert!(building.production.is_none());
    assert!(building.combat.is_none());
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert_eq!(
        restored.state.entities.get(site).unwrap().kind,
        EntityKind::Shop
    );
    game.state
        .entities
        .get_mut(site)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    assert!(game.state.entities.get(site).is_none());
}

#[test]
fn shop_waits_for_either_missing_resource_and_rejects_other_factions() {
    for (faction, steel, oil) in [
        ("cultivators", 99, 100),
        ("cultivators", 100, 99),
        ("kriegsia", 1000, 1000),
    ] {
        let (mut game, _) = setup(faction, steel, oil);
        for _ in 0..120 {
            game.tick();
        }
        assert!(!game
            .state
            .entities
            .iter()
            .any(|e| e.kind == EntityKind::Shop));
        assert_eq!(
            (game.state.players[0].steel, game.state.players[0].oil),
            (steel, oil)
        );
    }
}

#[test]
fn shop_cancellation_uses_normal_construction_refund() {
    let (mut game, worker) = setup("cultivators", 100, 100);
    let site = start(&mut game, worker);
    game.enqueue(
        1,
        Command::Cancel {
            building: site,
            construction: true,
        },
    );
    game.tick();
    assert!(game.state.entities.get(site).is_none());
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (100, 100)
    );
}

fn item_fixture() -> (Game, u32, u32) {
    item_fixture_for_kind(EntityKind::Warrior)
}

fn item_fixture_for_kind(kind: EntityKind) -> (Game, u32, u32) {
    let mut game = empty_flat_game(&[PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "cultivators".into(),
        name: "Shopper".into(),
        color: "#fff".into(),
        is_ai: false,
    }]);
    game.state.players[0].set_resources(0, 1000);
    let shop = game
        .state
        .entities
        .spawn_building(1, EntityKind::Shop, 320.0, 320.0, true)
        .unwrap();
    let unit = game
        .state
        .entities
        .spawn_unit(1, kind, 640.0, 320.0)
        .unwrap();
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    (game, shop, unit)
}

fn buy(game: &mut Game, shop: u32, target: u32, ability: AbilityKind) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability,
            units: vec![shop],
            x: None,
            y: None,
            target_id: Some(target),
            queued: false,
        },
    );
}

#[test]
fn regen_costs_50_and_arrives_after_ten_tiles_of_flight() {
    let (mut game, shop, unit) = item_fixture();
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .apply_damage(40, None);
    let hp = game.state.entities.get(unit).unwrap().hp;
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    assert_eq!(game.state.players[0].oil, 950);
    let object = game.state.ability_runtime.world_objects().next().unwrap();
    assert!((object.x.to_world() - (320.0 + 8.0)).abs() < 0.01);
    for _ in 1..39 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(unit).unwrap().hp, hp);
    game.tick();
    let target = game.state.entities.get(unit).unwrap();
    assert!(target.hp <= hp + 1, "delivery must not instantly heal");
    assert!(target.shop_item_active(AbilityKind::Regen));
    let before = target.hp;
    for _ in 0..config::TICK_HZ {
        game.tick();
    }
    assert_eq!(game.state.entities.get(unit).unwrap().hp, before + 3);
    for _ in 0..config::TICK_HZ * 20 {
        game.tick();
    }
    let target = game.state.entities.get(unit).unwrap();
    assert_eq!(target.hp, target.max_hp, "Regen caps at maximum HP");
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
}

#[test]
fn regen_has_unlimited_range_and_tracks_a_moving_target_until_arrival() {
    let (mut game, shop, unit) = item_fixture();
    let target = game.state.entities.get_mut(unit).unwrap();
    target.set_position(1600.0, 320.0); // Forty tiles from the Shop.
    target.apply_damage(target.max_hp - 1, None);
    let hp = target.hp;
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    assert!(hp > 0);
    assert!(game.state.fog.is_visible_world(1, 1600.0, 320.0));
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    assert_eq!(game.state.players[0].oil, 950);
    assert_eq!(
        game.state
            .ability_runtime
            .projectiles()
            .next()
            .unwrap()
            .expires_tick,
        u32::MAX
    );
    // Regen follows the unit rather than healing its original location.
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .set_position(1920.0, 320.0);
    for _ in 1..199 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(unit).unwrap().hp, hp);
    game.tick();
    let target = game.state.entities.get(unit).unwrap();
    assert!(target.hp <= hp + 1);
    assert!(target.shop_item_active(AbilityKind::Regen));
    assert_eq!(game.state.ability_runtime.projectiles().count(), 0);
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
}

#[test]
fn regen_disappears_without_refund_when_target_dies_or_is_removed() {
    for remove in [false, true] {
        let (mut game, shop, unit) = item_fixture();
        buy(&mut game, shop, unit, AbilityKind::Regen);
        game.tick();
        assert_eq!(game.state.ability_runtime.projectiles().count(), 1);
        if remove {
            game.state.entities.remove(unit);
        } else {
            game.state
                .entities
                .get_mut(unit)
                .unwrap()
                .apply_damage(u32::MAX, None);
        }
        game.tick();
        assert!(game.state.entities.get(unit).is_none());
        assert_eq!(game.state.players[0].oil, 950);
        assert_eq!(game.state.ability_runtime.projectiles().count(), 0);
        assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
    }
}

#[test]
fn shop_items_reject_buildings_hidden_stale_and_enemy_targets_without_spending() {
    let (mut game, shop, unit) = item_fixture();
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 640.0, 384.0)
        .unwrap();
    for target in [shop, enemy, u32::MAX] {
        buy(&mut game, shop, target, AbilityKind::Regen);
        game.tick();
    }
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .set_position(1400.0, 320.0);
    buy(&mut game, shop, unit, AbilityKind::EscapeTreasure);
    game.tick();
    assert_eq!(game.state.players[0].oil, 1000);
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
}

#[test]
fn escape_treasure_is_delivered_once_checkpointed_and_saves_a_lethal_unit_anywhere() {
    let (mut game, shop, unit) = item_fixture();
    buy(&mut game, shop, unit, AbilityKind::EscapeTreasure);
    game.tick();
    buy(&mut game, shop, unit, AbilityKind::EscapeTreasure);
    for _ in 1..41 {
        game.tick();
    }
    assert_eq!(game.state.players[0].oil, 900);
    assert_eq!(
        game.state.entities.get(unit).unwrap().escape_treasure,
        Some(shop)
    );
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    let target = game.state.entities.get_mut(unit).unwrap();
    target.set_position(2000.0, 2000.0);
    target.apply_damage(u32::MAX, None);
    game.tick();
    let target = game.state.entities.get(unit).unwrap();
    assert_eq!(target.hp, 1);
    assert_eq!(target.escape_treasure, None);
    assert!((target.pos_x() - 320.0).abs() < 200.0);
    assert!((target.pos_y() - 320.0).abs() < 200.0);
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .apply_damage(1, None);
    game.tick();
    assert!(game.state.entities.get(unit).is_none());
}

#[test]
fn destroyed_shop_cancels_its_escape_treasure() {
    let (mut game, shop, unit) = item_fixture();
    game.state.entities.get_mut(unit).unwrap().escape_treasure = Some(shop);
    game.state
        .entities
        .get_mut(shop)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    assert!(game.state.entities.get(unit).is_none());
}

#[test]
fn in_flight_shop_item_checkpoint_continues_identically() {
    let (mut game, shop, unit) = item_fixture();
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .apply_damage(80, None);
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    for _ in 0..60 {
        game.tick();
        restored.tick();
        assert_eq!(
            game.replay_end_state_hash().unwrap(),
            restored.replay_end_state_hash().unwrap()
        );
        assert_eq!(
            serde_json::to_value(game.snapshot_full_for(1)).unwrap(),
            serde_json::to_value(restored.snapshot_full_for(1)).unwrap()
        );
    }
}

#[test]
fn escaping_worker_releases_mining_slot_and_movement_state() {
    use crate::game::entity::{GatherPhase, Order};
    let (mut game, shop, _) = item_fixture();
    let worker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, 1000.0, 1000.0)
        .unwrap();
    let node = game
        .state
        .entities
        .spawn_node(EntityKind::Oil, 1040.0, 1000.0)
        .unwrap();
    let target = game.state.entities.get_mut(worker).unwrap();
    target.set_order(Order::gather(node));
    target.mark_gather_phase(GatherPhase::Harvesting);
    target.set_path_goal(Some((1000.0, 1000.0)));
    target.escape_treasure = Some(shop);
    assert!(game.state.entities.claim_miner(node, worker));
    game.state
        .entities
        .get_mut(worker)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    let target = game.state.entities.get(worker).unwrap();
    assert_eq!(target.hp, 1);
    assert!(matches!(target.order(), Order::Idle));
    assert!(target.path_goal().is_none());
    assert_eq!(game.state.entities.get(node).unwrap().miner(), None);
}

#[test]
fn shop_purchase_accepts_allies_and_checks_visibility_range_money_and_source() {
    let (mut game, shop, unit) = item_fixture();
    let ally_player = crate::game::PlayerState {
        id: 2,
        ..game.state.players[0].clone()
    };
    game.state.players.push(ally_player);
    let ally = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 700.0, 320.0)
        .unwrap();
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    buy(&mut game, shop, ally, AbilityKind::EscapeTreasure);
    game.tick();
    assert_eq!(game.state.players[0].oil, 900);
    for _ in 0..60 {
        game.tick();
    }
    assert_eq!(
        game.state.entities.get(ally).unwrap().escape_treasure,
        Some(shop)
    );

    // A friendly target behind unrecomputed fog cannot be used to probe hidden positions.
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .set_position(1248.0, 320.0);
    game.state.fog = crate::game::fog::Fog::new(game.state.map.width, game.state.map.height);
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    assert_eq!(game.state.players[0].oil, 900);

    // Exactly thirty tiles is accepted after the unit's sight is recomputed.
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .set_position(1280.0, 320.0);
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    assert_eq!(game.state.players[0].oil, 850);
    game.state.players[0].set_resources(0, 49);
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    assert_eq!(game.state.players[0].oil, 49);

    game.state.players[0].set_resources(0, 1000);
    let unfinished = game
        .state
        .entities
        .spawn_building(1, EntityKind::Shop, 1200.0, 600.0, false)
        .unwrap();
    let foreign = game
        .state
        .entities
        .spawn_building(2, EntityKind::Shop, 1200.0, 800.0, true)
        .unwrap();
    for source in [unfinished, foreign] {
        buy(&mut game, source, unit, AbilityKind::Regen);
        game.tick();
    }
    assert_eq!(game.state.players[0].oil, 1000);
}

#[test]
fn escape_search_finds_distant_clearings_at_both_grid_alignments() {
    // Workers fit a single tile; the larger Novice needs the center of a 2-by-2 area.
    for (kind, width) in [(EntityKind::Worker, 1), (EntityKind::Warrior, 2)] {
        let (mut game, shop, unit) = item_fixture();
        game.state.entities.remove(unit);
        let unit = game
            .state
            .entities
            .spawn_unit(1, kind, 640.0, 320.0)
            .unwrap();
        // Shop centers can lie on tile boundaries (the real footprint is 3 by 4 tiles).
        // Neither the Shop-aligned grid nor the tile-center grid alone covers both exits.
        game.state.map.terrain.fill(crate::protocol::terrain::ROCK);
        for y in 30..30 + width {
            for x in 30..30 + width {
                let clearing = game.state.map.index(x, y);
                game.state.map.terrain[clearing] = crate::protocol::terrain::GRASS;
            }
        }
        let center = (30.0 + width as f32 / 2.0) * config::TILE_SIZE as f32;
        assert!(
            crate::game::services::standability::unit_spawn_standable_for_entity(
                &game.state.map,
                &crate::game::services::occupancy::Occupancy::build(
                    &game.state.map,
                    &game.state.entities
                ),
                &game.state.entities,
                game.state.entities.get(unit).unwrap(),
                center,
                center,
            )
        );
        let target = game.state.entities.get_mut(unit).unwrap();
        target.escape_treasure = Some(shop);
        target.apply_damage(u32::MAX, None);

        game.tick();

        let target = game
            .state
            .entities
            .get(unit)
            .expect("legal clearing rescues unit");
        assert_eq!(target.hp, 1);
        assert_eq!((target.pos_x(), target.pos_y()), (center, center));
        assert_eq!(target.escape_treasure, None);
    }
}

#[test]
fn escape_manual_use_preserves_hp_and_consumes_once_even_with_autocast_off() {
    let (mut game, shop, unit) = item_fixture();
    buy(&mut game, shop, unit, AbilityKind::EscapeTreasure);
    for _ in 0..41 {
        game.tick();
    }
    game.enqueue(
        1,
        Command::SetAutocast {
            ability: AbilityKind::EscapeTreasure,
            units: vec![unit],
            enabled: false,
        },
    );
    game.tick();
    assert!(
        game.state
            .entities
            .get(unit)
            .unwrap()
            .escape_autocast_disabled
    );
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert!(
        game.state
            .entities
            .get(unit)
            .unwrap()
            .escape_autocast_disabled
    );
    let target = game.state.entities.get_mut(unit).unwrap();
    target.apply_damage(target.max_hp - 40, None);
    target.set_position(2000.0, 2000.0);
    game.enqueue(
        1,
        Command::UseAbility {
            ability: AbilityKind::EscapeTreasure,
            units: vec![unit],
            x: None,
            y: None,
            target_id: None,
            queued: false,
        },
    );
    game.tick();
    let target = game.state.entities.get(unit).unwrap();
    assert_eq!(target.hp, 40);
    assert_eq!(target.escape_treasure, None);
    assert!((target.pos_x() - 320.0).abs() < 200.0);
    assert_eq!(game.state.players[0].oil, 900);
}

#[test]
fn escape_autocast_can_be_disabled_and_reenabled_only_by_owner() {
    for reenable in [false, true] {
        let (mut game, shop, unit) = item_fixture();
        game.state.entities.get_mut(unit).unwrap().escape_treasure = Some(shop);
        game.enqueue(
            1,
            Command::SetAutocast {
                ability: AbilityKind::EscapeTreasure,
                units: vec![unit],
                enabled: false,
            },
        );
        game.tick();
        game.enqueue(
            2,
            Command::SetAutocast {
                ability: AbilityKind::EscapeTreasure,
                units: vec![unit],
                enabled: true,
            },
        );
        game.enqueue(
            2,
            Command::UseAbility {
                ability: AbilityKind::EscapeTreasure,
                units: vec![unit],
                x: None,
                y: None,
                target_id: None,
                queued: false,
            },
        );
        game.tick();
        let target = game.state.entities.get(unit).unwrap();
        assert_eq!(target.escape_treasure, Some(shop));
        assert!(target.escape_autocast_disabled);
        if reenable {
            game.enqueue(
                1,
                Command::SetAutocast {
                    ability: AbilityKind::EscapeTreasure,
                    units: vec![unit],
                    enabled: true,
                },
            );
            game.tick();
        }
        game.state
            .entities
            .get_mut(unit)
            .unwrap()
            .apply_damage(u32::MAX, None);
        game.tick();
        assert_eq!(game.state.entities.get(unit).is_some(), reenable);
    }
}

fn activate_item(game: &mut Game, unit: u32, ability: AbilityKind) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability,
            units: vec![unit],
            x: None,
            y: None,
            target_id: None,
            queued: false,
        },
    );
}

#[test]
fn carried_items_are_passive_and_permanent_after_checkpoint() {
    let (mut game, shop, unit) = item_fixture_for_kind(EntityKind::WindAdept);
    buy(&mut game, shop, unit, AbilityKind::Haste);
    buy(&mut game, shop, unit, AbilityKind::DoubleDamage);
    buy(&mut game, shop, unit, AbilityKind::ArmorTreasure);
    for _ in 0..42 {
        game.tick();
    }
    assert_eq!(game.state.players[0].oil, 650);
    let target = game.state.entities.get(unit).unwrap();
    assert_eq!(
        target.ability_uses_remaining.get(&AbilityKind::Haste),
        Some(&1)
    );
    assert_eq!(
        target
            .ability_uses_remaining
            .get(&AbilityKind::ArmorTreasure),
        Some(&1)
    );
    assert!(target.shop_item_active(AbilityKind::DoubleDamage));
    let enemy_view = game.snapshot_for(2);
    assert!(enemy_view.entities.iter().all(|e| e.id != unit
        || e.abilities
            .iter()
            .all(|a| a.ability != "haste" && a.ability != "armorTreasure")));
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    activate_item(&mut game, unit, AbilityKind::Haste);
    activate_item(&mut game, unit, AbilityKind::ArmorTreasure);
    game.tick();
    let target = game.state.entities.get_mut(unit).unwrap();
    assert!(target.shop_item_active(AbilityKind::Haste));
    assert!(target.shop_item_active(AbilityKind::ArmorTreasure));
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert!(restored
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::Haste));
    assert!(restored
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::ArmorTreasure));
    let target = game.state.entities.get_mut(unit).unwrap();
    let hp = target.hp;
    target.apply_damage(target.damage_after_armor(20, 0.0), None);
    assert_eq!(target.hp, hp - 10);
    buy(&mut game, shop, unit, AbilityKind::DoubleDamage);
    game.tick();
    assert_eq!(game.state.players[0].oil, 650);
    for _ in 0..900 {
        game.tick();
    }
    assert!(game
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::Haste));
    buy(&mut game, shop, unit, AbilityKind::DoubleDamage);
    for _ in 0..42 {
        game.tick();
    }
    assert_eq!(game.state.players[0].oil, 650);
    assert!(game
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::DoubleDamage));
}

#[test]
fn escape_purchase_cooldown_is_shared_by_old_and_new_shops_and_checkpointed() {
    let (mut game, shop, unit) = item_fixture();
    let second = game
        .state
        .entities
        .spawn_building(1, EntityKind::Shop, 960.0, 320.0, true)
        .unwrap();
    let other = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 680.0, 320.0)
        .unwrap();
    buy(&mut game, shop, unit, AbilityKind::EscapeTreasure);
    buy(&mut game, second, other, AbilityKind::EscapeTreasure);
    game.tick();
    assert_eq!(game.state.players[0].oil, 900);
    assert!(game.state.players[0].ability_cooldowns[&AbilityKind::EscapeTreasure] >= 1799);
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    let third = game
        .state
        .entities
        .spawn_building(1, EntityKind::Shop, 960.0, 640.0, true)
        .unwrap();
    buy(&mut game, third, other, AbilityKind::EscapeTreasure);
    game.tick();
    assert_eq!(game.state.players[0].oil, 900);
    let view = game.snapshot_for(1);
    for id in [shop, second, third] {
        assert!(view
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .abilities
            .iter()
            .any(|a| a.ability == "escapeTreasure" && a.cooldown_left > 0));
    }
    for _ in 0..1800 {
        game.tick();
    }
    buy(&mut game, third, other, AbilityKind::EscapeTreasure);
    game.tick();
    assert_eq!(game.state.players[0].oil, 800);
}

#[test]
fn vitality_treasure_is_passive_on_delivery_and_permanent_after_checkpoint() {
    let (mut game, shop, unit) = item_fixture_for_kind(EntityKind::WindAdept);
    let target = game.state.entities.get_mut(unit).unwrap();
    let base = target.max_hp;
    target.apply_damage(20, None);
    buy(&mut game, shop, unit, AbilityKind::VitalityTreasure);
    buy(&mut game, shop, unit, AbilityKind::ArmorTreasure);
    for _ in 0..42 {
        game.tick();
    }
    assert_eq!(
        game.state.players[0].oil, 800,
        "distinct defense items can be purchased together"
    );
    let target = game.state.entities.get(unit).unwrap();
    assert_eq!((target.hp, target.max_hp), (base + 130, base + 150));
    activate_item(&mut game, unit, AbilityKind::VitalityTreasure);
    game.tick();
    let target = game.state.entities.get_mut(unit).unwrap();
    assert_eq!((target.hp, target.max_hp), (base + 130, base + 150));
    target.apply_damage(140, None);
    let wounded = target.hp;
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    let target = game.state.entities.get(unit).unwrap();
    assert_eq!((target.hp, target.max_hp), (wounded, base + 150));
    buy(&mut game, shop, unit, AbilityKind::ArmorTreasure);
    game.tick();
    assert_eq!(game.state.players[0].oil, 800);
    for _ in 0..900 {
        game.tick();
    }
    let target = game.state.entities.get(unit).unwrap();
    assert_eq!((target.hp, target.max_hp), (wounded, base + 150));
    buy(&mut game, shop, unit, AbilityKind::VitalityTreasure);
    buy(&mut game, shop, unit, AbilityKind::ArmorTreasure);
    for _ in 0..42 {
        game.tick();
    }
    assert_eq!(game.state.players[0].oil, 800);
    let target = game.state.entities.get(unit).unwrap();
    assert_eq!((target.hp, target.max_hp), (wounded, base + 150));
    game.state
        .entities
        .get_mut(shop)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    assert_eq!(game.state.entities.get(unit).unwrap().max_hp, base + 150);
}

#[test]
fn vitality_treasure_keeps_a_permanent_flat_bonus_through_dao_advancement() {
    let mut unit =
        crate::game::entity::Entity::new_unit(1, EntityKind::Warrior, 320.0, 320.0).unwrap();
    unit.receive_shop_item(AbilityKind::VitalityTreasure);
    let base = config::unit_stats(EntityKind::Warrior).unwrap().hp;
    assert_eq!((unit.hp, unit.max_hp), (base + 150, base + 150));
    unit.insight = config::DISCIPLE_INSIGHT_REQUIRED;
    assert!(unit.evolve_dao(AbilityKind::EvolveWar));
    let adept = config::unit_stats(EntityKind::WarAdept).unwrap().hp;
    assert_eq!((unit.hp, unit.max_hp), (adept + 150, adept + 150));
    unit.insight = config::DISCIPLE_EVOLVED_INSIGHT_CAP;
    unit.advance_dao_master();
    let master = ((adept as f32) * config::DAO_MASTER_HP_MULTIPLIER).round() as u32;
    assert_eq!((unit.hp, unit.max_hp), (master + 150, master + 150));
    for _ in 0..900 {
        unit.tick_ability_cooldowns();
    }
    assert_eq!((unit.hp, unit.max_hp), (master + 150, master + 150));
}

#[test]
fn armor_treasure_applies_on_delivery_without_activation_or_expiry() {
    let (mut game, shop, unit) = item_fixture();
    buy(&mut game, shop, unit, AbilityKind::ArmorTreasure);
    for _ in 0..42 {
        game.tick();
    }
    let target = game.state.entities.get_mut(unit).unwrap();
    let hp = target.hp;
    target.apply_damage(target.damage_after_armor(20, 0.0), None);
    assert_eq!(target.hp, hp - 10);
    activate_item(&mut game, unit, AbilityKind::ArmorTreasure);
    game.tick();
    game.state
        .entities
        .get_mut(shop)
        .unwrap()
        .apply_damage(u32::MAX, None);
    for _ in 0..1800 {
        game.tick();
    }
    let target = game.state.entities.get_mut(unit).unwrap();
    assert_eq!(
        target
            .ability_uses_remaining
            .get(&AbilityKind::ArmorTreasure),
        Some(&1)
    );
    assert!(!target
        .ability_cooldowns
        .contains_key(&AbilityKind::ArmorTreasure));
    target.apply_damage(target.damage_after_armor(20, 0.0), None);
    assert_eq!(target.hp, hp - 20);
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert!(restored
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::ArmorTreasure));
}

#[test]
fn every_passive_item_applies_on_delivery_and_ignores_activation_forever() {
    for ability in [
        AbilityKind::Regen,
        AbilityKind::Haste,
        AbilityKind::DoubleDamage,
        AbilityKind::ArmorTreasure,
        AbilityKind::VitalityTreasure,
    ] {
        let (mut game, shop, unit) = item_fixture();
        buy(&mut game, shop, unit, ability);
        for _ in 0..42 {
            game.tick();
        }
        assert!(game
            .state
            .entities
            .get(unit)
            .unwrap()
            .shop_item_active(ability));
        let mut game = Game::restore_checkpoint_payload_text_for_test(
            &game.checkpoint_payload_text_for_test().unwrap(),
            game.state.map.clone(),
            game.state.map_metadata.clone(),
        )
        .unwrap();
        activate_item(&mut game, unit, ability);
        for _ in 0..1000 {
            game.tick();
        }
        let target = game.state.entities.get(unit).unwrap();
        assert!(target.shop_item_active(ability));
        assert_eq!(target.ability_uses_remaining.get(&ability), Some(&1));
        assert_eq!(
            target.ability_cooldowns.get(&ability).copied().unwrap_or(0),
            0
        );
    }
}

#[test]
fn duplicate_items_send_private_notices_without_spending() {
    for (first, second, _category) in [
        (
            AbilityKind::EscapeTreasure,
            AbilityKind::EscapeTreasure,
            "Movement",
        ),
        (
            AbilityKind::ArmorTreasure,
            AbilityKind::ArmorTreasure,
            "Defense",
        ),
        (AbilityKind::Haste, AbilityKind::Haste, "Attack"),
    ] {
        let (mut game, shop, unit) = item_fixture();
        buy(&mut game, shop, unit, first);
        game.tick();
        let oil = game.state.players[0].oil;
        // Both an in-flight delivery and an equipped item reserve the same item type.
        for ticks in [0, 32] {
            for _ in 0..ticks {
                game.tick();
            }
            buy(&mut game, shop, unit, second);
            let events = game.tick();
            let message = "Item already equipped or being delivered";
            assert!(events.iter().any(|(player, events)| *player == 1 && events.iter().any(
                |event| matches!(event, crate::protocol::Event::Notice { msg, .. } if msg == message))));
            assert!(events.iter().filter(|(player, _)| *player != 1).all(|(_, events)| events.iter().all(
                |event| !matches!(event, crate::protocol::Event::Notice { msg, .. } if msg == message))));
            assert_eq!(game.state.players[0].oil, oil);
        }
    }
}

#[test]
fn item_radiance_is_public_for_visible_carriers_without_private_inventory() {
    let mut game = empty_flat_game(&[
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "cultivators".into(),
            name: "Owner".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".into(),
            name: "Enemy".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 1,
            faction_id: "kriegsia".into(),
            name: "Ally".into(),
            color: "#fff".into(),
            is_ai: false,
        },
    ]);
    let carrier = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MagicAdept, 640.0, 640.0)
        .unwrap();
    let spotter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, 704.0, 640.0)
        .unwrap();
    game.state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, 600.0, 640.0)
        .unwrap();
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1, 2, 3], &game.state.entities, &game.state.map);
    game.state.entities.get_mut(carrier).unwrap().dao_master = true;
    for expected in 0..=5 {
        let unit = game.state.entities.get_mut(carrier).unwrap();
        match expected {
            1 => unit.escape_treasure = Some(999),
            2 => unit.receive_shop_item(AbilityKind::ArmorTreasure),
            3 => unit.receive_shop_item(AbilityKind::Haste),
            4 => unit.receive_shop_item(AbilityKind::VitalityTreasure),
            5 => unit.receive_shop_item(AbilityKind::DoubleDamage),
            _ => {}
        }
        for viewer in [1, 2, 3] {
            let snapshot = game.snapshot_for(viewer);
            let view = snapshot.entities.iter().find(|e| e.id == carrier).unwrap();
            assert_eq!(view.item_radiance_tier, Some(expected));
            if viewer != 1 {
                assert!(view.abilities.is_empty());
            }
            if viewer == 2 {
                assert!(!view.escape_treasure);
            }
        }
        for snapshot in [game.snapshot_for_spectator(&[2]), game.snapshot_full_for(2)] {
            let view = snapshot.entities.iter().find(|e| e.id == carrier).unwrap();
            assert_eq!(view.item_radiance_tier, Some(expected));
        }
    }
    // Regen contributes the sixth public tier without revealing its identity to visible enemies.
    game.state
        .entities
        .get_mut(carrier)
        .unwrap()
        .receive_shop_item(AbilityKind::Regen);
    for viewer in [1, 2, 3] {
        let snapshot = game.snapshot_for(viewer);
        let view = snapshot.entities.iter().find(|e| e.id == carrier).unwrap();
        assert_eq!(view.item_radiance_tier, Some(6));
        assert_eq!(
            view.abilities.iter().any(|entry| entry.ability == "regen"),
            viewer == 1,
            "only the owner sees Regen inventory on a visible carrier"
        );
    }
    game.state
        .entities
        .get_mut(carrier)
        .unwrap()
        .ability_uses_remaining
        .remove(&AbilityKind::Regen);
    // Permanent equipment keeps contributing without cooldowns; Escape consumption lowers the tier.
    let unit = game.state.entities.get_mut(carrier).unwrap();
    assert_eq!(unit.item_radiance_tier(), 5);
    unit.ability_uses_remaining
        .remove(&AbilityKind::VitalityTreasure);
    unit.ability_uses_remaining
        .remove(&AbilityKind::DoubleDamage);
    assert_eq!(unit.item_radiance_tier(), 3);
    assert_eq!(unit.ability_cooldowns.get(&AbilityKind::Haste), None);
    unit.escape_treasure = None;
    assert_eq!(unit.item_radiance_tier(), 2);
    // A complete snapshot must also clear radiance if equipment is removed by a scene restore.
    unit.ability_uses_remaining.remove(&AbilityKind::Haste);
    assert_eq!(unit.item_radiance_tier(), 1);
    unit.ability_uses_remaining
        .remove(&AbilityKind::ArmorTreasure);
    assert_eq!(unit.item_radiance_tier(), 0);
    let snapshot = game.snapshot_for(2);
    assert_eq!(
        snapshot
            .entities
            .iter()
            .find(|e| e.id == carrier)
            .unwrap()
            .item_radiance_tier,
        Some(0)
    );

    // Neither a hidden carrier nor its public tier may leak to enemy/selected-enemy views.
    let unit = game.state.entities.get_mut(carrier).unwrap();
    unit.receive_shop_item(AbilityKind::VitalityTreasure);
    unit.receive_shop_item(AbilityKind::DoubleDamage);
    unit.escape_treasure = Some(999);
    game.state
        .entities
        .get_mut(spotter)
        .unwrap()
        .set_position(2200.0, 2200.0);
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1, 2, 3], &game.state.entities, &game.state.map);
    for snapshot in [game.snapshot_for(2), game.snapshot_for_spectator(&[2])] {
        assert!(snapshot.entities.iter().all(|e| e.id != carrier));
    }
    assert_eq!(
        game.snapshot_full_for(2)
            .entities
            .iter()
            .find(|e| e.id == carrier)
            .unwrap()
            .item_radiance_tier,
        Some(3)
    );
}

#[test]
fn master_carries_all_six_current_items_without_category_limits() {
    let (mut game, shop, unit) = item_fixture();
    game.state.entities.get_mut(unit).unwrap().dao_master = true;
    let items = [
        AbilityKind::Regen,
        AbilityKind::Haste,
        AbilityKind::DoubleDamage,
        AbilityKind::ArmorTreasure,
        AbilityKind::VitalityTreasure,
    ];
    for ability in items {
        buy(&mut game, shop, unit, ability);
        game.tick();
    }
    buy(&mut game, shop, unit, AbilityKind::EscapeTreasure);
    for _ in 0..60 {
        game.tick();
    }
    let target = game.state.entities.get(unit).unwrap();
    for ability in items {
        assert!(target.shop_item_active(ability));
    }
    assert_eq!(target.escape_treasure, Some(shop));
    assert_eq!(
        target.item_radiance_tier(),
        6,
        "every current item contributes to the six-tier visual scale"
    );
}

#[test]
fn rank_slot_limits_reserve_deliveries_and_reject_regen_when_full() {
    for kind in [
        EntityKind::Warrior,
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ] {
        for master in [false, true] {
            let (mut game, shop, unit) = item_fixture_for_kind(kind);
            let carrier = game.state.entities.get_mut(unit).unwrap();
            carrier.dao_master = master;
            let capacity = if master {
                6
            } else if kind == EntityKind::Warrior {
                1
            } else {
                3
            };
            assert_eq!(carrier.carried_item_capacity(), Some(capacity));
            for ability in [
                AbilityKind::Haste,
                AbilityKind::DoubleDamage,
                AbilityKind::ArmorTreasure,
            ] {
                buy(&mut game, shop, unit, ability);
            }
            game.tick();
            assert_eq!(
                game.state.players[0].oil,
                if capacity == 1 { 900 } else { 650 }
            );
            let oil = game.state.players[0].oil;
            buy(&mut game, shop, unit, AbilityKind::VitalityTreasure);
            game.tick();
            assert_eq!(
                game.state.players[0].oil,
                oil - if master { 100 } else { 0 }
            );
            for _ in 0..60 {
                game.tick();
            }
            assert_eq!(
                game.state.entities.get(unit).unwrap().carried_item_count(),
                if master { 4 } else { capacity }
            );
            if !master {
                let oil = game.state.players[0].oil;
                buy(&mut game, shop, unit, AbilityKind::EscapeTreasure);
                game.tick();
                assert_eq!(game.state.players[0].oil, oil);
                assert!(!game.state.players[0]
                    .ability_cooldowns
                    .contains_key(&AbilityKind::EscapeTreasure));
                game.state
                    .entities
                    .get_mut(unit)
                    .unwrap()
                    .apply_damage(20, None);
                buy(&mut game, shop, unit, AbilityKind::Regen);
                for _ in 0..60 {
                    game.tick();
                }
                let carrier = game.state.entities.get(unit).unwrap();
                assert!(
                    !carrier.shop_item_active(AbilityKind::Regen),
                    "full slots reject Regen"
                );
                assert!(carrier.hp < carrier.max_hp);
                // Promotion opens the additional slots immediately.
                game.state.entities.get_mut(unit).unwrap().dao_master = true;
                buy(&mut game, shop, unit, AbilityKind::VitalityTreasure);
                for _ in 0..60 {
                    game.tick();
                }
                assert_eq!(
                    game.state.entities.get(unit).unwrap().carried_item_count(),
                    capacity + 1
                );
            }
        }
    }
}

#[test]
fn every_unit_targeted_good_has_unlimited_range_and_ten_hp_at_seven_point_five_tiles_per_second() {
    for ability in [
        AbilityKind::Regen,
        AbilityKind::EscapeTreasure,
        AbilityKind::Haste,
        AbilityKind::DoubleDamage,
        AbilityKind::ArmorTreasure,
        AbilityKind::VitalityTreasure,
    ] {
        let (mut game, shop, unit) = item_fixture();
        game.state
            .entities
            .get_mut(unit)
            .unwrap()
            .set_position(1600.0, 320.0);
        game.state
            .fog
            .recompute(&[1], &game.state.entities, &game.state.map);
        buy(&mut game, shop, unit, ability);
        game.tick();
        let projectile = game.state.ability_runtime.projectiles().next().unwrap();
        assert_eq!(projectile.expires_tick, u32::MAX);
        assert_eq!(projectile.x.to_world(), 328.0); // 8 px/tick = 7.5 tiles/s.
        let body = game
            .state
            .entities
            .get(projectile.source_object_id.unwrap())
            .unwrap();
        assert_eq!((body.hp, body.max_hp), (10, 10));
        assert_eq!(body.kind, EntityKind::ShopDelivery);
        assert!(body.is_targetable());
        assert!(!body.is_unit());
        assert_eq!(body.pos_x(), projectile.x.to_world());
        assert_eq!(body.sight_tiles(), 0);
    }
}

#[test]
fn destroyed_delivery_keeps_payment_releases_slot_and_never_applies_after_checkpoint() {
    let (mut game, shop, unit) = item_fixture();
    buy(&mut game, shop, unit, AbilityKind::VitalityTreasure);
    game.tick();
    let body_id = game
        .state
        .ability_runtime
        .projectiles()
        .next()
        .unwrap()
        .source_object_id
        .unwrap();
    game.state
        .entities
        .get_mut(body_id)
        .unwrap()
        .apply_damage(9, None);
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert_eq!(game.state.entities.get(body_id).unwrap().hp, 1);
    game.state
        .entities
        .get_mut(body_id)
        .unwrap()
        .apply_damage(1, None);
    game.tick();
    assert!(game.state.entities.get(body_id).is_none());
    assert_eq!(game.state.ability_runtime.projectiles().count(), 0);
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
    assert_eq!(game.state.players[0].oil, 900);
    for _ in 0..50 {
        game.tick();
    }
    assert!(!game
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::VitalityTreasure));
    buy(&mut game, shop, unit, AbilityKind::VitalityTreasure);
    for _ in 0..40 {
        game.tick();
    }
    assert_eq!(game.state.players[0].oil, 800);
    assert!(game
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::VitalityTreasure));
    assert!(!game
        .state
        .entities
        .iter()
        .any(|e| e.kind == EntityKind::ShopDelivery));
}

#[test]
fn enemy_rifle_fire_intercepts_flying_goods_without_refund() {
    let (mut game, shop, unit) = item_fixture();
    let enemy = PlayerState {
        id: 2,
        team_id: 2,
        faction_id: "kriegsia".into(),
        ..game.state.players[0].clone()
    };
    game.state.players.push(enemy);
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .set_position(1600.0, 320.0);
    let rifle = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, 800.0, 400.0)
        .unwrap();
    game.enqueue(
        2,
        Command::HoldPosition {
            units: vec![rifle],
            queued: false,
        },
    );
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    buy(&mut game, shop, unit, AbilityKind::VitalityTreasure);
    for _ in 0..170 {
        game.tick();
    }
    assert_eq!(game.state.players[0].oil, 900);
    assert_eq!(game.state.ability_runtime.projectiles().count(), 0);
    assert!(!game
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::VitalityTreasure));
}

#[test]
fn regen_reserves_slot_rejects_duplicates_projects_privately_and_survives_checkpoint() {
    let (mut game, shop, unit) = item_fixture();
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .apply_damage(40, None);
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    assert_eq!(game.state.players[0].oil, 950);
    for _ in 0..40 {
        game.tick();
    }
    assert_eq!(
        game.state.entities.get(unit).unwrap().carried_item_count(),
        1
    );
    let own = game.snapshot_for(1);
    assert!(own
        .entities
        .iter()
        .find(|e| e.id == unit)
        .unwrap()
        .abilities
        .iter()
        .any(|entry| entry.ability == "regen" && entry.remaining_uses == Some(1)));
    buy(&mut game, shop, unit, AbilityKind::Regen);
    game.tick();
    assert_eq!(game.state.players[0].oil, 950);
    game.state.entities.remove(shop);
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    let hp = game.state.entities.get(unit).unwrap().hp;
    for _ in 0..config::TICK_HZ {
        game.tick();
        restored.tick();
        assert_eq!(
            game.replay_end_state_hash().unwrap(),
            restored.replay_end_state_hash().unwrap()
        );
    }
    assert_eq!(game.state.entities.get(unit).unwrap().hp, hp + 3);
}
