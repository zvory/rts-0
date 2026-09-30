//! How the current Jeff's push comes at its target.
//!
//! From which side: a push that fails (falls back after losing half its Tanks, or is outnumbered in
//! the field) is followed by one that comes at the same target from another side: straight in, then
//! a side picked at random, then the opposite side, then straight in again, and round again. AI 2.1
//! points its whole defence down the direct line between the bases (its staging line, its Machine
//! Gunners 21-27 tiles out, its waves), so a push from the side meets less of it.
//!
//! In three legs: every push travels, loosely formed, to a staging point short of its target (a side
//! push's swing point off the direct approach, or a point behind the direct attack point), reforms
//! there, then closes in tight to the attack point and holds at the usual standoff.
//!
//! A side needs open ground away from the map edge, a way out that keeps clear of the target and
//! the enemy main, a swing point well off the direct approach, and a detour of at most half again
//! the direct approach. Otherwise, or once a side push stops making progress, it goes straight in.

use super::*;
use crate::ai_core::decision::geometry::squared;

/// Pushes with fewer Tanks keep the old march: straight to the attack point in the tight formation.
pub(super) const MIN_TANKS_FOR_LEGS: usize = 4;
/// Angles off the direct approach tried for a side, widest first.
const SIDE_ANGLES_DEGREES: [f32; 3] = [75.0, 60.0, 45.0];
/// A side push's swing point sits this much farther from the target than its attack point.
const SWING_EXTRA_TILES: f32 = 10.0;
/// A direct push reforms this far behind its attack point.
const DIRECT_STAGING_EXTRA_TILES: f32 = 8.0;
/// A side push lines up facing the target from this much farther out along its side.
const APPROACH_EXTRA_TILES: f32 = 10.0;
/// A side point may move this far to find open ground.
const POINT_SNAP_TILES: i32 = 3;
/// A swing point this close to the map edge leaves the push crawling along the rim.
const EDGE_MARGIN_TILES: f32 = 6.0;
/// The way out to the swing point may not pass this close to the target or the enemy main.
const ROUTE_CLEARANCE_OF_TARGET_TILES: f32 = 12.0;
const ROUTE_CLEARANCE_OF_ENEMY_MAIN_TILES: f32 = 18.0;
/// A side is only worth the walk if its swing point is this far off the direct approach, counting
/// the part of the direct approach within `DIRECT_APPROACH_TILES` of the target.
const MIN_OFFSET_FROM_DIRECT_TILES: f32 = 10.0;
const DIRECT_APPROACH_TILES: f32 = 35.0;
/// The way round may be at most this many times as long as the direct approach.
const MAX_DETOUR_RATIO: f32 = 1.5;
/// The push has reached its staging point within this distance.
const STAGING_REACHED_TILES: f32 = 4.0;
/// A push already inside the staging distance and within this angle of the staging point's bearing
/// from the target has got there some other way (fighting on the way in): it closes in from there.
const PAST_STAGING_DEGREES: f32 = 45.0;
/// The push closes in once reformed at the staging point, or after waiting this long.
const REFORM_TIMEOUT_TICKS: u32 = config::TICK_HZ * 20;
/// Travelling without getting this much closer to the staging point for `NO_PROGRESS_TICKS`, not
/// counting time spent fighting, is being stuck: a side push goes straight in instead, a direct one
/// closes in from where it is.
const PROGRESS_TILES: f32 = 2.0;
const NO_PROGRESS_TICKS: u32 = config::TICK_HZ * 60;
/// A staging point that moves no farther than this belongs to the same target: the natural's
/// nearest steel patch shifts a tile or two each time one runs dry or is destroyed.
const SAME_TARGET_TILES: f32 = 8.0;

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

/// The push's leg: travelling to the staging point, reforming there, or closing in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PushPhase {
    #[default]
    Travel,
    Reform,
    Final,
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

/// Where the pushes are in the center, side, opposite side loop, and the current push's leg.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PushApproach {
    failed_pushes: u32,
    /// The side this loop tried first, picked at random when the loop's first side push was due.
    first_side: Approach,
    lane: Option<CachedLane>,
    /// The current push gave up its side lane and goes straight in.
    lane_abandoned: bool,
    phase: PushPhase,
    phase_since: Option<u32>,
    /// The staging point the current leg is measured against.
    staging: Option<WorldPoint>,
    /// The push's closest approach to the staging point so far, and when it last got closer.
    closest_to_staging: Option<i32>,
    last_progress_tick: Option<u32>,
}

