//! Support-weapon setup, range, projectile, and area-effect constants.

use super::TICK_HZ;
use crate::EntityKind;

pub const MACHINE_GUNNER_SETUP_TICKS: u16 = TICK_HZ as u16;
pub const METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS: u16 = MACHINE_GUNNER_SETUP_TICKS / 2;
pub const ANTI_TANK_GUN_SETUP_TICKS: u16 = (TICK_HZ as u16) * 5 / 2;
pub const ANTI_TANK_GUN_TEARDOWN_TICKS: u16 = (TICK_HZ as u16) * 3 / 2;
pub const MORTAR_TEAM_SETUP_TICKS: u16 = (TICK_HZ as u16) * 3 / 2;
pub const MORTAR_TEAM_TEARDOWN_TICKS: u16 = (TICK_HZ as u16) / 2;
pub const MORTAR_RANGE_TILES: u32 = 17;
pub const MORTAR_MIN_RANGE_TILES: u32 = 5;
pub const MORTAR_FIELD_OF_FIRE_RAD: f32 = std::f32::consts::TAU;
pub const MORTAR_SHELL_DELAY_TICKS: u32 = (TICK_HZ * 9 + 2) / 4;
pub const MORTAR_OUTER_RADIUS_TILES: f32 = 1.5;
pub const MORTAR_INNER_RADIUS_TILES: f32 = 0.5;
pub const MORTAR_OUTER_DAMAGE: u32 = 40;
pub const MORTAR_INNER_DAMAGE: u32 = 100;
pub const MORTAR_VISIBLE_MEDIAN_SCATTER_TILES: f32 = 1.0;
pub const MORTAR_BLIND_MEDIAN_SCATTER_TILES: f32 = 4.0;

pub const ANTI_TANK_GUN_DEPLOYED_RANGE_TILES: u32 = 20;
pub const ANTI_TANK_GUN_FIELD_OF_FIRE_RAD: f32 = 35.0_f32 * std::f32::consts::PI / 180.0;

pub const PANZERFAUST_RANGE_TILES: u32 = 5;
pub const PANZERFAUST_DAMAGE: u32 = 100;
pub const PANZERFAUST_ARMOR_PENETRATION: f32 = 0.5;
pub const PANZERFAUST_WINDUP_TICKS: u16 = (TICK_HZ as u16) / 2;
pub const PANZERFAUST_TRAVEL_TICKS: u32 = TICK_HZ / 2;
pub const METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS: u16 =
    (PANZERFAUST_WINDUP_TICKS * 3).div_ceil(4);

pub const ARTILLERY_SETUP_TICKS: u16 = (TICK_HZ as u16) * 6;
pub const ARTILLERY_RELOAD_TICKS: u32 = TICK_HZ * 3;
pub const ARTILLERY_SHELL_DELAY_TICKS: u32 = TICK_HZ * 5;
pub const ARTILLERY_MIN_RANGE_TILES: u32 = 10;
pub const ARTILLERY_MAX_RANGE_TILES: u32 = 35;
pub const ARTILLERY_FIELD_OF_FIRE_RAD: f32 = 30.0_f32 * std::f32::consts::PI / 180.0;
pub const ARTILLERY_AMMO_COST_STEEL: u32 = 10;
pub const ARTILLERY_INNER_RADIUS_TILES: f32 = 1.0;
pub const ARTILLERY_OUTER_RADIUS_TILES: f32 = 3.0;
pub const ARTILLERY_INNER_DAMAGE: u32 = 75;
pub const ARTILLERY_OUTER_MIN_DAMAGE: u32 = 5;

pub const fn support_weapon_setup_ticks(kind: EntityKind) -> Option<u16> {
    match kind {
        EntityKind::MachineGunner => Some(MACHINE_GUNNER_SETUP_TICKS),
        EntityKind::AntiTankGun => Some(ANTI_TANK_GUN_SETUP_TICKS),
        EntityKind::MortarTeam => Some(MORTAR_TEAM_SETUP_TICKS),
        EntityKind::Artillery => Some(ARTILLERY_SETUP_TICKS),
        _ => None,
    }
}

pub const fn support_weapon_teardown_ticks(kind: EntityKind) -> Option<u16> {
    match kind {
        EntityKind::MachineGunner => Some(MACHINE_GUNNER_SETUP_TICKS),
        EntityKind::AntiTankGun => Some(ANTI_TANK_GUN_TEARDOWN_TICKS),
        EntityKind::MortarTeam => Some(MORTAR_TEAM_TEARDOWN_TICKS),
        EntityKind::Artillery => Some(ARTILLERY_SETUP_TICKS),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_weapon_transition_timing_is_kind_specific() {
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
        assert_eq!(
            support_weapon_setup_ticks(EntityKind::MortarTeam),
            Some(MORTAR_TEAM_SETUP_TICKS)
        );
        assert_eq!(
            support_weapon_teardown_ticks(EntityKind::MortarTeam),
            Some(MORTAR_TEAM_TEARDOWN_TICKS)
        );
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
