use super::defense::{main_steel_cluster_center, DEFENSIVE_MG_PERIMETER_DISTANCE_TILES};
use super::expansion::{
    expansion_candidate_resources, expansion_city_centre_site, nearest_enemy_start_distance2,
};
use super::geometry::{
    building_center, dist2, footprint_edge_distance_tiles, normalized_direction,
    point_line_distance2, squared,
};
use super::raids::enemy_main_steel_center;
use super::*;

use crate::ai_core::observation::{
    AiBuildIntent, AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiPlayerSummary,
    AiResourceSummary,
};
use crate::ai_core::profiles::{
    AI_1_0_TECH, AI_1_1_TANK_MG, AI_1_2_WAVE_COHORTS, AI_TURTLE_CHOKES, RIFLE_FLOOD_FAST,
    RIFLE_FLOOD_FULL_SATURATION, STEEL_EXPANSION_TANKS, TECH_TO_TANKS,
};
mod steel_line_tests;
fn worker(id: u32, state: AiEntityState) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind: EntityKind::Worker,
        x: id as f32,
        y: 0.0,
        state,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

fn worker_at(id: u32, state: AiEntityState, x: f32, y: f32) -> AiEntitySummary {
    let mut worker = worker(id, state);
    worker.x = x;
    worker.y = y;
    worker
}

fn steel_worker(id: u32, node: u32) -> AiEntitySummary {
    gathering_worker(id, node)
}

fn gathering_worker(id: u32, node: u32) -> AiEntitySummary {
    let mut worker = worker(id, AiEntityState::Gather);
    worker.latched_node = Some(node);
    worker
}

fn resource(id: u32, kind: EntityKind, x: f32, y: f32) -> AiResourceSummary {
    AiResourceSummary {
        id,
        kind,
        x,
        y,
        remaining: 1_000,
    }
}

fn building(id: u32, kind: EntityKind, queue_len: Option<usize>) -> AiEntitySummary {
    building_at(id, kind, queue_len, 0.0, 0.0)
}

fn building_training(id: u32, kind: EntityKind, unit: EntityKind) -> AiEntitySummary {
    let mut building = building(id, kind, Some(3));
    building.production_kind = Some(unit);
    building
}

fn building_at(
    id: u32,
    kind: EntityKind,
    queue_len: Option<usize>,
    x: f32,
    y: f32,
) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x,
        y,
        state: queue_len
            .filter(|queue| *queue > 0)
            .map(|_| AiEntityState::Train)
            .unwrap_or(AiEntityState::Idle),
        is_complete: true,
        production_queue_len: queue_len,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

fn combat(id: u32, kind: EntityKind) -> AiEntitySummary {
    combat_at(id, kind, 0.0, 0.0)
}

fn combat_at(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x,
        y,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

fn launched_combat_at(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
    let mut unit = combat_at(id, kind, x, y);
    unit.state = AiEntityState::Attack;
    unit
}

fn enemy(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 2,
        kind,
        x,
        y,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

fn observation(economy: AiEconomy, owned: Vec<AiEntitySummary>) -> AiObservation {
    let tile_size = config::TILE_SIZE;
    let mut resources = Vec::new();
    for i in 0..18 {
        resources.push(resource(
            100 + i,
            EntityKind::Steel,
            (8.5 + (i % 6) as f32) * tile_size as f32,
            (8.5 + (i / 6) as f32) * tile_size as f32,
        ));
    }
    for i in 0..3 {
        resources.push(resource(
            200 + i,
            EntityKind::Oil,
            (10.5 + i as f32) * tile_size as f32,
            12.5 * tile_size as f32,
        ));
    }
    AiObservation {
        player_id: 1,
        tick: 90,
        map: AiMapSummary {
            width: 64,
            height: 64,
            tile_size,
        },
        economy,
        own_start_tile: (8, 8),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (8, 8),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (48, 48),
                is_ai: false,
                is_alive: true,
            },
        ],
        owned,
        resources,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn with_expansion_resources(mut observation: AiObservation) -> AiObservation {
    let ts = observation.map.tile_size as f32;
    for i in 0..18 {
        observation.resources.push(resource(
            300 + i,
            EntityKind::Steel,
            (21.5 + (i % 6) as f32) * ts,
            (31.5 + (i / 6) as f32) * ts,
        ));
    }
    for i in 0..3 {
        observation.resources.push(resource(
            400 + i,
            EntityKind::Oil,
            (16.5 + i as f32) * ts,
            38.5 * ts,
        ));
    }
    observation.resources.sort_by_key(|resource| resource.id);
    observation
}

fn with_enemy_main_resources(mut observation: AiObservation) -> AiObservation {
    observation.resources.extend(base_site_resources(
        300,
        enemy_start_tile(&observation),
        observation.map.width,
    ));
    observation.resources.sort_by_key(|resource| resource.id);
    observation
}

#[test]
fn expansion_enemy_start_distance_ignores_allied_starts() {
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
        },
        Vec::new(),
    );
    observation.own_start_tile = (8, 8);
    observation.players = vec![
        AiPlayerSummary {
            id: 1,
            team_id: 7,
            start_tile: (8, 8),
            is_ai: true,
            is_alive: true,
        },
        AiPlayerSummary {
            id: 2,
            team_id: 7,
            start_tile: (10, 8),
            is_ai: true,
            is_alive: true,
        },
        AiPlayerSummary {
            id: 3,
            team_id: 3,
            start_tile: (24, 8),
            is_ai: true,
            is_alive: true,
        },
    ];
    let ally_center = tile_center((10, 8), observation.map.tile_size);
    let enemy_center = tile_center((24, 8), observation.map.tile_size);

    let distance2 = nearest_enemy_start_distance2(&observation, ally_center.0, ally_center.1)
        .expect("enemy start should be considered");

    assert_eq!(
        distance2,
        dist2(ally_center.0, ally_center.1, enemy_center.0, enemy_center.1)
    );
}

fn enemy_base_fact(observation: &AiObservation) -> EnemyBaseFact {
    let start_tile = enemy_start_tile(observation);
    let (x, y) = tile_center(start_tile, observation.map.tile_size);
    EnemyBaseFact {
        player_id: 2,
        start_tile,
        x,
        y,
    }
}

fn base_site_resources(first_id: u32, site: (u32, u32), map_size: u32) -> Vec<AiResourceSummary> {
    let ts = config::TILE_SIZE as f32;
    let (hx, hy) = (site.0 as f32 + 0.5, site.1 as f32 + 0.5);
    let map_center = map_size as f32 * 0.5;
    let base_angle = (map_center - hy).atan2(map_center - hx);
    let (perp_x, perp_y) = (-base_angle.sin(), base_angle.cos());
    let mut resources = Vec::new();
    let mut steel_index = 0;
    for (side, field_patches) in [
        (1.0, config::STEEL_PATCHES_PER_BASE.div_ceil(2)),
        (-1.0, config::STEEL_PATCHES_PER_BASE / 2),
    ] {
        let block_cx = hx + side * config::STEEL_BLOCK_DIST_TILES * base_angle.cos();
        let block_cy = hy + side * config::STEEL_BLOCK_DIST_TILES * base_angle.sin();
        let row_center = field_patches.div_ceil(6).saturating_sub(1) as f32 / 2.0;
        for i in 0..field_patches {
            let (off_x, off_y) = ((i % 6) as f32 - 2.5, (i / 6) as f32 - row_center);
            resources.push(resource(
                first_id + steel_index,
                EntityKind::Steel,
                (block_cx + off_x * perp_x + off_y * base_angle.cos()) * ts,
                (block_cy + off_x * perp_y + off_y * base_angle.sin()) * ts,
            ));
            steel_index += 1;
        }
    }

    let oil_angle = base_angle + std::f32::consts::FRAC_PI_2;
    let oil_perp_x = -oil_angle.sin();
    let oil_perp_y = oil_angle.cos();
    let oil_cx = hx + config::OIL_DIST_TILES * oil_angle.cos();
    let oil_cy = hy + config::OIL_DIST_TILES * oil_angle.sin();
    for (i, (off_x, off_y)) in [(-0.5, -0.5), (0.5, -0.5), (0.0, 0.5)]
        .into_iter()
        .enumerate()
    {
        resources.push(resource(
            first_id + config::STEEL_PATCHES_PER_BASE + i as u32,
            EntityKind::Oil,
            (oil_cx + off_x * oil_perp_x + off_y * oil_angle.cos()) * ts,
            (oil_cy + off_x * oil_perp_y + off_y * oil_angle.sin()) * ts,
        ));
    }
    resources
}

fn expansion_resource_counts_for_site(
    observation: &AiObservation,
    site: (u32, u32),
) -> (usize, usize) {
    let (cx, cy) = building_center(site, EntityKind::CityCentre, observation.map.tile_size)
        .expect("city centre should have a center");
    let max_dist = config::MINING_CC_RANGE_TILES * observation.map.tile_size as f32;
    let max_dist2 = squared(max_dist);
    let mut steel = 0usize;
    let mut oil = 0usize;
    for resource in expansion_candidate_resources(observation) {
        if dist2(cx, cy, resource.x, resource.y) > max_dist2 {
            continue;
        }
        match resource.kind {
            EntityKind::Steel => steel += 1,
            EntityKind::Oil => oil += 1,
            _ => {}
        }
    }
    (steel, oil)
}

fn decide(
    observation: &AiObservation,
    profile: &'static AiProfile,
    memory: &mut AiDecisionMemory,
) -> AiDecision {
    let width = observation.map.width;
    let height = observation.map.height;
    decide_profile_without_static_map_for_tests(
        observation,
        profile,
        memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, tx, ty| tx < width && ty < height,
    )
}

fn trace_goal(decision: &AiDecision, goal: trace::StrategicGoal) -> &trace::GoalTrace {
    decision
        .trace
        .goals
        .iter()
        .find(|entry| entry.goal == goal)
        .expect("trace should include every strategic goal")
}

#[test]
fn economy_manager_plan_tracks_steel_and_oil_targets() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        8.5 * ts,
        8.5 * ts,
    )];
    owned.extend((0..17).map(|i| steel_worker(20 + i, 100 + i)));
    owned.push(worker(60, AiEntityState::Idle));
    let observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 50,
            supply_cap: 70,
        },
        owned,
    );
    let facts = AiFacts::from_observation(&observation);

    let plan = plan_economy(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        None,
    );

    assert_eq!(plan.target_steel_workers, 18);
    assert_eq!(plan.current_steel_workers, 17);
    assert_eq!(plan.desired_oil_workers, 0);
    assert_eq!(plan.target_workers, 18);
}

#[test]
fn economy_manager_plan_allows_oil_after_steel_floor() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        8.5 * ts,
        8.5 * ts,
    )];
    owned.extend((0..8).map(|i| steel_worker(20 + i, 100 + i)));
    owned.extend((0..3).map(|i| worker(40 + i, AiEntityState::Idle)));
    let observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 11,
            supply_cap: 20,
        },
        owned,
    );
    let facts = AiFacts::from_observation(&observation);

    let plan = plan_economy(&observation, &facts, &TECH_TO_TANKS, false, None);

    assert_eq!(plan.target_steel_workers, 12);
    assert_eq!(plan.current_steel_workers, 8);
    assert_eq!(plan.desired_oil_workers, 3);
}

