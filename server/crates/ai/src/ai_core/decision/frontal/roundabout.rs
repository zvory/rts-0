//! Roundabout attacks. A push that fails (falls back after losing half its Tanks, or is outnumbered
//! in the field) is followed by one that comes at the same target from another side: straight in,
//! then a side picked at random, then the opposite side, then straight in again, and round again.
//! AI 2.1 points its whole defence down the direct line between the bases (its staging line, its
//! Machine Gunners 21-27 tiles out, its waves), so a push from the side meets less of it.
//!
//! A side push first swings out to a point off the direct approach, then closes in on the target
//! from that side and holds at the usual standoff. A side with no open ground there, or with no way
//! round that keeps clear of the target and the enemy main, goes straight in instead.

use super::*;
use crate::ai_core::decision::geometry::squared;

/// Angles off the direct approach tried for a side, widest first.
const SIDE_ANGLES_DEGREES: [f32; 3] = [75.0, 60.0, 45.0];
/// The swing point sits this much farther from the target than the attack point.
const SWING_EXTRA_TILES: f32 = 10.0;
/// The push lines up facing the target from this much farther out along its side.
const APPROACH_EXTRA_TILES: f32 = 10.0;
/// A side point may move this far to find open ground.
const POINT_SNAP_TILES: i32 = 3;
/// The way out to the swing point may not pass this close to the target or the enemy main.
const ROUTE_CLEARANCE_OF_TARGET_TILES: f32 = 12.0;
const ROUTE_CLEARANCE_OF_ENEMY_MAIN_TILES: f32 = 18.0;
/// A side is only worth the walk if its swing point is this far off the direct approach, counting
/// the part of the direct approach within `DIRECT_APPROACH_TILES` of the target.
const MIN_OFFSET_FROM_DIRECT_TILES: f32 = 10.0;
const DIRECT_APPROACH_TILES: f32 = 35.0;
/// The way round may be at most this many times as long as the direct approach.
const MAX_DETOUR_RATIO: f32 = 2.0;
/// The push has reached its swing point within this distance, or closes in from wherever it got to
/// once it has been heading there this long.
const SWING_REACHED_TILES: f32 = 3.5;
const SWING_TIMEOUT_TICKS: u32 = config::TICK_HZ * 180;
/// A target that moves no farther than this is the same target: the natural's nearest steel patch
/// shifts a tile or two each time one runs dry or is destroyed, and the push keeps its progress.
const SAME_TARGET_TILES: f32 = 8.0;
/// A push already within this much of the swing distance of the target, and within
/// `ON_SIDE_DEGREES` of its lane's bearing, is on the lane: it closes in without swinging out
/// again. After the natural falls, the push on its side goes on to the main from that side.
const ON_SIDE_EXTRA_TILES: f32 = 8.0;
const ON_SIDE_DEGREES: f32 = 35.0;

type WorldPoint = (i32, i32);

fn stored(point: (f32, f32)) -> WorldPoint {
    (point.0.round() as i32, point.1.round() as i32)
}

fn world(point: WorldPoint) -> (f32, f32) {
    (point.0 as f32, point.1 as f32)
}

/// Which way a push comes at its target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Approach {
    #[default]
    Direct,
    Left,
    Right,
}

