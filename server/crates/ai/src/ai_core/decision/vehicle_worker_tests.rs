use super::*;

use crate::ai_core::observation::{
    AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiPlayerSummary, AiResourceSummary,
};
use crate::ai_core::profiles::{AiProfile, AI_2_1, JEFFS_AI};

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

fn combat_unit(id: u32, kind: EntityKind) -> AiEntitySummary {
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
        free_for_combat: true,
    }
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
    decide_with_profile(observation, &AI_2_1)
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

fn jeff_opening_observation(worker_count: usize, pump_jacks: usize) -> AiObservation {
    let mut owned = vec![building(1, EntityKind::ResourceDepot, Some(0))];
    owned.extend((0..worker_count).map(|index| {
        worker(
            20 + index as u32,
            if index == 0 {
                AiEntityState::Idle
            } else {
                AiEntityState::Gather
            },
        )
    }));
    owned.extend(
        (0..pump_jacks).map(|index| building(60 + index as u32, EntityKind::PumpJack, None)),
    );
    observation(
        AiEconomy {
            steel: 200,
            oil: 50,
            supply_used: worker_count as u32,
            supply_cap: 40,
        },
        owned,
    )
}

#[test]
fn jeff_waits_for_two_pump_jacks_but_not_a_second_engineer_before_barracks() {
    let decision = decide_with_profile(&jeff_opening_observation(1, 1), &JEFFS_AI);
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));

    let decision = decide_with_profile(&jeff_opening_observation(1, 2), &JEFFS_AI);
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));
}

#[test]
fn jeff_does_not_issue_obsolete_extractor_repeat_commands() {
    let observation = jeff_opening_observation(2, 1);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let width = observation.map.width;
    let height = observation.map.height;

    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &JEFFS_AI,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, tx, ty| tx < width && ty < height,
    );

    assert!(!decision
        .commands
        .iter()
        .any(|command| matches!(command, Command::AdjustProductionRepeat { .. })));

    let second = decide_profile_without_static_map_for_tests(
        &observation,
        &JEFFS_AI,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, tx, ty| tx < width && ty < height,
    );
    assert!(!second
        .commands
        .iter()
        .any(|command| matches!(command, Command::AdjustProductionRepeat { .. })));
}

fn jeff_armored_tech_observation(factory: Option<AiEntitySummary>) -> AiObservation {
    let mut owned = vec![
        building(1, EntityKind::ResourceDepot, Some(0)),
        building(2, EntityKind::Barracks, Some(0)),
        building(3, EntityKind::TrainingCentre, None),
        building(4, EntityKind::EngineeringComplex, Some(0)),
        worker(20, AiEntityState::Idle),
    ];
    if let Some(factory) = factory {
        owned.push(factory);
    }
    observation(
        AiEconomy {
            steel: 2_000,
            oil: 2_000,
            supply_used: 20,
            supply_cap: 100,
        },
        owned,
    )
}

#[test]
fn jeff_starts_vehicle_works_before_tank_production_research() {
    let without_factory = jeff_armored_tech_observation(None);
    let decision = decide_with_profile(&without_factory, &JEFFS_AI);
    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Build {
                building: EntityKind::Factory,
                ..
            }
        )
    }));
    assert!(!decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Research {
                upgrade: UpgradeKind::TankUnlock,
                ..
            }
        )
    }));

    let with_factory =
        jeff_armored_tech_observation(Some(building(5, EntityKind::Factory, Some(0))));
    let decision = decide_with_profile(&with_factory, &JEFFS_AI);
    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Research {
                upgrade: UpgradeKind::TankUnlock,
                ..
            }
        )
    }));
}

