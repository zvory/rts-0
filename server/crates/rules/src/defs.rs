//! Data-driven rules definitions for entity kinds.
//!
//! Runtime identity still lives in [`EntityKind`]. Once an identity/protocol kind exists,
//! rule classification for a new unit such as a hypothetical `Halftrack` should require
//! appending one `UnitDef` here instead of adding category matches across rules modules.

use crate::balance;
use crate::EntityKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorClass {
    Small,
    Armored,
    Hard,
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
    pub stats: balance::UnitStats,
    pub armor_class: ArmorClass,
    pub weapon: WeaponClass,
    pub target_priority: TargetPriority,
    pub trained_at: Option<EntityKind>,
    pub train_requires: &'static [EntityKind],
}

#[derive(Debug, Clone, Copy)]
pub struct BuildingDef {
    pub kind: EntityKind,
    pub stats: balance::BuildingStats,
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
const BARRACKS_UNITS: &[EntityKind] = &[EntityKind::Rifleman, EntityKind::MachineGunner];
const STEELWORKS_UNITS: &[EntityKind] = &[
    EntityKind::MortarTeam,
    EntityKind::AntiTankGun,
    EntityKind::Artillery,
];
const FACTORY_UNITS: &[EntityKind] = &[
    EntityKind::ScoutCar,
    EntityKind::Tank,
    EntityKind::CommandCar,
];
const CITY_CENTRE_REQUIRED: &[EntityKind] = &[EntityKind::CityCentre];
const CITY_CENTRE_AND_BARRACKS_REQUIRED: &[EntityKind] =
    &[EntityKind::CityCentre, EntityKind::Barracks];
const TRAINING_CENTRE_REQUIRED: &[EntityKind] = &[EntityKind::TrainingCentre];
const CITY_CENTRE_AND_TRAINING_CENTRE_REQUIRED: &[EntityKind] =
    &[EntityKind::CityCentre, EntityKind::TrainingCentre];
const STEELWORKS_REQUIRED: &[EntityKind] = &[EntityKind::Steelworks];
const FACTORY_BUILDING_REQUIRED: &[EntityKind] = &[EntityKind::Factory];
const FACTORY_REQUIRED: &[EntityKind] = &[EntityKind::CityCentre, EntityKind::TrainingCentre];

pub const UNITS: &[UnitDef] = &[
    UnitDef {
        kind: EntityKind::Worker,
        stats: balance::UnitStats {
            hp: 40,
            dmg: 4,
            range_tiles: 1,
            cooldown: 24,
            speed: 2.0,
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
        trained_at: Some(EntityKind::CityCentre),
        train_requires: &[],
    },
    UnitDef {
        kind: EntityKind::Rifleman,
        stats: balance::UnitStats {
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
        stats: balance::UnitStats {
            hp: 55,
            dmg: 4,
            range_tiles: 6,
            cooldown: 6,
            speed: 1.28,
            sight_tiles: 8,
            cost_steel: 75,
            cost_oil: 10,
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
        kind: EntityKind::AntiTankGun,
        stats: balance::UnitStats {
            hp: 45,
            dmg: 60,
            range_tiles: 5,
            cooldown: 72,
            speed: 1.152,
            sight_tiles: 6,
            cost_steel: 75,
            cost_oil: 25,
            supply: 3,
            build_ticks: 440,
            radius: 20.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::AntiTank,
        target_priority: TargetPriority::PrefersArmored,
        trained_at: Some(EntityKind::Steelworks),
        train_requires: STEELWORKS_REQUIRED,
    },
    UnitDef {
        kind: EntityKind::MortarTeam,
        stats: balance::UnitStats {
            hp: 50,
            dmg: balance::MORTAR_OUTER_DAMAGE,
            range_tiles: 9,
            cooldown: 60,
            speed: 1.12,
            sight_tiles: 7,
            cost_steel: 100,
            cost_oil: 50,
            supply: 3,
            build_ticks: 460,
            radius: 18.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::Steelworks),
        train_requires: STEELWORKS_REQUIRED,
    },
    UnitDef {
        kind: EntityKind::Artillery,
        stats: balance::UnitStats {
            hp: 150,
            dmg: 0,
            range_tiles: balance::ARTILLERY_MAX_RANGE_TILES,
            cooldown: balance::ARTILLERY_RELOAD_TICKS,
            speed: 0.922,
            sight_tiles: 4,
            cost_steel: 300,
            cost_oil: 100,
            supply: 5,
            build_ticks: 750,
            radius: 18.0,
        },
        armor_class: ArmorClass::Hard,
        weapon: WeaponClass::None,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::Steelworks),
        train_requires: STEELWORKS_REQUIRED,
    },
    UnitDef {
        kind: EntityKind::Tank,
        stats: balance::UnitStats {
            hp: 292,
            dmg: 60,
            range_tiles: 5,
            cooldown: 72,
            speed: 2.0,
            sight_tiles: 6,
            cost_steel: 300,
            cost_oil: 150,
            supply: 6,
            build_ticks: 750,
            radius: 18.0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::AntiTank,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::Factory),
        train_requires: FACTORY_BUILDING_REQUIRED,
    },
    UnitDef {
        kind: EntityKind::ScoutCar,
        stats: balance::UnitStats {
            hp: 150,
            dmg: 6,
            range_tiles: 5,
            cooldown: 6,
            speed: 2.35,
            sight_tiles: 10,
            cost_steel: 125,
            cost_oil: 50,
            supply: 3,
            build_ticks: 480,
            radius: 9.6,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::Factory),
        train_requires: &[],
    },
    UnitDef {
        kind: EntityKind::CommandCar,
        stats: balance::UnitStats {
            hp: 225,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
            speed: 2.35,
            sight_tiles: 10,
            cost_steel: 150,
            cost_oil: 75,
            supply: 4,
            build_ticks: balance::TICK_HZ * 15,
            radius: 9.6,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::None,
        target_priority: TargetPriority::Default,
        trained_at: Some(EntityKind::Factory),
        train_requires: FACTORY_BUILDING_REQUIRED,
    },
];

pub const BUILDINGS: &[BuildingDef] = &[
    BuildingDef {
        kind: EntityKind::CityCentre,
        stats: balance::BuildingStats {
            hp: 600,
            sight_tiles: 9,
            cost_steel: 200,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 400,
            provides_supply: balance::CITY_CENTRE_SUPPLY,
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
        stats: balance::BuildingStats {
            hp: 110,
            sight_tiles: 4,
            cost_steel: 100,
            cost_oil: 0,
            foot_w: 2,
            foot_h: 2,
            build_ticks: 300,
            provides_supply: balance::DEPOT_SUPPLY,
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
        stats: balance::BuildingStats {
            hp: 165,
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
        build_requires: CITY_CENTRE_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::TrainingCentre,
        stats: balance::BuildingStats {
            hp: 300,
            sight_tiles: 6,
            cost_steel: 100,
            cost_oil: 50,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 560,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: &[],
        build_requires: CITY_CENTRE_AND_BARRACKS_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::Factory,
        stats: balance::BuildingStats {
            hp: 360,
            sight_tiles: 6,
            cost_steel: 125,
            cost_oil: 125,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 620,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: FACTORY_UNITS,
        build_requires: FACTORY_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::ResearchComplex,
        stats: balance::BuildingStats {
            hp: 165,
            sight_tiles: 6,
            cost_steel: 100,
            cost_oil: 100,
            foot_w: 3,
            foot_h: 3,
            build_ticks: balance::TICK_HZ * 15,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: &[],
        build_requires: CITY_CENTRE_AND_TRAINING_CENTRE_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::Steelworks,
        stats: balance::BuildingStats {
            hp: 300,
            sight_tiles: 6,
            cost_steel: 125,
            cost_oil: 125,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 620,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: STEELWORKS_UNITS,
        build_requires: FACTORY_REQUIRED,
    },
];

pub const NODES: &[NodeDef] = &[
    NodeDef {
        kind: EntityKind::Steel,
        amount: balance::STEEL_PATCH_AMOUNT,
    },
    NodeDef {
        kind: EntityKind::Oil,
        amount: balance::OIL_GEYSER_AMOUNT,
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

    #[test]
    fn depot_and_barracks_hp_keep_requested_ratio() {
        let depot_hp = building_def(EntityKind::Depot).expect("depot def").stats.hp;
        let barracks_hp = building_def(EntityKind::Barracks)
            .expect("barracks def")
            .stats
            .hp;

        assert_eq!(depot_hp, 110);
        assert_eq!(barracks_hp, depot_hp * 3 / 2);
    }

    #[test]
    fn workers_move_at_tank_speed() {
        let worker_speed = unit_def(EntityKind::Worker).expect("worker def").stats.speed;
        let tank_speed = unit_def(EntityKind::Tank).expect("tank def").stats.speed;

        assert_eq!(worker_speed, tank_speed);
    }

    #[test]
    fn gun_works_uses_square_vehicle_tech_footprint() {
        let stats = building_def(EntityKind::Steelworks)
            .expect("gun works def")
            .stats;

        assert_eq!((stats.foot_w, stats.foot_h), (3, 3));
    }

    #[test]
    fn research_complex_uses_requested_independent_stats() {
        let stats = building_def(EntityKind::ResearchComplex)
            .expect("research complex def")
            .stats;

        assert_eq!(stats.hp, 165);
        assert_eq!((stats.cost_steel, stats.cost_oil), (100, 100));
        assert_eq!((stats.foot_w, stats.foot_h), (3, 3));
        assert_eq!(stats.build_ticks, balance::TICK_HZ * 15);
    }
}
