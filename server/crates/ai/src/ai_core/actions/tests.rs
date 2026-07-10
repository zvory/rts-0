use super::*;
use crate::ai_core::observation::{AiEconomy, AiMapSummary, AiObservation, AiPlayerSummary};
use crate::ai_core::resource_availability::ResourceAvailability;

fn worker(id: u32, x: f32, y: f32, state: AiEntityState) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind: EntityKind::Worker,
        x,
        y,
        hp: 100,
        state,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

fn production_building(id: u32, kind: EntityKind, queue_len: usize) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x: 0.0,
        y: 0.0,
        hp: 100,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: Some(queue_len),
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

fn complete_building(id: u32, kind: EntityKind) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x: 0.0,
        y: 0.0,
        hp: 100,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

fn resource(id: u32, kind: EntityKind, x: f32, y: f32) -> AiResourceSummary {
    AiResourceSummary {
        id,
        kind,
        x,
        y,
        remaining: 100,
    }
}

fn observation(
    economy: AiEconomy,
    owned: Vec<AiEntitySummary>,
    resources: Vec<AiResourceSummary>,
) -> AiObservation {
    AiObservation {
        player_id: 1,
        tick: 0,
        map: AiMapSummary {
            width: 32,
            height: 32,
            tile_size: config::TILE_SIZE,
        },
        economy,
        own_start_tile: (8, 8),
        players: vec![AiPlayerSummary {
            id: 1,
            team_id: 1,
            start_tile: (8, 8),
            is_ai: false,
            is_alive: true,
        }],
        owned,
        resources,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn budget_from_observation(observation: &AiObservation) -> SpendBudget {
    SpendBudget::new(
        observation.economy.steel,
        observation.economy.oil,
        observation.economy.supply_used,
        observation.economy.supply_cap,
    )
}

fn context_from_facts<'a>(facts: &'a AiFacts, observation: &AiObservation) -> AiActionContext<'a> {
    AiActionContext::new(facts, budget_from_observation(observation))
}

fn facts_from_observation(observation: &AiObservation) -> AiFacts {
    AiFacts::from_observation(observation)
}

#[test]
fn committed_steel_is_reserved_from_budget() {
    let budget = SpendBudget::with_committed_steel(150, 0, 0, 10, 100);

    assert_eq!(budget.steel, 50);
    assert!(!budget.can_afford_building(EntityKind::Depot));
}

