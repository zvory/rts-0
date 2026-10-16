//! Two Riflemen go out first. The lead walks the flank route to the target base with the other a
//! leg behind it, sweeps every spot that could hold the base's Depot while the other waits at the
//! staging point, and watches a found Depot from outside to size up the garrison. Only then are the
//! Tanks and three more Riflemen sent, so an empty or defended base costs two Riflemen's walk and
//! never the push's Tanks. No Scout Car goes, and none is trained for the raid.
//!
//! The scouts keep out of harm's way: they move with plain move orders (they never attack), turn
//! away from any enemy fighter within `SCOUT_DANGER_TILES` (enemy sight is 9-11 tiles), wait before
//! going on, and keep out of reach of enemy buildings' defense zones. Strung out a leg apart, one
//! found is not both: a hurt scout comes home on its own and the other carries on, leading. The
//! trip is given up when both are gone or they keep being found.

use super::locate::{
    area_center, base_empty, clear_seen, depot_spots, found_depot, next_view, staging_point,
    survey_point,
};
use super::*;

/// Enemy fighters this close to a scout turn it away; Tanks, which reach 14 tiles parked, from
/// farther.
const SCOUT_DANGER_TILES: f32 = 14.0;
const SCOUT_TANK_DANGER_TILES: f32 = 16.0;
/// A scout turns this far away from them, then the scouts wait before going on.
const SCOUT_FLEE_TILES: f32 = 10.0;
const SCOUT_FLEE_WAIT_TICKS: u32 = config::TICK_HZ * 8;
/// Turned away more often than this away from home, the scouts give the trip up. Near home the
/// enemies are raiders home defense answers, and the scouts wait them out.
const SCOUT_MAX_FLEES: u32 = 4;
/// Enemies met lately are walked around: the way on keeps this far from where they were seen,
/// for this long, with at most so many waypoints added on one trip.
const DANGER_CLEAR_TILES: f32 = 16.0;
const DANGER_MEMORY_TICKS: u32 = config::TICK_HZ * 30;
const MAX_DETOURS: u32 = 6;
/// Below this share of its hit points a scout comes home.
const SCOUT_ABORT_HP_PERCENT: u32 = 70;
const SCOUT_TRIP_TIMEOUT_TICKS: u32 = config::TICK_HZ * 240;
/// Sized up, a found Depot is waited on this long, out of sight at the staging point, for a party
/// to become free before the scouts come home: they leave before the Tanks can be spared.
const COMMIT_WAIT_TICKS: u32 = config::TICK_HZ * 120;

/// A scout turned away: its id, where it stands, and the centre of the enemies near it.
type Threatened = (u32, (f32, f32), (f32, f32));

/// Sends the scout Riflemen alone toward `target`, the first of them leading.
pub(super) fn begin_scout(
    observation: &AiObservation,
    state: &mut ThirdPressure,
    scouts: Vec<u32>,
    target: EnemyBase,
    route: Vec<(f32, f32)>,
) {
    let tick = observation.tick;
    state.tanks.clear();
    state.riflemen = scouts.clone();
    state.scouts = scouts;
    state.spotter = None;
    state.target = Some(rounded(target.depot.unwrap_or(target.center)));
    state.area = target.patches.iter().map(|p| rounded(*p)).collect();
    state.route = route.into_iter().map(rounded).collect();
    state.reset_trip(tick);
    state.phase = Phase::Scout;
    state.phase_since = tick;
}

/// The scouts still out, the lead first.
fn scouts<'o>(observation: &'o AiObservation, state: &ThirdPressure) -> Vec<&'o AiEntitySummary> {
    state
        .scouts
        .iter()
        .filter_map(|id| {
            observation
                .owned
                .iter()
                .find(|unit| unit.id == *id && unit.hp > 0)
        })
        .collect()
}

pub(super) fn hurt(unit: &AiEntitySummary) -> bool {
    let max = config::unit_stats(unit.kind).map_or(unit.hp, |stats| stats.hp);
    unit.hp.saturating_mul(100) < max.saturating_mul(SCOUT_ABORT_HP_PERCENT)
}

/// Lets hurt scouts go home on their own. Returns whether any scout is left.
fn send_hurt_home(
    observation: &AiObservation,
    state: &mut ThirdPressure,
    intents: &mut Vec<AiIntent>,
) -> bool {
    let hurt_ids: Vec<u32> = scouts(observation, state)
        .into_iter()
        .filter(|unit| hurt(unit))
        .map(|unit| unit.id)
        .collect();
    if !hurt_ids.is_empty() {
        state.release(&hurt_ids);
        state.notes.push(format!("scouts {hurt_ids:?} hurt, home"));
        intents.push(AiIntent::Assemble { units: hurt_ids });
    }
    !state.scouts.is_empty()
}

