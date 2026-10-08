use super::*;
use crate::ai_core::observation::{AiAbilitySummary, AiEconomy};
use crate::ai_core::profiles::JEFFS_AI;

fn target_test_entity(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: if id == 1 { 1 } else { 2 },
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

fn regroup_test_observation(owned: Vec<AiEntitySummary>) -> AiObservation {
    AiObservation {
        player_id: 1,
        tick: 0,
        map: AiMapSummary {
            width: 64,
            height: 64,
            tile_size: 32,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: owned.len() as u32,
            supply_cap: 100,
        },
        own_start_tile: (10, 10),
        players: Vec::new(),
        owned,
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

#[test]
fn tank_push_selects_half_of_available_riflemen() {
    let riflemen = (1..=5)
        .map(|id| {
            let mut unit = target_test_entity(id, EntityKind::Rifleman, id as f32 * 32.0, 320.0);
            unit.owner = 1;
            unit
        })
        .collect::<Vec<_>>();
    let observation = regroup_test_observation(riflemen);
    let memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let selected = select_rifle_escorts(&observation, &memory, (0.0, 320.0));

    assert_eq!(selected, vec![5]);
}

#[test]
fn tank_push_caps_the_rifle_screen_at_six() {
    let riflemen = (1..=20)
        .map(|id| {
            let mut unit = target_test_entity(id, EntityKind::Rifleman, id as f32 * 32.0, 320.0);
            unit.owner = 1;
            unit
        })
        .collect::<Vec<_>>();
    let observation = regroup_test_observation(riflemen);
    let memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let selected = select_rifle_escorts(&observation, &memory, (10.0 * 32.0, 320.0));

    assert_eq!(selected.len(), 6);
    assert!(selected.iter().all(|id| *id > 4));
}

#[test]
fn rifle_screen_stays_two_tiles_ahead_of_the_tank_front() {
    let map = AiMapSummary {
        width: 100,
        height: 100,
        tile_size: 32,
    };

    let point =
        rifle_screen_point((320.0, 640.0), (960.0, 640.0), map).expect("forward screen point");

    assert_eq!(point, (384.0, 640.0));
}

#[test]
fn rifle_screen_spreads_escorts_across_the_tank_front() {
    let map = AiMapSummary {
        width: 100,
        height: 100,
        tile_size: 32,
    };

    let points = rifle_screen_points((320.0, 640.0), (960.0, 640.0), map, 3);

    assert_eq!(points, vec![(384.0, 576.0), (384.0, 640.0), (384.0, 704.0)]);
}

#[test]
fn large_rifle_screen_uses_a_staggered_second_rank() {
    let map = AiMapSummary {
        width: 100,
        height: 100,
        tile_size: 32,
    };

    let points = rifle_screen_points((320.0, 640.0), (960.0, 640.0), map, 6);

    assert_eq!(
        points,
        vec![
            (384.0, 544.0),
            (384.0, 608.0),
            (384.0, 672.0),
            (384.0, 736.0),
            (336.0, 608.0),
            (336.0, 672.0),
        ]
    );
}

#[test]
fn containment_uses_stationary_tank_range_and_forward_scout_vision() {
    let map = AiMapSummary {
        width: 100,
        height: 100,
        tile_size: 32,
    };
    let policy = ExpansionContainmentPolicy {
        tank_standoff_tiles: 13.5,
        scout_trailing_tiles: 1.5,
        scout_forward_tiles: 2.0,
        flank_tiles: 5.0,
        contact_stop_tiles: 18.0,
        minimum_tanks_to_continue: 2,
        recovery_tanks_to_continue: 3,
        additional_tanks_per_repush: 1,
        repush_regroup_radius_tiles: 5.0,
    };
    let objective = (2_000.0, 1_000.0);
    let (tank, scout) = containment_points((200.0, 1_000.0), objective, map, policy).unwrap();

    let tank_distance = dist2(objective.0, objective.1, tank.0, tank.1).sqrt() / 32.0;
    let scout_distance = dist2(scout.0, scout.1, tank.0, tank.1).sqrt() / 32.0;
    assert!((tank_distance - 13.5).abs() < 0.001);
    assert!((scout_distance - 2.0).abs() < 0.001);
    assert!(scout.0 < objective.0);

    let trailing =
        scout_trailing_point((1_000.0, 1_000.0), (200.0, 1_000.0), objective, map, 1.5).unwrap();
    assert_eq!((1_000.0 - trailing.0) / 32.0, 1.5);
}

#[test]
fn containment_flank_rotates_with_the_players() {
    let map = AiMapSummary {
        width: 100,
        height: 100,
        tile_size: 32,
    };
    let policy = JEFFS_AI.expansion_containment.unwrap();
    let world_size = map.width as f32 * map.tile_size as f32;
    let own_base = (200.0, 1_000.0);
    let objective = (2_000.0, 1_000.0);
    let original = containment_points(own_base, objective, map, policy).unwrap();
    let rotated = containment_points(
        (world_size - own_base.0, world_size - own_base.1),
        (world_size - objective.0, world_size - objective.1),
        map,
        policy,
    )
    .unwrap();

    for (actual, expected) in [
        (rotated.0 .0, world_size - original.0 .0),
        (rotated.0 .1, world_size - original.0 .1),
        (rotated.1 .0, world_size - original.1 .0),
        (rotated.1 .1, world_size - original.1 .1),
    ] {
        assert!((actual - expected).abs() < 0.001);
    }
}

#[test]
fn each_repush_adds_one_tank_to_the_grouped_cohort() {
    let policy = JEFFS_AI.expansion_containment.unwrap();
    assert_eq!(containment_repush_tank_count(policy, 1), 3);
    assert_eq!(containment_repush_tank_count(policy, 2), 4);
    assert_eq!(containment_repush_tank_count(policy, 3), 5);
    assert_eq!(containment_regroup_radius_tiles(policy, 3), 3.0);
    assert_eq!(containment_regroup_radius_tiles(policy, 4), 4.5);
    assert_eq!(containment_regroup_radius_tiles(policy, 5), 6.0);
}

#[test]
fn repush_selects_units_nearest_the_forward_rally_point() {
    let observation = regroup_test_observation(vec![
        target_test_entity(1, EntityKind::Tank, 100.0, 100.0),
        target_test_entity(2, EntityKind::Tank, 500.0, 500.0),
        target_test_entity(3, EntityKind::Tank, 515.0, 500.0),
        target_test_entity(4, EntityKind::Tank, 530.0, 500.0),
    ]);
    let mut candidates = vec![1, 2, 3, 4];

    select_nearest_units(&observation, &mut candidates, (520.0, 500.0), 3);

    assert_eq!(candidates, vec![3, 4, 2]);
}

#[test]
fn repush_requires_a_compact_group_near_its_rally_point() {
    let compact = regroup_test_observation(vec![
        target_test_entity(1, EntityKind::Tank, 490.0, 500.0),
        target_test_entity(2, EntityKind::Tank, 510.0, 500.0),
        target_test_entity(3, EntityKind::Tank, 500.0, 510.0),
        target_test_entity(4, EntityKind::ScoutCar, 500.0, 490.0),
    ]);
    let scattered = regroup_test_observation(vec![
        target_test_entity(1, EntityKind::Tank, 300.0, 500.0),
        target_test_entity(2, EntityKind::Tank, 700.0, 500.0),
        target_test_entity(3, EntityKind::Tank, 500.0, 300.0),
        target_test_entity(4, EntityKind::ScoutCar, 500.0, 700.0),
    ]);
    let cohort = [1, 2, 3, 4];

    assert!(compact_group_near(
        &compact,
        &cohort,
        (500.0, 500.0),
        5.0 * 32.0
    ));
    assert!(!compact_group_near(
        &scattered,
        &cohort,
        (500.0, 500.0),
        5.0 * 32.0
    ));
}

#[test]
fn anti_armor_threats_outrank_every_economic_target() {
    assert_eq!(outbound_wave_target_priority(EntityKind::Tank), 0);
    assert_eq!(outbound_wave_target_priority(EntityKind::AntiTankGun), 0);
    assert_eq!(outbound_wave_target_priority(EntityKind::Panzerfaust), 0);
    assert!(
        outbound_wave_target_priority(EntityKind::MachineGunner)
            > outbound_wave_target_priority(EntityKind::Tank)
    );
    assert!(
        outbound_wave_target_priority(EntityKind::Worker)
            > outbound_wave_target_priority(EntityKind::Panzerfaust)
    );
}

#[test]
fn main_resource_depot_is_acquired_outside_nominal_standoff_radius() {
    let tile_size = 32;
    let tank = target_test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0);
    let resource_depot = target_test_entity(2, EntityKind::ResourceDepot, 25.0 * 32.0, 10.0 * 32.0);
    let observation = AiObservation {
        player_id: 1,
        tick: 0,
        map: AiMapSummary {
            width: 64,
            height: 64,
            tile_size,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 1,
            supply_cap: 100,
        },
        own_start_tile: (10, 10),
        players: Vec::new(),
        owned: vec![tank],
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: vec![resource_depot],
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    };

    assert_eq!(
        visible_strategic_building_target_within_tiles(&observation, &[1], 13.5),
        None
    );
    assert_eq!(
        visible_strategic_building_target_within_tiles(&observation, &[1], 18.0),
        Some(2)
    );
}

#[test]
fn stationary_target_requires_every_tank_to_be_in_range() {
    let mut first = target_test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0);
    first.owner = 1;
    let mut second = target_test_entity(3, EntityKind::Tank, 3.0 * 32.0, 10.0 * 32.0);
    second.owner = 1;
    let enemy = target_test_entity(100, EntityKind::Tank, 23.0 * 32.0, 10.0 * 32.0);
    let mut observation = regroup_test_observation(vec![first, second]);
    observation.visible_enemies.push(enemy);

    assert_eq!(
        shared_stationary_tank_target(&observation, &[1, 3], 13.5, None, None),
        None
    );
    assert_eq!(
        shared_stationary_tank_target(&observation, &[1], 13.5, None, None),
        Some(100)
    );
}

fn smoke_test_observation(focus: (u32, f32, f32), other: (u32, f32, f32)) -> AiObservation {
    let mut tank_one = target_test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0);
    tank_one.owner = 1;
    tank_one.target_id = Some(focus.0);
    let mut tank_two = target_test_entity(3, EntityKind::Tank, 10.0 * 32.0, 11.0 * 32.0);
    tank_two.owner = 1;
    tank_two.target_id = Some(focus.0);
    let mut scout = target_test_entity(4, EntityKind::ScoutCar, 10.0 * 32.0, 10.0 * 32.0);
    scout.owner = 1;
    let mut engineering =
        target_test_entity(5, EntityKind::EngineeringComplex, 8.0 * 32.0, 8.0 * 32.0);
    engineering.owner = 1;
    let mut observation = regroup_test_observation(vec![tank_one, tank_two, scout, engineering]);
    observation.tick = 100;
    let mut focus_tank = target_test_entity(focus.0, EntityKind::Tank, focus.1, focus.2);
    focus_tank.hp = 220;
    let mut other_tank = target_test_entity(other.0, EntityKind::Tank, other.1, other.2);
    other_tank.hp = 292;
    observation.visible_enemies = vec![focus_tank, other_tank];
    observation.ability_states.push(AiAbilitySummary {
        entity_id: 4,
        kind: AbilityKind::Smoke,
        cooldown_left: 0,
        remaining_uses: Some(2),
        available_tick: Some(0),
        lockout_until_tick: None,
        charge_recharge_left: None,
    });
    observation
}

