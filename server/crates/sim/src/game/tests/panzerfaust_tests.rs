use super::fixtures::*;
use super::*;
use crate::game::entity::PanzerfaustState;

fn player(id: u32, team_id: u32, name: &str, color: &str) -> PlayerInit {
    PlayerInit {
        id,
        team_id,
        faction_id: "kriegsia".to_string(),
        name: name.into(),
        color: color.into(),
        is_ai: false,
    }
}

fn panzerfaust_players() -> [PlayerInit; 3] {
    [
        player(1, 1, "One", "#fff"),
        player(2, 2, "Two", "#000"),
        player(3, 3, "Three", "#888"),
    ]
}

fn refresh_world(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute(&ids, &game.state.entities, &game.state.map);
}

fn spawn_unit_on_tile(
    game: &mut Game,
    owner: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> u32 {
    let pos = game.state.map.tile_center(tile_x, tile_y);
    game.state
        .entities
        .spawn_unit(owner, kind, pos.0, pos.1)
        .expect("unit should spawn")
}

fn make_invulnerable(game: &mut Game, id: u32) {
    game.state
        .entities
        .get_mut(id)
        .expect("unit should exist")
        .set_invulnerable(true);
}

fn panzerfaust_fixture_at_tank_tile(tank_tile_x: u32) -> (Game, u32, u32) {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust = spawn_unit_on_tile(&mut game, 1, EntityKind::Panzerfaust, 8, 8);
    let tank = spawn_unit_on_tile(&mut game, 2, EntityKind::Tank, tank_tile_x, 8);
    make_invulnerable(&mut game, panzerfaust);
    refresh_world(&mut game);
    (game, panzerfaust, tank)
}

pub(super) fn panzerfaust_fixture() -> (Game, u32, u32) {
    panzerfaust_fixture_at_tank_tile(11)
}

pub(super) fn enqueue_attack(game: &mut Game, panzerfaust: u32, target: u32, queued: bool) {
    game.enqueue(
        1,
        Command::Attack {
            units: vec![panzerfaust],
            target,
            queued,
        },
    );
}

pub(super) fn player_events(events: &[(u32, Vec<Event>)], player: u32) -> &[Event] {
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

fn panzerfaust_state_of(game: &Game, id: u32) -> Option<PanzerfaustState> {
    game.state.entities
        .get(id)
        .and_then(|entity| entity.combat.as_ref())
        .and_then(|combat| combat.panzerfaust)
}

pub(super) fn panzerfaust_damage_to(victim_kind: EntityKind) -> u32 {
    crate::rules::combat::effective_damage(
        EntityKind::Panzerfaust,
        victim_kind,
        config::PANZERFAUST_DAMAGE,
        Some(crate::rules::terrain::TerrainKind::Open),
    )
}

#[test]
fn spawned_panzerfaust_direct_attack_damages_tank_and_converts_same_id() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let tank_hp = game.state.entities.get(tank).expect("tank exists").hp;
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
            tank_hp_on_impact = game.state.entities.get(tank).map(|tank| tank.hp);
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
        Some(tank_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Tank)))
    );
    let converted = game.state.entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert_eq!(converted.kind, EntityKind::Rifleman);
    assert_eq!(converted.owner, 1);
    assert_eq!(converted.hp, 45);
    assert_eq!(
        converted
            .combat
            .as_ref()
            .expect("converted Rifleman should have combat state")
            .panzerfaust,
        None,
        "same-id conversion must clear loaded Panzerfaust timers and target filters"
    );
    assert!(owner_saw_launch);
    assert!(owner_saw_impact);
    assert!(owner_saw_conversion);
    assert!(!uninvolved_saw_panzerfaust_event);
}

#[test]
fn panzerfaust_direct_attack_can_damage_owned_tank_targets() {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust = spawn_unit_on_tile(&mut game, 1, EntityKind::Panzerfaust, 8, 8);
    let tank = spawn_unit_on_tile(&mut game, 1, EntityKind::Tank, 11, 8);
    make_invulnerable(&mut game, panzerfaust);
    refresh_world(&mut game);

    let tank_hp = game.state.entities.get(tank).expect("tank exists").hp;
    enqueue_attack(&mut game, panzerfaust, tank, false);

    let mut tank_hp_on_impact = None;
    let mut owner_saw_launch = false;
    let mut owner_saw_under_attack_notice = false;
    for _ in 0..70 {
        let events = game.tick();
        owner_saw_launch |= player_events(&events, 1).iter().any(
            |event| matches!(event, Event::PanzerfaustLaunch { from, .. } if *from == panzerfaust),
        );
        let owner_events = player_events(&events, 1);
        let impact_this_tick = owner_events
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && tank_hp_on_impact.is_none() {
            tank_hp_on_impact = game.state.entities.get(tank).map(|tank| tank.hp);
        }
        owner_saw_under_attack_notice |= owner_events.iter().any(
            |event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack"),
        );
    }

    assert!(owner_saw_launch);
    assert_eq!(
        tank_hp_on_impact,
        Some(tank_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Tank)))
    );
    assert!(
        !owner_saw_under_attack_notice,
        "deliberate self-attacks should not raise enemy under-attack alerts"
    );
}

