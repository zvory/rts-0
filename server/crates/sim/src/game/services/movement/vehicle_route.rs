use crate::config;
use crate::game::entity::{
    uses_oriented_vehicle_body, uses_pivot_vehicle_movement, Entity, EntityKind, RoutePolicy,
};
use crate::game::map::Map;
use crate::game::services::geometry::{
    tile_rect, unit_body_intersects_rect, unit_body_with_facing, CircleBody, OrientedBoxBody,
    OrientedCapsuleBody, UnitBody,
};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::standability as static_standability;

use super::pivot_drive::distance_between;
use super::vehicle_profiles::{
    car_motion_profile, pivot_drive_profile, CarMotionProfile, SCOUT_CAR_ROUTE_LOOKAHEAD_PX,
};
use super::ARRIVE_EPS;

#[derive(Clone, Copy)]
pub(super) struct VehicleRouteContext {
    pub(super) next_index: usize,
    pub(super) pre_pop_count: usize,
    pub(super) target: (f32, f32),
    pub(super) lookahead: (f32, f32),
    pub(super) route_dir: (f32, f32),
    pub(super) final_goal: (f32, f32),
    pub(super) direct_goal_is_clear: bool,
}

pub(super) fn route_accepts_waypoint(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    current: (f32, f32),
    waypoint: (f32, f32),
    next_waypoint: Option<(f32, f32)>,
) -> bool {
    if distance_between(current, waypoint) <= config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX {
        if uses_pivot_vehicle_movement(e.kind)
            || (uses_oriented_vehicle_body(e.kind)
                && e.path_policy() == RoutePolicy::FastestTerrainTime)
        {
            return next_waypoint.is_some_and(|next_waypoint| {
                route_segment_standable_from_current_hull(map, occ, e, current, next_waypoint)
            });
        }
        return true;
    }

    let facing = e.facing();
    if uses_oriented_vehicle_body(e.kind) && facing.is_finite() {
        let forward = (facing.cos(), facing.sin());
        let to_waypoint = (waypoint.0 - current.0, waypoint.1 - current.1);
        if forward.0.is_finite()
            && forward.1.is_finite()
            && along_track_error(to_waypoint, forward) < -ARRIVE_EPS
        {
            return false;
        }
    }

    let Some(next_waypoint) = next_waypoint else {
        return false;
    };
    let Some(route_dir) = unit_direction(waypoint, next_waypoint) else {
        return false;
    };
    let from_waypoint_to_current = (current.0 - waypoint.0, current.1 - waypoint.1);
    if along_track_error(from_waypoint_to_current, route_dir) > 0.0 {
        return route_segment_standable_for_route_skip(map, occ, e, current, next_waypoint);
    }

    if e.path_policy() == RoutePolicy::FastestTerrainTime {
        return false;
    }

    route_segment_standable_for_route_skip(map, occ, e, current, next_waypoint)
}

fn route_segment_standable_for_route_skip(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    current: (f32, f32),
    next_waypoint: (f32, f32),
) -> bool {
    if uses_pivot_vehicle_movement(e.kind)
        || (uses_oriented_vehicle_body(e.kind)
            && e.path_policy() == RoutePolicy::FastestTerrainTime)
    {
        return route_segment_standable_from_current_hull(map, occ, e, current, next_waypoint);
    }

    static_standability::unit_static_standable_with_facing(
        map,
        occ,
        e.kind,
        current.0,
        current.1,
        e.facing(),
    ) && static_standability::unit_static_segment_standable(
        map,
        occ,
        e.kind,
        current,
        next_waypoint,
    )
}

fn route_segment_standable_from_current_hull(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    current: (f32, f32),
    next_waypoint: (f32, f32),
) -> bool {
    static_swept_segment_legal(map, occ, e.kind, current, next_waypoint, e.facing())
}

pub(super) fn vehicle_desired_path_point(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    x: f32,
    y: f32,
) -> Option<(f32, f32)> {
    vehicle_route_context(map, occ, e, (x, y)).map(|route| route.lookahead)
}

pub(super) fn vehicle_route_context(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    current: (f32, f32),
) -> Option<VehicleRouteContext> {
    let path = &e.movement.as_ref()?.path;
    let mut next_index = path.len().checked_sub(1)?;

    while next_index > 0 {
        let waypoint = path[next_index];
        let next_waypoint = path[next_index - 1];
        if !route_accepts_waypoint(map, occ, e, current, waypoint, Some(next_waypoint)) {
            break;
        }
        next_index -= 1;
        if e.path_policy() == RoutePolicy::FastestTerrainTime {
            break;
        }
    }

    let pre_pop_count = path.len() - 1 - next_index;
    let target = path[next_index];
    let final_goal = e
        .path_goal()
        .or_else(|| path.first().copied())
        .unwrap_or(target);
    let direct_goal_is_clear = e.path_policy() == RoutePolicy::LegacyShape
        && car_motion_profile(e.kind).is_some_and(|profile| {
            clear_car_direct_goal_route(profile, map, occ, e.kind, current, final_goal)
        });
    let route_focus = if direct_goal_is_clear {
        final_goal
    } else {
        target
    };
    let lookahead = if !direct_goal_is_clear
        && !static_standability::unit_static_segment_standable(map, occ, e.kind, current, target)
    {
        target
    } else {
        point_at_distance(current, route_focus, vehicle_route_lookahead_px(e.kind))
            .unwrap_or(route_focus)
    };
    let route_dir = unit_direction(current, lookahead)
        .or_else(|| unit_direction(current, route_focus))
        .or_else(|| unit_direction(current, final_goal))?;
    Some(VehicleRouteContext {
        next_index,
        pre_pop_count,
        target,
        lookahead,
        route_dir,
        final_goal,
        direct_goal_is_clear,
    })
}