#[test]
fn smoke_is_applied_to_healthy_rear_tank_and_focus_is_preserved() {
    let observation = smoke_test_observation(
        (100, 20.0 * 32.0, 10.0 * 32.0),
        (101, 22.0 * 32.0, 15.0 * 32.0),
    );
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.focus_target = Some(100);
    memory.containment.focus_stable_since = Some(90);
    let mut focus = 100;

    let _ = maybe_issue_isolation_smoke(
        &mut actions,
        &observation,
        &[1, 3],
        4,
        &mut focus,
        &mut memory,
        true,
    );
    issue_hp_aware_tank_volley(
        &mut actions,
        &observation,
        &[1, 3],
        &[],
        100,
        13.5,
        memory.containment.smoke_target,
    );
    let commands = actions.into_commands();

    assert_eq!(memory.containment.smoke_target, Some(101));
    assert!(matches!(
        commands.first(),
        Some(Command::UseAbility { units, ability: AbilityKind::Smoke, x: Some(_), y: Some(_), .. }) if units == &[4]
    ));
    assert!(commands.iter().any(|command| {
        matches!(command, Command::Attack { units, target: 100, .. } if units == &[1, 3])
    }));
    assert!(!commands
        .iter()
        .any(|command| { matches!(command, Command::Attack { target: 101, .. }) }));
}