#[test]
fn direct_attack_conversion_completes_consumed_order_and_promotes_queued_move() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let start = game.state.entities
        .get(panzerfaust)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("panzerfaust exists");
    let move_goal = game.state.map.tile_center(20, 8);
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

    let converted = game.state.entities
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
fn spawned_panzerfaust_rejects_direct_attack_on_non_loaded_shot_target() {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust = spawn_unit_on_tile(&mut game, 1, EntityKind::Panzerfaust, 8, 8);
    let rifleman = spawn_unit_on_tile(&mut game, 2, EntityKind::Rifleman, 10, 8);
    refresh_world(&mut game);

    enqueue_attack(&mut game, panzerfaust, rifleman, false);
    let events = game.tick();

    let panzerfaust_entity = game.state.entities.get(panzerfaust).expect("panzerfaust exists");
    assert_eq!(panzerfaust_entity.kind, EntityKind::Panzerfaust);
    assert_eq!(panzerfaust_entity.order(), Order::Idle);
    assert_eq!(panzerfaust_entity.target_id(), None);
    assert_eq!(game.state.entities.get(rifleman).expect("target exists").hp, 45);
    assert!(player_events(&events, 1)
        .iter()
        .all(|event| !matches!(event, Event::PanzerfaustLaunch { .. })));
}

#[test]
fn spawned_panzerfaust_direct_attack_damages_scout_car_and_converts_same_id() {
    let players = panzerfaust_players();
    let mut game = empty_flat_game(&players);
    let panzerfaust = spawn_unit_on_tile(&mut game, 1, EntityKind::Panzerfaust, 8, 8);
    let scout = spawn_unit_on_tile(&mut game, 2, EntityKind::ScoutCar, 11, 8);
    let scout_hp = game.state.entities.get(scout).expect("scout exists").hp;
    make_invulnerable(&mut game, panzerfaust);
    refresh_world(&mut game);

    enqueue_attack(&mut game, panzerfaust, scout, false);

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

    assert_eq!(
        panzerfaust_damage_to(EntityKind::ScoutCar),
        config::PANZERFAUST_DAMAGE
    );
    assert!(
        scout_hp <= panzerfaust_damage_to(EntityKind::ScoutCar),
        "Scout Car fixture should be destroyed by one Panzerfaust hit"
    );
    assert!(
        game.state.entities.get(scout).is_none(),
        "Scout Car should be a legal Panzerfaust target and die to the hit"
    );
    let converted = game.state.entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert_eq!(converted.kind, EntityKind::Rifleman);
    assert!(owner_saw_launch);
    assert!(owner_saw_scout_death);
}

#[test]
fn attack_move_acquires_tanks_but_plain_move_does_not_auto_fire() {
    let (mut moving_game, moving_panzerfaust, moving_tank) = panzerfaust_fixture();
    let moving_tank_hp = moving_game
        .state.entities
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
            .state.entities
            .get(moving_tank)
            .expect("tank exists")
            .hp,
        moving_tank_hp
    );
    assert!(!move_launched);

    let (mut attack_move_game, attack_move_panzerfaust, attack_move_tank) = panzerfaust_fixture();
    let attack_move_tank_hp = attack_move_game
        .state.entities
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
                .state.entities
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
fn methamphetamines_shortens_windup_and_recovery_timing() {
    fn conversion_tick(has_methamphetamines: bool) -> u32 {
        let (mut game, panzerfaust, tank) = panzerfaust_fixture();
        if has_methamphetamines {
            game.state.players
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
        .state.entities
        .get(outside_panzerfaust)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("panzerfaust exists");
    let outside_tank_hp = outside_game
        .state.entities
        .get(outside_tank)
        .expect("tank exists")
        .hp;
    outside_game
        .state.entities
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
        .state.entities
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
            .state.entities
            .get(outside_tank)
            .expect("tank exists")
            .hp,
        outside_tank_hp
    );
    assert!(!outside_launch);

    let (mut entrenched_game, entrenched_panzerfaust, entrenched_tank) =
        panzerfaust_fixture_at_tank_tile(12);
    let entrenched_tank_hp = entrenched_game
        .state.entities
        .get(entrenched_tank)
        .expect("tank exists")
        .hp;
    let trench_pos = entrenched_game
        .state.entities
        .get(entrenched_panzerfaust)
        .map(|panzerfaust| (panzerfaust.pos_x, panzerfaust.pos_y))
        .expect("panzerfaust exists");
    let trench = entrenched_game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");
    {
        let panzerfaust = entrenched_game
            .state.entities
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
    let mut saw_entrenched_conversion = false;
    for _ in 0..70 {
        let events = entrenched_game.tick();
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && entrenched_impact_hp.is_none() {
            entrenched_impact_hp = entrenched_game
                .state.entities
                .get(entrenched_tank)
                .map(|tank| tank.hp);
        }
        saw_entrenched_conversion |= player_events(&events, 1).iter().any(
            |event| matches!(event, Event::PanzerfaustConversion { id, .. } if *id == entrenched_panzerfaust),
        );
    }
    assert_eq!(
        entrenched_impact_hp,
        Some(entrenched_tank_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Tank)))
    );
    let converted = entrenched_game
        .state.entities
        .get(entrenched_panzerfaust)
        .expect("entrenched Panzerfaust should keep the same id");
    assert!(saw_entrenched_conversion);
    assert_eq!(converted.kind, EntityKind::Rifleman);
    assert_eq!(
        converted
            .movement
            .as_ref()
            .and_then(|movement| movement.occupied_trench_id),
        Some(trench),
        "same-id conversion should preserve active trench occupation"
    );
}