#[test]
fn build_action_reserves_worker_and_cost() {
    let observation = observation(
        AiEconomy {
            steel: 100,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
        Vec::new(),
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let empty = BTreeSet::new();
    let workers = [10];

    let action = try_build(
        &mut ctx,
        &[&workers],
        BuildPlacementRequest {
            building: EntityKind::Depot,
            map_width: 32,
            map_height: 32,
            start_tile: (8, 8),
            search: ai_shared::BuildSearch {
                min_radius: 0,
                max_radius: 0,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            skip_tiles: &empty,
            placeable: |tx, ty| (tx, ty) == (8, 8),
        },
    );

    assert_eq!(
        action,
        Some(BuildAction {
            worker: 10,
            building: EntityKind::Depot,
            tile_x: 8,
            tile_y: 8,
        })
    );
    assert!(ctx.reservations().worker_reserved(10));
    assert_eq!(ctx.budget().steel, 0);
    assert!(matches!(
        ctx.into_commands().as_slice(),
        [Command::Build { units, building, tile_x: 8, tile_y: 8, .. }]
            if units.as_slice() == [10] && *building == EntityKind::Depot
    ));
}

#[test]
fn second_build_action_cannot_reuse_same_worker() {
    let observation = observation(
        AiEconomy {
            steel: 300,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
        Vec::new(),
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let empty = BTreeSet::new();
    let workers = [10];
    let request = || BuildPlacementRequest {
        building: EntityKind::Depot,
        map_width: 32,
        map_height: 32,
        start_tile: (8, 8),
        search: ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        skip_tiles: &empty,
        placeable: |_, _| true,
    };

    assert!(try_build(&mut ctx, &[&workers], request()).is_some());
    assert!(try_build(&mut ctx, &[&workers], request()).is_none());
    assert_eq!(ctx.into_commands().len(), 1);
}

#[test]
fn unit_training_respects_local_budget_and_supply() {
    let observation = observation(
        AiEconomy {
            steel: 100,
            oil: 0,
            supply_used: 9,
            supply_cap: 10,
        },
        vec![
            production_building(20, EntityKind::CityCentre, 0),
            production_building(21, EntityKind::CityCentre, 0),
        ],
        Vec::new(),
    );
    let facts = AiFacts::from_observation(&observation);
    let mut ctx = AiActionContext::new(
        &facts,
        SpendBudget::new(
            observation.economy.steel,
            observation.economy.oil,
            observation.economy.supply_used,
            observation.economy.supply_cap,
        ),
    );

    let trained = train_units(
        &mut ctx,
        TrainUnitsRequest {
            buildings: facts.production_buildings(EntityKind::CityCentre),
            unit_priorities: &[EntityKind::Worker],
            completed_building_kinds: facts.complete_building_kinds(),
            completed_upgrades: facts.completed_upgrades(),
            max_queue_depth: 1,
            save_for_tech: false,
            current_counts: &[(EntityKind::Worker, 0)],
            max_counts: &[(EntityKind::Worker, 2)],
            balance_unit_priorities: false,
        },
    );

    assert_eq!(trained.len(), 1);
    assert_eq!(ctx.budget().free_supply(), 0);
    assert_eq!(ctx.into_commands().len(), 1);
}

#[test]
fn support_training_requires_tech_and_can_balance_priorities() {
    let without_tech = observation(
        AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 0,
            supply_cap: 20,
        },
        vec![
            production_building(20, EntityKind::Barracks, 0),
            production_building(21, EntityKind::Barracks, 0),
            production_building(22, EntityKind::Steelworks, 0),
        ],
        Vec::new(),
    );
    let facts = AiFacts::from_observation(&without_tech);
    let mut ctx = AiActionContext::new(&facts, budget_from_observation(&without_tech));

    let trained = train_units(
        &mut ctx,
        TrainUnitsRequest {
            buildings: facts.production_buildings(EntityKind::Barracks),
            unit_priorities: &[EntityKind::MachineGunner, EntityKind::AntiTankGun],
            completed_building_kinds: facts.complete_building_kinds(),
            completed_upgrades: facts.completed_upgrades(),
            max_queue_depth: 1,
            save_for_tech: false,
            current_counts: &[],
            max_counts: &[],
            balance_unit_priorities: true,
        },
    );

    assert!(trained.is_empty());
    assert!(ctx.into_commands().is_empty());

    let with_training_centre = observation(
        AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 0,
            supply_cap: 20,
        },
        vec![
            production_building(20, EntityKind::Barracks, 0),
            production_building(21, EntityKind::Barracks, 0),
            production_building(22, EntityKind::Steelworks, 0),
            complete_building(30, EntityKind::TrainingCentre),
        ],
        Vec::new(),
    );
    let facts = AiFacts::from_observation(&with_training_centre);
    let mut ctx = AiActionContext::new(&facts, budget_from_observation(&with_training_centre));

    let trained = train_units(
        &mut ctx,
        TrainUnitsRequest {
            buildings: facts.production_buildings(EntityKind::Barracks),
            unit_priorities: &[EntityKind::MachineGunner, EntityKind::AntiTankGun],
            completed_building_kinds: facts.complete_building_kinds(),
            completed_upgrades: facts.completed_upgrades(),
            max_queue_depth: 1,
            save_for_tech: false,
            current_counts: &[],
            max_counts: &[],
            balance_unit_priorities: true,
        },
    );

    assert_eq!(
        trained.iter().map(|action| action.unit).collect::<Vec<_>>(),
        vec![EntityKind::MachineGunner, EntityKind::MachineGunner]
    );

    let mut with_steelworks = observation(
        AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 0,
            supply_cap: 20,
        },
        vec![
            production_building(20, EntityKind::Barracks, 0),
            production_building(21, EntityKind::Barracks, 0),
            production_building(22, EntityKind::Steelworks, 0),
            complete_building(30, EntityKind::TrainingCentre),
            complete_building(31, EntityKind::Steelworks),
        ],
        Vec::new(),
    );
    with_steelworks
        .upgrades
        .push(UpgradeKind::AntiTankGunUnlock);
    let facts = AiFacts::from_observation(&with_steelworks);
    let mut ctx = AiActionContext::new(&facts, budget_from_observation(&with_steelworks));

    let trained = train_units(
        &mut ctx,
        TrainUnitsRequest {
            buildings: facts.production_buildings(EntityKind::Barracks),
            unit_priorities: &[EntityKind::MachineGunner, EntityKind::AntiTankGun],
            completed_building_kinds: facts.complete_building_kinds(),
            completed_upgrades: facts.completed_upgrades(),
            max_queue_depth: 1,
            save_for_tech: false,
            current_counts: &[],
            max_counts: &[],
            balance_unit_priorities: true,
        },
    );

    assert_eq!(
        trained.iter().map(|action| action.unit).collect::<Vec<_>>(),
        vec![EntityKind::MachineGunner, EntityKind::MachineGunner]
    );

    let trained = train_units(
        &mut ctx,
        TrainUnitsRequest {
            buildings: facts.production_buildings(EntityKind::Steelworks),
            unit_priorities: &[EntityKind::MachineGunner, EntityKind::AntiTankGun],
            completed_building_kinds: facts.complete_building_kinds(),
            completed_upgrades: facts.completed_upgrades(),
            max_queue_depth: 1,
            save_for_tech: false,
            current_counts: &[(EntityKind::MachineGunner, 2)],
            max_counts: &[],
            balance_unit_priorities: true,
        },
    );

    assert_eq!(
        trained.iter().map(|action| action.unit).collect::<Vec<_>>(),
        vec![EntityKind::AntiTankGun]
    );
}

#[test]
fn resource_assignment_picks_distinct_nodes() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![
            worker(10, 0.0, 0.0, AiEntityState::Idle),
            worker(11, 8.0, 0.0, AiEntityState::Idle),
        ],
        vec![
            resource(30, EntityKind::Steel, 64.0, 0.0),
            resource(31, EntityKind::Steel, 96.0, 0.0),
        ],
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let empty = BTreeSet::new();
    let assignable_node_ids = BTreeSet::from([30, 31]);

    let assigned = assign_workers_to_resource(
        &mut ctx,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Steel,
            assignable_node_ids: &assignable_node_ids,
            candidate_worker_ids: None,
            skip_workers: &empty,
            pre_reserved_nodes: &empty,
            idle_only: true,
            allow_latched_reassignment: false,
            max_assignments: None,
            max_worker_resource_distance_px: None,
            remote_worker_assignment_fallback: false,
        },
    );

    assert_eq!(assigned.len(), 2);
    assert_eq!(assigned[0].node, 30);
    assert_eq!(assigned[1].node, 31);
}

