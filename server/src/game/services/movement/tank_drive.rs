use std::collections::HashMap;

use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::geometry::unit_body_for_entity;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::standability as static_standability;
use crate::game::PlayerState;
use crate::protocol::{Event, NoticeSeverity};

use super::standability::{footing_profile, footing_resistance, FootingProfile};
use super::{MAX_UNIT_BOUNDING_RADIUS_PX, STEERING_MAX_NEIGHBORS};

pub(crate) const TANK_BODY_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
pub(super) const AT_GUN_BODY_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
pub(super) const TANK_BODY_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 5.0;
pub(super) const TANK_REVERSE_GOAL_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 3.0;
const TANK_REVERSE_MIN_BEHIND_ANGLE_RAD: f32 = std::f32::consts::FRAC_PI_2;
const TANK_CRAWL_ANGLE_RAD: f32 = 0.55;
const TANK_PIVOT_ANGLE_RAD: f32 = 1.25;
const TANK_TRAFFIC_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 2.0;
const TANK_TRAFFIC_TURN_BIAS_RAD: f32 = 0.28;

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
pub(super) struct TankDriveIntent {
    pub(super) desired_facing: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TankTrafficAdjustment {
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
) -> TankTrafficAdjustment {
    if !facing.is_finite() {
        return TankTrafficAdjustment {
            throttle_scale: 1.0,
            turn_bias: 0.0,
        };
    }

    let forward = (facing.cos(), facing.sin());
    let side = (-forward.1, forward.0);
    let vehicle_half_width = vehicle_body_half_width_with_clearance(kind);
    let query_radius = TANK_TRAFFIC_LOOKAHEAD_PX + MAX_UNIT_BOUNDING_RADIUS_PX;
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
        let ahead = dx * forward.0 + dy * forward.1;
        if ahead <= 0.0 || ahead > TANK_TRAFFIC_LOOKAHEAD_PX {
            continue;
        }
        let lateral = dx * side.0 + dy * side.1;
        let neighbor_radius = unit_body_for_entity(neighbor)
            .map(|body| body.bounding_radius())
            .unwrap_or_else(|| neighbor.radius());
        if lateral.abs() > vehicle_half_width + neighbor_radius {
            continue;
        }

        let closeness = 1.0 - (ahead / TANK_TRAFFIC_LOOKAHEAD_PX).clamp(0.0, 1.0);
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
        side_pressure.signum() * TANK_TRAFFIC_TURN_BIAS_RAD
    };

    TankTrafficAdjustment {
        throttle_scale,
        turn_bias,
    }
}

fn vehicle_body_half_width_with_clearance(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::AtTeam => config::AT_GUN_BODY_WIDTH_PX * 0.5 + config::AT_GUN_BODY_CLEARANCE_PX,
        EntityKind::ScoutCar => {
            config::SCOUT_CAR_BODY_WIDTH_PX * 0.5 + config::SCOUT_CAR_BODY_CLEARANCE_PX
        }
        _ => config::TANK_BODY_WIDTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX,
    }
}

pub(super) fn vehicle_body_turn_rate(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::AtTeam => AT_GUN_BODY_TURN_RATE_RAD_PER_TICK,
        _ => TANK_BODY_TURN_RATE_RAD_PER_TICK,
    }
}

pub(super) fn tank_drive_intent(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    x: f32,
    y: f32,
) -> Option<TankDriveIntent> {
    let (desired_x, desired_y) = tank_desired_path_point(map, occ, e, x, y)?;
    let dx = desired_x - x;
    let dy = desired_y - y;
    let dist = (dx * dx + dy * dy).sqrt();
    if !dist.is_finite() || dist <= 1.0e-4 {
        return None;
    }

    let forward_desired = dy.atan2(dx);
    if dist <= TANK_REVERSE_GOAL_DISTANCE_PX
        && angle_delta(e.facing(), forward_desired).abs() > TANK_REVERSE_MIN_BEHIND_ANGLE_RAD
    {
        return Some(TankDriveIntent {
            desired_facing: normalize_angle(forward_desired + std::f32::consts::PI),
        });
    }

    Some(TankDriveIntent {
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

pub(super) fn tank_speed_scale(abs_angle_error: f32) -> f32 {
    if !abs_angle_error.is_finite() {
        return 0.0;
    }
    if abs_angle_error <= TANK_CRAWL_ANGLE_RAD {
        1.0
    } else if abs_angle_error >= TANK_PIVOT_ANGLE_RAD {
        0.0
    } else {
        let t = (abs_angle_error - TANK_CRAWL_ANGLE_RAD)
            / (TANK_PIVOT_ANGLE_RAD - TANK_CRAWL_ANGLE_RAD);
        1.0 - t
    }
}

pub(super) fn distance_between(from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    (dx * dx + dy * dy).sqrt()
}

pub(super) fn tank_desired_path_point(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    x: f32,
    y: f32,
) -> Option<(f32, f32)> {
    let path = &e.movement.as_ref()?.path;
    let next = path.last().copied()?;
    let from = (x, y);
    let mut farthest_reachable = None;

    for waypoint in path.iter().rev().copied() {
        if !static_standability::unit_static_segment_standable(map, occ, e.kind, from, waypoint) {
            break;
        }
        farthest_reachable = Some(waypoint);

        if let Some(point) = point_at_distance(from, waypoint, TANK_BODY_LOOKAHEAD_PX) {
            return Some(point);
        }
    }

    farthest_reachable.or(Some(next))
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