#[test]
fn replacing_order_during_windup_cancels_without_spending_shot() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let tank_hp = game.state.entities.get(tank).expect("tank exists").hp;
    enqueue_attack(&mut game, panzerfaust, tank, false);
    game.tick();

    let move_goal = game.state.map.tile_center(20, 8);
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

    assert_eq!(game.state.entities.get(tank).expect("tank exists").hp, tank_hp);
    assert_eq!(
        game.state.entities
            .get(panzerfaust)
            .expect("panzerfaust exists")
            .kind,
        EntityKind::Panzerfaust
    );
    assert!(!saw_launch);
}

#[test]
fn target_death_during_windup_cancels_without_spending_shot() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    enqueue_attack(&mut game, panzerfaust, tank, false);
    game.tick();

    game.state.entities
        .get_mut(tank)
        .expect("tank exists")
        .apply_damage(u32::MAX, None);
    let mut saw_launch = false;
    for _ in 0..70 {
        let events = game.tick();
        saw_launch |= player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustLaunch { .. }));
    }

    assert!(game.state.entities.get(tank).is_none());
    let panzerfaust_entity = game.state.entities
        .get(panzerfaust)
        .expect("panzerfaust should survive with its shot still loaded");
    assert_eq!(panzerfaust_entity.kind, EntityKind::Panzerfaust);
    assert!(!saw_launch);
}

#[test]
fn panzerfaust_killed_during_windup_does_not_launch_or_convert() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    game.state.entities
        .get_mut(panzerfaust)
        .expect("panzerfaust exists")
        .set_invulnerable(false);
    enqueue_attack(&mut game, panzerfaust, tank, false);
    game.tick();
    assert!(matches!(
        panzerfaust_state_of(&game, panzerfaust),
        Some(PanzerfaustState::Windup { .. })
    ));

    game.state.entities
        .get_mut(panzerfaust)
        .expect("panzerfaust exists")
        .apply_damage(u32::MAX, None);
    let mut saw_panzerfaust_combat_event = false;
    let mut saw_death_as_panzerfaust = false;
    for _ in 0..70 {
        let events = game.tick();
        saw_panzerfaust_combat_event |= player_events(&events, 1).iter().any(|event| {
            matches!(
                event,
                Event::PanzerfaustLaunch { .. }
                    | Event::PanzerfaustImpact { .. }
                    | Event::PanzerfaustConversion { .. }
            )
        });
        saw_death_as_panzerfaust |= player_events(&events, 1).iter().any(|event| {
            matches!(event, Event::Death { id, kind, .. }
                if *id == panzerfaust && kind == crate::protocol::kinds::PANZERFAUST)
        });
    }

    assert!(
        game.state.entities.get(panzerfaust).is_none(),
        "dead Panzerfaust should be removed instead of converting"
    );
    assert!(
        !saw_panzerfaust_combat_event,
        "dead Panzerfaust windup must not leak launch, impact, or conversion events"
    );
    assert!(
        saw_death_as_panzerfaust,
        "death cleanup should report the loaded unit kind that actually died"
    );
}

