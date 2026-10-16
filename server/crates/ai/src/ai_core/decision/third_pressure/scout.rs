//! The Scout Car goes out alone first. It walks the flank route to the target base, sweeps every
//! spot that could hold its Depot, and watches a found Depot from outside to size up the garrison.
//! Only then are the Tanks and Riflemen sent, so an empty or defended base costs a Scout Car trip
//! and never the push's Tanks.
//!
//! The Scout Car is kept out of harm's way: it moves with plain move orders (it never attacks),
//! turns away from any enemy fighter within `SCOUT_DANGER_TILES` (enemy sight is 9-11 tiles), waits
//! before going on, keeps out of reach of enemy buildings' defense zones, and gives the trip up
//! when it is hurt or keeps being found.

use super::locate::{
    area_center, base_empty, clear_seen, depot_spots, found_depot, next_view, staging_point,
    survey_point,
};
use super::*;

/// Enemy fighters this close to the Scout Car turn it away; Tanks, which reach 14 tiles parked,
/// from farther.
const SCOUT_DANGER_TILES: f32 = 14.0;
const SCOUT_TANK_DANGER_TILES: f32 = 16.0;
/// It turns this far away from them, then waits before going on.
const SCOUT_FLEE_TILES: f32 = 10.0;
const SCOUT_FLEE_WAIT_TICKS: u32 = config::TICK_HZ * 8;
/// Turned away more often than this, the Scout Car gives the trip up.
const SCOUT_MAX_FLEES: u32 = 4;
/// Below this share of its hit points it comes home.
const SCOUT_ABORT_HP_PERCENT: u32 = 70;
const SCOUT_TRIP_TIMEOUT_TICKS: u32 = config::TICK_HZ * 240;
/// A found Depot is watched this long for a party to become free before the Scout Car comes home.
const COMMIT_WAIT_TICKS: u32 = config::TICK_HZ * 60;

/// Sends the Scout Car alone toward `target`.
pub(super) fn begin_scout(
    observation: &AiObservation,
    state: &mut ThirdPressure,
    scout: u32,
    target: EnemyBase,
    route: Vec<(f32, f32)>,
) {
    let tick = observation.tick;
    state.tanks.clear();
    state.riflemen.clear();
    state.scout = Some(scout);
    state.target = Some(rounded(target.depot.unwrap_or(target.center)));
    state.area = target.patches.iter().map(|p| rounded(*p)).collect();
    state.route = route.into_iter().map(rounded).collect();
    state.reset_trip(tick);
    state.phase = Phase::Scout;
    state.phase_since = tick;
}

fn scout_car<'o>(
    observation: &'o AiObservation,
    state: &ThirdPressure,
) -> Option<&'o AiEntitySummary> {
    let id = state.scout?;
    observation
        .owned
        .iter()
        .find(|unit| unit.id == id && unit.hp > 0)
}

fn hurt(car: &AiEntitySummary) -> bool {
    let max = config::unit_stats(car.kind).map_or(car.hp, |stats| stats.hp);
    car.hp.saturating_mul(100) < max.saturating_mul(SCOUT_ABORT_HP_PERCENT)
}

/// Turns the Scout Car away from enemy fighters close enough to see it. Returns whether it is
/// clear of them; `counting` counts the encounter toward giving the trip up.
pub(super) fn scout_clear(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    counting: bool,
) -> bool {
    let tick = observation.tick;
    let ts = tile(observation);
    let Some(car) = scout_car(observation, &memory.third_pressure) else {
        return true;
    };
    let car_at = (car.x, car.y);
    let threats: Vec<(f32, f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| {
            let danger = if enemy.kind == EntityKind::Tank {
                SCOUT_TANK_DANGER_TILES
            } else {
                SCOUT_DANGER_TILES
            };
            distance((enemy.x, enemy.y), car_at) <= danger * ts
        })
        .map(|enemy| (enemy.x, enemy.y))
        .collect();
    if threats.is_empty() {
        return true;
    }
    let home = home_point(observation, memory);
    let state = &mut memory.third_pressure;
    if counting && tick >= state.fallback_until {
        state.flees = state.flees.saturating_add(1);
        if state.flees > SCOUT_MAX_FLEES {
            begin_return(observation, memory, intents, "scout spotted");
            return false;
        }
    }
    let n = threats.len() as f32;
    let threat = threats
        .iter()
        .fold((0.0, 0.0), |sum, p| (sum.0 + p.0 / n, sum.1 + p.1 / n));
    let back_along_route = state
        .leg
        .checked_sub(1)
        .and_then(|leg| state.route.get(leg))
        .map(|p| unrounded(*p));
    let flee = normalized_direction(threat, car_at)
        .map(|away| {
            clamp_to_map(
                (
                    car_at.0 + away.0 * SCOUT_FLEE_TILES * ts,
                    car_at.1 + away.1 * SCOUT_FLEE_TILES * ts,
                ),
                observation.map,
            )
        })
        .into_iter()
        .chain(back_along_route)
        .chain(std::iter::once(home))
        .find(|point| {
            map_analysis.is_none_or(|analysis| {
                analysis.tile_is_passable((point.0 / ts) as u32, (point.1 / ts) as u32)
            })
        })
        .unwrap_or(home);
    state.fallback_until = tick.saturating_add(SCOUT_FLEE_WAIT_TICKS);
    if let Some(id) = state.scout {
        send(actions, observation, state, &[id], flee, intents);
    }
    false
}

