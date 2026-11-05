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
/// Each wall crossing goes straight down the middle of its 5-tile road gap (x 7-11 west, 154-158
/// east): the wall is a trap every other tile, its 1-tile slots look passable to the planner, and a
/// spread push aimed past the gap stuck on them north of the east wall from 8:46 to 10:11.
const WEST_WAY: [(f32, f32); 12] = [
    (9.5, 74.5),
    (9.5, 82.5),
    (9.5, 91.5),
    (30.5, 94.5),
    (55.5, 93.5),
    (73.5, 86.5),
    (93.5, 84.5),
    (113.5, 84.5),
    (130.5, 93.5),
    (156.5, 91.5),
    (156.5, 82.5),
    (156.5, 74.5),
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
    /// Holding beside the third base this step (`thirdguard`).
    guarding: bool,
    /// Walking the way back to the third base, outweighed (`southhold`).
    returning: bool,
    /// Held beside the third base until then after falling back.
    hold_until: Option<u32>,
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

/// Orders the push's Tanks and its infantry (Scout Car or spotter, escorts) on to `point` in
/// separate commands (`marchorders`). A Rifleman spotter is listed in home defense's stage as
/// well, and the live controller's stage filter then takes every attacking unit out of an order
/// that names a staged one: one order for all of them reached no Tank, and an 11-Tank push stood
/// in its main from 8:20 to 10:10 while the third base fell.
pub(in crate::ai_core::decision) fn order_push(
    actions: &mut AiActionContext<'_>,
    tanks: &[u32],
    infantry: &[u32],
    point: (f32, f32),
) {
    if super::off("marchorders") {
        let mut units = tanks.to_vec();
        units.extend(infantry.iter().copied());
        actions::attack_move_units(actions, units, point.0, point.1);
        return;
    }
    actions::attack_move_units(actions, tanks.iter().copied(), point.0, point.1);
    actions::attack_move_units(actions, infantry.iter().copied(), point.0, point.1);
}

/// The push falling back: V1.1 (`marchorders`) moves its Tanks and its infantry in separate
/// commands, as `order_push`; `false` leaves the order to the caller.
pub(in crate::ai_core::decision) fn order_fall_back(
    actions: &mut AiActionContext<'_>,
    memory: &AiDecisionMemory,
    tanks: &[u32],
    infantry: &[u32],
    point: (f32, f32),
) -> bool {
    if !memory.profile_id().is_some_and(is_jeff_v1_1) || super::off("marchorders") {
        return false;
    }
    actions::move_units(actions, tanks.iter().copied(), point.0, point.1);
    actions::move_units(actions, infantry.iter().copied(), point.0, point.1);
    true
}

/// Whether home defense leaves this Rifleman be: the push's Rifleman spotter (`marchorders`).
pub(in crate::ai_core::decision) fn push_spotter(memory: &AiDecisionMemory, id: u32) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1)
        && !super::off("marchorders")
        && memory.containment.active_scout == Some(id)
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
    let guard = !super::off("thirdguard");
    Some(
        WEST_WAY
            .iter()
            .enumerate()
            .map(|(leg, &(x, y))| {
                let (x, y) = if guard && leg == GUARD_LEG { GUARD_WEST } else { (x, y) };
                let x = if west { x } else { 166.0 - x };
                (x * ts, y * ts)
            })
            .collect(),
    )
}

/// `thirdguard`: where the push holds beside the third base, in tiles from the west start: on the
/// base's enemy side, 8 tiles east of its Depot, where both ways in (the corridor at y 94-98 and
/// the strip at y 108, each meeting the base's ground near x 54) are within a standing Tank's 14
/// tiles. Holding at the corridor's west end, north-west of the Depot, 13 Tanks never saw the 6
/// that shelled it from the south-east at 9:40 (16 tiles off) and lost it in 10 s.
const GUARD_WEST: (f32, f32) = (44.5, 100.5);
/// The guard line runs across the way in, Tanks this far apart, at most this many to a rank; a
/// further rank stands this far behind.
const GUARD_SPACING_TILES: f32 = 1.5;
const GUARD_RANK: usize = 10;
const GUARD_RANK_DEPTH_TILES: f32 = 1.75;
/// A guard Tank this close to its spot is left standing: ordered on every 2 s, the 13 guarding
/// Tanks never stood still long enough to reach their 14-tile range.
const GUARD_SLOT_TILES: f32 = 1.25;

