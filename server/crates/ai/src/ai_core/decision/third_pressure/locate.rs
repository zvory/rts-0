//! Finding the target base's Depot before the siege. AI 2.1 may stand it anywhere that mines a
//! patch, so every spot within mining reach of the base's patches is looked at.
//!
//! The Tanks and most Riflemen wait at the staging point while the Scout Car looks. Contact near
//! the waiting group is answered as on the march; contact near the looker alone either sends the
//! party home (a force it would lose to) or brings the looker back to the group, which holds
//! rather than charging into the base. A found Depot is sized up first: the looker watches it from
//! outside, and a garrison the party would lose to turns it home and leaves the base alone a while.
//! A base with nothing at any spot is empty; a Depot seen at another base near the party then
//! becomes the target, else the party goes home.

use super::*;

/// The looker sizes the found Depot up from this far inside its own sight.
const SURVEY_BACKOFF_TILES: f32 = 3.0;
const SURVEY_REACHED_TILES: f32 = 2.0;
const SURVEY_TIMEOUT_TICKS: u32 = config::TICK_HZ * 15;
/// Enemy fighters this close to the Depot are its garrison.
const GARRISON_TILES: f32 = 16.0;
/// After falling back from contact the looker waits with the party this long.
const FALLBACK_TICKS: u32 = config::TICK_HZ * 6;
/// A Depot seen at another base this close to the party becomes the target in the field.
const FIELD_RETARGET_TILES: f32 = 50.0;
/// A search out of time that has seen this share of the spots, and no Depot, found the base empty.
const EMPTY_SEEN_PERCENT: usize = 75;

/// At the staging point: every spot within mining reach of the base's patches could hold its
/// Depot, so they are all to be seen before the base counts as empty. The Scout Car looks (or the
/// two healthiest Riflemen without one) while the rest wait.
pub(super) fn begin_locate(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
) {
    let state = &mut memory.third_pressure;
    let area: Vec<(f32, f32)> = state.area.iter().map(|p| unrounded(*p)).collect();
    state.unseen = depot_spots(observation, map_analysis, &area)
        .into_iter()
        .map(rounded)
        .collect();
    state.spots_total = state.unseen.len();
    state.locator = locators(observation, state);
    state.survey_since = None;
    state.fallback_until = 0;
    state.phase = Phase::Locate;
    state.phase_since = observation.tick;
    state.clear_orders();
    locate(actions, observation, map_analysis, memory, intents);
}

/// Passable spots on a grid within `MINING_ANCHOR_RANGE_TILES` of a patch and off the patches.
fn depot_spots(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    area: &[(f32, f32)],
) -> Vec<(f32, f32)> {
    let ts = tile(observation);
    let reach = config::MINING_ANCHOR_RANGE_TILES * ts;
    let step = DEPOT_SEARCH_STEP_TILES * ts;
    let Some(min_x) = area.iter().map(|p| p.0).reduce(f32::min) else {
        return Vec::new();
    };
    let max_x = area.iter().map(|p| p.0).fold(f32::MIN, f32::max);
    let min_y = area.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    let max_y = area.iter().map(|p| p.1).fold(f32::MIN, f32::max);
    let width = (observation.map.width * observation.map.tile_size) as f32;
    let height = (observation.map.height * observation.map.tile_size) as f32;
    let mut spots = Vec::new();
    let mut y = min_y - reach;
    while y <= max_y + reach {
        let mut x = min_x - reach;
        while x <= max_x + reach {
            let spot = (x, y);
            if x >= 0.0
                && y >= 0.0
                && x < width
                && y < height
                && area.iter().any(|patch| distance(*patch, spot) <= reach)
                && area.iter().all(|patch| distance(*patch, spot) >= 1.5 * ts)
                && map_analysis.is_none_or(|analysis| {
                    analysis.tile_is_passable((x / ts) as u32, (y / ts) as u32)
                })
            {
                spots.push(spot);
            }
            x += step;
        }
        y += step;
    }
    spots
}

/// The Scout Car, or the two healthiest Riflemen.
fn locators(observation: &AiObservation, state: &ThirdPressure) -> Vec<u32> {
    if let Some(scout) = state.scout {
        return vec![scout];
    }
    let mut riflemen = units(observation, &state.riflemen);
    riflemen.sort_by(|left, right| right.hp.cmp(&left.hp).then(left.id.cmp(&right.id)));
    riflemen.iter().take(2).map(|unit| unit.id).collect()
}