#[test]
fn resource_assignment_can_reassign_latched_workers_when_allowed() {
    let pump_jack_steel = rts_rules::economy::cost(EntityKind::PumpJack).0;
    let mut latched = worker(10, 0.0, 0.0, AiEntityState::Gather);
    latched.latched_node = Some(30);
    let observation = observation(
        AiEconomy {
            steel: pump_jack_steel,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![latched],
        vec![
            resource(30, EntityKind::Steel, 64.0, 0.0),
            resource(31, EntityKind::Oil, 96.0, 0.0),
        ],
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let empty = BTreeSet::new();
    let assignable_node_ids = BTreeSet::from([31]);

    let assigned = assign_workers_to_resource(
        &mut ctx,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Oil,
            assignable_node_ids: &assignable_node_ids,
            candidate_worker_ids: Some(&[10]),
            skip_workers: &empty,
            pre_reserved_nodes: &empty,
            idle_only: false,
            allow_latched_reassignment: true,
            max_assignments: None,
            max_worker_resource_distance_px: None,
            remote_worker_assignment_fallback: false,
        },
    );

    assert_eq!(
        assigned,
        vec![ResourceAssignment {
            worker: 10,
            node: 31
        }]
    );
    assert_eq!(ctx.budget().steel(), 0);
    assert!(matches!(
        ctx.into_commands().as_slice(),
        [Command::Build {
            units,
            building: EntityKind::PumpJack,
            tile_x: 3,
            tile_y: 0,
            queued: false
        }] if units == &vec![10]
    ));
}

#[test]
fn oil_resource_assignment_requires_pump_jack_budget() {
    let pump_jack_steel = rts_rules::economy::cost(EntityKind::PumpJack).0;
    let observation = observation(
        AiEconomy {
            steel: pump_jack_steel.saturating_sub(1),
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
        vec![resource(31, EntityKind::Oil, 96.0, 0.0)],
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let empty = BTreeSet::new();
    let assignable_node_ids = BTreeSet::from([31]);

    let assigned = assign_workers_to_resource(
        &mut ctx,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Oil,
            assignable_node_ids: &assignable_node_ids,
            candidate_worker_ids: Some(&[10]),
            skip_workers: &empty,
            pre_reserved_nodes: &empty,
            idle_only: true,
            allow_latched_reassignment: false,
            max_assignments: Some(1),
            max_worker_resource_distance_px: None,
            remote_worker_assignment_fallback: false,
        },
    );

    assert!(assigned.is_empty());
    assert_eq!(
        ctx.reservations().counts(),
        ReservationCounts {
            workers: 0,
            resource_nodes: 0,
            production_buildings: 0
        }
    );
    assert!(ctx.into_commands().is_empty());
}

#[test]
fn resource_assignment_ignores_nodes_beyond_worker_distance_limit() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
        vec![
            resource(30, EntityKind::Steel, 64.0, 0.0),
            resource(31, EntityKind::Steel, 640.0, 0.0),
        ],
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let mut reserved = BTreeSet::new();
    reserved.insert(30);
    let empty = BTreeSet::new();
    let assignable_node_ids = BTreeSet::from([30, 31]);

    let assigned = assign_workers_to_resource(
        &mut ctx,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Steel,
            assignable_node_ids: &assignable_node_ids,
            candidate_worker_ids: None,
            skip_workers: &empty,
            pre_reserved_nodes: &reserved,
            idle_only: true,
            allow_latched_reassignment: false,
            max_assignments: None,
            max_worker_resource_distance_px: Some(128.0),
            remote_worker_assignment_fallback: false,
        },
    );

    assert!(assigned.is_empty());
    assert!(ctx.into_commands().is_empty());
}

