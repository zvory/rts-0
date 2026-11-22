//! Support-weapon setup, range, projectile, and area-effect constants.

use super::TICK_HZ;
use crate::EntityKind;

pub const MACHINE_GUNNER_SETUP_TICKS: u16 = TICK_HZ as u16;
pub const METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS: u16 = MACHINE_GUNNER_SETUP_TICKS / 2;
pub const ANTI_TANK_GUN_SETUP_TICKS: u16 = 120;
pub const ANTI_TANK_GUN_TEARDOWN_TICKS: u16 = 30;
// Tank HE: direct-hit damage stays in the cannon profile.
pub const TANK_HE_RADIUS_TILES: f32 = 1.4;
pub const TANK_HE_SPLASH_DAMAGE: u32 = 30;

pub const MORTAR_RANGE_TILES: u32 = 17;
pub const MORTAR_MIN_RANGE_TILES: u32 = 5;
pub const MORTAR_FIELD_OF_FIRE_RAD: f32 = std::f32::consts::TAU;
pub const MORTAR_MANUAL_SHELL_DELAY_TICKS: u32 = 53;
pub const MORTAR_OUTER_RADIUS_TILES: f32 = 2.0;
/// Cosmetic blast-core radius only; damage has no inner band.
pub const MORTAR_INNER_RADIUS_TILES: f32 = 0.5;
/// Damage at the outer radius, before armor and protection modifiers.
pub const MORTAR_OUTER_DAMAGE: u32 = 10;
/// Peak damage at the impact point.
pub const MORTAR_INNER_DAMAGE: u32 = 100;

/// Rounded inverse-square splash, measured from impact to target center in tiles.
/// The endpoint ratio fixes the curvature: 100 / (1 + 2.25 * distance_tiles^2).
/// Round base damage once before the existing armor/entrenchment/terrain pipeline.
pub fn mortar_damage_at_distance_squared(distance_squared_tiles: f32) -> u32 {
    let outer_squared = MORTAR_OUTER_RADIUS_TILES * MORTAR_OUTER_RADIUS_TILES;
    if !distance_squared_tiles.is_finite()
        || distance_squared_tiles < 0.0
        || distance_squared_tiles > outer_squared
    {
        return 0;
    }
    let curvature = MORTAR_INNER_DAMAGE as f32 / MORTAR_OUTER_DAMAGE as f32 - 1.0;
    (MORTAR_INNER_DAMAGE as f32 / (1.0 + curvature * (distance_squared_tiles / outer_squared)))
        .round() as u32
}

pub const ANTI_TANK_GUN_DEPLOYED_RANGE_TILES: u32 = 20;
pub const ANTI_TANK_GUN_FIELD_OF_FIRE_RAD: f32 = 40.0_f32 * std::f32::consts::PI / 180.0;
/// Anti-Tank Gun hits deal 30% of their normal damage to infantry-sized targets.
pub const ANTI_TANK_GUN_INFANTRY_DAMAGE_MULTIPLIER: f32 = 0.30;

/// Shared manual-fire ability and weapon reload for mortars.
pub const MORTAR_FIRE_COOLDOWN_TICKS: u32 = TICK_HZ * 6;

pub const PANZERFAUST_RANGE_TILES: u32 = 5;
pub const PANZERFAUST_DAMAGE: u32 = 100;
pub const PANZERFAUST_ARMOR_PENETRATION: f32 = 0.5;
pub const PANZERFAUST_WINDUP_TICKS: u16 = (TICK_HZ as u16) / 2;
pub const PANZERFAUST_TRAVEL_TICKS: u32 = TICK_HZ / 2;
pub const METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS: u16 =
    (PANZERFAUST_WINDUP_TICKS * 3).div_ceil(4);

