//! The matchup micro's in-game hook (Jeff's AI V1.1 with the `mm` switch): run over the finished
//! decision, it takes the units it steers out of every other movement order and orders them.

use super::*;
use rts_sim::game::command::SimCommand as Command;

/// The live parameters: `JEFF_V1_1_MM` (`key=value,...`, or `JEFF_V11_MM` in local testing) over
/// the tuned defaults.
fn live_params() -> Params {
    static PARAMS: std::sync::OnceLock<Params> = std::sync::OnceLock::new();
    *PARAMS.get_or_init(|| {
        let spec = std::env::var("JEFF_V11_MM")
            .unwrap_or_else(|_| crate::ai_core::profiles::JEFF_V1_1_MM.to_string());
        let mut params = Params::parse(&spec).unwrap_or_default();
        // `standfire` as a V1.1 switch too.
        params.stand_fire |= !crate::ai_core::profiles::jeff_v1_1_feature_off("standfire");
        params
    })
}

/// The units a command orders, for the commands that order units.
fn units_of(command: &Command) -> &[u32] {
    match command {
        Command::Move { units, .. }
        | Command::FormationMove { units, .. }
        | Command::AttackMove { units, .. }
        | Command::ClearObstacleArea { units, .. }
        | Command::Attack { units, .. }
        | Command::AttackTankTrapCluster { units, .. }
        | Command::UseAbility { units, .. }
        | Command::RecastAbility { units, .. }
        | Command::PointTanks { units, .. }
        | Command::Stop { units }
        | Command::HoldPosition { units, .. } => units,
        _ => &[],
    }
}

/// Takes `held` out of a movement or attack order; abilities (smoke) and economy orders stay.
/// Returns false when the order no longer orders anyone.
pub(in crate::ai_core::decision) fn strip(command: &mut Command, held: &BTreeSet<u32>) -> bool {
    match command {
        Command::FormationMove { units, points, .. } => {
            let pairs: Vec<(u32, (f32, f32))> = units
                .iter()
                .copied()
                .zip(points.iter().copied())
                .filter(|(unit, _)| !held.contains(unit))
                .collect();
            if pairs.len() != units.len() {
                *units = pairs.iter().map(|(unit, _)| *unit).collect();
                *points = pairs.iter().map(|(_, point)| *point).collect();
            }
            !units.is_empty()
        }
        Command::Move { units, .. }
        | Command::AttackMove { units, .. }
        | Command::ClearObstacleArea { units, .. }
        | Command::Attack { units, .. }
        | Command::AttackTankTrapCluster { units, .. }
        | Command::PointTanks { units, .. }
        | Command::Stop { units }
        | Command::HoldPosition { units, .. } => {
            units.retain(|unit| !held.contains(unit));
            !units.is_empty()
        }
        _ => true,
    }
}

