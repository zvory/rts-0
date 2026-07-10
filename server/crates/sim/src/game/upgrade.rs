use std::str::FromStr;

use crate::game::entity::EntityKind;
use serde::{Deserialize, Serialize};

const METHAMPHETAMINES: &str = "methamphetamines";
const ENTRENCHMENT: &str = "entrenchment";
const ANTI_TANK_GUN_UNLOCK: &str = "anti_tank_gun_unlock";
const ARTILLERY_UNLOCK: &str = "artillery_unlock";
const BALLISTIC_TABLES: &str = "ballistic_tables";
const TANK_UNLOCK: &str = "tank_unlock";
const COMMAND_CAR_UNLOCK: &str = "command_car_unlock";
const MORTAR_AUTOCAST: &str = "mortar_autocast";
const SMOKE_PLUS: &str = "smoke_plus";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum UpgradeKind {
    Methamphetamines,
    Entrenchment,
    AntiTankGunUnlock,
    ArtilleryUnlock,
    BallisticTables,
    TankUnlock,
    CommandCarUnlock,
    MortarAutocast,
    SmokePlus,
}

impl UpgradeKind {
    pub fn to_protocol_str(self) -> &'static str {
        match self {
            UpgradeKind::Methamphetamines => METHAMPHETAMINES,
            UpgradeKind::Entrenchment => ENTRENCHMENT,
            UpgradeKind::AntiTankGunUnlock => ANTI_TANK_GUN_UNLOCK,
            UpgradeKind::ArtilleryUnlock => ARTILLERY_UNLOCK,
            UpgradeKind::BallisticTables => BALLISTIC_TABLES,
            UpgradeKind::TankUnlock => TANK_UNLOCK,
            UpgradeKind::CommandCarUnlock => COMMAND_CAR_UNLOCK,
            UpgradeKind::MortarAutocast => MORTAR_AUTOCAST,
            UpgradeKind::SmokePlus => SMOKE_PLUS,
        }
    }
}

impl FromStr for UpgradeKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            METHAMPHETAMINES => Ok(UpgradeKind::Methamphetamines),
            ENTRENCHMENT => Ok(UpgradeKind::Entrenchment),
            ANTI_TANK_GUN_UNLOCK => Ok(UpgradeKind::AntiTankGunUnlock),
            ARTILLERY_UNLOCK => Ok(UpgradeKind::ArtilleryUnlock),
            BALLISTIC_TABLES => Ok(UpgradeKind::BallisticTables),
            TANK_UNLOCK => Ok(UpgradeKind::TankUnlock),
            COMMAND_CAR_UNLOCK => Ok(UpgradeKind::CommandCarUnlock),
            MORTAR_AUTOCAST => Ok(UpgradeKind::MortarAutocast),
            SMOKE_PLUS => Ok(UpgradeKind::SmokePlus),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeDefinition {
    pub kind: UpgradeKind,
    pub researched_at: EntityKind,
    pub requires_upgrade: Option<UpgradeKind>,
    pub cost_steel: u32,
    pub cost_oil: u32,
    pub research_ticks: u32,
}

/// All upgrade ids the simulation can decode from protocol or replay data.
pub const ALL: &[UpgradeKind] = &[
    UpgradeKind::Methamphetamines,
    UpgradeKind::Entrenchment,
    UpgradeKind::AntiTankGunUnlock,
    UpgradeKind::ArtilleryUnlock,
    UpgradeKind::BallisticTables,
    UpgradeKind::TankUnlock,
    UpgradeKind::CommandCarUnlock,
    UpgradeKind::MortarAutocast,
    UpgradeKind::SmokePlus,
];

const CURRENT_RESEARCHABLE: &[UpgradeKind] = &[
    UpgradeKind::Methamphetamines,
    UpgradeKind::Entrenchment,
    UpgradeKind::AntiTankGunUnlock,
    UpgradeKind::BallisticTables,
    UpgradeKind::TankUnlock,
    UpgradeKind::CommandCarUnlock,
    UpgradeKind::MortarAutocast,
    UpgradeKind::SmokePlus,
    UpgradeKind::ArtilleryUnlock,
];

pub fn researchable_upgrades(building: EntityKind) -> Vec<UpgradeKind> {
    CURRENT_RESEARCHABLE
        .iter()
        .copied()
        .filter(|upgrade| definition(*upgrade).researched_at == building)
        .collect()
}