#[test]
fn rear_focus_switches_the_smoke_candidate_to_the_forward_tank() {
    let observation = smoke_test_observation(
        (101, 22.0 * 32.0, 15.0 * 32.0),
        (100, 20.0 * 32.0, 10.0 * 32.0),
    );
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.focus_target = Some(101);
    memory.containment.focus_stable_since = Some(90);
    let mut focus = 101;

    let _ = maybe_issue_isolation_smoke(
        &mut actions,
        &observation,
        &[1, 3],
        4,
        &mut focus,
        &mut memory,
        true,
    );

    assert_eq!(memory.containment.smoke_target, Some(100));
}

#[test]
fn stale_split_tank_orders_do_not_suppress_a_coordinated_smoke_volley() {
    let mut observation = smoke_test_observation(
        (100, 20.0 * 32.0, 10.0 * 32.0),
        (101, 22.0 * 32.0, 15.0 * 32.0),
    );
    observation
        .owned
        .iter_mut()
        .find(|unit| unit.id == 3)
        .unwrap()
        .target_id = Some(101);
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.focus_target = Some(100);
    memory.containment.focus_stable_since = Some(80);
    let mut focus = 100;

    let _ = maybe_issue_isolation_smoke(
        &mut actions,
        &observation,
        &[1, 3],
        4,
        &mut focus,
        &mut memory,
        true,
    );

    assert_eq!(memory.containment.smoke_target, Some(101));
    assert!(matches!(
        actions.into_commands().first(),
        Some(Command::UseAbility {
            ability: AbilityKind::Smoke,
            ..
        })
    ));
}

