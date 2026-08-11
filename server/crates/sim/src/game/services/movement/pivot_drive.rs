use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::geometry::unit_body_for_entity;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;

use super::standability::{footing_profile, footing_resistance, FootingProfile};
use super::traffic::{
    car_will_start_reverse_to_final_waypoint, traffic_body_half_width,
    vehicle_body_half_width_with_clearance,
};
use super::vehicle_profiles::pivot_drive_profile;
use super::vehicle_route::vehicle_desired_path_point;
use super::{MAX_UNIT_BOUNDING_RADIUS_PX, STEERING_MAX_NEIGHBORS};

pub(super) use super::vehicle_profiles::VEHICLE_REVERSE_GOAL_DISTANCE_PX;
#[cfg(test)]
pub(super) use super::vehicle_profiles::{
    ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK, PIVOT_VEHICLE_BODY_TURN_RATE_RAD_PER_TICK,
    PIVOT_VEHICLE_LOOKAHEAD_PX,
};
const VEHICLE_TRAFFIC_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 2.0;
const VEHICLE_TRAFFIC_TURN_BIAS_RAD: f32 = 0.28;
const VEHICLE_FOLLOW_ALIGNMENT_COS_MIN: f32 = 0.5;
const VEHICLE_FOLLOW_LONGITUDINAL_DEADBAND_PX: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PivotDriveIntent {
    pub(super) desired_facing: f32,
    pub(super) traffic_facing: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PivotTrafficAdjustment {
    pub(super) throttle_scale: f32,
    pub(super) turn_bias: f32,
}

pub(super) fn vehicle_traffic_adjustment(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    id: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    facing: f32,
) -> PivotTrafficAdjustment {
    if !facing.is_finite() {
        return PivotTrafficAdjustment {
            throttle_scale: 1.0,
            turn_bias: 0.0,
        };
    }

    let forward = (facing.cos(), facing.sin());
    let side = (-forward.1, forward.0);
    let follow_forward = entities
        .get(id)
        .and_then(|entity| forward_traffic_heading(entity, facing));
    let vehicle_half_width = vehicle_body_half_width_with_clearance(kind);
    let query_radius = VEHICLE_TRAFFIC_LOOKAHEAD_PX + MAX_UNIT_BOUNDING_RADIUS_PX;
    let mut throttle_scale = 1.0_f32;
    let mut side_pressure = 0.0_f32;
    let mut neighbors: Vec<u32> = spatial
        .ids_in_circle_bbox(x, y, query_radius)
        .filter(|&neighbor_id| neighbor_id != id)
        .collect();
    neighbors.sort_unstable();
    neighbors.truncate(STEERING_MAX_NEIGHBORS);

    for neighbor_id in neighbors {
        let Some(neighbor) = entities.get(neighbor_id) else {
            continue;
        };
        if neighbor.hp == 0 || !neighbor.is_unit() {
            continue;
        }
        let profile = footing_profile(neighbor);
        if matches!(profile, FootingProfile::Ghost | FootingProfile::Soft) {
            continue;
        }

        let dx = neighbor.pos_x - x;
        let dy = neighbor.pos_y - y;
        // Similar-heading traffic yields only from the trailing vehicle to avoid reciprocal stops.
        if let (Some(ego_forward), Some(neighbor_forward)) = (
            follow_forward,
            forward_traffic_heading(neighbor, neighbor.facing()),
        ) {
            let alignment = ego_forward.0 * neighbor_forward.0 + ego_forward.1 * neighbor_forward.1;
            if alignment >= VEHICLE_FOLLOW_ALIGNMENT_COS_MIN {
                let shared = (
                    ego_forward.0 + neighbor_forward.0,
                    ego_forward.1 + neighbor_forward.1,
                );
                let shared_len = (shared.0 * shared.0 + shared.1 * shared.1).sqrt();
                if shared_len > 1.0e-4 {
                    let neighbor_ahead = (dx * shared.0 + dy * shared.1) / shared_len;
                    if neighbor_ahead <= VEHICLE_FOLLOW_LONGITUDINAL_DEADBAND_PX {
                        continue;
                    }
                }
            }
        }
        let ahead = dx * forward.0 + dy * forward.1;
        if ahead <= 0.0 || ahead > VEHICLE_TRAFFIC_LOOKAHEAD_PX {
            continue;
        }
        let lateral = dx * side.0 + dy * side.1;
        let neighbor_half_width = traffic_body_half_width(kind, neighbor.kind)
            .or_else(|| unit_body_for_entity(neighbor).map(|body| body.bounding_radius()))
            .unwrap_or_else(|| neighbor.radius());
        if lateral.abs() > vehicle_half_width + neighbor_half_width {
            continue;
        }

        let closeness = 1.0 - (ahead / VEHICLE_TRAFFIC_LOOKAHEAD_PX).clamp(0.0, 1.0);
        let resistance = footing_resistance(profile);
        if uses_oriented_vehicle_body(neighbor.kind) || profile == FootingProfile::Braced {
            throttle_scale = throttle_scale.min((1.0 - closeness * 0.95).clamp(0.0, 1.0));
        } else {
            throttle_scale = throttle_scale.min((1.0 - closeness * 0.65).clamp(0.25, 1.0));
        }

        let side_sign = if lateral.abs() <= 1.0e-4 {
            if id < neighbor_id {
                -1.0
            } else {
                1.0
            }
        } else {
            -lateral.signum()
        };
        side_pressure += side_sign * closeness * resistance.sqrt();
    }

    let turn_bias = if side_pressure.abs() <= 1.0e-4 {
        0.0
    } else {
        side_pressure.signum() * VEHICLE_TRAFFIC_TURN_BIAS_RAD
    };
    PivotTrafficAdjustment {
        throttle_scale,
        turn_bias,
    }
}

fn forward_traffic_heading(entity: &Entity, facing: f32) -> Option<(f32, f32)> {
    if !uses_oriented_vehicle_body(entity.kind) || entity.path_is_empty() || !facing.is_finite() {
        return None;
    }
    let forward = (facing.cos(), facing.sin());
    if matches!(entity.kind, EntityKind::ScoutCar | EntityKind::CommandCar) {
        let reversing = entity
            .movement
            .as_ref()
            .is_some_and(|movement| movement.scout_car_reverse_waypoint.is_some())
            || car_will_start_reverse_to_final_waypoint(entity, facing);
        return (!reversing).then_some(forward);
    }
    let next = entity.next_waypoint()?;
    let to_next = (next.0 - entity.pos_x, next.1 - entity.pos_y);
    let forward_progress = to_next.0 * forward.0 + to_next.1 * forward.1;
    if forward_progress > 0.0 {
        return Some(forward);
    }
    let distance = to_next.0.hypot(to_next.1);
    (super::armor_reaction::locked_source_facing(entity).is_some()
        && distance.is_finite()
        && distance > 1.0e-4)
        .then_some((to_next.0 / distance, to_next.1 / distance))
}

pub(super) fn vehicle_body_turn_rate(kind: EntityKind) -> f32 {
    pivot_drive_profile(kind).body_turn_rate_rad_per_tick
}

pub(super) fn pivot_drive_intent(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    x: f32,
    y: f32,
) -> Option<PivotDriveIntent> {
    let (desired_x, desired_y) = pivot_drive_desired_path_point(map, occ, e, x, y)?;
    let dx = desired_x - x;
    let dy = desired_y - y;
    let dist = (dx * dx + dy * dy).sqrt();
    if !dist.is_finite() || dist <= 1.0e-4 {
        return None;
    }

    let profile = pivot_drive_profile(e.kind);
    let travel_facing = dy.atan2(dx);
    let normal_desired_facing =
        if pivot_drive_desired_point_is_final_waypoint(e, (desired_x, desired_y))
            && dist <= profile.reverse_goal_distance_px
            && angle_delta(e.facing(), travel_facing).abs() > profile.reverse_min_behind_angle_rad
        {
            normalize_angle(travel_facing + std::f32::consts::PI)
        } else {
            travel_facing
        };

    let desired_facing =
        super::armor_reaction::locked_source_facing(e).map_or(normal_desired_facing, |preferred| {
            let reverse_facing = normalize_angle(travel_facing + std::f32::consts::PI);
            let preferred_facing = if angle_delta(preferred, reverse_facing).abs()
                < angle_delta(preferred, travel_facing).abs()
            {
                reverse_facing
            } else {
                travel_facing
            };
            if super::armor_reaction::facing_preference_within_pivot_cap(
                e.facing(),
                preferred_facing,
            ) {
                preferred_facing
            } else {
                normal_desired_facing
            }
        });
    let reverse_facing = normalize_angle(travel_facing + std::f32::consts::PI);
    let reversing = angle_delta(desired_facing, reverse_facing).abs() <= 1.0e-4;

    Some(PivotDriveIntent {
        desired_facing,
        traffic_facing: if reversing { travel_facing } else { e.facing() },
    })
}

/// Signed shortest angular delta from `from` to `to`, in radians.
pub(crate) fn angle_delta(from: f32, to: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    (to - from + std::f32::consts::PI).rem_euclid(two_pi) - std::f32::consts::PI
}

pub(super) fn normalize_angle(angle: f32) -> f32 {
    angle_delta(0.0, angle)
}

pub(crate) fn rotate_toward(current: f32, desired: f32, max_delta: f32) -> f32 {
    if !desired.is_finite() || !max_delta.is_finite() {
        return current;
    }
    if !current.is_finite() {
        return desired;
    }
    let delta = angle_delta(current, desired);
    if delta.abs() <= max_delta {
        desired
    } else {
        current + delta.signum() * max_delta
    }
}

pub(super) fn pivot_drive_speed_scale(kind: EntityKind, abs_angle_error: f32) -> f32 {
    pivot_drive_profile(kind).speed_scale(abs_angle_error)
}

pub(super) fn close_nudge_hull_axis_motion(
    kind: EntityKind,
    path_dir: (f32, f32),
    body_facing: f32,
    budget: f32,
) -> ((f32, f32), f32) {
    if !body_facing.is_finite() {
        return (path_dir, budget);
    }
    let (fx, fy) = (body_facing.cos(), body_facing.sin());
    if !fx.is_finite() || !fy.is_finite() {
        return (path_dir, budget);
    }
    let forward = (fx, fy);
    let dot = path_dir.0 * forward.0 + path_dir.1 * forward.1;
    let aligned = pivot_drive_profile(kind).close_nudge_allows_translation(dot);
    let step_budget = if aligned { budget } else { 0.0 };
    if dot < 0.0 {
        ((-forward.0, -forward.1), step_budget)
    } else {
        (forward, step_budget)
    }
}

pub(super) fn distance_between(from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    (dx * dx + dy * dy).sqrt()
}

pub(super) fn pivot_drive_desired_path_point(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    x: f32,
    y: f32,
) -> Option<(f32, f32)> {
    vehicle_desired_path_point(map, occ, e, x, y)
}

fn pivot_drive_desired_point_is_final_waypoint(e: &Entity, desired: (f32, f32)) -> bool {
    let Some(path) = e.movement.as_ref().map(|m| m.path.as_slice()) else {
        return false;
    };
    path.len() == 1 && distance_between(path[0], desired) <= 1.0e-3
}
