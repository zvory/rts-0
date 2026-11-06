//! Jeff's AI V1.1's Meth strike (`methstrike`): the four starting Riflemen rush across the map, win
//! the fight in the middle, then sneak round by the enemy natural to snipe the enemy's extractors.
//! The opening's new Riflemen stay home. Meth is done at 1:20; from 1:20 to 2:30 Pressure AI keeps
//! one Engineer and one or two Riflemen within 25 tiles of its Depot and Rush AI nothing, while
//! their fighters stand out on the map. A Steel Mine has 37 HP and a Pump Jack 56, and small arms
//! do a building a quarter damage. Sent straight down the route, the starters ran into the enemy's
//! stream of Riflemen at 50-70% of the way and died with nothing killed in all 16 games.
//!
//! - **Rush**: the starters take the strike at the start and walk one group route straight at the
//!   enemy main (points about `PATH_STRIDE_TILES` apart, so they do not split around walls and
//!   water). Enemy fighters within `ENGAGE_TILES` of any of them get the whole squad on the
//!   weakest, no stepping back. With `MIN_SQUAD` or more left once nothing is near after that
//!   fight, or past `SNEAK_FROM` of the way without one, they sneak.
//! - **Sneak**: on to a point `SIDE_TILES` beside the enemy natural on its far side from the
//!   straight line between the mains, then to a flank point `FLANK_TILES` beside the enemy main on
//!   that side, keeping on through anything they meet (Meth Riflemen fire on the move).
//! - **Raid**: within `BASE_TILES` of the enemy main or an enemy Depot in sight they shoot
//!   extractors only, the nearest, all on one, never a Depot or other building, and none in reach
//!   of a set-up gun or a Tank. Enemy fighters within `CHASE_TILES` are run from, to the extractor
//!   farthest from them or straight away, shooting on the way. With nothing left to shoot they go
//!   on to the flank point, then the main.
//! - **Home**: after `RAID_TICKS`, or when fewer than `MIN_SQUAD` are left or nothing is left at
//!   the enemy main.
//!
//! A Rifleman that stands still for `STALL_TICKS` since its last path gets it again. The orders go
//! out as assembly, as the home pocket's stage orders would otherwise drop them.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared, tile_center};