#[test]
fn local_defense_smoke_does_not_wait_for_frontal_focus_stability() {
    let observation = smoke_test_observation(
        (100, 20.0 * 32.0, 10.0 * 32.0),
        (101, 22.0 * 32.0, 15.0 * 32.0),
    );
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let directive = maybe_issue_local_defense_smoke(
        &mut actions,
        &observation,
        &[1, 3],
        &[1, 3, 4],
        &[100, 101],
        &mut memory,
    );

    assert_eq!(
        directive,
        Some(LocalDefenseSmokeDirective::Obscure {
            target: 101,
            scout: 4,
        })
    );
    assert!(matches!(
        actions.into_commands().first(),
        Some(Command::UseAbility {
            ability: AbilityKind::Smoke,
            ..
        })
    ));
}

#[test]
fn large_local_tank_response_keeps_ordinary_defense_targeting() {
    let mut observation = smoke_test_observation(
        (100, 20.0 * 32.0, 10.0 * 32.0),
        (101, 22.0 * 32.0, 15.0 * 32.0),
    );
    let mut third_tank = target_test_entity(6, EntityKind::Tank, 11.0 * 32.0, 10.0 * 32.0);
    third_tank.owner = 1;
    observation.owned.push(third_tank);
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let directive = maybe_issue_local_defense_smoke(
        &mut actions,
        &observation,
        &[1, 3, 6],
        &[1, 3, 4, 6],
        &[100, 101],
        &mut memory,
    );

    assert_eq!(directive, None);
    assert!(actions.into_commands().is_empty());
}

#[test]
fn lone_local_tank_is_smoked_only_when_an_exposed_target_can_be_engaged() {
    let mut observation = smoke_test_observation(
        (100, 20.0 * 32.0, 10.0 * 32.0),
        (101, 50.0 * 32.0, 50.0 * 32.0),
    );
    observation.visible_enemies.push(target_test_entity(
        102,
        EntityKind::Panzerfaust,
        18.0 * 32.0,
        15.0 * 32.0,
    ));
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.focus_target = Some(100);
    memory.containment.focus_stable_since = Some(80);
    let mut focus = 100;

    let _ = maybe_issue_isolation_smoke(
        &mut actions,
        &observation,
        &[1, 3],
        4,
        &mut focus,
        &mut memory,
        true,
    );

    assert_eq!(focus, 102);
    assert_eq!(memory.containment.smoke_target, Some(100));
    assert_eq!(memory.containment.smoke_focus_target, Some(102));
    assert!(matches!(
        actions.into_commands().first(),
        Some(Command::UseAbility {
            ability: AbilityKind::Smoke,
            ..
        })
    ));
}

#[test]
fn lone_local_tank_is_suppressed_while_grouped_tanks_hold_fire() {
    let observation = smoke_test_observation(
        (100, 20.0 * 32.0, 10.0 * 32.0),
        (101, 50.0 * 32.0, 50.0 * 32.0),
    );
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.focus_target = Some(100);
    memory.containment.focus_stable_since = Some(80);
    let mut focus = 100;

    let _ = maybe_issue_isolation_smoke(
        &mut actions,
        &observation,
        &[1, 3],
        4,
        &mut focus,
        &mut memory,
        true,
    );
    issue_hp_aware_tank_volley(
        &mut actions,
        &observation,
        &[1, 3],
        &[],
        focus,
        13.5,
        memory.containment.smoke_target,
    );
    let commands = actions.into_commands();

    assert_eq!(memory.containment.smoke_target, Some(100));
    assert_eq!(memory.containment.smoke_focus_target, None);
    assert!(matches!(
        commands.first(),
        Some(Command::UseAbility {
            ability: AbilityKind::Smoke,
            ..
        })
    ));
    assert!(commands.iter().any(|command| {
        matches!(command, Command::HoldPosition { units, .. } if units == &[1, 3])
    }));
    assert!(!commands
        .iter()
        .any(|command| matches!(command, Command::Attack { target: 100, .. })));
}

#[test]
fn distant_second_tank_requests_a_bounded_scout_launch_position() {
    let observation = smoke_test_observation(
        (100, 20.0 * 32.0, 10.0 * 32.0),
        (101, 25.0 * 32.0, 14.0 * 32.0),
    );
    assert!(!target_is_in_shared_tank_range(
        &observation,
        &[1, 3],
        101,
        15.5,
    ));
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.focus_target = Some(100);
    memory.containment.focus_stable_since = Some(90);
    let mut focus = 100;

    let launch = maybe_issue_isolation_smoke(
        &mut actions,
        &observation,
        &[1, 3],
        4,
        &mut focus,
        &mut memory,
        true,
    )
    .expect("bounded launch point");
    let tank_center = group_center(&observation, &[1, 3]).unwrap();
    let forward = normalized_direction(tank_center, (20.0 * 32.0, 10.0 * 32.0)).unwrap();
    let lateral_axis = (-forward.1, forward.0);
    let relative = (launch.0 - tank_center.0, launch.1 - tank_center.1);
    let forward_tiles =
        (relative.0 * forward.0 + relative.1 * forward.1) / observation.map.tile_size as f32;
    let lateral_tiles = (relative.0 * lateral_axis.0 + relative.1 * lateral_axis.1).abs()
        / observation.map.tile_size as f32;

    assert!(forward_tiles <= CONTAINMENT_SCOUT_SMOKE_FORWARD_LIMIT_TILES + 0.001);
    assert!(lateral_tiles <= CONTAINMENT_SCOUT_SMOKE_LATERAL_LIMIT_TILES + 0.001);
    assert!(actions.into_commands().is_empty());
}

