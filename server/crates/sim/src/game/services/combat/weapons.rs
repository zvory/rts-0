use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore, MovePhase, Order, WeaponSetup};
use crate::game::entrenchment_combat;
use crate::game::services::movement::{angle_delta, rotate_toward};
use crate::rules::combat as combat_rules;

use super::priority::{self, AttackPriorityContext, TargetCandidate};
use super::projection::tank_effective_range_tiles;
use super::{
    ANTI_TANK_GUN_FIRE_TOLERANCE_RAD, ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK,
    TANK_TURRET_FIRE_TOLERANCE_RAD, TANK_TURRET_TURN_RATE_RAD_PER_TICK,
};

const SUPPORT_WEAPON_ATTACK_MOVE_NO_TARGET_TICKS: u16 = config::TICK_HZ as u16;
const TANK_STATIONARY_RANGE_RAMP_TICKS: u16 = config::TICK_HZ as u16 * 3;

pub(super) fn tick_tank_stationary_range(e: &mut Entity) {
    if e.kind != EntityKind::Tank || e.hp == 0 {
        return;
    }
    let Some(c) = e.combat.as_mut() else {
        return;
    };
    if c.tank_stationary_range_reset_this_tick {
        c.tank_stationary_range_reset_this_tick = false;
    } else {
        c.tank_stationary_range_ticks = c
            .tank_stationary_range_ticks
            .saturating_add(1)
            .min(TANK_STATIONARY_RANGE_RAMP_TICKS);
    }
}

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

pub(super) fn rotate_anti_tank_gun_for_combat(e: &mut Entity, target_angle: f32) -> bool {
    if !target_angle.is_finite() {
        return false;
    }
    let desired = deployed_anti_tank_gun_desired_facing(e, target_angle);
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
    let rotated = rotate_toward(current, desired, ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_facing(rotated);
        e.set_weapon_facing(rotated);
    } else {
        return false;
    }
    anti_tank_gun_target_inside_field_of_fire(e, target_angle)
        && angle_delta(rotated, target_angle).abs() <= ANTI_TANK_GUN_FIRE_TOLERANCE_RAD
}

pub(super) fn tick_deployed_weapon_setup(e: &mut Entity) {
    if !requires_weapon_setup(e.kind) {
        return;
    }
    rotate_anti_tank_gun_toward_setup_facing(e);
    maybe_begin_anti_tank_gun_setup_after_alignment(e);
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
        Order::Idle | Order::HoldPosition | Order::Attack(_) | Order::AttackMove(_)
    ) {
        return;
    }
    if matches!(e.order(), Order::AttackMove(_)) && e.move_phase() != Some(MovePhase::Arrived) {
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
    if !requires_weapon_setup(e.kind) || e.kind == EntityKind::AntiTankGun {
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
        WeaponSetup::SettingUp { .. }
        | WeaponSetup::TearingDown { .. }
        | WeaponSetup::TearingDownToRedeploy { .. } => false,
    }
}

pub(super) fn deployed_weapon_ready_to_move(entities: &mut EntityStore, id: u32) -> bool {
    entities
        .get_mut(id)
        .is_some_and(Entity::begin_weapon_teardown_for_movement)
}

pub(super) fn update_attack_move_no_target_teardown(entities: &mut EntityStore, id: u32) {
    let teardown_due = entities
        .get_mut(id)
        .map(|e| {
            if support_weapon_attack_move_waiting_without_target(e) {
                e.increment_attack_move_no_target_ticks()
                    >= SUPPORT_WEAPON_ATTACK_MOVE_NO_TARGET_TICKS
            } else {
                e.reset_attack_move_no_target_ticks();
                false
            }
        })
        .unwrap_or(false);
    if teardown_due {
        deployed_weapon_ready_to_move(entities, id);
    }
}

pub(super) fn setup_ticks_for(kind: EntityKind) -> u16 {
    match kind {
        EntityKind::AntiTankGun => config::ANTI_TANK_GUN_SETUP_TICKS,
        EntityKind::Artillery => config::ARTILLERY_SETUP_TICKS,
        EntityKind::MortarTeam => config::MORTAR_TEAM_SETUP_TICKS,
        _ => config::MACHINE_GUNNER_SETUP_TICKS,
    }
}

fn requires_weapon_setup(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::MachineGunner | EntityKind::AntiTankGun | EntityKind::Artillery
    )
}

pub(super) fn uses_stationary_weapon_aggro(e: &Entity) -> bool {
    e.kind == EntityKind::MachineGunner
        || (e.kind == EntityKind::AntiTankGun && !matches!(e.weapon_setup(), WeaponSetup::Packed))
        || (e.kind == EntityKind::Artillery && !matches!(e.weapon_setup(), WeaponSetup::Packed))
}

pub(super) fn can_fire_while_moving(e: &Entity, methamphetamines_researched: bool) -> bool {
    crate::game::entity::fires_while_moving(e.kind)
        || (e.kind == EntityKind::Rifleman && methamphetamines_researched)
}

pub(super) fn uses_vehicle_weapon_policy(e: &Entity) -> bool {
    crate::game::entity::fires_while_moving(e.kind)
}

pub(super) fn moving_fire_move_order_holds_path(e: &Entity, can_fire_while_moving: bool) -> bool {
    can_fire_while_moving
        && matches!(e.order(), Order::Move(_))
        && !matches!(e.move_phase(), Some(MovePhase::Arrived))
}

pub(super) fn moving_fire_miss_chance(_e: &Entity) -> f32 {
    0.0
}

fn support_weapon_attack_move_waiting_without_target(e: &Entity) -> bool {
    matches!(e.kind, EntityKind::MachineGunner | EntityKind::AntiTankGun)
        && matches!(e.order(), Order::AttackMove(_))
        && e.move_phase() != Some(MovePhase::Arrived)
        && matches!(
            e.weapon_setup(),
            WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
        )
}

