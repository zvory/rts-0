use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore, Order, WeaponSetup};
use crate::game::services::movement::{angle_delta, rotate_toward};
use crate::rules::combat as combat_rules;

use super::{
    AT_GUN_FIRE_TOLERANCE_RAD, AT_GUN_TURN_RATE_RAD_PER_TICK, TANK_TURRET_FIRE_TOLERANCE_RAD,
    TANK_TURRET_TURN_RATE_RAD_PER_TICK,
};

pub(super) fn rotate_vehicle_weapon_for_combat(e: &mut Entity, target_angle: f32) -> bool {
    if !target_angle.is_finite() {
        return false;
    }
    e.set_desired_weapon_facing(target_angle);
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or_else(|| e.facing());
    let rotated = rotate_toward(current, target_angle, TANK_TURRET_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_weapon_facing(rotated);
    }
    angle_delta(rotated, target_angle).abs() <= TANK_TURRET_FIRE_TOLERANCE_RAD
}

pub(super) fn relax_vehicle_weapon_toward_body(e: &mut Entity) {
    let body = e.facing();
    if !body.is_finite() {
        return;
    }
    e.set_desired_weapon_facing(body);
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or(body);
    let rotated = rotate_toward(current, body, TANK_TURRET_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_weapon_facing(rotated);
    }
}

pub(super) fn mirror_weapon_to_body(e: &mut Entity, angle: f32) {
    if !angle.is_finite() {
        return;
    }
    e.set_desired_weapon_facing(angle);
    e.set_weapon_facing(angle);
}

pub(super) fn rotate_at_gun_for_combat(e: &mut Entity, target_angle: f32) -> bool {
    if !target_angle.is_finite() {
        return false;
    }
    let desired = deployed_at_gun_desired_facing(e, target_angle);
    e.set_desired_weapon_facing(desired);
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or_else(|| {
            let facing = e.facing();
            if facing.is_finite() {
                facing
            } else {
                0.0
            }
        });
    let rotated = rotate_toward(current, desired, AT_GUN_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_facing(rotated);
        e.set_weapon_facing(rotated);
    } else {
        return false;
    }
    at_gun_target_inside_field_of_fire(e, target_angle)
        && angle_delta(rotated, target_angle).abs() <= AT_GUN_FIRE_TOLERANCE_RAD
}

pub(super) fn tick_deployed_weapon_setup(e: &mut Entity) {
    if !requires_weapon_setup(e.kind) {
        return;
    }
    e.tick_weapon_setup();
}

pub(super) fn begin_idle_deployed_weapon_setup(e: &mut Entity) {
    if e.kind != EntityKind::MachineGunner {
        return;
    }
    if !e.path_is_empty() {
        return;
    }
    if !matches!(
        e.order(),
        Order::Idle | Order::Attack(_) | Order::AttackMove(_)
    ) {
        return;
    }
    if matches!(e.weapon_setup(), WeaponSetup::Packed) {
        e.set_weapon_setup(WeaponSetup::SettingUp {
            ticks: config::MACHINE_GUNNER_SETUP_TICKS,
        });
    }
}

pub(super) fn deployed_weapon_ready_to_fire(entities: &mut EntityStore, id: u32) -> bool {
    let Some(e) = entities.get_mut(id) else {
        return false;
    };
    if !requires_weapon_setup(e.kind) || e.kind == EntityKind::AtTeam {
        return true;
    }
    match e.weapon_setup() {
        WeaponSetup::Deployed => true,
        WeaponSetup::Packed => {
            e.set_weapon_setup(WeaponSetup::SettingUp {
                ticks: setup_ticks_for(e.kind),
            });
            false
        }
        WeaponSetup::SettingUp { .. } | WeaponSetup::TearingDown { .. } => false,
    }
}

