use std::str::FromStr;

use crate::game::entity::EntityKind;

const METHAMPHETAMINES: &str = "methamphetamines";
const AT_GUN_UNLOCK: &str = "at_gun_unlock";
const ARTILLERY_UNLOCK: &str = "artillery_unlock";
const TANK_UNLOCK: &str = "tank_unlock";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UpgradeKind {
    Methamphetamines,
    AtGunUnlock,
    ArtilleryUnlock,
    TankUnlock,
}

impl UpgradeKind {
    pub fn to_protocol_str(self) -> &'static str {
        match self {
            UpgradeKind::Methamphetamines => METHAMPHETAMINES,
            UpgradeKind::AtGunUnlock => AT_GUN_UNLOCK,
            UpgradeKind::ArtilleryUnlock => ARTILLERY_UNLOCK,
            UpgradeKind::TankUnlock => TANK_UNLOCK,
        }
    }
}

impl FromStr for UpgradeKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            METHAMPHETAMINES => Ok(UpgradeKind::Methamphetamines),
            AT_GUN_UNLOCK => Ok(UpgradeKind::AtGunUnlock),
            ARTILLERY_UNLOCK => Ok(UpgradeKind::ArtilleryUnlock),
            TANK_UNLOCK => Ok(UpgradeKind::TankUnlock),
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

pub const ALL: &[UpgradeKind] = &[
    UpgradeKind::Methamphetamines,
    UpgradeKind::AtGunUnlock,
    UpgradeKind::ArtilleryUnlock,
    UpgradeKind::TankUnlock,
];

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
        UpgradeKind::AtGunUnlock => UpgradeDefinition {
            kind,
            researched_at: EntityKind::Steelworks,
            cost_steel: crate::config::AT_GUN_UNLOCK_COST_STEEL,
            cost_oil: crate::config::AT_GUN_UNLOCK_COST_OIL,
            research_ticks: crate::config::AT_GUN_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::ArtilleryUnlock => UpgradeDefinition {
            kind,
            researched_at: EntityKind::Steelworks,
            cost_steel: crate::config::ARTILLERY_UNLOCK_COST_STEEL,
            cost_oil: crate::config::ARTILLERY_UNLOCK_COST_OIL,
            research_ticks: crate::config::ARTILLERY_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::TankUnlock => UpgradeDefinition {
            kind,
            researched_at: EntityKind::Factory,
            cost_steel: crate::config::TANK_UNLOCK_COST_STEEL,
            cost_oil: crate::config::TANK_UNLOCK_COST_OIL,
            research_ticks: crate::config::TANK_UNLOCK_RESEARCH_TICKS,
        },
    }
}

pub fn required_for_unit(unit: EntityKind) -> Option<UpgradeKind> {
    match unit {
        EntityKind::AtTeam => Some(UpgradeKind::AtGunUnlock),
        EntityKind::Artillery => Some(UpgradeKind::ArtilleryUnlock),
        EntityKind::Tank => Some(UpgradeKind::TankUnlock),
        _ => None,
    }
}
