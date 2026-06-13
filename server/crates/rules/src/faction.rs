//! Faction-aware catalog queries.
//!
//! Runtime and wire identity remain global: faction entries reference the same [`EntityKind`],
//! upgrade ids, ability ids, and Steel/Oil/Supply costs used by the current game. Reuse a global
//! id across factions only when its gameplay semantics are identical for every faction that can
//! use it; divergent behavior needs a distinct global id gated by catalog availability.

use crate::{defs, EntityKind};

pub const DEFAULT_FACTION_ID: &str = "steel_vanguard";
pub const EMPTY_FIXTURE_FACTION_ID: &str = "phase2_empty_fixture";

pub const METHAMPHETAMINES_UPGRADE: &str = "methamphetamines";
pub const ANTI_TANK_GUN_UNLOCK_UPGRADE: &str = "anti_tank_gun_unlock";
pub const ARTILLERY_UNLOCK_UPGRADE: &str = "artillery_unlock";
pub const TANK_UNLOCK_UPGRADE: &str = "tank_unlock";
pub const COMMAND_CAR_UNLOCK_UPGRADE: &str = "command_car_unlock";
pub const MORTAR_AUTOCAST_UPGRADE: &str = "mortar_autocast";

pub const SMOKE_ABILITY: &str = "smoke";
pub const MORTAR_FIRE_ABILITY: &str = "mortarFire";
pub const POINT_FIRE_ABILITY: &str = "pointFire";
pub const BREAKTHROUGH_ABILITY: &str = "breakthrough";

const DEFAULT_UNITS: &[EntityKind] = &[
    EntityKind::Worker,
    EntityKind::Rifleman,
    EntityKind::MachineGunner,
    EntityKind::AntiTankGun,
    EntityKind::MortarTeam,
    EntityKind::Artillery,
    EntityKind::Tank,
    EntityKind::ScoutCar,
    EntityKind::CommandCar,
];

const DEFAULT_BUILDINGS: &[EntityKind] = &[
    EntityKind::CityCentre,
    EntityKind::Depot,
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::Factory,
    EntityKind::ResearchComplex,
    EntityKind::Steelworks,
];

const DEFAULT_WORKER_BUILDABLES: &[EntityKind] = &[
    EntityKind::CityCentre,
    EntityKind::Depot,
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Factory,
    EntityKind::Steelworks,
];

const DEFAULT_UPGRADES: &[UpgradeCatalogEntry] = &[
    UpgradeCatalogEntry {
        id: METHAMPHETAMINES_UPGRADE,
        researched_at: EntityKind::TrainingCentre,
    },
    UpgradeCatalogEntry {
        id: ANTI_TANK_GUN_UNLOCK_UPGRADE,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        id: ARTILLERY_UNLOCK_UPGRADE,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        id: TANK_UNLOCK_UPGRADE,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        id: COMMAND_CAR_UNLOCK_UPGRADE,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        id: MORTAR_AUTOCAST_UPGRADE,
        researched_at: EntityKind::ResearchComplex,
    },
];

const DEFAULT_ABILITIES: &[AbilityCatalogEntry] = &[
    AbilityCatalogEntry {
        id: SMOKE_ABILITY,
        carriers: &[EntityKind::ScoutCar],
    },
    AbilityCatalogEntry {
        id: MORTAR_FIRE_ABILITY,
        carriers: &[EntityKind::MortarTeam],
    },
    AbilityCatalogEntry {
        id: POINT_FIRE_ABILITY,
        carriers: &[EntityKind::Artillery],
    },
    AbilityCatalogEntry {
        id: BREAKTHROUGH_ABILITY,
        carriers: &[EntityKind::CommandCar],
    },
];

pub const CURRENT_CATALOG: FactionCatalog = FactionCatalog {
    id: DEFAULT_FACTION_ID,
    units: DEFAULT_UNITS,
    buildings: DEFAULT_BUILDINGS,
    buildables: DEFAULT_WORKER_BUILDABLES,
    upgrades: DEFAULT_UPGRADES,
    abilities: DEFAULT_ABILITIES,
    builders: &[EntityKind::Worker],
    gatherers: &[EntityKind::Worker],
    production_anchors: &[
        EntityKind::CityCentre,
        EntityKind::Barracks,
        EntityKind::Factory,
        EntityKind::Steelworks,
    ],
};

pub const EMPTY_FIXTURE_CATALOG: FactionCatalog = FactionCatalog {
    id: EMPTY_FIXTURE_FACTION_ID,
    units: &[],
    buildings: &[],
    buildables: &[],
    upgrades: &[],
    abilities: &[],
    builders: &[],
    gatherers: &[],
    production_anchors: &[],
};

