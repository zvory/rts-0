//! Jeff's AI V1.1 raids the enemy's outlying bases with a small Tank party led by its Scout Car,
//! and runs before the defence arrives. Pressure AI runs five bases by 7:30 against Jeff's two to
//! three, and V1.0 only ever attacked with its whole army along the main route, into Pressure's
//! waves.
//!
//! - **Party**: `PARTY_TANKS` healthy Tanks no other system holds, once `HOME_TANKS` more stay
//!   home, and a free Scout Car to lead `SCOUT_LEAD_TILES` ahead: it sees 15 tiles to a Tank's 9.
//! - **Target**: the enemy Depots and extractors seen, then the resource fields near the enemy
//!   start not seen empty, never the enemy main: the one with the most worth for the least
//!   remembered defence and the shortest way, passed over while defended by more than
//!   `DEFENCE_SHARE` of the party's strength.
//! - **Route**: straight there or around either side through a point `DETOUR_SHARE` of the way out
//!   to the side, whichever passes the least remembered enemy strength; a route past more than
//!   `ROUTE_SHARE` of the party's is not taken. It moves, not attack-moves, so it is not drawn into
//!   the fights it goes around, and looks again as it goes.
//! - **Depots only**: a Depot's extractors decay to nothing within 20 s of it (#1707), so a raid
//!   goes only for Depots it has seen (the scout finds them; 7 of 27 raids on likely sites found
//!   none), parks `SIGHT_STANDOFF_TILES` from it (`RANGED_STANDOFF_TILES` with its Scout Car to see
//!   for it), and once parked `RAMP_TICKS`, when a Tank reaches 14 tiles, shells it: 180 HP, one
//!   volley of three. It leaves once the Depot is down. It used to drive in to 5 tiles past the
//!   extractors, firing its first shell at the Depot 9 s after arriving.
//! - **Hit and run**: once
//!   enemy strength in sight near it reaches `RUN_SHARE` of its own (a Depot has 180 HP, one volley
//!   of three Tanks, so it need only get there; run at 40% a lone Tank turned every raid back), or its Tanks
//!   are down to `LOW_HP` of their health, it steps `BREAK_TILES` away from the danger, makes for
//!   the nearest Jeff Depot, and leaves the base alone for `HOT_TICKS`. Running straight home down
//!   the way it came took it through the answering army: every Tank of two raids died on the way
//!   back. A base cleared, it goes home (`raidchain`: on to the next while healthy and with no enemy
//!   in sight): moving on after a kill, a party was caught 10 s later by 5 Tanks answering it. The Scout Car falls back to
//!   the Tanks from enemies within `SCOUT_SHY_TILES`.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared, tile_center};
use rts_rules::math::GameplayMath as _;

const PARTY_TANKS: usize = 3;
/// With this many free Tanks besides `HOME_TANKS`, the party takes one more (`raid4`).
const BIG_PARTY_TANKS: usize = 4;
/// Remembered enemy strength this close to a target is the answer it would get.
const RESPONSE_TILES: f32 = 30.0;
/// A target with more than this many times the party's strength near it is left alone.
const RESPONSE_SKIP: f32 = 2.5;
const HOME_TANKS: usize = 1;
const SCOUT_LEAD_TILES: f32 = 8.0;
const WAYPOINT_TILES: f32 = 4.0;
/// Route waypoints are kept about this far apart.
const LEG_TILES: f32 = 10.0;
const HIT_TILES: f32 = 13.0;
/// Enemy fighters this close to a party Tank count against it.
const DANGER_TILES: f32 = 14.0;
const RUN_SHARE: f32 = 0.7;
const ROUTE_SHARE: f32 = 0.4;
const DEFENCE_SHARE: f32 = 0.5;
const DEFENCE_TILES: f32 = 14.0;
/// Remembered strength this close to a route waypoint counts against the route.
const ROUTE_THREAT_TILES: f32 = 10.0;
const DETOUR_SHARE: f32 = 0.35;
const TRAVEL_TILES: f32 = 60.0;
const LOW_HP: f32 = 0.45;
/// A party Tank must have this much of its health to set out or go on.
const SET_OUT_HP: f32 = 0.7;
const HOT_TICKS: u32 = config::TICK_HZ * 90;
const HOME_TILES: f32 = 15.0;
const REST_TICKS: u32 = config::TICK_HZ * 20;
const REORDER_TICKS: u32 = config::TICK_HZ * 2;
/// A base with no building seen within this of it, while a party unit is this close, is cleared.
const CLEARED_TILES: f32 = 8.0;
const SIGHT_STANDOFF_TILES: f32 = 8.0;
const RANGED_STANDOFF_TILES: f32 = 11.0;
const SLOT_SPACING_TILES: f32 = 2.5;
/// A Tank this close to its slot is parked there.
const PARK_TILES: f32 = 1.5;
/// A parked Tank's range reaches 14 tiles after 3 s.
const RAMP_TICKS: u32 = config::TICK_HZ * 7 / 2;
/// A Depot this close to the target point is the one raided.
const DEPOT_TILES: f32 = 6.0;
/// Running, the party first steps this far away from the danger.
const BREAK_TILES: f32 = 12.0;
/// The Scout Car falls back to the Tanks from enemy fighters this close.
const SCOUT_SHY_TILES: f32 = 10.0;
/// An enemy Tank counts toward the enemy's Tanks this long after it was last seen (`raidodds`).
const ENEMY_SEEN_TICKS: u32 = config::TICK_HZ * 60;
/// No raid sets out while the enemy Tanks known outnumber all of Jeff's by more than this
/// (`raidodds`). On Crossroads vs Pressure AI (seeds 0/1) a raid took 3 of Jeff's 5 Tanks out at
/// 6:54 with Pressure on 6-9; Jeff was down to 2 Tanks by 8:00 and its push never set out.
const ODDS_MARGIN: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Out,
    Hit,
    Home,
}

