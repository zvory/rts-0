use super::*;
use crate::ai_core::observation::{AiBuildIntent, AiBuildIntentPhase, AiEconomy, AiMapSummary};
use crate::ai_core::profiles::JEFFS_AI;

fn ts() -> f32 {
    config::TILE_SIZE as f32
}

fn entity(id: u32, owner: u32, kind: EntityKind, tiles: (f32, f32)) -> AiEntitySummary {
    let hp = config::unit_stats(kind)
        .map(|stats| stats.hp)
        .or_else(|| config::building_stats(kind).map(|stats| stats.hp))
        .unwrap_or(100);
    AiEntitySummary {
        id,
        owner,
        kind,
        x: tiles.0 * ts(),
        y: tiles.1 * ts(),
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

/// Jeff's main at (10, 10) with its Depot and a Steel Mine three tiles east; the enemy far to the
/// south-east.
fn base(tick: u32) -> AiObservation {
    AiObservation {
        player_id: 1,
        tick,
        map: AiMapSummary {
            width: 80,
            height: 80,
            tile_size: config::TILE_SIZE,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (10, 10),
        players: Vec::new(),
        owned: vec![
            entity(1, 1, EntityKind::ResourceDepot, (10.5, 10.5)),
            entity(2, 1, EntityKind::SteelMine, (14.5, 10.5)),
        ],
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn respond_commands(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    defenders: &[u32],
) -> (Option<Vec<u32>>, Vec<Command>) {
    respond_with(observation, memory, defenders, true)
}

fn respond_with(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    defenders: &[u32],
    resend: bool,
) -> (Option<Vec<u32>>, Vec<Command>) {
    let facts = AiFacts::from_observation(observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let answer = match respond(&mut actions, observation, memory, defenders, None, resend) {
        RaidAnswer::NotARaid => None,
        RaidAnswer::Handled(units) => Some(units),
    };
    (answer, actions.into_commands())
}

fn live_memory() -> AiDecisionMemory {
    AiDecisionMemory::for_profile(&JEFFS_AI)
}

#[test]
fn a_raid_fourteen_tiles_out_is_answered_before_it_reaches_the_extractors() {
    let mut observation = base(1_200);
    // Five Riflemen in the frontal pocket, west of the Depot.
    for (i, id) in (20..25).enumerate() {
        observation
            .owned
            .push(entity(id, 1, EntityKind::Rifleman, (4.0, 8.0 + i as f32)));
    }
    // Three raiders north-east of the Steel Mine, 14 tiles from it.
    for (i, id) in (90..93).enumerate() {
        observation.visible_enemies.push(entity(
            id,
            2,
            EntityKind::Rifleman,
            (24.0 + i as f32, 0.5),
        ));
    }
    let mut memory = live_memory();
    let (answer, commands) = respond_commands(&observation, &mut memory, &[20, 21, 22, 23, 24]);
    let units = answer.expect("the raid is answered");
    assert!(units.len() >= 4, "{units:?}");
    // Nobody is sent to attack yet: the response gathers first.
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::Attack { .. } | Command::AttackMove { .. })),
        "{commands:?}"
    );
    assert!(commands
        .iter()
        .any(|command| matches!(command, Command::Move { .. })));
}

#[test]
fn the_response_gathers_outside_the_raid_reach_and_then_goes_in_together() {
    let mut observation = base(1_200);
    for (i, id) in (20..26).enumerate() {
        observation
            .owned
            .push(entity(id, 1, EntityKind::Rifleman, (4.0, 6.0 + i as f32)));
    }
    for (i, id) in (90..93).enumerate() {
        observation.visible_enemies.push(entity(
            id,
            2,
            EntityKind::Rifleman,
            (17.0 + i as f32, 4.0),
        ));
    }
    let defenders: Vec<u32> = (20..26).collect();
    let mut memory = live_memory();
    let (_, commands) = respond_commands(&observation, &mut memory, &defenders);
    let gather = commands
        .iter()
        .find_map(|command| match command {
            Command::Move { x, y, .. } => Some((*x, *y)),
            _ => None,
        })
        .expect("defenders gather first");
    let raid_center = (18.0 * ts(), 4.0 * ts());
    let reach = reach_px(EntityKind::Rifleman, ts(), false);
    assert!(
        distance(gather, raid_center) >= reach,
        "gathering point {:.1} tiles from the raid",
        distance(gather, raid_center) / ts()
    );

    // Everyone arrives: the response engages together.
    observation.tick += 9;
    for unit in observation
        .owned
        .iter_mut()
        .filter(|unit| defenders.contains(&unit.id))
    {
        unit.x = gather.0;
        unit.y = gather.1;
    }
    let (_, commands) = respond_commands(&observation, &mut memory, &defenders);
    let engaged: Vec<u32> = commands
        .iter()
        .filter_map(|command| match command {
            Command::AttackMove { units, .. } => Some(units.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(engaged.len() >= 4, "{commands:?}");
}

#[test]
fn a_raid_that_backs_off_is_not_chased() {
    let mut observation = base(1_200);
    for (i, id) in (20..26).enumerate() {
        observation.owned.push(entity(
            id,
            1,
            EntityKind::Rifleman,
            (14.0 + i as f32 * 0.5, 6.0),
        ));
    }
    for (i, id) in (90..93).enumerate() {
        observation.visible_enemies.push(entity(
            id,
            2,
            EntityKind::Rifleman,
            (17.0 + i as f32, 1.0),
        ));
    }
    let defenders: Vec<u32> = (20..26).collect();
    let mut memory = live_memory();
    respond_commands(&observation, &mut memory, &defenders);
    // The raid falls back to 13+ tiles from anything Jeff owns.
    observation.tick += 9;
    for (i, enemy) in observation.visible_enemies.iter_mut().enumerate() {
        enemy.x = (27.0 + i as f32) * ts();
        enemy.y = -0.0;
        enemy.state = AiEntityState::Move;
    }
    let (answer, commands) = respond_commands(&observation, &mut memory, &defenders);
    assert!(answer.is_some(), "the raid is still watched");
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::AttackMove { .. } | Command::Attack { .. })),
        "{commands:?}"
    );
}

#[test]
fn a_raid_already_at_the_dug_in_line_is_left_to_the_line() {
    let mut observation = base(1_200);
    observation
        .owned
        .push(entity(30, 1, EntityKind::MachineGunner, (17.0, 10.5)));
    observation
        .visible_enemies
        .push(entity(90, 2, EntityKind::Rifleman, (21.0, 10.5)));
    let mut memory = live_memory();
    let (answer, _) = respond_commands(&observation, &mut memory, &[30]);
    assert!(answer.is_none());
}

#[test]
fn a_raid_with_a_tank_is_left_to_the_tank_defense() {
    let mut observation = base(1_200);
    observation
        .owned
        .push(entity(20, 1, EntityKind::Rifleman, (4.0, 8.0)));
    observation.visible_enemies.extend([
        entity(90, 2, EntityKind::Rifleman, (24.0, 10.5)),
        entity(91, 2, EntityKind::Tank, (25.0, 11.5)),
    ]);
    let mut memory = live_memory();
    let (answer, _) = respond_commands(&observation, &mut memory, &[20]);
    assert!(answer.is_none());
}

fn outranged_commands(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    units: &[u32],
) -> (Vec<u32>, Vec<Command>) {
    let facts = AiFacts::from_observation(observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let held = answer_outranged(&mut actions, observation, memory, None, units);
    (held, actions.into_commands())
}

#[test]
fn a_lone_outranged_rifleman_steps_back_out_of_reach() {
    // Past the earliest Entrenchment, a standing enemy Rifleman may be dug in and reach 6 tiles.
    let mut observation = base(3_000);
    observation
        .owned
        .push(entity(20, 1, EntityKind::Rifleman, (20.0, 20.0)));
    observation
        .visible_enemies
        .push(entity(90, 2, EntityKind::Rifleman, (26.0, 20.0)));
    let mut memory = live_memory();
    let (held, commands) = outranged_commands(&observation, &mut memory, &[20]);
    assert_eq!(held, vec![20]);
    let (x, y) = commands
        .iter()
        .find_map(|command| match command {
            Command::Move { units, x, y, .. } if units == &[20] => Some((*x, *y)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("steps back: {commands:?}"));
    let reach = reach_px(EntityKind::Rifleman, ts(), true);
    assert!(distance((x, y), (26.0 * ts(), 20.0 * ts())) > reach);
}

#[test]
fn outranged_riflemen_with_the_numbers_step_in_to_fire() {
    let mut observation = base(3_000);
    for (i, id) in (20..24).enumerate() {
        observation.owned.push(entity(
            id,
            1,
            EntityKind::Rifleman,
            (20.0, 19.0 + i as f32 * 0.7),
        ));
    }
    observation
        .visible_enemies
        .push(entity(90, 2, EntityKind::Rifleman, (26.0, 20.0)));
    let mut memory = live_memory();
    let (_, commands) = outranged_commands(&observation, &mut memory, &[20, 21, 22, 23]);
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::AttackMove { units, .. } if units == &[20])),
        "{commands:?}"
    );
}

#[test]
fn a_rifleman_in_reach_of_its_shooter_is_left_alone() {
    let mut observation = base(3_000);
    observation
        .owned
        .push(entity(20, 1, EntityKind::Rifleman, (20.0, 20.0)));
    observation
        .visible_enemies
        .push(entity(90, 2, EntityKind::Rifleman, (24.5, 20.0)));
    let mut memory = live_memory();
    let (held, commands) = outranged_commands(&observation, &mut memory, &[20]);
    assert!(held.is_empty() && commands.is_empty(), "{commands:?}");
}

#[test]
fn enemy_infantry_seen_early_raises_the_rifle_target_until_matched() {
    let mut observation = base(2_000);
    for (i, id) in (90..95).enumerate() {
        observation.visible_enemies.push(entity(
            id,
            2,
            EntityKind::Rifleman,
            (30.0 + i as f32, 10.5),
        ));
    }
    let mut memory = live_memory();
    // Five raiders seen, two Riflemen at home: train five more, and Riflemen lead production.
    observation.owned.extend([
        entity(20, 1, EntityKind::Rifleman, (11.0, 12.0)),
        entity(21, 1, EntityKind::Rifleman, (12.0, 12.0)),
    ]);
    let mut priorities = vec![EntityKind::Tank, EntityKind::Rifleman];
    let answer = rifle_answer(JEFFS_AI_ID, &observation, &mut memory, &mut priorities);
    assert_eq!(answer, Some(7));
    assert_eq!(priorities, vec![EntityKind::Rifleman, EntityKind::Tank]);
    // Other profiles never do.
    let mut other = vec![EntityKind::Tank];
    assert_eq!(
        rifle_answer("ai_2_1", &observation, &mut memory, &mut other),
        None
    );
    assert_eq!(other, vec![EntityKind::Tank]);
    // Seven at home match five raiders with two to spare.
    for id in 22..27 {
        observation
            .owned
            .push(entity(id, 1, EntityKind::Rifleman, (11.0, 13.0)));
    }
    let answer = rifle_answer(JEFFS_AI_ID, &observation, &mut memory, &mut priorities);
    assert_eq!(answer, None);
    // Not after the opening minutes.
    observation.tick = INFANTRY_ANSWER_UNTIL_TICK;
    observation.owned.truncate(4);
    let answer = rifle_answer(JEFFS_AI_ID, &observation, &mut memory, &mut priorities);
    assert_eq!(answer, None);
}

#[test]
fn sentries_take_posts_off_both_flanks_as_home_riflemen_come_free() {
    let mut observation = base(3_000);
    for (i, id) in (20..26).enumerate() {
        observation
            .owned
            .push(entity(id, 1, EntityKind::Rifleman, (14.0, 8.0 + i as f32)));
    }
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let mut memory = live_memory();
    let enemy = (70.5 * ts(), 70.5 * ts());
    let free: Vec<u32> = (20..26).collect();
    let sentries = post_sentries(
        &mut actions,
        &observation,
        &mut memory,
        None,
        Some(enemy),
        &free,
    );
    assert_eq!(sentries.len(), 2);
    let home = (10.5 * ts(), 10.5 * ts());
    for command in actions.into_commands() {
        let Command::Move { x, y, .. } = command else {
            continue;
        };
        let away = distance((x, y), home) / ts();
        assert!(
            (SENTRY_MIN_TILES..=SENTRY_TILES + 2.5).contains(&away),
            "{away}"
        );
    }
}

#[test]
fn a_raid_nobody_can_beat_is_met_on_the_line_and_the_guns_keep_their_posts() {
    let mut observation = base(1_200);
    // Two Machine Gunners on the line west of the Depot, two Riflemen east of it.
    observation.owned.extend([
        entity(30, 1, EntityKind::MachineGunner, (6.0, 9.0)),
        entity(31, 1, EntityKind::MachineGunner, (6.0, 12.0)),
        entity(20, 1, EntityKind::Rifleman, (15.0, 13.0)),
        entity(21, 1, EntityKind::Rifleman, (15.0, 14.0)),
    ]);
    // Ten raiders north-east of the Steel Mine.
    for (i, id) in (90..100).enumerate() {
        observation.visible_enemies.push(entity(
            id,
            2,
            EntityKind::Rifleman,
            (20.0 + (i % 5) as f32, 2.0 + (i / 5) as f32),
        ));
    }
    let mut memory = live_memory();
    let (answer, commands) = respond_commands(&observation, &mut memory, &[20, 21, 30, 31]);
    assert!(answer.is_some());
    let line = (6.0 * ts(), 10.5 * ts());
    let sent = commands
        .iter()
        .find_map(|command| match command {
            Command::Move { units, x, y, .. } if units.contains(&20) => Some((*x, *y)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the Riflemen fall back: {commands:?}"));
    assert!(distance(sent, line) <= 2.0 * ts(), "{:?}", sent);
    assert!(
        !commands.iter().any(|command| match command {
            Command::Move { units, .. }
            | Command::AttackMove { units, .. }
            | Command::HoldPosition { units, .. } => units.contains(&30) || units.contains(&31),
            _ => false,
        }),
        "{commands:?}"
    );
}

#[test]
fn riflemen_away_from_the_main_step_back_from_a_parked_tank_but_not_inside_it() {
    let mut observation = base(3_000);
    // Two Riflemen at a natural 30 tiles out, a parked enemy Tank 9 tiles from them.
    observation.owned.extend([
        entity(20, 1, EntityKind::Rifleman, (40.0, 10.0)),
        entity(21, 1, EntityKind::Rifleman, (40.0, 11.0)),
    ]);
    observation
        .visible_enemies
        .push(entity(90, 2, EntityKind::Tank, (49.0, 10.5)));
    let mut memory = live_memory();
    let (_, commands) = outranged_commands(&observation, &mut memory, &[20, 21]);
    let back = commands
        .iter()
        .find_map(|command| match command {
            Command::Move { units, x, y, .. } if units == &[20] => Some((*x, *y)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("steps back: {commands:?}"));
    assert!(distance(back, (49.0 * ts(), 10.5 * ts())) > 14.0 * ts());

    // The same Tank 9 tiles from Riflemen in the main is the home Tanks' fight.
    let mut observation = base(3_000);
    observation.owned.extend([
        entity(20, 1, EntityKind::Rifleman, (13.0, 10.0)),
        entity(21, 1, EntityKind::Rifleman, (13.0, 11.0)),
    ]);
    observation
        .visible_enemies
        .push(entity(90, 2, EntityKind::Tank, (22.0, 10.5)));
    let mut memory = live_memory();
    let (held, commands) = outranged_commands(&observation, &mut memory, &[20, 21]);
    assert!(held.is_empty() && commands.is_empty(), "{commands:?}");
}

fn with_enemy_start(mut observation: AiObservation, start_tile: (u32, u32)) -> AiObservation {
    use crate::ai_core::observation::AiPlayerSummary;
    observation.players = vec![
        AiPlayerSummary {
            id: 1,
            team_id: 1,
            start_tile: observation.own_start_tile,
            is_ai: true,
            is_alive: true,
        },
        AiPlayerSummary {
            id: 2,
            team_id: 2,
            start_tile,
            is_ai: false,
            is_alive: true,
        },
    ];
    observation
}

#[test]
fn machine_gunners_answer_a_small_flank_raid_when_nothing_else_is_home_but_not_a_frontal_one() {
    // The enemy starts due west; the line of guns faces it.
    let mut observation = with_enemy_start(base(1_500), (0, 10));
    observation.owned.extend([
        entity(30, 1, EntityKind::MachineGunner, (6.0, 9.0)),
        entity(31, 1, EntityKind::MachineGunner, (6.0, 12.0)),
        entity(32, 1, EntityKind::MachineGunner, (5.0, 10.5)),
    ]);
    // Two raiders north of the Steel Mine: a flank.
    observation.visible_enemies.extend([
        entity(90, 2, EntityKind::Rifleman, (14.5, 1.0)),
        entity(91, 2, EntityKind::Rifleman, (15.5, 1.0)),
    ]);
    let mut memory = live_memory();
    let (answer, commands) = respond_commands(&observation, &mut memory, &[30, 31, 32]);
    let units = answer.expect("the flank raid is answered");
    assert!(
        units.iter().any(|id| [30, 31, 32].contains(id)),
        "{units:?}"
    );
    assert!(commands
        .iter()
        .any(|command| matches!(command, Command::Move { .. })));

    // The same raid coming from the west, toward the guns, is left to them.
    let mut observation = with_enemy_start(base(1_500), (0, 10));
    observation.owned.extend([
        entity(30, 1, EntityKind::MachineGunner, (6.0, 9.0)),
        entity(31, 1, EntityKind::MachineGunner, (6.0, 12.0)),
        entity(32, 1, EntityKind::MachineGunner, (5.0, 10.5)),
    ]);
    observation.visible_enemies.extend([
        entity(90, 2, EntityKind::Rifleman, (0.5, 9.0)),
        entity(91, 2, EntityKind::Rifleman, (0.5, 12.0)),
    ]);
    let mut memory = live_memory();
    let (answer, commands) = respond_commands(&observation, &mut memory, &[30, 31, 32]);
    assert!(answer.is_none_or(|units| units.is_empty()), "{commands:?}");
    assert!(commands.is_empty(), "{commands:?}");
}

#[test]
fn no_sentry_stands_on_the_natural_its_guards_hold() {
    let mut observation = base(3_000);
    for (i, id) in (20..26).enumerate() {
        observation
            .owned
            .push(entity(id, 1, EntityKind::Rifleman, (14.0, 8.0 + i as f32)));
    }
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let mut memory = live_memory();
    // The natural sits where the south-eastern post would be.
    memory.expansion_security.site = Some((20, 1));
    let natural = building_center((20, 1), EntityKind::ResourceDepot, config::TILE_SIZE).unwrap();
    let enemy = (70.5 * ts(), 70.5 * ts());
    let free: Vec<u32> = (20..26).collect();
    let sentries = post_sentries(
        &mut actions,
        &observation,
        &mut memory,
        None,
        Some(enemy),
        &free,
    );
    assert_eq!(sentries.len(), 1);
    for command in actions.into_commands() {
        if let Command::Move { x, y, .. } = command {
            assert!(distance((x, y), natural) >= SENTRY_NATURAL_CLEAR_TILES * ts());
        }
    }
}

#[test]
fn a_member_turned_away_from_the_line_is_sent_back_by_the_live_jeff() {
    for resend in [true, false] {
        let mut observation = base(1_200);
        // Two Machine Gunners on the line west of the Depot, two Riflemen east of it.
        observation.owned.extend([
            entity(30, 1, EntityKind::MachineGunner, (6.0, 9.0)),
            entity(31, 1, EntityKind::MachineGunner, (6.0, 12.0)),
            entity(20, 1, EntityKind::Rifleman, (15.0, 13.0)),
            entity(21, 1, EntityKind::Rifleman, (15.0, 14.0)),
        ]);
        // Ten raiders north-east of the Steel Mine: the Riflemen fall back to the line.
        for (i, id) in (90..100).enumerate() {
            observation.visible_enemies.push(entity(
                id,
                2,
                EntityKind::Rifleman,
                (20.0 + (i % 5) as f32, 2.0 + (i / 5) as f32),
            ));
        }
        let defenders = [20, 21, 30, 31];
        let mut memory = live_memory();
        let sent = |commands: &[Command], id: u32| {
            commands.iter().any(
                |command| matches!(command, Command::Move { units, .. } if units.contains(&id)),
            )
        };
        let (_, commands) = respond_with(&observation, &mut memory, &defenders, resend);
        assert!(sent(&commands, 20) && sent(&commands, 21), "{commands:?}");
        // Another order walks Rifleman 20 two tiles further from the line.
        observation.tick += 9;
        for unit in observation.owned.iter_mut() {
            if unit.id == 20 || unit.id == 21 {
                unit.state = AiEntityState::Move;
            }
            if unit.id == 20 {
                unit.x += 2.0 * ts();
            }
        }
        let (_, commands) = respond_with(&observation, &mut memory, &defenders, resend);
        assert_eq!(sent(&commands, 20), resend, "{commands:?}");
        assert!(!sent(&commands, 21), "{commands:?}");
    }
}

#[test]
fn a_gathering_point_on_a_footprint_jeff_has_ordered_is_moved_off_it() {
    let mut observation = base(1_200);
    // The natural's Depot is ordered at tile (20, 20); nothing stands there yet.
    let foot = config::building_stats(EntityKind::ResourceDepot).unwrap();
    let (right, bottom) = (20.0 + foot.foot_w as f32, 20.0 + foot.foot_h as f32);
    observation.pending_builds.push(AiBuildIntent {
        worker_id: 1,
        kind: EntityKind::ResourceDepot,
        tile_x: 20,
        tile_y: 20,
        phase: AiBuildIntentPhase::ToSite,
    });
    let home = (10.5 * ts(), 10.5 * ts());
    let on = (21.5 * ts(), (bottom - 0.5) * ts());
    let moved = off_planned_footprints(&observation, on, home);
    let margin = rts_rules::balance::unit_placement_radius(EntityKind::Rifleman);
    let inside = |p: (f32, f32)| {
        p.0 > 20.0 * ts() - margin
            && p.0 < right * ts() + margin
            && p.1 > 20.0 * ts() - margin
            && p.1 < bottom * ts() + margin
    };
    assert!(inside(on) && !inside(moved), "{moved:?}");
    // It leaves on the side it stood, past the footprint's lower edge.
    assert!(moved.1 > bottom * ts(), "{moved:?}");
    // A point clear of every footprint stays where it is.
    let clear = (30.0 * ts(), 30.0 * ts());
    assert_eq!(off_planned_footprints(&observation, clear, home), clear);
}
