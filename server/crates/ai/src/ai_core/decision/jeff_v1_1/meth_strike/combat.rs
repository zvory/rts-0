//! The Meth strike's fights (`openfight`): the starters' fight in the middle, Rifleman by Rifleman,
//! their line short of it (`fightline`), and the volley count that says who wins.

use super::*;

/// `openfight`: the fight in the middle, Rifleman by Rifleman (see `PULL_HP`). It lasts while any
/// enemy fighter stands within `ENGAGE_TILES`: one walking away is never chased, the squad stands.
/// The V1.1 fight changes (`fightreach`, `fightstick`, `fighttrade`, `fightline`) are described at
/// `PULL_CLEAR_TILES`; `line_ok` lets the squad line up short of the fight (the Rush phase only).
#[allow(clippy::too_many_arguments)]
pub(super) fn fight(
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    state: &mut MethStrikeMemory,
    squad: &[&AiEntitySummary],
    near: &[&AiEntitySummary],
    line_ok: bool,
) {
    let reach_rule = fight_on(observation, "fightreach");
    let sticky = fight_on(observation, "fightstick");
    let trade = fight_on(observation, "fighttrade");
    let line = line_ok && fight_on(observation, "fightline");
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let weapon = rts_rules::combat::default_weapon_profile(EntityKind::Rifleman);
    let radius =
        rts_rules::defs::unit_def(EntityKind::Rifleman).map_or(9.0, |def| def.stats.radius);
    let reach = weapon.map_or(5.0, |w| w.range_tiles) * ts + radius + SIM_RANGE_SLACK_PX;
    let damage = weapon.map_or(5, |w| w.dmg).max(1);
    let mid = center(squad.iter().copied());
    // Enemies walking away from the squad since the last decision.
    let leaving: BTreeSet<u32> = near
        .iter()
        .filter(|enemy| {
            state.seen.get(&enemy.id).is_some_and(|&(x, y)| {
                let Some(out) = normalized_direction(mid, (enemy.x, enemy.y)) else {
                    return false;
                };
                (enemy.x - x as f32) * out.0 + (enemy.y - y as f32) * out.1 > LEAVING_TILES * ts
            })
        })
        .map(|enemy| enemy.id)
        .collect();
    state.seen = near
        .iter()
        .map(|enemy| (enemy.id, (enemy.x.round() as i32, enemy.y.round() as i32)))
        .collect();
    let in_reach = |unit: &AiEntitySummary, enemy: &AiEntitySummary| {
        dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(reach)
    };
    // A wounded Rifleman under fire steps back while any of the squad is still healthy
    // (`fighttrade`: unless the squad loses the fight anyway).
    state
        .pulled
        .retain(|id, until| *until > tick && squad.iter().any(|unit| unit.id == *id));
    let healthy = squad.iter().any(|unit| unit.hp > PULL_HP);
    for unit in squad {
        if !healthy || unit.hp > PULL_HP || state.pulled.contains_key(&unit.id) {
            continue;
        }
        if !near.iter().any(|enemy| enemy.target_id == Some(unit.id)) {
            continue;
        }
        // `fighttrade`: a fight the squad loses anyway is traded out, every rifle shooting (rule 1,
        // the default); rule 0 pulls only while the squad wins without the wounded Rifleman.
        if trade {
            let allowed = if fight_param("fpullrule", 1.0) == 0.0 {
                wins_without(squad, near, unit.id)
            } else {
                !loses(squad, near)
            };
            if !allowed {
                continue;
            }
        }
        let Some(threat) = near.iter().min_by(|a, b| {
            dist2(a.x, a.y, unit.x, unit.y)
                .total_cmp(&dist2(b.x, b.y, unit.x, unit.y))
                .then(a.id.cmp(&b.id))
        }) else {
            continue;
        };
        // `fighttrade` pulls just out of the enemy's reach (soupman's step; `fpulld` tiles instead
        // when set) for `TRADE_PULL_TICKS` (`fpullt`). Against Pressure AI, Rush AI and the shipped
        // fight on nine maps this beat the shipped 3 tiles for 1.5 s.
        let pull_tiles = if trade {
            fight_param("fpulld", 0.0)
        } else {
            PULL_TILES
        };
        let length = if pull_tiles <= 0.0 {
            let gap = dist2(threat.x, threat.y, unit.x, unit.y).det_sqrt();
            (reach + PULL_CLEAR_TILES * ts - gap).max(PULL_MIN_TILES * ts)
        } else {
            pull_tiles * ts
        };
        let to = away((threat.x, threat.y), (unit.x, unit.y), length, observation);
        if let Some(units) = actions::move_units(actions, [unit.id], to.0, to.1) {
            intents.push(AiIntent::Assemble { units });
        }
        let ticks = if trade {
            fight_param("fpullt", TRADE_PULL_TICKS as f32).max(1.0) as u32
        } else {
            PULL_TICKS
        };
        state.pulled.insert(unit.id, tick + ticks);
        state.aim.remove(&unit.id);
        state.holding.remove(&unit.id);
        state.advance.remove(&unit.id);
        state.lined.remove(&unit.id);
    }
    let fighters: Vec<&AiEntitySummary> = squad
        .iter()
        .copied()
        .filter(|unit| !state.pulled.contains_key(&unit.id))
        .collect();
    let anyone_in_reach = fighters
        .iter()
        .any(|unit| near.iter().any(|enemy| in_reach(unit, enemy)));
    // `fightline`: nobody in reach yet, so line up outside the enemy's reach and let them come.
    if line && !anyone_in_reach {
        hold_line(
            actions,
            intents,
            observation,
            analysis,
            state,
            &fighters,
            near,
            &leaving,
            reach,
        );
        return;
    }
    if line {
        state.line_since = None;
        state.slots.clear();
        state.lined.clear();
    }
    // One ranking for the whole squad, weakest first, then nearest the squad: every Rifleman with
    // the top enemy in reach shoots it, so the squad's shots land on one man (nine to a kill).
    let mut ranked: Vec<&AiEntitySummary> = near.to_vec();
    ranked.sort_by(|a, b| {
        a.hp.cmp(&b.hp)
            .then_with(|| dist2(a.x, a.y, mid.0, mid.1).total_cmp(&dist2(b.x, b.y, mid.0, mid.1)))
            .then(a.id.cmp(&b.id))
    });
    // At first contact the lead waits for the last to come up.
    let since = *state.contact_since.get_or_insert(tick);
    let gap = |unit: &AiEntitySummary| {
        near.iter()
            .map(|enemy| dist2(unit.x, unit.y, enemy.x, enemy.y).det_sqrt())
            .fold(f32::INFINITY, f32::min)
    };
    let last = fighters
        .iter()
        .map(|unit| gap(unit))
        .fold(0.0_f32, f32::max);
    let regrouping = !line && !anyone_in_reach && tick.saturating_sub(since) < REGROUP_TICKS;
    // Riflemen with fewer enemies in reach choose first; shots stop going to an enemy once those
    // already aimed at it cover its HP (`fightstick`: and one shot more).
    let mut choosers: Vec<(&AiEntitySummary, Vec<&AiEntitySummary>)> = fighters
        .iter()
        .map(|unit| {
            let options = near
                .iter()
                .copied()
                .filter(|enemy| in_reach(unit, enemy))
                .collect::<Vec<_>>();
            (*unit, options)
        })
        .collect();
    choosers.sort_by(|(a, a_options), (b, b_options)| {
        a_options.len().cmp(&b_options.len()).then(a.id.cmp(&b.id))
    });
    let spare = if sticky { damage } else { 0 };
    let mut committed: BTreeMap<u32, u32> = BTreeMap::new();
    for (unit, options) in choosers {
        if regrouping && last - gap(unit) > REGROUP_TILES * ts {
            if state.holding.insert(unit.id) {
                if let Some(units) = actions::hold_position_units(actions, [unit.id]) {
                    intents.push(AiIntent::Assemble { units });
                }
            }
            state.aim.remove(&unit.id);
            continue;
        }
        let open = |enemy: &&&AiEntitySummary| {
            committed.get(&enemy.id).copied().unwrap_or(0) < enemy.hp + spare
        };
        if options.is_empty() && reach_rule {
            // `fightreach`: nothing in reach, so walk on to the nearest enemy not walking away by
            // attack-move (it stops and shoots whatever comes into reach first), never after one
            // ordered target out of reach; or stand.
            let toward = near
                .iter()
                .filter(|enemy| !leaving.contains(&enemy.id))
                .min_by(|a, b| {
                    dist2(a.x, a.y, unit.x, unit.y)
                        .total_cmp(&dist2(b.x, b.y, unit.x, unit.y))
                        .then(a.id.cmp(&b.id))
                });
            if let Some(enemy) = toward {
                state.holding.remove(&unit.id);
                state.aim.remove(&unit.id);
                let fresh = state.advance.get(&unit.id) != Some(&enemy.id)
                    || unit.state == AiEntityState::Idle;
                if fresh {
                    if let Some(units) =
                        actions::attack_move_units(actions, [unit.id], enemy.x, enemy.y)
                    {
                        intents.push(AiIntent::Assemble { units });
                    }
                    state.advance.insert(unit.id, enemy.id);
                }
                continue;
            }
        }
        state.advance.remove(&unit.id);
        let target = if options.is_empty() {
            if reach_rule {
                None
            } else {
                // Nothing in reach: on to the top enemy not walking away, or stand.
                ranked
                    .iter()
                    .find(|enemy| !leaving.contains(&enemy.id))
                    .copied()
            }
        } else if sticky {
            // `fightstick`: a kill shot first; else the Rifleman's own target while it stays in
            // reach; else the weakest in reach.
            let kill = options
                .iter()
                .filter(|enemy| {
                    enemy.hp <= damage && committed.get(&enemy.id).copied().unwrap_or(0) < enemy.hp
                })
                .min_by(|a, b| {
                    a.hp.cmp(&b.hp)
                        .then_with(|| {
                            dist2(a.x, a.y, unit.x, unit.y)
                                .total_cmp(&dist2(b.x, b.y, unit.x, unit.y))
                        })
                        .then(a.id.cmp(&b.id))
                })
                .copied();
            let kept = state
                .aim
                .get(&unit.id)
                .and_then(|id| options.iter().find(|enemy| enemy.id == *id))
                .copied();
            kill.or(kept).or_else(|| {
                ranked
                    .iter()
                    .filter(|enemy| in_reach(unit, enemy))
                    .find(open)
                    .or_else(|| ranked.iter().find(|enemy| in_reach(unit, enemy)))
                    .copied()
            })
        } else {
            ranked
                .iter()
                .filter(|enemy| in_reach(unit, enemy))
                .find(open)
                .or_else(|| ranked.iter().find(|enemy| in_reach(unit, enemy)))
                .copied()
        };
        let Some(target) = target else {
            if state.holding.insert(unit.id) {
                if let Some(units) = actions::hold_position_units(actions, [unit.id]) {
                    intents.push(AiIntent::Assemble { units });
                }
            }
            state.aim.remove(&unit.id);
            continue;
        };
        state.holding.remove(&unit.id);
        if !options.is_empty() {
            *committed.entry(target.id).or_insert(0) += damage;
        }
        let engaged = state.aim.get(&unit.id) == Some(&target.id)
            && unit.target_id == Some(target.id)
            && unit.state == AiEntityState::Attack;
        if !engaged {
            if let Some(units) = actions::attack_units(actions, [unit.id], target.id) {
                intents.push(AiIntent::Attack { units });
            }
            state.aim.insert(unit.id, target.id);
        }
    }
}