/// V1.1's Tank raid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct TankRaidMemory {
    tanks: BTreeSet<u32>,
    scout: Option<u32>,
    phase: Option<Phase>,
    target: Option<(i32, i32)>,
    route: Vec<(i32, i32)>,
    leg: usize,
    ordered: u32,
    hot: BTreeMap<(i32, i32), u32>,
    rest_until: u32,
    /// The party's strength and the danger near it at the last decision, for the decision log.
    weighed: (u32, u32),
    /// Since when the party has stood parked on its slots at the Depot.
    parked_since: Option<u32>,
    /// When its Scout Car last smoked for it.
    smoked: u32,
    /// Whether the Depot raided was seen standing at the base: its fall is the raid's kill.
    depot_seen: bool,
    /// Each enemy Tank seen and when it was last seen (`raidodds`).
    enemy_tanks: BTreeMap<u32, u32>,
    /// The enemy Tanks known and Jeff's at the last launch check, for the decision log.
    odds: (u32, u32),
}

impl TankRaidMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.tanks.contains(&id) || self.scout == Some(id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.tanks.iter().copied().chain(self.scout)
    }

    /// Whether the raid is on its way to a base or at it.
    pub(in crate::ai_core::decision) fn going(&self) -> bool {
        matches!(self.phase, Some(Phase::Out | Phase::Hit))
    }

    /// The raid's Tanks.
    pub(in crate::ai_core::decision) fn tank_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.tanks.iter().copied()
    }
}

fn point((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32, y as f32)
}

fn stored((x, y): (f32, f32)) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

fn hp_share(unit: &AiEntitySummary) -> f32 {
    let max = config::unit_stats(unit.kind).map_or(1, |stats| stats.hp.max(1));
    unit.hp as f32 / max as f32
}

