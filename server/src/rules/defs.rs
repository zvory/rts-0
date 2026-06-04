//! Data-driven rules definitions for entity kinds.
//!
//! Runtime identity still lives in [`EntityKind`]. Once an identity/protocol kind exists,
//! rule classification for a new unit such as a hypothetical `Halftrack` should require
//! appending one `UnitDef` here instead of adding category matches across rules modules.

use crate::config;
use crate::game::entity::EntityKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorClass {
    Small,
    Armored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponClass {
    None,
    SmallArms,
    AntiTank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPriority {
    Default,
    PrefersArmored,
}

#[derive(Debug, Clone, Copy)]
pub struct UnitDef {
    pub kind: EntityKind,
    pub stats: config::UnitStats,
    pub armor_class: ArmorClass,
    pub weapon: WeaponClass,
    pub target_priority: TargetPriority,
    pub trained_at: Option<EntityKind>,
    pub train_requires: &'static [EntityKind],
}

#[derive(Debug, Clone, Copy)]
pub struct BuildingDef {
    pub kind: EntityKind,
    pub stats: config::BuildingStats,
    pub armor_class: ArmorClass,
    pub weapon: WeaponClass,
    pub trains: &'static [EntityKind],
    pub build_requires: &'static [EntityKind],
}

#[derive(Debug, Clone, Copy)]
pub struct NodeDef {
    pub kind: EntityKind,
    pub amount: u32,
}

const WORKER_ONLY: &[EntityKind] = &[EntityKind::Worker];
const BARRACKS_UNITS: &[EntityKind] = &[
    EntityKind::Rifleman,
    EntityKind::MachineGunner,
    EntityKind::AtTeam,
];
const TANK_ONLY: &[EntityKind] = &[EntityKind::Tank];
const INDUSTRIAL_CENTER_REQUIRED: &[EntityKind] = &[EntityKind::IndustrialCenter];
const INDUSTRIAL_CENTER_AND_BARRACKS_REQUIRED: &[EntityKind] =
    &[EntityKind::IndustrialCenter, EntityKind::Barracks];
const TRAINING_CENTRE_REQUIRED: &[EntityKind] = &[EntityKind::TrainingCentre];
const STEELWORKS_REQUIRED: &[EntityKind] = &[EntityKind::Steelworks];
const FACTORY_REQUIRED: &[EntityKind] = &[EntityKind::IndustrialCenter, EntityKind::TrainingCentre];

pub const UNITS: &[UnitDef] = &[
    UnitDef {
        kind: EntityKind::Worker,
        stats: config::UnitStats {
            hp: 40,
            dmg: 4,
            range_tiles: 1,
            cooldown: 24,
            speed: 1.6,
            sight_tiles: 7,
            cost_steel: 50,
            cost_oil: 0,
            supply: 1,
            build_ticks: 360,
            radius: 9.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::IndustrialCenter),
        train_requires: &[],
    },
    UnitDef {
        kind: EntityKind::Rifleman,
        stats: config::UnitStats {
            hp: 45,
            dmg: 5,
            range_tiles: 4,
            cooldown: 16,
            speed: 1.6,
            sight_tiles: 8,
            cost_steel: 50,
            cost_oil: 0,
            supply: 1,
            build_ticks: 300,
            radius: 9.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::Barracks),
        train_requires: &[],
    },
    UnitDef {
        kind: EntityKind::MachineGunner,
        stats: config::UnitStats {
            hp: 55,
            dmg: 4,
            range_tiles: 5,
            cooldown: 6,
            speed: 1.28,
            sight_tiles: 8,
            cost_steel: 75,
            cost_oil: 25,
            supply: 2,
            build_ticks: 400,
            radius: 10.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::Barracks),
        train_requires: TRAINING_CENTRE_REQUIRED,
    },
    UnitDef {
        kind: EntityKind::AtTeam,
        stats: config::UnitStats {
            hp: 45,
            dmg: 48,
            range_tiles: 5,
            cooldown: 72,
            speed: 1.28,
            sight_tiles: 8,
            cost_steel: 75,
            cost_oil: 25,
            supply: 2,
            build_ticks: 440,
            radius: 10.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::AntiTank,
        target_priority: TargetPriority::PrefersArmored,
        trained_at: Some(EntityKind::Barracks),
        train_requires: TRAINING_CENTRE_REQUIRED,
    },
    UnitDef {
        kind: EntityKind::Tank,
        stats: config::UnitStats {
            hp: 390,
            dmg: 60,
            range_tiles: 3,
            cooldown: 72,
            speed: 2.0,
            sight_tiles: 7,
            cost_steel: 200,
            cost_oil: 150,
            supply: 6,
            build_ticks: 750,
            radius: 15.0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::AntiTank,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::Factory),
        train_requires: STEELWORKS_REQUIRED,
    },
];

pub const BUILDINGS: &[BuildingDef] = &[
    BuildingDef {
        kind: EntityKind::IndustrialCenter,
        stats: config::BuildingStats {
            hp: 600,
            sight_tiles: 9,
            cost_steel: 200,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 400,
            provides_supply: config::INDUSTRIAL_CENTER_SUPPLY,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: WORKER_ONLY,
        build_requires: &[],
    },
    BuildingDef {
        kind: EntityKind::Depot,
        stats: config::BuildingStats {
            hp: 220,
            sight_tiles: 4,
            cost_steel: 100,
            cost_oil: 0,
            foot_w: 2,
            foot_h: 2,
            build_ticks: 180,
            provides_supply: config::DEPOT_SUPPLY,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: &[],
        build_requires: &[],
    },
    BuildingDef {
        kind: EntityKind::Barracks,
        stats: config::BuildingStats {
            hp: 320,
            sight_tiles: 6,
            cost_steel: 150,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 200,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: BARRACKS_UNITS,
        build_requires: INDUSTRIAL_CENTER_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::TrainingCentre,
        stats: config::BuildingStats {
            hp: 300,
            sight_tiles: 6,
            cost_steel: 100,
            cost_oil: 50,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 220,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: &[],
        build_requires: INDUSTRIAL_CENTER_AND_BARRACKS_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::Factory,
        stats: config::BuildingStats {
            hp: 360,
            sight_tiles: 6,
            cost_steel: 200,
            cost_oil: 100,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 240,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: TANK_ONLY,
        build_requires: FACTORY_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::Steelworks,
        stats: config::BuildingStats {
            hp: 300,
            sight_tiles: 6,
            cost_steel: 125,
            cost_oil: 125,
            foot_w: 2,
            foot_h: 2,
            build_ticks: 220,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: &[],
        build_requires: FACTORY_REQUIRED,
    },
];

pub const NODES: &[NodeDef] = &[
    NodeDef {
        kind: EntityKind::Steel,
        amount: config::STEEL_PATCH_AMOUNT,
    },
    NodeDef {
        kind: EntityKind::Oil,
        amount: config::OIL_GEYSER_AMOUNT,
    },
];

pub fn unit_def(kind: EntityKind) -> Option<&'static UnitDef> {
    UNITS.iter().find(|d| d.kind == kind)
}

pub fn building_def(kind: EntityKind) -> Option<&'static BuildingDef> {
    BUILDINGS.iter().find(|d| d.kind == kind)
}

pub fn node_def(kind: EntityKind) -> Option<&'static NodeDef> {
    NODES.iter().find(|d| d.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entity_kind_has_exactly_one_def() {
        for kind in EntityKind::ALL {
            let def_count = [
                unit_def(kind).is_some(),
                building_def(kind).is_some(),
                node_def(kind).is_some(),
            ]
            .into_iter()
            .filter(|has_def| *has_def)
            .count();
            assert_eq!(def_count, 1, "{kind} should resolve to exactly one def");
        }
    }

    #[test]
    fn unit_training_tables_are_bidirectional() {
        for unit in UNITS {
            let Some(trainer_kind) = unit.trained_at else {
                continue;
            };
            let trainer = building_def(trainer_kind).expect("trainer must be a building def");
            assert!(
                trainer.trains.contains(&unit.kind),
                "{trainer_kind} must list {} as trainable",
                unit.kind
            );
        }

        for building in BUILDINGS {
            for unit_kind in building.trains {
                let unit = unit_def(*unit_kind).expect("trained kind must be a unit def");
                assert_eq!(
                    unit.trained_at,
                    Some(building.kind),
                    "{} must point back to {} as trainer",
                    unit.kind,
                    building.kind
                );
            }
        }
    }
}
