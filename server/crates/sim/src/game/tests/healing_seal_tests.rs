use super::fixtures::*;
use super::*;
use crate::game::ability::AbilityKind;
use crate::game::services::shop_items::heal_seals;
use crate::game::teams::TeamRelations;

fn fixture() -> (Game, u32, u32) {
    let mut game = empty_flat_game(&[PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "cultivators".into(),
        name: "Seal".into(),
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
        .spawn_unit(1, EntityKind::Warrior, 656.0, 336.0)
        .unwrap();
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    (game, shop, unit)
}

fn purchase(game: &mut Game, shop: u32, x: f32, y: f32) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: AbilityKind::HealingSeal,
            units: vec![shop],
            x: Some(x),
            y: Some(y),
            target_id: None,
            queued: false,
        },
    );
    game.tick();
}

#[test]
fn authored_healing_seal_capacity_and_checkpoint() {
    let (mut game, shop, unit) = fixture();
    purchase(&mut game, shop, 656.0, 336.0);
    assert_eq!(
        game.state.players[0].oil, 1000,
        "Healing Seal is no longer sold"
    );
    assert!(!game
        .state
        .entities
        .iter()
        .any(|e| e.kind == EntityKind::HealingSeal));
    game.state
        .entities
        .spawn_building(1, EntityKind::HealingSeal, 656.0, 336.0, true)
        .unwrap();
    let seal = game
        .state
        .entities
        .iter()
        .find(|e| e.kind == EntityKind::HealingSeal)
        .unwrap();
    let id = seal.id;
    assert_eq!((seal.hp, seal.max_hp), (400, 400));
    assert!(!seal.is_targetable());
    assert_eq!(
        rts_rules::static_blocker_class(seal.kind),
        rts_rules::StaticBlockerClass::None
    );
    assert!(!rts_rules::blocks_line_of_sight(seal.kind));
    assert!(!game
        .state
        .entities
        .get_mut(id)
        .unwrap()
        .apply_damage(u32::MAX, None));
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .apply_damage(30, None);
    for _ in 0..30 {
        game.tick();
    }
    assert_eq!(game.state.entities.get(id).unwrap().hp, 397);
    assert!(!game.state.entities.get_mut(id).unwrap().restore_hp(400));
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    for _ in 0..30 {
        game.tick();
        restored.tick();
    }
    assert_eq!(game.state.entities.get(id).unwrap().hp, 394);
    assert_eq!(restored.state.entities.get(id).unwrap().hp, 394);
    assert_eq!(
        restored.state.entities.get(unit).unwrap().hp,
        game.state.entities.get(unit).unwrap().hp
    );
    game.state.entities.remove(shop);
    assert!(
        !game.alive_players().contains(&1),
        "seals cannot prevent defeat"
    );
    for _ in 0..10 {
        game.tick();
    }
    assert!(
        game.state.entities.get(id).unwrap().hp < 394,
        "seal survives its supplying shop"
    );
}

#[test]
fn healing_seals_do_not_block_building_sites_for_any_owner() {
    use crate::game::services::standability::{
        building_site_status_for_build_intent, BuildSiteStatus,
    };

    let (mut game, _, unit) = fixture();
    game.state.entities.remove(unit);
    for owner in [1, 2] {
        game.state
            .entities
            .spawn_building(owner, EntityKind::HealingSeal, 656.0, 336.0, true)
            .unwrap();
    }
    assert_eq!(
        building_site_status_for_build_intent(
            &game.state.map,
            &game.state.entities,
            EntityKind::Portal,
            19,
            9,
            0,
        ),
        BuildSiteStatus::Clear,
        "neither allied nor enemy ground magic reserves a construction footprint"
    );
    game.state
        .entities
        .spawn_building(2, EntityKind::Portal, 656.0, 336.0, true)
        .unwrap();
    assert_eq!(
        building_site_status_for_build_intent(
            &game.state.map,
            &game.state.entities,
            EntityKind::Portal,
            19,
            9,
            0,
        ),
        BuildSiteStatus::BlockedByBuilding,
        "ordinary structures still block construction"
    );
}

