use std::collections::{BTreeMap, BTreeSet};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::CheckpointPayloadError;

pub(super) fn serde_convert<T, U>(value: T) -> Result<U, CheckpointPayloadError>
where
    T: Serialize,
    U: DeserializeOwned,
{
    serde_json::from_value(
        serde_json::to_value(value)
            .map_err(|err| CheckpointPayloadError::MalformedJson(err.to_string()))?,
    )
    .map_err(|err| CheckpointPayloadError::MalformedJson(err.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PlayerStateV1 {
    pub(super) id: u32,
    pub(super) team_id: u32,
    pub(super) faction_id: String,
    pub(super) name: String,
    pub(super) color: String,
    pub(super) start_tile: (u32, u32),
    pub(super) steel: u32,
    pub(super) oil: u32,
    pub(super) supply_used: u32,
    pub(super) supply_cap: u32,
    pub(super) is_ai: bool,
    pub(super) score: ScoreStateV1,
    pub(super) upgrades: BTreeSet<super::super::upgrade::UpgradeKind>,
    #[serde(default)]
    pub(super) ability_cooldowns: BTreeMap<super::super::ability::AbilityKind, u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ScoreStateV1 {
    unit_score: u32,
    structure_score: u32,
    units_killed: u32,
    units_lost: u32,
    buildings_killed: u32,
    buildings_lost: u32,
    units_lost_by_kind: BTreeMap<super::super::entity::EntityKind, u32>,
    #[serde(default)]
    resources_mined: super::super::ResourceTotals,
    #[serde(default)]
    resource_income_history: Vec<super::super::ResourceIncomeRecord>,
}

impl PlayerStateV1 {
    pub(super) fn resource_income_history_len(&self) -> usize {
        self.score.resource_income_history.len()
    }

    pub(super) fn resource_income_history_ticks(&self) -> impl Iterator<Item = u32> + '_ {
        self.score
            .resource_income_history
            .iter()
            .map(|record| record.tick)
    }
}