/// Turns each scout away from the enemy fighters close enough to see it. Returns whether all are
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
    let threatened: Vec<Threatened> = scouts(observation, &memory.third_pressure)
        .into_iter()
        .filter_map(|scout| {
            let at = (scout.x, scout.y);
            let threats: Vec<(f32, f32)> = observation
                .visible_enemies
                .iter()
                .filter(|enemy| {
                    enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker
                })
                .filter(|enemy| {
                    let danger = if enemy.kind == EntityKind::Tank {
                        SCOUT_TANK_DANGER_TILES
                    } else {
                        SCOUT_DANGER_TILES
                    };
                    distance((enemy.x, enemy.y), at) <= danger * ts
                })
                .map(|enemy| (enemy.x, enemy.y))
                .collect();
            let n = threats.len() as f32;
            (n > 0.0).then(|| {
                let threat = threats
                    .iter()
                    .fold((0.0, 0.0), |sum, p| (sum.0 + p.0 / n, sum.1 + p.1 / n));
                (scout.id, at, threat)
            })
        })
        .collect();
    if threatened.is_empty() {
        return true;
    }
    let home = home_point(observation, memory);
    let state = &mut memory.third_pressure;
    for (_, _, threat) in &threatened {
        note_danger(state, *threat, tick, ts);
    }
    let away_from_home = threatened
        .iter()
        .any(|(_, at, _)| distance(*at, home) > DISBAND_NEAR_HOME_TILES * ts);
    if counting && away_from_home && tick >= state.fallback_until {
        state.flees = state.flees.saturating_add(1);
        if state.flees > SCOUT_MAX_FLEES {
            begin_return(observation, memory, intents, "scouts spotted");
            return false;
        }
    }
    let back_along_route = state
        .leg
        .checked_sub(1)
        .and_then(|leg| state.route.get(leg))
        .map(|p| unrounded(*p));
    for (id, at, threat) in threatened {
        let flee = normalized_direction(threat, at)
            .map(|away| {
                clamp_to_map(
                    (
                        at.0 + away.0 * SCOUT_FLEE_TILES * ts,
                        at.1 + away.1 * SCOUT_FLEE_TILES * ts,
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
        send(actions, observation, state, &[id], flee, intents);
    }
    state.fallback_until = tick.saturating_add(SCOUT_FLEE_WAIT_TICKS);
    false
}

/// Remembers enemies seen at `at`, merging with a danger noted nearby.
fn note_danger(state: &mut ThirdPressure, at: (f32, f32), tick: u32, ts: f32) {
    state
        .dangers
        .retain(|(_, seen)| tick.saturating_sub(*seen) <= DANGER_MEMORY_TICKS);
    match state
        .dangers
        .iter_mut()
        .find(|(spot, _)| distance(unrounded(*spot), at) <= 6.0 * ts)
    {
        Some(danger) => *danger = (rounded(at), tick),
        None => state.dangers.push((rounded(at), tick)),
    }
}

/// When the way from `from` to the next waypoint passes within `DANGER_CLEAR_TILES` of enemies met
/// lately, adds a waypoint beside them, on the side away from them, so the scouts walk around.
fn walk_around(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    state: &mut ThirdPressure,
    from: (f32, f32),
) {
    let tick = observation.tick;
    let ts = tile(observation);
    if state.detours >= MAX_DETOURS {
        return;
    }
    let Some(to) = state.route.get(state.leg).map(|p| unrounded(*p)) else {
        return;
    };
    let clear = DANGER_CLEAR_TILES * ts;
    let fresh: Vec<(f32, f32)> = state
        .dangers
        .iter()
        .filter(|(_, seen)| tick.saturating_sub(*seen) <= DANGER_MEMORY_TICKS)
        .map(|(spot, _)| unrounded(*spot))
        .collect();
    let walked_around: Vec<(f32, f32)> =
        state.walked_around.iter().map(|p| unrounded(*p)).collect();
    let in_the_way = fresh
        .iter()
        .copied()
        .filter(|danger| {
            walked_around
                .iter()
                .all(|done| distance(*done, *danger) > 8.0 * ts)
        })
        .filter(|danger| distance(closest_on(from, to, *danger), *danger) < clear)
        .min_by(|left, right| distance(from, *left).total_cmp(&distance(from, *right)));
    let Some(danger) = in_the_way else {
        return;
    };
    let closest = closest_on(from, to, danger);
    let side = normalized_direction(danger, closest)
        .or_else(|| normalized_direction(from, to).map(|(x, y)| (-y, x)))
        .unwrap_or((0.0, 1.0));
    let passable = |point: (f32, f32)| {
        map_analysis.is_none_or(|analysis| {
            analysis.tile_is_passable((point.0 / ts) as u32, (point.1 / ts) as u32)
        })
    };
    let around = [1.0, -1.0]
        .into_iter()
        .map(|sign| {
            clamp_to_map(
                (
                    danger.0 + side.0 * sign * (clear + 2.0 * ts),
                    danger.1 + side.1 * sign * (clear + 2.0 * ts),
                ),
                observation.map,
            )
        })
        .find(|point| {
            passable(*point) && fresh.iter().all(|other| distance(*point, *other) >= clear)
        });
    if let Some(point) = around {
        if distance(point, to) > 4.0 * ts {
            state.route.insert(state.leg, rounded(point));
            state.leg_since = tick;
            state.detours += 1;
            state.walked_around.push(rounded(danger));
            state
                .notes
                .push(format!("scouts walk around {:?}", rounded(danger)));
        }
    }
}

/// The route's waypoint a leg before its staging point.
fn leg_back(state: &ThirdPressure) -> Option<(f32, f32)> {
    let n = state.route.len();
    n.checked_sub(2)
        .and_then(|i| state.route.get(i))
        .map(|p| unrounded(*p))
}

/// The point of the segment from `a` to `b` nearest `p`.
fn closest_on(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> (f32, f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length2 = dx * dx + dy * dy;
    if length2 <= f32::EPSILON {
        return a;
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length2).clamp(0.0, 1.0);
    (a.0 + dx * t, a.1 + dy * t)
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
    if memory.third_pressure.scouts.is_empty() {
        // Lost on the way: the base they went to is left alone a while.
        let center = area_center(&memory.third_pressure);
        let state = &mut memory.third_pressure;
        state.defended.push((rounded(center), tick));
        begin_return(observation, memory, intents, "scouts lost");
        return;
    }
    // Waiting on a sized-up Depot has its own limit.
    if memory.third_pressure.sized_up.is_none()
        && tick.saturating_sub(memory.third_pressure.phase_since) > SCOUT_TRIP_TIMEOUT_TICKS
    {
        begin_return(observation, memory, intents, "scouts out of time");
        return;
    }
    if !send_hurt_home(observation, &mut memory.third_pressure, intents) {
        begin_return(observation, memory, intents, "scouts hurt");
        return;
    }
    if !scout_clear(actions, observation, map_analysis, memory, intents, true) {
        return;
    }
    let state = &mut memory.third_pressure;
    if tick < state.fallback_until {
        return;
    }
    let out = scouts(observation, state);
    let Some(lead) = out.first().copied() else {
        return;
    };
    let lead_at = (lead.x, lead.y);
    let rest: Vec<u32> = out.iter().skip(1).map(|unit| unit.id).collect();
    // On the way: the lead walks the route, the others two legs behind it, as the lead turns to
    // the next waypoint 6 tiles before each.
    if let Some(point) = state.route.get(state.leg).map(|p| unrounded(*p)) {
        if distance(lead_at, point) <= LEG_REACHED_TILES * ts
            || tick.saturating_sub(state.leg_since) > LEG_TIMEOUT_TICKS
        {
            state.leg += 1;
            state.leg_since = tick;
        }
        walk_around(observation, map_analysis, state, lead_at);
        if let Some(point) = state.route.get(state.leg).map(|p| unrounded(*p)) {
            send(actions, observation, state, &[lead.id], point, intents);
            let behind = state
                .leg
                .checked_sub(2)
                .and_then(|leg| state.route.get(leg))
                .map(|p| unrounded(*p));
            if let Some(behind) = behind {
                send(actions, observation, state, &rest, behind, intents);
            }
            return;
        }
    }
    // At the base: the lead finds the Depot while the others wait a leg back from the staging
    // point.
    let center = area_center(state);
    let staging = staging_point(state, center);
    let back = leg_back(state).unwrap_or(staging);
    if let Some(since) = state.sized_up {
        // The Depot is sized up: they wait for the party out of sight, still a leg apart.
        send(actions, observation, state, &[lead.id], staging, intents);
        send(actions, observation, state, &rest, back, intents);
        if commit(observation, map_analysis, memory) {
            return;
        }
        if tick.saturating_sub(since) > COMMIT_WAIT_TICKS {
            begin_return(observation, memory, intents, "no party free");
        }
        return;
    }
    send(actions, observation, state, &rest, back, intents);
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
        let point = survey_point(observation, map_analysis, depot, staging, sight(lead));
        send(actions, observation, state, &[lead.id], point, intents);
        let arrived = distance(lead_at, point) <= 2.0 * ts;
        if !arrived && tick.saturating_sub(since) < SURVEY_TIMEOUT_TICKS {
            return;
        }
        report(observation, map_analysis, memory, intents, depot);
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
    clear_seen(observation, state, &out);
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
    if let Some(view) = next_view(observation, map_analysis, state, &[lead], center, staging) {
        send(actions, observation, state, &[lead.id], view, intents);
    }
}

/// The lead scout has watched the found Depot: a garrison the party would lose to keeps the Tanks
/// home; otherwise the party is sent, now or once Jeff can spare it, while the scouts wait at the
/// staging point.
fn report(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    depot: (f32, f32),
) {
    let tick = observation.tick;
    let ts = tile(observation);
    let garrison: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| distance((enemy.x, enemy.y), depot) <= GARRISON_TILES * ts)
        .collect();
    if !garrison.is_empty() {
        let would_send: Vec<u32> = match party(observation, memory) {
            Some((tanks, riflemen)) => tanks.into_iter().chain(riflemen).collect(),
            // The Tanks cannot be spared yet: weigh the garrison against two of Jeff's.
            None => {
                let mut tanks: Vec<&AiEntitySummary> = observation
                    .owned
                    .iter()
                    .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
                    .collect();
                tanks.sort_by(|left, right| right.hp.cmp(&left.hp).then(left.id.cmp(&right.id)));
                tanks
                    .iter()
                    .take(PARTY_TANKS)
                    .map(|unit| unit.id)
                    .chain(memory.third_pressure.scouts.iter().copied())
                    .collect()
            }
        };
        if would_send.is_empty() || loses_to(observation, &would_send, &garrison) {
            let center = area_center(&memory.third_pressure);
            memory.third_pressure.defended.push((rounded(center), tick));
            begin_return(observation, memory, intents, "base defended");
            return;
        }
    }
    memory.third_pressure.sized_up = Some(tick);
    commit(observation, map_analysis, memory);
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

/// The party that would follow the scouts now: two army Tanks and enough Riflemen to make five
/// with the scouts.
fn party(observation: &AiObservation, memory: &AiDecisionMemory) -> Option<(Vec<u32>, Vec<u32>)> {
    let home = home_point(observation, memory);
    let scouts = &memory.third_pressure.scouts;
    let more = PARTY_RIFLEMEN.saturating_sub(scouts.len());
    let tanks = party_tanks(observation, memory)?;
    let riflemen = party_riflemen(observation, memory, home, more, scouts)?;
    Some((tanks, riflemen))
}

/// Sends two army Tanks and three more Riflemen to the base the scouts found, by the flank route,
/// when Jeff can spare them now; the scouts wait for them at the staging point. Returns whether
/// the party left.
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
    let Some((tanks, more)) = party(observation, memory) else {
        return false;
    };
    // Enemies beside the units that would go would turn the party straight back: wait.
    let ts = tile(observation);
    let leaving: Vec<u32> = tanks.iter().chain(&more).copied().collect();
    let contact = units(observation, &leaving).iter().any(|unit| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && enemy.kind != EntityKind::Worker
                && distance((unit.x, unit.y), (enemy.x, enemy.y)) <= THREAT_TILES * ts
        })
    });
    if contact {
        return false;
    }
    let tick = observation.tick;
    let state = &mut memory.third_pressure;
    let Some(goal) = state.target.map(unrounded) else {
        return false;
    };
    state
        .notes
        .push(format!("party sent to {:?}", rounded(goal)));
    state.tanks = tanks;
    state.riflemen = state.scouts.iter().chain(&more).copied().collect();
    state.route = flank_route(observation, map_analysis, home, goal)
        .into_iter()
        .map(rounded)
        .collect();
    state.watch = if state.scouts.is_empty() {
        None
    } else {
        state.route.last().copied()
    };
    state.leg = 0;
    state.leg_since = tick;
    state.phase = Phase::March;
    state.phase_since = tick;
    state.survey_since = None;
    state.sized_up = None;
    state.clear_orders();
    true
}

/// While the party marches, the scouts wait for it at the staging point, turning away from
/// enemies; a hurt one goes home.
pub(super) fn keep_watch(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    if !send_hurt_home(observation, &mut memory.third_pressure, intents) {
        memory.third_pressure.watch = None;
        return;
    }
    if !scout_clear(actions, observation, map_analysis, memory, intents, false) {
        return;
    }
    let state = &mut memory.third_pressure;
    if observation.tick < state.fallback_until {
        return;
    }
    if let Some(point) = state.watch.map(unrounded) {
        let ids = state.scouts.clone();
        let back = leg_back(state).unwrap_or(point);
        if let Some((lead, rest)) = ids.split_first() {
            send(actions, observation, state, &[*lead], point, intents);
            send(actions, observation, state, rest, back, intents);
        }
    }
}
