use super::fixtures::*;
use super::*;

fn panzerfaust_players() -> [PlayerInit; 3] {
    [
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
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#888".into(),
            is_ai: false,
        },
    ]
}

fn refresh_world(game: &mut Game) {
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
}

fn panzerfaust_fixture_at_tank_tile(tank_tile_x: u32) -> (Game, u32, u32) {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust_pos = game.map.tile_center(8, 8);
    let tank_pos = game.map.tile_center(tank_tile_x, 8);
    let panzerfaust = game
        .entities
        .spawn_unit(
            1,
            EntityKind::Panzerfaust,
            panzerfaust_pos.0,
            panzerfaust_pos.1,
        )
        .expect("panzerfaust should spawn");
    let tank = game
        .entities
        .spawn_unit(2, EntityKind::Tank, tank_pos.0, tank_pos.1)
        .expect("tank should spawn");
    game.entities
        .get_mut(panzerfaust)
        .expect("panzerfaust exists")
        .set_invulnerable(true);
    refresh_world(&mut game);
    (game, panzerfaust, tank)
}

fn panzerfaust_fixture() -> (Game, u32, u32) {
    panzerfaust_fixture_at_tank_tile(11)
}

fn enqueue_attack(game: &mut Game, panzerfaust: u32, target: u32, queued: bool) {
    game.enqueue(
        1,
        Command::Attack {
            units: vec![panzerfaust],
            target,
            queued,
        },
    );
}

fn player_events(events: &[(u32, Vec<Event>)], player: u32) -> &[Event] {
    events
        .iter()
        .find(|(id, _)| *id == player)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[])
}

fn distance_sq(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

#[test]
fn spawned_panzerfaust_direct_attack_damages_tank_and_converts_same_id() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let tank_hp = game.entities.get(tank).expect("tank exists").hp;
    enqueue_attack(&mut game, panzerfaust, tank, false);

    let mut owner_saw_launch = false;
    let mut owner_saw_impact = false;
    let mut owner_saw_conversion = false;
    let mut uninvolved_saw_panzerfaust_event = false;
    let mut tank_hp_on_impact = None;
    for _ in 0..70 {
        let events = game.tick();
        owner_saw_launch |= player_events(&events, 1).iter().any(
            |event| matches!(event, Event::PanzerfaustLaunch { from, .. } if *from == panzerfaust),
        );
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && tank_hp_on_impact.is_none() {
            tank_hp_on_impact = game.entities.get(tank).map(|tank| tank.hp);
        }
        owner_saw_impact |= impact_this_tick;
        owner_saw_conversion |= player_events(&events, 1).iter().any(|event| {
            matches!(event, Event::PanzerfaustConversion { id, to_kind }
                if *id == panzerfaust && to_kind == crate::protocol::kinds::RIFLEMAN)
        });
        uninvolved_saw_panzerfaust_event |= player_events(&events, 3).iter().any(|event| {
            matches!(
                event,
                Event::PanzerfaustLaunch { .. }
                    | Event::PanzerfaustImpact { .. }
                    | Event::PanzerfaustConversion { .. }
            )
        });
    }

    assert_eq!(
        tank_hp_on_impact,
        Some(tank_hp.saturating_sub(config::PANZERFAUST_DAMAGE))
    );
    let converted = game
        .entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert_eq!(converted.kind, EntityKind::Rifleman);
    assert_eq!(converted.owner, 1);
    assert_eq!(converted.hp, 45);
    assert!(owner_saw_launch);
    assert!(owner_saw_impact);
    assert!(owner_saw_conversion);
    assert!(!uninvolved_saw_panzerfaust_event);
}

#[test]
fn direct_attack_conversion_completes_consumed_order_and_promotes_queued_move() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let start = game
        .entities
        .get(panzerfaust)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("panzerfaust exists");
    let move_goal = game.map.tile_center(20, 8);
    enqueue_attack(&mut game, panzerfaust, tank, false);
    game.enqueue(
        1,
        Command::Move {
            units: vec![panzerfaust],
            x: move_goal.0,
            y: move_goal.1,
            queued: true,
        },
    );

    let mut saw_conversion = false;
    for _ in 0..120 {
        let events = game.tick();
        saw_conversion |= player_events(&events, 1).iter().any(
            |event| matches!(event, Event::PanzerfaustConversion { id, .. } if *id == panzerfaust),
        );
    }

    let converted = game
        .entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert!(saw_conversion);
    assert_eq!(converted.kind, EntityKind::Rifleman);
    assert!(
        !matches!(converted.order(), Order::Attack(_)),
        "the consumed loaded shot should not leave a direct attack order active"
    );
    assert!(
        distance_sq((converted.pos_x, converted.pos_y), start) > 4.0,
        "queued movement should resume after same-id conversion"
    );
}

