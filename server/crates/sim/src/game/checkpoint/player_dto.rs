use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
}