#[test]
fn resource_assignment_can_fall_back_to_remote_node_after_local_nodes_fill() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
        vec![
            resource(30, EntityKind::Steel, 64.0, 0.0),
            resource(31, EntityKind::Steel, 640.0, 0.0),
        ],
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let reserved = BTreeSet::from([30]);
    let empty = BTreeSet::new();
    let assignable_node_ids = BTreeSet::from([30, 31]);

    let assigned = assign_workers_to_resource(
        &mut ctx,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Steel,
            assignable_node_ids: &assignable_node_ids,
            candidate_worker_ids: None,
            skip_workers: &empty,
            pre_reserved_nodes: &reserved,
            idle_only: true,
            allow_latched_reassignment: false,
            max_assignments: None,
            max_worker_resource_distance_px: Some(128.0),
            remote_worker_assignment_fallback: true,
        },
    );

    assert_eq!(
        assigned,
        vec![ResourceAssignment {
            worker: 10,
            node: 31
        }]
    );
}

#[test]
fn assign_workers_to_resource_ignores_non_mineable_oil_without_reserving_worker() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
        vec![
            resource(30, EntityKind::Steel, 32.0, 0.0),
            resource(31, EntityKind::Oil, 64.0, 0.0),
        ],
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let empty = BTreeSet::new();
    let no_mineable_oil = BTreeSet::new();

    let assigned_oil = assign_workers_to_resource(
        &mut ctx,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Oil,
            assignable_node_ids: &no_mineable_oil,
            candidate_worker_ids: Some(&[10]),
            skip_workers: &empty,
            pre_reserved_nodes: &empty,
            idle_only: true,
            allow_latched_reassignment: false,
            max_assignments: Some(1),
            max_worker_resource_distance_px: None,
            remote_worker_assignment_fallback: false,
        },
    );

    assert!(assigned_oil.is_empty());
    assert_eq!(
        ctx.reservations().counts(),
        ReservationCounts {
            workers: 0,
            resource_nodes: 0,
            production_buildings: 0
        }
    );

    let mineable_steel = BTreeSet::from([30]);
    let assigned_steel = assign_workers_to_resource(
        &mut ctx,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Steel,
            assignable_node_ids: &mineable_steel,
            candidate_worker_ids: Some(&[10]),
            skip_workers: &empty,
            pre_reserved_nodes: &empty,
            idle_only: true,
            allow_latched_reassignment: false,
            max_assignments: Some(1),
            max_worker_resource_distance_px: None,
            remote_worker_assignment_fallback: false,
        },
    );

    assert_eq!(
        assigned_steel,
        vec![ResourceAssignment {
            worker: 10,
            node: 30
        }]
    );
    assert!(matches!(
        ctx.into_commands().as_slice(),
        [Command::Gather { units, node: 30, queued: false }] if units == &vec![10]
    ));
}