#[test]
fn economy_manager_plan_suppresses_oil_when_only_steel_is_mineable() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        8.5 * ts,
        8.5 * ts,
    )];
    owned.extend((0..8).map(|i| steel_worker(20 + i, 100 + i)));
    owned.extend((0..3).map(|i| worker(40 + i, AiEntityState::Idle)));
    let mut observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 11,
            supply_cap: 20,
        },
        owned,
    );
    observation
        .resources
        .retain(|resource| resource.kind == EntityKind::Steel);
    observation
        .resources
        .push(resource(500, EntityKind::Oil, 40.5 * ts, 40.5 * ts));
    let facts = AiFacts::from_observation(&observation);

    let plan = plan_economy(&observation, &facts, &TECH_TO_TANKS, false, None);

    assert_eq!(plan.target_steel_workers, 12);
    assert_eq!(plan.current_steel_workers, 8);
    assert_eq!(plan.desired_oil_workers, 0);
}

#[test]
fn economy_manager_plan_waits_for_completed_expansion_before_oil() {
    let ts = config::TILE_SIZE as f32;
    let mut incomplete_expansion =
        building_at(11, EntityKind::CityCentre, Some(0), 40.5 * ts, 40.5 * ts);
    incomplete_expansion.is_complete = false;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        incomplete_expansion,
    ];
    owned.extend((0..8).map(|i| steel_worker(20 + i, 100 + i)));
    owned.extend((0..3).map(|i| worker(40 + i, AiEntityState::Idle)));
    let mut observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 11,
            supply_cap: 20,
        },
        owned,
    );
    observation
        .resources
        .retain(|resource| resource.kind == EntityKind::Steel);
    observation
        .resources
        .push(resource(500, EntityKind::Oil, 40.5 * ts, 40.5 * ts));
    let facts = AiFacts::from_observation(&observation);

    let plan = plan_economy(&observation, &facts, &TECH_TO_TANKS, false, None);

    assert_eq!(plan.desired_oil_workers, 0);

    observation.owned[1].is_complete = true;
    let facts = AiFacts::from_observation(&observation);
    let plan = plan_economy(&observation, &facts, &TECH_TO_TANKS, false, None);

    assert_eq!(plan.desired_oil_workers, 3);
}

#[test]
fn supply_goal_reports_depot_budget_blocker_when_capped() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 10,
            supply_cap: 10,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            worker(20, AiEntityState::Idle),
        ],
    );

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(trace_goal(&decision, trace::StrategicGoal::Supply)
        .blockers
        .contains(&trace::GoalBlocker::BudgetSteel));
    assert!(trace_goal(&decision, trace::StrategicGoal::Production)
        .blockers
        .contains(&trace::GoalBlocker::SupplyCap));
}

#[test]
fn expansion_manager_reports_missing_prerequisite_building() {
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 0,
            supply_used: 30,
            supply_cap: 60,
        },
        vec![building(10, EntityKind::CityCentre, Some(0))],
    ));
    let facts = AiFacts::from_observation(&observation);

    let plan = super::expansion::plan_expansion(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        false,
    );

    assert!(!plan.should_save);
    assert_eq!(plan.policy, RIFLE_FLOOD_FULL_SATURATION.expansion);
    assert_eq!(
        plan.blockers,
        vec![super::expansion::ExpansionBlocker::MissingRequiredBuilding]
    );
}

#[test]
fn expansion_trace_reports_no_valid_site_when_all_sites_are_blocked() {
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 1_000,
            oil: 500,
            supply_used: 12,
            supply_cap: 30,
        },
        owned,
    ));
    let mut memory = AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS);
    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, _, _| false,
    );

    assert!(trace_goal(&decision, trace::StrategicGoal::Expansion)
        .blockers
        .contains(&trace::GoalBlocker::MissingPrerequisite(
            "no_expansion_site"
        )));
}

#[test]
fn manager_trace_formats_goals_and_blockers_deterministically() {
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
        },
        Vec::new(),
    );

    let first = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );
    let second = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert_eq!(first.commands, second.commands);
    assert_eq!(first.intents, second.intents);
    assert_eq!(first.trace.format_compact(), second.trace.format_compact());
    assert_eq!(
        first
            .trace
            .goals
            .iter()
            .map(|entry| entry.goal)
            .collect::<Vec<_>>(),
        vec![
            trace::StrategicGoal::Economy,
            trace::StrategicGoal::Supply,
            trace::StrategicGoal::Expansion,
            trace::StrategicGoal::Tech,
            trace::StrategicGoal::Production,
            trace::StrategicGoal::LocalDefense,
            trace::StrategicGoal::FrontalAttack,
            trace::StrategicGoal::Harassment,
        ]
    );
    assert_eq!(
        trace_goal(&first, trace::StrategicGoal::FrontalAttack).blockers,
        vec![trace::GoalBlocker::WaitingForUnits]
    );
    assert!(first
        .trace
        .format_compact()
        .contains("goal=FrontalAttack status=Skipped blockers=WaitingForUnits"));
}

#[test]
fn manager_trace_records_emitted_command_labels() {
    let ts = config::TILE_SIZE as f32;
    let observation = observation(
        AiEconomy {
            steel: 500,
            oil: 0,
            supply_used: 5,
            supply_cap: 10,
        },
        vec![
            building_at(1, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            worker(10, AiEntityState::Idle),
            worker(11, AiEntityState::Idle),
        ],
    );

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert_eq!(decision.trace.commands.len(), decision.commands.len());
    assert!(decision
        .trace
        .commands
        .iter()
        .any(|entry| { entry.starts_with("train building=1 unit=Worker") }));
    assert!(trace_goal(&decision, trace::StrategicGoal::Economy)
        .intents
        .iter()
        .any(|intent| intent == "train:Worker"));
    assert!(decision
        .trace
        .format_compact()
        .contains("command=train building=1 unit=Worker"));
}

fn enemy_start_tile(observation: &AiObservation) -> (u32, u32) {
    observation
        .players
        .iter()
        .find(|player| player.id != observation.player_id)
        .expect("test observation should have an enemy")
        .start_tile
}

fn footprint_center_tiles(tile: (u32, u32), kind: EntityKind) -> (f32, f32) {
    let stats = config::building_stats(kind).expect("test kind should be a building");
    (
        tile.0 as f32 + stats.foot_w as f32 * 0.5,
        tile.1 as f32 + stats.foot_h as f32 * 0.5,
    )
}

fn proxy_distance_to_enemy_tiles(observation: &AiObservation, tile: (u32, u32)) -> f32 {
    let enemy = enemy_start_tile(observation);
    let enemy_center = (enemy.0 as f32 + 0.5, enemy.1 as f32 + 0.5);
    let barracks_center = footprint_center_tiles(tile, EntityKind::Barracks);
    let dx = barracks_center.0 - enemy_center.0;
    let dy = barracks_center.1 - enemy_center.1;
    (dx * dx + dy * dy).sqrt()
}

fn point_distance_to_enemy_tiles(observation: &AiObservation, point: (f32, f32)) -> f32 {
    let enemy = enemy_start_tile(observation);
    let enemy_center = (enemy.0 as f32 + 0.5, enemy.1 as f32 + 0.5);
    let dx = point.0 - enemy_center.0;
    let dy = point.1 - enemy_center.1;
    (dx * dx + dy * dy).sqrt()
}

fn point_edge_distance_tiles(observation: &AiObservation, point: (f32, f32)) -> f32 {
    point
        .0
        .min(point.1)
        .min(observation.map.width as f32 - point.0)
        .min(observation.map.height as f32 - point.1)
}

fn point_scout_path_distance_tiles(observation: &AiObservation, point: (f32, f32)) -> f32 {
    let own_center = (
        observation.own_start_tile.0 as f32 + 0.5,
        observation.own_start_tile.1 as f32 + 0.5,
    );
    let enemy = enemy_start_tile(observation);
    let enemy_center = (enemy.0 as f32 + 0.5, enemy.1 as f32 + 0.5);
    point_line_distance2(point, own_center, enemy_center).sqrt()
}

fn assert_hidden_proxy_point(observation: &AiObservation, point: (f32, f32)) {
    let distance = point_distance_to_enemy_tiles(observation, point);
    assert!(
        distance >= 18.0,
        "proxy transit target should not be within 18 tiles of the enemy base, got {distance}"
    );
    assert!(
        distance < 20.0,
        "proxy transit target should stay close to the requested 18-tile ring, got {distance}"
    );
    let edge_distance = point_edge_distance_tiles(observation, point);
    assert!(
        edge_distance <= 2.0,
        "proxy transit target should hug a map edge, got {edge_distance} tiles from the edge"
    );
    let scout_path_distance = point_scout_path_distance_tiles(observation, point);
    assert!(
        scout_path_distance >= 8.0,
        "proxy transit target should be off the direct scouting line, got {scout_path_distance}"
    );
}

fn assert_hidden_proxy_site(observation: &AiObservation, tile: (u32, u32)) {
    let distance = proxy_distance_to_enemy_tiles(observation, tile);
    assert!(
        distance >= 18.0,
        "proxy barracks target should not be within 18 tiles of the enemy base, got {distance}"
    );
    assert!(
        distance < 20.0,
        "proxy barracks target should stay close to the requested 18-tile ring, got {distance}"
    );
    let stats = config::building_stats(EntityKind::Barracks).expect("barracks stats");
    let edge_distance =
        footprint_edge_distance_tiles(tile, &stats, observation.map.width, observation.map.height);
    assert!(
        edge_distance <= 1,
        "proxy barracks target should be near a map edge, got {edge_distance} tiles"
    );
    let center = footprint_center_tiles(tile, EntityKind::Barracks);
    let scout_path_distance = point_scout_path_distance_tiles(observation, center);
    assert!(
        scout_path_distance >= 8.0,
        "proxy barracks target should be off the direct scouting line, got {scout_path_distance}"
    );
    assert_ne!(
        tile,
        (observation.map.width / 2, observation.map.height / 2),
        "proxy barracks should no longer use the map center"
    );
}

#[test]
fn fast_flood_sends_proxy_worker_before_barracks_is_affordable() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        8.5 * ts,
        8.5 * ts,
    )];
    owned.extend((0..4).map(|i| worker(20 + i, AiEntityState::Idle)));
    let observation = observation(
        AiEconomy {
            steel: config::STARTING_STEEL,
            oil: 0,
            supply_used: config::STARTING_WORKERS,
            supply_cap: config::CITY_CENTRE_SUPPLY,
        },
        owned,
    );

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    assert!(decision.intents.iter().any(|intent| {
        matches!(
            intent,
            AiIntent::Move { units } if units.as_slice() == [20]
        )
    }));
    let move_target = decision
        .commands
        .iter()
        .find_map(|command| match command {
            Command::Move { units, x, y, .. } if units.as_slice() == [20] => Some((*x, *y)),
            _ => None,
        })
        .expect("proxy worker should receive a move command");
    let tile_size = observation.map.tile_size as f32;
    let move_target_tiles = (move_target.0 / tile_size, move_target.1 / tile_size);
    assert_hidden_proxy_point(&observation, move_target_tiles);
    assert!(
        point_distance_to_enemy_tiles(&observation, move_target_tiles)
            < point_distance_to_enemy_tiles(
                &observation,
                (
                    observation.own_start_tile.0 as f32 + 0.5,
                    observation.own_start_tile.1 as f32 + 0.5,
                ),
            )
    );
    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Build { building, .. }
                    if *building == EntityKind::Barracks
            )
        }),
        "the proxy worker should move out before the barracks is affordable"
    );
    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
}

#[test]
fn fast_flood_stops_worker_training_after_one_extra_worker() {
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
    let observation = observation(
        AiEconomy {
            steel: 75,
            oil: 0,
            supply_used: 5,
            supply_cap: config::CITY_CENTRE_SUPPLY,
        },
        owned,
    );

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
}