pub(super) fn sight(unit: &AiEntitySummary) -> f32 {
    config::unit_stats(unit.kind).map_or(0.0, |stats| stats.sight_tiles as f32)
}

/// The centre of the target base's patches.
fn area_center(state: &ThirdPressure) -> (f32, f32) {
    let n = state.area.len().max(1) as f32;
    state.area.iter().fold((0.0, 0.0), |sum, p| {
        (sum.0 + p.0 as f32 / n, sum.1 + p.1 as f32 / n)
    })
}

fn staging_point(state: &ThirdPressure, center: (f32, f32)) -> (f32, f32) {
    state
        .route
        .last()
        .map(|point| unrounded(*point))
        .unwrap_or(center)
}

/// Visible enemy fighters within `tiles` of any of `units`.
fn fighters_near<'o>(
    observation: &'o AiObservation,
    units: &[&AiEntitySummary],
    tiles: f32,
) -> Vec<&'o AiEntitySummary> {
    let reach = tiles * tile(observation);
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| {
            units
                .iter()
                .any(|unit| distance((unit.x, unit.y), (enemy.x, enemy.y)) <= reach)
        })
        .collect()
}

/// Whether `enemies` would beat the whole party, or bring at least as many Tanks.
fn party_loses_to(
    observation: &AiObservation,
    state: &ThirdPressure,
    enemies: &[&AiEntitySummary],
) -> bool {
    let meth = observation
        .upgrades
        .contains(&UpgradeKind::Methamphetamines);
    let ours: Vec<_> = units(observation, &state.members())
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
        || (their_tanks > 0 && their_tanks >= state.tanks.len())
}

/// Contact while the party looks for or sizes up the Depot. Returns whether the search carries on
/// this decision.
fn locate_contact(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) -> bool {
    let state = &memory.third_pressure;
    let members = units(observation, &state.members());
    let (lookers, group): (Vec<&AiEntitySummary>, Vec<&AiEntitySummary>) = members
        .iter()
        .copied()
        .partition(|unit| state.locator.contains(&unit.id));
    // At the waiting group: fight or turn home, as on the march.
    if !fighters_near(observation, &group, THREAT_TILES).is_empty() {
        return answer_threat(actions, observation, memory, intents, None);
    }
    let near = fighters_near(observation, &lookers, THREAT_TILES);
    if near.is_empty() {
        return true;
    }
    let area: Vec<(f32, f32)> = state.area.iter().map(|p| unrounded(*p)).collect();
    if party_loses_to(observation, state, &near) {
        // A force like that standing at the base itself means the base is defended.
        let at_base = near
            .iter()
            .any(|enemy| mines_from(observation, (enemy.x, enemy.y), &area));
        let center = area_center(state);
        let state = &mut memory.third_pressure;
        if at_base {
            state.defended.push((rounded(center), observation.tick));
        }
        begin_return(observation, memory, intents, "outmatched at the base");
        return false;
    }
    // The looker comes back to the group; the group holds at the staging point.
    let center = area_center(state);
    let staging = staging_point(state, center);
    let state = &mut memory.third_pressure;
    state.fallback_until = observation.tick.saturating_add(FALLBACK_TICKS);
    let ids = state.members();
    send(actions, observation, state, &ids, staging, intents);
    false
}