impl PushApproach {
    /// The approach the next (or current) push takes.
    pub(crate) fn approach(&self) -> Approach {
        match self.failed_pushes % 3 {
            0 => Approach::Direct,
            1 => self.first_side,
            _ => self.first_side.opposite(),
        }
    }

    pub(super) fn phase(&self) -> PushPhase {
        self.phase
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

    /// A push sets out: it travels to its staging point again, and may take its side lane.
    pub(super) fn start_push(&mut self) {
        self.lane_abandoned = false;
        self.restart_legs();
    }

    fn restart_legs(&mut self) {
        self.phase = PushPhase::Travel;
        self.phase_since = None;
        self.staging = None;
        self.closest_to_staging = None;
        self.last_progress_tick = None;
    }

    fn enter(&mut self, phase: PushPhase, tick: u32) {
        self.phase = phase;
        self.phase_since = Some(tick);
    }

    /// Whether the push is formed up at its staging point: once it is, or has waited long enough,
    /// it closes in.
    pub(super) fn note_reform(&mut self, tick: u32, reformed: bool) {
        if self.phase != PushPhase::Reform {
            return;
        }
        let waited = tick.saturating_sub(self.phase_since.unwrap_or(tick));
        if reformed || waited >= REFORM_TIMEOUT_TICKS {
            self.enter(PushPhase::Final, tick);
        }
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

fn away_from_edge(point: (f32, f32), map: AiMapSummary) -> bool {
    let ts = map.tile_size as f32;
    let margin = EDGE_MARGIN_TILES * ts;
    point.0 >= margin
        && point.1 >= margin
        && point.0 <= map.width as f32 * ts - margin
        && point.1 <= map.height as f32 * ts - margin
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
        if !away_from_edge(swing, observation.map) {
            return None;
        }
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
fn current_lane(
    memory: &mut AiDecisionMemory,
    analysis: Option<&AiMapAnalysis>,
    observation: &AiObservation,
    from: (f32, f32),
    enemy_main: (f32, f32),
    objective: (f32, f32),
    direct_point: (f32, f32),
    standoff_tiles: f32,
) -> Option<FlankLane> {
    let approach = memory.approach.approach();
    if approach == Approach::Direct || memory.approach.lane_abandoned {
        return None;
    }
    let key = stored(objective);
    if let Some(cached) = memory
        .approach
        .lane
        .filter(|cached| cached.approach == approach && cached.objective == key)
    {
        return cached.lane;
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
    memory.approach.lane = Some(CachedLane {
        approach,
        objective: key,
        lane,
    });
    lane
}

/// The current push's staging and attack points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PushLegs {
    staging: (f32, f32),
    attack: (f32, f32),
    /// The push faces the target from here while it reforms and closes in.
    face_from: (f32, f32),
    side: bool,
}

/// The legs of the current push on `objective`: a side lane when this push takes one, otherwise
/// straight in to `direct_point`, reforming `DIRECT_STAGING_EXTRA_TILES` behind it.
#[allow(clippy::too_many_arguments)]
pub(super) fn push_legs(
    memory: &mut AiDecisionMemory,
    analysis: Option<&AiMapAnalysis>,
    observation: &AiObservation,
    from: (f32, f32),
    own_base: (f32, f32),
    enemy_main: (f32, f32),
    objective: (f32, f32),
    direct_point: (f32, f32),
    standoff_tiles: f32,
) -> PushLegs {
    if let Some(lane) = current_lane(
        memory,
        analysis,
        observation,
        from,
        enemy_main,
        objective,
        direct_point,
        standoff_tiles,
    ) {
        return PushLegs {
            staging: world(lane.swing),
            attack: world(lane.attack),
            face_from: world(lane.approach_from),
            side: true,
        };
    }
    let ts = observation.map.tile_size as f32;
    let staging = normalized_direction(objective, direct_point)
        .and_then(|back| {
            let behind = clamp_to_map(
                (
                    direct_point.0 + back.0 * DIRECT_STAGING_EXTRA_TILES * ts,
                    direct_point.1 + back.1 * DIRECT_STAGING_EXTRA_TILES * ts,
                ),
                observation.map,
            );
            analysis.and_then(|analysis| {
                analysis.open_ground_near(own_base, behind, 2, POINT_SNAP_TILES)
            })
        })
        .unwrap_or(direct_point);
    PushLegs {
        staging,
        attack: direct_point,
        face_from: own_base,
        side: false,
    }
}

/// Where the push heads this decision, which way it faces, and in which leg.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LegOrders {
    pub(super) destination: (f32, f32),
    pub(super) face_from: (f32, f32),
    pub(super) face_to: (f32, f32),
    pub(super) phase: PushPhase,
}

/// Advance the current push through its legs and give this decision's destination: the staging
/// point while travelling and reforming, then the attack point.
pub(super) fn leg_orders(
    memory: &mut AiDecisionMemory,
    observation: &AiObservation,
    legs: PushLegs,
    tanks_center: Option<(f32, f32)>,
    contact_active: bool,
    own_base: (f32, f32),
    objective: (f32, f32),
) -> LegOrders {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let state = &mut memory.approach;
    // A staging point that moved well away belongs to a new target (the natural fell): travel again.
    if state.staging.is_some_and(|previous| {
        let previous = world(previous);
        dist2(previous.0, previous.1, legs.staging.0, legs.staging.1)
            > squared(SAME_TARGET_TILES * ts)
    }) {
        state.restart_legs();
    }
    state.staging = Some(stored(legs.staging));
    if state.phase_since.is_none() {
        state.phase_since = Some(tick);
    }
    if state.phase == PushPhase::Travel {
        if let Some(center) = tanks_center {
            let distance = dist2(center.0, center.1, legs.staging.0, legs.staging.1).sqrt();
            let closer = state
                .closest_to_staging
                .is_none_or(|best| distance < best as f32 - PROGRESS_TILES * ts);
            if closer {
                state.closest_to_staging = Some(distance.round() as i32);
            }
            if closer || contact_active || state.last_progress_tick.is_none() {
                state.last_progress_tick = Some(tick);
            }
            let stuck =
                tick.saturating_sub(state.last_progress_tick.unwrap_or(tick)) >= NO_PROGRESS_TICKS;
            if distance <= STAGING_REACHED_TILES * ts {
                state.enter(PushPhase::Reform, tick);
            } else if past_staging(center, objective, legs.staging, ts) {
                state.enter(PushPhase::Final, tick);
            } else if stuck && legs.side {
                // From the next decision the push heads for the direct staging point.
                state.lane_abandoned = true;
                state.restart_legs();
            } else if stuck {
                state.enter(PushPhase::Final, tick);
            }
        }
    }
    match state.phase {
        PushPhase::Travel => LegOrders {
            destination: legs.staging,
            // Face the way it walks; from the base itself if that is where it stands.
            face_from: if dist2(own_base.0, own_base.1, legs.staging.0, legs.staging.1) > ts * ts {
                own_base
            } else {
                legs.face_from
            },
            face_to: legs.staging,
            phase: PushPhase::Travel,
        },
        PushPhase::Reform => LegOrders {
            destination: legs.staging,
            face_from: legs.face_from,
            face_to: objective,
            phase: PushPhase::Reform,
        },
        PushPhase::Final => LegOrders {
            destination: legs.attack,
            face_from: legs.face_from,
            face_to: objective,
            phase: PushPhase::Final,
        },
    }
}

/// Whether a push at `center` is already inside the staging distance of `objective`, close to the
/// staging point's bearing.
fn past_staging(center: (f32, f32), objective: (f32, f32), staging: (f32, f32), ts: f32) -> bool {
    let staging_distance = dist2(staging.0, staging.1, objective.0, objective.1).sqrt();
    if dist2(center.0, center.1, objective.0, objective.1).sqrt() > staging_distance - ts {
        return false;
    }
    let (Some(to_center), Some(to_staging)) = (
        normalized_direction(objective, center),
        normalized_direction(objective, staging),
    ) else {
        return false;
    };
    to_center.0 * to_staging.0 + to_center.1 * to_staging.1
        >= PAST_STAGING_DEGREES.to_radians().cos()
}

#[cfg(test)]
#[path = "approach_tests.rs"]
mod tests;
