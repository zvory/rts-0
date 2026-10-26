use super::*;
use crate::ai_core::profiles::PRESSURE;

#[test]
fn pressure_stops_legacy_rifleman_orders_only_after_panzerfaust_research_completes() {
    let mut owned = vec![
        worker(1, AiEntityState::Idle),
        building(2, EntityKind::ResourceDepot, Some(0)),
        building(3, EntityKind::Barracks, Some(0)),
        building(4, EntityKind::TrainingCentre, Some(0)),
    ];
    owned.extend((5..9).map(|id| combat(id, EntityKind::MachineGunner)));
    let mut obs = observation(
        AiEconomy {
            steel: 200,
            oil: 50,
            supply_used: 10,
            supply_cap: 300,
        },
        owned,
    );
    obs.tick = 3600;
    obs.upgrades.push(UpgradeKind::Methamphetamines);
    let before = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(
        before.commands.iter().any(|c| matches!(
            c,
            Command::Train {
                unit: EntityKind::Rifleman,
                ..
            }
        )),
        "Rifleman production must continue while Panzerfausts are locked"
    );
    obs.upgrades.push(UpgradeKind::Panzerfausts);
    obs.owned.push(combat(99, EntityKind::Panzerfaust));
    let after = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(
        !after.commands.iter().any(|c| matches!(
            c,
            Command::Train {
                unit: EntityKind::Rifleman,
                ..
            }
        )),
        "legacy Rifleman training must not compete with Panzerfaust repeat"
    );
    assert!(
        after.pressure_wave_units.contains(&99),
        "Panzerfausts must join the assault staging owner"
    );
}