/// Sends, steers and brings home V1.1's Tank raid (see the module docs).
pub(in crate::ai_core::decision) fn raid(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
) {
    if !is_jeff_v1_1(profile_id)
        || super::off("tankraid")
        || super::south_route::skips_raid(profile_id, observation)
    {
        memory.jeff_v1_1.tank_raid = TankRaidMemory::default();
        return;
    }
    let Some(analysis) = map_analysis else {
        return;
    };
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    let alive = |id: &u32| by_id.get(id).is_some_and(|unit| unit.hp > 0);
    {
        let state = &mut memory.jeff_v1_1.tank_raid;
        for enemy in observation
            .visible_enemies
            .iter()
            .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        {
            state.enemy_tanks.insert(enemy.id, tick);
        }
        state
            .enemy_tanks
            .retain(|_, seen| tick.saturating_sub(*seen) <= ENEMY_SEEN_TICKS);
        state.hot.retain(|_, until| tick < *until);
        state.tanks.retain(|id| alive(id));
        if state.scout.is_some_and(|id| !alive(&id)) {
            state.scout = None;
        }
        if state.phase.is_some() && state.tanks.len() < 2 && state.phase != Some(Phase::Home) {
            state.phase = Some(Phase::Home);
        }
        if state.tanks.is_empty() {
            state.phase = None;
            state.scout = None;
        }
    }
    let Some(enemy_main) = super::push::enemy_main(observation) else {
        return;
    };
    if memory.jeff_v1_1.tank_raid.phase.is_none() {
        if tick < memory.jeff_v1_1.tank_raid.rest_until {
            return;
        }
        launch(observation, analysis, memory, enemy_main);
        if memory.jeff_v1_1.tank_raid.phase.is_none() {
            return;
        }
    }
    let tanks: Vec<&AiEntitySummary> = memory
        .jeff_v1_1
        .tank_raid
        .tanks
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .collect();
    let n = tanks.len().max(1) as f32;
    let center = (
        tanks.iter().map(|unit| unit.x).sum::<f32>() / n,
        tanks.iter().map(|unit| unit.y).sum::<f32>() / n,
    );
    let home = memory
        .jeff_v1_1
        .front_line
        .rally()
        .filter(|_| !super::off("raidline"))
        .or_else(|| nearest_depot(observation, center))
        .unwrap_or_else(|| home_point(observation));
    let strength = party_strength(&tanks);
    let danger = danger_near(observation, &tanks);
    let away = danger_centre(observation, center);
    let health = tanks.iter().map(|unit| hp_share(unit)).sum::<f32>() / n;
    memory.jeff_v1_1.tank_raid.weighed = (strength as u32, danger as u32);
    let state = &memory.jeff_v1_1.tank_raid;
    let phase = state.phase.unwrap_or(Phase::Home);
    // A Depot seen standing and now gone from a site in sight is the raid's kill.
    if let Some(target) = memory.jeff_v1_1.tank_raid.target.map(point) {
        if memory.jeff_v1_1.tank_raid.depot_seen
            && visible_depot(observation, target, ts).is_none()
            && site_in_sight(observation, memory, target, ts)
        {
            memory.jeff_v1_1.tank_raid.depot_seen = false;
            super::deny::note_kill(memory, target, tick);
        }
    }
    // Run: too much near it, or worn down.
    if phase != Phase::Home && (danger >= strength * RUN_SHARE || health < LOW_HP) {
        let state = &mut memory.jeff_v1_1.tank_raid;
        if let Some(target) = state.target {
            state.hot.insert(target, tick + HOT_TICKS);
        }
        go_home(state, analysis, observation, center, home, away);
    }
    let phase = memory.jeff_v1_1.tank_raid.phase.unwrap_or(Phase::Home);
    let tank_ids: Vec<u32> = tanks.iter().map(|unit| unit.id).collect();
    match phase {
        Phase::Out => {
            let Some(target) = memory.jeff_v1_1.tank_raid.target.map(point) else {
                go_home(
                    &mut memory.jeff_v1_1.tank_raid,
                    analysis,
                    observation,
                    center,
                    home,
                    away,
                );
                return;
            };
            if dist2(center.0, center.1, target.0, target.1) <= squared(HIT_TILES * ts) {
                memory.jeff_v1_1.tank_raid.phase = Some(Phase::Hit);
            } else {
                // The way ahead, looked at again with what is now known.
                let ahead_threat = {
                    let state = &memory.jeff_v1_1.tank_raid;
                    state.route[state.leg.min(state.route.len())..]
                        .iter()
                        .map(|at| point(*at))
                        .filter(|at| {
                            dist2(at.0, at.1, target.0, target.1) > squared(DEFENCE_TILES * ts)
                        })
                        .map(|at| {
                            super::push::threat_near(observation, memory, at, ROUTE_THREAT_TILES)
                        })
                        .fold(0.0, f32::max)
                };
                if ahead_threat > strength * ROUTE_SHARE {
                    match best_route(observation, analysis, memory, center, target, strength) {
                        Some(route) => {
                            let state = &mut memory.jeff_v1_1.tank_raid;
                            state.route = route;
                            state.leg = 0;
                            state.ordered = 0;
                        }
                        None => {
                            let state = &mut memory.jeff_v1_1.tank_raid;
                            if let Some(target) = state.target {
                                state.hot.insert(target, tick + HOT_TICKS);
                            }
                            go_home(state, analysis, observation, center, home, away);
                        }
                    }
                }
            }
        }
        Phase::Hit => {
            let target = memory
                .jeff_v1_1
                .tank_raid
                .target
                .map(point)
                .unwrap_or(center);
            let in_sight = site_in_sight(observation, memory, target, ts);
            if visible_depot(observation, target, ts).is_none() && in_sight {
                // The Depot is down, or was never there: its extractors decay on their own. On to the
                // next base while healthy and unseen, else home.
                let next = (!super::off("raidchain") && health >= SET_OUT_HP && danger == 0.0)
                    .then(|| {
                        choose_target(observation, analysis, memory, center, enemy_main, strength)
                    })
                    .flatten();
                let state = &mut memory.jeff_v1_1.tank_raid;
                if let Some(target) = state.target {
                    state.hot.insert(target, tick + HOT_TICKS);
                }
                match next {
                    Some((target, route)) => {
                        state.target = Some(stored(target));
                        state.route = route;
                        state.leg = 0;
                        state.ordered = 0;
                        state.phase = Some(Phase::Out);
                    }
                    None => go_home(state, analysis, observation, center, home, away),
                }
            }
        }
        Phase::Home => {
            if dist2(center.0, center.1, home.0, home.1) <= squared(HOME_TILES * ts) {
                let state = &mut memory.jeff_v1_1.tank_raid;
                state.tanks.clear();
                state.scout = None;
                state.phase = None;
                state.target = None;
                state.route.clear();
                state.rest_until = tick + REST_TICKS;
                return;
            }
        }
    }
    // The Scout Car smokes the enemy Tanks shooting at the party, above all on the way home.
    let running = memory.jeff_v1_1.tank_raid.phase == Some(Phase::Home);
    let scout = memory.jeff_v1_1.tank_raid.scout;
    let mut smoked = memory.jeff_v1_1.tank_raid.smoked;
    super::smoke_cover::cover(
        actions,
        observation,
        memory,
        scout,
        &tank_ids,
        running,
        &mut smoked,
    );
    memory.jeff_v1_1.tank_raid.smoked = smoked;
    order(actions, observation, memory, intents, &tank_ids, center, ts);
}