/// The path keeps a group route point about this often.
const PATH_STRIDE_TILES: f32 = 10.0;
/// A Rifleman standing still this long since its last path gets it again.
const STALL_TICKS: u32 = config::TICK_HZ * 2;
/// Enemy fighters this close to a starter are the fight in the middle.
const ENGAGE_TILES: f32 = 7.0;
/// The starters sneak once this far across without a fight.
const SNEAK_FROM: f32 = 0.5;
/// The sneak's point beside the enemy natural, and its flank point beside the enemy main.
const SIDE_TILES: f32 = 10.0;
const FLANK_TILES: f32 = 15.0;
/// A natural this close to the straight line between the mains has no far side: the side with
/// less remembered enemy strength is taken.
const ON_LINE_TILES: f32 = 4.0;
/// A sneak point is passed once this close.
const PASSED_TILES: f32 = 6.0;
/// The raid starts this close to the enemy main or an enemy Depot.
const BASE_TILES: f32 = 16.0;
/// Extractors this close to an enemy Depot or the enemy start are the raid's.
const FIELD_TILES: f32 = 16.0;
/// Enemy fighters this close to a raider chase it.
const CHASE_TILES: f32 = 8.0;
/// Extractors this close to a set-up gun or a Tank are left alone.
const COVERED_TILES: f32 = 7.5;
/// The raid runs only to extractors at least this far from every chaser, else this far away.
const RUN_CLEAR_TILES: f32 = 8.0;
const RUN_TILES: f32 = 12.0;
const RAID_TICKS: u32 = config::TICK_HZ * 90;
const MIN_SQUAD: usize = 2;
/// Released once this close to home.
const HOME_TILES: f32 = 12.0;
/// `wideswing`, soupman's way round (replays 461, 462): out of the fight in the middle the squad
/// turns for the map edge, `EDGE_TILES` short of it, then follows the edge to the flank point beside
/// the enemy main. Of the edge points on either side of the line between the mains it takes the
/// cheapest way round: tiles walked, plus `THREAT_TILES_PER_VALUE` tiles for every point of
/// remembered enemy army within `SIDE_THREAT_TILES` of each leg's middle and end. soupman's starters
/// turned north 15 tiles clear of Pressure AI's army and passed it 13-31 tiles off; V1.1's walked
/// up beside it, 6-8 tiles off for 40 s.
const EDGE_TILES: f32 = 6.0;
const EDGE_STEP_TILES: f32 = 4.0;
const THREAT_TILES_PER_VALUE: f32 = 0.1;
const SIDE_THREAT_TILES: f32 = 12.0;
/// Edge points further than this, or this far back toward Jeff's own side, are no way round; with
/// none left the squad turns `OUT_FALLBACK_TILES` straight off the line.
const MAX_OUT_TILES: f32 = 70.0;
const BACK_TILES: f32 = 5.0;
const OUT_FALLBACK_TILES: f32 = 25.0;
/// After the raid the squad goes for the enemy natural's Engineers (soupman's last Rifleman killed
/// the one walking out to build it), giving up after `NATURAL_WAIT_TICKS` there with none in sight.
const NATURAL_REACHED_TILES: f32 = 8.0;
const WORKER_TILES: f32 = 14.0;
const NATURAL_WAIT_TICKS: u32 = config::TICK_HZ * 15;
/// `openfight`: the fight in the middle Rifleman by Rifleman. All four on the weakest enemy lost the
/// first fight on Crossroads, Schone Tage and The River in every game of the b4 120 (Jeff lost 108
/// starters and killed 56 vs Pressure there, 110 for 10 vs Rush): the squad walked in single file so
/// the lead met all four enemies alone, and once an enemy Rifleman was wounded Pressure and Rush walked
/// it away while all four followed it 5.5-7 tiles back, out of their own reach, past the three still
/// shooting them from 1-3 tiles. Now each Rifleman shoots the weakest enemy in its reach, spreading
/// shots so none are wasted; with nothing in reach it goes for the nearest enemy not walking away; the
/// lead waits for the rest at first contact; and a wounded Rifleman under fire steps back.
/// A Rifleman at or below `PULL_HP` that an enemy shoots at steps `PULL_TILES` away for `PULL_TICKS`.
const PULL_HP: u32 = 15;
const PULL_TILES: f32 = 3.0;
const PULL_TICKS: u32 = 45;
/// At first contact a Rifleman this much nearer the enemy than the squad's last holds, for at most
/// `REGROUP_TICKS`.
const REGROUP_TILES: f32 = 2.0;
const REGROUP_TICKS: u32 = config::TICK_HZ * 2;
/// An enemy that moved this far away from the squad since the last decision is walking away.
const LEAVING_TILES: f32 = 0.15;
/// Mirrors the simulation combat service's range slack.
const SIM_RANGE_SLACK_PX: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    Rush,
    Sneak,
    Raid,
    /// `wideswing`: after the raid, at the enemy natural's Engineers.
    Natural,
    Home,
    Done,
}

/// V1.1's Meth strike: the starters, the way round, and the raid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct MethStrikeMemory {
    phase: Phase,
    squad: BTreeSet<u32>,
    /// When each was last sent down its path (`None`: it needs one).
    sent: BTreeMap<u32, Option<u32>>,
    /// The squad has fought in the middle.
    fought: bool,
    /// The sneak's points beside the natural and the main, and whether the first is passed.
    side: Option<(i32, i32)>,
    flank: Option<(i32, i32)>,
    passed_side: bool,
    passed_flank: bool,
    raid_since: Option<u32>,
    /// Why the strike went home, for the decision log.
    why: &'static str,
    /// `wideswing`: the enemy natural's field, and whether the squad has been there after the raid.
    natural: Option<(i32, i32)>,
    at_natural_since: Option<u32>,
    /// `openfight`: wounded Riflemen stepping back, until when; each Rifleman's last target; where
    /// each enemy near stood at the last decision; when the first contact began; who holds for the
    /// rest to come up.
    pulled: BTreeMap<u32, u32>,
    aim: BTreeMap<u32, u32>,
    seen: BTreeMap<u32, (i32, i32)>,
    contact_since: Option<u32>,
    holding: BTreeSet<u32>,
}

impl MethStrikeMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.squad.contains(&id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.squad.iter().copied()
    }

    /// For the decision log.
    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        format!(
            "{:?} squad={:?} fought={} side={:?} flank={:?} why={}",
            self.phase, self.squad, self.fought, self.side, self.flank, self.why
        )
    }
}