#[test]
fn assign_workers_to_resource_accepts_completed_expansion_oil_candidate() {
    let pump_jack_steel = rts_rules::economy::cost(EntityKind::PumpJack).0;
    let observation = observation(
        AiEconomy {
            steel: pump_jack_steel,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![
            complete_building(20, EntityKind::CityCentre),
            worker(10, 0.0, 0.0, AiEntityState::Idle),
        ],
        vec![resource(31, EntityKind::Oil, 0.0, 0.0)],
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);
    let empty = BTreeSet::new();
    let availability = ResourceAvailability::from_observation(&observation, &empty);
    let mineable_oil = availability.free_mineable_node_ids(EntityKind::Oil);

    let assigned = assign_workers_to_resource(
        &mut ctx,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Oil,
            assignable_node_ids: &mineable_oil,
            candidate_worker_ids: Some(&[10]),
            skip_workers: &empty,
            pre_reserved_nodes: &empty,
            idle_only: true,
            allow_latched_reassignment: false,
            max_assignments: Some(1),
            max_worker_resource_distance_px: None,
            remote_worker_assignment_fallback: false,
        },
    );

    assert_eq!(
        assigned,
        vec![ResourceAssignment {
            worker: 10,
            node: 31
        }]
    );
}

#[test]
fn attack_command_unit_order_is_deterministic() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        Vec::new(),
        Vec::new(),
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);

    let units = attack_move_units(&mut ctx, [5, 3, 4, 3], 10.0, 20.0);

    assert_eq!(units, Some(vec![3, 4, 5]));
    assert!(matches!(
        ctx.into_commands().as_slice(),
        [Command::AttackMove { units, x: 10.0, y: 20.0, .. }]
            if units == &vec![3, 4, 5]
    ));
}

#[test]
fn move_command_unit_order_is_deterministic() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        Vec::new(),
        Vec::new(),
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);

    let units = move_units(&mut ctx, [5, 3, 4, 3], 10.0, 20.0);

    assert_eq!(units, Some(vec![3, 4, 5]));
    assert!(matches!(
        ctx.into_commands().as_slice(),
        [Command::Move { units, x: 10.0, y: 20.0, .. }]
            if units == &vec![3, 4, 5]
    ));
}

#[test]
fn staging_uses_point_toward_target_and_deterministic_units() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        Vec::new(),
        Vec::new(),
    );
    let facts = facts_from_observation(&observation);
    let mut ctx = context_from_facts(&facts, &observation);

    let units = stage_units_toward(&mut ctx, [8, 6, 8], (0.0, 0.0), (96.0, 0.0), 32, 2.0);

    assert_eq!(units, Some(vec![6, 8]));
    assert!(matches!(
        ctx.into_commands().as_slice(),
        [Command::AttackMove { units, x: 64.0, y: 0.0, .. }]
            if units == &vec![6, 8]
    ));
}
