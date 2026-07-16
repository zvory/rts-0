use std::collections::HashMap;

use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::geometry::unit_body_for_entity;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::PlayerState;
use crate::protocol::{Event, NoticeSeverity};

use super::scout_car::vehicle_desired_path_point;
use super::standability::{footing_profile, footing_resistance, FootingProfile};
use super::{MAX_UNIT_BOUNDING_RADIUS_PX, STEERING_MAX_NEIGHBORS};

pub(crate) const TANK_BODY_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
pub(super) const ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
pub(super) const PIVOT_VEHICLE_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 5.0;
pub(super) const VEHICLE_REVERSE_GOAL_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 3.0;
const VEHICLE_REVERSE_MIN_BEHIND_ANGLE_RAD: f32 = std::f32::consts::FRAC_PI_2;
const PIVOT_VEHICLE_CRAWL_ANGLE_RAD: f32 = 0.55;
const PIVOT_VEHICLE_PIVOT_ANGLE_RAD: f32 = 1.25;
const VEHICLE_TRAFFIC_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 2.0;
const VEHICLE_TRAFFIC_TURN_BIAS_RAD: f32 = 0.28;
const VEHICLE_FOLLOW_ALIGNMENT_COS_MIN: f32 = 0.5;
const VEHICLE_FOLLOW_LONGITUDINAL_DEADBAND_PX: f32 = 1.0;

pub(super) fn vehicle_oil_starves_movement(
    entities: &mut EntityStore,
    players: &[PlayerState],
    events: &mut HashMap<u32, Vec<Event>>,
    id: u32,
) -> bool {
    let (owner, x, y) = match entities.get(id) {
        Some(e) => (e.owner, e.pos_x, e.pos_y),
        None => return false,
    };

    let pause_ticks = entities
        .get(id)
        .and_then(|e| e.movement.as_ref())
        .map(|m| m.oil_starved_pause_ticks)
        .unwrap_or(0);
    if pause_ticks > 0 {
        if let Some(e) = entities.get_mut(id) {
            if let Some(m) = e.movement.as_mut() {
                m.oil_starved_pause_ticks = pause_ticks.saturating_sub(1);
            }
        }
        return true;
    }

    let out_of_oil = players
        .iter()
        .find(|p| p.id == owner)
        .is_some_and(|p| p.oil == 0);
    if out_of_oil {
        if let Some(e) = entities.get_mut(id) {
            if let Some(m) = e.movement.as_mut() {
                m.oil_starved_pause_ticks = config::TANK_OIL_STARVED_PAUSE_TICKS.saturating_sub(1);
            }
        }
        events.entry(owner).or_default().push(Event::Notice {
            msg: "alert:out_of_oil".to_string(),
            x: Some(x),
            y: Some(y),
            severity: NoticeSeverity::Alert,
        });
        return true;
    }

    false
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PivotDriveIntent {
    pub(super) desired_facing: f32,
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
        // Similar-heading moving vehicles form a local traffic stream. Only the trailing vehicle
        // yields; making the leader react to its follower can trap both in reciprocal throttling.
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
    (forward_progress > 0.0).then_some(forward)
}

fn car_will_start_reverse_to_final_waypoint(entity: &Entity, facing: f32) -> bool {
    let Some(movement) = entity.movement.as_ref() else {
        return false;
    };
    if movement.path.len() != 1 {
        return false;
    }
    let Some(next) = entity.next_waypoint() else {
        return false;
    };
    let dx = next.0 - entity.pos_x;
    let dy = next.1 - entity.pos_y;
    let dist = (dx * dx + dy * dy).sqrt();
    if !dist.is_finite() || dist <= 1.0e-4 || dist > VEHICLE_REVERSE_GOAL_DISTANCE_PX {
        return false;
    }
    let desired = dy.atan2(dx);
    angle_delta(facing, desired).abs() > VEHICLE_REVERSE_MIN_BEHIND_ANGLE_RAD
}

fn vehicle_body_half_width_with_clearance(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::AntiTankGun | EntityKind::MortarTeam => {
            config::ANTI_TANK_GUN_BODY_WIDTH_PX * 0.5 + config::ANTI_TANK_GUN_BODY_CLEARANCE_PX
        }
        EntityKind::Artillery => {
            config::ARTILLERY_BODY_WIDTH_PX * 0.5 + config::ARTILLERY_BODY_CLEARANCE_PX
        }
        EntityKind::ScoutCar => {
            config::SCOUT_CAR_BODY_WIDTH_PX * 0.5 + config::SCOUT_CAR_BODY_CLEARANCE_PX
        }
        _ => config::TANK_BODY_WIDTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX,
    }
}

fn traffic_body_half_width(ego_kind: EntityKind, neighbor_kind: EntityKind) -> Option<f32> {
    (ego_kind == EntityKind::ScoutCar && uses_oriented_vehicle_body(neighbor_kind))
        .then(|| vehicle_body_half_width_with_clearance(neighbor_kind))
}

pub(super) fn vehicle_body_turn_rate(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::MortarTeam => std::f32::consts::TAU,
        EntityKind::AntiTankGun | EntityKind::Artillery => {
            ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK
        }
        _ => TANK_BODY_TURN_RATE_RAD_PER_TICK,
    }
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

    let forward_desired = dy.atan2(dx);
    if pivot_drive_desired_point_is_final_waypoint(e, (desired_x, desired_y))
        && dist <= VEHICLE_REVERSE_GOAL_DISTANCE_PX
        && angle_delta(e.facing(), forward_desired).abs() > VEHICLE_REVERSE_MIN_BEHIND_ANGLE_RAD
    {
        return Some(PivotDriveIntent {
            desired_facing: normalize_angle(forward_desired + std::f32::consts::PI),
        });
    }

    Some(PivotDriveIntent {
        desired_facing: forward_desired,
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

pub(super) fn pivot_drive_speed_scale(abs_angle_error: f32) -> f32 {
    if !abs_angle_error.is_finite() {
        return 0.0;
    }
    if abs_angle_error <= PIVOT_VEHICLE_CRAWL_ANGLE_RAD {
        1.0
    } else if abs_angle_error >= PIVOT_VEHICLE_PIVOT_ANGLE_RAD {
        0.0
    } else {
        let t = (abs_angle_error - PIVOT_VEHICLE_CRAWL_ANGLE_RAD)
            / (PIVOT_VEHICLE_PIVOT_ANGLE_RAD - PIVOT_VEHICLE_CRAWL_ANGLE_RAD);
        1.0 - t
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
