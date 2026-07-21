use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, Entity, EntityKind};

use super::pivot_drive::{angle_delta, VEHICLE_REVERSE_GOAL_DISTANCE_PX};

const VEHICLE_REVERSE_MIN_BEHIND_ANGLE_RAD: f32 = std::f32::consts::FRAC_PI_2;

pub(super) fn car_will_start_reverse_to_final_waypoint(entity: &Entity, facing: f32) -> bool {
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

pub(super) fn vehicle_body_half_width_with_clearance(kind: EntityKind) -> f32 {
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

pub(super) fn traffic_body_half_width(
    ego_kind: EntityKind,
    neighbor_kind: EntityKind,
) -> Option<f32> {
    (ego_kind == EntityKind::ScoutCar && uses_oriented_vehicle_body(neighbor_kind))
        .then(|| vehicle_body_half_width_with_clearance(neighbor_kind))
}
