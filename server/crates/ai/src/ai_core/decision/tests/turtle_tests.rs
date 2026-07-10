use super::*;

use crate::ai_core::profiles::AI_TURTLE_CHOKES;

#[test]
fn turtle_expansion_ignores_opening_rifleman_losses() {
    let ts = config::TILE_SIZE as f32;
    let observation = with_expansion_resources(observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 30,
            supply_cap: 60,
        },
        vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building(11, EntityKind::TrainingCentre, None),
        ],
    ));
    let facts = AiFacts::from_observation(&observation);

    let plan = super::super::expansion::plan_expansion(
        &observation,
        &facts,
        &AI_TURTLE_CHOKES,
        false,
        false,
    );

    assert!(
        plan.should_save,
        "Turtle should save for its second City Centre even after its opening Riflemen die"
    );
    assert!(plan.blockers.is_empty());
}

#[test]
fn turtle_opening_starts_one_pump_jack_without_stalling_worker_training() {
    let ts = config::TILE_SIZE as f32;
    let owned = vec![
        building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(11, EntityKind::Barracks, Some(0), 9.5 * ts, 8.5 * ts),
        worker_at(20, AiEntityState::Idle, 9.5 * ts, 9.5 * ts),
        worker_at(21, AiEntityState::Idle, 10.5 * ts, 9.5 * ts),
    ];
    let observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 0,
            supply_used: 18,
            supply_cap: 40,
        },
        owned,
    );

    let decision = decide(
        &observation,
        &AI_TURTLE_CHOKES,
        &mut AiDecisionMemory::for_profile(&AI_TURTLE_CHOKES),
    );

    assert!(
        decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }),
        "the Turtle opening must keep the City Centre on worker production"
    );
    assert_eq!(
        decision
            .commands
            .iter()
            .filter(|command| {
                matches!(
                    command,
                    Command::Build {
                        building: EntityKind::PumpJack,
                        ..
                    }
                )
            })
            .count(),
        1,
        "the opening should fund one Pump Jack, not hold oil entirely or overcommit workers"
    );
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
            AiIntent::Stage { units } if units.as_slice() == [30, 31]
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
    owned.extend((0..2).map(|i| combat(30 + i, EntityKind::Rifleman)));
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
                Command::Train {
                    unit: EntityKind::MachineGunner,
                    ..
                }
            )
        }),
        "the turtle profile should not queue surplus Machine Gunners before they reach the line"
    );
}

#[test]
fn turtle_spends_large_float_on_second_gun_works_not_a_second_barracks() {
    let mut owned = vec![
        building(10, EntityKind::CityCentre, Some(0)),
        building(11, EntityKind::CityCentre, Some(0)),
        building(12, EntityKind::Barracks, Some(0)),
        building(13, EntityKind::TrainingCentre, None),
        building(14, EntityKind::ResearchComplex, None),
        building(15, EntityKind::Steelworks, Some(0)),
        worker(20, AiEntityState::Idle),
        worker(21, AiEntityState::Idle),
    ];
    owned.extend((0..3).map(|i| combat(30 + i, EntityKind::Rifleman)));
    let mut observation = observation(
        AiEconomy {
            steel: 1_200,
            oil: 600,
            supply_used: 20,
            supply_cap: 80,
        },
        owned,
    );
    observation.upgrades = vec![UpgradeKind::Entrenchment, UpgradeKind::AntiTankGunUnlock];

    let decision = decide(
        &observation,
        &AI_TURTLE_CHOKES,
        &mut AiDecisionMemory::for_profile(&AI_TURTLE_CHOKES),
    );

    assert!(
        decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Steelworks
        }),
        "the Turtle should spend its large float on a second Gun Works"
    );
    assert!(decision.commands.iter().any(|command| {
        matches!(
            command,
            Command::Build {
                building: EntityKind::Steelworks,
                ..
            }
        )
    }));
    assert!(
        !decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Barracks
        }),
        "the Turtle should keep its single Barracks instead of delaying support tech"
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