pub const CATALOGS: &[FactionCatalog] = &[CURRENT_CATALOG, EMPTY_FIXTURE_CATALOG];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeCatalogEntry {
    pub id: &'static str,
    pub researched_at: EntityKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityCatalogEntry {
    pub id: &'static str,
    pub carriers: &'static [EntityKind],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactionCatalog {
    pub id: &'static str,
    pub units: &'static [EntityKind],
    pub buildings: &'static [EntityKind],
    pub buildables: &'static [EntityKind],
    pub upgrades: &'static [UpgradeCatalogEntry],
    pub abilities: &'static [AbilityCatalogEntry],
    pub builders: &'static [EntityKind],
    pub gatherers: &'static [EntityKind],
    pub production_anchors: &'static [EntityKind],
}

impl FactionCatalog {
    pub fn allows_unit(self, kind: EntityKind) -> bool {
        self.units.contains(&kind)
    }

    pub fn allows_building(self, kind: EntityKind) -> bool {
        self.buildings.contains(&kind)
    }

    pub fn can_build(self, builder: EntityKind, building: EntityKind) -> bool {
        self.builders.contains(&builder) && self.buildables.contains(&building)
    }

    pub fn can_gather(self, unit: EntityKind) -> bool {
        self.gatherers.contains(&unit)
    }

    pub fn can_act_as_production_anchor(self, building: EntityKind) -> bool {
        self.production_anchors.contains(&building)
    }

    pub fn trainable_units(self, building_kind: EntityKind) -> Vec<EntityKind> {
        if !self.can_act_as_production_anchor(building_kind) {
            return Vec::new();
        }
        defs::building_def(building_kind)
            .map(|d| d.trains)
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|unit| self.allows_unit(*unit))
            .collect::<Vec<_>>()
    }

    pub fn researchable_upgrades(self, building_kind: EntityKind) -> Vec<&'static str> {
        self.upgrades
            .iter()
            .filter(|entry| entry.researched_at == building_kind)
            .map(|entry| entry.id)
            .collect()
    }

    pub fn allows_research(self, upgrade_id: &str, building_kind: EntityKind) -> bool {
        self.upgrades
            .iter()
            .any(|entry| entry.id == upgrade_id && entry.researched_at == building_kind)
    }
}

pub fn catalog_for(faction_id: &str) -> Option<FactionCatalog> {
    CATALOGS
        .iter()
        .copied()
        .find(|catalog| catalog.id == faction_id)
}

pub fn catalog_for_or_default(faction_id: &str) -> FactionCatalog {
    catalog_for(faction_id).unwrap_or(CURRENT_CATALOG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_matches_defs_inventory() {
        let units: Vec<_> = defs::UNITS.iter().map(|d| d.kind).collect();
        assert_eq!(CURRENT_CATALOG.units, units.as_slice());

        let buildings: Vec<_> = defs::BUILDINGS.iter().map(|d| d.kind).collect();
        assert_eq!(CURRENT_CATALOG.buildings, buildings.as_slice());
    }

    #[test]
    fn default_catalog_routes_current_tech_tree() {
        let catalog = CURRENT_CATALOG;

        assert_eq!(
            catalog.trainable_units(EntityKind::CityCentre),
            vec![EntityKind::Worker]
        );
        assert_eq!(
            catalog.trainable_units(EntityKind::Barracks),
            vec![EntityKind::Rifleman, EntityKind::MachineGunner]
        );
        assert_eq!(
            catalog.trainable_units(EntityKind::Factory),
            vec![
                EntityKind::ScoutCar,
                EntityKind::Tank,
                EntityKind::CommandCar
            ]
        );
        assert_eq!(
            catalog.trainable_units(EntityKind::Steelworks),
            vec![
                EntityKind::MortarTeam,
                EntityKind::AntiTankGun,
                EntityKind::Artillery
            ]
        );
        assert!(catalog.allows_research(METHAMPHETAMINES_UPGRADE, EntityKind::TrainingCentre));
        assert!(catalog.allows_research(TANK_UNLOCK_UPGRADE, EntityKind::ResearchComplex));
        assert!(!catalog.allows_research(TANK_UNLOCK_UPGRADE, EntityKind::TrainingCentre));
    }

    #[test]
    fn fixture_catalog_rejects_global_current_kinds() {
        let catalog = EMPTY_FIXTURE_CATALOG;

        assert!(!catalog.allows_unit(EntityKind::Worker));
        assert!(!catalog.allows_building(EntityKind::Depot));
        assert!(!catalog.can_build(EntityKind::Worker, EntityKind::Depot));
        assert!(catalog.trainable_units(EntityKind::CityCentre).is_empty());
        assert!(!catalog.allows_research(METHAMPHETAMINES_UPGRADE, EntityKind::TrainingCentre));
    }
}