/// Remembers where this decision's orders send each unit.
fn note_goals(commands: &[Command], memory: &mut MatchupMemory) {
    for command in commands {
        match command {
            Command::Move { units, x, y, .. } | Command::AttackMove { units, x, y, .. } => {
                let attack = matches!(command, Command::AttackMove { .. });
                for unit in units {
                    memory.goals.insert(
                        *unit,
                        Goal {
                            attack,
                            x: *x,
                            y: *y,
                        },
                    );
                }
            }
            Command::FormationMove {
                units,
                points,
                attack_move,
                ..
            } => {
                for (unit, point) in units.iter().zip(points) {
                    memory.goals.insert(
                        *unit,
                        Goal {
                            attack: *attack_move,
                            x: point.0,
                            y: point.1,
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

/// Marks the units this decision sends at a building, worker or obstacle (raids, trap clearing),
/// and forgets marks that ran out (a mark lasts `eco` ticks, or while the unit still shoots it).
fn note_directed(
    commands: &[Command],
    observation: &AiObservation,
    params: &Params,
    memory: &mut MatchupMemory,
) {
    let tick = observation.tick;
    if params.eco == 0 {
        memory.directed.clear();
        return;
    }
    let soft = |target: u32| {
        observation
            .visible_enemies
            .iter()
            .find(|enemy| enemy.id == target)
            .is_none_or(|enemy| !fights(enemy.kind))
    };
    for command in commands {
        match command {
            Command::Attack { units, target, .. } if soft(*target) => {
                for unit in units {
                    memory
                        .directed
                        .insert(*unit, (tick + params.eco, Some(*target)));
                }
            }
            Command::ClearObstacleArea { units, .. }
            | Command::AttackTankTrapCluster { units, .. } => {
                for unit in units {
                    memory.directed.insert(*unit, (tick + params.eco, None));
                }
            }
            _ => {}
        }
    }
    let shooting: BTreeMap<u32, Option<u32>> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit.target_id))
        .collect();
    memory.directed.retain(|unit, (until, target)| {
        shooting
            .get(unit)
            .is_some_and(|now| tick < *until || (target.is_some() && *now == *target))
    });
}

fn to_command(unit: u32, order: Order) -> Command {
    match order {
        Order::Move(x, y) => Command::Move {
            units: vec![unit],
            x,
            y,
            queued: false,
        },
        Order::Attack(target) => Command::Attack {
            units: vec![unit],
            target,
            queued: false,
        },
        Order::Point(x, y) => Command::PointTanks {
            units: vec![unit],
            x,
            y,
            queued: false,
        },
        Order::Hold => Command::HoldPosition {
            units: vec![unit],
            queued: false,
        },
    }
}

/// Runs the matchup micro over V1.1's finished decision: the units it steers lose every other
/// movement order, it adds its own, and units it lets go get their last other order back.
pub(in crate::ai_core::decision) fn finish(
    decision: &mut crate::ai_core::decision::AiDecision,
    observation: &AiObservation,
    memory: &mut MatchupMemory,
    reserved: &BTreeSet<u32>,
) {
    if !crate::ai_core::profiles::is_jeff_v1_1(decision.profile_id)
        || crate::ai_core::profiles::jeff_v1_1_feature_off("mm")
    {
        return;
    }
    let params = live_params();
    let ts = observation.map.tile_size.max(1) as f32;
    let home = (
        (observation.own_start_tile.0 as f32 + 0.5) * ts,
        (observation.own_start_tile.1 as f32 + 0.5) * ts,
    );
    note_goals(&decision.commands, memory);
    note_directed(&decision.commands, observation, &params, memory);
    let directed: BTreeSet<u32> = memory.directed.keys().copied().collect();
    let kinds: BTreeMap<u32, EntityKind> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit.kind))
        .collect();
    memory.goals.retain(|id, _| kinds.contains_key(id));
    // Units on another system's errand that an enemy is about to fight: the micro takes them.
    let broken: BTreeSet<u32> = if params.tank_break > 0.0 || params.unit_break > 0.0 {
        observation
            .owned
            .iter()
            .filter(|unit| {
                unit.hp > 0 && (reserved.contains(&unit.id) || directed.contains(&unit.id))
            })
            .filter(|unit| {
                let (range, tanks_only) = if unit.kind == EntityKind::Tank {
                    (params.tank_break, true)
                } else {
                    (params.unit_break, false)
                };
                range > 0.0
                    && observation.visible_enemies.iter().any(|enemy| {
                        enemy.hp > 0
                            && if tanks_only {
                                enemy.kind == EntityKind::Tank
                            } else {
                                fights(enemy.kind)
                            }
                            && dist((unit.x, unit.y), (enemy.x, enemy.y)) <= range * ts
                    })
            })
            .map(|unit| unit.id)
            .collect()
    } else {
        BTreeSet::new()
    };
    let eligible = |id: u32| {
        kinds.get(&id).is_some_and(|kind| params.steers_kind(*kind))
            && (broken.contains(&id) || (!reserved.contains(&id) && !directed.contains(&id)))
    };
    let plan = plan(observation, &params, memory, &eligible, home);
    if !plan.held.is_empty() {
        decision
            .commands
            .retain_mut(|command| strip(command, &plan.held));
    }
    for (unit, order) in &plan.orders {
        decision.commands.push(to_command(*unit, *order));
    }
    let ordered: BTreeSet<u32> = decision
        .commands
        .iter()
        .flat_map(|command| units_of(command).iter().copied())
        .collect();
    for unit in &plan.released {
        if ordered.contains(unit) {
            continue;
        }
        if let Some(goal) = memory.goals.get(unit) {
            decision.commands.push(if goal.attack {
                Command::AttackMove {
                    units: vec![*unit],
                    x: goal.x,
                    y: goal.y,
                    queued: false,
                }
            } else {
                Command::Move {
                    units: vec![*unit],
                    x: goal.x,
                    y: goal.y,
                    queued: false,
                }
            });
        }
    }
    decision.trace.matchup = Some(memory.trace());
}