#[test]
fn rifle_sector_prioritizes_panzerfausts_and_never_targets_tanks() {
    let mut rifle = target_test_entity(1, EntityKind::Rifleman, 12.0 * 32.0, 10.0 * 32.0);
    rifle.owner = 1;
    let mut observation = regroup_test_observation(vec![rifle]);
    observation.visible_enemies = vec![
        target_test_entity(100, EntityKind::Tank, 14.0 * 32.0, 10.0 * 32.0),
        target_test_entity(101, EntityKind::Rifleman, 14.0 * 32.0, 10.5 * 32.0),
        target_test_entity(102, EntityKind::Panzerfaust, 14.5 * 32.0, 9.5 * 32.0),
    ];

    assert_eq!(
        rifle_sector_target(
            &observation,
            1,
            (12.0 * 32.0, 10.0 * 32.0),
            (10.0 * 32.0, 10.0 * 32.0),
            (30.0 * 32.0, 10.0 * 32.0),
        ),
        Some(102)
    );
}

#[test]
fn endgame_search_visits_inner_and_outer_base_rings() {
    let map = AiMapSummary {
        width: 100,
        height: 100,
        tile_size: 32,
    };
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (50, 50),
        x: 50.5 * 32.0,
        y: 50.5 * 32.0,
    };
    let own_base = (9.5 * 32.0, 9.5 * 32.0);
    assert_eq!(
        endgame_search_point(own_base, enemy_base, map, 0),
        (1616.0, 1616.0)
    );
    assert_eq!(
        endgame_search_point(own_base, enemy_base, map, 1),
        (1872.0, 1616.0)
    );
    assert_eq!(
        endgame_search_point(own_base, enemy_base, map, 9),
        (2128.0, 1616.0)
    );
    assert_eq!(
        endgame_search_point(own_base, enemy_base, map, ENDGAME_SEARCH_OFFSETS.len()),
        endgame_search_point(own_base, enemy_base, map, 0)
    );
}

#[test]
fn endgame_search_ring_rotates_with_the_players() {
    let map = AiMapSummary {
        width: 100,
        height: 100,
        tile_size: 32,
    };
    let world_size = map.width as f32 * map.tile_size as f32;
    let own_base = (9.5 * 32.0, 9.5 * 32.0);
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (80, 80),
        x: 80.5 * 32.0,
        y: 80.5 * 32.0,
    };
    let rotated_enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (19, 19),
        x: world_size - enemy_base.x,
        y: world_size - enemy_base.y,
    };
    for waypoint in 0..ENDGAME_SEARCH_OFFSETS.len() {
        let original = endgame_search_point(own_base, enemy_base, map, waypoint);
        let rotated = endgame_search_point(
            (world_size - own_base.0, world_size - own_base.1),
            rotated_enemy_base,
            map,
            waypoint,
        );
        assert_eq!(rotated, (world_size - original.0, world_size - original.1));
    }
}

fn held_tank_observation(tank_state: AiEntityState) -> AiObservation {
    let mut tank = target_test_entity(1, EntityKind::Tank, 20.0 * 32.0, 20.0 * 32.0);
    tank.state = tank_state;
    tank.target_id = Some(100);
    let mut other = target_test_entity(3, EntityKind::Tank, 22.0 * 32.0, 20.0 * 32.0);
    other.owner = 1;
    regroup_test_observation(vec![tank, other])
}

