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

pub(super) fn panzerfaust_players() -> [PlayerInit; 3] {
    [
        player(1, 1, "One", "#fff"),
        player(2, 2, "Two", "#000"),
        player(3, 3, "Three", "#888"),
    ]
}

pub(super) fn refresh_world(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
}

pub(super) fn spawn_unit_on_tile(
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

pub(super) fn spawn_building_on_tile(
    game: &mut Game,
    owner: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> u32 {
    let pos =
        crate::game::services::occupancy::footprint_center(&game.state.map, kind, tile_x, tile_y);
    game.state
        .entities
        .spawn_building(owner, kind, pos.0, pos.1, true)
        .expect("building should spawn")
}

pub(super) fn make_invulnerable(game: &mut Game, id: u32) {
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
    game.state
        .entities
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
fn spawned_panzerfaust_direct_attack_damages_tank_reloads_and_fires_again() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let tank_hp = game.state.entities.get(tank).expect("tank exists").hp;
    enqueue_attack(&mut game, panzerfaust, tank, false);

    let mut owner_launch_ticks = Vec::new();
    let mut owner_saw_impact = false;
    let mut owner_saw_conversion = false;
    let mut uninvolved_saw_panzerfaust_event = false;
    let mut tank_hp_on_impact = None;
    let mut snapshot_reported_unloaded = false;
    for tick in 1..160 {
        let events = game.tick();
        if player_events(&events, 1).iter().any(
            |event| matches!(event, Event::PanzerfaustLaunch { from, .. } if *from == panzerfaust),
        ) {
            owner_launch_ticks.push(tick);
        }
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && tank_hp_on_impact.is_none() {
            tank_hp_on_impact = game.state.entities.get(tank).map(|tank| tank.hp);
            snapshot_reported_unloaded = game
                .snapshot_for(1)
                .entities
                .iter()
                .find(|entity| entity.id == panzerfaust)
                .and_then(|entity| entity.panzerfaust_loaded)
                == Some(false);
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
    let reloading = game
        .state
        .entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert_eq!(reloading.kind, EntityKind::Panzerfaust);
    assert_eq!(reloading.owner, 1);
    assert_eq!(reloading.hp, 45);
    assert!(
        reloading
            .combat
            .as_ref()
            .expect("Panzerfaust should have combat state")
            .panzerfaust
            .is_some(),
        "Panzerfaust should keep its loaded-shot runtime instead of converting"
    );
    assert!(owner_launch_ticks.len() >= 2);
    assert!(
        owner_launch_ticks[1]
            > owner_launch_ticks[0]
                + config::PANZERFAUST_TRAVEL_TICKS
                + u32::from(config::PANZERFAUST_RECOVERY_TICKS),
        "second launch should wait for projectile travel plus the full reload"
    );
    assert!(owner_saw_impact);
    assert!(!owner_saw_conversion);
    assert!(snapshot_reported_unloaded);
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
        owner_saw_under_attack_notice |= owner_events
            .iter()
            .any(|event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack"));
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
fn direct_attack_reload_keeps_attack_order_and_delays_queued_move() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let start = game
        .state
        .entities
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

    for _ in 0..120 {
        game.tick();
    }

    let reloading = game
        .state
        .entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert_eq!(reloading.kind, EntityKind::Panzerfaust);
    assert!(
        matches!(reloading.order(), Order::Attack(_)),
        "reloadable Panzerfaust should keep the direct attack until the target is gone"
    );
    assert_eq!(
        reloading.queued_orders().len(),
        1,
        "queued movement should remain queued while the direct attack is active"
    );
    assert!(
        reloading.path_is_empty(),
        "queued movement should not start pathing while the direct attack remains active"
    );
    assert!(
        distance_sq((reloading.pos_x, reloading.pos_y), start) < 16.0,
        "direct attack should only allow incidental combat-position drift before the queued move starts"
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

    let panzerfaust_entity = game
        .state
        .entities
        .get(panzerfaust)
        .expect("panzerfaust exists");
    assert_eq!(panzerfaust_entity.kind, EntityKind::Panzerfaust);
    assert_eq!(panzerfaust_entity.order(), Order::Idle);
    assert_eq!(panzerfaust_entity.target_id(), None);
    assert_eq!(
        game.state.entities.get(rifleman).expect("target exists").hp,
        45
    );
    assert!(player_events(&events, 1)
        .iter()
        .all(|event| !matches!(event, Event::PanzerfaustLaunch { .. })));
}

#[test]
fn spawned_panzerfaust_direct_attack_damages_scout_car_and_reloads() {
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
    let reloading = game
        .state
        .entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert_eq!(reloading.kind, EntityKind::Panzerfaust);
    assert!(owner_saw_launch);
    assert!(owner_saw_scout_death);
}

#[test]
fn methamphetamines_shortens_windup_but_not_reload_timing() {
    fn launch_tick(has_methamphetamines: bool) -> u32 {
        let (mut game, panzerfaust, tank) = panzerfaust_fixture();
        if has_methamphetamines {
            game.state
                .players
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
                .any(|event| matches!(event, Event::PanzerfaustLaunch { from, .. } if *from == panzerfaust))
            {
                return tick;
            }
        }
        panic!("Panzerfaust did not launch within test window");
    }

    let normal = launch_tick(false);
    let boosted = launch_tick(true);
    assert_eq!(
        normal - boosted,
        u32::from(
            config::PANZERFAUST_WINDUP_TICKS - config::METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS
        )
    );
    assert_eq!(
        config::PANZERFAUST_RECOVERY_TICKS,
        config::METHAMPHETAMINES_PANZERFAUST_RECOVERY_TICKS
    );
}

#[test]
fn hold_position_uses_only_current_entrenched_panzerfaust_range() {
    let (mut outside_game, outside_panzerfaust, outside_tank) =
        panzerfaust_fixture_at_tank_tile(12);
    let outside_start = outside_game
        .state
        .entities
        .get(outside_panzerfaust)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("panzerfaust exists");
    let outside_tank_hp = outside_game
        .state
        .entities
        .get(outside_tank)
        .expect("tank exists")
        .hp;
    outside_game
        .state
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
        .state
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
            .state
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
        .state
        .entities
        .get(entrenched_tank)
        .expect("tank exists")
        .hp;
    let trench_pos = entrenched_game
        .state
        .entities
        .get(entrenched_panzerfaust)
        .map(|panzerfaust| (panzerfaust.pos_x, panzerfaust.pos_y))
        .expect("panzerfaust exists");
    let trench = entrenched_game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");
    {
        let panzerfaust = entrenched_game
            .state
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
    let mut saw_entrenched_conversion = false;
    for _ in 0..70 {
        let events = entrenched_game.tick();
        let impact_this_tick = player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustImpact { .. }));
        if impact_this_tick && entrenched_impact_hp.is_none() {
            entrenched_impact_hp = entrenched_game
                .state
                .entities
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
    let reloading = entrenched_game
        .state
        .entities
        .get(entrenched_panzerfaust)
        .expect("entrenched Panzerfaust should keep the same id");
    assert!(!saw_entrenched_conversion);
    assert_eq!(reloading.kind, EntityKind::Panzerfaust);
    assert_eq!(
        reloading
            .movement
            .as_ref()
            .and_then(|movement| movement.occupied_trench_id),
        Some(trench),
        "reload should preserve active trench occupation"
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

    assert_eq!(
        game.state.entities.get(tank).expect("tank exists").hp,
        tank_hp
    );
    assert_eq!(
        game.state
            .entities
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

    game.state
        .entities
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
    let panzerfaust_entity = game
        .state
        .entities
        .get(panzerfaust)
        .expect("panzerfaust should survive with its shot still loaded");
    assert_eq!(panzerfaust_entity.kind, EntityKind::Panzerfaust);
    assert!(!saw_launch);
}

#[test]
fn panzerfaust_killed_during_windup_does_not_launch() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    game.state
        .entities
        .get_mut(panzerfaust)
        .expect("panzerfaust exists")
        .set_invulnerable(false);
    enqueue_attack(&mut game, panzerfaust, tank, false);
    game.tick();
    assert!(matches!(
        panzerfaust_state_of(&game, panzerfaust),
        Some(PanzerfaustState::Windup { .. })
    ));

    game.state
        .entities
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
        "dead Panzerfaust should be removed"
    );
    assert!(
        !saw_panzerfaust_combat_event,
        "dead Panzerfaust windup must not leak launch, impact, or legacy conversion events"
    );
    assert!(
        saw_death_as_panzerfaust,
        "death cleanup should report the loaded unit kind that actually died"
    );
}

#[test]
fn panzerfaust_killed_during_reload_does_not_emit_conversion_after_death() {
    let (mut game, panzerfaust, _tank) = panzerfaust_fixture();
    {
        let entity = game
            .state
            .entities
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
        "reload should not emit conversion after the entity has already died"
    );
    assert!(
        player_events(&events, 1).iter().any(|event| {
            matches!(event, Event::Death { id, kind, .. }
                if *id == panzerfaust && kind == crate::protocol::kinds::PANZERFAUST)
        }),
        "death cleanup should retain the Panzerfaust kind"
    );
}

#[test]
fn replacing_order_after_launch_spends_shot_and_resumes_movement_during_reload() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    let start = game
        .state
        .entities
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
    let reloading = game
        .state
        .entities
        .get(panzerfaust)
        .expect("same entity id should remain");
    assert!(!saw_conversion);
    assert_eq!(reloading.kind, EntityKind::Panzerfaust);
    assert!(
        distance_sq((reloading.pos_x, reloading.pos_y), start) > 4.0,
        "replacement movement should resume while the spent shot reloads"
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
fn target_death_during_travel_spends_shot_and_reloads() {
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
    game.state
        .entities
        .get_mut(tank)
        .expect("tank exists")
        .apply_damage(u32::MAX, None);

    let mut saw_impact = false;
    let mut saw_conversion = false;
    for _ in 0..120 {
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
        game.state
            .entities
            .get(panzerfaust)
            .expect("panzerfaust id should remain")
            .kind,
        EntityKind::Panzerfaust
    );
    assert!(saw_impact);
    assert!(!saw_conversion);
    assert_eq!(
        panzerfaust_state_of(&game, panzerfaust),
        Some(PanzerfaustState::Loaded)
    );
}