fn vehicle_route_lookahead_px(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::Tank | EntityKind::AntiTankGun => pivot_drive_profile(kind).lookahead_px,
        _ => car_motion_profile(kind)
            .map(|profile| profile.route_lookahead_px)
            .unwrap_or(SCOUT_CAR_ROUTE_LOOKAHEAD_PX),
    }
}

pub(super) fn along_track_error(delta: (f32, f32), segment_dir: (f32, f32)) -> f32 {
    delta.0 * segment_dir.0 + delta.1 * segment_dir.1
}

pub(super) fn lateral_error(delta: (f32, f32), segment_dir: (f32, f32)) -> f32 {
    (delta.0 * segment_dir.1 - delta.1 * segment_dir.0).abs()
}

pub(super) fn unit_direction(from: (f32, f32), to: (f32, f32)) -> Option<(f32, f32)> {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() || len <= 1.0e-4 {
        return None;
    }
    Some((dx / len, dy / len))
}

fn point_at_distance(from: (f32, f32), to: (f32, f32), distance: f32) -> Option<(f32, f32)> {
    if !distance.is_finite() || distance <= 0.0 {
        return None;
    }
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let segment_len = (dx * dx + dy * dy).sqrt();
    if !segment_len.is_finite() || segment_len < distance {
        return None;
    }
    if segment_len <= 1.0e-4 {
        return Some(to);
    }

    let t = distance / segment_len;
    Some((from.0 + dx * t, from.1 + dy * t))
}

pub(super) fn static_swept_segment_legal(
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
    facing: f32,
) -> bool {
    let distance = distance_between(from, to);
    if !distance.is_finite() {
        return false;
    }
    let sweep_sample_step_px = car_motion_profile(kind)
        .map(|profile| profile.sweep_sample_step_px)
        .unwrap_or(config::TILE_SIZE as f32 * 0.125);
    let steps = (distance / sweep_sample_step_px).ceil().max(1.0) as u32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let pos = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        if !static_standability::unit_static_standable_with_facing(
            map, occ, kind, pos.0, pos.1, facing,
        ) {
            return false;
        }
    }
    true
}

fn clear_car_direct_goal_route(
    profile: CarMotionProfile,
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
) -> bool {
    let Some(route_dir) = unit_direction(from, to) else {
        return false;
    };
    let distance = distance_between(from, to);
    if !distance.is_finite() || distance <= ARRIVE_EPS {
        return false;
    }
    let facing = route_dir.1.atan2(route_dir.0);
    if !facing.is_finite() {
        return false;
    }
    let steps = (distance / profile.sweep_sample_step_px).ceil().max(1.0) as u32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let pos = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        if static_clearance_px(profile, map, occ, kind, pos, facing) + 0.001
            < profile.clearance_score_max_px
        {
            return false;
        }
    }
    true
}

pub(super) fn static_clearance_px(
    profile: CarMotionProfile,
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    pos: (f32, f32),
    facing: f32,
) -> f32 {
    let Some(body) = unit_body_with_facing(kind, pos.0, pos.1, facing) else {
        return -1.0;
    };
    if body_hits_static_blocker(map, occ, kind, body) {
        return -1.0;
    }

    let mut clearance = 0.0;
    while clearance <= profile.clearance_score_max_px {
        let expanded = expanded_body(body, clearance + 2.0);
        if body_hits_static_blocker(map, occ, kind, expanded) {
            return clearance;
        }
        clearance += 2.0;
    }
    profile.clearance_score_max_px
}

fn expanded_body(body: UnitBody, extra_px: f32) -> UnitBody {
    match body {
        UnitBody::Circle(body) => UnitBody::Circle(CircleBody {
            x: body.x,
            y: body.y,
            radius: body.radius + extra_px,
        }),
        UnitBody::OrientedCapsule(body) => UnitBody::OrientedCapsule(OrientedCapsuleBody {
            x: body.x,
            y: body.y,
            half_segment: body.half_segment,
            radius: body.radius + extra_px,
            facing: body.facing,
        }),
        UnitBody::OrientedBox(body) => UnitBody::OrientedBox(OrientedBoxBody {
            x: body.x,
            y: body.y,
            half_len: body.half_len + extra_px,
            half_width: body.half_width + extra_px,
            facing: body.facing,
        }),
    }
}

fn body_hits_static_blocker(map: &Map, occ: &Occupancy, kind: EntityKind, body: UnitBody) -> bool {
    let aabb = body.aabb();
    if aabb.min_x < 0.0
        || aabb.min_y < 0.0
        || aabb.max_x > map.world_width_px()
        || aabb.max_y > map.world_height_px()
    {
        return true;
    }

    for (tx, ty) in body_tile_range(body) {
        if !map.in_bounds(tx, ty) {
            return true;
        }
        if (!map.is_passable(tx, ty) || !occ.passable_for_kind(tx, ty, kind))
            && unit_body_intersects_rect(body, tile_rect(tx, ty))
        {
            return true;
        }
    }
    false
}

fn body_tile_range(body: UnitBody) -> impl Iterator<Item = (i32, i32)> {
    let ts = config::TILE_SIZE as f32;
    let eps = 0.001;
    let aabb = body.aabb();
    let min_tx = ((aabb.min_x - eps) / ts).floor() as i32;
    let min_ty = ((aabb.min_y - eps) / ts).floor() as i32;
    let max_tx = ((aabb.max_x + eps) / ts).ceil() as i32 - 1;
    let max_ty = ((aabb.max_y + eps) / ts).ceil() as i32 - 1;

    (min_ty..=max_ty).flat_map(move |ty| (min_tx..=max_tx).map(move |tx| (tx, ty)))
}