pub fn definition(kind: UpgradeKind) -> UpgradeDefinition {
    match kind {
        UpgradeKind::Methamphetamines => UpgradeDefinition {
            kind,
            researched_at: EntityKind::TrainingCentre,
            requires_upgrade: None,
            cost_steel: crate::config::METHAMPHETAMINES_COST_STEEL,
            cost_oil: crate::config::METHAMPHETAMINES_COST_OIL,
            research_ticks: crate::config::METHAMPHETAMINES_RESEARCH_TICKS,
        },
        UpgradeKind::Entrenchment => UpgradeDefinition {
            kind,
            researched_at: EntityKind::TrainingCentre,
            requires_upgrade: None,
            cost_steel: crate::config::ENTRENCHMENT_COST_STEEL,
            cost_oil: crate::config::ENTRENCHMENT_COST_OIL,
            research_ticks: crate::config::ENTRENCHMENT_RESEARCH_TICKS,
        },
        UpgradeKind::AntiTankGunUnlock => UpgradeDefinition {
            kind,
            researched_at: EntityKind::ResearchComplex,
            requires_upgrade: None,
            cost_steel: crate::config::ANTI_TANK_GUN_UNLOCK_COST_STEEL,
            cost_oil: crate::config::ANTI_TANK_GUN_UNLOCK_COST_OIL,
            research_ticks: crate::config::ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::ArtilleryUnlock => UpgradeDefinition {
            kind,
            researched_at: EntityKind::ResearchComplex,
            requires_upgrade: Some(UpgradeKind::AntiTankGunUnlock),
            cost_steel: crate::config::ARTILLERY_UNLOCK_COST_STEEL,
            cost_oil: crate::config::ARTILLERY_UNLOCK_COST_OIL,
            research_ticks: crate::config::ARTILLERY_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::BallisticTables => UpgradeDefinition {
            kind,
            researched_at: EntityKind::ResearchComplex,
            requires_upgrade: Some(UpgradeKind::ArtilleryUnlock),
            cost_steel: crate::config::BALLISTIC_TABLES_COST_STEEL,
            cost_oil: crate::config::BALLISTIC_TABLES_COST_OIL,
            research_ticks: crate::config::BALLISTIC_TABLES_RESEARCH_TICKS,
        },
        UpgradeKind::TankUnlock => UpgradeDefinition {
            kind,
            researched_at: EntityKind::ResearchComplex,
            requires_upgrade: None,
            cost_steel: crate::config::TANK_UNLOCK_COST_STEEL,
            cost_oil: crate::config::TANK_UNLOCK_COST_OIL,
            research_ticks: crate::config::TANK_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::CommandCarUnlock => UpgradeDefinition {
            kind,
            researched_at: EntityKind::ResearchComplex,
            requires_upgrade: Some(UpgradeKind::TankUnlock),
            cost_steel: crate::config::COMMAND_CAR_UNLOCK_COST_STEEL,
            cost_oil: crate::config::COMMAND_CAR_UNLOCK_COST_OIL,
            research_ticks: crate::config::COMMAND_CAR_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::MortarAutocast => UpgradeDefinition {
            kind,
            researched_at: EntityKind::ResearchComplex,
            requires_upgrade: None,
            cost_steel: crate::config::MORTAR_AUTOCAST_COST_STEEL,
            cost_oil: crate::config::MORTAR_AUTOCAST_COST_OIL,
            research_ticks: crate::config::MORTAR_AUTOCAST_RESEARCH_TICKS,
        },
        UpgradeKind::SmokePlus => UpgradeDefinition {
            kind,
            researched_at: EntityKind::ResearchComplex,
            requires_upgrade: None,
            cost_steel: crate::config::SMOKE_PLUS_COST_STEEL,
            cost_oil: crate::config::SMOKE_PLUS_COST_OIL,
            research_ticks: crate::config::SMOKE_PLUS_RESEARCH_TICKS,
        },
    }
}

pub fn required_for_unit(unit: EntityKind) -> Option<UpgradeKind> {
    match unit {
        EntityKind::AntiTankGun => Some(UpgradeKind::AntiTankGunUnlock),
        EntityKind::Artillery => Some(UpgradeKind::ArtilleryUnlock),
        EntityKind::Tank => Some(UpgradeKind::TankUnlock),
        EntityKind::CommandCar => Some(UpgradeKind::CommandCarUnlock),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn researchable_upgrades_include_current_gun_unlocks() {
        assert_eq!(
            researchable_upgrades(EntityKind::TrainingCentre),
            vec![UpgradeKind::Methamphetamines, UpgradeKind::Entrenchment]
        );
        assert_eq!(
            researchable_upgrades(EntityKind::ResearchComplex),
            vec![
                UpgradeKind::AntiTankGunUnlock,
                UpgradeKind::BallisticTables,
                UpgradeKind::TankUnlock,
                UpgradeKind::CommandCarUnlock,
                UpgradeKind::MortarAutocast,
                UpgradeKind::SmokePlus,
                UpgradeKind::ArtilleryUnlock,
            ]
        );
        assert!(ALL.contains(&UpgradeKind::ArtilleryUnlock));
        assert!(researchable_upgrades(EntityKind::ResearchComplex)
            .contains(&UpgradeKind::ArtilleryUnlock));
    }

    #[test]
    fn entrenchment_definition_matches_training_centre_research_contract() {
        assert_eq!(
            "entrenchment".parse::<UpgradeKind>(),
            Ok(UpgradeKind::Entrenchment)
        );
        assert_eq!(UpgradeKind::Entrenchment.to_protocol_str(), "entrenchment");

        let definition = definition(UpgradeKind::Entrenchment);
        assert_eq!(definition.researched_at, EntityKind::TrainingCentre);
        assert_eq!(definition.requires_upgrade, None);
        assert_eq!(definition.cost_steel, 100);
        assert_eq!(definition.cost_oil, 0);
        assert_eq!(definition.research_ticks, crate::config::TICK_HZ * 30);
    }
}
