//! Jeff's AI V1.1's southern route on Schone Tage (`southroute`).
//!
//! Schone Tage's V of 20 Tank Traps lies across the direct way between the mains. The path planner
//! routes Tanks into it (its diagonal arms look porous) and they stick; the trap breach that shoots
//! it down held the whole Tank army at the V for minutes: against Pressure AI 13-25 Tanks at the trap
//! line at 8:00-9:00 and 12:00 with no push out and no fourth base, and against Rush AI 18-21 there
//! from 11:00 to 12:30 while 22 Rush Tanks took the main (the 24 live-fix test games, 2026-11-04).
//! A trap-free way runs south through each main's road gap in the wall at y 86.5, east along y 84-94
//! north of the central plateau, and up through the enemy's gap: 228 tiles against 150.
//!
//! Under `southroute` V1.1 skips the trap breach there, and its push marches waypoint by waypoint
//! along that way before its usual targeting takes over past the enemy's gap; falling back, or
//! called home to strike, it walks the waypoints back; Tanks joining it in a convoy follow them up
//! to where the push is.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

/// The way from the west main (8,47), in tiles; the east main's is its mirror (x to 165 - x).
const WEST_WAY: [(f32, f32); 10] = [
    (8.5, 72.5),
    (10.5, 90.5),
    (30.5, 94.5),
    (55.5, 93.5),
    (73.5, 86.5),
    (93.5, 84.5),
    (113.5, 84.5),
    (130.5, 93.5),
    (150.5, 92.5),
    (155.5, 80.5),
];
/// A group whose centre comes this close to a waypoint has reached it.
const ARRIVED_TILES: f32 = 7.0;

/// Where the push and its convoy are along the way.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct SouthRouteMemory {
    /// The next waypoint the push makes for; past the last, its usual targeting.
    leg: usize,
    /// The next waypoint the convoy makes for.
    convoy_leg: usize,
    /// When the push was last ordered on: the march keeps its own clock, since the shared
    /// formation clock is reset by other orders (after a fall-back the push sat at home on the
    /// first waypoint's order for two minutes).
    last_order: Option<u32>,
    /// What the last step decided, for the trace.
    last_step: String,
}

/// The marching push is ordered on this often.
const ORDER_TICKS: u32 = config::TICK_HZ * 2;

/// Whether the marching push is due another order; notes it when it is.
pub(in crate::ai_core::decision) fn order_due(memory: &mut AiDecisionMemory, tick: u32) -> bool {
    let south = &mut memory.jeff_v1_1.south;
    let due = south
        .last_order
        .is_none_or(|last| tick.saturating_sub(last) >= ORDER_TICKS || last > tick);
    if due {
        south.last_order = Some(tick);
    }
    due
}

/// The way in world pixels for this game, or `None` off Schone Tage, for another profile, or with
/// `southroute` off.
pub(in crate::ai_core::decision) fn way(
    profile_id: &str,
    observation: &AiObservation,
) -> Option<Vec<(f32, f32)>> {
    if !is_jeff_v1_1(profile_id) || super::off("southroute") {
        return None;
    }
    let west = match (observation.map.width, observation.map.height, observation.own_start_tile) {
        (166, 166, (8, 47)) => true,
        (166, 166, (157, 47)) => false,
        _ => return None,
    };
    let ts = observation.map.tile_size as f32;
    Some(
        WEST_WAY
            .iter()
            .map(|&(x, y)| {
                let x = if west { x } else { 166.0 - x };
                (x * ts, y * ts)
            })
            .collect(),
    )
}

/// Whether the launched push is marching the way (not past it, not striking home).
pub(in crate::ai_core::decision) fn marching(observation: &AiObservation, memory: &AiDecisionMemory) -> bool {
    let Some(way) = memory.profile_id().and_then(|profile| way(profile, observation)) else {
        return false;
    };
    memory.containment.wave_launched
        && !memory.containment.recovery_active
        && !memory.jeff_v1_1.push.striking()
        && memory.jeff_v1_1.south.leg < way.len()
}