/// Runs the strike (see the module docs).
pub(in crate::ai_core::decision) fn strike(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
) {
    if !is_jeff_v1_1(profile_id) || super::off("methstrike") {
        memory.jeff_v1_1.meth_strike = MethStrikeMemory::default();
        return;
    }
    let ts = observation.map.tile_size as f32;
    let Some(enemy) = observation
        .players
        .iter()
        .filter(|p| p.is_alive && observation.is_enemy_player(p.id))
        .min_by_key(|p| p.id)
    else {
        return;
    };
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy_main = tile_center(enemy.start_tile, observation.map.tile_size);
    if memory.jeff_v1_1.meth_strike.phase == Phase::Idle
        && memory.opening_rush.out_squad().is_some()
    {
        let starters = memory.opening_rush.hand_over();
        let state = &mut memory.jeff_v1_1.meth_strike;
        for id in starters {
            state.squad.insert(id);
            state.sent.insert(id, None);
        }
        state.phase = Phase::Rush;
    }
    // The sneak's points, worked out once. `wideswing` works them out from where the squad stands
    // while it rushes, so they hold where it leaves the fight.
    let wide = !super::off("wideswing");
    if wide && memory.jeff_v1_1.meth_strike.phase == Phase::Rush {
        let from =
            center(observation.owned.iter().filter(|unit| {
                memory.jeff_v1_1.meth_strike.squad.contains(&unit.id) && unit.hp > 0
            }));
        let (side, flank) = swing_points(observation, map_analysis, memory, from, main, enemy_main);
        let natural = enemy_natural(observation, enemy_main);
        let state = &mut memory.jeff_v1_1.meth_strike;
        state.side = Some(stored(side));
        state.flank = Some(stored(flank));
        state.natural = Some(stored(natural));
    } else if memory.jeff_v1_1.meth_strike.phase == Phase::Rush
        && memory.jeff_v1_1.meth_strike.flank.is_none()
    {
        let (side, flank) = sneak_points(observation, map_analysis, memory, main, enemy_main);
        let state = &mut memory.jeff_v1_1.meth_strike;
        state.side = Some(stored(side));
        state.flank = Some(stored(flank));
    }
    let squad: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| memory.jeff_v1_1.meth_strike.squad.contains(&unit.id) && unit.hp > 0)
        .collect();
    let state = &mut memory.jeff_v1_1.meth_strike;
    state.squad = squad.iter().map(|unit| unit.id).collect();
    state.sent.retain(|id, _| state.squad.contains(id));
    let ids: Vec<u32> = squad.iter().map(|unit| unit.id).collect();
    let mid = center(squad.iter().copied());
    if matches!(state.phase, Phase::Rush | Phase::Sneak | Phase::Raid) && squad.len() < MIN_SQUAD {
        state.phase = Phase::Home;
        state.why = "losses";
    }
    let fighters: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .collect();
    let side = state.side.map(point).unwrap_or(enemy_main);
    let flank = state.flank.map(point).unwrap_or(enemy_main);
    match state.phase {
        Phase::Idle | Phase::Done => {}
        Phase::Home => {
            if ids.is_empty() || dist2(mid.0, mid.1, main.0, main.1) <= squared(HOME_TILES * ts) {
                state.phase = Phase::Done;
                state.squad.clear();
                if !ids.is_empty() {
                    intents.push(AiIntent::Assemble { units: ids });
                }
            } else if let Some(units) = actions::move_units(actions, ids, main.0, main.1) {
                intents.push(AiIntent::Assemble { units });
            }
        }
        Phase::Rush => {
            // The fight in the middle: the whole squad on the weakest enemy near it.
            let near: Vec<&AiEntitySummary> = fighters
                .iter()
                .copied()
                .filter(|enemy| {
                    squad.iter().any(|unit| {
                        dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(ENGAGE_TILES * ts)
                    })
                })
                .collect();
            if !super::off("openfight") {
                if !near.is_empty() {
                    fight(actions, intents, observation, state, &squad, &near);
                    state.fought = true;
                    for id in &ids {
                        state.sent.insert(*id, None);
                    }
                    return;
                }
            } else if let Some(target) = near
                .iter()
                .min_by(|a, b| a.hp.cmp(&b.hp).then(a.id.cmp(&b.id)))
            {
                state.fought = true;
                if let Some(units) = actions::attack_units(actions, ids.iter().copied(), target.id)
                {
                    intents.push(AiIntent::Attack { units });
                }
                for id in &ids {
                    state.sent.insert(*id, None);
                }
                return;
            }
            state.holding.clear();
            let across = share(mid, main, enemy_main);
            if state.fought || across >= SNEAK_FROM {
                state.phase = Phase::Sneak;
                for id in &ids {
                    state.sent.insert(*id, None);
                }
            } else {
                send(
                    actions,
                    intents,
                    observation,
                    map_analysis,
                    state,
                    &squad,
                    &[enemy_main],
                );
                return;
            }
            sneak(
                actions,
                intents,
                observation,
                map_analysis,
                state,
                &squad,
                side,
                flank,
                enemy_main,
            );
        }
        Phase::Sneak => {
            // `openfight`: before Meth a Rifleman cannot shoot on the move, so the sneak stops and
            // fights what comes near instead of walking on past it. On Classic two wounded Pressure
            // Riflemen that stepped back out of the fight shot the sneaking squad for 20 s.
            let meth = observation
                .upgrades
                .contains(&UpgradeKind::Methamphetamines);
            if !super::off("openfight") && !meth {
                let near: Vec<&AiEntitySummary> = fighters
                    .iter()
                    .copied()
                    .filter(|enemy| {
                        squad.iter().any(|unit| {
                            dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(ENGAGE_TILES * ts)
                        })
                    })
                    .collect();
                if !near.is_empty() {
                    fight(actions, intents, observation, state, &squad, &near);
                    for id in &ids {
                        state.sent.insert(*id, None);
                    }
                    return;
                }
            }
            state.holding.clear();
            sneak(
                actions,
                intents,
                observation,
                map_analysis,
                state,
                &squad,
                side,
                flank,
                enemy_main,
            );
        }
        Phase::Raid => raid(
            actions,
            observation,
            intents,
            state,
            &squad,
            &fighters,
            enemy_main,
            flank,
        ),
        Phase::Natural => to_natural(actions, observation, intents, state, &squad, &fighters),
    }
}

