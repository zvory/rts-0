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

#[test]
fn pressure_retries_first_wave_research_on_a_replacement_training_centre() {
    let mut obs = observation(
        AiEconomy {
            steel: 1000,
            oil: 1000,
            supply_used: 10,
            supply_cap: 300,
        },
        vec![
            worker(1, AiEntityState::Idle),
            building(2, EntityKind::ResourceDepot, Some(0)),
            building(3, EntityKind::Barracks, Some(0)),
            building(4, EntityKind::TrainingCentre, Some(0)),
            building(5, EntityKind::EngineeringComplex, Some(0)),
            building(6, EntityKind::Factory, Some(0)),
        ],
    );
    obs.tick = 2500;
    let mut memory = AiDecisionMemory::for_profile(&PRESSURE);
    let first = decide(&obs, &PRESSURE, &mut memory);
    assert!(first.commands.iter().any(|c| matches!(
        c,
        Command::Research {
            building: 4,
            upgrade: UpgradeKind::Methamphetamines
        }
    )));
    obs.tick += 9;
    let pending = decide(&obs, &PRESSURE, &mut memory);
    assert!(
        !pending.commands.iter().any(|c| matches!(
            c,
            Command::Research {
                upgrade: UpgradeKind::Methamphetamines,
                ..
            }
        )),
        "live producer research must not be requeued"
    );
    obs.owned.retain(|e| e.id != 4);
    obs.owned
        .push(building(14, EntityKind::TrainingCentre, Some(0)));
    obs.tick += 9;
    let retry = decide(&obs, &PRESSURE, &mut memory);
    assert!(
        retry.commands.iter().any(|c| matches!(
            c,
            Command::Research {
                building: 14,
                upgrade: UpgradeKind::Methamphetamines
            }
        )),
        "lost research must not permanently lock the opening wave"
    );
}

#[test]
fn pressure_gives_nearby_panzerfausts_to_home_defense_before_staging() {
    let home = 8.5 * config::TILE_SIZE as f32;
    let mut obs = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 10,
            supply_cap: 300,
        },
        vec![
            worker_at(1, AiEntityState::Idle, home, home),
            building_at(2, EntityKind::ResourceDepot, Some(0), home, home),
            combat_at(99, EntityKind::Panzerfaust, home, home),
        ],
    );
    obs.tick = 6000;
    obs.upgrades
        .extend([UpgradeKind::Methamphetamines, UpgradeKind::Panzerfausts]);
    let mut enemy = combat_at(100, EntityKind::Tank, home + 32.0, home);
    enemy.owner = 2;
    obs.visible_enemies.push(enemy);
    let decision = decide(
        &obs,
        &PRESSURE,
        &mut AiDecisionMemory::for_profile(&PRESSURE),
    );
    assert!(decision
        .commands
        .iter()
        .any(|c| matches!(c, Command::Attack { units, target: 100, .. } if units.contains(&99))));
    assert!(
        !decision.pressure_wave_units.contains(&99),
        "home defense must keep the Panzerfaust this decision"
    );
}

#[test]
fn pressure_keeps_engaged_home_defenders_but_can_stage_engaged_units_outside_home() {
    let home = 8.5 * config::TILE_SIZE as f32;
    for kind in [EntityKind::Panzerfaust, EntityKind::Rifleman] {
        let mut defender = combat_at(99, kind, home, home);
        defender.state = AiEntityState::Attack;
        defender.target_id = Some(100);
        defender.free_for_combat = false;
        let mut obs = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 10,
                supply_cap: 300,
            },
            vec![
                worker_at(1, AiEntityState::Idle, home, home),
                building_at(2, EntityKind::ResourceDepot, Some(0), home, home),
                defender,
            ],
        );
        obs.tick = 6009;
        obs.upgrades
            .extend([UpgradeKind::Methamphetamines, UpgradeKind::Panzerfausts]);
        let mut enemy = combat_at(100, EntityKind::Tank, home + 32.0, home);
        enemy.owner = 2;
        obs.visible_enemies.push(enemy);
        let mut memory = AiDecisionMemory::for_profile(&PRESSURE);
        let defending = decide(&obs, &PRESSURE, &mut memory);
        assert!(
            !defending.pressure_wave_units.contains(&99),
            "engaged {kind:?} must retain home defense across decisions"
        );
        obs.owned.iter_mut().filter(|e| e.id == 99).for_each(|e| {
            e.x = 1300.0;
            e.y = 1300.0;
        });
        obs.tick += 9;
        let away = decide(&obs, &PRESSURE, &mut memory);
        assert!(
            away.pressure_wave_units.contains(&99),
            "a fight outside home must not permanently reserve staging recruits"
        );
        obs.owned.iter_mut().filter(|e| e.id == 99).for_each(|e| {
            e.x = home;
            e.y = home;
        });
        obs.visible_enemies.clear();
        obs.tick += 9;
        let released = decide(&obs, &PRESSURE, &mut memory);
        assert!(
            released.pressure_wave_units.contains(&99),
            "staging resumes when the home threat is gone"
        );
    }
}
