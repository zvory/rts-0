use std::str::FromStr;

use crate::game::entity::EntityKind;

pub const METHAMPHETAMINES: &str = "methamphetamines";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UpgradeKind {
    Methamphetamines,
}

impl UpgradeKind {
    pub fn to_protocol_str(self) -> &'static str {
        match self {
            UpgradeKind::Methamphetamines => METHAMPHETAMINES,
        }
    }
}

impl FromStr for UpgradeKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            METHAMPHETAMINES => Ok(UpgradeKind::Methamphetamines),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeDefinition {
    pub kind: UpgradeKind,
    pub researched_at: EntityKind,
    pub cost_steel: u32,
    pub cost_oil: u32,
    pub research_ticks: u32,
}

pub const ALL: &[UpgradeKind] = &[UpgradeKind::Methamphetamines];

pub fn researchable_upgrades(building: EntityKind) -> Vec<UpgradeKind> {
    ALL.iter()
        .copied()
        .filter(|upgrade| definition(*upgrade).researched_at == building)
        .collect()
}

pub fn definition(kind: UpgradeKind) -> UpgradeDefinition {
    match kind {
        UpgradeKind::Methamphetamines => UpgradeDefinition {
            kind,
            researched_at: EntityKind::TrainingCentre,
            cost_steel: crate::config::METHAMPHETAMINES_COST_STEEL,
            cost_oil: crate::config::METHAMPHETAMINES_COST_OIL,
            research_ticks: crate::config::METHAMPHETAMINES_RESEARCH_TICKS,
        },
    }
}