fn held_units(commands: &[Command]) -> Vec<u32> {
    commands
        .iter()
        .filter_map(|command| match command {
            Command::HoldPosition { units, .. } => Some(units.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn a_tank_the_push_already_holds_is_not_held_again() {
    let observation = held_tank_observation(AiEntityState::Idle);
    let facts = AiFacts::from_observation(&observation);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.held_tanks.insert(1);

    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    hold_containment_tanks(&mut actions, &observation, &memory, [1, 3]);
    // Tank 1 keeps the target it picked while holding; tank 3 was never held by the push.
    assert_eq!(held_units(&actions.into_commands()), vec![3]);
}

#[test]
fn a_held_tank_seen_moving_or_attacking_is_held_again() {
    for state in [AiEntityState::Move, AiEntityState::Attack] {
        let observation = held_tank_observation(state);
        let facts = AiFacts::from_observation(&observation);
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        memory.containment.held_tanks.insert(1);

        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        hold_containment_tanks(&mut actions, &observation, &memory, [1]);
        assert_eq!(held_units(&actions.into_commands()), vec![1], "{state:?}");
    }
}

#[test]
fn any_order_other_than_hold_releases_a_held_tank() {
    let observation = held_tank_observation(AiEntityState::Idle);
    let facts = AiFacts::from_observation(&observation);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));

    let start = actions.emitted_len();
    actions::hold_position_units(&mut actions, [1, 3]);
    actions::attack_units(&mut actions, [3], 100);
    note_containment_holds(&actions, &mut memory, start);
    assert_eq!(memory.containment.held_tanks, BTreeSet::from([1]));
}

#[test]
fn volley_leaves_already_holding_tanks_on_their_own_targets() {
    // Both tanks are out of reach of every target, so neither is assigned a volley shot.
    let observation = held_tank_observation(AiEntityState::Idle);
    let facts = AiFacts::from_observation(&observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    issue_hp_aware_tank_volley(&mut actions, &observation, &[1, 3], &[1], 100, 13.5, None);
    assert_eq!(held_units(&actions.into_commands()), vec![3]);
}

#[test]
fn a_push_is_outnumbered_only_by_more_enemy_tanks_close_by() {
    let ts = 32.0;
    let push = |enemies: &[(f32, f32)]| {
        let mut owned = vec![
            target_test_entity(1, EntityKind::Tank, 20.0 * ts, 20.0 * ts),
            target_test_entity(1, EntityKind::Tank, 21.0 * ts, 20.0 * ts),
        ];
        owned[1].id = 2;
        let mut observation = regroup_test_observation(owned);
        observation.visible_enemies = enemies
            .iter()
            .enumerate()
            .map(|(index, (x, y))| {
                target_test_entity(500 + index as u32, EntityKind::Tank, x * ts, y * ts)
            })
            .collect();
        push_outnumbered(&observation, &[1, 2])
    };
    assert!(
        !push(&[(30.0, 20.0), (31.0, 20.0)]),
        "two against two is even"
    );
    assert!(push(&[(30.0, 20.0), (31.0, 20.0), (32.0, 21.0)]));
    assert!(
        !push(&[(50.0, 20.0), (51.0, 20.0), (52.0, 21.0)]),
        "too far to matter"
    );
}

/// Jeff on the Crossroads east start with `tanks` Tanks and a Scout Car ready by the HQ, the
/// armored attack due, and nothing else in the way.
fn crossroads_armor(tanks: u32) -> (AiObservation, FrontalWavePlan, EnemyBaseFact) {
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, PlayerInit};
    let players: Vec<_> = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        })
        .collect();
    let map = Map::load_for_players("Crossroads", &[(1, 1), (2, 2)], 0x1234_5678).unwrap();
    let game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        0x1234_5678,
        map,
        Map::metadata_for_name("Crossroads").unwrap(),
    );
    let start = game.start_payload();
    let mut observation =
        AiObservation::from_snapshot_with_alive(&start, &game.snapshot_for(1), 1, [], None)
            .unwrap();
    assert_eq!(observation.own_start_tile, (117, 78));
    let ts = observation.map.tile_size as f32;
    let mut ready = Vec::new();
    for index in 0..=tanks {
        let id = 900 + index;
        let kind = if index == tanks {
            EntityKind::ScoutCar
        } else {
            EntityKind::Tank
        };
        let mut unit = target_test_entity(id, kind, (110.0 + index as f32) * ts, 76.5 * ts);
        unit.owner = 1;
        observation.owned.push(unit);
        ready.push(id);
    }
    let plan = FrontalWavePlan {
        ready_units: ready,
        desired_size: 3,
        attack_due: true,
        required_unit_ready: true,
        methamphetamines_ready: true,
        blockers: Vec::new(),
    };
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (47, 8),
        x: 47.5 * ts,
        y: 8.5 * ts,
    };
    (observation, plan, enemy_base)
}

#[test]
fn crossroads_armor_too_small_for_the_push_stays_home_instead_of_attacking() {
    let armored = JEFFS_AI.tech_transition.unwrap().attack;
    let (observation, plan, enemy_base) = crossroads_armor(3);
    let facts = AiFacts::from_observation(&observation);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let intent = issue_frontal_wave(
        &mut actions,
        &observation,
        &JEFFS_AI,
        armored,
        &plan,
        enemy_base,
        None,
        None,
        &mut memory,
    );
    assert!(
        !matches!(intent, Some(AiIntent::Attack { .. })),
        "{intent:?}"
    );
    assert!(memory.containment.active_tanks.is_empty());
    assert!(!memory.containment.wave_launched);

    // Two Tanks stay home, so eight ready Tanks push six. With four enemy Tanks seen recently the
    // push needs a three-Tank lead, seven, so nine must be ready.
    let form = |tanks: u32, seen_enemy_tanks: u32| {
        let (mut observation, plan, enemy_base) = crossroads_armor(tanks);
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        observation.visible_enemies = (0..seen_enemy_tanks)
            .map(|index| {
                target_test_entity(500 + index, EntityKind::Tank, 60.0 * 32.0, 60.0 * 32.0)
            })
            .collect();
        memory.note_enemy_tanks(&observation);
        observation.visible_enemies.clear();
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let intent = issue_frontal_wave(
            &mut actions,
            &observation,
            &JEFFS_AI,
            armored,
            &plan,
            enemy_base,
            None,
            None,
            &mut memory,
        );
        (intent, memory.containment.active_tanks.len())
    };
    let (intent, pushing) = form(8, 0);
    assert!(
        matches!(intent, Some(AiIntent::Assemble { .. })),
        "{intent:?}"
    );
    assert_eq!(pushing, CROSSROADS_PUSH_MIN_TANKS);
    let (intent, pushing) = form(6, 4);
    assert!(
        !matches!(intent, Some(AiIntent::Attack { .. })),
        "{intent:?}"
    );
    assert_eq!(pushing, 0, "six Tanks are no lead over four");
    let (intent, pushing) = form(9, 4);
    assert!(
        matches!(intent, Some(AiIntent::Assemble { .. })),
        "{intent:?}"
    );
    assert_eq!(pushing, 7);
}