#[test]
fn jeff_moves_machine_gunner_off_blocked_vehicle_works_footprint() {
    let mut observation = jeff_armored_tech_observation(None);
    let ts = observation.map.tile_size as f32;
    let mut machine_gunner = combat_unit(30, EntityKind::MachineGunner);
    machine_gunner.hp = config::unit_stats(EntityKind::MachineGunner)
        .expect("Machine Gunner stats")
        .hp;
    machine_gunner.x = 2.5 * ts;
    machine_gunner.y = 2.5 * ts;
    observation.owned.push(machine_gunner);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &JEFFS_AI,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, _, _| false,
    );

    assert!(!decision.commands.iter().any(|command| matches!(
        command,
        Command::Build {
            building: EntityKind::Factory,
            ..
        }
    )));
    assert!(decision.commands.iter().any(|command| matches!(
        command,
        Command::Move { units, .. } if units == &[30]
    )));
    assert!(decision
        .intents
        .iter()
        .any(|intent| matches!(intent, AiIntent::Move { units } if units == &[30])));
}

#[test]
fn jeff_clears_machine_gunner_when_issuing_vehicle_works_build() {
    let mut observation = jeff_armored_tech_observation(None);
    let ts = observation.map.tile_size as f32;
    let mut machine_gunner = combat_unit(30, EntityKind::MachineGunner);
    machine_gunner.hp = config::unit_stats(EntityKind::MachineGunner)
        .expect("Machine Gunner stats")
        .hp;
    machine_gunner.x = 2.5 * ts;
    machine_gunner.y = 2.5 * ts;
    observation.owned.push(machine_gunner);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &JEFFS_AI,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |kind, tx, ty| kind == EntityKind::Factory && (tx, ty) == (2, 2),
    );

    assert!(decision.commands.iter().any(|command| matches!(
        command,
        Command::Build {
            building: EntityKind::Factory,
            tile_x: 2,
            tile_y: 2,
            ..
        }
    )));
    let destination = decision.commands.iter().find_map(|command| match command {
        Command::Move { units, x, y, .. } if units == &[30] => Some((*x, *y)),
        _ => None,
    });
    let destination = destination.expect("Machine Gunner move alongside Factory build");
    let moved_tiles =
        ((destination.0 - 2.5 * ts).powi(2) + (destination.1 - 2.5 * ts).powi(2)).sqrt() / ts;
    assert!((moved_tiles - 4.0).abs() < 0.001);
}

#[test]
fn jeff_interrupts_busy_machine_gunner_to_clear_vehicle_works_site() {
    let mut observation = jeff_armored_tech_observation(None);
    let ts = observation.map.tile_size as f32;
    let mut machine_gunner = combat_unit(30, EntityKind::MachineGunner);
    machine_gunner.hp = config::unit_stats(EntityKind::MachineGunner)
        .expect("Machine Gunner stats")
        .hp;
    machine_gunner.x = 2.5 * ts;
    machine_gunner.y = 2.5 * ts;
    machine_gunner.state = AiEntityState::Attack;
    machine_gunner.target_id = Some(99);
    machine_gunner.free_for_combat = false;
    observation.owned.push(machine_gunner);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &JEFFS_AI,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |kind, tx, ty| kind == EntityKind::Factory && (tx, ty) == (2, 2),
    );

    assert!(decision.commands.iter().any(|command| matches!(
        command,
        Command::Build {
            building: EntityKind::Factory,
            tile_x: 2,
            tile_y: 2,
            ..
        }
    )));
    assert!(decision.commands.iter().any(|command| matches!(
        command,
        Command::Move { units, .. } if units == &[30]
    )));
    assert!(decision
        .intents
        .iter()
        .any(|intent| matches!(intent, AiIntent::Move { units } if units == &[30])));
}

#[test]
fn jeff_starts_defensive_tank_before_anti_tank_research() {
    let mut factory = building(5, EntityKind::Factory, Some(0));
    let mut observation = jeff_armored_tech_observation(Some(factory.clone()));
    observation.upgrades = vec![UpgradeKind::TankUnlock, UpgradeKind::Entrenchment];
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment_wave_launched = true;
    let width = observation.map.width;
    let height = observation.map.height;
    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &JEFFS_AI,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, tx, ty| tx < width && ty < height,
    );
    assert!(!decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Research {
                upgrade: UpgradeKind::AntiTankGunUnlock,
                ..
            }
        )
    }));

    factory.state = AiEntityState::Train;
    factory.production_queue_len = Some(1);
    factory.production_kind = Some(EntityKind::Tank);
    observation = jeff_armored_tech_observation(Some(factory));
    observation.upgrades = vec![UpgradeKind::TankUnlock, UpgradeKind::Entrenchment];
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment_wave_launched = true;
    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &JEFFS_AI,
        &mut memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, tx, ty| tx < width && ty < height,
    );
    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Research {
                upgrade: UpgradeKind::AntiTankGunUnlock,
                ..
            }
        )
    }));
}

