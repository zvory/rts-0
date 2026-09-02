use super::*;
use crate::ai_core::observation::AiEconomy;
use crate::ai_core::profiles::{JEFFS_AI, JEFFS_AI_BETA};

fn test_entity(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x,
        y,
        hp: 300,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

fn test_observation(owned: Vec<AiEntitySummary>, tick: u32) -> AiObservation {
    let map = AiMapSummary {
        width: 64,
        height: 64,
        tile_size: 32,
    };
    AiObservation {
        player_id: 1,
        tick,
        map,
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: owned.len() as u32,
            supply_cap: 100,
        },
        own_start_tile: (8, 8),
        players: Vec::new(),
        owned,
        resources: vec![AiResourceSummary {
            id: 900,
            kind: EntityKind::Steel,
            x: 48.5 * map.tile_size as f32,
            y: 40.5 * map.tile_size as f32,
            remaining: 1_000,
        }],
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn test_enemy_base(observation: &AiObservation) -> EnemyBaseFact {
    EnemyBaseFact {
        player_id: 2,
        start_tile: (56, 48),
        x: 56.5 * observation.map.tile_size as f32,
        y: 48.5 * observation.map.tile_size as f32,
    }
}

fn issue_test_containment(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) -> (Option<AiIntent>, Vec<Command>) {
    let facts = AiFacts::from_observation(observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let plan = FrontalWavePlan {
        ready_units: observation
            .owned
            .iter()
            .filter(|unit| matches!(unit.kind, EntityKind::Tank | EntityKind::ScoutCar))
            .map(|unit| unit.id)
            .collect(),
        desired_size: 3,
        attack_due: true,
        required_unit_ready: true,
        methamphetamines_ready: true,
        blockers: Vec::new(),
    };
    let intent = issue_expansion_containment_wave(
        &mut actions,
        observation,
        &plan,
        test_enemy_base(observation),
        JEFFS_AI.expansion_containment.unwrap(),
        true,
        None,
        memory,
    );
    (intent, actions.into_commands())
}

fn apply_command_destinations(
    observation: &mut AiObservation,
    commands: &[Command],
    should_apply: impl Fn(u32) -> bool,
) {
    for command in commands {
        let (units, point) = match command {
            Command::Move { units, x, y, .. } | Command::AttackMove { units, x, y, .. } => {
                (units, (*x, *y))
            }
            _ => continue,
        };
        for unit_id in units {
            if !should_apply(*unit_id) {
                continue;
            }
            if let Some(unit) = observation
                .owned
                .iter_mut()
                .find(|unit| unit.id == *unit_id)
            {
                unit.x = point.0;
                unit.y = point.1;
            }
        }
    }
}

fn test_force() -> Vec<AiEntitySummary> {
    vec![
        test_entity(1, EntityKind::Tank, 8.5 * 32.0, 8.5 * 32.0),
        test_entity(2, EntityKind::Tank, 9.5 * 32.0, 8.5 * 32.0),
        test_entity(3, EntityKind::ScoutCar, 8.5 * 32.0, 9.5 * 32.0),
        test_entity(4, EntityKind::Rifleman, 7.5 * 32.0, 8.5 * 32.0),
        test_entity(5, EntityKind::Rifleman, 7.5 * 32.0, 9.5 * 32.0),
        test_entity(6, EntityKind::Rifleman, 6.5 * 32.0, 8.5 * 32.0),
        test_entity(7, EntityKind::Rifleman, 6.5 * 32.0, 9.5 * 32.0),
        test_entity(8, EntityKind::Rifleman, 8.0 * 32.0, 8.0 * 32.0),
        test_entity(9, EntityKind::Rifleman, 9.0 * 32.0, 8.0 * 32.0),
    ]
}

#[test]
fn containment_waits_for_tanks_scout_and_rifle_screen_to_assemble() {
    let mut observation = test_observation(test_force(), 100);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let (intent, assembly_commands) = issue_test_containment(&observation, &mut memory);

    assert!(matches!(intent, Some(AiIntent::Stage { .. })));
    assert!(!memory.containment_wave_launched);
    assert_eq!(memory.containment_active_tanks.len(), 2);
    assert_eq!(memory.containment_active_riflemen.len(), 2);
    assert!(assembly_commands
        .iter()
        .any(|command| matches!(command, Command::Move { units, .. } if units == &[1])));
    assert!(assembly_commands
        .iter()
        .any(|command| matches!(command, Command::Move { units, .. } if units == &[3])));
    assert!(assembly_commands.iter().any(|command| {
        matches!(command, Command::AttackMove { units, .. } if memory.containment_active_riflemen.contains(&units[0]))
    }));

    apply_command_destinations(&mut observation, &assembly_commands, |_| true);
    observation.tick += CONTAINMENT_FORMATION_REISSUE_TICKS;
    let (intent, launch_commands) = issue_test_containment(&observation, &mut memory);

    assert!(matches!(intent, Some(AiIntent::Attack { .. })));
    assert!(memory.containment_wave_launched);
    let waypoint = stored_waypoint(&memory).expect("short first march waypoint");
    let tank_center = group_center(&observation, &[1, 2]).unwrap();
    let step_tiles = dist2(tank_center.0, tank_center.1, waypoint.0, waypoint.1).sqrt()
        / observation.map.tile_size as f32;
    assert!(step_tiles <= CONTAINMENT_MARCH_STEP_TILES + 0.05);
    assert!(launch_commands
        .iter()
        .any(|command| matches!(command, Command::AttackMove { units, .. } if units == &[1])));
}

#[test]
fn containment_launches_after_assembly_timeout_when_core_is_grouped() {
    let mut observation = test_observation(test_force(), 100);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let (intent, _) = issue_test_containment(&observation, &mut memory);
    assert!(matches!(intent, Some(AiIntent::Stage { .. })));

    observation.tick += CONTAINMENT_ASSEMBLY_TIMEOUT_TICKS;
    let (intent, commands) = issue_test_containment(&observation, &mut memory);

    assert!(matches!(intent, Some(AiIntent::Attack { .. })));
    assert!(memory.containment_wave_launched);
    assert!(commands.iter().any(|command| {
        matches!(command, Command::AttackMove { units, .. } if memory.containment_active_tanks.contains(&units[0]))
    }));
}

#[test]
fn containment_timeout_does_not_launch_split_tanks() {
    let mut observation = test_observation(test_force(), 100);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let _ = issue_test_containment(&observation, &mut memory);
    let split_tank = *memory.containment_active_tanks.iter().next().unwrap();
    let tank = observation
        .owned
        .iter_mut()
        .find(|unit| unit.id == split_tank)
        .unwrap();
    tank.x += 10.0 * observation.map.tile_size as f32;
    observation.tick += CONTAINMENT_ASSEMBLY_TIMEOUT_TICKS;

    let (intent, _) = issue_test_containment(&observation, &mut memory);

    assert!(matches!(intent, Some(AiIntent::Stage { .. })));
    assert!(!memory.containment_wave_launched);
}

#[test]
fn containment_hard_timeout_departs_with_vehicle_core_when_no_screen_is_available() {
    let force = test_force()
        .into_iter()
        .filter(|unit| unit.kind != EntityKind::Rifleman)
        .collect();
    let mut observation = test_observation(force, 100);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let _ = issue_test_containment(&observation, &mut memory);
    observation.tick += CONTAINMENT_ASSEMBLY_HARD_TIMEOUT_TICKS;
    let (intent, _) = issue_test_containment(&observation, &mut memory);

    assert!(matches!(intent, Some(AiIntent::Attack { .. })));
    assert!(memory.containment_wave_launched);
}

#[test]
fn river_opening_guard_waits_then_requires_a_clear_pressure_window() {
    let mut observation = test_observation(test_force(), 100);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let started = observation.tick;

    assert!(river_opening_guard_active(
        &observation,
        &[1, 2],
        started,
        &mut memory,
    ));

    observation.tick = started + RIVER_OPENING_GUARD_TICKS;
    let mut enemy = test_entity(99, EntityKind::Tank, 12.0 * 32.0, 12.0 * 32.0);
    enemy.owner = 2;
    observation.visible_enemies.push(enemy);
    assert!(river_opening_guard_active(
        &observation,
        &[1, 2],
        started,
        &mut memory,
    ));

    observation.visible_enemies.clear();
    observation.tick += RIVER_OPENING_CLEAR_TICKS;
    assert!(river_opening_guard_active(
        &observation,
        &[1, 2],
        started,
        &mut memory,
    ));

    observation.tick += 1;
    assert!(!river_opening_guard_active(
        &observation,
        &[1, 2],
        started,
        &mut memory,
    ));
}

#[test]
fn escort_selection_ignores_distant_and_engaged_riflemen() {
    let mut force = test_force();
    let mut engaged = test_entity(10, EntityKind::Rifleman, 8.0 * 32.0, 8.0 * 32.0);
    engaged.free_for_combat = false;
    force.push(engaged);
    force.push(test_entity(
        11,
        EntityKind::Rifleman,
        30.0 * 32.0,
        8.0 * 32.0,
    ));
    force.push(test_entity(
        12,
        EntityKind::Rifleman,
        8.0 * 32.0,
        8.0 * 32.0,
    ));
    force.push(test_entity(
        13,
        EntityKind::Rifleman,
        9.0 * 32.0,
        8.0 * 32.0,
    ));
    let observation = test_observation(force, 100);
    let memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let selected = select_rifle_escorts(&observation, &memory, (8.5 * 32.0, 8.5 * 32.0));

    assert!(selected.iter().all(|id| [8, 9, 12, 13].contains(id)));
    assert!(!selected.contains(&10));
    assert!(!selected.contains(&11));
}

#[test]
fn emergency_recall_redirects_the_whole_active_group() {
    let mut observation = test_observation(test_force(), 100);
    let mut enemy = test_entity(99, EntityKind::Tank, 9.0 * 32.0, 9.0 * 32.0);
    enemy.owner = 2;
    observation.visible_enemies.push(enemy);
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment_active_tanks.extend([1, 2]);
    memory.containment_active_scout = Some(3);
    memory.containment_active_riflemen.extend([4, 5]);

    let intent = issue_containment_recall(&mut actions, &observation, &mut memory, 99);
    let commands = actions.into_commands();

    assert!(matches!(intent, Some(AiIntent::Attack { .. })));
    assert!(memory.containment_recall_active);
    assert!(matches!(
        commands.as_slice(),
        [Command::Attack { units, target: 99, .. }] if units == &[1, 2, 3, 4, 5]
    ));
}

#[test]
fn containment_timeout_allows_minor_screen_pathing_slop() {
    let mut force = vec![
        test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0),
        test_entity(2, EntityKind::Tank, 10.0 * 32.0, 11.0 * 32.0),
        test_entity(3, EntityKind::ScoutCar, 12.0 * 32.0, 10.5 * 32.0),
    ];
    for index in 0..6 {
        let distance_tiles = if index < 2 {
            3.0
        } else if index == 2 {
            6.5
        } else {
            10.0
        };
        force.push(test_entity(
            4 + index,
            EntityKind::Rifleman,
            (10.0 + distance_tiles) * 32.0,
            10.5 * 32.0,
        ));
    }
    let observation = test_observation(force, 100);
    let formation = ContainmentFormation {
        tanks: vec![(1, (0.0, 0.0)), (2, (0.0, 0.0))],
        scout: (3, (0.0, 0.0)),
        riflemen: (4..10).map(|id| (id, (0.0, 0.0))).collect(),
    };

    assert!(formation_core_is_grouped(
        &observation,
        &formation,
        (8.5 * 32.0, 8.5 * 32.0),
        (56.5 * 32.0, 48.5 * 32.0),
    ));
}

#[test]
fn beta_snapshot_preserves_the_immediate_containment_launch() {
    let observation = test_observation(test_force(), 100);
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI_BETA);
    let plan = FrontalWavePlan {
        ready_units: vec![1, 2, 3],
        desired_size: 3,
        attack_due: true,
        required_unit_ready: true,
        methamphetamines_ready: true,
        blockers: Vec::new(),
    };

    let intent = issue_frontal_wave(
        &mut actions,
        &observation,
        &JEFFS_AI_BETA,
        JEFFS_AI_BETA.tech_transition.unwrap().attack,
        &plan,
        test_enemy_base(&observation),
        None,
        None,
        &mut memory,
    );

    assert!(matches!(intent, Some(AiIntent::Attack { .. })));
    assert!(memory.containment_wave_launched);
}

#[test]
fn containment_does_not_advance_past_a_slow_rifle_screen() {
    let mut observation = test_observation(test_force(), 100);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let (_, assembly_commands) = issue_test_containment(&observation, &mut memory);
    apply_command_destinations(&mut observation, &assembly_commands, |_| true);
    observation.tick += CONTAINMENT_FORMATION_REISSUE_TICKS;
    let (_, first_step_commands) = issue_test_containment(&observation, &mut memory);
    let first_waypoint = stored_waypoint(&memory).unwrap();
    let escorts = memory.containment_active_riflemen.clone();

    apply_command_destinations(&mut observation, &first_step_commands, |unit_id| {
        !escorts.contains(&unit_id)
    });
    observation.tick += CONTAINMENT_FORMATION_REISSUE_TICKS;
    let (_, catch_up_commands) = issue_test_containment(&observation, &mut memory);

    assert_eq!(stored_waypoint(&memory), Some(first_waypoint));
    assert!(catch_up_commands.iter().all(|command| {
        !matches!(command, Command::AttackMove { units, .. } if units.iter().any(|unit| memory.containment_active_tanks.contains(unit)))
    }));
    assert!(catch_up_commands.iter().any(|command| {
        matches!(command, Command::AttackMove { units, .. } if escorts.contains(&units[0]))
    }));

    apply_command_destinations(&mut observation, &catch_up_commands, |_| true);
    observation.tick += CONTAINMENT_FORMATION_REISSUE_TICKS;
    let _ = issue_test_containment(&observation, &mut memory);
    let second_waypoint = stored_waypoint(&memory).expect("next cohesive march waypoint");
    assert_ne!(second_waypoint, first_waypoint);
    let step_tiles = dist2(
        first_waypoint.0,
        first_waypoint.1,
        second_waypoint.0,
        second_waypoint.1,
    )
    .sqrt()
        / observation.map.tile_size as f32;
    assert!(step_tiles <= CONTAINMENT_MARCH_STEP_TILES + 0.05);
}

#[test]
fn containment_advances_when_one_screen_rifleman_is_late() {
    let mut observation = test_observation(test_force(), 100);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let (_, assembly_commands) = issue_test_containment(&observation, &mut memory);
    apply_command_destinations(&mut observation, &assembly_commands, |_| true);
    observation.tick += CONTAINMENT_FORMATION_REISSUE_TICKS;
    let (_, first_step_commands) = issue_test_containment(&observation, &mut memory);
    let first_waypoint = stored_waypoint(&memory).unwrap();
    let laggard = *memory.containment_active_riflemen.iter().next().unwrap();

    apply_command_destinations(&mut observation, &first_step_commands, |unit_id| {
        unit_id != laggard
    });
    observation.tick += CONTAINMENT_FORMATION_REISSUE_TICKS;
    let _ = issue_test_containment(&observation, &mut memory);

    assert_ne!(stored_waypoint(&memory), Some(first_waypoint));
}

#[test]
fn containment_detects_a_tank_that_has_run_ahead() {
    let observation = test_observation(
        vec![
            test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0),
            test_entity(2, EntityKind::Tank, 18.0 * 32.0, 10.0 * 32.0),
        ],
        100,
    );

    assert!(!tank_group_is_cohesive(&observation, &[1, 2], (1.0, 0.0)));
}