#[test]
fn fast_flood_initial_affordable_proxy_uses_hidden_edge_target() {
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
    let observation = observation(
        AiEconomy {
            steel: 150,
            oil: 0,
            supply_used: 5,
            supply_cap: config::CITY_CENTRE_SUPPLY,
        },
        owned,
    );

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    let proxy_builds: Vec<_> = decision
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::Build {
                units,
                building,
                tile_x,
                tile_y,
                ..
            } if *building == EntityKind::Barracks => units
                .first()
                .copied()
                .map(|worker| (worker, (*tile_x, *tile_y))),
            _ => None,
        })
        .collect();

    assert_eq!(
        proxy_builds.len(),
        1,
        "fast rush should send exactly one worker to build the proxy barracks"
    );
    assert_eq!(proxy_builds[0].0, 20);
    assert_hidden_proxy_site(&observation, proxy_builds[0].1);
}

#[test]
fn fast_flood_builds_proxy_barracks_with_reserved_worker_once_affordable() {
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    let tile_size = config::TILE_SIZE as f32;
    let worker_tile = (30.5, 20.5);
    owned.push(worker_at(
        20,
        AiEntityState::Move,
        worker_tile.0 * tile_size,
        worker_tile.1 * tile_size,
    ));
    owned.extend((0..4).map(|i| worker(21 + i, AiEntityState::Gather)));
    let observation = observation(
        AiEconomy {
            steel: 150,
            oil: 0,
            supply_used: 5,
            supply_cap: config::CITY_CENTRE_SUPPLY,
        },
        owned,
    );
    let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
    memory.proxy_worker_id = Some(20);

    let decision = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));
    let proxy_builds: Vec<_> = decision
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::Build {
                units,
                building,
                tile_x,
                tile_y,
                ..
            } if *building == EntityKind::Barracks => units
                .first()
                .copied()
                .map(|worker| (worker, (*tile_x, *tile_y))),
            _ => None,
        })
        .collect();

    assert_eq!(
        proxy_builds.len(),
        1,
        "fast rush should send exactly one worker to build the proxy barracks"
    );
    assert_eq!(proxy_builds[0].0, 20);
    let build_center = footprint_center_tiles(proxy_builds[0].1, EntityKind::Barracks);
    assert!(
        dist2(build_center.0, build_center.1, worker_tile.0, worker_tile.1) <= squared(1.0),
        "committed proxy worker should build near its current position"
    );
    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Build { units, building, .. }
                if units.as_slice() == [20]
                    && *building == EntityKind::Barracks
        )
    }));
    assert!(
        !decision.commands.iter().any(
            |command| matches!(command, Command::Move { units, .. } if units.as_slice() == [20])
        ),
        "the reserved proxy worker should build instead of receiving another move once affordable"
    );
}

#[test]
fn fast_flood_does_not_replace_missing_proxy_worker() {
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
    let observation = observation(
        AiEconomy {
            steel: 150,
            oil: 0,
            supply_used: 5,
            supply_cap: config::CITY_CENTRE_SUPPLY,
        },
        owned,
    );
    let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
    memory.proxy_worker_id = Some(999);

    let decision = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

    assert!(!decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Build { building, .. }
                if *building == EntityKind::Barracks
        )
    }));
    assert!(
        !decision
            .commands
            .iter()
            .any(|command| matches!(command, Command::Move { units, .. } if units.len() == 1)),
        "fast rush should not send a replacement proxy worker after committing one"
    );
}

#[test]
fn fast_flood_spends_first_fifty_steel_on_rifle_where_full_saturation_trains_worker() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
    ];
    owned.extend((0..8).map(|i| worker(20 + i, AiEntityState::Gather)));
    let observation = observation(
        AiEconomy {
            steel: 50,
            oil: 0,
            supply_used: 8,
            supply_cap: 10,
        },
        owned,
    );

    let fast = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );
    let full = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(fast.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
    assert!(full.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
    assert!(!full.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
}

#[test]
fn fast_flood_recovers_after_barracks_rifle_window() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
    ];
    owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
    let mut observation = observation(
        AiEconomy {
            steel: 200,
            oil: 0,
            supply_used: 5,
            supply_cap: 20,
        },
        owned,
    );
    let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
    let before = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

    assert!(
        !before.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }),
        "fast flood should keep its five-worker cap before the recovery window"
    );

    let rifle_build_ticks = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats should exist")
        .build_ticks;
    observation.tick = observation
        .tick
        .saturating_add(rifle_build_ticks.saturating_mul(7));
    let after = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

    assert!(
        after.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }),
        "fast flood should resume worker production once the proxy window has passed"
    );
    assert!(
            after.intents.contains(&AiIntent::Build {
                kind: EntityKind::Barracks
            }),
            "fast flood should add a home barracks during recovery instead of relying only on the proxy"
        );
}

#[test]
fn fast_flood_recovery_builds_support_tech_and_takes_oil() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
    ];
    owned.extend((0..8).map(|i| steel_worker(20 + i, 100 + i)));
    owned.extend((0..3).map(|i| worker(40 + i, AiEntityState::Idle)));
    let mut observation = observation(
        AiEconomy {
            steel: 500,
            oil: 50,
            supply_used: 11,
            supply_cap: 28,
        },
        owned,
    );
    let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
    let _ = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);
    let rifle_build_ticks = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats should exist")
        .build_ticks;
    observation.tick = observation
        .tick
        .saturating_add(rifle_build_ticks.saturating_mul(7));

    let decision = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::TrainingCentre
    }));
    assert!(decision.intents.iter().any(|intent| {
        matches!(
            intent,
            AiIntent::Gather {
                resource: EntityKind::Oil,
                assignments
            } if *assignments > 0
        )
    }));
}

#[test]
fn tech_to_tanks_delays_oil_until_steel_floor_and_builds_tank_tech() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
        building(12, EntityKind::TrainingCentre, None),
    ];
    owned.extend((0..4).map(|i| worker(20 + i, AiEntityState::Idle)));
    let initial_observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 4,
            supply_cap: 20,
        },
        owned,
    );

    let decision = decide(
        &initial_observation,
        &TECH_TO_TANKS,
        &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::ResearchComplex
    }));
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Steelworks
    }));
    assert!(
        decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }),
        "tech_to_tanks should keep worker production alive while saving for tank tech"
    );
    assert!(
        !decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }),
        "tech_to_tanks should save barracks steel once the factory is buildable"
    );
    assert!(
        !decision.intents.iter().any(|intent| matches!(
            intent,
            AiIntent::Gather {
                resource: EntityKind::Oil,
                ..
            }
        )),
        "tech_to_tanks should not send workers to oil before the steel floor is saturated"
    );

    let mut steel_floor_owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
        building(12, EntityKind::TrainingCentre, None),
        building(13, EntityKind::ResearchComplex, None),
    ];
    steel_floor_owned.extend((0..8).map(|i| steel_worker(20 + i, 100 + i)));
    steel_floor_owned.extend((0..3).map(|i| worker(40 + i, AiEntityState::Idle)));
    let steel_floor_observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 11,
            supply_cap: 20,
        },
        steel_floor_owned,
    );

    let steel_floor_decision = decide(
        &steel_floor_observation,
        &TECH_TO_TANKS,
        &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
    );

    assert!(steel_floor_decision.intents.iter().any(|intent| {
        matches!(
            intent,
            AiIntent::Gather {
                resource: EntityKind::Oil,
                assignments
            } if *assignments > 0
        )
    }));
}

#[test]
fn ai_1_0_opens_with_riflemen_before_vehicle_tech() {
    let observation = observation(
        AiEconomy {
            steel: 250,
            oil: 0,
            supply_used: 10,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
        ],
    );

    let decision = decide(
        &observation,
        &AI_1_0_TECH,
        &mut AiDecisionMemory::for_profile(&AI_1_0_TECH),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::ScoutCar
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
}

#[test]
fn ai_1_0_saves_for_tank_tech_and_uses_scout_cars_while_blocked() {
    let mut observation = observation(
        AiEconomy {
            steel: 450,
            oil: 250,
            supply_used: 30,
            supply_cap: 60,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, Some(0)),
            building(13, EntityKind::ResearchComplex, Some(0)),
            building(14, EntityKind::Factory, Some(0)),
        ],
    );

    let mut memory = AiDecisionMemory::for_profile(&AI_1_0_TECH);
    let decision = decide(&observation, &AI_1_0_TECH, &mut memory);

    assert!(decision.intents.contains(&AiIntent::Research {
        upgrade: UpgradeKind::TankUnlock
    }));
    assert!(decision.intents.contains(&AiIntent::Research {
        upgrade: UpgradeKind::Methamphetamines
    }));
    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::ScoutCar
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));

    observation.upgrades.push(UpgradeKind::TankUnlock);
    let unlocked = decide(
        &observation,
        &AI_1_0_TECH,
        &mut AiDecisionMemory::for_profile(&AI_1_0_TECH),
    );

    assert!(!unlocked.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    assert!(unlocked.intents.contains(&AiIntent::Train {
        kind: EntityKind::ScoutCar
    }));

    observation.upgrades.push(UpgradeKind::Methamphetamines);
    let meth_unlocked = decide(
        &observation,
        &AI_1_0_TECH,
        &mut AiDecisionMemory::for_profile(&AI_1_0_TECH),
    );

    assert!(meth_unlocked.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    assert!(!meth_unlocked.intents.contains(&AiIntent::Train {
        kind: EntityKind::ScoutCar
    }));
}

#[test]
fn ai_1_0_tech_path_omits_support_weapon_buildings() {
    let observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 30,
            supply_cap: 70,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            worker(20, AiEntityState::Idle),
            worker(21, AiEntityState::Idle),
        ],
    );

    let decision = decide(
        &observation,
        &AI_1_0_TECH,
        &mut AiDecisionMemory::for_profile(&AI_1_0_TECH),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::ResearchComplex
    }));
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Steelworks
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
}

#[test]
fn ai_1_1_trains_bounded_machine_gunners_without_scout_cars() {
    let observation = observation(
        AiEconomy {
            steel: 800,
            oil: 300,
            supply_used: 30,
            supply_cap: 80,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, Some(0)),
            building(13, EntityKind::ResearchComplex, None),
            building(14, EntityKind::Factory, Some(0)),
            combat(30, EntityKind::MachineGunner),
            combat(31, EntityKind::MachineGunner),
        ],
    );

    let decision = decide(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::ScoutCar
    }));
    assert!(!decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Train {
                unit: EntityKind::ScoutCar,
                ..
            }
        )
    }));
}

#[test]
fn ai_1_1_stops_machine_gunner_training_at_perimeter_target() {
    let observation = observation(
        AiEconomy {
            steel: 800,
            oil: 300,
            supply_used: 30,
            supply_cap: 80,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, Some(0)),
            building(13, EntityKind::ResearchComplex, None),
            building(14, EntityKind::Factory, Some(0)),
            combat(30, EntityKind::MachineGunner),
            combat(31, EntityKind::MachineGunner),
            combat(32, EntityKind::MachineGunner),
            combat(33, EntityKind::MachineGunner),
        ],
    );

    let decision = decide(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
    );

    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::ScoutCar
    }));
}