pub(super) fn locate(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let tick = observation.tick;
    let ts = tile(observation);
    let state = &memory.third_pressure;
    if state.tanks.is_empty() || memory.home_outgunned {
        begin_return(observation, memory, intents, "depot not found");
        return;
    }
    if tick.saturating_sub(state.phase_since) > LOCATE_TIMEOUT_TICKS {
        // Out of time. Having seen most spots without a Depot, the base counts as empty, so the
        // stop rule also reaches maps where no search finishes in time.
        let seen = state.spots_total.saturating_sub(state.unseen.len());
        if state.survey_since.is_none()
            && state.spots_total > 0
            && seen * 100 >= state.spots_total * EMPTY_SEEN_PERCENT
        {
            let center = area_center(state);
            base_empty(observation, map_analysis, memory, intents, center);
        } else {
            begin_merge(observation, memory, intents, "depot not found");
        }
        return;
    }
    if !locate_contact(actions, observation, memory, intents) {
        return;
    }
    let state = &mut memory.third_pressure;
    let area: Vec<(f32, f32)> = state.area.iter().map(|p| unrounded(*p)).collect();
    let center = area_center(state);
    if units(observation, &state.locator).is_empty() {
        state.locator = locators(observation, state);
    }
    if state.survey_since.is_none() {
        // A Depot seen mining the base is the target, at its exact spot.
        let seen = observation
            .visible_enemies
            .iter()
            .filter(|enemy| enemy.kind == EntityKind::ResourceDepot && enemy.hp > 0)
            .filter(|enemy| mines_from(observation, (enemy.x, enemy.y), &area))
            .min_by(|left, right| {
                distance((left.x, left.y), center).total_cmp(&distance((right.x, right.y), center))
            });
        let remembered = state
            .seen_depots
            .iter()
            .copied()
            .find(|at| mines_from(observation, unrounded(*at), &area));
        if let Some(depot) = seen {
            state.target = Some(rounded((depot.x, depot.y)));
            state.depot = Some(depot.id);
            state.survey_since = Some(tick);
        } else if let Some(at) = remembered {
            state.target = Some(at);
            state.depot = None;
            state.survey_since = Some(tick);
        }
    }
    if state.survey_since.is_some() {
        survey(actions, observation, map_analysis, memory, intents);
        return;
    }
    let members = units(observation, &state.members());
    state.unseen.retain(|spot| {
        let spot = unrounded(*spot);
        !members
            .iter()
            .any(|unit| distance((unit.x, unit.y), spot) <= (sight(unit) - SIGHT_MARGIN_TILES) * ts)
    });
    if state.unseen.is_empty() {
        base_empty(observation, map_analysis, memory, intents, center);
        return;
    }
    let lookers = units(observation, &state.locator);
    let Some(lead) = lookers.first() else {
        begin_merge(observation, memory, intents, "no locator");
        return;
    };
    let staging = staging_point(state, center);
    // Extractors only stand within reach of the Depot that anchors them.
    let extractors: Vec<(f32, f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_building()
                && enemy.kind != EntityKind::ResourceDepot
                && mines_from(observation, (enemy.x, enemy.y), &area)
        })
        .map(|enemy| (enemy.x, enemy.y))
        .collect();
    let reach = config::MINING_ANCHOR_RANGE_TILES * ts;
    let score = |spot: (f32, f32)| {
        let off_extractors = !extractors.is_empty()
            && !extractors
                .iter()
                .any(|extractor| distance(*extractor, spot) <= reach);
        f32::from(u8::from(off_extractors)) * 1_000.0 * ts
            + distance(spot, center)
            + 0.5 * distance(spot, (lead.x, lead.y))
    };
    let Some(spot) = state
        .unseen
        .iter()
        .map(|spot| unrounded(*spot))
        .min_by(|left, right| score(*left).total_cmp(&score(*right)))
    else {
        return;
    };
    let look = lookers
        .iter()
        .map(|unit| sight(unit))
        .fold(f32::MAX, f32::min);
    let back = (look - VIEW_BACKOFF_TILES).max(2.0) * ts;
    // Look from outside the base, else from the staging side, else from where the looker is.
    let view = [
        normalized_direction(center, spot),
        normalized_direction(spot, staging),
        normalized_direction(spot, (lead.x, lead.y)),
    ]
    .into_iter()
    .flatten()
    .map(|dir| {
        clamp_to_map(
            (spot.0 + dir.0 * back, spot.1 + dir.1 * back),
            observation.map,
        )
    })
    .find(|point| passable(observation, map_analysis, *point))
    .unwrap_or(spot);
    let view = if tick < state.fallback_until {
        staging
    } else {
        view
    };
    send_looker_and_group(
        actions,
        observation,
        state,
        &lookers,
        view,
        staging,
        intents,
    );
}

fn passable(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    point: (f32, f32),
) -> bool {
    let ts = tile(observation);
    map_analysis.is_none_or(|analysis| {
        analysis.tile_is_passable((point.0 / ts) as u32, (point.1 / ts) as u32)
    })
}

fn send_looker_and_group(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    state: &mut ThirdPressure,
    lookers: &[&AiEntitySummary],
    look_from: (f32, f32),
    staging: (f32, f32),
    intents: &mut Vec<AiIntent>,
) {
    let looking: Vec<u32> = lookers.iter().map(|unit| unit.id).collect();
    send(actions, observation, state, &looking, look_from, intents);
    let waiting: Vec<u32> = state
        .members()
        .into_iter()
        .filter(|id| !looking.contains(id))
        .collect();
    send(actions, observation, state, &waiting, staging, intents);
}

