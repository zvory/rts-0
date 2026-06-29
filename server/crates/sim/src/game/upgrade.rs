use std::str::FromStr;

use crate::game::entity::EntityKind;

const METHAMPHETAMINES: &str = "methamphetamines";
const ANTI_TANK_GUN_UNLOCK: &str = "anti_tank_gun_unlock";
const ARTILLERY_UNLOCK: &str = "artillery_unlock";
const BALLISTIC_TABLES: &str = "ballistic_tables";
const TANK_UNLOCK: &str = "tank_unlock";
const COMMAND_CAR_UNLOCK: &str = "command_car_unlock";
const MORTAR_AUTOCAST: &str = "mortar_autocast";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UpgradeKind {
    Methamphetamines,
    AntiTankGunUnlock,
    ArtilleryUnlock,
    BallisticTables,
    TankUnlock,
    CommandCarUnlock,
    MortarAutocast,
}

impl UpgradeKind {
    pub fn to_protocol_str(self) -> &'static str {
        match self {
            UpgradeKind::Methamphetamines => METHAMPHETAMINES,
            UpgradeKind::AntiTankGunUnlock => ANTI_TANK_GUN_UNLOCK,
            UpgradeKind::ArtilleryUnlock => ARTILLERY_UNLOCK,
            UpgradeKind::BallisticTables => BALLISTIC_TABLES,
            UpgradeKind::TankUnlock => TANK_UNLOCK,
            UpgradeKind::CommandCarUnlock => COMMAND_CAR_UNLOCK,
            UpgradeKind::MortarAutocast => MORTAR_AUTOCAST,
        }
    }
}

impl FromStr for UpgradeKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            METHAMPHETAMINES => Ok(UpgradeKind::Methamphetamines),
            ANTI_TANK_GUN_UNLOCK => Ok(UpgradeKind::AntiTankGunUnlock),
            ARTILLERY_UNLOCK => Ok(UpgradeKind::ArtilleryUnlock),
            BALLISTIC_TABLES => Ok(UpgradeKind::BallisticTables),
            TANK_UNLOCK => Ok(UpgradeKind::TankUnlock),
            COMMAND_CAR_UNLOCK => Ok(UpgradeKind::CommandCarUnlock),
            MORTAR_AUTOCAST => Ok(UpgradeKind::MortarAutocast),
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

pub const ALL: &[UpgradeKind] = &[
    UpgradeKind::Methamphetamines,
    UpgradeKind::AntiTankGunUnlock,
    UpgradeKind::ArtilleryUnlock,
    UpgradeKind::BallisticTables,
    UpgradeKind::TankUnlock,
    UpgradeKind::CommandCarUnlock,
    UpgradeKind::MortarAutocast,
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
            requires_upgrade: None,
            cost_steel: crate::config::METHAMPHETAMINES_COST_STEEL,
            cost_oil: crate::config::METHAMPHETAMINES_COST_OIL,
            research_ticks: crate::config::METHAMPHETAMINES_RESEARCH_TICKS,
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