/// `fightline`: with no enemy in reach yet, each Rifleman takes a slot on a line abreast facing the
/// enemies, `LINE_GAP_TILES` outside their reach, and holds there; enemies walking into it meet four
/// standing rifles. After `LINE_TICKS` with nobody coming, the line goes in together by attack-move.
/// Enemies all walking away are not followed.
#[allow(clippy::too_many_arguments)]
fn hold_line(
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    state: &mut MethStrikeMemory,
    fighters: &[&AiEntitySummary],
    near: &[&AiEntitySummary],
    leaving: &BTreeSet<u32>,
    reach: f32,
) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let coming: Vec<&AiEntitySummary> = near
        .iter()
        .copied()
        .filter(|enemy| !leaving.contains(&enemy.id))
        .collect();
    if coming.is_empty() || fighters.is_empty() {
        for unit in fighters {
            stand(actions, intents, state, unit.id);
        }
        return;
    }
    let mid = center(fighters.iter().copied());
    let foe = center(coming.iter().copied());
    let since = *state.line_since.get_or_insert(tick);
    if tick.saturating_sub(since) >= fight_param("flinet", LINE_TICKS as f32) as u32 {
        // Nobody came: in together, each toward the nearest coming enemy.
        for unit in fighters {
            let Some(enemy) = coming.iter().min_by(|a, b| {
                dist2(a.x, a.y, unit.x, unit.y)
                    .total_cmp(&dist2(b.x, b.y, unit.x, unit.y))
                    .then(a.id.cmp(&b.id))
            }) else {
                continue;
            };
            state.holding.remove(&unit.id);
            state.aim.remove(&unit.id);
            if state.advance.get(&unit.id) != Some(&enemy.id) || unit.state == AiEntityState::Idle {
                if let Some(units) =
                    actions::attack_move_units(actions, [unit.id], enemy.x, enemy.y)
                {
                    intents.push(AiIntent::Assemble { units });
                }
                state.advance.insert(unit.id, enemy.id);
            }
        }
        return;
    }
    // The line: through the squad's front, pulled back to stay outside the nearest enemy's reach.
    let Some(dir) = normalized_direction(mid, foe) else {
        return;
    };
    let side = (-dir.1, dir.0);
    let front = fighters
        .iter()
        .map(|unit| (unit.x - mid.0) * dir.0 + (unit.y - mid.1) * dir.1)
        .fold(f32::NEG_INFINITY, f32::max)
        .max(0.0);
    let mut centre = (mid.0 + dir.0 * front, mid.1 + dir.1 * front);
    let nearest = coming
        .iter()
        .map(|enemy| dist2(enemy.x, enemy.y, centre.0, centre.1).det_sqrt())
        .fold(f32::INFINITY, f32::min);
    let keep = reach + LINE_GAP_TILES * ts;
    if nearest < keep {
        let back = keep - nearest;
        centre = (centre.0 - dir.0 * back, centre.1 - dir.1 * back);
    }
    if state.slots.is_empty() || state.slots.len() != fighters.len() {
        let mut order: Vec<&AiEntitySummary> = fighters.to_vec();
        order.sort_by(|a, b| {
            ((a.x - mid.0) * side.0 + (a.y - mid.1) * side.1)
                .total_cmp(&((b.x - mid.0) * side.0 + (b.y - mid.1) * side.1))
                .then(a.id.cmp(&b.id))
        });
        let count = order.len() as f32;
        state.slots = order
            .iter()
            .enumerate()
            .map(|(index, unit)| {
                let offset = (index as f32 - (count - 1.0) / 2.0) * LINE_SPACING_TILES * ts;
                let at = (centre.0 + side.0 * offset, centre.1 + side.1 * offset);
                let at = analysis
                    .and_then(|analysis| analysis.open_ground_near((unit.x, unit.y), at, 1, 2))
                    .unwrap_or(at);
                let at = clamp_to_map(at, observation.map);
                (unit.id, stored(at))
            })
            .collect();
        state.lined.clear();
    }
    for unit in fighters {
        let Some(slot) = state.slots.get(&unit.id).copied().map(point) else {
            continue;
        };
        state.aim.remove(&unit.id);
        state.advance.remove(&unit.id);
        if dist2(unit.x, unit.y, slot.0, slot.1) <= squared(LINE_ARRIVED_TILES * ts) {
            stand(actions, intents, state, unit.id);
        } else if !state.lined.contains(&unit.id) || unit.state == AiEntityState::Idle {
            // Sent once (`lined`), again only if it stopped short.
            state.holding.remove(&unit.id);
            if let Some(units) = actions::move_units(actions, [unit.id], slot.0, slot.1) {
                intents.push(AiIntent::Assemble { units });
            }
            state.lined.insert(unit.id);
        }
    }
}