/// The Depot is found: the looker watches it from outside, on the staging side, while the rest
/// wait. Once it is there (or after `SURVEY_TIMEOUT_TICKS`) the garrison it sees decides: one the
/// party would lose to turns it home and leaves the base alone a while; else the siege begins.
fn survey(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let tick = observation.tick;
    let ts = tile(observation);
    let state = &mut memory.third_pressure;
    let center = area_center(state);
    let Some(depot) = state.target.map(unrounded) else {
        begin_merge(observation, memory, intents, "no target");
        return;
    };
    let staging = staging_point(state, center);
    let lookers = units(observation, &state.locator);
    let look = lookers
        .iter()
        .map(|unit| sight(unit))
        .fold(f32::MAX, f32::min)
        .min(15.0);
    let dir = normalized_direction(depot, staging).unwrap_or((0.0, 1.0));
    let back = (look - SURVEY_BACKOFF_TILES).max(2.0) * ts;
    let point = clamp_to_map(
        (depot.0 + dir.0 * back, depot.1 + dir.1 * back),
        observation.map,
    );
    let point = if passable(observation, map_analysis, point) {
        point
    } else {
        staging
    };
    let watch_from = if tick < state.fallback_until {
        staging
    } else {
        point
    };
    send_looker_and_group(
        actions,
        observation,
        state,
        &lookers,
        watch_from,
        staging,
        intents,
    );
    let arrived = lookers
        .iter()
        .any(|unit| distance((unit.x, unit.y), point) <= SURVEY_REACHED_TILES * ts);
    let since = state.survey_since.unwrap_or(tick);
    if !arrived && !lookers.is_empty() && tick.saturating_sub(since) < SURVEY_TIMEOUT_TICKS {
        return;
    }
    let garrison: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| distance((enemy.x, enemy.y), depot) <= GARRISON_TILES * ts)
        .collect();
    if !garrison.is_empty() && party_loses_to(observation, state, &garrison) {
        state.defended.push((rounded(center), tick));
        begin_return(observation, memory, intents, "base defended");
        return;
    }
    begin_siege(observation, map_analysis, memory, actions, intents);
}

/// Nothing at any spot that could hold the Depot: the base counts as empty. A Depot seen at another
/// base near the party becomes the target; else the party goes home.
fn base_empty(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    center: (f32, f32),
) {
    let state = &mut memory.third_pressure;
    state.checked.push(rounded(center));
    state.empty_bases = state.empty_bases.saturating_add(1);
    match field_target(observation, state, false) {
        Some(base) => retarget(observation, map_analysis, state, base),
        None => begin_merge(observation, memory, intents, "base empty"),
    }
}

/// A base other than the current target where a Depot has been seen, within
/// `FIELD_RETARGET_TILES` of the party. On the march (`only_for_a_guess`) only while the current
/// target's Depot has not been seen.
pub(super) fn field_target(
    observation: &AiObservation,
    state: &ThirdPressure,
    only_for_a_guess: bool,
) -> Option<EnemyBase> {
    let ts = tile(observation);
    let area: Vec<(f32, f32)> = state.area.iter().map(|p| unrounded(*p)).collect();
    let current_known = state.depot.is_some()
        || state
            .seen_depots
            .iter()
            .any(|at| mines_from(observation, unrounded(*at), &area));
    if only_for_a_guess && current_known {
        return None;
    }
    let base = target_site(observation, state)?;
    let depot = base.depot?;
    if distance(base.center, area_center(state)) < SITE_SAME_TILES * ts {
        return None;
    }
    let from = centroid(&units(observation, &state.members()))?;
    (distance(from, depot) <= FIELD_RETARGET_TILES * ts).then_some(base)
}

/// Sends the party from where it stands to `base`, marching again.
pub(super) fn retarget(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    state: &mut ThirdPressure,
    base: EnemyBase,
) {
    let tick = observation.tick;
    let goal = base.depot.unwrap_or(base.center);
    let from = centroid(&units(observation, &state.members())).unwrap_or(goal);
    state.notes.push(format!("retarget {:?}", rounded(goal)));
    state.target = Some(rounded(goal));
    state.area = base.patches.iter().map(|p| rounded(*p)).collect();
    state.depot = None;
    state.killed_at = None;
    state.route = flank_route(observation, map_analysis, from, goal)
        .into_iter()
        .map(rounded)
        .collect();
    state.leg = 0;
    state.leg_since = tick;
    state.phase = Phase::March;
    state.phase_since = tick;
    state.unseen.clear();
    state.locator.clear();
    state.survey_since = None;
    state.fallback_until = 0;
    state.clear_orders();
}