/// `openfight`: the fight in the middle, Rifleman by Rifleman (see `PULL_HP`). It lasts while any
/// enemy fighter stands within `ENGAGE_TILES`: one walking away is never chased, the squad stands.
fn fight(
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
    observation: &AiObservation,
    state: &mut MethStrikeMemory,
    squad: &[&AiEntitySummary],
    near: &[&AiEntitySummary],
) {
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
    // A wounded Rifleman under fire steps back while any of the squad is still healthy.
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
        let Some(threat) = near.iter().min_by(|a, b| {
            dist2(a.x, a.y, unit.x, unit.y)
                .total_cmp(&dist2(b.x, b.y, unit.x, unit.y))
                .then(a.id.cmp(&b.id))
        }) else {
            continue;
        };
        let to = away(
            (threat.x, threat.y),
            (unit.x, unit.y),
            PULL_TILES * ts,
            observation,
        );
        if let Some(units) = actions::move_units(actions, [unit.id], to.0, to.1) {
            intents.push(AiIntent::Assemble { units });
        }
        state.pulled.insert(unit.id, tick + PULL_TICKS);
        state.aim.remove(&unit.id);
        state.holding.remove(&unit.id);
    }
    let fighters: Vec<&AiEntitySummary> = squad
        .iter()
        .copied()
        .filter(|unit| !state.pulled.contains_key(&unit.id))
        .collect();
    let anyone_in_reach = fighters
        .iter()
        .any(|unit| near.iter().any(|enemy| in_reach(unit, enemy)));
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
            .map(|enemy| dist2(unit.x, unit.y, enemy.x, enemy.y).sqrt())
            .fold(f32::INFINITY, f32::min)
    };
    let last = fighters
        .iter()
        .map(|unit| gap(unit))
        .fold(0.0_f32, f32::max);
    let regrouping = !anyone_in_reach && tick.saturating_sub(since) < REGROUP_TICKS;
    // Riflemen with fewer enemies in reach choose first; shots stop going to an enemy once those
    // already aimed at it cover its HP.
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
        let open =
            |enemy: &&&AiEntitySummary| committed.get(&enemy.id).copied().unwrap_or(0) < enemy.hp;
        let target = if options.is_empty() {
            // Nothing in reach: on to the top enemy not walking away, or stand.
            ranked
                .iter()
                .find(|enemy| !leaving.contains(&enemy.id))
                .copied()
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

/// On round by the natural to the flank point, into the raid once at an enemy base.
#[allow(clippy::too_many_arguments)]
fn sneak(
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    state: &mut MethStrikeMemory,
    squad: &[&AiEntitySummary],
    side: (f32, f32),
    flank: (f32, f32),
    enemy_main: (f32, f32),
) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let mid = center(squad.iter().copied());
    let near_base = |at: (f32, f32)| {
        dist2(at.0, at.1, enemy_main.0, enemy_main.1) <= squared(BASE_TILES * ts)
            || observation.visible_enemies.iter().any(|enemy| {
                enemy.hp > 0
                    && enemy.kind == EntityKind::ResourceDepot
                    && dist2(at.0, at.1, enemy.x, enemy.y) <= squared(BASE_TILES * ts)
            })
    };
    if near_base(mid) {
        state.phase = Phase::Raid;
        state.raid_since = Some(tick);
        return;
    }
    if dist2(mid.0, mid.1, side.0, side.1) <= squared(PASSED_TILES * ts) {
        state.passed_side = true;
    }
    let mut legs = Vec::new();
    if !state.passed_side {
        legs.push(side);
    }
    legs.push(flank);
    send(actions, intents, observation, analysis, state, squad, &legs);
}

