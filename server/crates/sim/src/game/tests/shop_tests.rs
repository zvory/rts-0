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
fn shop_costs_both_resources_and_finishes_after_1200_work_ticks() {
    let (mut game, worker) = setup("cultivators", 200, 200);
    let site = start(&mut game, worker);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
    let building = game.state.entities.get(site).unwrap();
    assert_eq!(building.max_hp, 200);
    assert_eq!(building.construction.as_ref().unwrap().total, 1200);
    let progress = building.construction.as_ref().unwrap().progress;
    for _ in progress..1199 {
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
        ("cultivators", 199, 200),
        ("cultivators", 200, 199),
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
    let (mut game, worker) = setup("cultivators", 200, 200);
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
        (200, 200)
    );
}

fn item_fixture() -> (Game, u32, u32) {
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
        .spawn_unit(1, EntityKind::Warrior, 640.0, 320.0)
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
fn healing_potion_costs_50_and_arrives_after_ten_tiles_of_flight() {
    let (mut game, shop, unit) = item_fixture();
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .apply_damage(80, None);
    let hp = game.state.entities.get(unit).unwrap().hp;
    buy(&mut game, shop, unit, AbilityKind::HealingPotion);
    game.tick();
    assert_eq!(game.state.players[0].oil, 950);
    let object = game.state.ability_runtime.world_objects().next().unwrap();
    assert!((object.x.to_world() - (320.0 + 320.0 / 30.0)).abs() < 0.01);
    for _ in 1..29 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(unit).unwrap().hp, hp);
    game.tick();
    let unit = game.state.entities.get(unit).unwrap();
    assert_eq!(unit.hp, unit.max_hp);
    assert_eq!(game.state.ability_runtime.world_objects().count(), 0);
}

#[test]
fn shop_items_reject_buildings_out_of_range_stale_and_enemy_targets_without_spending() {
    let (mut game, shop, unit) = item_fixture();
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 640.0, 352.0)
        .unwrap();
    for target in [shop, enemy, u32::MAX] {
        buy(&mut game, shop, target, AbilityKind::HealingPotion);
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
    for _ in 1..31 {
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
    buy(&mut game, shop, unit, AbilityKind::HealingPotion);
    game.tick();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    for _ in 0..40 {
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
    let mut ally_player = game.state.players[0].clone();
    ally_player.id = 2;
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
    for _ in 0..40 {
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
    buy(&mut game, shop, unit, AbilityKind::HealingPotion);
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
    buy(&mut game, shop, unit, AbilityKind::HealingPotion);
    game.tick();
    assert_eq!(game.state.players[0].oil, 850);
    game.state.players[0].set_resources(0, 49);
    buy(&mut game, shop, unit, AbilityKind::HealingPotion);
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
        buy(&mut game, source, unit, AbilityKind::HealingPotion);
        game.tick();
    }
    assert_eq!(game.state.players[0].oil, 1000);
}