#[test]
fn spawned_panzerfaust_rejects_direct_attack_on_non_tank() {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(10, 8);
    let panzerfaust = game
        .entities
        .spawn_unit(
            1,
            EntityKind::Panzerfaust,
            panzerfaust_pos.0,
            panzerfaust_pos.1,
        )
        .expect("panzerfaust should spawn");
    let rifleman = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("rifleman should spawn");
    refresh_world(&mut game);

    enqueue_attack(&mut game, panzerfaust, rifleman, false);
    let events = game.tick();

    let panzerfaust_entity = game.entities.get(panzerfaust).expect("panzerfaust exists");
    assert_eq!(panzerfaust_entity.kind, EntityKind::Panzerfaust);
    assert_eq!(panzerfaust_entity.order(), Order::Idle);
    assert_eq!(panzerfaust_entity.target_id(), None);
    assert_eq!(game.entities.get(rifleman).expect("target exists").hp, 45);
    assert!(player_events(&events, 1)
        .iter()
        .all(|event| !matches!(event, Event::PanzerfaustLaunch { .. })));
}

#[test]
fn attack_move_acquires_tanks_but_plain_move_does_not_auto_fire() {
    let (mut moving_game, moving_panzerfaust, moving_tank) = panzerfaust_fixture();
    let moving_tank_hp = moving_game
        .entities
        .get(moving_tank)
        .expect("tank exists")
        .hp;
    let move_goal = moving_game.map.tile_center(20, 8);
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
            .entities
            .get(moving_tank)
            .expect("tank exists")
            .hp,
        moving_tank_hp
    );
    assert!(!move_launched);

    let (mut attack_move_game, attack_move_panzerfaust, attack_move_tank) = panzerfaust_fixture();
    let attack_move_tank_hp = attack_move_game
        .entities
        .get(attack_move_tank)
        .expect("tank exists")
        .hp;
    let attack_move_goal = attack_move_game.map.tile_center(20, 8);
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
                .entities
                .get(attack_move_tank)
                .map(|tank| tank.hp);
        }
    }
    assert_eq!(
        attack_move_impact_hp,
        Some(attack_move_tank_hp.saturating_sub(config::PANZERFAUST_DAMAGE))
    );
}

#[test]
fn methamphetamines_shortens_windup_and_recovery_timing() {
    fn conversion_tick(has_methamphetamines: bool) -> u32 {
        let (mut game, panzerfaust, tank) = panzerfaust_fixture();
        if has_methamphetamines {
            game.players
                .iter_mut()
                .find(|player| player.id == 1)
                .expect("player exists")
                .upgrades
                .insert(upgrade::UpgradeKind::Methamphetamines);
        }
        enqueue_attack(&mut game, panzerfaust, tank, false);
        for tick in 1..80 {
            let events = game.tick();
            if player_events(&events, 1)
                .iter()
                .any(|event| matches!(event, Event::PanzerfaustConversion { id, .. } if *id == panzerfaust))
            {
                return tick;
            }
        }
        panic!("Panzerfaust did not convert within test window");
    }

    let normal = conversion_tick(false);
    let boosted = conversion_tick(true);
    assert_eq!(
        normal - boosted,
        u32::from(
            config::PANZERFAUST_WINDUP_TICKS - config::METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS
        ) + u32::from(
            config::PANZERFAUST_RECOVERY_TICKS
                - config::METHAMPHETAMINES_PANZERFAUST_RECOVERY_TICKS
        )
    );
}

