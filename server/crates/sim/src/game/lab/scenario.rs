use serde::{Deserialize, Serialize};

use crate::config;
use crate::game::entity::{Entity, EntityKind, WeaponSetup};
use crate::game::map::Map;

use super::{validate_world_position, LabError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioV1 {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub seed: u32,
    pub map: LabScenarioMap,
    pub players: Vec<LabScenarioPlayer>,
    pub entities: Vec<LabScenarioEntity>,
    pub metadata: LabScenarioMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioMetadata {
    pub exported_tick: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioMap {
    pub name: String,
    pub schema_version: u32,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPlayer {
    pub id: u32,
    pub team_id: u32,
    pub faction_id: String,
    pub name: String,
    pub color: String,
    pub is_ai: bool,
    pub resources: LabScenarioResources,
    pub research: LabScenarioResearch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioResources {
    pub steel: u32,
    pub oil: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioResearch {
    pub completed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioEntity {
    pub id: u32,
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub completed: bool,
    pub construction_progress: Option<u32>,
    pub construction_total: Option<u32>,
    pub resource_remaining: Option<u32>,
    #[serde(default)]
    pub set_up: bool,
    pub setup_target: Option<LabScenarioPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPoint {
    pub x: f32,
    pub y: f32,
}

pub(super) fn lab_entity_is_set_up(entity: &Entity) -> bool {
    lab_setup_capable(entity.kind) && matches!(entity.weapon_setup(), WeaponSetup::Deployed)
}

pub(super) fn lab_entity_setup_target(map: &Map, entity: &Entity) -> Option<LabScenarioPoint> {
    if !lab_entity_is_set_up(entity) {
        return None;
    }
    let facing = entity
        .emplacement_facing()
        .or_else(|| entity.weapon_facing())
        .filter(|facing| facing.is_finite())?;
    Some(point_from_setup_facing(map, entity.pos_x, entity.pos_y, facing))
}

fn point_from_setup_facing(map: &Map, x: f32, y: f32, facing: f32) -> LabScenarioPoint {
    let distance = config::TILE_SIZE as f32 * 4.0;
    let world_max = (map.world_size_px() - 1.0).max(0.0);
    LabScenarioPoint {
        x: (x + facing.cos() * distance).clamp(0.0, world_max),
        y: (y + facing.sin() * distance).clamp(0.0, world_max),
    }
}

pub(super) fn validate_lab_entity_setup_shape(
    entity: &LabScenarioEntity,
    kind: EntityKind,
) -> Result<(), LabError> {
    if entity.setup_target.is_some() && !entity.set_up {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} has setupTarget without setUp", entity.id),
        });
    }
    if (entity.set_up || entity.setup_target.is_some()) && !lab_setup_capable(kind) {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} kind {} cannot be set up", entity.id, entity.kind),
        });
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
    let target = entity
        .setup_target
        .as_ref()
        .ok_or_else(|| LabError::InvalidScenario {
            reason: format!("entity {} has setUp without setupTarget", entity.id),
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

    restored.set_weapon_setup(WeaponSetup::Deployed);
    restored.set_weapon_facing(facing);
    restored.set_desired_weapon_facing(facing);
    if uses_fixed_setup_facing(restored.kind) {
        restored.set_emplacement_facing(Some(facing));
    }
    Ok(())
}

fn lab_setup_capable(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::MachineGunner
            | EntityKind::AntiTankGun
            | EntityKind::MortarTeam
            | EntityKind::Artillery
    )
}

fn uses_fixed_setup_facing(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::AntiTankGun | EntityKind::Artillery)
}

pub(super) fn normalize_lab_angle(angle: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    (angle + std::f32::consts::PI).rem_euclid(two_pi) - std::f32::consts::PI
}