/// Sends the squad members that need it down one queued group route through `legs`, as one group
/// order from their centre, so they keep together.
fn send(
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    state: &mut MethStrikeMemory,
    squad: &[&AiEntitySummary],
    legs: &[(f32, f32)],
) {
    let tick = observation.tick;
    let due: Vec<&AiEntitySummary> = squad
        .iter()
        .copied()
        .filter(|unit| {
            let sent = state.sent.get(&unit.id).copied().flatten();
            let stalled = sent.is_some_and(|at| {
                unit.state == AiEntityState::Idle && tick.saturating_sub(at) >= STALL_TICKS
            });
            sent.is_none() || stalled
        })
        .collect();
    if due.is_empty() {
        return;
    }
    let ids: Vec<u32> = due.iter().map(|unit| unit.id).collect();
    let mut from = center(due.iter().copied());
    let mut way = Vec::new();
    for leg in legs {
        way.extend(path(observation, analysis, from, *leg));
        from = *leg;
    }
    for (index, leg) in way.iter().enumerate() {
        if let Some(units) =
            actions::move_units_with_queue(actions, ids.iter().copied(), leg.0, leg.1, index > 0)
        {
            intents.push(AiIntent::Assemble { units });
        }
    }
    for id in ids {
        state.sent.insert(id, Some(tick));
    }
}

