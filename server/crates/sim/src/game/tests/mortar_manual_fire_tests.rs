use super::fixtures::*;
use super::*;
use crate::game::entity::MovePhase;
use crate::game::mortar_scatter::scattered_mortar_impact;
use crate::game::teams::TeamRelations;

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
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(14, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_facing(0.0);
        mortar_entity.set_weapon_facing(0.0);
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
        mortar_entity.set_emplacement_facing(Some(0.0));
    }
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    (game, mortar, target_pos)
}

fn mortar_launch_count(events: &[(u32, Vec<Event>)], player_id: u32, mortar: u32) -> usize {
    events
        .iter()
        .find(|(id, _)| *id == player_id)
        .map(|(_, player_events)| {
            player_events
                .iter()
                .filter(
                    |event| matches!(event, Event::MortarLaunch { from, .. } if *from == mortar),
                )
                .count()
        })
        .unwrap_or(0)
}

fn mortar_launch_targets(
    events: &[(u32, Vec<Event>)],
    player_id: u32,
    mortar: u32,
) -> Vec<(f32, f32)> {
    events
        .iter()
        .find(|(id, _)| *id == player_id)
        .map(|(_, player_events)| {
            player_events
                .iter()
                .filter_map(|event| match event {
                    Event::MortarLaunch {
                        from, to_x, to_y, ..
                    } if *from == mortar => Some((*to_x, *to_y)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
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

fn enqueue_queued_manual_mortar_fire(game: &mut Game, mortar: u32, target_pos: (f32, f32)) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: true,
        },
    );
}

fn expected_mortar_impact(
    game: &Game,
    mortar: u32,
    target_pos: (f32, f32),
    tick: u32,
) -> (f32, f32) {
    let owner = game
        .state
        .entities
        .get(mortar)
        .expect("mortar should exist")
        .owner;
    let teams =
        TeamRelations::from_player_teams(game.state.players.iter().map(|p| (p.id, p.team_id)));
    scattered_mortar_impact(
        &game.state.fog,
        &teams,
        owner,
        mortar,
        target_pos.0,
        target_pos.1,
        tick,
    )
}

fn points_nearly_equal(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() <= 0.001 && (a.1 - b.1).abs() <= 0.001
}

#[test]
fn manual_mortar_fire_with_autocast_enabled_only_launches_once() {
    let (mut game, mortar, target_pos) = manual_fire_fixture();
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_autocast_enabled(ability::AbilityKind::MortarFire, true);
    }
    game.state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    game.state.players[0]
        .upgrades
        .insert(upgrade::UpgradeKind::MortarAutocast);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    enqueue_manual_mortar_fire(&mut game, mortar, target_pos);
    let events = game.tick();

    assert_eq!(
        mortar_launch_count(&events, 1, mortar),
        1,
        "manual mortar fire should consume the weapon cycle so same-tick autocast cannot double launch"
    );
    assert!(
        game.state
            .entities
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
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
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
            game.state
                .entities
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
fn manual_mortar_fire_turns_while_waiting_for_weapon_cooldown() {
    let (mut game, mortar, _) = manual_fire_fixture();
    let target_pos = game.state.map.tile_center(8, 14);
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_attack_cd(4);
        mortar_entity.set_facing(0.0);
        mortar_entity.set_weapon_facing(0.0);
    }

    enqueue_manual_mortar_fire(&mut game, mortar, target_pos);
    let events = game.tick();

    assert_eq!(
        mortar_launch_count(&events, 1, mortar),
        0,
        "manual mortar fire should wait while the weapon cycle is still cooling down"
    );
    let facing_after_first_wait = game
        .state
        .entities
        .get(mortar)
        .expect("mortar should exist")
        .weapon_facing()
        .expect("mortar should have weapon facing");
    assert!(
        facing_after_first_wait > 0.0,
        "manual mortar fire should rotate toward the queued target while waiting to reload"
    );

    let mut launched_after_reload = false;
    for _ in 0..8 {
        let events = game.tick();
        launched_after_reload |= mortar_launch_count(&events, 1, mortar) == 1;
        if launched_after_reload {
            break;
        }
    }

    assert!(
        launched_after_reload,
        "manual mortar fire should launch once the mortar has both reloaded and faced the target"
    );
}

#[test]
fn deployed_manual_mortar_fire_inside_minimum_range_tears_down_and_repositions() {
    let (mut game, mortar, _) = manual_fire_fixture();
    let target_pos = game.state.map.tile_center(8, 8);

    enqueue_manual_mortar_fire(&mut game, mortar, target_pos);
    let events = game.tick();

    assert_eq!(mortar_launch_count(&events, 1, mortar), 0);
    let mortar_entity = game
        .state
        .entities
        .get(mortar)
        .expect("mortar should exist");
    assert!(matches!(mortar_entity.order(), Order::Ability(_)));
    assert!(matches!(
        mortar_entity.weapon_setup(),
        WeaponSetup::TearingDown { .. }
    ));

    let mut launched = false;
    for _ in 0..240 {
        let events = game.tick();
        if mortar_launch_count(&events, 1, mortar) == 1 {
            launched = true;
            break;
        }
    }

    assert!(
        launched,
        "manual fire should launch after the deployed mortar tears down and exits its dead zone"
    );
}

#[test]
fn queued_mortar_setup_promotes_instead_of_being_discarded() {
    let (mut game, mortar, _) = manual_fire_fixture();
    let target_pos = game.state.map.tile_center(8, 14);

    game.enqueue(
        1,
        Command::SetupAntiTankGuns {
            units: vec![mortar],
            x: target_pos.0,
            y: target_pos.1,
            queued: true,
        },
    );
    game.tick();

    let mortar_entity = game
        .state
        .entities
        .get(mortar)
        .expect("mortar should exist");
    assert!(mortar_entity.queued_orders().is_empty());
    assert!(matches!(
        mortar_entity.weapon_setup(),
        WeaponSetup::TearingDownToRedeploy { .. }
    ));
    assert!(
        (mortar_entity.pending_redeploy_facing().unwrap_or_default() - std::f32::consts::FRAC_PI_2)
            .abs()
            < 0.001,
        "queued mortar setup should promote toward the submitted point"
    );
}

#[test]
fn queued_manual_mortar_fire_promotes_to_wait_for_weapon_cooldown() {
    let (mut game, mortar, target_pos) = manual_fire_fixture();
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
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
    let mortar_entity = game
        .state
        .entities
        .get(mortar)
        .expect("mortar should exist");
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

#[test]
fn queued_manual_mortar_fire_commands_fire_finite_shots_across_reload_cycles() {
    let (mut game, mortar, target_pos) = manual_fire_fixture();
    let enemy_pos = game.state.map.tile_center(8, 14);
    if let Some(mortar_entity) = game.state.entities.get_mut(mortar) {
        mortar_entity.set_autocast_enabled(ability::AbilityKind::MortarFire, true);
    }
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    game.state
        .entities
        .get_mut(enemy)
        .expect("enemy should exist")
        .hold_position();
    game.state.players[0]
        .upgrades
        .insert(upgrade::UpgradeKind::MortarAutocast);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    for _ in 0..3 {
        enqueue_queued_manual_mortar_fire(&mut game, mortar, target_pos);
    }

    let mut launched_targets = Vec::new();
    let mut expected_targets = Vec::new();
    for _ in 0..240 {
        let tick = game.tick_count().saturating_add(1);
        let expected = expected_mortar_impact(&game, mortar, target_pos, tick);
        let events = game.tick();
        let launches = mortar_launch_targets(&events, 1, mortar);
        expected_targets.extend(std::iter::repeat_n(expected, launches.len()));
        launched_targets.extend(launches);
        if launched_targets.len() == 3 {
            break;
        }
    }

    assert_eq!(
        launched_targets.len(),
        3,
        "queued manual mortar fire should produce three finite manual shots"
    );
    assert!(
        launched_targets
            .iter()
            .copied()
            .zip(expected_targets)
            .all(|(actual, expected)| points_nearly_equal(actual, expected)),
        "queued manual mortar fire should produce finite manual shots before autocast can take the weapon cycle"
    );
}
