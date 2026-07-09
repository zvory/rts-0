use super::fixtures::empty_flat_game;
use super::panzerfaust_tests::{
    make_invulnerable, panzerfaust_damage_to, panzerfaust_fixture, panzerfaust_players,
    player_events, refresh_world, spawn_building_on_tile, spawn_unit_on_tile,
};
use super::*;

#[test]
fn attack_move_acquires_tanks_but_plain_move_does_not_auto_fire() {
    let (mut moving_game, moving_panzerfaust, moving_tank) = panzerfaust_fixture();
    let moving_tank_hp = moving_game
        .state
        .entities
        .get(moving_tank)
        .expect("tank exists")
        .hp;
    let move_goal = moving_game.state.map.tile_center(20, 8);
    moving_game.enqueue(
        1,
        Command::Move {
            units: vec![moving_panzerfaust],
            x: move_goal.0,
            y: move_goal.1,
            queued: false,
        },
    );
    let mut move_launched = false;
    for _ in 0..50 {
        let events = moving_game.tick();
        move_launched |= player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustLaunch { .. }));
    }
    assert_eq!(
        moving_game
            .state
            .entities
            .get(moving_tank)
            .expect("tank exists")
            .hp,
        moving_tank_hp
    );
    assert!(!move_launched);

    let (mut attack_move_game, attack_move_panzerfaust, attack_move_tank) = panzerfaust_fixture();
    let attack_move_tank_hp = attack_move_game
        .state
        .entities
        .get(attack_move_tank)
        .expect("tank exists")
        .hp;
    let attack_move_goal = attack_move_game.state.map.tile_center(20, 8);
    attack_move_game.enqueue(
        1,
        Command::AttackMove {
            units: vec![attack_move_panzerfaust],
            x: attack_move_goal.0,
            y: attack_move_goal.1,
            queued: false,
        },
    );
    let mut attack_move_impact_hp = None;
    for _ in 0..70 {
        let events = attack_move_game.tick();
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && attack_move_impact_hp.is_none() {
            attack_move_impact_hp = attack_move_game
                .state
                .entities
                .get(attack_move_tank)
                .map(|tank| tank.hp);
        }
    }
    assert_eq!(
        attack_move_impact_hp,
        Some(attack_move_tank_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Tank)))
    );
}

#[test]
fn attack_move_acquires_scout_cars() {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust = spawn_unit_on_tile(&mut game, 1, EntityKind::Panzerfaust, 8, 8);
    let scout = spawn_unit_on_tile(&mut game, 2, EntityKind::ScoutCar, 11, 8);
    make_invulnerable(&mut game, panzerfaust);
    refresh_world(&mut game);

    let attack_move_goal = game.state.map.tile_center(20, 8);
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![panzerfaust],
            x: attack_move_goal.0,
            y: attack_move_goal.1,
            queued: false,
        },
    );

    let mut owner_saw_launch = false;
    let mut owner_saw_scout_death = false;
    for _ in 0..70 {
        let events = game.tick();
        owner_saw_launch |= player_events(&events, 1).iter().any(
            |event| matches!(event, Event::PanzerfaustLaunch { from, .. } if *from == panzerfaust),
        );
        owner_saw_scout_death |= player_events(&events, 1).iter().any(|event| {
            matches!(event, Event::Death { id, kind, .. }
                if *id == scout && kind == crate::protocol::kinds::SCOUT_CAR)
        });
    }

    assert!(owner_saw_launch);
    assert!(owner_saw_scout_death);
    assert!(
        game.state.entities.get(scout).is_none(),
        "attack-move Panzerfaust should auto-acquire Scout Cars"
    );
}

#[test]
fn attack_move_can_fallback_to_buildings_when_no_tank_is_visible() {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust = spawn_unit_on_tile(&mut game, 1, EntityKind::Panzerfaust, 8, 8);
    let depot = spawn_building_on_tile(&mut game, 2, EntityKind::Depot, 10, 7);
    let depot_hp = game.state.entities.get(depot).expect("depot exists").hp;
    make_invulnerable(&mut game, panzerfaust);
    refresh_world(&mut game);

    let attack_move_goal = game.state.map.tile_center(20, 8);
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![panzerfaust],
            x: attack_move_goal.0,
            y: attack_move_goal.1,
            queued: false,
        },
    );

    let mut depot_hp_on_impact = None;
    for _ in 0..90 {
        let events = game.tick();
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && depot_hp_on_impact.is_none() {
            depot_hp_on_impact = game.state.entities.get(depot).map(|depot| depot.hp);
        }
    }

    assert_eq!(
        depot_hp_on_impact,
        Some(depot_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Depot)))
    );
}

#[test]
fn attack_move_prefers_visible_tank_over_building_fallback() {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust = spawn_unit_on_tile(&mut game, 1, EntityKind::Panzerfaust, 8, 8);
    let depot = spawn_building_on_tile(&mut game, 2, EntityKind::Depot, 10, 9);
    let tank = spawn_unit_on_tile(&mut game, 2, EntityKind::Tank, 11, 7);
    let depot_hp = game.state.entities.get(depot).expect("depot exists").hp;
    let tank_hp = game.state.entities.get(tank).expect("tank exists").hp;
    make_invulnerable(&mut game, panzerfaust);
    refresh_world(&mut game);

    let attack_move_goal = game.state.map.tile_center(20, 8);
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![panzerfaust],
            x: attack_move_goal.0,
            y: attack_move_goal.1,
            queued: false,
        },
    );

    let mut tank_hp_on_impact = None;
    for _ in 0..90 {
        let events = game.tick();
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && tank_hp_on_impact.is_none() {
            tank_hp_on_impact = game.state.entities.get(tank).map(|tank| tank.hp);
        }
    }

    assert_eq!(
        tank_hp_on_impact,
        Some(tank_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Tank)))
    );
    assert_eq!(
        game.state.entities.get(depot).expect("depot exists").hp,
        depot_hp,
        "Panzerfaust should not shoot a building while a Tank is visible"
    );
}