/// The Jeff Depot nearest `from`.
fn nearest_depot(observation: &AiObservation, from: (f32, f32)) -> Option<(f32, f32)> {
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0 && unit.is_complete)
        .min_by(|a, b| {
            dist2(a.x, a.y, from.0, from.1)
                .total_cmp(&dist2(b.x, b.y, from.0, from.1))
                .then(a.id.cmp(&b.id))
        })
        .map(|unit| (unit.x, unit.y))
}

/// The centre of the enemy fighters in sight near the party, if any.
fn danger_centre(observation: &AiObservation, center: (f32, f32)) -> Option<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let near: Vec<(f32, f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| {
            dist2(enemy.x, enemy.y, center.0, center.1) <= squared((DANGER_TILES + 4.0) * ts)
        })
        .map(|enemy| (enemy.x, enemy.y))
        .collect();
    let n = near.len() as f32;
    (n > 0.0).then(|| {
        (
            near.iter().map(|at| at.0).sum::<f32>() / n,
            near.iter().map(|at| at.1).sum::<f32>() / n,
        )
    })
}

/// Where the raid comes home to without a Depot: the main.
fn home_point(observation: &AiObservation) -> (f32, f32) {
    tile_center(observation.own_start_tile, observation.map.tile_size)
}

/// The party's strength: each Tank's cost, Oil counted double, by its health.
fn party_strength(tanks: &[&AiEntitySummary]) -> f32 {
    tanks
        .iter()
        .map(|unit| super::push::unit_value(unit.kind) as f32 * hp_share(unit))
        .sum()
}

/// Enemy fighters in sight within `DANGER_TILES` of a party Tank, by value.
fn danger_near(observation: &AiObservation, tanks: &[&AiEntitySummary]) -> f32 {
    let ts = observation.map.tile_size as f32;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| {
            tanks
                .iter()
                .any(|tank| dist2(tank.x, tank.y, enemy.x, enemy.y) <= squared(DANGER_TILES * ts))
        })
        .map(|enemy| super::push::unit_value(enemy.kind) as f32)
        .sum()
}

/// The enemy buildings standing within `HIT_TILES` of `at`, the Depot first, then extractors.
fn buildings_at(observation: &AiObservation, at: (f32, f32), ts: f32) -> Vec<&AiEntitySummary> {
    let mut found: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_building())
        .filter(|enemy| enemy.kind != EntityKind::TankTrap)
        .filter(|enemy| dist2(enemy.x, enemy.y, at.0, at.1) <= squared(HIT_TILES * ts))
        .collect();
    let rank = |kind: EntityKind| match kind {
        EntityKind::ResourceDepot => 0,
        EntityKind::SteelMine | EntityKind::PumpJack => 1,
        _ => 2,
    };
    found.sort_by(|a, b| {
        rank(a.kind)
            .cmp(&rank(b.kind))
            .then(dist2(a.x, a.y, at.0, at.1).total_cmp(&dist2(b.x, b.y, at.0, at.1)))
            .then(a.id.cmp(&b.id))
    });
    found
}