#[test]
fn ai_1_1_reserves_machine_gunner_perimeter_from_tank_wave() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = with_enemy_main_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 54,
            supply_cap: 100,
        },
        vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::ResearchComplex, None),
            building(14, EntityKind::Factory, Some(0)),
            combat_at(30, EntityKind::MachineGunner, 8.5 * ts, 8.5 * ts),
            combat_at(31, EntityKind::MachineGunner, 9.0 * ts, 8.5 * ts),
            combat_at(32, EntityKind::MachineGunner, 9.5 * ts, 8.5 * ts),
            combat_at(33, EntityKind::MachineGunner, 10.0 * ts, 8.5 * ts),
            combat_at(40, EntityKind::Tank, 13.0 * ts, 13.0 * ts),
            combat_at(41, EntityKind::Tank, 13.5 * ts, 13.0 * ts),
            combat_at(42, EntityKind::Tank, 14.0 * ts, 13.0 * ts),
            combat_at(43, EntityKind::Rifleman, 14.5 * ts, 13.0 * ts),
            combat_at(44, EntityKind::Rifleman, 15.0 * ts, 13.0 * ts),
            combat_at(45, EntityKind::Rifleman, 15.5 * ts, 13.0 * ts),
        ],
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
    );

    let perimeter_targets: Vec<(u32, f32, f32)> = decision
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::AttackMove { units, x, y, .. }
                if units.len() == 1 && (30..=33).contains(&units[0]) =>
            {
                Some((units[0], *x, *y))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        perimeter_targets
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>(),
        vec![30, 31, 32, 33]
    );
    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::AttackMove { units, .. } if units.as_slice() == [40, 41, 42]
        )
    }));
    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::AttackMove { units, .. } if units.iter().any(|id| (30..=33).contains(id)) && units.len() > 1
            )
        }),
        "reserved Machine Gunners should not join the Tank frontal wave"
    );

    let steel_center =
        main_steel_cluster_center(&observation).expect("main steel cluster should be found");
    let enemy = AiFacts::from_observation(&observation)
        .nearest_public_enemy_base
        .expect("enemy base should be public");
    let dir = normalized_direction(steel_center, (enemy.x, enemy.y))
        .expect("enemy should not overlap the main steel");
    let perp = (-dir.1, dir.0);
    let mut lateral_offsets = Vec::new();
    for (_, x, y) in &perimeter_targets {
        let dx = *x - steel_center.0;
        let dy = *y - steel_center.1;
        let front_tiles = (dx * dir.0 + dy * dir.1) / ts;
        assert!(
            ((DEFENSIVE_MG_PERIMETER_DISTANCE_TILES - 1.0)
                ..=(DEFENSIVE_MG_PERIMETER_DISTANCE_TILES + 1.0))
                .contains(&front_tiles),
            "perimeter point should be in front of the steel patch, got {front_tiles} tiles"
        );
        lateral_offsets.push((dx * perp.0 + dy * perp.1) / ts);
    }
    lateral_offsets.sort_by(|left, right| left.total_cmp(right));
    let spread = lateral_offsets.last().unwrap() - lateral_offsets.first().unwrap();
    assert!(
        spread >= 4.0,
        "Machine Gunners should spread across the perimeter, got {spread} tiles"
    );
}

#[test]
fn ai_1_1_local_defense_pulls_perimeter_machine_gunners() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = with_enemy_main_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 40,
            supply_cap: 80,
        },
        vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            combat_at(30, EntityKind::MachineGunner, 8.5 * ts, 8.5 * ts),
            combat_at(31, EntityKind::MachineGunner, 9.0 * ts, 8.5 * ts),
        ],
    ));
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));

    let decision = decide(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
    );

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. } if units.as_slice() == [30, 31] && *target == 90
        )
    }));
    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::AttackMove { units, .. } if units.iter().any(|id| [30, 31].contains(id))
            )
        }),
        "local defense should take priority over passive perimeter staging"
    );
}

#[test]
fn ai_1_1_tank_transition_does_not_train_riflemen() {
    let mut observation = observation(
        AiEconomy {
            steel: 600,
            oil: 300,
            supply_used: 54,
            supply_cap: 120,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::ResearchComplex, None),
            building(14, EntityKind::Factory, Some(0)),
            combat(30, EntityKind::MachineGunner),
            combat(31, EntityKind::MachineGunner),
            combat(32, EntityKind::MachineGunner),
            combat(33, EntityKind::MachineGunner),
        ],
    );
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
}

#[test]
fn ai_1_1_and_1_2_float_resources_into_tank_tech_below_old_supply_gate() {
    for profile in [&AI_1_1_TANK_MG, &AI_1_2_WAVE_COHORTS] {
        let observation = observation(
            AiEconomy {
                steel: 450,
                oil: 200,
                supply_used: 12,
                supply_cap: 30,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                worker(20, AiEntityState::Idle),
            ],
        );

        let decision = decide(
            &observation,
            profile,
            &mut AiDecisionMemory::for_profile(profile),
        );

        assert!(
            decision.intents.contains(&AiIntent::Build {
                kind: EntityKind::TrainingCentre
            }),
            "{} should start tank tech from a floated resource bank even below 30 supply",
            profile.id
        );
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }));
    }
}

#[test]
fn high_supply_without_resource_float_does_not_start_ai_1_1_tank_transition() {
    let observation = observation(
        AiEconomy {
            steel: 399,
            oil: 149,
            supply_used: 54,
            supply_cap: 80,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            worker(20, AiEntityState::Idle),
        ],
    );

    let decision = decide(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
    );

    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::TrainingCentre
    }));
}

#[test]
fn ai_1_0_can_expand_while_transitioning_to_vehicles() {
    let mut owned = vec![
        building(10, EntityKind::CityCentre, Some(0)),
        building(11, EntityKind::Barracks, Some(0)),
        building(12, EntityKind::TrainingCentre, None),
    ];
    owned.extend((0..4).map(|i| combat(30 + i, EntityKind::Rifleman)));
    owned.push(worker(80, AiEntityState::Idle));
    owned.push(worker(81, AiEntityState::Idle));
    owned.push(worker(82, AiEntityState::Idle));
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 30,
            supply_cap: 70,
        },
        owned,
    ));

    let decision = decide(
        &observation,
        &AI_1_0_TECH,
        &mut AiDecisionMemory::for_profile(&AI_1_0_TECH),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::CityCentre
    }));
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::ResearchComplex
    }));
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
}

#[test]
fn full_saturation_pivots_to_tank_tech_but_waits_for_full_steel_before_oil() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
    ];
    owned.extend((0..17).map(|i| steel_worker(20 + i, 100 + i)));
    owned.extend((0..40).map(|i| combat(200 + i, EntityKind::Rifleman)));
    owned.extend((0..2).map(|i| worker(300 + i, AiEntityState::Idle)));
    let mut observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 50,
            supply_cap: 70,
        },
        owned,
    );
    let facts = AiFacts::from_observation(&observation);
    let target_steel_workers = target_steel_workers_for_profile(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        RIFLE_FLOOD_FULL_SATURATION
            .workers
            .target_steel_workers(facts.target_steel_workers, usize::MAX),
    );
    let desired_oil = desired_oil_workers(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        target_steel_workers,
    );

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::TrainingCentre
    }));
    assert_eq!(target_steel_workers, 18);
    assert_eq!(desired_oil, 0);

    observation.owned.push(steel_worker(37, 117));
    let facts = AiFacts::from_observation(&observation);
    let target_steel_workers = target_steel_workers_for_profile(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        RIFLE_FLOOD_FULL_SATURATION
            .workers
            .target_steel_workers(facts.target_steel_workers, usize::MAX),
    );
    let desired_oil = desired_oil_workers(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        target_steel_workers,
    );
    assert_eq!(desired_oil, 6);
}

#[test]
fn full_saturation_oil_timing_tracks_observed_steel_patch_count() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
    ];
    owned.extend((0..18).map(|i| steel_worker(20 + i, 100 + i)));
    owned.push(worker(300, AiEntityState::Idle));
    let mut observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 50,
            supply_cap: 70,
        },
        owned,
    );
    observation
        .resources
        .push(resource(118, EntityKind::Steel, 13.5 * ts, 11.5 * ts));

    let facts = AiFacts::from_observation(&observation);
    let target_steel_workers = target_steel_workers_for_profile(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        RIFLE_FLOOD_FULL_SATURATION
            .workers
            .target_steel_workers(facts.target_steel_workers, usize::MAX),
    );
    let desired_oil = desired_oil_workers(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        target_steel_workers,
    );

    assert_eq!(target_steel_workers, 19);
    assert_eq!(desired_oil, 0);
}

#[test]
fn full_saturation_assigns_oil_before_steel_saturation_when_expansion_complete() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
        building(12, EntityKind::Barracks, Some(0)),
        building(13, EntityKind::TrainingCentre, Some(0)),
    ];
    // 15 steel workers — below target saturation of 36 (18 main + 18 expansion)
    owned.extend((0..15).map(|i| steel_worker(20 + i, 100 + i)));
    owned.push(worker(300, AiEntityState::Idle));
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 0,
            supply_used: 35,
            supply_cap: 80,
        },
        owned,
    ));
    let facts = AiFacts::from_observation(&observation);
    let target_steel_workers = target_steel_workers_for_profile(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        RIFLE_FLOOD_FULL_SATURATION
            .workers
            .target_steel_workers(facts.target_steel_workers, usize::MAX),
    );
    let desired_oil = desired_oil_workers(
        &observation,
        &facts,
        &RIFLE_FLOOD_FULL_SATURATION,
        false,
        target_steel_workers,
    );
    assert!(
        desired_oil > 0,
        "full saturation should want oil workers at expansion even before steel saturation; target_steel={target_steel_workers}, desired_oil={desired_oil}"
    );
}

#[test]
fn full_saturation_can_expand_while_teching_to_tanks() {
    let mut owned = vec![
        building(10, EntityKind::CityCentre, Some(0)),
        building(11, EntityKind::Barracks, Some(0)),
        building(12, EntityKind::TrainingCentre, Some(0)),
    ];
    owned.extend((0..18).map(|i| steel_worker(20 + i, 100 + i)));
    owned.extend((0..29).map(|i| combat(200 + i, EntityKind::Rifleman)));
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 2_000,
            oil: 2_000,
            supply_used: 50,
            supply_cap: 70,
        },
        owned,
    ));

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::CityCentre
    }));
}

#[test]
fn steel_expansion_tanks_builds_expansion_cc_before_any_barracks() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        8.5 * ts,
        8.5 * ts,
    )];
    owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
    owned.push(worker(60, AiEntityState::Idle));
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 12,
            supply_cap: 30,
        },
        owned,
    ));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::CityCentre
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::TrainingCentre
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
    let non_depot_builds: Vec<_> = decision
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::Build { building, .. } if *building != EntityKind::Depot => Some(*building),
            _ => None,
        })
        .collect();
    assert_eq!(
        non_depot_builds,
        vec![EntityKind::CityCentre],
        "the first non-depot build should be the expansion City Centre"
    );
    assert!(
        !decision.intents.iter().any(|intent| matches!(
            intent,
            AiIntent::Gather {
                resource: EntityKind::Oil,
                ..
            }
        )),
        "expansion profile should not move into oil before the second City Centre is planned"
    );
}

