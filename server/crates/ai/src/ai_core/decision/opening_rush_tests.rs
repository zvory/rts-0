use super::*;

fn rifle(hp: u32, entrenched: bool) -> Fighter {
    Fighter {
        kind: EntityKind::Rifleman,
        hp,
        cooldown: 32,
        value: 35,
        entrenched,
    }
}

fn enemy(id: u32, kind: EntityKind, tiles_away: f32, hp: u32) -> AiEntitySummary {
    AiEntitySummary {
        industrial_center_slot_reserved: false,
        id,
        owner: 2,
        kind,
        x: tiles_away * 32.0,
        y: 0.0,
        hp,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

/// Units at `(id, kind, tiles east of home)`; home is tile (5, 5).
fn home_observation(
    own: &[(u32, EntityKind, f32)],
    enemies: &[(u32, EntityKind, f32)],
) -> AiObservation {
    use crate::ai_core::observation::{AiEconomy, AiMapSummary};
    let at = |id: u32, owner: u32, kind: EntityKind, tiles: f32| AiEntitySummary {
        owner,
        kind,
        x: (5.5 + tiles) * 32.0,
        y: 5.5 * 32.0,
        ..enemy(id, kind, 0.0, 45)
    };
    AiObservation {
        player_id: 1,
        tick: 5_000,
        map: AiMapSummary {
            width: 128,
            height: 128,
            tile_size: 32,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (5, 5),
        players: Vec::new(),
        owned: own
            .iter()
            .map(|(id, kind, t)| at(*id, 1, *kind, *t))
            .collect(),
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: enemies
            .iter()
            .map(|(id, kind, t)| at(*id, 2, *kind, *t))
            .collect(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

#[test]
fn a_raid_the_home_can_hold_does_not_recall_the_rush() {
    let squad: BTreeSet<u32> = (1..=4).collect();
    let rush: Vec<(u32, EntityKind, f32)> =
        (1..=4).map(|id| (id, EntityKind::Rifleman, 70.0)).collect();
    let raid: Vec<(u32, EntityKind, f32)> = (90..94)
        .map(|id| (id, EntityKind::Rifleman, 25.0))
        .collect();
    // Four enemy Riflemen near the natural against six Riflemen and four Machine Gunners home.
    let mut own = rush.clone();
    own.extend((10..16).map(|id| (id, EntityKind::Rifleman, 8.0)));
    own.extend((20..24).map(|id| (id, EntityKind::MachineGunner, 6.0)));
    assert!(!raid_outmatches_home(
        &home_observation(&own, &raid),
        &squad
    ));
    // Three Riflemen home do not hold it.
    let mut own = rush.clone();
    own.extend((10..13).map(|id| (id, EntityKind::Rifleman, 8.0)));
    assert!(raid_outmatches_home(&home_observation(&own, &raid), &squad));
    // A Tank counts as four.
    own.push((30, EntityKind::Tank, 8.0));
    own.push((31, EntityKind::Rifleman, 8.0));
    assert!(!raid_outmatches_home(
        &home_observation(&own, &raid),
        &squad
    ));
}

#[test]
fn three_riflemen_take_a_good_trade_against_one_dug_in_rifleman() {
    let ours = [rifle(45, false); 3];
    let theirs = [rifle(45, true)];
    let estimate = estimate_trade(&ours, &theirs, 20);
    assert_eq!(estimate.units_killed, 1);
    assert_eq!(estimate.units_lost, 0);
    assert!(!estimate.clearly_losing());
}

#[test]
fn a_lone_rifleman_loses_to_a_dug_in_rifleman() {
    let estimate = estimate_trade(&[rifle(45, false)], &[rifle(45, true)], 20);
    assert_eq!(estimate.units_lost, 1);
    assert_eq!(estimate.units_killed, 0);
    assert!(estimate.clearly_losing());
}

#[test]
fn an_even_open_fight_is_taken_but_being_outnumbered_two_to_one_is_not() {
    let four = [rifle(45, false); 4];
    assert!(!estimate_trade(&four, &four, 0).clearly_losing());
    let two = [rifle(45, false); 2];
    assert!(estimate_trade(&two, &four, 0).clearly_losing());
    assert!(!estimate_trade(&four, &two, 0).clearly_losing());
}

#[test]
fn wounded_squad_against_fresh_enemies_is_a_bad_trade() {
    let wounded = [rifle(10, false); 4];
    let fresh = [rifle(45, false); 3];
    assert!(estimate_trade(&wounded, &fresh, 0).clearly_losing());
}

#[test]
fn a_dug_in_denial_holds_unless_it_would_die_without_a_kill() {
    // Four in trenches against six fresh Riflemen take some with them, so they stay.
    let outnumbered = estimate_trade(&[rifle(45, true); 4], &[rifle(45, false); 6], 0);
    assert!(outnumbered.units_killed > 0);
    assert!(!deny_withdraws(outnumbered, 4, 4));
    assert!(!deny_withdraws(outnumbered, 4, 2));
    // A badly wounded Rifleman in its trench against four dies without a kill, so it goes.
    let hopeless = estimate_trade(&[rifle(5, true)], &[rifle(45, false); 4], 0);
    assert!(hopeless.hopeless(1));
    assert!(deny_withdraws(hopeless, 1, 1));
}

#[test]
fn a_denial_not_yet_dug_in_leaves_only_a_clearly_lost_fight() {
    let trade = |lost: usize, killed: usize| TradeEstimate {
        value_lost: lost as u32 * 35,
        value_killed: killed as u32 * 35,
        units_lost: lost,
        units_killed: killed,
    };
    // One of four in a trench is not dug in: three for two is taken, three for one is not.
    assert!(!deny_withdraws(trade(3, 2), 4, 1));
    assert!(deny_withdraws(trade(3, 1), 4, 1));
    // Two of four is half the squad dug in: three for one is held.
    assert!(!deny_withdraws(trade(3, 1), 4, 2));
    assert!(deny_withdraws(trade(4, 0), 4, 2));
}

#[test]
fn a_dug_in_squad_holds_against_as_many_attackers() {
    let dug_in = [rifle(45, true); 4];
    let attackers = [rifle(45, false); 4];
    let estimate = estimate_trade(&dug_in, &attackers, 0);
    assert_eq!(estimate.units_killed, 4);
    assert!(!estimate.clearly_losing());
}

#[test]
fn denial_shoots_the_builder_first_then_fighters_then_extractors_then_the_depot() {
    let tile = 32.0;
    let reach = rifle_reach_px(tile);
    let mut enemies = vec![
        enemy(1, EntityKind::ResourceDepot, 4.0, 100),
        enemy(2, EntityKind::SteelMine, 4.0, 37),
        enemy(3, EntityKind::Rifleman, 4.0, 45),
        enemy(4, EntityKind::Worker, 4.5, 40),
    ];
    assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), Some(4));
    enemies.retain(|e| e.id != 4);
    assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), Some(3));
    enemies.retain(|e| e.id != 3);
    assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), Some(2));
    enemies.retain(|e| e.id != 2);
    assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), Some(1));
}