/// Picks the party and its first target, if both are there.
fn launch(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    memory: &mut AiDecisionMemory,
    enemy_main: (f32, f32),
) {
    // Outnumbered in Tanks, Jeff keeps them together (`raidodds`).
    let ours = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .count();
    let theirs = memory
        .jeff_v1_1
        .tank_raid
        .enemy_tanks
        .len()
        .max(memory.recent_enemy_tanks());
    memory.jeff_v1_1.tank_raid.odds = (theirs as u32, ours as u32);
    if !super::off("raidodds") && theirs > ours + ODDS_MARGIN {
        return;
    }
    let reserved = memory.squad_reserved();
    let free = |unit: &&AiEntitySummary| {
        unit.is_complete
            && unit.hp > 0
            && !reserved.contains(&unit.id)
            && !memory.containment.active_tanks.contains(&unit.id)
            && memory.containment.active_scout != Some(unit.id)
            && memory.home_defensive_tank != Some(unit.id)
            && !memory.later_bases.guards.contains(&unit.id)
            && memory.jeff_v1_1.scout.unit != Some(unit.id)
            && !memory.jeff_v1_1.home_guard.holds(unit.id)
    };
    // The scout's Scout Car goes with the raid (`raidcar`): Jeff keeps one, and without it the raid
    // had no smoke and parked within its Tanks' 9 tiles of sight.
    let free_car = |unit: &&AiEntitySummary| {
        unit.is_complete
            && unit.hp > 0
            && !reserved.contains(&unit.id)
            && memory.containment.active_scout != Some(unit.id)
            && (memory.jeff_v1_1.scout.unit != Some(unit.id) || !super::off("raidcar"))
    };
    let mut tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank)
        .filter(free)
        .collect();
    if tanks.len() < PARTY_TANKS + HOME_TANKS {
        return;
    }
    tanks.retain(|unit| hp_share(unit) >= SET_OUT_HP);
    if tanks.len() < PARTY_TANKS {
        return;
    }
    let party_size = if !super::off("raid4") && tanks.len() >= BIG_PARTY_TANKS + HOME_TANKS {
        BIG_PARTY_TANKS
    } else {
        PARTY_TANKS
    };
    // The Tanks nearest the enemy go.
    tanks.sort_by(|a, b| {
        dist2(a.x, a.y, enemy_main.0, enemy_main.1)
            .total_cmp(&dist2(b.x, b.y, enemy_main.0, enemy_main.1))
            .then(a.id.cmp(&b.id))
    });
    tanks.truncate(party_size);
    let n = tanks.len() as f32;
    let center = (
        tanks.iter().map(|unit| unit.x).sum::<f32>() / n,
        tanks.iter().map(|unit| unit.y).sum::<f32>() / n,
    );
    let strength = party_strength(&tanks);
    let party: BTreeSet<u32> = tanks.iter().map(|unit| unit.id).collect();
    let scout = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ScoutCar)
        .filter(free_car)
        .min_by(|a, b| {
            dist2(a.x, a.y, center.0, center.1)
                .total_cmp(&dist2(b.x, b.y, center.0, center.1))
                .then(a.id.cmp(&b.id))
        })
        .map(|unit| unit.id);
    let Some((target, route)) =
        choose_target(observation, analysis, memory, center, enemy_main, strength)
    else {
        return;
    };
    let state = &mut memory.jeff_v1_1.tank_raid;
    state.tanks = party;
    state.scout = scout;
    state.phase = Some(Phase::Out);
    state.target = Some(stored(target));
    state.route = route;
    state.leg = 0;
    state.ordered = 0;
}

/// A raid target and the way there, in stored tiles.
type RaidPlan = ((f32, f32), Vec<(i32, i32)>);