impl Approach {
    fn opposite(self) -> Self {
        match self {
            Self::Direct => Self::Direct,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// Turn off the direct approach, seen from the target: positive for left.
    fn sign(self) -> f32 {
        match self {
            Self::Direct => 0.0,
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

/// A side push's way in: out to the swing point, then in to the attack point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FlankLane {
    swing: WorldPoint,
    attack: WorldPoint,
    /// A point farther out on the same side, so the push lines up across its own approach.
    approach_from: WorldPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CachedLane {
    approach: Approach,
    objective: WorldPoint,
    lane: Option<FlankLane>,
}

/// Where the push is in the center, side, opposite side loop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Roundabout {
    failed_pushes: u32,
    /// The side this loop tried first, picked at random when the loop's first side push was due.
    first_side: Approach,
    lane: Option<CachedLane>,
    swing_reached: bool,
    swing_started_tick: Option<u32>,
}

impl Roundabout {
    /// The approach the next (or current) push takes.
    pub(crate) fn approach(&self) -> Approach {
        match self.failed_pushes % 3 {
            0 => Approach::Direct,
            1 => self.first_side,
            _ => self.first_side.opposite(),
        }
    }

    /// A push failed: the next one takes the next approach in the loop.
    pub(super) fn note_failed_push(&mut self, player_id: u32, tick: u32) {
        self.failed_pushes = self.failed_pushes.saturating_add(1);
        if self.failed_pushes % 3 == 1 {
            self.first_side = random_side(player_id, tick, self.failed_pushes);
        }
        self.lane = None;
        self.start_push();
    }

    /// A push sets out: a side push swings out again before it closes in.
    pub(super) fn start_push(&mut self) {
        self.swing_reached = false;
        self.swing_started_tick = None;
    }
}

/// Left or right, from the game's own facts: a replay or arena rerun makes the same choice, but
/// different games and different failures do not all go the same way.
fn random_side(player_id: u32, tick: u32, failed_pushes: u32) -> Approach {
    // SplitMix64 finaliser.
    let mut z = (u64::from(player_id) << 40) ^ (u64::from(failed_pushes) << 32) ^ u64::from(tick);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    if z & 1 == 0 {
        Approach::Left
    } else {
        Approach::Right
    }
}

fn rotate(vector: (f32, f32), degrees: f32) -> (f32, f32) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (
        vector.0 * cos - vector.1 * sin,
        vector.0 * sin + vector.1 * cos,
    )
}

fn route_length(from: (f32, f32), route: &[(f32, f32)]) -> f32 {
    let mut previous = from;
    let mut length = 0.0;
    for point in route {
        length += dist2(previous.0, previous.1, point.0, point.1).sqrt();
        previous = *point;
    }
    length
}

/// The lane on side `approach` of `objective` for a push setting out from `from`, whose direct
/// approach ends at `direct_point`. The widest workable angle wins; `None` if no angle works.
#[allow(clippy::too_many_arguments)]
pub(super) fn flank_lane(
    analysis: &AiMapAnalysis,
    observation: &AiObservation,
    from: (f32, f32),
    enemy_main: (f32, f32),
    objective: (f32, f32),
    direct_point: (f32, f32),
    standoff_tiles: f32,
    approach: Approach,
) -> Option<FlankLane> {
    if approach == Approach::Direct {
        return None;
    }
    let ts = observation.map.tile_size as f32;
    let direct = analysis.compact_group_route(from, direct_point, 1);
    // The route search falls back to the bare destination when there is no path.
    if direct.len() < 2 {
        return None;
    }
    let direct_length = route_length(from, &direct);
    // Which way the direct approach comes in: from its last point well outside the attack point.
    let swing_tiles = standoff_tiles + SWING_EXTRA_TILES;
    let arrival = direct
        .iter()
        .rev()
        .find(|point| {
            dist2(point.0, point.1, objective.0, objective.1) >= squared(swing_tiles * ts)
        })
        .copied()
        .unwrap_or(from);
    let back = normalized_direction(objective, arrival)?;
    let near_direct: Vec<(f32, f32)> = direct
        .iter()
        .copied()
        .filter(|point| {
            dist2(point.0, point.1, objective.0, objective.1) <= squared(DIRECT_APPROACH_TILES * ts)
        })
        .collect();
    let target_clearance2 = squared(ROUTE_CLEARANCE_OF_TARGET_TILES * ts);
    let main_clearance2 = squared(ROUTE_CLEARANCE_OF_ENEMY_MAIN_TILES * ts);
    // Once the natural is gone the target is the main itself; its own clearance covers it.
    let objective_is_main =
        dist2(objective.0, objective.1, enemy_main.0, enemy_main.1) < main_clearance2;
    SIDE_ANGLES_DEGREES.iter().find_map(|angle| {
        let dir = rotate(back, approach.sign() * angle);
        let at = |tiles: f32| {
            clamp_to_map(
                (
                    objective.0 + dir.0 * tiles * ts,
                    objective.1 + dir.1 * tiles * ts,
                ),
                observation.map,
            )
        };
        let attack = analysis.open_ground_near(from, at(standoff_tiles), 2, POINT_SNAP_TILES)?;
        let swing = analysis.open_ground_near(from, at(swing_tiles), 2, POINT_SNAP_TILES)?;
        let offset2 = squared(MIN_OFFSET_FROM_DIRECT_TILES * ts);
        if near_direct
            .iter()
            .any(|point| dist2(point.0, point.1, swing.0, swing.1) < offset2)
        {
            return None;
        }
        let out = analysis.compact_group_route(from, swing, 1);
        if out.len() < 2
            || out.iter().any(|point| {
                dist2(point.0, point.1, objective.0, objective.1) < target_clearance2
                    || (!objective_is_main
                        && dist2(point.0, point.1, enemy_main.0, enemy_main.1) < main_clearance2)
            })
        {
            return None;
        }
        let closing = analysis.compact_group_route(swing, attack, 1);
        if closing.len() < 2 {
            return None;
        }
        let total = route_length(from, &out) + route_length(swing, &closing);
        if total > MAX_DETOUR_RATIO * direct_length {
            return None;
        }
        Some(FlankLane {
            swing: stored(swing),
            attack: stored(attack),
            approach_from: stored(at(swing_tiles + APPROACH_EXTRA_TILES)),
        })
    })
}

/// The side lane the current push takes, worked out once per approach and target, or `None` to go
/// straight in.
#[allow(clippy::too_many_arguments)]
pub(super) fn current_lane(
    memory: &mut AiDecisionMemory,
    analysis: Option<&AiMapAnalysis>,
    observation: &AiObservation,
    from: (f32, f32),
    enemy_main: (f32, f32),
    objective: (f32, f32),
    direct_point: (f32, f32),
    standoff_tiles: f32,
) -> Option<FlankLane> {
    let approach = memory.roundabout.approach();
    if approach == Approach::Direct {
        return None;
    }
    let key = stored(objective);
    if let Some(cached) = memory
        .roundabout
        .lane
        .filter(|cached| cached.approach == approach && cached.objective == key)
    {
        return cached.lane;
    }
    // The target moved well away (the natural fell): swing out again for the new one, unless the
    // push is already on that side. A steel patch running dry at the same natural is not a new target.
    let same_target2 = squared(SAME_TARGET_TILES * observation.map.tile_size as f32);
    if memory.roundabout.lane.is_some_and(|cached| {
        let previous = world(cached.objective);
        dist2(previous.0, previous.1, objective.0, objective.1) > same_target2
    }) {
        memory.roundabout.start_push();
    }
    let lane = analysis.and_then(|analysis| {
        flank_lane(
            analysis,
            observation,
            from,
            enemy_main,
            objective,
            direct_point,
            standoff_tiles,
            approach,
        )
    });
    memory.roundabout.lane = Some(CachedLane {
        approach,
        objective: key,
        lane,
    });
    lane
}

/// Where a side push heads this decision and which way it faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LaneOrders {
    pub(super) destination: (f32, f32),
    pub(super) face_from: (f32, f32),
    pub(super) face_to: (f32, f32),
}

/// Out to the swing point, facing the way it walks; then, once there (or after trying for long
/// enough), in to the attack point, facing the target from the side.
pub(super) fn lane_orders(
    memory: &mut AiDecisionMemory,
    observation: &AiObservation,
    lane: FlankLane,
    tanks_center: Option<(f32, f32)>,
    own_base: (f32, f32),
    objective: (f32, f32),
) -> LaneOrders {
    let state = &mut memory.roundabout;
    let swing = world(lane.swing);
    if !state.swing_reached {
        let started = *state.swing_started_tick.get_or_insert(observation.tick);
        let reach2 = squared(SWING_REACHED_TILES * observation.map.tile_size as f32);
        let arrived = tanks_center.is_some_and(|center| {
            dist2(center.0, center.1, swing.0, swing.1) <= reach2
                || on_lane_side(center, objective, lane, observation.map.tile_size as f32)
        });
        if arrived || observation.tick.saturating_sub(started) >= SWING_TIMEOUT_TICKS {
            state.swing_reached = true;
        }
    }
    if state.swing_reached {
        LaneOrders {
            destination: world(lane.attack),
            face_from: world(lane.approach_from),
            face_to: objective,
        }
    } else {
        LaneOrders {
            destination: swing,
            face_from: own_base,
            face_to: swing,
        }
    }
}

/// Whether a push at `center` is already on the lane's side of `objective`: no farther out than the
/// swing point (plus a margin) and close to the bearing of the lane's attack point.
fn on_lane_side(center: (f32, f32), objective: (f32, f32), lane: FlankLane, ts: f32) -> bool {
    let (swing, attack) = (world(lane.swing), world(lane.attack));
    let reach = dist2(swing.0, swing.1, objective.0, objective.1).sqrt() + ON_SIDE_EXTRA_TILES * ts;
    if dist2(center.0, center.1, objective.0, objective.1) > squared(reach) {
        return false;
    }
    let (Some(to_center), Some(to_attack)) = (
        normalized_direction(objective, center),
        normalized_direction(objective, attack),
    ) else {
        return false;
    };
    let cos = to_center.0 * to_attack.0 + to_center.1 * to_attack.1;
    cos >= ON_SIDE_DEGREES.to_radians().cos()
}

#[cfg(test)]
#[path = "roundabout_tests.rs"]
mod tests;
