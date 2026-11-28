use crate::game::entity::{EntityKind, EntityStore, ResearchItem};
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
    // Entity state must cover every faction; command admission remains faction-gated.
    let mut upgrades = Vec::new();
    for catalog in crate::rules::faction::CATALOGS {
        for kind in catalog.researchable_upgrade_kinds(building) {
            if !upgrades.contains(&kind) {
                upgrades.push(kind);
            }
        }
    }
    upgrades
}

/// Dao prices are selected when payment starts; paid queues reserve their tier.
fn is_dao(kind: UpgradeKind) -> bool {
    matches!(
        kind,
        UpgradeKind::DaoWind | UpgradeKind::DaoOfHeaviness | UpgradeKind::DaoOfMagic
    )
}

pub(crate) fn next_definition(
    kind: UpgradeKind,
    owner: u32,
    completed: &std::collections::BTreeSet<UpgradeKind>,
    entities: &EntityStore,
) -> UpgradeDefinition {
    if !is_dao(kind) {
        return definition(kind);
    }
    let count = completed.iter().filter(|kind| is_dao(**kind)).count()
        + entities
            .iter()
            .filter(|entity| entity.owner == owner)
            .flat_map(|entity| entity.research_queue())
            .filter(|item| item.paid && is_dao(item.upgrade))
            .count();
    tier_definition(kind, count)
}

fn tier_definition(kind: UpgradeKind, count: usize) -> UpgradeDefinition {
    let mut def = definition(kind);
    if is_dao(kind) {
        (def.cost_steel, def.cost_oil, def.research_ticks) =
            crate::config::dao_research_tier(count);
    }
    def
}

/// Paid Dao duration records the committed tier, even if another research is cancelled.
pub(crate) fn paid_cost(item: &ResearchItem) -> crate::rules::economy::ResourceCost {
    let count = match item.total {
        ticks if ticks == crate::config::DAO_RESEARCH_TICKS / 2 => 0,
        ticks if ticks == crate::config::DAO_RESEARCH_TICKS => 1,
        _ => 2,
    };
    let def = tier_definition(item.upgrade, count);
    crate::rules::economy::ResourceCost::new(def.cost_steel, def.cost_oil)
}

pub fn definition(kind: UpgradeKind) -> UpgradeDefinition {
    let catalog = crate::rules::faction::upgrade_definition(kind);
    match kind {
        UpgradeKind::TechniqueCharge
        | UpgradeKind::TechniqueBlink
        | UpgradeKind::TechniqueShield
        | UpgradeKind::TechniqueThrowSword => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::TECHNIQUE_COST_STEEL,
            cost_oil: crate::config::TECHNIQUE_COST_OIL,
            research_ticks: if kind == UpgradeKind::TechniqueBlink {
                crate::config::BLINK_RESEARCH_TICKS
            } else {
                crate::config::TECHNIQUE_RESEARCH_TICKS
            },
        },
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
            requires_upgrade: None,
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
        UpgradeKind::TankPlus => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: Some(UpgradeKind::TankUnlock),
            cost_steel: crate::config::TANK_PLUS_COST_STEEL,
            cost_oil: crate::config::TANK_PLUS_COST_OIL,
            research_ticks: crate::config::TANK_PLUS_RESEARCH_TICKS,
        },
        UpgradeKind::SmokePlus => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::SMOKE_PLUS_COST_STEEL,
            cost_oil: crate::config::SMOKE_PLUS_COST_OIL,
            research_ticks: crate::config::SMOKE_PLUS_RESEARCH_TICKS,
        },
        UpgradeKind::ScoutPlaneUnlock => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::SCOUT_PLANE_UNLOCK_COST_STEEL,
            cost_oil: crate::config::SCOUT_PLANE_UNLOCK_COST_OIL,
            research_ticks: crate::config::SCOUT_PLANE_UNLOCK_RESEARCH_TICKS,
        },
        UpgradeKind::DaoWind | UpgradeKind::DaoOfMagic | UpgradeKind::DaoOfHeaviness => {
            UpgradeDefinition {
                kind,
                researched_at: catalog.researched_at,
                requires_upgrade: None,
                cost_steel: crate::config::DAO_COST_STEEL,
                cost_oil: crate::config::DAO_COST_OIL,
                research_ticks: crate::config::DAO_RESEARCH_TICKS,
            }
        }
        UpgradeKind::Rockets => UpgradeDefinition {
            kind,
            researched_at: catalog.researched_at,
            requires_upgrade: None,
            cost_steel: crate::config::ROCKETS_COST_STEEL,
            cost_oil: crate::config::ROCKETS_COST_OIL,
            research_ticks: crate::config::ROCKETS_RESEARCH_TICKS,
        },
    }
}

pub fn required_for_unit(unit: EntityKind) -> Option<UpgradeKind> {
    match unit {
        EntityKind::Panzerfaust => Some(UpgradeKind::Panzerfausts),
        EntityKind::Artillery => Some(UpgradeKind::ArtilleryUnlock),
        EntityKind::RocketLauncher => Some(UpgradeKind::Rockets),
        EntityKind::Tank => Some(UpgradeKind::TankUnlock),
        _ => unit.dao(),
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
            researchable_upgrades(EntityKind::EngineeringComplex),
            vec![
                UpgradeKind::ArtilleryUnlock,
                UpgradeKind::BallisticTables,
                UpgradeKind::TankUnlock,
                UpgradeKind::SmokePlus,
                UpgradeKind::ScoutPlaneUnlock,
                UpgradeKind::Rockets,
                UpgradeKind::TankPlus,
            ]
        );
        assert_eq!(
            researchable_upgrades(EntityKind::ThreeDaoPavilion),
            vec![
                UpgradeKind::DaoWind,
                UpgradeKind::DaoOfMagic,
                UpgradeKind::DaoOfHeaviness
            ]
        );
        assert!(ALL.contains(&UpgradeKind::ArtilleryUnlock));
        assert!(researchable_upgrades(EntityKind::EngineeringComplex)
            .contains(&UpgradeKind::ArtilleryUnlock));
        assert!(researchable_upgrades(EntityKind::EngineeringComplex)
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