/// The best base to raid from `from` and the way there (see the module docs).
fn choose_target(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    memory: &mut AiDecisionMemory,
    from: (f32, f32),
    enemy_main: (f32, f32),
    strength: f32,
) -> Option<RaidPlan> {
    let ts = observation.map.tile_size as f32;
    let found: Vec<((f32, f32), f32)> = super::push::known_depots(observation, memory, enemy_main)
        .into_iter()
        .map(|at| (at, 1.0))
        .collect();
    let hot: Vec<(f32, f32)> = memory
        .jeff_v1_1
        .tank_raid
        .hot
        .keys()
        .map(|at| point(*at))
        .collect();
    let mut best: Option<(f32, RaidPlan)> = None;
    for (at, worth) in found {
        if hot
            .iter()
            .any(|spot| dist2(spot.0, spot.1, at.0, at.1) <= squared(DEFENCE_TILES * ts))
        {
            continue;
        }
        let defence = super::push::threat_near(observation, memory, at, DEFENCE_TILES);
        if defence > strength * DEFENCE_SHARE {
            continue;
        }
        // The answer it would get: the enemy army remembered near it (`raidwindow`). Busy at Jeff's
        // bases, it is far from the enemy's own.
        let response = if super::off("raidwindow") {
            0.0
        } else {
            super::push::threat_near(observation, memory, at, RESPONSE_TILES)
        };
        if response > strength * RESPONSE_SKIP {
            continue;
        }
        let Some(route) = best_route(observation, analysis, memory, from, at, strength) else {
            continue;
        };
        let length = route
            .windows(2)
            .map(|pair| {
                dist2(
                    pair[0].0 as f32,
                    pair[0].1 as f32,
                    pair[1].0 as f32,
                    pair[1].1 as f32,
                )
                .det_sqrt()
            })
            .sum::<f32>()
            / ts;
        let score = worth
            / (1.0 + defence / strength.max(1.0) + 0.5 * response / strength.max(1.0))
            / (1.0 + length / TRAVEL_TILES);
        if best.as_ref().is_none_or(|(top, _)| score > *top) {
            best = Some((score, (at, route)));
        }
    }
    best.map(|(_, plan)| plan)
}

/// The way from `from` to `to` past the least remembered enemy strength: straight, or around
/// either side. `None` when every way passes more than `ROUTE_SHARE` of `strength`.
fn best_route(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    memory: &AiDecisionMemory,
    from: (f32, f32),
    to: (f32, f32),
    strength: f32,
) -> Option<Vec<(i32, i32)>> {
    let ts = observation.map.tile_size as f32;
    let distance = dist2(from.0, from.1, to.0, to.1).det_sqrt();
    let direction = normalized_direction(from, to)?;
    let across = (-direction.1, direction.0);
    let mut options = vec![analysis.compact_group_route(from, to, 4)];
    for side in [-1.0, 1.0] {
        let swing = clamp_to_map(
            (
                (from.0 + to.0) * 0.5 + across.0 * side * distance * DETOUR_SHARE,
                (from.1 + to.1) * 0.5 + across.1 * side * distance * DETOUR_SHARE,
            ),
            observation.map,
        );
        let mut way = analysis.compact_group_route(from, swing, 4);
        way.extend(analysis.compact_group_route(swing, to, 4));
        options.push(way);
    }
    options
        .into_iter()
        .filter(|way| !way.is_empty())
        .map(|way| {
            let threat = way
                .iter()
                .filter(|at| dist2(at.0, at.1, to.0, to.1) > squared(DEFENCE_TILES * ts))
                .map(|at| super::push::threat_near(observation, memory, *at, ROUTE_THREAT_TILES))
                .fold(0.0, f32::max);
            (threat, way)
        })
        .filter(|(threat, _)| *threat <= strength * ROUTE_SHARE)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.len().cmp(&b.1.len())))
        .map(|(_, way)| thin(&way, ts))
}

/// Keeps route waypoints about `LEG_TILES` apart, and the last.
fn thin(way: &[(f32, f32)], ts: f32) -> Vec<(i32, i32)> {
    let mut kept: Vec<(f32, f32)> = Vec::new();
    for at in way {
        if kept
            .last()
            .is_none_or(|last| dist2(last.0, last.1, at.0, at.1) >= squared(LEG_TILES * ts))
        {
            kept.push(*at);
        }
    }
    if let (Some(last), Some(end)) = (kept.last().copied(), way.last().copied()) {
        if last != end {
            kept.push(end);
        }
    }
    kept.into_iter().map(stored).collect()
}

