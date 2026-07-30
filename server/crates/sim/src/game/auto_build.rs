use serde::{Deserialize, Serialize};

use super::PlayerState;
use crate::rules::economy::ResourceCost;

pub(super) const RESERVE_MAX: u32 = 9_950;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AutoBuildSettings {
    pub(crate) paused: bool,
    pub(crate) reserve_steel: u32,
    pub(crate) reserve_oil: u32,
}

impl Default for AutoBuildSettings {
    fn default() -> Self {
        Self {
            paused: false,
            reserve_steel: 200,
            reserve_oil: 100,
        }
    }
}

impl PlayerState {
    pub(in crate::game) fn can_auto_build(&self, cost: ResourceCost) -> bool {
        self.is_ai
            || (!self.auto_build.paused
                && self.steel.saturating_sub(cost.steel) >= self.auto_build.reserve_steel
                && self.oil.saturating_sub(cost.oil) >= self.auto_build.reserve_oil)
    }
}
