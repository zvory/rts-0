use super::fixtures::*;
use super::*;
use crate::game::entity::MovePhase;

fn manual_fire_fixture() -> (Game, u32, (f32, f32)) {
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
    let mortar_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(12, 8);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.entities.get_mut(mortar) {
        mortar_entity.set_facing(0.0);
        mortar_entity.set_weapon_facing(0.0);
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    (game, mortar, target_pos)
}

fn mortar_launch_count(events: &[(u32, Vec<Event>)], player_id: u32, mortar: u32) -> usize {
    events
        .iter()
        .find(|(id, _)| *id == player_id)
        .map(|(_, player_events)| {
            player_events
                .iter()
                .filter(|event| matches!(event, Event::MortarLaunch { from, .. } if *from == mortar))
                .count()
        })
        .unwrap_or(0)
}

fn enqueue_manual_mortar_fire(game: &mut Game, mortar: u32, target_pos: (f32, f32)) {
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
}

#[test]
fn manual_mortar_fire_with_autocast_enabled_only_launches_once() {
    let (mut game, mortar, target_pos) = manual_fire_fixture();
    if let Some(mortar_entity) = game.entities.get_mut(mortar) {
        mortar_entity.set_autocast_enabled(ability::AbilityKind::MortarFire, true);
    }
    game.entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    game.players[0]
        .upgrades
        .insert(upgrade::UpgradeKind::MortarAutocast);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    enqueue_manual_mortar_fire(&mut game, mortar, target_pos);
    let events = game.tick();

    assert_eq!(
        mortar_launch_count(&events, 1, mortar),
        1,
        "manual mortar fire should consume the weapon cycle so same-tick autocast cannot double launch"
    );
    assert!(
        game.entities
            .get(mortar)
            .expect("mortar should exist")
            .attack_cd()
            > 0,
        "manual mortar fire should start the shared mortar weapon cooldown"
    );
}

#[test]
fn manual_mortar_fire_waits_for_weapon_cooldown() {
    let (mut game, mortar, target_pos) = manual_fire_fixture();
    if let Some(mortar_entity) = game.entities.get_mut(mortar) {
        mortar_entity.set_attack_cd(2);
    }

    enqueue_manual_mortar_fire(&mut game, mortar, target_pos);
    let events = game.tick();

    assert_eq!(
        mortar_launch_count(&events, 1, mortar),
        0,
        "manual mortar fire should not launch while the weapon cycle is still cooling down"
    );
    assert!(
        matches!(
            game.entities
                .get(mortar)
                .expect("mortar should exist")
                .order(),
            Order::Ability(_)
        ),
        "manual mortar fire order should be retained while waiting for weapon cooldown"
    );

    let mut launched_after_reload = false;
    for _ in 0..4 {
        let events = game.tick();
        launched_after_reload |= mortar_launch_count(&events, 1, mortar) == 1;
        if launched_after_reload {
            break;
        }
    }

    assert!(
        launched_after_reload,
        "manual mortar fire should launch once the shared weapon cooldown is ready"
    );
}

#[test]
fn queued_manual_mortar_fire_promotes_to_wait_for_weapon_cooldown() {
    let (mut game, mortar, target_pos) = manual_fire_fixture();
    if let Some(mortar_entity) = game.entities.get_mut(mortar) {
        mortar_entity.set_attack_cd(2);
        mortar_entity.set_order(Order::move_to(mortar_entity.pos_x, mortar_entity.pos_y));
        mortar_entity.mark_move_phase(MovePhase::Arrived);
        mortar_entity.append_queued_order(OrderIntent::ability(
            ability::AbilityKind::MortarFire,
            target_pos.0,
            target_pos.1,
        ));
    }

    let events = game.tick();

    assert_eq!(
        mortar_launch_count(&events, 1, mortar),
        0,
        "queued manual mortar fire should not launch while the weapon cycle is still cooling down"
    );
    let mortar_entity = game.entities.get(mortar).expect("mortar should exist");
    assert!(
        matches!(mortar_entity.order(), Order::Ability(_)),
        "queued manual mortar fire should become the active waiting order instead of being skipped"
    );
    assert!(
        mortar_entity.queued_orders().is_empty(),
        "promoting the queued manual shot should consume the queued intent"
    );

    let mut launched_after_reload = false;
    for _ in 0..4 {
        let events = game.tick();
        launched_after_reload |= mortar_launch_count(&events, 1, mortar) == 1;
        if launched_after_reload {
            break;
        }
    }

    assert!(
        launched_after_reload,
        "queued manual mortar fire should launch once the shared weapon cooldown is ready"
    );
}