/// Turns the raid home: first `BREAK_TILES` away from the danger, between straight away from it
/// and toward home, then the way home from there.
fn go_home(
    state: &mut TankRaidMemory,
    analysis: &AiMapAnalysis,
    observation: &AiObservation,
    from: (f32, f32),
    home: (f32, f32),
    away: Option<(f32, f32)>,
) {
    let ts = observation.map.tile_size as f32;
    let start = away
        .and_then(|danger| {
            let off = normalized_direction(danger, from)?;
            let toward = normalized_direction(from, home).unwrap_or(off);
            let way = normalized_direction((0.0, 0.0), (off.0 + toward.0, off.1 + toward.1))
                .unwrap_or(off);
            Some(clamp_to_map(
                (
                    from.0 + way.0 * BREAK_TILES * ts,
                    from.1 + way.1 * BREAK_TILES * ts,
                ),
                observation.map,
            ))
        })
        .unwrap_or(from);
    state.phase = Some(Phase::Home);
    state.parked_since = None;
    state.route = std::iter::once(start)
        .filter(|at| *at != from)
        .chain(analysis.compact_group_route(start, home, 4))
        .step_by(3)
        .chain(std::iter::once(home))
        .map(stored)
        .collect();
    state.leg = 0;
    state.ordered = 0;
}

/// This decision's orders for the party.
fn order(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    tanks: &[u32],
    center: (f32, f32),
    ts: f32,
) {
    let tick = observation.tick;
    let state = &mut memory.jeff_v1_1.tank_raid;
    let Some(phase) = state.phase else {
        return;
    };
    // Advance along the route.
    while let Some(next) = state.route.get(state.leg).map(|at| point(*at)) {
        if dist2(center.0, center.1, next.0, next.1) > squared(WAYPOINT_TILES * ts)
            || state.leg + 1 >= state.route.len()
        {
            break;
        }
        state.leg += 1;
        state.ordered = 0;
    }
    if phase == Phase::Hit {
        shell_depot(actions, observation, memory, intents, tanks, center, ts);
        return;
    }
    if tick.saturating_sub(state.ordered) < REORDER_TICKS {
        return;
    }
    state.ordered = tick;
    let scout = state.scout;
    match phase {
        Phase::Out | Phase::Home => {
            let Some(next) = state.route.get(state.leg).map(|at| point(*at)) else {
                return;
            };
            actions::move_units(actions, tanks.iter().copied(), next.0, next.1);
            let mut moved = tanks.to_vec();
            if let Some(scout) = scout {
                // The Scout Car leads on the way out, and covers the way home behind the Tanks.
                let lead = state
                    .route
                    .get(state.leg + 1)
                    .map(|at| point(*at))
                    .unwrap_or(next);
                let ahead = normalized_direction(center, lead).unwrap_or((0.0, 0.0));
                let shy = observation
                    .owned
                    .iter()
                    .find(|unit| unit.id == scout)
                    .is_some_and(|car| {
                        observation.visible_enemies.iter().any(|enemy| {
                            enemy.hp > 0
                                && enemy.kind.is_unit()
                                && enemy.kind != EntityKind::Worker
                                && dist2(enemy.x, enemy.y, car.x, car.y)
                                    <= squared(SCOUT_SHY_TILES * ts)
                        })
                    });
                let spot = if shy {
                    center
                } else if phase == Phase::Out {
                    (
                        center.0 + ahead.0 * SCOUT_LEAD_TILES * ts,
                        center.1 + ahead.1 * SCOUT_LEAD_TILES * ts,
                    )
                } else {
                    next
                };
                let spot = clamp_to_map(spot, observation.map);
                actions::move_units(actions, [scout], spot.0, spot.1);
                moved.push(scout);
            }
            intents.push(AiIntent::Assemble { units: moved });
        }
        Phase::Hit => {}
    }
}

