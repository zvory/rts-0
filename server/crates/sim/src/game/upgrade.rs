use crate::game::entity::EntityKind;
pub use crate::rules::faction::UpgradeKind;

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
pub const ALL: &[UpgradeKind] = UpgradeKind::ALL;

pub fn researchable_upgrades(building: EntityKind) -> Vec<UpgradeKind> {
    crate::rules::faction::CURRENT_CATALOG
        .researchable_upgrade_kinds(building)
        .collect()
}

pub fn definition(kind: UpgradeKind) -> UpgradeDefinition {
    let catalog = crate::rules::faction::upgrade_definition(kind);
    match kind {
        UpgradeKind::Methamphetamines => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::METHAMPHETAMINES_COST_STEEL,
            cost_oil: crate::config::METHAMPHETAMINES_COST_OIL,
            research_ticks: crate::config::METHAMPHETAMINES_RESEARCH_TICKS,
        },
        UpgradeKind::Panzerfausts => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::PANZERFAUSTS_COST_STEEL,
            cost_oil: crate::config::PANZERFAUSTS_COST_OIL,
            research_ticks: crate::config::PANZERFAUSTS_RESEARCH_TICKS,
        },
        UpgradeKind::Entrenchment => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::ENTRENCHMENT_COST_STEEL,
            cost_oil: crate::config::ENTRENCHMENT_COST_OIL,
            research_ticks: crate::config::ENTRENCHMENT_RESEARCH_TICKS,
        },
        UpgradeKind::AntiTankGunUnlock => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::ANTI_TANK_GUN_UNLOCK_COST_STEEL,
            cost_oil: crate::config::ANTI_TANK_GUN_UNLOCK_COST_OIL,
            research_ticks: crate::config::ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::ArtilleryUnlock => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: Some(UpgradeKind::AntiTankGunUnlock),
            cost_steel: crate::config::ARTILLERY_UNLOCK_COST_STEEL,
            cost_oil: crate::config::ARTILLERY_UNLOCK_COST_OIL,
            research_ticks: crate::config::ARTILLERY_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::BallisticTables => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: Some(UpgradeKind::ArtilleryUnlock),
            cost_steel: crate::config::BALLISTIC_TABLES_COST_STEEL,
            cost_oil: crate::config::BALLISTIC_TABLES_COST_OIL,
            research_ticks: crate::config::BALLISTIC_TABLES_RESEARCH_TICKS,
        },
        UpgradeKind::TankUnlock => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::TANK_UNLOCK_COST_STEEL,
            cost_oil: crate::config::TANK_UNLOCK_COST_OIL,
            research_ticks: crate::config::TANK_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::MortarAutocast => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::MORTAR_AUTOCAST_COST_STEEL,
            cost_oil: crate::config::MORTAR_AUTOCAST_COST_OIL,
            research_ticks: crate::config::MORTAR_AUTOCAST_RESEARCH_TICKS,
        },
        UpgradeKind::SmokePlus => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::SMOKE_PLUS_COST_STEEL,
            cost_oil: crate::config::SMOKE_PLUS_COST_OIL,
            research_ticks: crate::config::SMOKE_PLUS_RESEARCH_TICKS,
        },
    }
}

pub fn required_for_unit(unit: EntityKind) -> Option<UpgradeKind> {
    match unit {
        EntityKind::Panzerfaust => Some(UpgradeKind::Panzerfausts),
        EntityKind::AntiTankGun => Some(UpgradeKind::AntiTankGunUnlock),
        EntityKind::Artillery => Some(UpgradeKind::ArtilleryUnlock),
        EntityKind::Tank => Some(UpgradeKind::TankUnlock),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn researchable_upgrades_match_current_building_catalogs() {
        assert_eq!(
            researchable_upgrades(EntityKind::TrainingCentre),
            vec![
                UpgradeKind::Methamphetamines,
                UpgradeKind::Panzerfausts,
                UpgradeKind::Entrenchment
            ]
        );
        assert_eq!(
            researchable_upgrades(EntityKind::ResearchComplex),
            vec![
                UpgradeKind::AntiTankGunUnlock,
                UpgradeKind::ArtilleryUnlock,
                UpgradeKind::BallisticTables,
                UpgradeKind::TankUnlock,
                UpgradeKind::MortarAutocast,
                UpgradeKind::SmokePlus,
            ]
        );
        assert!(ALL.contains(&UpgradeKind::ArtilleryUnlock));
        assert!(researchable_upgrades(EntityKind::ResearchComplex)
            .contains(&UpgradeKind::ArtilleryUnlock));
        assert!(researchable_upgrades(EntityKind::ResearchComplex)
            .contains(&UpgradeKind::BallisticTables));
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
        assert_eq!(definition.research_ticks, crate::config::TICK_HZ * 20);
    }
}
