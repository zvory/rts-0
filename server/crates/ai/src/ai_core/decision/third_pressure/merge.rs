//! Joining the main push once the raid is over. A base found empty, a Depot not found in time, or a
//! base cleared sends the party to meet the push rather than home: on the push's way from its rally
//! point to the enemy main, at the point nearest the party (in the middle stretch while the push
//! still forms at home; ahead of it once it marches). The push never waits for the party; its
//! Tanks count toward the push's size while the party is out. When the push reaches the party, the
//! party's units become the push's.

use super::*;

/// The party joins the push once the push's Tanks are this close.
const MERGE_JOIN_TILES: f32 = 10.0;
/// A party that met no push in this long goes home.
const MERGE_WAIT_TICKS: u32 = config::TICK_HZ * 150;
/// While the push forms, the meeting point lies on this stretch of its way, as shares of the
/// distance from its rally point to the enemy main.
const MERGE_SPAN: (f32, f32) = (0.3, 0.7);
/// The push forms this far from the main toward the enemy (its regroup point).
const PUSH_RALLY_TILES: f32 = 8.0;

/// Ends the raid by heading to meet the push.
pub(super) fn begin_merge(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    why: &str,
) {
    if memory.third_pressure.tanks.is_empty() {
        begin_return(observation, memory, intents, why);
        return;
    }
    let state = &mut memory.third_pressure;
    state.notes.push(format!("merge: {why}"));
    state.phase = Phase::Merge;
    state.phase_since = observation.tick;
    state.clear_orders();
}

pub(super) fn merge(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let tick = observation.tick;
    let ts = tile(observation);
    let state = &memory.third_pressure;
    if state.tanks.is_empty() || tick.saturating_sub(state.phase_since) > MERGE_WAIT_TICKS {
        begin_return(observation, memory, intents, "no push met");
        return;
    }
    if !answer_threat(actions, observation, memory, intents, None) {
        return;
    }
    let party = units(observation, &memory.third_pressure.members());
    let Some(at) = centroid(&party) else {
        return;
    };
    let Some(enemy) = enemy_start(observation) else {
        begin_return(observation, memory, intents, "no enemy main");
        return;
    };
    let push_tanks: Vec<u32> = memory.containment.active_tanks.iter().copied().collect();
    let marching = memory.containment.wave_launched && !memory.containment.recovery_active;
    let push_center = marching
        .then(|| centroid(&units(observation, &push_tanks)))
        .flatten();
    if let Some(center) = push_center {
        if distance(center, at) <= MERGE_JOIN_TILES * ts {
            join_push(observation, memory, intents);
            return;
        }
    }
    let (from, span) = match push_center {
        Some(center) => (center, (0.0, 1.0)),
        None => {
            let own = tile_center(observation.own_start_tile, observation.map.tile_size);
            let dir = normalized_direction(own, enemy).unwrap_or((0.0, 0.0));
            let rally = (
                own.0 + dir.0 * PUSH_RALLY_TILES * ts,
                own.1 + dir.1 * PUSH_RALLY_TILES * ts,
            );
            (rally, MERGE_SPAN)
        }
    };
    let point = meeting_point(observation, map_analysis, from, enemy, at, span);
    let state = &mut memory.third_pressure;
    let ids = state.members();
    send(actions, observation, state, &ids, point, intents);
    // Waiting there, the party holds rather than wandering after what it sees.
    let arrived: Vec<u32> = party
        .iter()
        .filter(|unit| distance((unit.x, unit.y), point) <= LEG_REACHED_TILES * ts)
        .filter(|unit| unit.state == AiEntityState::Idle)
        .map(|unit| unit.id)
        .collect();
    let hold: Vec<u32> = arrived
        .into_iter()
        .filter(|id| state.held.insert(*id))
        .collect();
    actions::hold_position_units(actions, hold);
}

/// The point of the segment `from`-`to` nearest `at`, kept within `span` (shares of its length),
/// moved along the segment to the nearest passable ground.
fn meeting_point(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    from: (f32, f32),
    to: (f32, f32),
    at: (f32, f32),
    span: (f32, f32),
) -> (f32, f32) {
    let seg = (to.0 - from.0, to.1 - from.1);
    let len2 = seg.0 * seg.0 + seg.1 * seg.1;
    let share = if len2 <= f32::EPSILON {
        0.0
    } else {
        (((at.0 - from.0) * seg.0 + (at.1 - from.1) * seg.1) / len2).clamp(span.0, span.1)
    };
    let ts = tile(observation);
    let point_at = |share: f32| {
        clamp_to_map(
            (from.0 + seg.0 * share, from.1 + seg.1 * share),
            observation.map,
        )
    };
    (0..=20)
        .flat_map(|step| [share - step as f32 * 0.02, share + step as f32 * 0.02])
        .filter(|share| (0.0..=1.0).contains(share))
        .map(point_at)
        .find(|point| {
            map_analysis.is_none_or(|analysis| {
                analysis.tile_is_passable((point.0 / ts) as u32, (point.1 / ts) as u32)
            })
        })
        .unwrap_or_else(|| point_at(share))
}

/// The party's units become the push's: its Tanks and Riflemen march with it.
fn join_push(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let state = &memory.third_pressure;
    let tanks = state.tanks.clone();
    let riflemen = state.riflemen.clone();
    memory.containment.active_tanks.extend(tanks);
    memory.containment.active_riflemen.extend(riflemen);
    let state = &mut memory.third_pressure;
    state.notes.push("joined the push".to_string());
    disband(observation.tick, state, intents);
}