pub(super) fn scout_trip(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let tick = observation.tick;
    let ts = tile(observation);
    let state = &memory.third_pressure;
    let Some(car) = scout_car(observation, state) else {
        // Lost on the way: the base it went to is left alone a while.
        let center = area_center(state);
        let state = &mut memory.third_pressure;
        state.defended.push((rounded(center), tick));
        begin_return(observation, memory, intents, "scout lost");
        return;
    };
    if tick.saturating_sub(state.phase_since) > SCOUT_TRIP_TIMEOUT_TICKS {
        begin_return(observation, memory, intents, "scout out of time");
        return;
    }
    if hurt(car) {
        begin_return(observation, memory, intents, "scout hurt");
        return;
    }
    let car_at = (car.x, car.y);
    let car_sight = sight(car);
    let car_id = car.id;
    if !scout_clear(actions, observation, map_analysis, memory, intents, true) {
        return;
    }
    let state = &mut memory.third_pressure;
    if tick < state.fallback_until {
        return;
    }
    // On the way.
    if let Some(point) = state.route.get(state.leg).map(|p| unrounded(*p)) {
        if distance(car_at, point) <= LEG_REACHED_TILES * ts
            || tick.saturating_sub(state.leg_since) > LEG_TIMEOUT_TICKS
        {
            state.leg += 1;
            state.leg_since = tick;
        }
        if let Some(point) = state.route.get(state.leg).map(|p| unrounded(*p)) {
            send(actions, observation, state, &[car_id], point, intents);
            return;
        }
    }
    // At the base: find the Depot.
    let center = area_center(state);
    let staging = staging_point(state, center);
    if state.survey_since.is_none() {
        if let Some((at, id)) = found_depot(observation, state) {
            state.target = Some(at);
            state.depot = id;
            state.survey_since = Some(tick);
        }
    }
    if let Some(since) = state.survey_since {
        let Some(depot) = state.target.map(unrounded) else {
            begin_return(observation, memory, intents, "no target");
            return;
        };
        let point = survey_point(observation, map_analysis, depot, staging, car_sight);
        send(actions, observation, state, &[car_id], point, intents);
        let arrived = distance(car_at, point) <= 2.0 * ts;
        if !arrived && tick.saturating_sub(since) < SURVEY_TIMEOUT_TICKS {
            return;
        }
        state.watch = Some(rounded(point));
        report(observation, map_analysis, memory, intents, depot, since);
        return;
    }
    if state.spots_total == 0 {
        let area: Vec<(f32, f32)> = state.area.iter().map(|p| unrounded(*p)).collect();
        state.unseen = depot_spots(observation, map_analysis, &area)
            .into_iter()
            .map(rounded)
            .collect();
        state.spots_total = state.unseen.len();
    }
    let search_since = *state.search_since.get_or_insert(tick);
    clear_seen(observation, state, &[car]);
    let seen = state.spots_total.saturating_sub(state.unseen.len());
    let mostly_seen = state.spots_total > 0 && seen * 100 >= state.spots_total * EMPTY_SEEN_PERCENT;
    let out_of_time = tick.saturating_sub(search_since) > LOCATE_TIMEOUT_TICKS;
    if state.unseen.is_empty() || (out_of_time && mostly_seen) {
        base_empty(observation, map_analysis, memory, intents, center);
        return;
    }
    if out_of_time {
        begin_return(observation, memory, intents, "depot not found");
        return;
    }
    if let Some(view) = next_view(observation, map_analysis, state, &[car], center, staging) {
        send(actions, observation, state, &[car_id], view, intents);
    }
}