/// In an enemy base: extractors only, running from chasers.
#[allow(clippy::too_many_arguments)]
fn raid(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    intents: &mut Vec<AiIntent>,
    state: &mut MethStrikeMemory,
    squad: &[&AiEntitySummary],
    fighters: &[&AiEntitySummary],
    enemy_main: (f32, f32),
    flank: (f32, f32),
) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    if state
        .raid_since
        .is_some_and(|since| tick.saturating_sub(since) >= RAID_TICKS)
    {
        state.phase = Phase::Home;
        state.why = "time";
        return;
    }
    let ids: Vec<u32> = squad.iter().map(|unit| unit.id).collect();
    let mid = center(squad.iter().copied());
    let depots: Vec<(f32, f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind == EntityKind::ResourceDepot)
        .map(|enemy| (enemy.x, enemy.y))
        .chain(std::iter::once(enemy_main))
        .collect();
    let covered = |x: f32, y: f32| {
        fighters.iter().any(|enemy| {
            matches!(enemy.kind, EntityKind::MachineGunner | EntityKind::Tank)
                && enemy.state != AiEntityState::Move
                && dist2(enemy.x, enemy.y, x, y) <= squared(COVERED_TILES * ts)
        })
    };
    let targets: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            enemy.hp > 0 && matches!(enemy.kind, EntityKind::SteelMine | EntityKind::PumpJack)
        })
        .filter(|enemy| {
            depots
                .iter()
                .any(|depot| dist2(enemy.x, enemy.y, depot.0, depot.1) <= squared(FIELD_TILES * ts))
        })
        .filter(|enemy| !covered(enemy.x, enemy.y))
        .collect();
    let chasers: Vec<&AiEntitySummary> = fighters
        .iter()
        .copied()
        .filter(|enemy| {
            squad
                .iter()
                .any(|unit| dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(CHASE_TILES * ts))
        })
        .collect();
    if !chasers.is_empty() {
        let threat = center(chasers.iter().copied());
        let clear = |x: f32, y: f32| {
            chasers
                .iter()
                .all(|enemy| dist2(enemy.x, enemy.y, x, y) >= squared(RUN_CLEAR_TILES * ts))
        };
        let run = targets
            .iter()
            .filter(|target| clear(target.x, target.y))
            .max_by(|a, b| {
                dist2(a.x, a.y, threat.0, threat.1)
                    .total_cmp(&dist2(b.x, b.y, threat.0, threat.1))
                    .then(b.id.cmp(&a.id))
            })
            .map(|target| (target.x, target.y))
            .unwrap_or_else(|| away(threat, mid, RUN_TILES * ts, observation));
        if let Some(units) = actions::move_units(actions, ids, run.0, run.1) {
            intents.push(AiIntent::Assemble { units });
        }
        return;
    }
    let target = targets.iter().min_by(|a, b| {
        dist2(a.x, a.y, mid.0, mid.1)
            .total_cmp(&dist2(b.x, b.y, mid.0, mid.1))
            .then(a.id.cmp(&b.id))
    });
    if let Some(target) = target {
        if let Some(units) = actions::attack_units(actions, ids, target.id) {
            intents.push(AiIntent::Attack { units });
        }
        return;
    }
    // Nothing left here: on to the flank point, then the main.
    if dist2(mid.0, mid.1, flank.0, flank.1) <= squared(PASSED_TILES * ts) {
        state.passed_flank = true;
    }
    if dist2(mid.0, mid.1, enemy_main.0, enemy_main.1) <= squared(PASSED_TILES * ts) {
        // `wideswing`: on to the natural's Engineers first.
        if state.natural.is_some() && !super::off("wideswing") {
            state.phase = Phase::Natural;
            state.at_natural_since = None;
            return;
        }
        state.phase = Phase::Home;
        state.why = "bare";
        return;
    }
    let next = if state.passed_flank {
        enemy_main
    } else {
        flank
    };
    if let Some(units) = actions::move_units(actions, ids, next.0, next.1) {
        intents.push(AiIntent::Assemble { units });
    }
}

/// The sneak's point beside the enemy natural, on its far side from the straight line between the
/// mains, and the flank point beside the enemy main on the same side.
fn sneak_points(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &AiDecisionMemory,
    main: (f32, f32),
    enemy_main: (f32, f32),
) -> ((f32, f32), (f32, f32)) {
    let ts = observation.map.tile_size as f32;
    let start_exclusion = (config::START_RESOURCE_MAX_DIST_TILES + 1.5) * ts;
    let natural = observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
        .filter(|resource| {
            dist2(resource.x, resource.y, enemy_main.0, enemy_main.1) > squared(start_exclusion)
        })
        .min_by(|a, b| {
            dist2(a.x, a.y, enemy_main.0, enemy_main.1)
                .total_cmp(&dist2(b.x, b.y, enemy_main.0, enemy_main.1))
                .then(a.id.cmp(&b.id))
        })
        .map(|resource| (resource.x, resource.y))
        .unwrap_or(enemy_main);
    let line = normalized_direction(main, enemy_main).unwrap_or((1.0, 0.0));
    let across = (-line.1, line.0);
    let offset = (natural.0 - main.0) * across.0 + (natural.1 - main.1) * across.1;
    let at = |base: (f32, f32), sign: f32, tiles: f32| {
        clamp_to_map(
            (
                base.0 + across.0 * sign * tiles * ts,
                base.1 + across.1 * sign * tiles * ts,
            ),
            observation.map,
        )
    };
    let sign = if offset.abs() >= ON_LINE_TILES * ts {
        offset.signum()
    } else {
        let threat = |sign: f32| {
            super::push::threat_near(observation, memory, at(natural, sign, SIDE_TILES), 12.0)
                + super::push::threat_near(
                    observation,
                    memory,
                    at(enemy_main, sign, FLANK_TILES),
                    12.0,
                )
        };
        if threat(1.0) <= threat(-1.0) {
            1.0
        } else {
            -1.0
        }
    };
    let snap = |point: (f32, f32)| {
        analysis
            .and_then(|analysis| analysis.open_ground_near(main, point, 2, 4))
            .unwrap_or(point)
    };
    (
        snap(at(natural, sign, SIDE_TILES)),
        snap(at(enemy_main, sign, FLANK_TILES)),
    )
}