#[test]
fn the_push_takes_every_ready_tank_but_the_home_reserve() {
    assert_eq!(push_tank_count(23, 2, 4), Some(19));
    assert_eq!(push_tank_count(8, 2, 1), Some(7));
    // Never below the minimum, and never out of the reserve: the push waits instead.
    assert_eq!(push_tank_count(3, 2, 2), None);
    assert_eq!(push_tank_count(8, 6, 3), None);
}

#[test]
fn the_home_reserve_scales_with_the_largest_recent_attack() {
    let ts = 32.0;
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    assert_eq!(memory.home_tank_reserve(), 2, "nothing seen yet");
    let mut attacked = |tanks: u32, tick: u32| {
        let mut depot = target_test_entity(1, EntityKind::ResourceDepot, 20.0 * ts, 20.0 * ts);
        depot.owner = 1;
        let mut observation = regroup_test_observation(vec![depot]);
        observation.tick = tick;
        observation.visible_enemies = (0..tanks)
            .map(|index| {
                target_test_entity(
                    500 + index,
                    EntityKind::Tank,
                    (22.0 + 0.4 * index as f32) * ts,
                    20.0 * ts,
                )
            })
            .collect();
        memory.note_enemy_tanks(&observation);
        memory.home_tank_reserve()
    };
    assert_eq!(attacked(2, 1000), 2);
    assert_eq!(attacked(4, 2000), 3);
    assert_eq!(attacked(8, 3000), 5, "at most five");
    // Forgotten once the attacks are more than three minutes old.
    assert_eq!(attacked(0, 3000 + memory::ENEMY_ATTACK_MEMORY_TICKS + 1), 2);
}

#[test]
fn tanks_join_the_push_until_it_leaves_and_not_after() {
    let armored = JEFFS_AI.tech_transition.unwrap().attack;
    let (mut observation, mut plan, enemy_base) = crossroads_armor(20);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let issue =
        |observation: &AiObservation, plan: &FrontalWavePlan, memory: &mut AiDecisionMemory| {
            let facts = AiFacts::from_observation(observation);
            let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
            issue_frontal_wave(
                &mut actions,
                observation,
                &JEFFS_AI,
                armored,
                plan,
                enemy_base,
                None,
                None,
                memory,
            )
        };
    issue(&observation, &plan, &mut memory);
    assert_eq!(
        memory.containment.active_tanks.len(),
        18,
        "20 ready, 2 stay home"
    );

    // Three Tanks finish while it forms up: 23 ready, so the push grows to 21.
    let ts = observation.map.tile_size as f32;
    let add_tanks = |observation: &mut AiObservation, plan: &mut FrontalWavePlan, ids: [u32; 3]| {
        for id in ids {
            let mut tank = target_test_entity(id, EntityKind::Tank, 112.0 * ts, 80.5 * ts);
            tank.owner = 1;
            observation.owned.push(tank);
            plan.ready_units.push(id);
        }
    };
    add_tanks(&mut observation, &mut plan, [950, 951, 952]);
    issue(&observation, &plan, &mut memory);
    assert_eq!(memory.containment.active_tanks.len(), 21);

    // Once it has left, Tanks built afterwards stay home.
    memory.containment.wave_launched = true;
    memory.containment.recovery_active = false;
    memory.containment.launch_tanks = 21;
    let left_with = memory.containment.active_tanks.clone();
    add_tanks(&mut observation, &mut plan, [960, 961, 962]);
    issue(&observation, &plan, &mut memory);
    assert_eq!(memory.containment.active_tanks, left_with);
}

#[test]
fn a_large_push_carries_on_until_half_of_it_is_lost() {
    let tanks: Vec<AiEntitySummary> = (1..=10)
        .map(|id| {
            let mut tank = target_test_entity(id, EntityKind::Tank, 20.0 * 32.0, 20.0 * 32.0);
            tank.owner = 1;
            tank
        })
        .collect();
    let mut scout = target_test_entity(99, EntityKind::ScoutCar, 22.0 * 32.0, 20.0 * 32.0);
    scout.owner = 1;
    let pushed = |alive: u32| {
        let mut owned: Vec<_> = tanks
            .iter()
            .filter(|tank| tank.id <= alive)
            .cloned()
            .collect();
        owned.push(scout.clone());
        let observation = regroup_test_observation(owned);
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        memory.containment.wave_launched = true;
        memory.containment.active_tanks = (1..=10).collect();
        memory.containment.active_scout = Some(99);
        memory.containment.launch_tanks = 10;
        sync_containment_recovery(&observation, &JEFFS_AI, &mut memory);
        !memory.containment.recovery_active
    };
    assert!(pushed(10));
    assert!(pushed(6), "four of ten lost");
    assert!(pushed(5), "half of it left");
    assert!(!pushed(4), "more than half lost");
}