#[test]
fn denial_never_targets_what_is_out_of_reach() {
    let tile = 32.0;
    let reach = rifle_reach_px(tile);
    let enemies = vec![enemy(4, EntityKind::Worker, 7.0, 40)];
    assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), None);
    let dug_in = reach + tile;
    assert_eq!(
        pick_deny_target(&enemies, (0.0, 0.0), dug_in, tile),
        None,
        "seven tiles is beyond even a dug-in Rifleman"
    );
    let close = vec![enemy(4, EntityKind::Worker, 6.0, 40)];
    assert_eq!(pick_deny_target(&close, (0.0, 0.0), dug_in, tile), Some(4));
}

#[test]
fn the_middle_of_the_map_is_equally_far_from_both_bases() {
    let own = (0.0, 0.0);
    let enemy = (1000.0, 0.0);
    assert!((march_progress((500.0, 0.0), own, enemy) - 0.5).abs() < 1e-6);
    assert!(march_progress((400.0, 0.0), own, enemy) < MISSED_CONTACT_PROGRESS);
    // A lane off the straight line still reaches the middle where both distances match.
    assert!((march_progress((500.0, 300.0), own, enemy) - 0.5).abs() < 1e-6);
    assert!(march_progress((600.0, 300.0), own, enemy) > MISSED_CONTACT_PROGRESS);
}