#[test]
fn jeff_attack_phase_switch_preserves_home_defense_assignment() {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment_wave_launched = true;
    memory.containment_opening_tanks.extend([101, 102]);
    memory.home_defensive_tank = Some(103);
    memory.home_defensive_tank_assigned_once = true;
    memory.enemy_natural_resource_depot = Some(201);

    let mut transitioned_attack = JEFFS_AI.attack;
    transitioned_attack.first_attack_size += 1;
    memory.desired_attack_size_for(&JEFFS_AI, transitioned_attack, 9_000);

    assert!(memory.containment_wave_launched);
    assert_eq!(memory.containment_opening_tanks, BTreeSet::from([101, 102]));
    assert_eq!(memory.home_defensive_tank, Some(103));
    assert!(memory.home_defensive_tank_assigned_once);
    assert_eq!(memory.enemy_natural_resource_depot, Some(201));
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
            building(10, EntityKind::ResourceDepot, Some(0)),
            building(11, EntityKind::ResourceDepot, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::EngineeringComplex, None),
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
            building_at(10, EntityKind::ResourceDepot, Some(0), 8.5 * ts, 8.5 * ts),
            building_at(11, EntityKind::ResourceDepot, Some(0), 23.5 * ts, 36.5 * ts),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::EngineeringComplex, None),
            worker(60, AiEntityState::Idle),
        ],
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Methamphetamines);

    let width = observation.map.width;
    let height = observation.map.height;
    let decision = decide_profile_without_static_map_for_tests(
        &observation,
        &AI_2_1,
        &mut AiDecisionMemory::for_profile(&AI_2_1),
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
fn ai_2_1_builds_second_factory_above_resource_float() {
    let observation = second_factory_observation(501, 326);

    let decision = decide_with_profile(&observation, &AI_2_1);

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Steelworks
    }));
}

#[test]
fn ai_2_1_waits_until_above_second_factory_resource_float() {
    let observation = second_factory_observation(500, 325);

    let decision = decide_with_profile(&observation, &AI_2_1);

    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
}

fn jeff_surplus_observation(steel: u32) -> AiObservation {
    let mut observation = with_expansion_resources(observation(
        AiEconomy {
            steel,
            oil: 1,
            supply_used: 54,
            supply_cap: 120,
        },
        vec![
            building(10, EntityKind::ResourceDepot, Some(0)),
            building(11, EntityKind::ResourceDepot, Some(0)),
            building(12, EntityKind::Barracks, Some(0)),
            building(13, EntityKind::TrainingCentre, None),
            building(14, EntityKind::EngineeringComplex, None),
            building(15, EntityKind::Factory, Some(1)),
            combat_unit(30, EntityKind::Tank),
            combat_unit(31, EntityKind::Tank),
            combat_unit(32, EntityKind::Tank),
            combat_unit(33, EntityKind::ScoutCar),
            combat_unit(34, EntityKind::MachineGunner),
            combat_unit(35, EntityKind::MachineGunner),
            worker(40, AiEntityState::Idle),
            worker(41, AiEntityState::Idle),
        ],
    ));
    observation.upgrades.push(UpgradeKind::TankUnlock);
    observation.upgrades.push(UpgradeKind::Entrenchment);
    observation
}

#[test]
fn jeff_does_not_spend_the_six_hundred_steel_reserve_on_riflemen() {
    let decision = decide_with_profile(&jeff_surplus_observation(600), &JEFFS_AI);

    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));
}