#[test]
fn healing_seal_overlaps_survive_normal_ticks() {
    let (mut game, _, unit) = fixture();
    for _ in 0..2 {
        game.state
            .entities
            .spawn_building(1, EntityKind::HealingSeal, 656.0, 336.0, true)
            .unwrap();
    }
    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::HealingSeal)
            .count(),
        2,
    );
    game.state.entities.remove(unit);
    game.state
        .entities
        .spawn_building(1, EntityKind::Portal, 656.0, 336.0, true)
        .unwrap();
    game.tick();
    game.assert_invariants();
}

#[test]
fn healing_seal_purchase_rejects_resource_nodes() {
    for kind in [EntityKind::Steel, EntityKind::Oil] {
        let (mut game, shop, _) = fixture();
        game.state.entities.spawn_node(kind, 656.0, 336.0).unwrap();
        purchase(&mut game, shop, 656.0, 336.0);
        assert_eq!(game.state.players[0].oil, 1000);
        assert!(!game
            .state
            .entities
            .iter()
            .any(|e| e.kind == EntityKind::HealingSeal));
    }
}

#[test]
fn healing_seal_invalid_purchases_spend_nothing() {
    for (x, y) in [
        (320.0, 320.0),
        (0.0, 0.0),
        (-1.0, 336.0),
        (f32::NAN, 336.0),
        (f32::INFINITY, 336.0),
        (2000.0, 2000.0),
    ] {
        let (mut game, shop, _) = fixture();
        purchase(&mut game, shop, x, y);
        assert_eq!(game.state.players[0].oil, 1000, "invalid ({x},{y})");
        assert!(!game
            .state
            .entities
            .iter()
            .any(|e| e.kind == EntityKind::HealingSeal));
    }
    let (mut game, shop, _) = fixture();
    game.state.players[0].set_resources(0, 149);
    purchase(&mut game, shop, 656.0, 336.0);
    assert_eq!(game.state.players[0].oil, 149);
    assert!(!game
        .state
        .entities
        .iter()
        .any(|e| e.kind == EntityKind::HealingSeal));
    game.state.players[0].set_resources(0, 1000);
    game.state.players[0].faction_id = "kriegsia".into();
    purchase(&mut game, shop, 656.0, 336.0);
    assert_eq!(game.state.players[0].oil, 1000);
}

#[test]
fn healing_seal_pulses_filter_targets_and_spend_only_actual_hp() {
    let mut entities = EntityStore::new();
    let seal = entities
        .spawn_building(1, EntityKind::HealingSeal, 336.0, 336.0, true)
        .unwrap();
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 1)]);
    let mut ids = Vec::new();
    for (owner, kind, x) in [
        (1, EntityKind::Warrior, 336.0),
        (3, EntityKind::Warrior, 384.0), // inclusive boundary
        (1, EntityKind::Warrior, 385.0),
        (2, EntityKind::Warrior, 336.0),
        (1, EntityKind::ScoutPlane, 336.0),
    ] {
        let id = entities.spawn_unit(owner, kind, x, 336.0).unwrap();
        entities.get_mut(id).unwrap().apply_damage(10, None);
        ids.push((id, entities.get(id).unwrap().hp));
    }
    let building = entities
        .spawn_building(1, EntityKind::Shop, 336.0, 336.0, true)
        .unwrap();
    entities.get_mut(building).unwrap().apply_damage(10, None);
    let full = entities
        .spawn_unit(1, EntityKind::Warrior, 336.0, 336.0)
        .unwrap();
    let dead = entities
        .spawn_unit(1, EntityKind::Warrior, 336.0, 336.0)
        .unwrap();
    entities.get_mut(dead).unwrap().apply_damage(u32::MAX, None);
    heal_seals(&mut entities, &teams, 9);
    assert_eq!(entities.get(seal).unwrap().hp, 400);
    for tick in [10, 20, 30] {
        heal_seals(&mut entities, &teams, tick);
    }
    for (index, (id, hp)) in ids.iter().enumerate() {
        assert_eq!(
            entities.get(*id).unwrap().hp,
            *hp + if index < 2 { 3 } else { 0 }
        );
    }
    assert_eq!(entities.get(seal).unwrap().hp, 394);
    assert_eq!(entities.get(building).unwrap().hp, 190);
    assert_eq!(entities.get(dead).unwrap().hp, 0);
    assert_eq!(
        entities.get(full).unwrap().hp,
        entities.get(full).unwrap().max_hp
    );
    // Only one point remains: stable id order assigns it once and removes the exhausted field.
    entities.get_mut(seal).unwrap().spend_seal_healing(393);
    heal_seals(&mut entities, &teams, 40);
    assert!(entities.get(seal).is_none());
    assert_eq!(entities.get(ids[0].0).unwrap().hp, ids[0].1 + 4);
    assert_eq!(entities.get(ids[1].0).unwrap().hp, ids[1].1 + 3);
}

