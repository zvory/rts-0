use super::*;

use crate::ai_core::observation::{
    AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiPlayerSummary, AiResourceSummary,
};
use crate::ai_core::profiles::{AiProfile, AI_1_1_TANK_MG, AI_1_2_WAVE_COHORTS};

fn worker(id: u32, state: AiEntityState) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind: EntityKind::Worker,
        x: id as f32,
        y: 0.0,
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

fn steel_worker(id: u32, node: u32) -> AiEntitySummary {
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
        hp: 100,
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

fn observation(economy: AiEconomy, owned: Vec<AiEntitySummary>) -> AiObservation {
    let tile_size = config::TILE_SIZE;
    let ts = tile_size as f32;
    let mut resources = Vec::new();
    for i in 0..18 {
        resources.push(resource(
            100 + i,
            EntityKind::Steel,
            (8.5 + (i % 6) as f32) * ts,
            (8.5 + (i / 6) as f32) * ts,
        ));
    }
    for i in 0..3 {
        resources.push(resource(
            200 + i,
            EntityKind::Oil,
            (10.5 + i as f32) * ts,
            12.5 * ts,
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

fn decide(observation: &AiObservation) -> AiDecision {
    decide_with_profile(observation, &AI_1_1_TANK_MG)
}

fn decide_with_profile(observation: &AiObservation, profile: &'static AiProfile) -> AiDecision {
    let width = observation.map.width;
    let height = observation.map.height;
    decide_profile_without_static_map_for_tests(
        observation,
        profile,
        &mut AiDecisionMemory::for_profile(profile),
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, tx, ty| tx < width && ty < height,
    )
}

fn second_factory_observation(steel: u32, oil: u32) -> AiObservation {
    with_expansion_resources(observation(
        AiEconomy {
            steel,
            oil,
            supply_used: 54,
            supply_cap: 120,
        },
        vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::CityCentre, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::ResearchComplex, None),
            building(15, EntityKind::Factory, Some(0)),
            worker(20, AiEntityState::Idle),
        ],
    ))
}

#[test]
fn places_first_factory_in_shorter_forward_band() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 1_500,
            oil: 800,
            supply_used: 54,
            supply_cap: 120,
        },
        vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::ResearchComplex, None),
            worker(60, AiEntityState::Idle),
        ],
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let width = observation.map.width;
    let height = observation.map.height;
    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &AI_1_1_TANK_MG,
        &mut AiDecisionMemory::for_profile(&AI_1_1_TANK_MG),
        ai_shared::BuildSearch {
            min_radius: 2,
            max_radius: 6,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |kind, tx, ty| {
            tx < width
                && ty < height
                && kind == EntityKind::Factory
                && matches!((tx, ty), (20, 20) | (28, 28))
        },
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(
        decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Build { building: EntityKind::Factory, tile_x, tile_y, .. }
                    if (*tile_x, *tile_y) == (20, 20)
            )
        }),
        "Factory placement should use the nearer forward site instead of the old far-forward edge"
    );
}

#[test]
fn does_not_build_second_factory_for_tank_production() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 1_500,
            oil: 800,
            supply_used: 54,
            supply_cap: 120,
        },
        vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::ResearchComplex, None),
            building(15, EntityKind::Factory, Some(0)),
            worker(60, AiEntityState::Idle),
        ],
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(&observation);

    assert!(
        !decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Factory
        }),
        "AI 1.1 should stay capped at one Factory"
    );
}

#[test]
fn ai_1_2_builds_second_factory_above_resource_float() {
    let observation = second_factory_observation(601, 401);

    let decision = decide_with_profile(&observation, &AI_1_2_WAVE_COHORTS);

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Steelworks
    }));
}

#[test]
fn ai_1_2_waits_until_above_second_factory_resource_float() {
    let observation = second_factory_observation(600, 400);

    let decision = decide_with_profile(&observation, &AI_1_2_WAVE_COHORTS);

    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
}

#[test]
fn ai_1_1_does_not_build_second_factory_at_ai_1_2_float() {
    let observation = second_factory_observation(601, 401);

    let decision = decide(&observation);

    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
}

#[test]
fn trains_worker_before_first_factory_when_below_saturation() {
    let ts = config::TILE_SIZE as f32;
    let (factory_steel, factory_oil) = rts_rules::economy::cost(EntityKind::Factory);
    let observation = observation(
        AiEconomy {
            steel: factory_steel,
            oil: factory_oil,
            supply_used: 8,
            supply_cap: 40,
        },
        vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::ResearchComplex, None),
            worker(20, AiEntityState::Idle),
        ],
    );

    let decision = decide(&observation);

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
    assert!(
        !decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Factory
        }),
        "Worker production should reserve the first spend when below saturation"
    );
}

#[test]
fn trains_workers_before_first_tank_when_below_two_base_saturation() {
    let ts = config::TILE_SIZE as f32;
    let (tank_steel, tank_oil) = rts_rules::economy::cost(EntityKind::Tank);
    let mut owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
        building(12, EntityKind::Barracks, Some(0)),
        building(13, EntityKind::TrainingCentre, None),
        building(14, EntityKind::ResearchComplex, None),
        building(15, EntityKind::Factory, Some(0)),
        building(16, EntityKind::Steelworks, Some(0)),
    ];
    owned.extend((0..18).map(|i| steel_worker(40 + i, 100 + i)));
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel: tank_steel,
            oil: tank_oil,
            supply_used: 28,
            supply_cap: 80,
        },
        owned,
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let decision = decide(&observation);

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
    assert!(
        !decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Tank
        }),
        "first Tank should not preempt Worker queues below main-plus-natural saturation"
    );
}