/// Orders the push holding beside the third base into its guard line (`thirdguard`): a line
/// across the way in, each Tank to the spot whose place along the line matches its own, and only
/// the Tanks away from their spots. Its infantry stand just ahead of the line's middle. `false`
/// while not guarding, for the usual march order.
pub(in crate::ai_core::decision) fn guard_orders(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    tanks: &[u32],
    infantry: &[u32],
) -> bool {
    if !memory.jeff_v1_1.south.guarding || super::off("thirdguard") {
        return false;
    }
    let Some(way) = memory.profile_id().and_then(|profile| way(profile, observation)) else {
        return false;
    };
    let ts = observation.map.tile_size as f32;
    let center = way[GUARD_LEG];
    // The enemy side: east of the west third, west of the east one.
    let ahead = if center.0 < observation.map.width as f32 * ts / 2.0 { 1.0 } else { -1.0 };
    let mut units: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id) && unit.hp > 0)
        .collect();
    units.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.id.cmp(&b.id)));
    let n = units.len();
    let ranks = n.div_ceil(GUARD_RANK).max(1);
    let per_rank = n.div_ceil(ranks).max(1);
    // Every spot, ordered along the line; a rear rank's spots sit between the front's.
    let mut spots: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let rank = i % ranks;
            let place = i / ranks;
            let along = (place as f32 - (per_rank as f32 - 1.0) / 2.0) * GUARD_SPACING_TILES
                + rank as f32 * GUARD_SPACING_TILES / ranks as f32;
            (
                center.0 - ahead * rank as f32 * GUARD_RANK_DEPTH_TILES * ts,
                center.1 + along * ts,
            )
        })
        .collect();
    spots.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (unit, spot) in units.iter().zip(spots) {
        if dist2(unit.x, unit.y, spot.0, spot.1) > squared(GUARD_SLOT_TILES * ts) {
            actions::attack_move_units(actions, [unit.id], spot.0, spot.1);
        }
    }
    let front = (center.0 + ahead * 2.0 * ts, center.1);
    let away: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| infantry.contains(&unit.id) && unit.hp > 0)
        .filter(|unit| dist2(unit.x, unit.y, front.0, front.1) > squared(2.0 * ts))
        .map(|unit| unit.id)
        .collect();
    actions::attack_move_units(actions, away, front.0, front.1);
    true
}

/// `noschoneraid`: no Tank raid where the push takes the southern way: the raid goes the direct
/// way, and its three Tanks stuck in the V at 10:00 for the rest of the game while the third base
/// fell.
pub(in crate::ai_core::decision) fn skips_raid(profile_id: &str, observation: &AiObservation) -> bool {
    !super::off("noschoneraid") && way(profile_id, observation).is_some()
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
        memory.jeff_v1_1.south.returning = false;
    }
}

/// After falling back the push holds beside the third base at least this long before it goes on.
const RETURN_HOLD_TICKS: u32 = config::TICK_HZ * 30;

/// The launched push outweighed out on the way (`southhold`): it stays together and walks the
/// way back to the third base, where it holds until strong again, instead of breaking up to
/// re-form at home. Re-forming, the Tanks nearest home were picked and gathered on their midpoint,
/// out on the central plateau between the corridor and the main, where 18 stood from 10:46 to the
/// end. Holding beside the third base, it stays there. `false` off the way, for the usual fall-back.
pub(in crate::ai_core::decision) fn hold_way(observation: &AiObservation, memory: &mut AiDecisionMemory) -> bool {
    let Some(profile) = memory.profile_id() else {
        return false;
    };
    if super::off("southhold") || way(profile, observation).is_none() {
        return false;
    }
    let south = &mut memory.jeff_v1_1.south;
    if south.leg > GUARD_LEG && !south.returning {
        south.returning = true;
    }
    south.hold_until = Some(observation.tick + RETURN_HOLD_TICKS);
    true
}

/// Whether the push is walking the way back to the third base: it moves rather than attack-moves.
pub(in crate::ai_core::decision) fn returning(memory: &AiDecisionMemory) -> bool {
    memory.jeff_v1_1.south.returning
}

/// The wall across each main's south exit, in tiles: the third base and the corridor lie south of it.
const WALL_Y_TILES: f32 = 87.0;
/// The waypoint beside Jeff's third base, south of its road gap, where the push holds until strong:
/// against Pressure AI the third base fell at 10:00 in each of three games to 12-15 fighters coming
/// along the corridor, the push away down it or still squeezing through the gap.
const GUARD_LEG: usize = 3;
/// The push goes on past the third base only with this many Tanks, and this many times the most
/// enemy Tanks remembered at once lately.
const MARCH_TANKS: usize = 16;
const MARCH_LEAD: f32 = 1.3;
/// A waypoint is reached once this share of the Tanks are within `ARRIVED_TILES` and 2 of it, not
/// only once their centre is: stragglers in the road gap held the push at the first waypoint past
/// it from 8:43 to 10:11.
const ARRIVED_SHARE: f32 = 0.6;

