use crate::config;
use crate::game::entity::{Entity, EntityKind, WeaponSetup};
use crate::game::map::Map;

use super::scenario::{LabScenarioEntity, LabScenarioPoint};
use super::{validate_world_position, LabError};

pub(super) fn lab_entity_is_set_up(entity: &Entity) -> bool {
    lab_setup_capable(entity.kind) && matches!(entity.weapon_setup(), WeaponSetup::Deployed)
}

pub(super) fn lab_entity_facing(entity: &Entity) -> Option<f32> {
    entity
        .kind
        .is_unit()
        .then(|| finite_normalized_angle(entity.facing()))
        .flatten()
}

pub(super) fn lab_entity_weapon_facing(entity: &Entity) -> Option<f32> {
    entity.weapon_facing().and_then(finite_normalized_angle)
}

pub(super) fn lab_entity_setup_facing(entity: &Entity) -> Option<f32> {
    if !lab_entity_is_set_up(entity) {
        return None;
    }
    entity
        .emplacement_facing()
        .or_else(|| entity.weapon_facing())
        .and_then(finite_normalized_angle)
}

pub(super) fn lab_entity_setup_target(map: &Map, entity: &Entity) -> Option<LabScenarioPoint> {
    let facing = lab_entity_setup_facing(entity)?;
    Some(point_from_setup_facing(
        map,
        entity.pos_x,
        entity.pos_y,
        facing,
    ))
}

fn point_from_setup_facing(map: &Map, x: f32, y: f32, facing: f32) -> LabScenarioPoint {
    let distance = config::TILE_SIZE as f32 * 4.0;
    let world_max = (map.world_size_px() - 1.0).max(0.0);
    LabScenarioPoint {
        x: (x + facing.cos() * distance).clamp(0.0, world_max),
        y: (y + facing.sin() * distance).clamp(0.0, world_max),
    }
}

pub(super) fn restore_lab_entity_orientation(
    entity: &LabScenarioEntity,
    restored: &mut Entity,
) -> Result<(), LabError> {
    if let Some(facing) = entity.facing {
        restored.set_facing(validate_lab_angle(entity, "facing", facing)?);
    }
    if let Some(weapon_facing) = entity.weapon_facing {
        let facing = validate_lab_angle(entity, "weaponFacing", weapon_facing)?;
        restored.set_weapon_facing(facing);
        restored.set_desired_weapon_facing(facing);
    }
    Ok(())
}

pub(super) fn restore_lab_entity_setup(
    map: &Map,
    entity: &LabScenarioEntity,
    restored: &mut Entity,
) -> Result<(), LabError> {
    if !entity.set_up {
        return Ok(());
    }
    let setup_facing = lab_entity_setup_facing_from_scenario(map, entity, restored)?;

    restored.set_weapon_setup(WeaponSetup::Deployed);
    if uses_fixed_setup_facing(restored.kind) {
        restored.set_emplacement_facing(Some(setup_facing));
    }
    if entity.weapon_facing.is_none() {
        restored.set_weapon_facing(setup_facing);
        restored.set_desired_weapon_facing(setup_facing);
    }
    Ok(())
}

fn lab_entity_setup_facing_from_scenario(
    map: &Map,
    entity: &LabScenarioEntity,
    restored: &Entity,
) -> Result<f32, LabError> {
    if let Some(facing) = entity.setup_facing {
        return validate_lab_angle(entity, "setupFacing", facing);
    }
    let target = entity
        .setup_target
        .as_ref()
        .ok_or_else(|| LabError::InvalidScenario {
            reason: format!(
                "entity {} has setUp without setupFacing or setupTarget",
                entity.id
            ),
        })?;
    validate_world_position(map, target.x, target.y)?;
    let facing = normalize_lab_angle((target.y - restored.pos_y).atan2(target.x - restored.pos_x));
    if !facing.is_finite() {
        return Err(LabError::InvalidPosition {
            x: target.x,
            y: target.y,
            reason: "setup target must produce a finite facing",
        });
    }
    Ok(facing)
}

pub(super) fn lab_setup_capable(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::MachineGunner
            | EntityKind::AntiTankGun
            | EntityKind::MortarTeam
            | EntityKind::Artillery
    )
}

pub(super) fn lab_weapon_facing_capable(kind: EntityKind) -> bool {
    config::unit_stats(kind)
        .map(|stats| stats.dmg > 0 || kind == EntityKind::Artillery)
        .or_else(|| config::building_stats(kind).map(|stats| stats.dmg > 0))
        .unwrap_or(false)
}

fn uses_fixed_setup_facing(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::AntiTankGun | EntityKind::Artillery)
}

pub(super) fn validate_optional_lab_angle(
    entity: &LabScenarioEntity,
    field: &'static str,
    value: Option<f32>,
) -> Result<(), LabError> {
    if let Some(angle) = value {
        validate_lab_angle(entity, field, angle)?;
    }
    Ok(())
}

pub(super) fn validate_lab_angle(
    entity: &LabScenarioEntity,
    field: &'static str,
    angle: f32,
) -> Result<f32, LabError> {
    if !angle.is_finite() {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} has non-finite {field}", entity.id),
        });
    }
    Ok(normalize_lab_angle(angle))
}

fn finite_normalized_angle(angle: f32) -> Option<f32> {
    angle.is_finite().then(|| normalize_lab_angle(angle))
}

fn normalize_lab_angle(angle: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    (angle + std::f32::consts::PI).rem_euclid(two_pi) - std::f32::consts::PI
}