/// The raid at its Depot: Tanks to slots `SIGHT_STANDOFF_TILES` (`RANGED_STANDOFF_TILES` with the
/// Scout Car) out from it on their side, held there, and once parked `RAMP_TICKS` set on it. Not
/// yet in sight, they close on the target point.
fn shell_depot(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    tanks: &[u32],
    center: (f32, f32),
    ts: f32,
) {
    let tick = observation.tick;
    let state = &mut memory.jeff_v1_1.tank_raid;
    let target = state.target.map(point).unwrap_or(center);
    let scout = state.scout;
    let due = tick.saturating_sub(state.ordered) >= REORDER_TICKS;
    let Some(depot) = visible_depot(observation, target, ts) else {
        state.parked_since = None;
        if due {
            state.ordered = tick;
            actions::move_units(actions, tanks.iter().copied(), target.0, target.1);
            let mut moved = tanks.to_vec();
            if let Some(scout) = scout {
                actions::move_units(actions, [scout], target.0, target.1);
                moved.push(scout);
            }
            intents.push(AiIntent::Assemble { units: moved });
        }
        return;
    };
    state.depot_seen = true;
    let standoff = if scout.is_some() {
        RANGED_STANDOFF_TILES
    } else {
        SIGHT_STANDOFF_TILES
    };
    let back = normalized_direction((depot.x, depot.y), center).unwrap_or((1.0, 0.0));
    let across = (-back.1, back.0);
    let n = tanks.len();
    let mut slots: Vec<(f32, f32)> = (0..n)
        .map(|index| {
            let side = (index as f32 - (n as f32 - 1.0) * 0.5) * SLOT_SPACING_TILES;
            clamp_to_map(
                (
                    depot.x + (back.0 * standoff + across.0 * side) * ts,
                    depot.y + (back.1 * standoff + across.1 * side) * ts,
                ),
                observation.map,
            )
        })
        .collect();
    let lateral = |x: f32, y: f32| x * across.0 + y * across.1;
    let mut party: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id))
        .collect();
    party.sort_by(|a, b| {
        lateral(a.x, a.y)
            .total_cmp(&lateral(b.x, b.y))
            .then(a.id.cmp(&b.id))
    });
    slots.sort_by(|a, b| lateral(a.0, a.1).total_cmp(&lateral(b.0, b.1)));
    let parked = party
        .iter()
        .zip(&slots)
        .all(|(unit, slot)| dist2(unit.x, unit.y, slot.0, slot.1) <= squared(PARK_TILES * ts));
    if parked {
        let first = state.parked_since.is_none();
        let since = *state.parked_since.get_or_insert(tick);
        if first {
            actions::hold_position_units(actions, tanks.iter().copied());
        } else if tick.saturating_sub(since) >= RAMP_TICKS && due {
            state.ordered = tick;
            if let Some(units) = actions::attack_units(actions, tanks.iter().copied(), depot.id) {
                intents.push(AiIntent::Attack { units });
            }
        }
    } else {
        state.parked_since = None;
        if due {
            state.ordered = tick;
            let mut moved = Vec::new();
            for (unit, slot) in party.iter().zip(&slots) {
                if dist2(unit.x, unit.y, slot.0, slot.1) > squared(PARK_TILES * ts) {
                    actions::move_units(actions, [unit.id], slot.0, slot.1);
                    moved.push(unit.id);
                }
            }
            if !moved.is_empty() {
                intents.push(AiIntent::Assemble { units: moved });
            }
        }
    }
    if let Some(scout) = scout {
        // The Scout Car stands with the Tanks and sees the Depot for them.
        if due {
            actions::move_units(actions, [scout], center.0, center.1);
        }
    }
}

/// Whether a party Tank stands within its sight of `at`, or its Scout Car within the car's.
fn site_in_sight(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    at: (f32, f32),
    ts: f32,
) -> bool {
    observation
        .owned
        .iter()
        .filter(|unit| memory.jeff_v1_1.tank_raid.holds(unit.id) && unit.hp > 0)
        .any(|unit| {
            let sight = config::unit_stats(unit.kind)
                .map_or(CLEARED_TILES, |stats| stats.sight_tiles as f32 - 1.0);
            dist2(unit.x, unit.y, at.0, at.1) <= squared(sight * ts)
        })
}

/// The enemy Depot standing within `DEPOT_TILES` of `at`, in sight.
fn visible_depot(observation: &AiObservation, at: (f32, f32), ts: f32) -> Option<&AiEntitySummary> {
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::ResourceDepot && enemy.hp > 0)
        .find(|enemy| dist2(enemy.x, enemy.y, at.0, at.1) <= squared(DEPOT_TILES * ts))
}

/// The decision-log line: phase, target, party, way and the strength weighed.
pub(in crate::ai_core::decision) fn trace_line(memory: &AiDecisionMemory) -> Option<String> {
    let state = &memory.jeff_v1_1.tank_raid;
    (memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("tankraid")).then(|| {
        format!(
            "v1_1_raid phase={:?} odds={}/{} target={:?} tanks={:?} scout={:?} leg={}/{} strength={} danger={} hot={:?} rest_until={} seen={} deny={}",
            state.phase,
            state.odds.0,
            state.odds.1,
            state.target,
            state.tanks,
            state.scout,
            state.leg,
            state.route.len(),
            state.weighed.0,
            state.weighed.1,
            state.hot.keys().collect::<Vec<_>>(),
            state.rest_until,
            state.depot_seen,
            memory.jeff_v1_1.deny.trace(),
        )
    })
}