/// Whether the squad still wins the fight with `without` out of it (`fight_outcome`), counting
/// the enemies near; a vehicle among them is never a won fight for Riflemen.
fn wins_without(squad: &[&AiEntitySummary], near: &[&AiEntitySummary], without: u32) -> bool {
    if armored(near) {
        return false;
    }
    let ours = volley_side(&mut squad.iter().filter(|unit| unit.id != without));
    let theirs = volley_side(&mut near.iter());
    let (ours_left, theirs_left) = fight_outcome(ours, theirs);
    ours_left > 0 && theirs_left == 0
}

/// Whether the squad loses the fight with everyone in it (`fight_outcome`): then nobody steps back.
fn loses(squad: &[&AiEntitySummary], near: &[&AiEntitySummary]) -> bool {
    if armored(near) {
        return true;
    }
    let ours = volley_side(&mut squad.iter());
    let theirs = volley_side(&mut near.iter());
    fight_outcome(ours, theirs).0 == 0
}

pub(super) fn armored(near: &[&AiEntitySummary]) -> bool {
    near.iter()
        .any(|enemy| matches!(enemy.kind, EntityKind::Tank | EntityKind::ScoutCar))
}

/// Each armed unit as `(hp, damage in one Rifleman cooldown)`.
pub(super) fn volley_side(units: &mut dyn Iterator<Item = &&AiEntitySummary>) -> Vec<(u32, u32)> {
    units
        .filter_map(|unit| {
            let weapon = rts_rules::combat::default_weapon_profile(unit.kind)?;
            let per = weapon.dmg * RIFLE_COOLDOWN / weapon.cooldown.max(1);
            Some((unit.hp, per.max(1)))
        })
        .collect()
}