#[test]
fn steel_expansion_tanks_places_expansion_cc_in_range_of_whole_resource_line() {
    let map_size = 96;
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        10.5 * ts,
        85.5 * ts,
    )];
    owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
    owned.push(worker(60, AiEntityState::Idle));
    let mut resources = base_site_resources(100, (10, 85), map_size);
    resources.extend(base_site_resources(200, (48, 73), map_size));
    resources.sort_by_key(|resource| resource.id);
    let observation = AiObservation {
        player_id: 1,
        tick: 90,
        map: AiMapSummary {
            width: map_size,
            height: map_size,
            tile_size: config::TILE_SIZE,
        },
        economy: AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 12,
            supply_cap: 30,
        },
        own_start_tile: (10, 85),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (10, 85),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (85, 10),
                is_ai: false,
                is_alive: true,
            },
        ],
        owned,
        resources,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    };

    let mut placeable = |_: EntityKind, tx: u32, ty: u32| tx < map_size && ty < map_size;
    let site = expansion_city_centre_site(
        &observation,
        STEEL_EXPANSION_TANKS.expansion.unwrap(),
        EntityKind::CityCentre,
        &mut placeable,
    )
    .expect("expansion City Centre site should be found");

    assert_eq!(
        expansion_resource_counts_for_site(&observation, site),
        (
            config::STEEL_PATCHES_PER_BASE as usize,
            config::OIL_PATCHES_PER_BASE as usize
        ),
        "expansion City Centre at {site:?} should cover the whole natural resource line"
    );

    let mut memory = AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS);
    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, tx, ty| tx < map_size && ty < map_size,
    );
    let command_site = decision
        .commands
        .iter()
        .find_map(|command| match command {
            Command::Build {
                building,
                tile_x,
                tile_y,
                ..
            } if *building == EntityKind::CityCentre => Some((*tile_x, *tile_y)),
            _ => None,
        })
        .expect("decision should issue an expansion City Centre build");

    assert_eq!(
            expansion_resource_counts_for_site(&observation, command_site),
            (
                config::STEEL_PATCHES_PER_BASE as usize,
                config::OIL_PATCHES_PER_BASE as usize
            ),
            "emitted expansion City Centre build at {command_site:?} should cover all expansion resources"
        );
}

#[test]
fn steel_expansion_tanks_prefers_closer_natural_resource_cluster() {
    let map_size = 96;
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        10.5 * ts,
        85.5 * ts,
    )];
    owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
    owned.push(worker(60, AiEntityState::Idle));
    let mut resources = base_site_resources(100, (10, 85), map_size);
    resources.extend(base_site_resources(200, (23, 47), map_size));
    resources.extend(base_site_resources(300, (48, 73), map_size));
    resources.sort_by_key(|resource| resource.id);
    let observation = AiObservation {
        player_id: 1,
        tick: 90,
        map: AiMapSummary {
            width: map_size,
            height: map_size,
            tile_size: config::TILE_SIZE,
        },
        economy: AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 12,
            supply_cap: 30,
        },
        own_start_tile: (10, 85),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (10, 85),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (85, 10),
                is_ai: false,
                is_alive: true,
            },
        ],
        owned,
        resources,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    };

    let mut placeable = |_: EntityKind, tx: u32, ty: u32| tx < map_size && ty < map_size;
    let site = expansion_city_centre_site(
        &observation,
        STEEL_EXPANSION_TANKS.expansion.unwrap(),
        EntityKind::CityCentre,
        &mut placeable,
    )
    .expect("expansion City Centre site should be found");
    let center = building_center(site, EntityKind::CityCentre, observation.map.tile_size)
        .expect("city centre should have a center");
    let closer_natural = tile_center((23, 47), observation.map.tile_size);
    let farther_natural = tile_center((48, 73), observation.map.tile_size);

    assert!(
        dist2(center.0, center.1, closer_natural.0, closer_natural.1)
            < dist2(center.0, center.1, farther_natural.0, farther_natural.1),
        "expansion City Centre at {site:?} should choose the closer natural cluster"
    );
    assert_eq!(
        expansion_resource_counts_for_site(&observation, site),
        (
            config::STEEL_PATCHES_PER_BASE as usize,
            config::OIL_PATCHES_PER_BASE as usize
        ),
        "chosen closer natural should still cover its whole resource line"
    );
}

#[test]
fn expansion_site_selection_prefers_oil_over_steel_only_output() {
    let map_size = 96;
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        10.5 * ts,
        85.5 * ts,
    )];
    owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
    let mut resources = base_site_resources(100, (10, 85), map_size);
    resources.extend(
        base_site_resources(200, (22, 75), map_size)
            .into_iter()
            .filter(|resource| resource.kind == EntityKind::Steel),
    );
    resources.extend(base_site_resources(300, (55, 55), map_size));
    resources.sort_by_key(|resource| resource.id);
    let observation = AiObservation {
        player_id: 1,
        tick: 90,
        map: AiMapSummary {
            width: map_size,
            height: map_size,
            tile_size: config::TILE_SIZE,
        },
        economy: AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 70,
            supply_cap: 80,
        },
        own_start_tile: (10, 85),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (10, 85),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (86, 10),
                is_ai: false,
                is_alive: true,
            },
        ],
        owned,
        resources,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    };
    let expansion = STEEL_EXPANSION_TANKS.expansion.unwrap();

    let site = expansion_city_centre_site(
        &observation,
        expansion,
        EntityKind::CityCentre,
        &mut |_, _, _| true,
    )
    .expect("oil-bearing expansion site should be found");

    let (_, oil) = expansion_resource_counts_for_site(&observation, site);
    assert_eq!(oil, config::OIL_PATCHES_PER_BASE as usize);
}

#[test]
fn expansion_site_selection_filters_resource_range_before_placeable() {
    let map_size = 96;
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 70,
            supply_cap: 80,
        },
        vec![building(10, EntityKind::CityCentre, Some(0))],
    );
    observation.map.width = map_size;
    observation.map.height = map_size;
    observation.own_start_tile = (8, 8);
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
            start_tile: (88, 88),
            is_ai: false,
            is_alive: true,
        },
    ];
    observation.resources = vec![resource(300, EntityKind::Steel, 40.5 * ts, 40.5 * ts)];

    let expansion = RIFLE_FLOOD_FULL_SATURATION.expansion.unwrap();
    let full_search_window = (expansion.search_radius_tiles * 2 + 1).pow(2) as usize;
    let mut placeable_calls = 0usize;
    let site = expansion_city_centre_site(
        &observation,
        expansion,
        EntityKind::CityCentre,
        &mut |_, _, _| {
            placeable_calls += 1;
            true
        },
    )
    .expect("single-patch expansion site should be found");

    assert_eq!(
        expansion_resource_counts_for_site(&observation, site),
        (1, 0)
    );
    assert!(
        placeable_calls > 0,
        "resource-qualified candidates should still be checked for placement"
    );
    assert!(
        placeable_calls < full_search_window,
        "resource range filtering should avoid checking all {full_search_window} search tiles for placement; checked {placeable_calls}"
    );
}

#[test]
fn steel_expansion_tanks_prefers_safer_natural_when_distances_are_similar() {
    let map_size = 96;
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        10.5 * ts,
        85.5 * ts,
    )];
    owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
    owned.push(worker(60, AiEntityState::Idle));
    let mut resources = base_site_resources(100, (10, 85), map_size);
    resources.extend(base_site_resources(200, (23, 47), map_size));
    resources.extend(base_site_resources(300, (48, 73), map_size));
    resources.sort_by_key(|resource| resource.id);
    let observation = AiObservation {
        player_id: 1,
        tick: 90,
        map: AiMapSummary {
            width: map_size,
            height: map_size,
            tile_size: config::TILE_SIZE,
        },
        economy: AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 12,
            supply_cap: 30,
        },
        own_start_tile: (10, 85),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (10, 85),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (85, 85),
                is_ai: false,
                is_alive: true,
            },
        ],
        owned,
        resources,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    };

    let mut placeable = |_: EntityKind, tx: u32, ty: u32| tx < map_size && ty < map_size;
    let site = expansion_city_centre_site(
        &observation,
        STEEL_EXPANSION_TANKS.expansion.unwrap(),
        EntityKind::CityCentre,
        &mut placeable,
    )
    .expect("expansion City Centre site should be found");
    let center = building_center(site, EntityKind::CityCentre, observation.map.tile_size)
        .expect("city centre should have a center");
    let safer_natural = tile_center((23, 47), observation.map.tile_size);
    let exposed_natural = tile_center((48, 73), observation.map.tile_size);

    assert!(
        dist2(center.0, center.1, safer_natural.0, safer_natural.1)
            < dist2(center.0, center.1, exposed_natural.0, exposed_natural.1),
        "expansion City Centre at {site:?} should choose the natural farther from the enemy start"
    );
    assert_eq!(
        expansion_resource_counts_for_site(&observation, site),
        (
            config::STEEL_PATCHES_PER_BASE as usize,
            config::OIL_PATCHES_PER_BASE as usize
        ),
        "chosen safer natural should still cover its whole resource line"
    );
}

#[test]
fn steel_expansion_tanks_builds_barracks_after_expansion_cc_is_planned() {
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 10,
            supply_cap: 30,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            worker(60, AiEntityState::Idle),
        ],
    ));
    observation
        .pending_builds
        .push(AiBuildIntent::to_site(60, EntityKind::CityCentre, 20, 30));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::CityCentre
    }));
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::TrainingCentre
    }));
}

#[test]
fn steel_expansion_tanks_builds_training_centre_before_training_support_units() {
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 10,
            supply_cap: 30,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            worker(60, AiEntityState::Idle),
        ],
    ));
    observation
        .pending_builds
        .push(AiBuildIntent::to_site(60, EntityKind::CityCentre, 20, 30));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::TrainingCentre
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
}

#[test]
fn steel_expansion_tanks_builds_steelworks_before_training_anti_tank_guns() {
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 10,
            supply_cap: 40,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::Barracks, Some(0)),
            building(14, EntityKind::TrainingCentre, None),
            worker(60, AiEntityState::Idle),
        ],
    ));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::ResearchComplex
    }));
    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
}

#[test]
fn steel_expansion_tanks_researches_anti_tank_gun_crews_before_training_anti_tank_guns() {
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 10,
            supply_cap: 40,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::Barracks, Some(0)),
            building(14, EntityKind::TrainingCentre, None),
            building(15, EntityKind::ResearchComplex, Some(0)),
            building(16, EntityKind::Steelworks, Some(0)),
            worker(60, AiEntityState::Idle),
        ],
    ));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Research {
        upgrade: UpgradeKind::AntiTankGunUnlock
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
}

#[test]
fn steel_expansion_tanks_balances_machine_gunner_and_anti_tank_gun_training() {
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 10,
            supply_cap: 40,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::Barracks, Some(0)),
            building(14, EntityKind::TrainingCentre, None),
            building(15, EntityKind::Steelworks, Some(0)),
            worker(60, AiEntityState::Idle),
        ],
    ));
    observation.upgrades.push(UpgradeKind::AntiTankGunUnlock);

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );
    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
}

#[test]
fn steel_expansion_tanks_counts_queued_machine_gunners_when_balancing_support() {
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 14,
            supply_cap: 50,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building_training(12, EntityKind::Barracks, EntityKind::MachineGunner),
            building(13, EntityKind::Barracks, Some(0)),
            building(15, EntityKind::TrainingCentre, None),
            building(16, EntityKind::Steelworks, Some(0)),
        ],
    ));
    observation.upgrades.push(UpgradeKind::AntiTankGunUnlock);

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
}

#[test]
fn steel_expansion_tanks_sends_workers_to_oil_after_expansion_is_planned() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 550,
            oil: 0,
            supply_used: 17,
            supply_cap: 40,
        },
        {
            let mut owned = vec![building_at(
                10,
                EntityKind::CityCentre,
                Some(0),
                8.5 * ts,
                8.5 * ts,
            )];
            owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
            owned.extend((0..6).map(|i| worker(60 + i, AiEntityState::Idle)));
            owned
        },
    ));
    observation
        .pending_builds
        .push(AiBuildIntent::to_site(60, EntityKind::CityCentre, 20, 30));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    let oil_assignments = decision
        .intents
        .iter()
        .filter_map(|intent| match intent {
            AiIntent::Gather {
                resource: EntityKind::Oil,
                assignments,
            } => Some(*assignments),
            _ => None,
        })
        .sum::<usize>();
    assert_eq!(
        oil_assignments, 3,
        "support tech should fill the three mineable oil nodes once expanding"
    );
}

