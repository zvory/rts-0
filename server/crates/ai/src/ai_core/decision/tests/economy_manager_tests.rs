use super::*;

use crate::ai_core::decision::economy_manager::{
    propose_economy, EconomyManagerInput, EconomyManagerSignals, EconomyProposal, OilDemandSignal,
};
use crate::ai_core::decision::expansion::ExpansionPlan;
use crate::ai_core::profiles::{AI_2_1, AI_TURTLE};

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
fn canonical_profiles_resume_a_quiet_unfinished_city_centre() {
    for profile in [&AI_2_1, &AI_TURTLE] {
        let mut memory = AiDecisionMemory::for_profile(profile);
        let mut observation = abandoned_city_centre_observation(100);

        assert!(!has_city_centre_resume(&decide(
            &observation,
            profile,
            &mut memory,
        )));

        observation.tick += config::TICK_HZ * 3;
        let resumed = decide(&observation, profile, &mut memory);
        assert!(
            has_city_centre_resume(&resumed),
            "{} should resume",
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
    let profile = &AI_2_1;
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

    observation.tick += config::TICK_HZ * 3;
    assert!(has_city_centre_resume(&decide(
        &observation,
        profile,
        &mut memory,
    )));
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
        policy: AI_2_1.expansion,
        should_save: true,
        blocks_tech_path: false,
        blockers: Vec::new(),
    };

    let output = propose_economy(EconomyManagerInput {
        observation: &observation,
        facts: &facts,
        profile: &AI_2_1,
        expansion_plan: &expansion_plan,
        signals: EconomyManagerSignals {
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
fn economy_manager_can_hold_oil_at_current_assignment() {
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
        profile: &AI_2_1,
        expansion_plan: &expansion_plan,
        signals: EconomyManagerSignals {
            oil_demand: OilDemandSignal::HoldCurrent,
            defer_supply_for_tech: false,
            emergency_supply: false,
            defer_worker_training_for_tech: false,
        },
    });

    assert_eq!(
        output.plan.desired_oil_workers,
        output.plan.current_oil_workers
    );
    assert!(!output.proposes(EconomyProposal::AssignOilWorkers));
}