pub(super) const RIFLE_COOLDOWN: u32 = 32;

/// The units each side has left when both stand and shoot, one volley at a time, every shot of a
/// side on the other side's weakest: `(hp, damage per volley)` each.
pub(super) fn fight_outcome(
    mut ours: Vec<(u32, u32)>,
    mut theirs: Vec<(u32, u32)>,
) -> (usize, usize) {
    fn volley(shooters: &[(u32, u32)], targets: &mut [(u32, u32)]) {
        for &(hp, damage) in shooters {
            if hp == 0 {
                continue;
            }
            if let Some(target) = targets
                .iter_mut()
                .filter(|(hp, _)| *hp > 0)
                .min_by_key(|(hp, _)| *hp)
            {
                target.0 = target.0.saturating_sub(damage);
            }
        }
    }
    for _ in 0..200 {
        let alive = |side: &[(u32, u32)]| side.iter().filter(|(hp, _)| *hp > 0).count();
        if alive(&ours) == 0 || alive(&theirs) == 0 {
            break;
        }
        // Both sides fire at once: each shoots with the units it had before the volley.
        let theirs_before = theirs.clone();
        volley(&ours, &mut theirs);
        volley(&theirs_before, &mut ours);
    }
    (
        ours.iter().filter(|(hp, _)| *hp > 0).count(),
        theirs.iter().filter(|(hp, _)| *hp > 0).count(),
    )
}

/// Holds a Rifleman where it stands (once), with no aim or advance left over.
fn stand(
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
    state: &mut MethStrikeMemory,
    id: u32,
) {
    if state.holding.insert(id) {
        if let Some(units) = actions::hold_position_units(actions, [id]) {
            intents.push(AiIntent::Assemble { units });
        }
    }
    state.aim.remove(&id);
    state.advance.remove(&id);
}