#[test]
fn steel_expansion_tanks_keeps_main_workers_off_distant_expansion_steel() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
    ];
    owned.extend((0..18u32).map(|i| gathering_worker(40 + i, 100 + i)));
    owned.extend((0..6u32).map(|i| {
        let node = if i < 3 { 200 + i } else { 400 + (i - 3) };
        gathering_worker(70 + i, node)
    }));
    owned.push(worker_at(90, AiEntityState::Idle, 8.5 * ts, 8.5 * ts));
    owned.push(worker_at(91, AiEntityState::Idle, 9.5 * ts, 8.5 * ts));
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 26,
            supply_cap: 80,
        },
        owned,
    ));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(
        !decision.commands.iter().any(|command| {
            matches!(command, Command::Gather { node, .. } if (300..318).contains(node))
        }),
        "main-base idle workers should not be sent to expansion steel patches"
    );
}

#[test]
fn ai_1_1_sends_main_idle_worker_to_expansion_steel_when_main_is_saturated() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
        building(12, EntityKind::Barracks, Some(0)),
        building(13, EntityKind::TrainingCentre, None),
        building(14, EntityKind::ResearchComplex, Some(0)),
        building(15, EntityKind::Factory, Some(0)),
        building(16, EntityKind::Factory, Some(0)),
        building(17, EntityKind::Steelworks, Some(0)),
    ];
    owned.extend((0..18u32).map(|i| gathering_worker(40 + i, 100 + i)));
    owned.push(worker_at(90, AiEntityState::Idle, 8.5 * ts, 8.5 * ts));
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 40,
            supply_cap: 80,
        },
        owned,
    ));
    observation
        .resources
        .retain(|resource| resource.kind == EntityKind::Steel);

    let decision = decide(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
    );

    assert!(
        decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Gather { units, node, .. }
                    if units.as_slice() == [90] && (300..318).contains(node)
            )
        }),
        "AI 1.1 should use expansion steel when the main steel line is fully saturated"
    );
}

#[test]
fn steel_expansion_tanks_sends_expansion_workers_to_expansion_steel() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
    ];
    owned.extend((0..18u32).map(|i| gathering_worker(40 + i, 100 + i)));
    owned.extend((0..6u32).map(|i| {
        let node = if i < 3 { 200 + i } else { 400 + (i - 3) };
        gathering_worker(70 + i, node)
    }));
    owned.push(worker_at(90, AiEntityState::Idle, 23.5 * ts, 36.5 * ts));
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 25,
            supply_cap: 80,
        },
        owned,
    ));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(
        decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Gather { units, node, .. }
                    if units.as_slice() == [90] && (300..318).contains(node)
            )
        }),
        "an idle expansion worker should take a local expansion steel patch"
    );
}

#[test]
fn steel_expansion_tanks_stages_support_weapons_on_enemy_facing_main_steel_line() {
    let ts = config::TILE_SIZE as f32;
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 24,
            supply_cap: 80,
        },
        vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            combat_at(30, EntityKind::MachineGunner, 8.5 * ts, 8.5 * ts),
            combat_at(31, EntityKind::AntiTankGun, 9.5 * ts, 8.5 * ts),
            combat_at(32, EntityKind::MachineGunner, 10.5 * ts, 8.5 * ts),
        ],
    ));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    let stage_targets: Vec<(u32, f32, f32)> = decision
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::AttackMove { units, x, y, .. } if units.len() == 1 => Some((units[0], *x, *y)),
            _ => None,
        })
        .collect();
    assert_eq!(
        stage_targets
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>(),
        vec![30, 31, 32],
        "support weapons should receive deterministic individual stage slots"
    );

    let steel_center =
        main_steel_cluster_center(&observation).expect("main steel cluster should be found");
    let enemy = AiFacts::from_observation(&observation)
        .nearest_public_enemy_base
        .expect("enemy base should be public");
    let dir = normalized_direction(steel_center, (enemy.x, enemy.y))
        .expect("enemy should not overlap the main steel");
    let perp = (-dir.1, dir.0);
    let mut lateral_offsets = Vec::new();
    for (_, x, y) in &stage_targets {
        let dx = *x - steel_center.0;
        let dy = *y - steel_center.1;
        let front_tiles = (dx * dir.0 + dy * dir.1) / ts;
        assert!(
            (2.0..=4.0).contains(&front_tiles),
            "stage point should be in front of the steel patch, got {front_tiles} tiles"
        );
        lateral_offsets.push((dx * perp.0 + dy * perp.1) / ts);
    }
    lateral_offsets.sort_by(|left, right| left.total_cmp(right));
    let spread = lateral_offsets.last().unwrap() - lateral_offsets.first().unwrap();
    assert!(
        spread >= 2.5,
        "support weapons should spread across a line, got {spread} tiles"
    );
}

#[test]
fn steel_expansion_tanks_switches_to_factory_at_resource_float() {
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 500,
            supply_used: 50,
            supply_cap: 130,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::Barracks, Some(0)),
            building(14, EntityKind::TrainingCentre, None),
            building(15, EntityKind::ResearchComplex, None),
            worker(60, AiEntityState::Idle),
        ],
    ));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
}

#[test]
fn steel_expansion_tanks_trains_only_tanks_after_resource_float() {
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 500,
            oil: 300,
            supply_used: 50,
            supply_cap: 130,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::Factory, Some(0)),
            building(15, EntityKind::Steelworks, None),
        ],
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
}

#[test]
fn steel_expansion_tanks_attacks_with_three_or_more_tanks_after_transition() {
    let two_tanks = with_expansion_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 50,
            supply_cap: 130,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::Factory, Some(0)),
            combat(30, EntityKind::Tank),
            combat(31, EntityKind::Tank),
        ],
    ));
    let mut two_tanks = two_tanks;
    two_tanks.upgrades.push(UpgradeKind::Methamphetamines);
    let two_tank_decision = decide(
        &two_tanks,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(
        !two_tank_decision
            .intents
            .iter()
            .any(|intent| matches!(intent, AiIntent::Attack { .. })),
        "two tanks should not launch an outbound attack"
    );

    let three_tanks = with_expansion_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 50,
            supply_cap: 130,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::Factory, Some(0)),
            combat(30, EntityKind::Tank),
            combat(31, EntityKind::Tank),
            combat(32, EntityKind::Tank),
            combat(40, EntityKind::MachineGunner),
            combat(41, EntityKind::AntiTankGun),
        ],
    ));
    let mut three_tanks = three_tanks;
    three_tanks.upgrades.push(UpgradeKind::Methamphetamines);
    let three_tank_decision = decide(
        &three_tanks,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(three_tank_decision.intents.iter().any(|intent| {
        matches!(
            intent,
            AiIntent::Attack { units } if units.as_slice() == [30, 31, 32]
        )
    }));
    assert!(
        three_tank_decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::AttackMove { units, .. } if units.as_slice() == [30, 31, 32]
            )
        }),
        "three ready tanks should attack as a group"
    );
}

#[test]
fn tank_wave_waits_for_methamphetamines_before_attacking() {
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 50,
            supply_cap: 130,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::Factory, Some(0)),
            combat(30, EntityKind::Tank),
            combat(31, EntityKind::Tank),
            combat(32, EntityKind::Tank),
        ],
    ));

    let decision = decide(
        &observation,
        &STEEL_EXPANSION_TANKS,
        &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
    );

    assert!(
        !decision
            .intents
            .iter()
            .any(|intent| matches!(intent, AiIntent::Attack { .. })),
        "tank frontal waves should not launch before Methamphetamines"
    );
    assert_eq!(
        trace_goal(&decision, trace::StrategicGoal::FrontalAttack).blockers,
        vec![
            trace::GoalBlocker::WaitingForMethamphetamines,
            trace::GoalBlocker::Staging,
        ]
    );
    assert!(decision.intents.iter().any(|intent| {
        matches!(
            intent,
            AiIntent::Stage { units } if units.as_slice() == [30, 31, 32]
        )
    }));
}

#[test]
fn frontal_wave_readiness_reports_required_tank_gate() {
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    owned.extend((0..6).map(|i| combat(30 + i, EntityKind::Rifleman)));
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 6,
            supply_cap: 20,
        },
        owned,
    );
    let mut memory = AiDecisionMemory::for_profile(&TECH_TO_TANKS);

    let plan = frontal::plan_frontal_wave(
        &observation,
        TECH_TO_TANKS.attack,
        &mut memory,
        &TECH_TO_TANKS,
        &BTreeSet::new(),
    );

    assert_eq!(
        plan.blockers,
        vec![
            frontal::FrontalWaveBlocker::WaitingForTank,
            frontal::FrontalWaveBlocker::WaitingForMethamphetamines,
        ]
    );
}

#[test]
fn full_saturation_prioritizes_second_city_centre_before_resource_float() {
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: rts_rules::economy::cost(EntityKind::CityCentre).0,
            oil: 150,
            supply_used: 50,
            supply_cap: 100,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::ResearchComplex, None),
            worker(60, AiEntityState::Idle),
        ],
    ));

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::CityCentre
    }));
    assert!(
        !decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Factory
        }),
        "the first resource-float macro spend should not let Factory preempt the expansion"
    );
}

#[test]
fn full_saturation_builds_factory_after_expansion_is_planned() {
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 400,
            oil: 150,
            supply_used: 50,
            supply_cap: 100,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::ResearchComplex, None),
            worker(60, AiEntityState::Idle),
        ],
    ));
    observation
        .pending_builds
        .push(AiBuildIntent::to_site(60, EntityKind::CityCentre, 20, 30));

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::CityCentre
    }));
}

#[test]
fn full_saturation_trains_tanks_after_tech_transition_completes() {
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 450,
            oil: 150,
            supply_used: 50,
            supply_cap: 100,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::ResearchComplex, None),
            building(15, EntityKind::Factory, Some(0)),
            building(16, EntityKind::Steelworks, None),
        ],
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
}

#[test]
fn tech_to_tanks_trains_tank_before_spending_barracks_budget() {
    let mut observation = observation(
        AiEconomy {
            steel: 450,
            oil: 150,
            supply_used: 4,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::ResearchComplex, None),
            building(14, EntityKind::Factory, Some(0)),
            building(15, EntityKind::Steelworks, Some(0)),
            worker(20, AiEntityState::Gather),
            worker(21, AiEntityState::Gather),
            worker(22, AiEntityState::Gather),
            worker(23, AiEntityState::Gather),
        ],
    );
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(
        &observation,
        &TECH_TO_TANKS,
        &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
}

#[test]
fn infantry_heavy_home_threat_prefers_machine_gunners_before_tanks() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 200,
            oil: 150,
            supply_used: 4,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::Factory, Some(0)),
            building(14, EntityKind::Steelworks, Some(0)),
            worker(20, AiEntityState::Gather),
            worker(21, AiEntityState::Gather),
            worker(22, AiEntityState::Gather),
            worker(23, AiEntityState::Gather),
        ],
    );
    observation.upgrades.push(UpgradeKind::AntiTankGunUnlock);
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));
    observation
        .visible_enemies
        .push(enemy(91, EntityKind::Rifleman, 10.5 * ts, 11.5 * ts));
    observation
        .visible_enemies
        .push(enemy(92, EntityKind::Rifleman, 11.5 * ts, 10.5 * ts));

    let decision = decide(
        &observation,
        &TECH_TO_TANKS,
        &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::MachineGunner
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
}