pub const ARTILLERY_SETUP_TICKS: u16 = (TICK_HZ as u16) * 6;
pub const ARTILLERY_RELOAD_TICKS: u32 = TICK_HZ * 6;
pub const ARTILLERY_SHELL_DELAY_TICKS: u32 = TICK_HZ * 5;
pub const ARTILLERY_MIN_RANGE_TILES: u32 = 10;
pub const ARTILLERY_MAX_RANGE_TILES: u32 = 35;
pub const ARTILLERY_FIELD_OF_FIRE_RAD: f32 = 30.0_f32 * std::f32::consts::PI / 180.0;
pub const ARTILLERY_AMMO_COST_STEEL: u32 = 0;
pub const ARTILLERY_INNER_RADIUS_TILES: f32 = 2.0 / 3.0;
pub const ARTILLERY_OUTER_RADIUS_TILES: f32 = 2.0;
pub const ARTILLERY_INNER_DAMAGE: u32 = 75;
pub const ARTILLERY_OUTER_MIN_DAMAGE: u32 = 20;

pub const fn support_weapon_setup_ticks(kind: EntityKind) -> Option<u16> {
    match kind {
        EntityKind::MachineGunner => Some(MACHINE_GUNNER_SETUP_TICKS),
        EntityKind::AntiTankGun => Some(ANTI_TANK_GUN_SETUP_TICKS),
        EntityKind::Artillery => Some(ARTILLERY_SETUP_TICKS),
        _ => None,
    }
}

pub const fn support_weapon_teardown_ticks(kind: EntityKind) -> Option<u16> {
    match kind {
        EntityKind::MachineGunner => Some(MACHINE_GUNNER_SETUP_TICKS),
        EntityKind::AntiTankGun => Some(ANTI_TANK_GUN_TEARDOWN_TICKS),
        EntityKind::Artillery => Some(ARTILLERY_SETUP_TICKS),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mortar_inverse_square_has_requested_endpoints_and_no_inner_plateau() {
        for (distance, expected) in [
            (0.0, 100),
            (0.25, 88),
            (0.5, 64),
            (0.75, 44),
            (1.0, 31),
            (1.25, 22),
            (1.5, 16),
            (1.75, 13),
            (2.0, 10),
            (2.001, 0),
        ] {
            assert_eq!(
                mortar_damage_at_distance_squared(distance * distance),
                expected,
                "distance={distance} tiles"
            );
        }
        let mut previous = MORTAR_INNER_DAMAGE;
        for step in 0..=2000 {
            let distance = step as f32 / 1000.0;
            let damage = mortar_damage_at_distance_squared(distance * distance);
            assert!((MORTAR_OUTER_DAMAGE..=previous).contains(&damage));
            previous = damage;
        }
        for invalid in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(mortar_damage_at_distance_squared(invalid), 0);
        }
    }

    #[test]
    fn support_weapon_transition_timing_is_kind_specific() {
        assert_eq!(ANTI_TANK_GUN_SETUP_TICKS, 120);
        assert_eq!(ANTI_TANK_GUN_TEARDOWN_TICKS, 30);
        assert_eq!(
            support_weapon_setup_ticks(EntityKind::MachineGunner),
            Some(MACHINE_GUNNER_SETUP_TICKS)
        );
        assert_eq!(
            support_weapon_teardown_ticks(EntityKind::MachineGunner),
            Some(MACHINE_GUNNER_SETUP_TICKS)
        );
        assert_eq!(
            support_weapon_setup_ticks(EntityKind::AntiTankGun),
            Some(ANTI_TANK_GUN_SETUP_TICKS)
        );
        assert_eq!(
            support_weapon_teardown_ticks(EntityKind::AntiTankGun),
            Some(ANTI_TANK_GUN_TEARDOWN_TICKS)
        );
        assert_eq!(support_weapon_setup_ticks(EntityKind::MortarTeam), None);
        assert_eq!(support_weapon_teardown_ticks(EntityKind::MortarTeam), None);
        assert_eq!(
            support_weapon_setup_ticks(EntityKind::Artillery),
            Some(ARTILLERY_SETUP_TICKS)
        );
        assert_eq!(
            support_weapon_teardown_ticks(EntityKind::Artillery),
            Some(ARTILLERY_SETUP_TICKS)
        );
        assert_eq!(support_weapon_setup_ticks(EntityKind::Rifleman), None);
        assert_eq!(support_weapon_teardown_ticks(EntityKind::Rifleman), None);
    }
}