/// The Scout Car has watched the found Depot: a garrison the party would lose to keeps the Tanks
/// home; otherwise the party is sent while the Scout Car keeps watch.
fn report(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    depot: (f32, f32),
    since: u32,
) {
    let tick = observation.tick;
    let ts = tile(observation);
    let home = home_point(observation, memory);
    let garrison: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| distance((enemy.x, enemy.y), depot) <= GARRISON_TILES * ts)
        .collect();
    let tanks = party_tanks(observation, memory);
    let riflemen = party_riflemen(observation, memory, home);
    if !garrison.is_empty() {
        let would_send: Vec<u32> = tanks
            .iter()
            .flatten()
            .chain(riflemen.iter().flatten())
            .copied()
            .collect();
        if would_send.is_empty() || loses_to(observation, &would_send, &garrison) {
            let center = area_center(&memory.third_pressure);
            memory.third_pressure.defended.push((rounded(center), tick));
            begin_return(observation, memory, intents, "base defended");
            return;
        }
    }
    if commit(observation, map_analysis, memory) {
        return;
    }
    if tick.saturating_sub(since) > COMMIT_WAIT_TICKS {
        begin_return(observation, memory, intents, "no party free");
    }
}

/// Whether `enemies` would beat the party made of `ours`, or bring at least as many Tanks.
fn loses_to(observation: &AiObservation, ours: &[u32], enemies: &[&AiEntitySummary]) -> bool {
    let meth = observation
        .upgrades
        .contains(&UpgradeKind::Methamphetamines);
    let ours_units = units(observation, ours);
    let our_tanks = ours_units
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank)
        .count();
    let ours: Vec<_> = ours_units
        .into_iter()
        .filter_map(|unit| fighter(unit, meth, false))
        .collect();
    let theirs: Vec<_> = enemies
        .iter()
        .filter_map(|enemy| fighter(enemy, false, enemy.state != AiEntityState::Move))
        .collect();
    let their_tanks = enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank)
        .count();
    estimate_trade(&ours, &theirs, 0).clearly_losing()
        || (their_tanks > 0 && their_tanks >= our_tanks)
}

/// Sends two army Tanks and five Riflemen to the base the Scout Car found, by the flank route,
/// when Jeff can spare them now. Returns whether the party left.
pub(super) fn commit(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
) -> bool {
    let bases = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .count();
    if bases >= 3
        || memory.later_bases.planning()
        || memory.home_outgunned
        || defense::local_defense_contact(observation).is_some()
    {
        return false;
    }
    let home = home_point(observation, memory);
    let Some(tanks) = party_tanks(observation, memory) else {
        return false;
    };
    let Some(riflemen) = party_riflemen(observation, memory, home) else {
        return false;
    };
    let tick = observation.tick;
    let state = &mut memory.third_pressure;
    let Some(goal) = state.target.map(unrounded) else {
        return false;
    };
    state
        .notes
        .push(format!("party sent to {:?}", rounded(goal)));
    state.tanks = tanks;
    state.riflemen = riflemen;
    state.route = flank_route(observation, map_analysis, home, goal)
        .into_iter()
        .map(rounded)
        .collect();
    state.leg = 0;
    state.leg_since = tick;
    state.phase = Phase::March;
    state.phase_since = tick;
    state.survey_since = None;
    state.clear_orders();
    true
}

/// While the party marches, the Scout Car keeps watch on the Depot from outside, turning away
/// from enemies; hurt, it is let go.
pub(super) fn keep_watch(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let Some(car) = scout_car(observation, &memory.third_pressure) else {
        memory.third_pressure.watch = None;
        return;
    };
    if hurt(car) {
        let id = car.id;
        let state = &mut memory.third_pressure;
        state.release(&[id]);
        state.watch = None;
        intents.push(AiIntent::Assemble { units: vec![id] });
        return;
    }
    let car_id = car.id;
    if !scout_clear(actions, observation, map_analysis, memory, intents, false) {
        return;
    }
    let state = &mut memory.third_pressure;
    if observation.tick < state.fallback_until {
        return;
    }
    if let Some(point) = state.watch.map(unrounded) {
        send(actions, observation, state, &[car_id], point, intents);
    }
}