pub(super) fn anti_tank_gun_can_chase(e: &Entity) -> bool {
    !matches!(e.kind, EntityKind::AntiTankGun | EntityKind::Artillery)
        || matches!(e.weapon_setup(), WeaponSetup::Packed)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct EffectiveAttackProfile {
    pub weapon: Option<&'static combat_rules::WeaponProfile>,
    pub range_tiles: f32,
    pub dmg: u32,
    pub cooldown: u32,
}

pub(super) fn effective_attack_profile(e: &Entity) -> EffectiveAttackProfile {
    let weapon = combat_rules::default_weapon_profile(e.kind);
    let base = combat_rules::attack_profile(e.kind);
    let mut profile = EffectiveAttackProfile {
        weapon,
        range_tiles: entrenchment_combat::attack_range_tiles(
            e,
            tank_effective_range_tiles(e, base.range_tiles as f32),
        ),
        dmg: base.dmg,
        cooldown: base.cooldown,
    };
    if e.kind != EntityKind::AntiTankGun {
        return profile;
    }
    match e.weapon_setup() {
        WeaponSetup::Packed => {
            profile.range_tiles = config::ANTI_TANK_GUN_PACKED_RANGE_TILES as f32;
            profile.dmg = ((profile.dmg as f32) * config::ANTI_TANK_GUN_PACKED_DAMAGE_MULTIPLIER)
                .round() as u32;
        }
        WeaponSetup::Deployed => {
            profile.range_tiles = config::ANTI_TANK_GUN_DEPLOYED_RANGE_TILES as f32
        }
        WeaponSetup::SettingUp { .. }
        | WeaponSetup::TearingDown { .. }
        | WeaponSetup::TearingDownToRedeploy { .. } => {
            profile.range_tiles = config::ANTI_TANK_GUN_PACKED_RANGE_TILES as f32;
            profile.dmg = 0;
        }
    }
    profile
}

fn deployed_anti_tank_gun_desired_facing(e: &Entity, target_angle: f32) -> f32 {
    if !matches!(e.weapon_setup(), WeaponSetup::Deployed) {
        return target_angle;
    }
    let Some(center) = anti_tank_gun_field_center(e) else {
        return target_angle;
    };
    let half = config::ANTI_TANK_GUN_FIELD_OF_FIRE_RAD * 0.5;
    let delta = angle_delta(center, target_angle);
    if delta.abs() <= half {
        target_angle
    } else {
        center + delta.signum() * half
    }
}

pub(super) fn anti_tank_gun_target_inside_field_of_fire(e: &Entity, target_angle: f32) -> bool {
    if !matches!(e.weapon_setup(), WeaponSetup::Deployed) {
        return true;
    }
    let Some(center) = anti_tank_gun_field_center(e) else {
        return true;
    };
    angle_delta(center, target_angle).abs() <= config::ANTI_TANK_GUN_FIELD_OF_FIRE_RAD * 0.5
}

pub(super) fn choose_target_preferring_anti_tank_field(
    context: &AttackPriorityContext,
    attacker: &Entity,
    px: f32,
    py: f32,
    candidates: &[TargetCandidate],
    filter: impl Fn(&TargetCandidate) -> bool,
) -> Option<u32> {
    if attacker.kind == EntityKind::AntiTankGun {
        let in_field = priority::choose_target(
            context,
            candidates.iter().filter(|candidate| {
                filter(candidate)
                    && anti_tank_gun_target_inside_field_of_fire(
                        attacker,
                        (candidate.pos_y - py).atan2(candidate.pos_x - px),
                    )
            }),
        );
        if in_field.is_some() {
            return in_field;
        }
    }
    priority::choose_target(
        context,
        candidates.iter().filter(|candidate| filter(candidate)),
    )
}

fn anti_tank_gun_field_center(e: &Entity) -> Option<f32> {
    e.emplacement_facing()
        .or_else(|| e.weapon_facing())
        .filter(|facing| facing.is_finite())
}

fn rotate_anti_tank_gun_toward_setup_facing(e: &mut Entity) {
    if !matches!(e.kind, EntityKind::AntiTankGun | EntityKind::Artillery) {
        return;
    }
    let target = match e.weapon_setup() {
        WeaponSetup::Packed => e.emplacement_facing(),
        WeaponSetup::SettingUp { .. } => e.emplacement_facing(),
        _ => None,
    };
    let Some(target) = target.filter(|facing| facing.is_finite()) else {
        return;
    };
    e.set_desired_weapon_facing(target);
    let current = e.facing();
    let rotated = rotate_toward(current, target, ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_facing(rotated);
        e.set_weapon_facing(rotated);
    }
}

fn maybe_begin_anti_tank_gun_setup_after_alignment(e: &mut Entity) {
    if !matches!(e.kind, EntityKind::AntiTankGun | EntityKind::Artillery)
        || !matches!(e.weapon_setup(), WeaponSetup::Packed)
    {
        return;
    }
    if !e.path_is_empty()
        || !matches!(
            e.order(),
            Order::Idle | Order::ArtilleryPointFire(_) | Order::ArtilleryBlanketFire(_)
        )
    {
        return;
    }
    let Some(target) = e.emplacement_facing().filter(|facing| facing.is_finite()) else {
        return;
    };
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or_else(|| e.facing());
    if angle_delta(current, target).abs() <= ANTI_TANK_GUN_FIRE_TOLERANCE_RAD {
        e.set_weapon_setup(WeaponSetup::SettingUp {
            ticks: setup_ticks_for(e.kind),
        });
    }
}