#[test]
fn hold_position_uses_only_current_entrenched_panzerfaust_range() {
    let (mut outside_game, outside_panzerfaust, outside_tank) =
        panzerfaust_fixture_at_tank_tile(12);
    let outside_start = outside_game
        .entities
        .get(outside_panzerfaust)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("panzerfaust exists");
    let outside_tank_hp = outside_game
        .entities
        .get(outside_tank)
        .expect("tank exists")
        .hp;
    outside_game
        .entities
        .get_mut(outside_panzerfaust)
        .expect("panzerfaust exists")
        .hold_position();
    let mut outside_launch = false;
    for _ in 0..50 {
        let events = outside_game.tick();
        outside_launch |= player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustLaunch { .. }));
    }
    let outside_panzerfaust_entity = outside_game
        .entities
        .get(outside_panzerfaust)
        .expect("panzerfaust exists");
    assert_eq!(
        (
            outside_panzerfaust_entity.pos_x,
            outside_panzerfaust_entity.pos_y
        ),
        outside_start
    );
    assert_eq!(
        outside_game
            .entities
            .get(outside_tank)
            .expect("tank exists")
            .hp,
        outside_tank_hp
    );
    assert!(!outside_launch);

    let (mut entrenched_game, entrenched_panzerfaust, entrenched_tank) =
        panzerfaust_fixture_at_tank_tile(12);
    let entrenched_tank_hp = entrenched_game
        .entities
        .get(entrenched_tank)
        .expect("tank exists")
        .hp;
    let trench_pos = entrenched_game
        .entities
        .get(entrenched_panzerfaust)
        .map(|panzerfaust| (panzerfaust.pos_x, panzerfaust.pos_y))
        .expect("panzerfaust exists");
    let trench = entrenched_game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");
    {
        let panzerfaust = entrenched_game
            .entities
            .get_mut(entrenched_panzerfaust)
            .expect("panzerfaust exists");
        panzerfaust.hold_position();
        panzerfaust
            .movement
            .as_mut()
            .expect("panzerfaust has movement")
            .occupied_trench_id = Some(trench);
    }
    let mut entrenched_impact_hp = None;
    for _ in 0..70 {
        let events = entrenched_game.tick();
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && entrenched_impact_hp.is_none() {
            entrenched_impact_hp = entrenched_game
                .entities
                .get(entrenched_tank)
                .map(|tank| tank.hp);
        }
    }
    assert_eq!(
        entrenched_impact_hp,
        Some(entrenched_tank_hp.saturating_sub(config::PANZERFAUST_DAMAGE))
    );
}

#[test]
fn replacing_order_during_windup_cancels_without_spending_shot() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let tank_hp = game.entities.get(tank).expect("tank exists").hp;
    enqueue_attack(&mut game, panzerfaust, tank, false);
    game.tick();

    let move_goal = game.map.tile_center(20, 8);
    game.enqueue(
        1,
        Command::Move {
            units: vec![panzerfaust],
            x: move_goal.0,
            y: move_goal.1,
            queued: false,
        },
    );
    let mut saw_launch = false;
    for _ in 0..60 {
        let events = game.tick();
        saw_launch |= player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustLaunch { .. }));
    }

    assert_eq!(game.entities.get(tank).expect("tank exists").hp, tank_hp);
    assert_eq!(
        game.entities
            .get(panzerfaust)
            .expect("panzerfaust exists")
            .kind,
        EntityKind::Panzerfaust
    );
    assert!(!saw_launch);
}

#[test]
fn target_death_during_travel_spends_shot_and_still_converts() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    enqueue_attack(&mut game, panzerfaust, tank, false);

    let mut saw_launch = false;
    for _ in 0..30 {
        let events = game.tick();
        if player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustLaunch { .. }))
        {
            saw_launch = true;
            break;
        }
    }
    assert!(
        saw_launch,
        "test setup should reach launch before killing target"
    );
    game.entities
        .get_mut(tank)
        .expect("tank exists")
        .apply_damage(u32::MAX, None);

    let mut saw_impact = false;
    let mut saw_conversion = false;
    for _ in 0..60 {
        let events = game.tick();
        saw_impact |= player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        saw_conversion |= player_events(&events, 1).iter().any(
            |event| matches!(event, Event::PanzerfaustConversion { id, .. } if *id == panzerfaust),
        );
    }

    assert!(game.entities.get(tank).is_none());
    assert_eq!(
        game.entities
            .get(panzerfaust)
            .expect("panzerfaust id should remain")
            .kind,
        EntityKind::Rifleman
    );
    assert!(saw_impact);
    assert!(saw_conversion);
}
