//! Bounded longitudinal escape for a hull that fits at its old angle but cannot turn.
use super::pivot_drive::{angle_delta, distance_between, pivot_drive_intent};
use super::standability::unit_static_standable;
use super::vehicle_route::static_swept_segment_legal;
use super::{EntityStore, Occupancy, SpatialIndex};
use crate::config;
use crate::game::entity::{Entity, EntityKind, PivotRecovery};
use crate::game::map::Map;
use crate::game::services::geometry::{
    unit_bodies_intersect, unit_body_for_entity, unit_body_with_facing, UnitBody,
};
use rts_rules::math::GameplayMath as _;

const MAX_DISTANCE: f32 = config::TILE_SIZE as f32 * 2.0;
const SEARCH_STEP: f32 = 2.0;
const MAX_TICKS: u16 = config::TICK_HZ as u16 * 3;

pub(super) fn begin(
    e: &mut Entity,
    map: &Map,
    occ: &Occupancy,
    pos: (f32, f32),
    blocked_facing: f32,
    speed: f32,
    neighbors: &[UnitBody],
) {
    if !speed.is_finite() || speed <= 0.0 || !e.facing().is_finite() {
        return;
    }
    let facing = e.facing();
    let axis = (facing.det_cos(), facing.det_sin());
    let mut best: Option<((f32, f32), f32)> = None;
    for sign in [1.0_f32, -1.0] {
        let mut distance = speed + SEARCH_STEP;
        while distance <= MAX_DISTANCE {
            let goal = (
                pos.0 + axis.0 * distance * sign,
                pos.1 + axis.1 * distance * sign,
            );
            if !static_swept_segment_legal(map, occ, e.kind, pos, goal, facing)
                || !traffic_clear(e.kind, pos, goal, facing, neighbors)
            {
                break;
            }
            let desired = pivot_drive_intent(map, occ, e, goal.0, goal.1)
                .map_or(blocked_facing, |intent| intent.desired_facing);
            if unit_static_standable(occ, map, e.kind, goal.0, goal.1, blocked_facing)
                && turn_is_clear(e, map, occ, goal, facing, desired)
            {
                // Prefer the shortest complete escape, using route distance only to break ties.
                let score = distance
                    + e.next_waypoint()
                        .map_or(0.0, |wp| distance_between(goal, wp) * 0.01);
                if best.is_none_or(|(_, old)| score < old) {
                    best = Some((goal, score));
                }
                break;
            }
            distance += SEARCH_STEP;
        }
    }
    if let Some((goal, _)) = best {
        if let Some(m) = e.movement.as_mut() {
            m.pivot_drive = None;
            m.pivot_recovery = Some(PivotRecovery {
                goal,
                facing,
                ticks_left: MAX_TICKS,
            });
        }
    }
}

/// True means this tick belongs to the committed escape, including its terminal tick.
pub(super) fn advance(
    e: &mut Entity,
    map: &Map,
    occ: &Occupancy,
    speed: f32,
    neighbors: &[UnitBody],
) -> bool {
    let Some(mut recovery) = e.movement.as_ref().and_then(|m| m.pivot_recovery) else {
        return false;
    };
    let pos = (e.pos_x(), e.pos_y());
    let distance = distance_between(pos, recovery.goal);
    // Collision separation can move the hull off the committed corridor. Replan
    // from that pose rather than steering sideways back toward the old goal.
    let lateral = (recovery.goal.0 - pos.0) * -recovery.facing.det_sin()
        + (recovery.goal.1 - pos.1) * recovery.facing.det_cos();
    if recovery.ticks_left == 0
        || !distance.is_finite()
        || !lateral.is_finite()
        || lateral.abs() > 0.01
        || !speed.is_finite()
        || speed <= 0.0
        || angle_delta(e.facing(), recovery.facing).abs() > 0.001
        || distance > MAX_DISTANCE + 1.0
    {
        if let Some(m) = e.movement.as_mut() {
            m.pivot_recovery = None;
        }
        return false;
    }
    let step = speed.min(distance);
    let next = if distance > 0.001 {
        (
            pos.0 + (recovery.goal.0 - pos.0) * step / distance,
            pos.1 + (recovery.goal.1 - pos.1) * step / distance,
        )
    } else {
        pos
    };
    if !static_swept_segment_legal(map, occ, e.kind, pos, next, recovery.facing)
        || !traffic_clear(e.kind, pos, next, recovery.facing, neighbors)
    {
        if let Some(m) = e.movement.as_mut() {
            m.pivot_recovery = None;
        }
        return false;
    }
    e.set_position(next.0, next.1);
    let next = (e.pos_x(), e.pos_y());
    e.set_movement_delta(next.0 - pos.0, next.1 - pos.1);
    if e.kind == EntityKind::Tank && distance_between(pos, next) > 0.01 {
        if let Some(c) = e.combat.as_mut() {
            c.tank_stationary_range_ticks = 0;
            c.tank_stationary_range_reset_this_tick = true;
        }
    }
    recovery.ticks_left = recovery.ticks_left.saturating_sub(1);
    if let Some(m) = e.movement.as_mut() {
        m.stuck_ticks = 0;
        m.static_blocked_ticks = 0;
        m.last_progress_pos = next;
        m.pivot_recovery = if distance_between(next, recovery.goal) <= 0.01 {
            None
        } else {
            Some(recovery)
        };
    }
    true
}

fn turn_is_clear(
    e: &Entity,
    map: &Map,
    occ: &Occupancy,
    pos: (f32, f32),
    from: f32,
    to: f32,
) -> bool {
    let delta = angle_delta(from, to);
    if !delta.is_finite() {
        return false;
    }
    let steps = (delta.abs() / 0.05).ceil().max(1.0) as u32;
    (0..=steps).all(|i| {
        unit_static_standable(
            occ,
            map,
            e.kind,
            pos.0,
            pos.1,
            from + delta * i as f32 / steps as f32,
        )
    })
}

pub(super) fn neighbors(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    e: &Entity,
) -> Vec<UnitBody> {
    spatial
        .ids_in_circle_bbox(
            e.pos_x(),
            e.pos_y(),
            MAX_DISTANCE + super::MAX_UNIT_BOUNDING_RADIUS_PX * 2.0,
        )
        .filter(|id| *id != e.id)
        .filter_map(|id| entities.get(id))
        .filter(|other| other.hp > 0 && other.is_unit() && other.kind != EntityKind::ScoutPlane)
        .filter_map(unit_body_for_entity)
        .collect()
}

fn traffic_clear(
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
    facing: f32,
    neighbors: &[UnitBody],
) -> bool {
    if neighbors.is_empty() {
        return true;
    }
    let steps = (distance_between(from, to) / 2.0).ceil().max(1.0) as u32;
    (0..=steps).all(|i| {
        let t = i as f32 / steps as f32;
        unit_body_with_facing(
            kind,
            from.0 + (to.0 - from.0) * t,
            from.1 + (to.1 - from.1) * t,
            facing,
        )
        .is_some_and(|body| {
            neighbors
                .iter()
                .all(|other| !unit_bodies_intersect(body, *other))
        })
    })
}
