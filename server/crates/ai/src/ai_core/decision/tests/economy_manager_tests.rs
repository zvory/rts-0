use super::*;

use crate::ai_core::decision::economy_manager::{
    propose_economy, EconomyManagerInput, EconomyManagerSignals, EconomyProposal, OilDemandSignal,
};
use crate::ai_core::decision::expansion::ExpansionPlan;
use crate::ai_core::profiles::{
    AI_2_0_TANK_PRESSURE, AI_2_1_ECONOMY_MANAGER, AI_TURTLE_CHOKES,
};
use rts_sim::game::command::SimCommand as Command;

fn abandoned_city_centre(id: u32, tile: (u32, u32), tile_size: u32) -> AiEntitySummary {
    let (x, y) = building_center(tile, EntityKind::CityCentre, tile_size)
        .expect("city centre should have a center");
    let mut city_centre = building_at(id, EntityKind::CityCentre, None, x, y);
    city_centre.hp = 300;
    city_centre.is_complete = false;
    city_centre.state = AiEntityState::Construct;
    city_centre
}

fn completed_city_centre(id: u32, tile: (u32, u32), tile_size: u32) -> AiEntitySummary {
    let (x, y) = building_center(tile, EntityKind::CityCentre, tile_size)
        .expect("city centre should have a center");
    building_at(id, EntityKind::CityCentre, None, x, y)
}

fn abandoned_city_centre_observation(tick: u32) -> AiObservation {
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        vec![
            completed_city_centre(1, (8, 8), config::TILE_SIZE),
            completed_city_centre(2, (20, 20), config::TILE_SIZE),
            abandoned_city_centre(10, (30, 30), config::TILE_SIZE),
            worker(20, AiEntityState::Idle),
        ],
    );
    observation.tick = tick;
    observation
}

fn has_city_centre_resume(decision: &AiDecision) -> bool {
    decision.intents.contains(&AiIntent::ResumeConstruction {
        kind: EntityKind::CityCentre,
    })
}

#[test]
fn direct_and_proposal_economy_profiles_resume_any_quiet_unfinished_city_centre_without_new_resources(
) {
    for profile in [
        &AI_2_0_TANK_PRESSURE,
        &AI_2_1_ECONOMY_MANAGER,
        &AI_TURTLE_CHOKES,
    ] {
        let mut memory = AiDecisionMemory::for_profile(profile);
        let mut observation = abandoned_city_centre_observation(100);

        let first = decide(&observation, profile, &mut memory);
        assert!(
            !has_city_centre_resume(&first),
            "{} should wait before resuming a newly observed site",
            profile.id
        );

        observation.tick += config::TICK_HZ * 3;
        let resumed = decide(&observation, profile, &mut memory);
        assert!(
            has_city_centre_resume(&resumed),
            "{} should resume an unfinished city centre after three quiet seconds",
            profile.id
        );
        assert!(matches!(
            resumed.commands.first(),
            Some(Command::Build {
                units,
                building: EntityKind::CityCentre,
                tile_x: 30,
                tile_y: 30,
                queued: false,
            }) if units == &[20]
        ));
    }
}

#[test]
fn city_centre_recovery_restarts_the_quiet_timer_after_damage() {
    let profile = &AI_2_0_TANK_PRESSURE;
    let mut memory = AiDecisionMemory::for_profile(profile);
    let mut observation = abandoned_city_centre_observation(100);

    assert!(!has_city_centre_resume(&decide(
        &observation,
        profile,
        &mut memory,
    )));

    observation.tick = 130;
    observation
        .owned
        .iter_mut()
        .find(|entity| entity.id == 10)
        .expect("unfinished city centre should be present")
        .hp -= 25;
    assert!(!has_city_centre_resume(&decide(
        &observation,
        profile,
        &mut memory,
    )));

    observation.tick = 130 + config::TICK_HZ * 3 - 1;
    assert!(!has_city_centre_resume(&decide(
        &observation,
        profile,
        &mut memory,
    )));

    observation.tick += 1;
    assert!(has_city_centre_resume(&decide(
        &observation,
        profile,
        &mut memory,
    )));
}

#[test]
fn city_centre_recovery_does_not_duplicate_an_assigned_builder() {
    let profile = &AI_2_0_TANK_PRESSURE;
    let mut memory = AiDecisionMemory::for_profile(profile);
    let mut observation = abandoned_city_centre_observation(100);
    let worker = observation
        .owned
        .iter_mut()
        .find(|entity| entity.id == 20)
        .expect("worker should be present");
    worker.state = AiEntityState::Build;
    worker.target_id = Some(10);

    assert!(!has_city_centre_resume(&decide(
        &observation,
        profile,
        &mut memory,
    )));
    observation.tick += config::TICK_HZ * 3;
    let decision = decide(&observation, profile, &mut memory);
    assert!(!has_city_centre_resume(&decision));
    assert!(!decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Build {
                building: EntityKind::CityCentre,
                ..
            }
        )
    }));
}

#[test]
fn turtle_profile_routes_economy_through_proposal_manager() {
    assert!(AI_2_1_ECONOMY_MANAGER.uses_proposal_economy_manager());
    assert!(AI_TURTLE_CHOKES.uses_proposal_economy_manager());
    assert!(!AI_2_0_TANK_PRESSURE.uses_proposal_economy_manager());
}