#[test]
fn healing_seal_full_budget_and_overlapping_fields() {
    let mut entities = EntityStore::new();
    let teams = TeamRelations::from_player_teams([(1, 1)]);
    let seal = entities
        .spawn_building(1, EntityKind::HealingSeal, 336.0, 336.0, true)
        .unwrap();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 336.0, 336.0)
        .unwrap();
    // Repeated injuries model a wounded army rotating through, without overheal wasting capacity.
    let mut total = 0;
    for tick in 1..=400 {
        entities.get_mut(tank).unwrap().apply_damage(1, None);
        let hp = entities.get(tank).unwrap().hp;
        heal_seals(&mut entities, &teams, tick * 10);
        total += entities.get(tank).unwrap().hp - hp;
    }
    assert_eq!(total, 400);
    assert!(entities.get(seal).is_none());
    let first = entities
        .spawn_building(1, EntityKind::HealingSeal, 336.0, 336.0, true)
        .unwrap();
    let second = entities
        .spawn_building(1, EntityKind::HealingSeal, 336.0, 336.0, true)
        .unwrap();
    entities.get_mut(tank).unwrap().apply_damage(2, None);
    heal_seals(&mut entities, &teams, 4010);
    assert_eq!(entities.get(first).unwrap().hp, 399);
    assert_eq!(entities.get(second).unwrap().hp, 399);
}

#[test]
fn healing_seal_fog_drops_hidden_fields_without_building_memory_or_vision() {
    let players: Vec<_> = [1, 2]
        .into_iter()
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "cultivators".into(),
            name: format!("P{id}"),
            color: "#fff".into(),
            is_ai: false,
        })
        .collect();
    let mut game = empty_flat_game(&players);
    let seal = game
        .state
        .entities
        .spawn_building(1, EntityKind::HealingSeal, 656.0, 336.0, true)
        .unwrap();
    let scout = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 720.0, 336.0)
        .unwrap();
    game.tick();
    game.tick();
    assert!(game.snapshot_for(2).entities.iter().any(|e| e.id == seal));
    assert!(
        !game.state.fog.is_visible_world(1, 656.0, 336.0),
        "seal grants no sight"
    );
    game.state
        .entities
        .get_mut(scout)
        .unwrap()
        .set_position(2000.0, 2000.0);
    game.tick();
    game.tick();
    assert!(!game.snapshot_for(2).entities.iter().any(|e| e.id == seal));
    assert!(!game
        .state
        .building_memory
        .entries_for_player_for_test(2)
        .any(|e| e.id == seal));
    assert!(
        game.snapshot_for(1).entities.iter().any(|e| e.id == seal),
        "owner can still inspect its field"
    );
}
