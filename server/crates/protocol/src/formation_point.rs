use serde::{Deserialize, Serialize};

pub const MAX_FORMATION_POINTS: usize = 64;

pub(super) fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormationPoint {
    pub x: f32,
    pub y: f32,
}