#[test]
fn jeff_pays_the_full_rifleman_cost_above_the_reserve() {
    let reserve = JEFFS_AI.surplus_steel_production.unwrap().reserve;
    let (rifle_steel, _) = rts_rules::economy::cost(EntityKind::Rifleman);
    let decision = decide_with_profile(
        &jeff_surplus_observation(reserve + rifle_steel - 1),
        &JEFFS_AI,
    );
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));

    let decision = decide_with_profile(&jeff_surplus_observation(reserve + rifle_steel), &JEFFS_AI);

    assert!(decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));
}

#[test]
fn jeff_does_not_queue_riflemen_after_a_second_barracks_spends_to_the_reserve() {
    let reserve = JEFFS_AI.surplus_steel_production.unwrap().reserve;
    let (barracks_steel, _) = rts_rules::economy::cost(EntityKind::Barracks);
    let decision = decide_with_profile(
        &jeff_surplus_observation(reserve + barracks_steel),
        &JEFFS_AI,
    );

    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Barracks
    }));
    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Rifleman
    }));
}

#[test]
fn jeff_enters_larger_repush_recovery_after_an_active_tank_is_lost() {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment_wave_launched = true;
    memory.containment_opening_tanks = BTreeSet::from([101, 102]);
    memory.containment_active_tanks = BTreeSet::from([101, 102]);
    memory.containment_active_scout = Some(203);
    memory.containment_active_riflemen = BTreeSet::from([301, 302]);
    memory.home_defensive_tank = Some(200);
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 20,
            supply_cap: 120,
        },
        vec![
            combat_unit(200, EntityKind::Tank),
            combat_unit(203, EntityKind::ScoutCar),
        ],
    );

    sync_containment_recovery(&observation, &JEFFS_AI, &mut memory);
    assert!(memory.containment_recovery_active);
    assert_eq!(memory.containment_repush_count, 1);
    assert!(memory.containment_active_tanks.is_empty());
    assert!(memory.containment_active_riflemen.is_empty());
    assert_eq!(
        JEFFS_AI
            .expansion_containment
            .unwrap()
            .recovery_tanks_to_continue,
        3
    );
}

#[test]
fn jeff_adds_the_second_vehicle_works_after_three_tanks_at_the_earlier_float() {
    let mut observation = jeff_surplus_observation(251);
    observation.economy.oil = 126;

    let decision = decide_with_profile(&observation, &JEFFS_AI);
    assert!(decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));

    observation.owned.retain(|entity| entity.id != 32);
    let decision = decide_with_profile(&observation, &JEFFS_AI);
    assert!(!decision.intents.contains(&AiIntent::Build {
        kind: EntityKind::Factory
    }));
}

#[test]
fn extractor_saturation_does_not_train_mining_workers_before_first_factory() {
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
            building_at(10, EntityKind::ResourceDepot, Some(0), 8.5 * ts, 8.5 * ts),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
            building(13, EntityKind::EngineeringComplex, None),
            worker(20, AiEntityState::Idle),
        ],
    );

    let decision = decide(&observation);

    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
    assert!(
        decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Factory
        }),
        "the old mining-worker spend must not block the first Factory"
    );
}

#[test]
fn extractor_saturation_does_not_train_mining_workers_before_first_tank() {
    let ts = config::TILE_SIZE as f32;
    let (tank_steel, tank_oil) = rts_rules::economy::cost(EntityKind::Tank);
    let mut owned = vec![
        building_at(10, EntityKind::ResourceDepot, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(11, EntityKind::ResourceDepot, Some(0), 23.5 * ts, 36.5 * ts),
        building(12, EntityKind::Barracks, Some(0)),
        building(13, EntityKind::TrainingCentre, None),
        building(14, EntityKind::EngineeringComplex, None),
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

    assert!(!decision.intents.contains(&AiIntent::Train {
        kind: EntityKind::Worker
    }));
    assert!(
        decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Tank
        }),
        "the old mining-worker spend must not preempt Tank production"
    );
}
