//! `pfdef`: the Panzerfaust pack's base guard (see the parent module docs). Off by default.

use super::*;

/// `pfdef`: one charge of the base guard at an enemy Tank.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct GuardCharge {
    /// When it began.
    start: u32,
    /// The Panzerfausts it needed when it began.
    need: usize,
    /// Whether its Panzerfausts have gone in (until then they gather `GUARD_STAGE_TILES` out).
    launched: bool,
    /// Whether it waits for Jeff's Tanks to reach the enemy Tank (`pfjoin`).
    join: bool,
}

/// `pfdef`: the base guard's orders (see the module docs).
pub(super) fn guard_orders(
    decision: &mut crate::ai_core::decision::AiDecision,
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    pack: &mut FaustPackMemory,
    (fausts, loaded, spent): (&[&AiEntitySummary], &[&AiEntitySummary], &BTreeSet<u32>),
) {
    use std::fmt::Write as _;
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let within =
        |a: (f32, f32), b: (f32, f32), tiles: f32| dist2(a.0, a.1, b.0, b.1) <= squared(tiles * ts);
    let at = |unit: &AiEntitySummary| (unit.x, unit.y);
    let depots = depots(observation);
    let enemy_tanks: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .collect();
    let own_tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .collect();
    // A Tank's cannon takes an enemy Tank within its reach over any infantry: an enemy Tank with a
    // Jeff Tank this close, or shooting at one, has its cannon held.
    let locked = |tank: &AiEntitySummary| {
        own_tanks
            .iter()
            .any(|own| within(at(own), at(tank), ENGAGE_TILES) || tank.target_id == Some(own.id))
    };
    let home_within = |tank: &AiEntitySummary, slack: f32| {
        depots
            .iter()
            .any(|depot| within(*depot, at(tank), HOME_RAID_TILES + slack))
    };
    let tank_of = |id: u32| enemy_tanks.iter().copied().find(|tank| tank.id == id);
    let infantry = |kind: EntityKind| {
        matches!(
            kind,
            EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::Panzerfaust
        )
    };
    let escort = |tank: &AiEntitySummary| {
        observation
            .visible_enemies
            .iter()
            .filter(|enemy| enemy.hp > 0 && infantry(enemy.kind))
            .filter(|enemy| within(at(enemy), at(tank), ESCORT_TILES))
            .count()
    };

    // Where the guard rests: behind the Depot an enemy Tank raids (or, failing that, is nearest to
    // within `THREAT_TILES`), else behind the most central Depot.
    let pair = |tiles: f32| {
        depots
            .iter()
            .flat_map(|depot| {
                enemy_tanks
                    .iter()
                    .map(move |tank| (*depot, (tank.x, tank.y)))
            })
            .filter(move |(depot, tank)| within(*depot, *tank, tiles))
            .min_by(|a, b| {
                dist2(a.0 .0, a.0 .1, a.1 .0, a.1 .1)
                    .total_cmp(&dist2(b.0 .0, b.0 .1, b.1 .0, b.1 .1))
            })
    };
    // The guarded Depot stays while an enemy Tank is still near it: picking the nearest pair afresh
    // every tick flipped it between bases and walked the guard past the raiders (3 Player Map).
    let near_current = |tiles: f32| {
        pack.guard.and_then(|((x, y), _, _)| {
            let depot = (x as f32, y as f32);
            enemy_tanks
                .iter()
                .filter(|tank| within(depot, at(tank), tiles + RAID_SLACK_TILES))
                .min_by(|a, b| {
                    dist2(depot.0, depot.1, a.x, a.y).total_cmp(&dist2(depot.0, depot.1, b.x, b.y))
                })
                .map(|tank| (depot, (tank.x, tank.y)))
        })
    };
    let raided = if pack.guard_raid {
        near_current(HOME_RAID_TILES).or_else(|| pair(HOME_RAID_TILES))
    } else {
        pair(HOME_RAID_TILES)
    };
    let threat = || {
        if pack.guard.is_some() && !pack.guard_raid {
            near_current(THREAT_TILES).or_else(|| pair(THREAT_TILES))
        } else {
            pair(THREAT_TILES)
        }
    };
    let keep_raided = raided.is_none()
        && pack.guard.is_some_and(|(_, _, when)| {
            pack.guard_raid && tick.saturating_sub(when) <= THREAT_MEMORY_TICKS
        });
    if let Some((depot, tank)) = raided {
        pack.guard = Some((
            (depot.0 as i32, depot.1 as i32),
            (tank.0 as i32, tank.1 as i32),
            tick,
        ));
        pack.guard_raid = true;
    } else if !keep_raided {
        if let Some((depot, tank)) = threat() {
            pack.guard = Some((
                (depot.0 as i32, depot.1 as i32),
                (tank.0 as i32, tank.1 as i32),
                tick,
            ));
            pack.guard_raid = false;
        }
    }
    if let Some((depot, _, when)) = pack.guard {
        let standing = depots.iter().any(|at| (at.0 as i32, at.1 as i32) == depot);
        if !standing || tick.saturating_sub(when) > THREAT_MEMORY_TICKS {
            pack.guard = None;
            pack.guard_raid = false;
        }
    }
    let central = {
        let spread = |from: &(f32, f32)| {
            depots
                .iter()
                .map(|other| dist2(from.0, from.1, other.0, other.1))
                .fold(0.0, f32::max)
        };
        depots
            .iter()
            .copied()
            .min_by(|a, b| spread(a).total_cmp(&spread(b)))
            .map(|central| {
                let forward = enemy_start(observation)
                    .and_then(|enemy| unit_vector(central, enemy))
                    .unwrap_or((1.0, 0.0));
                (central, forward)
            })
    };
    let rest = match pack.guard {
        Some((depot, tank, _)) => {
            let depot = (depot.0 as f32, depot.1 as f32);
            unit_vector(depot, (tank.0 as f32, tank.1 as f32)).map(|forward| (depot, forward))
        }
        None => central,
    };

    // Charges keep their Panzerfausts while the Tank is in sight at a base, or once they are
    // within `COMMIT_TILES` of it; a charge waiting on Jeff's Tanks gives up after `JOIN_TICKS`.
    pack.charges.retain(|target, charge| {
        tank_of(*target).is_some_and(|tank| {
            home_within(tank, RAID_SLACK_TILES)
                && (charge.launched
                    || !charge.join
                    || tick.saturating_sub(charge.start) <= JOIN_TICKS)
        })
    });
    let charges = &pack.charges;
    pack.charging.retain(|id, target| {
        let (Some(unit), Some(tank)) =
            (loaded.iter().find(|unit| unit.id == *id), tank_of(*target))
        else {
            return false;
        };
        within(at(unit), at(tank), GUARD_LEASH_TILES)
            && (charges.contains_key(target) || within(at(unit), at(tank), COMMIT_TILES))
    });
    let booked_targets: BTreeSet<u32> = pack.charging.values().copied().collect();
    pack.charges
        .retain(|target, _| booked_targets.contains(target));
    pack.joined.retain(|own, target| {
        pack.charges.get(target).is_some_and(|charge| charge.join)
            && own_tanks.iter().any(|tank| tank.id == *own)
    });

    // New charges, the Tank needing the fewest Panzerfausts first.
    let tank_hp = rts_rules::defs::unit_def(EntityKind::Tank).map_or(292, |def| def.stats.hp);
    let mut note = String::new();
    let mut candidates: Vec<(&AiEntitySummary, usize, Vec<u32>)> = Vec::new();
    let recent = super::push::recent_tank_spots(memory, tick, HIDDEN_TANK_TICKS);
    let lock_only = !off("pflock");
    for tank in enemy_tanks.iter().copied() {
        if !home_within(tank, 0.0) || pack.charges.contains_key(&tank.id) {
            continue;
        }
        let near: Vec<&AiEntitySummary> = enemy_tanks
            .iter()
            .copied()
            .filter(|other| other.id != tank.id && within(at(other), at(tank), DANGER_TILES))
            .collect();
        // Tanks out of sight now but seen near it lately count too: a 7-Tank group on Afrika was
        // charged when fog showed only two of them.
        let hidden = recent
            .iter()
            .filter(|(id, spot)| {
                *id != tank.id
                    && !enemy_tanks.iter().any(|seen| seen.id == *id)
                    && within(*spot, at(tank), DANGER_TILES)
            })
            .count();
        let escorts = escort(tank);
        let rockets = tank.hp.div_ceil(ROCKET_DAMAGE) as usize;
        // `pflock`: only a raider whose cannon is on a Jeff Tank is charged, the way the user
        // pictured it (the Tanks trading shots when the Panzerfausts run in); then only machine
        // guns within reach of the run-in threaten them.
        if lock_only {
            let close = near
                .iter()
                .filter(|other| within(at(other), at(tank), LOCK_NEAR_TILES))
                .count()
                + recent
                    .iter()
                    .filter(|(id, spot)| {
                        *id != tank.id
                            && !enemy_tanks.iter().any(|seen| seen.id == *id)
                            && within(*spot, at(tank), LOCK_NEAR_TILES)
                    })
                    .count();
            if !locked(tank) || rockets <= FINISH_ROCKETS || close > MAX_LOCK_NEAR {
                let _ = write!(
                    note,
                    " t{}:{}x{}n{}",
                    tank.id,
                    if locked(tank) { "L" } else { "u" },
                    rockets,
                    close
                );
                continue;
            }
            let need = rockets + close + escorts / 2;
            let _ = write!(
                note,
                " t{}:{}L r{}n{}e{}",
                tank.id, need, rockets, close, escorts
            );
            candidates.push((tank, need, Vec::new()));
            continue;
        }
        // A Tank a Jeff Tank is finishing is left to it; a melee of Tanks is left alone: each one's
        // machine gun fires at the Panzerfausts first (seven killed one every 0.7 s on Afrika).
        if (locked(tank) && rockets <= FINISH_ROCKETS) || near.len() + hidden > MAX_NEAR_TANKS {
            let _ = write!(note, " t{}:x{}n{}h{}", tank.id, rockets, near.len(), hidden);
            continue;
        }
        // `pfjoin`: Jeff's Tanks near it, at least one for every enemy Tank there, go in with them.
        let joiners: Vec<u32> = if !off("pfjoin") && !locked(tank) && escorts <= MAX_JOIN_ESCORT {
            let mut free: Vec<&&AiEntitySummary> = own_tanks
                .iter()
                .filter(|own| !pack.joined.contains_key(&own.id))
                .filter(|own| own.hp * 2 >= tank_hp)
                .filter(|own| within(at(own), at(tank), JOIN_TILES))
                .collect();
            free.sort_by(|a, b| {
                dist2(a.x, a.y, tank.x, tank.y)
                    .total_cmp(&dist2(b.x, b.y, tank.x, tank.y))
                    .then(a.id.cmp(&b.id))
            });
            if free.len() > near.len() {
                free.iter().take(near.len() + 2).map(|own| own.id).collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let held = locked(tank) || !joiners.is_empty();
        // Each cannon not held by a Jeff Tank may turn on them; each machine gun near it fires at them.
        let cannons = if held {
            0
        } else {
            1 + near.iter().filter(|other| !locked(other)).count()
        };
        let need = rockets + cannons + near.len() + escorts / 2;
        let _ = write!(
            note,
            " t{}:{}r{}c{}n{}e{}{}",
            tank.id,
            need,
            rockets,
            cannons,
            near.len(),
            escorts,
            if joiners.is_empty() {
                String::new()
            } else {
                format!("j{}", joiners.len())
            }
        );
        candidates.push((tank, need, joiners));
    }
    candidates.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.id.cmp(&b.0.id)));
    for (tank, need, joiners) in candidates {
        let mut free: Vec<&AiEntitySummary> = loaded
            .iter()
            .copied()
            .filter(|unit| !pack.charging.contains_key(&unit.id))
            .filter(|unit| within(at(unit), at(tank), GUARD_REACH_TILES))
            .collect();
        if free.len() < need {
            continue;
        }
        free.sort_by(|a, b| {
            dist2(a.x, a.y, tank.x, tank.y)
                .total_cmp(&dist2(b.x, b.y, tank.x, tank.y))
                .then(a.id.cmp(&b.id))
        });
        for unit in free.into_iter().take(need + 1) {
            pack.charging.insert(unit.id, tank.id);
        }
        let join = !joiners.is_empty();
        for own in joiners {
            pack.joined.insert(own, tank.id);
        }
        pack.charges.insert(
            tank.id,
            GuardCharge {
                start: tick,
                need,
                launched: false,
                join,
            },
        );
    }

    // A charge goes in once a Jeff Tank holds the enemy Tank's cannon, a Panzerfaust of it comes
    // within `LAUNCH_TILES`, or (not waiting on Jeff's Tanks) once `need` of them have gathered or
    // after `GATHER_TICKS`.
    let mut orders: Vec<Command> = Vec::new();
    let targets: Vec<u32> = pack.charges.keys().copied().collect();
    for target in targets {
        let (Some(tank), Some(mut charge)) = (tank_of(target), pack.charges.get(&target).cloned())
        else {
            continue;
        };
        let chargers: Vec<&AiEntitySummary> = pack
            .charging
            .iter()
            .filter(|(_, aim)| **aim == target)
            .filter_map(|(id, _)| loaded.iter().copied().find(|unit| unit.id == *id))
            .collect();
        if !charge.launched {
            let ready = chargers
                .iter()
                .filter(|unit| within(at(unit), at(tank), GUARD_STAGE_TILES + STAGE_SLACK_TILES))
                .count();
            // Even with the enemy Tank's cannon held, they gather first: let go at once, they came
            // in one at a time from up to 26 tiles and its machine gun shot them down.
            charge.launched = chargers
                .iter()
                .any(|unit| within(at(unit), at(tank), GUARD_LAUNCH_TILES))
                || (ready >= charge.need.min(chargers.len()) && (!charge.join || locked(tank)))
                || (!charge.join && tick.saturating_sub(charge.start) >= GATHER_TICKS);
            pack.charges.insert(target, charge.clone());
        }
        // They gather on an arc around the Tank, `GUARD_SPACING_TILES` apart, in the order they
        // come from, so one shell cannot splash several and their paths do not cross.
        let mut stage: BTreeMap<u32, (f32, f32)> = BTreeMap::new();
        if !charge.launched && !chargers.is_empty() {
            let n = chargers.len() as f32;
            let mid = (
                chargers.iter().map(|unit| unit.x).sum::<f32>() / n,
                chargers.iter().map(|unit| unit.y).sum::<f32>() / n,
            );
            let base = unit_vector(at(tank), mid).unwrap_or((0.0, 1.0));
            let angle = |unit: &AiEntitySummary| {
                let (dx, dy) = (unit.x - tank.x, unit.y - tank.y);
                (base.0 * dy - base.1 * dx).det_atan2(base.0 * dx + base.1 * dy)
            };
            let mut order: Vec<&&AiEntitySummary> = chargers.iter().collect();
            order.sort_by(|a, b| angle(a).total_cmp(&angle(b)).then(a.id.cmp(&b.id)));
            let step = GUARD_SPACING_TILES / GUARD_STAGE_TILES;
            for (index, unit) in order.into_iter().enumerate() {
                let offset = (index as f32 - (n - 1.0) / 2.0) * step;
                let (sin, cos) = offset.det_sin_cos();
                let dir = (base.0 * cos - base.1 * sin, base.0 * sin + base.1 * cos);
                stage.insert(
                    unit.id,
                    (
                        tank.x + dir.0 * GUARD_STAGE_TILES * ts,
                        tank.y + dir.1 * GUARD_STAGE_TILES * ts,
                    ),
                );
            }
        }
        for unit in chargers {
            if charge.launched {
                if unit.target_id != Some(target) {
                    orders.push(Command::Attack {
                        units: vec![unit.id],
                        target,
                        queued: false,
                    });
                }
                pack.sent.remove(&unit.id);
                continue;
            }
            if within(at(unit), at(tank), GUARD_STAGE_TILES) {
                if unit.state != AiEntityState::Idle {
                    orders.push(Command::HoldPosition {
                        units: vec![unit.id],
                        queued: false,
                    });
                }
                pack.sent
                    .insert(unit.id, (tick, (unit.x as i32, unit.y as i32)));
                continue;
            }
            let place = stage.get(&unit.id).copied().unwrap_or_else(|| {
                let away = unit_vector(at(tank), at(unit)).unwrap_or((0.0, 1.0));
                (
                    tank.x + away.0 * GUARD_STAGE_TILES * ts,
                    tank.y + away.1 * GUARD_STAGE_TILES * ts,
                )
            });
            send(
                &mut orders,
                pack,
                unit,
                clamp_to_map(place, observation.map),
                tick,
                ts,
            );
        }
    }
    // `pfjoin`: Jeff's joining Tanks take only the guard's orders while their charge lasts.
    let joined: BTreeSet<u32> = pack.joined.keys().copied().collect();
    if !joined.is_empty() {
        decision
            .commands
            .retain_mut(|command| super::matchup::strip(command, &joined));
        for (own, target) in &pack.joined {
            let aiming = own_tanks
                .iter()
                .any(|tank| tank.id == *own && tank.target_id == Some(*target));
            if !aiming {
                orders.push(Command::Attack {
                    units: vec![*own],
                    target: *target,
                    queued: false,
                });
            }
        }
    }

    // The rest stand behind the guarded Depot (the spent ones, riflemen now, behind the central
    // one). They never walk past a Tank on the way: with one close they back off from it, and one
    // whose way to its slot passes a Tank stays put; resting and spent ones walking across raiders
    // were half of the Panzerfausts lost in traced games.
    let clear_way = |from: (f32, f32), to: (f32, f32)| {
        enemy_tanks
            .iter()
            .all(|tank| segment_dist2(from, to, at(tank)) > squared(PATH_CLEAR_TILES * ts))
    };
    let resting: Vec<(&AiEntitySummary, Option<Rest>, usize)> = {
        let mut out = Vec::new();
        let free = loaded
            .iter()
            .copied()
            .filter(|unit| !pack.charging.contains_key(&unit.id));
        for (index, unit) in free.enumerate() {
            out.push((unit, rest, index));
        }
        let used = fausts
            .iter()
            .copied()
            .filter(|unit| spent.contains(&unit.id));
        for (index, unit) in used.enumerate() {
            out.push((unit, central, index));
        }
        out
    };
    for (unit, rest, index) in resting {
        let fighting = unit.target_id.is_some_and(|target| {
            observation.visible_enemies.iter().any(|enemy| {
                enemy.id == target && enemy.hp > 0 && within(at(unit), at(enemy), FIGHTING_TILES)
            })
        });
        if fighting && !spent.contains(&unit.id) {
            continue;
        }
        let threat = enemy_tanks
            .iter()
            .filter(|tank| within(at(unit), at(tank), SAFE_TILES))
            .min_by(|a, b| {
                dist2(unit.x, unit.y, a.x, a.y).total_cmp(&dist2(unit.x, unit.y, b.x, b.y))
            });
        if let Some(tank) = threat {
            let away = unit_vector(at(tank), at(unit)).unwrap_or((0.0, 1.0));
            let place = clamp_to_map(
                (
                    unit.x + away.0 * FLEE_TILES * ts,
                    unit.y + away.1 * FLEE_TILES * ts,
                ),
                observation.map,
            );
            send(&mut orders, pack, unit, place, tick, ts);
            continue;
        }
        let Some(rest) = rest else {
            continue;
        };
        let place = guard_slot(observation, rest, index);
        if !clear_way(at(unit), place) {
            if unit.state == AiEntityState::Move {
                orders.push(Command::HoldPosition {
                    units: vec![unit.id],
                    queued: false,
                });
                pack.sent.remove(&unit.id);
            }
            continue;
        }
        send(&mut orders, pack, unit, place, tick, ts);
    }
    decision.trace.notes.push(format!(
        "pfdef loaded={} spent={} rest={} guard={} charging={} raid={} charges={} joined={}{} trained={}",
        loaded.len(),
        spent.len(),
        rest.map_or_else(
            || "-".to_string(),
            |(place, _)| format!("{:.0},{:.0}", place.0 / ts, place.1 / ts)
        ),
        u8::from(pack.guard.is_some()) + u8::from(pack.guard_raid),
        pack.charging.len(),
        u8::from(pack.raid),
        pack.charges
            .iter()
            .map(|(target, charge)| format!(
                "t{}/{}{}{}",
                target,
                charge.need,
                if charge.launched { "L" } else { "" },
                if charge.join { "J" } else { "" }
            ))
            .collect::<Vec<_>>()
            .join(","),
        pack.joined.len(),
        note,
        pack.trained
    ));
    decision.commands.extend(orders);
}

/// `pfdef`: slot `index` of the base guard behind `center` facing `forward`, spaced past a Tank
/// shell's splash (1.4 tiles): one shell into the 1.2-tile pack slots hit three at once.
fn guard_slot(
    observation: &AiObservation,
    (center, forward): ((f32, f32), (f32, f32)),
    index: usize,
) -> (f32, f32) {
    let ts = observation.map.tile_size as f32;
    let side = (-forward.1, forward.0);
    let row = (index / SLOT_ROW) as f32;
    let column = (index % SLOT_ROW) as f32 - (SLOT_ROW as f32 - 1.0) / 2.0;
    let back = (BACK_TILES + row * GUARD_SPACING_TILES) * ts;
    let across = column * GUARD_SPACING_TILES * ts;
    clamp_to_map(
        (
            center.0 - forward.0 * back + side.0 * across,
            center.1 - forward.1 * back + side.1 * across,
        ),
        observation.map,
    )
}