#[test]
fn lone_rifle_near_base_does_not_trigger_defensive_panic() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 8,
            supply_cap: 30,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            worker(20, AiEntityState::Gather),
            worker(21, AiEntityState::Gather),
        ],
    );
    observation.upgrades.push(UpgradeKind::AntiTankGunUnlock);
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));

    let decision = decide(
        &observation,
        &TECH_TO_TANKS,
        &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::TrainingCentre
    }));
}

#[test]
fn armor_heavy_home_threat_prefers_anti_tank_guns_before_tanks() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 200,
            oil: 150,
            supply_used: 4,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::Factory, Some(0)),
            building(14, EntityKind::Steelworks, Some(0)),
            worker(20, AiEntityState::Gather),
            worker(21, AiEntityState::Gather),
            worker(22, AiEntityState::Gather),
            worker(23, AiEntityState::Gather),
        ],
    );
    observation.upgrades.push(UpgradeKind::AntiTankGunUnlock);
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Tank, 10.5 * ts, 10.5 * ts));

    let decision = decide(
        &observation,
        &TECH_TO_TANKS,
        &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
    );

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::AntiTankGun
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Tank
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
}

#[test]
fn sustained_support_panic_falls_back_to_riflemen_without_training_centre() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 8,
            supply_cap: 30,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            worker(20, AiEntityState::Gather),
            worker(21, AiEntityState::Gather),
        ],
    );
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));
    observation
        .visible_enemies
        .push(enemy(91, EntityKind::Rifleman, 10.5 * ts, 11.5 * ts));
    let mut memory = AiDecisionMemory::for_profile(&TECH_TO_TANKS);

    let first_decision = decide(&observation, &TECH_TO_TANKS, &mut memory);
    assert!(
        !first_decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Barracks
        }),
        "fresh panic should use the existing barracks before adding another one"
    );
    assert!(
        !first_decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TrainingCentre
        }),
        "panic mode must not create support tech"
    );
    assert!(first_decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
    assert!(
        !first_decision.intents.iter().any(|intent| matches!(
            intent,
            AiIntent::Gather {
                resource: EntityKind::Oil,
                ..
            }
        )),
        "support fallback should not build Pump Jacks"
    );

    let started_tick = observation.tick;
    observation.tick = started_tick.saturating_add(DEFENSIVE_PANIC_GRACE_TICKS);
    let _ = decide(&observation, &TECH_TO_TANKS, &mut memory);
    observation.tick = started_tick.saturating_add(DEFENSIVE_PANIC_SUSTAINED_TICKS);
    let sustained_decision = decide(&observation, &TECH_TO_TANKS, &mut memory);

    assert!(sustained_decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));
    assert!(
        !sustained_decision.intents.iter().any(|intent| matches!(
            intent,
            AiIntent::Build {
                kind: EntityKind::TrainingCentre | EntityKind::Factory
            }
        )),
        "panic mode should block all tech spending"
    );
    assert!(sustained_decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
}

#[test]
fn visible_home_threat_preempts_outbound_tank_attack() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 10,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::Factory, Some(0)),
            combat_at(30, EntityKind::Tank, 8.5 * ts, 8.5 * ts),
        ],
    );
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));

    let decision = decide(
        &observation,
        &TECH_TO_TANKS,
        &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
    );

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. } if *target == 90 && units == &[30]
        )
    }));
    assert!(
        !decision
            .commands
            .iter()
            .any(|command| matches!(command, Command::AttackMove { .. })),
        "local defense should preempt the outbound tank wave"
    );
}

#[test]
fn far_tank_is_not_recalled_for_home_threat() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 10,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::Factory, Some(0)),
            combat_at(30, EntityKind::Tank, 48.5 * ts, 48.5 * ts),
        ],
    );
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));

    let decision = decide(
        &observation,
        &TECH_TO_TANKS,
        &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
    );

    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target, .. } if *target == 90 && units == &[30]
            )
        }),
        "far outbound tanks should not be pulled back by local defense"
    );
    assert!(
        decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::AttackMove { units, .. } if units == &[30]
            )
        }),
        "far tanks should keep their outbound attack behavior"
    );
}

#[test]
fn ai_1_1_attacks_with_its_first_ready_tank() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 30,
            supply_cap: 60,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::ResearchComplex, None),
            building(14, EntityKind::Factory, Some(0)),
            combat_at(30, EntityKind::Tank, 18.0 * ts, 18.0 * ts),
        ],
    );
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
    );

    assert!(
        decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::AttackMove { units, .. } if units.as_slice() == [30]
            )
        }),
        "AI 1.1 should launch the first ready Tank instead of waiting for a larger Tank group"
    );
}

#[test]
fn ai_1_2_stages_forming_rifle_waves_on_a_line() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building_at(
        10,
        EntityKind::CityCentre,
        Some(0),
        8.5 * ts,
        8.5 * ts,
    )];
    owned.extend((0..3).map(|i| {
        combat_at(
            30 + i,
            EntityKind::Rifleman,
            (8.0 + i as f32 * 0.5) * ts,
            8.0 * ts,
        )
    }));
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 3,
            supply_cap: 20,
        },
        owned,
    );

    let decision = decide(
        &observation,
        &AI_1_2_WAVE_COHORTS,
        &mut AiDecisionMemory::for_profile(&AI_1_2_WAVE_COHORTS),
    );

    let stage_targets: Vec<(u32, f32, f32)> = decision
        .commands
        .iter()
        .filter_map(|command| match command {
            Command::AttackMove { units, x, y, .. } if units.len() == 1 => Some((units[0], *x, *y)),
            _ => None,
        })
        .collect();
    assert_eq!(
        stage_targets
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>(),
        vec![30, 31, 32],
        "forming AI 1.2 waves should receive deterministic individual stage slots"
    );

    let steel_center =
        main_steel_cluster_center(&observation).expect("main steel cluster should be found");
    let enemy = AiFacts::from_observation(&observation)
        .nearest_public_enemy_base
        .expect("enemy base should be public");
    let dir = normalized_direction(steel_center, (enemy.x, enemy.y))
        .expect("enemy should not overlap the main steel");
    let perp = (-dir.1, dir.0);
    let mut lateral_offsets = Vec::new();
    for (_, x, y) in &stage_targets {
        let dx = *x - steel_center.0;
        let dy = *y - steel_center.1;
        lateral_offsets.push((dx * perp.0 + dy * perp.1) / ts);
    }
    lateral_offsets.sort_by(|left, right| left.total_cmp(right));
    let spread = lateral_offsets.last().unwrap() - lateral_offsets.first().unwrap();
    assert!(
        spread >= 2.5,
        "forming AI 1.2 waves should spread across a line, got {spread} tiles"
    );
}

#[test]
fn ai_1_2_fresh_rifle_wave_does_not_count_already_launched_attackers() {
    let ts = config::TILE_SIZE as f32;
    let mut memory = AiDecisionMemory::for_profile(&AI_1_2_WAVE_COHORTS);
    let first_wave = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 4,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            combat_at(30, EntityKind::Rifleman, 8.5 * ts, 8.5 * ts),
            combat_at(31, EntityKind::Rifleman, 9.0 * ts, 8.5 * ts),
            combat_at(32, EntityKind::Rifleman, 9.5 * ts, 8.5 * ts),
            combat_at(33, EntityKind::Rifleman, 10.0 * ts, 8.5 * ts),
        ],
    );

    let first_decision = decide(&first_wave, &AI_1_2_WAVE_COHORTS, &mut memory);
    assert!(
        first_decision.commands.iter().any(|command| matches!(
            command,
            Command::AttackMove { units, .. } if units.as_slice() == [30, 31, 32, 33]
        )),
        "AI 1.2 should still launch the first four-Rifleman wave"
    );

    let mut next_wave = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 6,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            launched_combat_at(30, EntityKind::Rifleman, 14.0 * ts, 14.0 * ts),
            launched_combat_at(31, EntityKind::Rifleman, 14.5 * ts, 14.0 * ts),
            launched_combat_at(32, EntityKind::Rifleman, 15.0 * ts, 14.0 * ts),
            launched_combat_at(33, EntityKind::Rifleman, 15.5 * ts, 14.0 * ts),
            combat_at(34, EntityKind::Rifleman, 8.5 * ts, 9.0 * ts),
            combat_at(35, EntityKind::Rifleman, 9.0 * ts, 9.0 * ts),
        ],
    );
    next_wave.tick = first_wave.tick + AI_1_2_WAVE_COHORTS.attack.reissue_cadence_ticks;

    let next_decision = decide(&next_wave, &AI_1_2_WAVE_COHORTS, &mut memory);

    assert!(
        !next_decision.intents.iter().any(|intent| {
            matches!(
                intent,
                AiIntent::Attack { units } if units.iter().any(|id| [30, 31, 32, 33].contains(id))
            )
        }),
        "launched Riflemen should not satisfy the next outbound wave threshold"
    );
    assert!(
        next_decision.intents.iter().any(|intent| {
            matches!(
                intent,
                AiIntent::Stage { units } if units.as_slice() == [34, 35]
            )
        }),
        "only fresh Riflemen should stage for the next AI 1.2 wave"
    );
}

#[test]
fn ai_1_2_launched_attackers_can_still_defend_when_local() {
    let ts = config::TILE_SIZE as f32;
    let mut memory = AiDecisionMemory::for_profile(&AI_1_2_WAVE_COHORTS);
    let first_wave = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 4,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            combat_at(30, EntityKind::Rifleman, 8.5 * ts, 8.5 * ts),
            combat_at(31, EntityKind::Rifleman, 9.0 * ts, 8.5 * ts),
            combat_at(32, EntityKind::Rifleman, 9.5 * ts, 8.5 * ts),
            combat_at(33, EntityKind::Rifleman, 10.0 * ts, 8.5 * ts),
        ],
    );
    let first_decision = decide(&first_wave, &AI_1_2_WAVE_COHORTS, &mut memory);
    assert!(first_decision.intents.iter().any(|intent| {
        matches!(
            intent,
            AiIntent::Attack { units } if units.as_slice() == [30, 31, 32, 33]
        )
    }));

    let mut defense = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 4,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            launched_combat_at(30, EntityKind::Rifleman, 8.5 * ts, 8.5 * ts),
            launched_combat_at(31, EntityKind::Rifleman, 9.0 * ts, 8.5 * ts),
            launched_combat_at(32, EntityKind::Rifleman, 9.5 * ts, 8.5 * ts),
            launched_combat_at(33, EntityKind::Rifleman, 10.0 * ts, 8.5 * ts),
        ],
    );
    defense.tick = first_wave.tick + AI_1_2_WAVE_COHORTS.attack.reissue_cadence_ticks;
    defense
        .visible_enemies
        .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));

    let decision = decide(&defense, &AI_1_2_WAVE_COHORTS, &mut memory);

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. } if units.as_slice() == [30, 31, 32, 33] && *target == 90
        )
    }));
}

#[test]
fn full_saturation_rifle_wave_uses_attack_move_to_enemy_base() {
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    owned.extend((0..6).map(|i| combat(30 + i, EntityKind::Rifleman)));
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 6,
            supply_cap: 20,
        },
        owned,
    );

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(
        decision.commands.iter().any(|command| matches!(
            command,
            Command::AttackMove { units, .. } if units.as_slice() == [30, 31, 32, 33, 34, 35]
        )),
        "macro rifle waves should attack-move instead of moving past enemy armies"
    );
}