#[test]
fn a_large_push_forms_ranks_of_six() {
    let tanks: Vec<AiEntitySummary> = (1..=12)
        .map(|id| {
            let mut tank =
                target_test_entity(id, EntityKind::Tank, (10.0 + id as f32) * 32.0, 30.0 * 32.0);
            tank.owner = 1;
            tank
        })
        .collect();
    let observation = regroup_test_observation(tanks);
    let ids: Vec<u32> = (1..=12).collect();
    let center = (30.0 * 32.0, 30.0 * 32.0);
    let assignments = compact_tank_formation_assignments(
        &observation,
        &ids,
        center,
        (0.0, -1.0),
        observation.map,
        CONTAINMENT_TANK_SPACING_TILES,
    );
    assert_eq!(assignments.len(), 12);
    let front: Vec<_> = assignments
        .iter()
        .filter(|(_, point)| (point.1 - center.1).abs() < 1.0)
        .collect();
    let back: Vec<_> = assignments
        .iter()
        .filter(|(_, point)| (point.1 - (center.1 + 2.0 * 32.0)).abs() < 1.0)
        .collect();
    assert_eq!((front.len(), back.len()), (6, 6));
    let width = |rank: &[&(u32, (f32, f32))]| {
        let xs: Vec<f32> = rank.iter().map(|(_, point)| point.0).collect();
        xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min)
    };
    assert!(width(&front) <= 5.0 * CONTAINMENT_TANK_SPACING_TILES * 32.0 + 0.5);
}

#[test]
fn a_wedged_tank_is_let_go_of_until_it_moves_again() {
    let tank = |id: u32, x: f32, state: AiEntityState| AiEntitySummary {
        state,
        ..target_test_entity(id, EntityKind::Tank, x * 32.0, 320.0)
    };
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.active_tanks = [1, 2].into_iter().collect();
    // Tank 1 is ordered on but stays put (wedged); Tank 2 drives on.
    let mut observation = regroup_test_observation(Vec::new());
    for step in 0..=70u32 {
        observation.tick = step * 9;
        observation.owned = vec![
            tank(1, 20.0, AiEntityState::Move),
            tank(2, 20.0 + step as f32 * 0.1, AiEntityState::Move),
        ];
        containment::drop_stuck_push_tanks(&observation, &mut memory);
    }
    assert!(
        !memory.containment.active_tanks.contains(&1),
        "the wedged Tank is let go of"
    );
    assert!(memory.containment.stuck_tanks.contains_key(&1));
    assert!(
        memory.containment.active_tanks.contains(&2),
        "the moving Tank stays"
    );

    // A Tank standing still on a hold order is not stuck.
    memory.containment.active_tanks.insert(3);
    for step in 71..=150u32 {
        observation.tick = step * 9;
        observation.owned = vec![
            tank(1, 20.0, AiEntityState::Move),
            tank(3, 30.0, AiEntityState::Idle),
        ];
        containment::drop_stuck_push_tanks(&observation, &mut memory);
    }
    assert!(memory.containment.active_tanks.contains(&3));

    // Once it gets free and moves off, pushes may take it again.
    observation.owned = vec![tank(1, 24.0, AiEntityState::Move)];
    containment::drop_stuck_push_tanks(&observation, &mut memory);
    assert!(!memory.containment.stuck_tanks.contains_key(&1));
}

#[test]
fn a_natural_mined_out_or_beside_our_own_depot_is_no_push_objective() {
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (116, 116),
        x: 116.5 * 32.0,
        y: 116.5 * 32.0,
    };
    let tile = |x: f32, y: f32| (x * 32.0, y * 32.0);
    let mut depot = target_test_entity(1, EntityKind::ResourceDepot, 90.0 * 32.0, 41.0 * 32.0);
    depot.owner = 1;
    let observation = regroup_test_observation(vec![depot]);
    // The enemy's natural, 26 tiles out, is the objective.
    assert!(!containment::natural_is_gone(
        &observation,
        tile(110.0, 91.0),
        enemy_base
    ));
    // Steel left only beside our own expansion is not.
    assert!(containment::natural_is_gone(
        &observation,
        tile(94.0, 44.0),
        enemy_base
    ));
    // Nor is steel far from the enemy main.
    let none = regroup_test_observation(Vec::new());
    assert!(containment::natural_is_gone(
        &none,
        tile(54.0, 110.0),
        enemy_base
    ));
}