/// Whether V1.1 leaves the trap breach off here.
pub(in crate::ai_core::decision) fn skips_breach(profile_id: &str, observation: &AiObservation) -> bool {
    way(profile_id, observation).is_some()
}

fn nearest_leg(way: &[(f32, f32)], at: (f32, f32)) -> usize {
    (0..way.len())
        .min_by(|a, b| {
            dist2(way[*a].0, way[*a].1, at.0, at.1).total_cmp(&dist2(way[*b].0, way[*b].1, at.0, at.1))
        })
        .unwrap_or(0)
}

/// The push leaves: it picks up the way at the waypoint nearest it.
pub(in crate::ai_core::decision) fn start(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    center: Option<(f32, f32)>,
) {
    let Some(profile) = memory.profile_id() else {
        return;
    };
    if let (Some(way), Some(center)) = (way(profile, observation), center) {
        memory.jeff_v1_1.south.leg = nearest_leg(&way, center);
    }
}

/// Where the launched push makes for while on the way: the next waypoint, or the one behind it
/// while it strikes back home; `None` once past the way (or off it), for its usual targeting.
pub(in crate::ai_core::decision) fn push_waypoint(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    center: (f32, f32),
) -> Option<(f32, f32)> {
    let profile = memory.profile_id()?;
    let way = way(profile, observation)?;
    if memory.jeff_v1_1.push.striking() {
        return back_waypoint(observation, memory, center);
    }
    let ts = observation.map.tile_size as f32;
    let south = &mut memory.jeff_v1_1.south;
    while south.leg < way.len() {
        let at = way[south.leg];
        if dist2(center.0, center.1, at.0, at.1) > squared(ARRIVED_TILES * ts) {
            south.last_step = format!("t{} to {} from ({:.0},{:.0})", observation.tick, south.leg, center.0 / ts, center.1 / ts);
            return Some(at);
        }
        south.leg += 1;
    }
    south.last_step = format!("t{} past the way", observation.tick);
    None
}

/// The waypoint behind the push, for falling back or striking home along the way; `None` at the
/// start of the way (or off it).
pub(in crate::ai_core::decision) fn back_waypoint(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    center: (f32, f32),
) -> Option<(f32, f32)> {
    let profile = memory.profile_id()?;
    let way = way(profile, observation)?;
    let ts = observation.map.tile_size as f32;
    let south = &mut memory.jeff_v1_1.south;
    south.leg = south.leg.min(way.len());
    while south.leg > 0 {
        let at = way[south.leg - 1];
        if dist2(center.0, center.1, at.0, at.1) > squared(ARRIVED_TILES * ts) {
            return Some(at);
        }
        south.leg -= 1;
    }
    None
}

/// Where a convoy at `center` heading for the push makes for: the next waypoint until it has
/// caught up with the push along the way, then the push itself.
pub(in crate::ai_core::decision) fn convoy_waypoint(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    center: (f32, f32),
    push: (f32, f32),
    fresh: bool,
) -> (f32, f32) {
    let Some(way) = memory.profile_id().and_then(|profile| way(profile, observation)) else {
        return push;
    };
    let ts = observation.map.tile_size as f32;
    let south = &mut memory.jeff_v1_1.south;
    if fresh {
        south.convoy_leg = nearest_leg(&way, center);
    }
    while south.convoy_leg < south.leg.min(way.len()) {
        let at = way[south.convoy_leg];
        if dist2(center.0, center.1, at.0, at.1) > squared(ARRIVED_TILES * ts) {
            return at;
        }
        south.convoy_leg += 1;
    }
    push
}

/// The trace note.
pub(in crate::ai_core::decision) fn trace(memory: &AiDecisionMemory) -> String {
    format!(
        "v1_1_south leg={} convoy_leg={} step=[{}]",
        memory.jeff_v1_1.south.leg, memory.jeff_v1_1.south.convoy_leg, memory.jeff_v1_1.south.last_step
    )
}
