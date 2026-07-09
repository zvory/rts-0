use super::*;

use crate::ai_core::profiles::{AI_2_0_TANK_PRESSURE, AI_2_1_ECONOMY_MANAGER};

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
