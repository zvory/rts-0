//! Vehicle and support-weapon body dimensions mirrored for collision/render previews.

use crate::EntityKind;

pub const TANK_BODY_LENGTH_PX: f32 = 50.4;
pub const TANK_BODY_WIDTH_PX: f32 = 28.8;
pub const TANK_BODY_CLEARANCE_PX: f32 = 1.5;
pub const ANTI_TANK_GUN_BODY_LENGTH_PX: f32 = 42.0;
pub const ANTI_TANK_GUN_BODY_WIDTH_PX: f32 = 24.0;
pub const ANTI_TANK_GUN_BODY_CLEARANCE_PX: f32 = 1.0;
pub const ARTILLERY_BODY_SCALE: f32 = 0.75;
pub const ARTILLERY_BODY_LENGTH_PX: f32 = TANK_BODY_LENGTH_PX * ARTILLERY_BODY_SCALE;
pub const ARTILLERY_BODY_WIDTH_PX: f32 = TANK_BODY_WIDTH_PX * ARTILLERY_BODY_SCALE;
pub const ARTILLERY_BODY_CLEARANCE_PX: f32 = TANK_BODY_CLEARANCE_PX * ARTILLERY_BODY_SCALE;
pub const ARTILLERY_SELECTION_RADIUS_PX: f32 = 18.0 * ARTILLERY_BODY_SCALE;
pub const SCOUT_CAR_BODY_LENGTH_PX: f32 = 40.8;
pub const SCOUT_CAR_BODY_WIDTH_PX: f32 = 21.6;
pub const SCOUT_CAR_BODY_CLEARANCE_PX: f32 = 1.0;
pub const COMMAND_CAR_BODY_LENGTH_PX: f32 = 34.8;
pub const COMMAND_CAR_BODY_WIDTH_PX: f32 = 18.4;
pub const COMMAND_CAR_BODY_CLEARANCE_PX: f32 = 1.0;
pub const ROCKET_LAUNCHER_BODY_LENGTH_PX: f32 = 40.0;
pub const ROCKET_LAUNCHER_BODY_WIDTH_PX: f32 = 22.0;
pub const ROCKET_LAUNCHER_BODY_CLEARANCE_PX: f32 = 1.0;

/// Conservative circular placement clearance for units whose runtime collision body may rotate.
pub fn unit_placement_radius(kind: EntityKind) -> f32 {
    let oriented_half_diagonal = match kind {
        EntityKind::Tank => Some((
            TANK_BODY_LENGTH_PX,
            TANK_BODY_WIDTH_PX,
            TANK_BODY_CLEARANCE_PX,
        )),
        EntityKind::ScoutCar => Some((
            SCOUT_CAR_BODY_LENGTH_PX,
            SCOUT_CAR_BODY_WIDTH_PX,
            SCOUT_CAR_BODY_CLEARANCE_PX,
        )),
        EntityKind::CommandCar => Some((
            COMMAND_CAR_BODY_LENGTH_PX,
            COMMAND_CAR_BODY_WIDTH_PX,
            COMMAND_CAR_BODY_CLEARANCE_PX,
        )),
        EntityKind::AntiTankGun => Some((
            ANTI_TANK_GUN_BODY_LENGTH_PX,
            ANTI_TANK_GUN_BODY_WIDTH_PX,
            ANTI_TANK_GUN_BODY_CLEARANCE_PX,
        )),
        EntityKind::Artillery => Some((
            ARTILLERY_BODY_LENGTH_PX,
            ARTILLERY_BODY_WIDTH_PX,
            ARTILLERY_BODY_CLEARANCE_PX,
        )),
        EntityKind::RocketLauncher => Some((
            ROCKET_LAUNCHER_BODY_LENGTH_PX,
            ROCKET_LAUNCHER_BODY_WIDTH_PX,
            ROCKET_LAUNCHER_BODY_CLEARANCE_PX,
        )),
        _ => None,
    };
    oriented_half_diagonal.map_or_else(
        || super::unit_stats(kind).map_or(0.0, |stats| stats.radius),
        |(length, width, clearance)| {
            ((length * 0.5).powi(2) + (width * 0.5).powi(2)).sqrt() + clearance
        },
    )
}
