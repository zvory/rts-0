use crate::config;
use crate::game::entity::EntityKind;

pub(super) const PIVOT_VEHICLE_BODY_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
pub(super) const TANK_BODY_TURN_RATE_RAD_PER_TICK: f32 =
    PIVOT_VEHICLE_BODY_TURN_RATE_RAD_PER_TICK * 1.25;
pub(super) const ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK: f32 =
    50.0_f32.to_radians() / config::TICK_HZ as f32;
pub(super) const PIVOT_VEHICLE_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 5.0;
pub(super) const VEHICLE_REVERSE_GOAL_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 3.0;

const PIVOT_VEHICLE_CRAWL_ANGLE_RAD: f32 = 0.55;
const PIVOT_VEHICLE_PIVOT_ANGLE_RAD: f32 = 1.25;
const VEHICLE_REVERSE_MIN_BEHIND_ANGLE_RAD: f32 = std::f32::consts::FRAC_PI_2;

// Gives the scout car roughly a 1.7-body-length outer swept turning circle.
pub(super) const SCOUT_CAR_MIN_TURN_RADIUS_PX: f32 = 22.9;
pub(super) const SCOUT_CAR_ROUTE_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 3.0;
const SCOUT_CAR_SWEEP_SAMPLE_STEP_PX: f32 = config::TILE_SIZE as f32 * 0.125;
const SCOUT_CAR_CLEARANCE_SCORE_MAX_PX: f32 = config::TILE_SIZE as f32 * 0.5;
const SCOUT_CAR_SCORE_EPS: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PivotDriveProfile {
    pub(super) body_turn_rate_rad_per_tick: f32,
    pub(super) full_speed_angle_rad: f32,
    pub(super) stop_turn_angle_rad: f32,
    pub(super) lookahead_px: f32,
    pub(super) reverse_goal_distance_px: f32,
    pub(super) reverse_min_behind_angle_rad: f32,
}

impl PivotDriveProfile {
    pub(super) fn speed_scale(self, abs_angle_error: f32) -> f32 {
        if !abs_angle_error.is_finite() {
            return 0.0;
        }
        if abs_angle_error <= self.full_speed_angle_rad {
            1.0
        } else if abs_angle_error >= self.stop_turn_angle_rad {
            0.0
        } else {
            let t = (abs_angle_error - self.full_speed_angle_rad)
                / (self.stop_turn_angle_rad - self.full_speed_angle_rad);
            1.0 - t
        }
    }

    pub(super) fn close_nudge_allows_translation(self, path_forward_dot: f32) -> bool {
        path_forward_dot.abs() >= self.full_speed_angle_rad.cos()
    }
}

pub(super) fn pivot_drive_profile(kind: EntityKind) -> PivotDriveProfile {
    PivotDriveProfile {
        body_turn_rate_rad_per_tick: match kind {
            EntityKind::MortarTeam => std::f32::consts::TAU,
            EntityKind::AntiTankGun => ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK,
            EntityKind::Tank => TANK_BODY_TURN_RATE_RAD_PER_TICK,
            _ => PIVOT_VEHICLE_BODY_TURN_RATE_RAD_PER_TICK,
        },
        full_speed_angle_rad: PIVOT_VEHICLE_CRAWL_ANGLE_RAD,
        stop_turn_angle_rad: PIVOT_VEHICLE_PIVOT_ANGLE_RAD,
        lookahead_px: PIVOT_VEHICLE_LOOKAHEAD_PX,
        reverse_goal_distance_px: VEHICLE_REVERSE_GOAL_DISTANCE_PX,
        reverse_min_behind_angle_rad: VEHICLE_REVERSE_MIN_BEHIND_ANGLE_RAD,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CarMotionProfile {
    pub(super) min_turn_radius_px: f32,
    pub(super) route_lookahead_px: f32,
    pub(super) sweep_sample_step_px: f32,
    pub(super) clearance_score_max_px: f32,
    pub(super) score_eps: f32,
    pub(super) reverse_min_behind_angle_rad: f32,
}

pub(super) fn car_motion_profile(kind: EntityKind) -> Option<CarMotionProfile> {
    match kind {
        EntityKind::ScoutCar | EntityKind::CommandCar | EntityKind::RocketLauncher => Some(CarMotionProfile {
            min_turn_radius_px: SCOUT_CAR_MIN_TURN_RADIUS_PX,
            route_lookahead_px: SCOUT_CAR_ROUTE_LOOKAHEAD_PX,
            sweep_sample_step_px: SCOUT_CAR_SWEEP_SAMPLE_STEP_PX,
            clearance_score_max_px: SCOUT_CAR_CLEARANCE_SCORE_MAX_PX,
            score_eps: SCOUT_CAR_SCORE_EPS,
            reverse_min_behind_angle_rad: VEHICLE_REVERSE_MIN_BEHIND_ANGLE_RAD,
        }),
        _ => None,
    }
}