#[test]
fn panzerfaust_killed_during_recovery_does_not_convert_after_death() {
    let (mut game, panzerfaust, _tank) = panzerfaust_fixture();
    {
        let entity = game.state.entities
            .get_mut(panzerfaust)
            .expect("panzerfaust exists");
        entity.set_invulnerable(false);
        entity
            .combat
            .as_mut()
            .expect("panzerfaust has combat state")
            .panzerfaust = Some(PanzerfaustState::Recovery { ticks_remaining: 1 });
        entity.apply_damage(u32::MAX, None);
    }

    let events = game.tick();
    assert!(
        game.state.entities.get(panzerfaust).is_none(),
        "dead recovering Panzerfaust should be removed"
    );
    assert!(
        player_events(&events, 1)
            .iter()
            .all(|event| !matches!(event, Event::PanzerfaustConversion { .. })),
        "recovery should not convert after the entity has already died"
    );
    assert!(
        player_events(&events, 1).iter().any(|event| {
            matches!(event, Event::Death { id, kind, .. }
                if *id == panzerfaust && kind == crate::protocol::kinds::PANZERFAUST)
        }),
        "death cleanup should retain the Panzerfaust kind rather than converting first"
    );
}

#[test]
fn replacing_order_after_launch_spends_shot_and_resumes_after_conversion() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let start = game.state.entities
        .get(panzerfaust)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("panzerfaust exists");
    let tank_hp = game.state.entities.get(tank).expect("tank exists").hp;
    enqueue_attack(&mut game, panzerfaust, tank, false);

    let mut saw_launch = false;
    for _ in 0..40 {
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
        "test setup should reach launch before replacing the order"
    );

    let move_goal = game.state.map.tile_center(20, 8);
    game.enqueue(
        1,
        Command::Move {
            units: vec![panzerfaust],
            x: move_goal.0,
            y: move_goal.1,
            queued: false,
        },
    );

    let mut impact_hp = None;
    let mut saw_conversion = false;
    for _ in 0..130 {
        let events = game.tick();
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && impact_hp.is_none() {
            impact_hp = game.state.entities.get(tank).map(|tank| tank.hp);
        }
        saw_conversion |= player_events(&events, 1).iter().any(
            |event| matches!(event, Event::PanzerfaustConversion { id, .. } if *id == panzerfaust),
        );
    }

    assert_eq!(
        impact_hp,
        Some(tank_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Tank)))
    );
    let converted = game.state.entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert!(saw_conversion);
    assert_eq!(converted.kind, EntityKind::Rifleman);
    assert!(
        distance_sq((converted.pos_x, converted.pos_y), start) > 4.0,
        "replacement movement should resume after the spent shot completes"
    );
}

#[test]
fn impact_visual_uses_launch_endpoint_after_target_leaves_visibility() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let tank_hp = game.state.entities.get(tank).expect("tank exists").hp;
    enqueue_attack(&mut game, panzerfaust, tank, false);

    let mut launch_endpoint = None;
    for _ in 0..40 {
        let events = game.tick();
        if let Some(endpoint) = player_events(&events, 1)
            .iter()
            .find_map(|event| match event {
                Event::PanzerfaustLaunch { to_x, to_y, .. } => Some((*to_x, *to_y)),
                _ => None,
            })
        {
            launch_endpoint = Some(endpoint);
            break;
        }
    }
    let launch_endpoint = launch_endpoint.expect("test setup should reach launch");

    let moved_pos = game.state.map.tile_center(24, 8);
    {
        let tank_entity = game.state.entities.get_mut(tank).expect("tank exists");
        tank_entity.set_position(moved_pos.0, moved_pos.1);
        tank_entity.clear_path();
    }
    refresh_world(&mut game);

    let mut impact_pos = None;
    for _ in 0..40 {
        let events = game.tick();
        if let Some(pos) = player_events(&events, 1)
            .iter()
            .find_map(|event| match event {
                Event::PanzerfaustImpact { x, y } => Some((*x, *y)),
                _ => None,
            })
        {
            impact_pos = Some(pos);
            break;
        }
    }

    let impact_pos = impact_pos.expect("impact should still be emitted");
    assert!(
        distance_sq(impact_pos, launch_endpoint) < 0.01,
        "owner impact feedback should stay at the launch-safe endpoint"
    );
    assert!(
        distance_sq(impact_pos, moved_pos) > (config::TILE_SIZE * config::TILE_SIZE) as f32,
        "impact feedback should not reveal the target's new hidden position"
    );
    assert_eq!(
        game.state.entities.get(tank).expect("tank exists").hp,
        tank_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Tank))
    );
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
    game.state.entities
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

    assert!(game.state.entities.get(tank).is_none());
    assert_eq!(
        game.state.entities
            .get(panzerfaust)
            .expect("panzerfaust id should remain")
            .kind,
        EntityKind::Rifleman
    );
    assert!(saw_impact);
    assert!(saw_conversion);
}