/// Group route points about `PATH_STRIDE_TILES` apart from `from` to `to`, or `to` alone without a
/// map analysis.
fn path(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    from: (f32, f32),
    to: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let Some(analysis) = analysis else {
        return vec![to];
    };
    let way = analysis.compact_group_route(from, to, 4);
    let mut kept: Vec<(f32, f32)> = Vec::new();
    for at in &way {
        if kept
            .last()
            .is_none_or(|last| dist2(last.0, last.1, at.0, at.1) >= squared(PATH_STRIDE_TILES * ts))
        {
            kept.push(*at);
        }
    }
    if let (Some(last), Some(end)) = (kept.last(), way.last()) {
        if last != end {
            kept.push(*end);
        }
    }
    if kept.is_empty() {
        kept.push(to);
    }
    kept
}

/// How far `at` is along the way from `main` to `enemy_main`, 0 to 1.
fn share(at: (f32, f32), main: (f32, f32), enemy_main: (f32, f32)) -> f32 {
    let span2 = dist2(main.0, main.1, enemy_main.0, enemy_main.1).max(1.0);
    ((at.0 - main.0) * (enemy_main.0 - main.0) + (at.1 - main.1) * (enemy_main.1 - main.1)) / span2
}

/// `wideswing`: after the raid, on to the enemy natural's field and the Engineers there; home once
/// none have shown there for `NATURAL_WAIT_TICKS`, or when enemy fighters close in.
fn to_natural(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    intents: &mut Vec<AiIntent>,
    state: &mut MethStrikeMemory,
    squad: &[&AiEntitySummary],
    fighters: &[&AiEntitySummary],
) {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let ids: Vec<u32> = squad.iter().map(|unit| unit.id).collect();
    let mid = center(squad.iter().copied());
    let Some(natural) = state.natural.map(point).filter(|_| !ids.is_empty()) else {
        state.phase = Phase::Home;
        state.why = "bare";
        return;
    };
    let chased = fighters.iter().any(|enemy| {
        squad
            .iter()
            .any(|unit| dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(CHASE_TILES * ts))
    });
    if chased {
        state.phase = Phase::Home;
        state.why = "chased";
        return;
    }
    let worker = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind == EntityKind::Worker)
        .filter(|enemy| dist2(enemy.x, enemy.y, natural.0, natural.1) <= squared(WORKER_TILES * ts))
        .min_by(|a, b| {
            dist2(a.x, a.y, mid.0, mid.1)
                .total_cmp(&dist2(b.x, b.y, mid.0, mid.1))
                .then(a.id.cmp(&b.id))
        });
    if let Some(worker) = worker {
        state.at_natural_since = None;
        if let Some(units) = actions::attack_units(actions, ids, worker.id) {
            intents.push(AiIntent::Attack { units });
        }
        return;
    }
    if dist2(mid.0, mid.1, natural.0, natural.1) <= squared(NATURAL_REACHED_TILES * ts) {
        let since = *state.at_natural_since.get_or_insert(tick);
        if tick.saturating_sub(since) >= NATURAL_WAIT_TICKS {
            state.phase = Phase::Home;
            state.why = "natural";
        }
        return;
    }
    if let Some(units) = actions::move_units(actions, ids, natural.0, natural.1) {
        intents.push(AiIntent::Assemble { units });
    }
}

/// The enemy natural's field: the Steel nearest the enemy main outside its own fields.
fn enemy_natural(observation: &AiObservation, enemy_main: (f32, f32)) -> (f32, f32) {
    let ts = observation.map.tile_size as f32;
    let start_exclusion = (config::START_RESOURCE_MAX_DIST_TILES + 1.5) * ts;
    observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
        .filter(|resource| {
            dist2(resource.x, resource.y, enemy_main.0, enemy_main.1) > squared(start_exclusion)
        })
        .min_by(|a, b| {
            dist2(a.x, a.y, enemy_main.0, enemy_main.1)
                .total_cmp(&dist2(b.x, b.y, enemy_main.0, enemy_main.1))
                .then(a.id.cmp(&b.id))
        })
        .map(|resource| (resource.x, resource.y))
        .unwrap_or(enemy_main)
}