pub(super) fn deployed_weapon_ready_to_move(entities: &mut EntityStore, id: u32) -> bool {
    let Some(e) = entities.get_mut(id) else {
        return false;
    };
    if !requires_weapon_setup(e.kind) {
        return true;
    }
    match e.weapon_setup() {
        WeaponSetup::Packed => true,
        WeaponSetup::Deployed | WeaponSetup::SettingUp { .. } => {
            e.set_weapon_setup(WeaponSetup::TearingDown {
                ticks: setup_ticks_for(e.kind),
            });
            false
        }
        WeaponSetup::TearingDown { .. } => false,
    }
}

pub(super) fn setup_ticks_for(kind: EntityKind) -> u16 {
    match kind {
        EntityKind::AtTeam => config::AT_TEAM_SETUP_TICKS,
        _ => config::MACHINE_GUNNER_SETUP_TICKS,
    }
}

fn requires_weapon_setup(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::MachineGunner | EntityKind::AtTeam)
}

pub(super) fn uses_stationary_weapon_aggro(e: &Entity) -> bool {
    matches!(e.kind, EntityKind::MachineGunner)
        || (e.kind == EntityKind::AtTeam && !matches!(e.weapon_setup(), WeaponSetup::Packed))
}

pub(super) fn can_fire_while_moving(e: &Entity) -> bool {
    crate::game::entity::fires_while_moving(e.kind)
        || (e.kind == EntityKind::Rifleman && matches!(e.order(), Order::AttackMove(_)))
}

pub(super) fn moving_fire_miss_chance(e: &Entity) -> f32 {
    if e.kind == EntityKind::Rifleman
        && matches!(e.order(), Order::AttackMove(_))
        && !e.path_is_empty()
    {
        combat_rules::RIFLEMAN_CHARGE_MISS_CHANCE
    } else {
        0.0
    }
}

pub(super) fn at_gun_can_chase(e: &Entity) -> bool {
    e.kind != EntityKind::AtTeam || matches!(e.weapon_setup(), WeaponSetup::Packed)
}

pub(super) fn effective_attack_profile(e: &Entity) -> combat_rules::AttackProfile {
    let mut profile = combat_rules::attack_profile(e.kind);
    if e.kind != EntityKind::AtTeam {
        return profile;
    }
    match e.weapon_setup() {
        WeaponSetup::Packed => {
            profile.range_tiles = config::AT_GUN_PACKED_RANGE_TILES;
            profile.dmg =
                ((profile.dmg as f32) * config::AT_GUN_PACKED_DAMAGE_MULTIPLIER).round() as u32;
        }
        WeaponSetup::Deployed => {
            profile.range_tiles = config::AT_GUN_DEPLOYED_RANGE_TILES;
        }
        WeaponSetup::SettingUp { .. } | WeaponSetup::TearingDown { .. } => {
            profile.range_tiles = config::AT_GUN_PACKED_RANGE_TILES;
            profile.dmg = 0;
        }
    }
    profile
}

fn deployed_at_gun_desired_facing(e: &Entity, target_angle: f32) -> f32 {
    if !matches!(e.weapon_setup(), WeaponSetup::Deployed) {
        return target_angle;
    }
    let Some(center) = at_gun_field_center(e) else {
        return target_angle;
    };
    let half = config::AT_GUN_FIELD_OF_FIRE_RAD * 0.5;
    let delta = angle_delta(center, target_angle);
    if delta.abs() <= half {
        target_angle
    } else {
        center + delta.signum() * half
    }
}

fn at_gun_target_inside_field_of_fire(e: &Entity, target_angle: f32) -> bool {
    if !matches!(e.weapon_setup(), WeaponSetup::Deployed) {
        return true;
    }
    let Some(center) = at_gun_field_center(e) else {
        return true;
    };
    angle_delta(center, target_angle).abs() <= config::AT_GUN_FIELD_OF_FIRE_RAD * 0.5
}

fn at_gun_field_center(e: &Entity) -> Option<f32> {
    e.emplacement_facing()
        .or_else(|| e.weapon_facing())
        .filter(|facing| facing.is_finite())
}