/// Where the launched push makes for while on the way: the next waypoint (holding beside the
/// third base until strong), or, striking a group, `None` to go straight at one south of the wall
/// or the waypoint behind it for one on the far side of it; `None` once past the way (or off it),
/// for its usual targeting.
pub(in crate::ai_core::decision) fn push_waypoint(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    center: (f32, f32),
    tanks: &[(f32, f32)],
) -> Option<(f32, f32)> {
    let profile = memory.profile_id()?;
    let way = way(profile, observation)?;
    let ts = observation.map.tile_size as f32;
    memory.jeff_v1_1.south.guarding = false;
    // `marchstrike`: up to the third base the push still strikes groups at Jeff's bases; its
    // usual targeting, which looks for them, does not run while it marches the way, and 13
    // guarding Tanks let 6 enemy Tanks shell the third's Depot down 16 tiles off. Past it, strong
    // and on its way, it leaves them to home defense.
    if !super::off("marchstrike") {
        let strikes = memory.jeff_v1_1.south.leg <= GUARD_LEG;
        super::push::note_march_strike(observation, memory, strikes);
    }
    if memory.jeff_v1_1.push.striking() {
        let south_of_wall = |at: (f32, f32)| at.1 / ts > WALL_Y_TILES;
        let strike = memory.jeff_v1_1.push.strike_point();
        // A group at the third base, south of the wall, is struck straight: no V lies between.
        if strike.is_some_and(south_of_wall) || !south_of_wall(center) {
            memory.jeff_v1_1.south.last_step = format!("t{} strike direct", observation.tick);
            return None;
        }
        return back_waypoint(observation, memory, center);
    }
    let enemy_tanks = super::push::peak_enemy_tanks(memory).max(memory.recent_enemy_tanks());
    let strong = tanks.len() >= MARCH_TANKS
        && tanks.len() as f32 >= enemy_tanks as f32 * MARCH_LEAD
        && memory.jeff_v1_1.south.hold_until.is_none_or(|until| observation.tick >= until);
    let south = &mut memory.jeff_v1_1.south;
    // `southhold`: outweighed out on the way, the push walks it back to the third base.
    while south.returning {
        if south.leg <= GUARD_LEG {
            south.returning = false;
            break;
        }
        let at = way[south.leg - 1];
        let near = tanks
            .iter()
            .filter(|tank| dist2(tank.0, tank.1, at.0, at.1) <= squared((ARRIVED_TILES + 2.0) * ts))
            .count();
        let reached = dist2(center.0, center.1, at.0, at.1) <= squared(ARRIVED_TILES * ts)
            || near as f32 >= tanks.len() as f32 * ARRIVED_SHARE;
        if !reached {
            south.last_step = format!("t{} back to {} from ({:.0},{:.0})", observation.tick, south.leg - 1, center.0 / ts, center.1 / ts);
            return Some(at);
        }
        south.leg -= 1;
    }
    while south.leg < way.len() {
        let at = way[south.leg];
        let near = tanks
            .iter()
            .filter(|tank| dist2(tank.0, tank.1, at.0, at.1) <= squared((ARRIVED_TILES + 2.0) * ts))
            .count();
        let reached = dist2(center.0, center.1, at.0, at.1) <= squared(ARRIVED_TILES * ts)
            || near as f32 >= tanks.len() as f32 * ARRIVED_SHARE;
        if !reached {
            south.last_step = format!("t{} to {} from ({:.0},{:.0})", observation.tick, south.leg, center.0 / ts, center.1 / ts);
            return Some(at);
        }
        if south.leg == GUARD_LEG && !strong {
            south.last_step = format!("t{} guarding at {} ({} Tanks, enemy {})", observation.tick, south.leg, tanks.len(), enemy_tanks);
            south.guarding = true;
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
        "v1_1_south leg={} convoy_leg={} ret={} guard={} step=[{}]",
        memory.jeff_v1_1.south.leg,
        memory.jeff_v1_1.south.convoy_leg,
        memory.jeff_v1_1.south.returning,
        memory.jeff_v1_1.south.guarding,
        memory.jeff_v1_1.south.last_step
    )
}