#[test]
fn emergency_recruiting_stops_once_its_riflemen_are_trained_or_queued() {
    let mut rush = OpeningRush {
        emergency_rifle_target: Some(6),
        ..OpeningRush::default()
    };
    let mut priorities = vec![EntityKind::Tank, EntityKind::Rifleman];
    assert_eq!(rush.lead_with_emergency_riflemen(&mut priorities), Some(6));
    assert_eq!(priorities, vec![EntityKind::Rifleman, EntityKind::Tank]);
    let counts = |riflemen| vec![(EntityKind::Rifleman, riflemen)];
    assert_eq!(rush.unmet_emergency_target(&counts(5)), Some(6));
    assert_eq!(rush.unmet_emergency_target(&counts(6)), None);
    // Later losses do not restart it.
    assert_eq!(rush.unmet_emergency_target(&counts(2)), None);
    let mut untouched = vec![EntityKind::Tank];
    assert_eq!(rush.lead_with_emergency_riflemen(&mut untouched), None);
    assert_eq!(untouched, vec![EntityKind::Tank]);
    let mut caps = vec![(EntityKind::MachineGunner, 4), (EntityKind::Rifleman, 3)];
    raise_rifle_cap(&mut caps, 6);
    assert_eq!(caps[1], (EntityKind::Rifleman, 6));
}

/// Withdraw orders for a marching four-Rifleman squad 40 tiles east of home, with enemy
/// Riflemen at `raid_tiles` east of home.
fn withdraw_commands(raid_tiles: f32, live_jeff: bool, rush: &mut OpeningRush) -> Vec<Command> {
    let own: Vec<(u32, EntityKind, f32)> =
        (1..=4).map(|id| (id, EntityKind::Rifleman, 40.0)).collect();
    let raid: Vec<(u32, EntityKind, f32)> = (90..94)
        .map(|id| (id, EntityKind::Rifleman, raid_tiles))
        .collect();
    let mut observation = home_observation(&own, &raid);
    for unit in &mut observation.owned {
        unit.state = AiEntityState::Move;
    }
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let squad: Vec<&AiEntitySummary> = observation.owned.iter().collect();
    let near: Vec<&AiEntitySummary> = observation.visible_enemies.iter().collect();
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let mut decision = RushDecision::default();
    withdraw(
        &mut actions,
        &observation,
        None,
        rush,
        &squad,
        &near,
        home,
        &mut decision,
        live_jeff,
    );
    actions.into_commands()
}

#[test]
fn a_withdrawing_live_squad_attacks_a_raid_between_it_and_home() {
    let withdrawing = || OpeningRush {
        phase: RushPhase::Withdraw,
        withdraw_reason: Some(WithdrawReason::Entrenchment),
        ..OpeningRush::default()
    };
    let attacks = |commands: &[Command]| {
        commands
            .iter()
            .any(|command| matches!(command, Command::AttackMove { .. }))
    };
    // Raiders 20 tiles from home, the squad 40: the live Jeff's squad goes in behind them.
    let mut rush = withdrawing();
    let commands = withdraw_commands(20.0, true, &mut rush);
    assert!(attacks(&commands), "{commands:?}");
    // While it marches on them it is not re-ordered.
    let commands = withdraw_commands(20.0, true, &mut rush);
    assert!(commands.is_empty(), "{commands:?}");
    // Other profiles still step around them.
    let commands = withdraw_commands(20.0, false, &mut withdrawing());
    assert!(!attacks(&commands), "{commands:?}");
    assert!(commands
        .iter()
        .any(|command| matches!(command, Command::Move { .. })));
    // Enemies beyond the squad are not raiding the base: the live squad steps around them too.
    let commands = withdraw_commands(55.0, true, &mut withdrawing());
    assert!(!attacks(&commands), "{commands:?}");
}

#[test]
fn only_the_live_jeff_rushes() {
    assert!(uses_opening_rush(JEFFS_AI_ID));
    assert!(!uses_opening_rush(JEFFS_AI_PRE_OPENING_RUSH_ID));
    assert!(!uses_opening_rush(crate::ai_core::profiles::AI_2_1_ID));
}
