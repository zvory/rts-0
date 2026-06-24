use serde::{Deserialize, Serialize};

use crate::game::entity::EntityKind;

use super::orientation::{
    lab_setup_capable, lab_weapon_facing_capable, validate_optional_lab_angle,
};
use super::LabError;

pub(super) const LAB_SCENARIO_V1_SCHEMA_VERSION: u32 = 1;
pub(super) const LAB_SCENARIO_KIND: &str = "labScenario";
pub(super) const MAX_LAB_SCENARIO_NAME_LEN: usize = 80;
pub(super) const MAX_LAB_SCENARIO_PLAYERS: usize = 8;
pub(super) const MAX_LAB_SCENARIO_ENTITIES: usize = 2000;
pub(super) const MAX_LAB_SCENARIO_UPGRADES_PER_PLAYER: usize = 32;

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
    pub facing: Option<f32>,
    #[serde(default)]
    pub weapon_facing: Option<f32>,
    #[serde(default)]
    pub set_up: bool,
    #[serde(default)]
    pub setup_facing: Option<f32>,
    #[serde(default)]
    pub setup_target: Option<LabScenarioPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPoint {
    pub x: f32,
    pub y: f32,
}

pub(super) fn validate_lab_scenario_shape(scenario: &LabScenarioV1) -> Result<(), LabError> {
    if scenario.schema_version != LAB_SCENARIO_V1_SCHEMA_VERSION {
        return Err(LabError::InvalidScenarioVersion {
            version: scenario.schema_version,
        });
    }
    if scenario.kind != LAB_SCENARIO_KIND {
        return Err(LabError::InvalidScenario {
            reason: "scenario kind must be labScenario".to_string(),
        });
    }
    if scenario.name.trim().is_empty() || scenario.name.len() > MAX_LAB_SCENARIO_NAME_LEN {
        return Err(LabError::InvalidScenario {
            reason: "scenario name must be non-empty and at most 80 bytes".to_string(),
        });
    }
    if scenario.players.is_empty() {
        return Err(LabError::InvalidScenario {
            reason: "scenario must contain at least one player".to_string(),
        });
    }
    if scenario.players.len() > MAX_LAB_SCENARIO_PLAYERS {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "scenario has too many players: {} > {}",
                scenario.players.len(),
                MAX_LAB_SCENARIO_PLAYERS
            ),
        });
    }
    if scenario.entities.len() > MAX_LAB_SCENARIO_ENTITIES {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "scenario has too many entities: {} > {}",
                scenario.entities.len(),
                MAX_LAB_SCENARIO_ENTITIES
            ),
        });
    }
    Ok(())
}

pub(super) fn validate_lab_entity_setup_shape(
    entity: &LabScenarioEntity,
    kind: EntityKind,
) -> Result<(), LabError> {
    validate_optional_lab_angle(entity, "facing", entity.facing)?;
    validate_optional_lab_angle(entity, "weaponFacing", entity.weapon_facing)?;
    validate_optional_lab_angle(entity, "setupFacing", entity.setup_facing)?;

    if entity.facing.is_some() && !kind.is_unit() {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} kind {} cannot have facing",
                entity.id, entity.kind
            ),
        });
    }
    if entity.weapon_facing.is_some() && !lab_weapon_facing_capable(kind) {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} kind {} cannot have weaponFacing",
                entity.id, entity.kind
            ),
        });
    }
    if entity.setup_target.is_some() && !entity.set_up {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} has setupTarget without setUp", entity.id),
        });
    }
    if entity.setup_facing.is_some() && !entity.set_up {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} has setupFacing without setUp", entity.id),
        });
    }
    if (entity.set_up || entity.setup_target.is_some() || entity.setup_facing.is_some())
        && !lab_setup_capable(kind)
    {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} kind {} cannot be set up", entity.id, entity.kind),
        });
    }
    if entity.set_up && entity.setup_facing.is_none() && entity.setup_target.is_none() {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} has setUp without setupFacing or setupTarget",
                entity.id
            ),
        });
    }
    Ok(())
}