#[test]
fn full_saturation_rifle_wave_targets_visible_enemy_army() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    owned.extend((0..6).map(|i| {
        combat_at(
            30 + i,
            EntityKind::Rifleman,
            (26.0 + i as f32 * 0.2) * ts,
            28.0 * ts,
        )
    }));
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 6,
            supply_cap: 20,
        },
        owned,
    );
    observation
        .visible_enemies
        .push(enemy(80, EntityKind::Worker, 30.5 * ts, 30.5 * ts));
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Rifleman, 28.5 * ts, 28.5 * ts));

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FULL_SATURATION,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
    );

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. }
                if units.as_slice() == [30, 31, 32, 33, 34, 35] && *target == 90
        )
    }));
}

#[test]
fn moving_rifle_raid_targets_visible_workers_before_buildings() {
    let ts = config::TILE_SIZE as f32;
    let mut raider = combat_at(30, EntityKind::Rifleman, 46.0 * ts, 46.0 * ts);
    raider.state = AiEntityState::Move;
    raider.free_for_combat = false;
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
        },
        vec![building(10, EntityKind::CityCentre, Some(0)), raider],
    );
    observation
        .visible_enemies
        .push(enemy(80, EntityKind::Depot, 45.5 * ts, 45.5 * ts));
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::Worker, 48.5 * ts, 48.5 * ts));

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. } if units.as_slice() == [30] && *target == 90
        )
    }));
}

#[test]
fn moving_rifle_raid_targets_visible_scout_car() {
    let ts = config::TILE_SIZE as f32;
    let mut raider = combat_at(30, EntityKind::Rifleman, 46.0 * ts, 46.0 * ts);
    raider.state = AiEntityState::Move;
    raider.free_for_combat = false;
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
        },
        vec![building(10, EntityKind::CityCentre, Some(0)), raider],
    );
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::ScoutCar, 48.5 * ts, 48.5 * ts));

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. } if units.as_slice() == [30] && *target == 90
        )
    }));
}

#[test]
fn local_defense_does_not_block_moving_raid_from_targeting_scout_car() {
    let ts = config::TILE_SIZE as f32;
    let mut raider = combat_at(30, EntityKind::Rifleman, 46.0 * ts, 46.0 * ts);
    raider.state = AiEntityState::Move;
    raider.free_for_combat = false;
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 2,
            supply_cap: 10,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            combat_at(20, EntityKind::Rifleman, 8.5 * ts, 8.5 * ts),
            raider,
        ],
    );
    observation
        .visible_enemies
        .push(enemy(80, EntityKind::Worker, 9.5 * ts, 9.5 * ts));
    observation
        .visible_enemies
        .push(enemy(90, EntityKind::ScoutCar, 48.5 * ts, 48.5 * ts));

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. } if units.as_slice() == [20] && *target == 80
        )
    }));
    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. } if units.as_slice() == [30] && *target == 90
        )
    }));
}

#[test]
fn rifle_raid_attacks_buildings_after_reaching_enemy_main_steel_line_without_units() {
    let ts = config::TILE_SIZE as f32;
    let observation = {
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
            },
            vec![building(10, EntityKind::CityCentre, Some(0))],
        );
        observation = with_enemy_main_resources(observation);
        let enemy_base = enemy_base_fact(&observation);
        let steel_center = enemy_main_steel_center(&observation, enemy_base)
            .expect("enemy main steel should be present");
        observation.owned.push(combat_at(
            30,
            EntityKind::Rifleman,
            steel_center.0 + ts,
            steel_center.1,
        ));
        observation
            .visible_enemies
            .push(enemy(80, EntityKind::Depot, 48.5 * ts, 48.5 * ts));
        observation
    };

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Attack { units, target, .. } if units.as_slice() == [30] && *target == 80
        )
    }));
}

#[test]
fn rifle_raid_ignores_buildings_near_enemy_start_before_reaching_main_steel_line() {
    let ts = config::TILE_SIZE as f32;
    let observation = {
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                combat_at(30, EntityKind::Rifleman, 49.0 * ts, 49.0 * ts),
            ],
        );
        observation = with_enemy_main_resources(observation);
        observation
            .visible_enemies
            .push(enemy(80, EntityKind::Depot, 48.5 * ts, 48.5 * ts));
        observation
    };

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target, .. } if units.as_slice() == [30] && *target == 80
            )
        }),
        "rifle raids should not switch to buildings until they reach the enemy main steel line"
    );
}

#[test]
fn moving_rifle_raid_ignores_visible_buildings_until_arrival() {
    let ts = config::TILE_SIZE as f32;
    let mut raider = combat_at(30, EntityKind::Rifleman, 46.0 * ts, 46.0 * ts);
    raider.state = AiEntityState::Move;
    raider.free_for_combat = false;
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
        },
        vec![building(10, EntityKind::CityCentre, Some(0)), raider],
    );
    observation
        .visible_enemies
        .push(enemy(80, EntityKind::Depot, 48.5 * ts, 48.5 * ts));

    let decision = decide(
        &observation,
        &RIFLE_FLOOD_FAST,
        &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
    );

    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target, .. } if units.as_slice() == [30] && *target == 80
            )
        }),
        "moving rifle raids should keep moving past buildings"
    );
}

#[test]
fn scout_cars_join_the_normal_frontal_wave() {
    let ts = config::TILE_SIZE as f32;
    let mut owned = vec![
        building(10, EntityKind::CityCentre, Some(0)),
        building(11, EntityKind::Barracks, Some(0)),
        building(12, EntityKind::TrainingCentre, None),
        building(13, EntityKind::ResearchComplex, None),
        building(14, EntityKind::Factory, Some(0)),
        combat_at(30, EntityKind::ScoutCar, 20.0 * ts, 20.0 * ts),
        combat_at(31, EntityKind::ScoutCar, 20.5 * ts, 20.0 * ts),
        combat_at(40, EntityKind::Tank, 18.0 * ts, 18.0 * ts),
    ];
    owned.extend((0..5).map(|i| combat_at(41 + i, EntityKind::Rifleman, 18.0 * ts, 19.0 * ts)));
    let mut observation = with_enemy_main_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 50,
            supply_cap: 90,
        },
        owned,
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(
        &observation,
        &AI_1_0_TECH,
        &mut AiDecisionMemory::for_profile(&AI_1_0_TECH),
    );

    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::AttackMove { units, .. } if units.as_slice() == [30, 31, 40, 41, 42, 43, 44, 45]
        )
    }));
    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Move { units, .. } if units.contains(&30) || units.contains(&31)
            )
        }),
        "Scout Cars should not receive separate flank or evasion moves"
    );
}

#[test]
fn idle_midfield_rifle_raid_resumes_after_cleared_fight() {
    let ts = config::TILE_SIZE as f32;
    let raider = combat_at(30, EntityKind::Rifleman, 30.0 * ts, 30.0 * ts);
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
        },
        vec![building(10, EntityKind::CityCentre, Some(0)), raider],
    );
    let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
    memory.note_attack_for(
        &RIFLE_FLOOD_FAST,
        RIFLE_FLOOD_FAST.attack,
        observation.tick,
        &[30],
    );

    let decision = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

    let enemy_base = tile_center(enemy_start_tile(&observation), observation.map.tile_size);
    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Move { units, x, y, .. }
                if units.as_slice() == [30] && *x > enemy_base.0 && *y > enemy_base.1
        )
    }));
}

#[test]
fn idle_home_rifle_does_not_resume_raid_before_wave_cadence() {
    let ts = config::TILE_SIZE as f32;
    let raider = combat_at(30, EntityKind::Rifleman, 8.5 * ts, 8.5 * ts);
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
        },
        vec![building(10, EntityKind::CityCentre, Some(0)), raider],
    );
    let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
    memory.note_attack_for(
        &RIFLE_FLOOD_FAST,
        RIFLE_FLOOD_FAST.attack,
        observation.tick,
        &[30],
    );

    let decision = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Move { units, .. } if units.as_slice() == [30]
            )
        }),
        "idle riflemen at home should wait for normal attack cadence"
    );
}

#[test]
fn attack_memory_uses_profile_thresholds_and_growth() {
    let owned = vec![combat(30, EntityKind::Rifleman)];
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
        },
        owned,
    );
    let mut fast_memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
    let mut full_memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION);

    let fast = decide(&observation, &RIFLE_FLOOD_FAST, &mut fast_memory);
    let full = decide(&observation, &RIFLE_FLOOD_FULL_SATURATION, &mut full_memory);

    assert!(fast.intents.iter().any(|intent| matches!(
        intent,
        AiIntent::Attack { units } if units.as_slice() == [30]
    )));
    assert!(full.intents.iter().any(|intent| matches!(
        intent,
        AiIntent::Stage { units } if units.as_slice() == [30]
    )));
    assert_eq!(fast_memory.desired_attack_size(&RIFLE_FLOOD_FAST, 91), 1);
}

#[test]
fn turtle_rifle_opening_reports_stage_intent_for_steel_line() {
    let ts = config::TILE_SIZE as f32;
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 3,
            supply_cap: 20,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            combat_at(30, EntityKind::Rifleman, 8.5 * ts, 8.5 * ts),
            combat_at(31, EntityKind::Rifleman, 9.0 * ts, 8.5 * ts),
            combat_at(32, EntityKind::Rifleman, 9.5 * ts, 8.5 * ts),
        ],
    );

    let decision = decide(
        &observation,
        &AI_TURTLE_CHOKES,
        &mut AiDecisionMemory::for_profile(&AI_TURTLE_CHOKES),
    );

    assert!(decision.intents.iter().any(|intent| {
        matches!(
            intent,
            AiIntent::Stage { units } if units.as_slice() == [30, 31, 32]
        )
    }));
}

#[test]
fn turtle_machine_gunner_training_stops_at_choke_line_target() {
    let mut owned = vec![
        building(10, EntityKind::CityCentre, Some(0)),
        building(11, EntityKind::Barracks, Some(0)),
        building(12, EntityKind::TrainingCentre, None),
    ];
    owned.extend((0..3).map(|i| combat(30 + i, EntityKind::Rifleman)));
    owned.extend((0..8).map(|i| combat(40 + i, EntityKind::MachineGunner)));
    let mut observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 15,
            supply_cap: 40,
        },
        owned,
    );
    observation.upgrades.push(UpgradeKind::Entrenchment);

    let decision = decide(
        &observation,
        &AI_TURTLE_CHOKES,
        &mut AiDecisionMemory::for_profile(&AI_TURTLE_CHOKES),
    );

    assert!(
        !decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::MachineGunner
        }),
        "the turtle profile should count existing Machine Gunners against its line staffing cap"
    );
    assert!(
        !decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Train { unit: EntityKind::MachineGunner, .. }
            )
        }),
        "the turtle profile should not queue surplus Machine Gunners before they reach the line"
    );
}

#[test]
fn each_required_profile_emits_a_starting_state_command() {
    let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
    owned.extend((0..4).map(|i| worker(20 + i, AiEntityState::Idle)));
    let observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 4,
            supply_cap: 20,
        },
        owned,
    );

    for profile in crate::ai_core::profiles::required_profiles() {
        let decision = decide(
            &observation,
            profile,
            &mut AiDecisionMemory::for_profile(profile),
        );

        assert!(
            !decision.commands.is_empty(),
            "{} should emit at least one plausible opening command",
            profile.id
        );
    }
}
