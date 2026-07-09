use super::*;

use crate::ai_core::profiles::AI_TURTLE_CHOKES;

#[test]
fn turtle_opening_does_not_train_workers_for_held_oil_assignments() {
    let mut owned = vec![
        building(10, EntityKind::CityCentre, Some(0)),
        building(11, EntityKind::Barracks, Some(0)),
    ];
    owned.extend((0..18).map(|i| steel_worker(20 + i, 100 + i)));
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
        !decision
            .intents
            .contains(&AiIntent::Train { kind: EntityKind::Worker }),
        "the Turtle opening should not train workers toward oil while oil assignments are held"
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