/// `wideswing`'s points from `from`: the nearest map edge, `EDGE_TILES` short of it, on the side of
/// the line between the mains with less remembered enemy army near the way round (the shorter way
/// round on a tie), and the flank point beside the enemy main on that side, inside the same margin.
fn swing_points(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &AiDecisionMemory,
    from: (f32, f32),
    main: (f32, f32),
    enemy_main: (f32, f32),
) -> ((f32, f32), (f32, f32)) {
    let ts = observation.map.tile_size as f32;
    let (width, height) = (
        observation.map.width as f32 * ts,
        observation.map.height as f32 * ts,
    );
    let inset = EDGE_TILES * ts;
    let line = normalized_direction(main, enemy_main).unwrap_or((1.0, 0.0));
    let across = (-line.1, line.0);
    let side_of = |at: (f32, f32)| (at.0 - main.0) * across.0 + (at.1 - main.1) * across.1;
    let inside = |at: (f32, f32)| {
        (
            at.0.clamp(inset, (width - inset).max(inset)),
            at.1.clamp(inset, (height - inset).max(inset)),
        )
    };
    let progress = |at: (f32, f32)| (at.0 - main.0) * line.0 + (at.1 - main.1) * line.1;
    // A leg's cost: its length in tiles, and `THREAT_TILES_PER_VALUE` tiles for every point of
    // remembered enemy army near its middle and its end.
    let leg = |a: (f32, f32), b: (f32, f32)| {
        let middle = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
        let threat: f32 = [middle, b]
            .iter()
            .map(|at| super::push::threat_near(observation, memory, *at, SIDE_THREAT_TILES))
            .sum();
        dist2(a.0, a.1, b.0, b.1).sqrt() / ts + threat * THREAT_TILES_PER_VALUE
    };
    // The edge, `EDGE_TILES` in, every `EDGE_STEP_TILES`.
    let step = EDGE_STEP_TILES * ts;
    let mut edge = Vec::new();
    let mut along = inset;
    while along <= width - inset {
        edge.push((along, inset));
        edge.push((along, height - inset));
        along += step;
    }
    let mut along = inset;
    while along <= height - inset {
        edge.push((inset, along));
        edge.push((width - inset, along));
        along += step;
    }
    let way_round = |sign: f32| {
        let flank = inside((
            enemy_main.0 + across.0 * sign * FLANK_TILES * ts,
            enemy_main.1 + across.1 * sign * FLANK_TILES * ts,
        ));
        // The way out is the cheapest edge point on its own, not the one on the shortest way to the
        // flank point: that one lies beside the flank point, and the squad walked straight there.
        let out = edge
            .iter()
            .copied()
            .filter(|at| side_of(*at) * sign > 0.0)
            .filter(|at| dist2(at.0, at.1, from.0, from.1) <= squared(MAX_OUT_TILES * ts))
            // Not back toward Jeff's own side.
            .filter(|at| progress(*at) >= progress(from) - BACK_TILES * ts)
            .map(|out| (leg(from, out), out))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, out)| out)
            .unwrap_or_else(|| {
                inside((
                    from.0 + across.0 * sign * OUT_FALLBACK_TILES * ts,
                    from.1 + across.1 * sign * OUT_FALLBACK_TILES * ts,
                ))
            });
        (leg(from, out) + leg(out, flank), out, flank)
    };
    let (left, right) = (way_round(1.0), way_round(-1.0));
    let pick = if left.0 <= right.0 { left } else { right };
    let snap = |at: (f32, f32)| {
        analysis
            .and_then(|analysis| analysis.open_ground_near(main, at, 2, 4))
            .unwrap_or(at)
    };
    (snap(pick.1), snap(pick.2))
}

fn center<'a>(units: impl Iterator<Item = &'a AiEntitySummary>) -> (f32, f32) {
    let (mut x, mut y, mut n) = (0.0, 0.0, 0.0);
    for unit in units {
        x += unit.x;
        y += unit.y;
        n += 1.0;
    }
    if n > 0.0 {
        (x / n, y / n)
    } else {
        (0.0, 0.0)
    }
}

/// The point `length` from `from`, straight away from `threat`.
fn away(
    threat: (f32, f32),
    from: (f32, f32),
    length: f32,
    observation: &AiObservation,
) -> (f32, f32) {
    let dir = normalized_direction(threat, from).unwrap_or((0.0, 1.0));
    clamp_to_map(
        (from.0 + dir.0 * length, from.1 + dir.1 * length),
        observation.map,
    )
}

fn stored((x, y): (f32, f32)) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

fn point((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32, y as f32)
}