fn assert_ai_2_1_matches_ai_2_0_decision(label: &str, observation: &AiObservation) {
    let ai_2_0 = decide(
        observation,
        &AI_2_0_TANK_PRESSURE,
        &mut AiDecisionMemory::for_profile(&AI_2_0_TANK_PRESSURE),
    );
    let ai_2_1 = decide(
        observation,
        &AI_2_1_ECONOMY_MANAGER,
        &mut AiDecisionMemory::for_profile(&AI_2_1_ECONOMY_MANAGER),
    );

    assert_eq!(ai_2_1.intents, ai_2_0.intents, "{label}: intents");
    assert_eq!(ai_2_1.commands, ai_2_0.commands, "{label}: commands");
}

#[test]
fn economy_manager_outputs_action_proposals() {
    let mut owned = vec![building_at(
        1,
        EntityKind::CityCentre,
        Some(0),
        8.0 * config::TILE_SIZE as f32,
        8.0 * config::TILE_SIZE as f32,
    )];
    owned.push(worker(2, AiEntityState::Idle));
    let observation = observation(
        AiEconomy {
            steel: 500,
            oil: 0,
            supply_used: 4,
            supply_cap: 12,
        },
        owned,
    );
    let facts = AiFacts::from_observation(&observation);
    let expansion_plan = ExpansionPlan {
        policy: AI_2_1_ECONOMY_MANAGER.expansion,
        should_save: true,
        blocks_tech_path: false,
        blockers: Vec::new(),
    };

    let output = propose_economy(EconomyManagerInput {
        observation: &observation,
        facts: &facts,
        profile: &AI_2_1_ECONOMY_MANAGER,
        expansion_plan: &expansion_plan,
        signals: EconomyManagerSignals {
            recovery_active: false,
            oil_demand: OilDemandSignal::ExactWorkers(1),
            defer_supply_for_tech: false,
            emergency_supply: false,
            defer_worker_training_for_tech: false,
        },
    });

    assert!(output.proposes(EconomyProposal::BuildExpansionCityCentre));
    assert!(output.proposes(EconomyProposal::TrainWorker));
    assert!(output.proposes(EconomyProposal::AssignOilWorkers));
    assert!(output.proposes(EconomyProposal::AssignSteelWorkers));
}

#[test]
fn economy_manager_signals_can_hold_oil_at_current_assignment() {
    let mut owned = vec![building_at(
        1,
        EntityKind::CityCentre,
        Some(0),
        8.0 * config::TILE_SIZE as f32,
        8.0 * config::TILE_SIZE as f32,
    )];
    owned.push(worker(2, AiEntityState::Idle));
    let observation = observation(
        AiEconomy {
            steel: 500,
            oil: 0,
            supply_used: 4,
            supply_cap: 12,
        },
        owned,
    );
    let facts = AiFacts::from_observation(&observation);
    let expansion_plan = ExpansionPlan {
        policy: None,
        should_save: false,
        blocks_tech_path: false,
        blockers: Vec::new(),
    };

    let output = propose_economy(EconomyManagerInput {
        observation: &observation,
        facts: &facts,
        profile: &AI_2_1_ECONOMY_MANAGER,
        expansion_plan: &expansion_plan,
        signals: EconomyManagerSignals {
            recovery_active: false,
            oil_demand: OilDemandSignal::HoldCurrent,
            defer_supply_for_tech: false,
            emergency_supply: false,
            defer_worker_training_for_tech: false,
        },
    });

    assert_eq!(output.plan.desired_oil_workers, output.plan.current_oil_workers);
    assert!(!output.proposes(EconomyProposal::AssignOilWorkers));
}

#[test]
fn ai_2_1_economy_manager_matches_ai_2_0_decisions_on_core_economy_states() {
    let ts = config::TILE_SIZE as f32;

    let mut tech_opening_owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
    ];
    tech_opening_owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
    assert_ai_2_1_matches_ai_2_0_decision(
        "tech opening",
        &observation(
            AiEconomy {
                steel: 800,
                oil: 250,
                supply_used: 10,
                supply_cap: 14,
            },
            tech_opening_owned,
        ),
    );

    let mut expansion_owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building(11, EntityKind::Barracks, Some(0)),
        building(12, EntityKind::TrainingCentre, Some(0)),
    ];
    expansion_owned.extend((0..18).map(|i| steel_worker(20 + i, 100 + i)));
    expansion_owned.extend((0..4).map(|i| combat(60 + i, EntityKind::Rifleman)));
    expansion_owned.extend((0..2).map(|i| worker(80 + i, AiEntityState::Idle)));
    assert_ai_2_1_matches_ai_2_0_decision(
        "expansion save",
        &with_expansion_resources(observation(
            AiEconomy {
                steel: 900,
                oil: 250,
                supply_used: 24,
                supply_cap: 36,
            },
            expansion_owned,
        )),
    );

    let mut expanded_owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 33.5 * ts),
        building(12, EntityKind::Barracks, Some(0)),
        building(13, EntityKind::TrainingCentre, Some(0)),
        building(14, EntityKind::Factory, Some(0)),
    ];
    expanded_owned.extend((0..18).map(|i| steel_worker(20 + i, 100 + i)));
    expanded_owned.extend((0..6).map(|i| worker(80 + i, AiEntityState::Idle)));
    assert_ai_2_1_matches_ai_2_0_decision(
        "expanded oil assignment",
        &with_expansion_resources(observation(
            AiEconomy {
                steel: 1_000,
                oil: 100,
                supply_used: 30,
                supply_cap: 48,
            },
            expanded_owned,
        )),
    );
}
