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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponClass {
    None,
    SmallArms,
    AntiTank,
}

#[derive(Debug, Clone, Copy)]
pub struct UnitDef {
    pub kind: EntityKind,
    pub stats: balance::UnitStats,
    pub armor_class: ArmorClass,
    pub weapon: WeaponClass,
    pub trained_at: Option<EntityKind>,
    pub train_requirement: TechRequirement,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechRequirement {
    All(&'static [EntityKind]),
    Any(&'static [EntityKind]),
}

impl TechRequirement {
    pub fn is_met(self, owned: &[EntityKind]) -> bool {
        match self {
            Self::All(requirements) => requirements.iter().all(|req| owned.contains(req)),
            Self::Any(requirements) => requirements.iter().any(|req| owned.contains(req)),
        }
    }
}

const CITY_CENTRE_UNITS: &[EntityKind] = &[EntityKind::Worker];
const GOLEM_ONLY: &[EntityKind] = &[EntityKind::Golem];
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
            sight_tiles: 10,
            cost_steel: 50,
            cost_oil: 0,
            supply: 1,
            build_ticks: 396,
            radius: 9.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        trained_at: Some(EntityKind::CityCentre),
        train_requirement: TechRequirement::All(&[]),
    },
    UnitDef {
        kind: EntityKind::Golem,
        stats: balance::UnitStats {
            hp: 160,
            dmg: 16,
            range_tiles: 1,
            cooldown: 24,
            speed: 2.0,
            sight_tiles: 10,
            cost_steel: 0,
            cost_oil: 0,
            supply: 4,
            build_ticks: 396,
            radius: 9.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        trained_at: Some(EntityKind::Zamok),
        train_requirement: TechRequirement::All(&[]),
    },
    UnitDef {
        kind: EntityKind::Rifleman,
        stats: balance::UnitStats {
            hp: 45,
            dmg: 5,
            range_tiles: 4,
            cooldown: 16,
            speed: 1.6,
            sight_tiles: 11,
            cost_steel: 50,
            cost_oil: 0,
            supply: 1,
            build_ticks: 300,
            radius: 9.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        trained_at: Some(EntityKind::Barracks),
        train_requirement: TechRequirement::All(&[]),
    },
    UnitDef {
        kind: EntityKind::MachineGunner,
        stats: balance::UnitStats {
            hp: 55,
            dmg: 4,
            range_tiles: 6,
            cooldown: 6,
            speed: 1.28,
            sight_tiles: 11,
            cost_steel: 75,
            cost_oil: 10,
            supply: 2,
            build_ticks: 400,
            radius: 10.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        trained_at: Some(EntityKind::Barracks),
        train_requirement: TechRequirement::All(TRAINING_CENTRE_REQUIRED),
    },
    UnitDef {
        kind: EntityKind::AntiTankGun,
        stats: balance::UnitStats {
            hp: 45,
            dmg: 100,
            range_tiles: 5,
            cooldown: 72,
            speed: 1.6,
            sight_tiles: 9,
            cost_steel: 75,
            cost_oil: 25,
            supply: 3,
            build_ticks: 440,
            radius: 20.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::AntiTank,
        trained_at: Some(EntityKind::Steelworks),
        train_requirement: TechRequirement::All(STEELWORKS_REQUIRED),
    },
    UnitDef {
        kind: EntityKind::MortarTeam,
        stats: balance::UnitStats {
            hp: 75,
            dmg: balance::MORTAR_OUTER_DAMAGE,
            range_tiles: balance::MORTAR_RANGE_TILES,
            cooldown: 60,
            speed: 1.6,
            sight_tiles: 10,
            cost_steel: 100,
            cost_oil: 50,
            supply: 3,
            build_ticks: 460,
            radius: 18.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        trained_at: Some(EntityKind::Steelworks),
        train_requirement: TechRequirement::All(STEELWORKS_REQUIRED),
    },
    UnitDef {
        kind: EntityKind::Artillery,
        stats: balance::UnitStats {
            hp: 200,
            dmg: 0,
            range_tiles: balance::ARTILLERY_MAX_RANGE_TILES,
            cooldown: balance::ARTILLERY_RELOAD_TICKS,
            speed: 1.6,
            sight_tiles: 7,
            cost_steel: 300,
            cost_oil: 100,
            supply: 5,
            build_ticks: 750,
            radius: 18.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::None,
        trained_at: Some(EntityKind::Steelworks),
        train_requirement: TechRequirement::All(STEELWORKS_REQUIRED),
    },
    UnitDef {
        kind: EntityKind::Tank,
        stats: balance::UnitStats {
            hp: 292,
            dmg: 60,
            range_tiles: 5,
            cooldown: 72,
            speed: 2.0,
            sight_tiles: 9,
            cost_steel: 425,
            cost_oil: 150,
            supply: 8,
            build_ticks: 750,
            radius: 18.0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::AntiTank,
        trained_at: Some(EntityKind::Factory),
        train_requirement: TechRequirement::All(FACTORY_BUILDING_REQUIRED),
    },
    UnitDef {
        kind: EntityKind::ScoutCar,
        stats: balance::UnitStats {
            hp: 100,
            dmg: 6,
            range_tiles: 6,
            cooldown: 6,
            speed: 2.35,
            sight_tiles: 15,
            cost_steel: 125,
            cost_oil: 50,
            supply: 3,
            build_ticks: 480,
            radius: 9.6,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::SmallArms,
        trained_at: Some(EntityKind::Factory),
        train_requirement: TechRequirement::All(&[]),
    },
    UnitDef {
        kind: EntityKind::ScoutPlane,
        stats: balance::UnitStats {
            hp: balance::SCOUT_PLANE_HP,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
            speed: balance::SCOUT_PLANE_SPEED_PX_PER_TICK,
            sight_tiles: balance::SCOUT_PLANE_SIGHT_TILES,
            cost_steel: balance::SCOUT_PLANE_COST_STEEL,
            cost_oil: balance::SCOUT_PLANE_COST_OIL,
            supply: balance::SCOUT_PLANE_SUPPLY,
            build_ticks: 0,
            radius: 0.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::None,
        trained_at: None,
        train_requirement: TechRequirement::All(&[]),
    },
    UnitDef {
        kind: EntityKind::CommandCar,
        stats: balance::UnitStats {
            hp: 150,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
            speed: 2.35,
            sight_tiles: 13,
            cost_steel: 150,
            cost_oil: 75,
            supply: 4,
            build_ticks: balance::TICK_HZ * 15,
            radius: 9.6,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::None,
        trained_at: Some(EntityKind::Factory),
        train_requirement: TechRequirement::All(FACTORY_BUILDING_REQUIRED),
    },
    UnitDef {
        kind: EntityKind::Ekat,
        stats: balance::UnitStats {
            hp: 150,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
            speed: 1.6,
            sight_tiles: 12,
            cost_steel: 0,
            cost_oil: 0,
            supply: 0,
            build_ticks: 0,
            radius: 10.0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::None,
        trained_at: None,
        train_requirement: TechRequirement::All(&[]),
    },
];

pub const BUILDINGS: &[BuildingDef] = &[
    BuildingDef {
        kind: EntityKind::CityCentre,
        stats: balance::BuildingStats {
            hp: 600,
            sight_tiles: 1,
            cost_steel: 450,
            cost_oil: 150,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 750,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: CITY_CENTRE_UNITS,
        build_requires: &[],
    },
    BuildingDef {
        kind: EntityKind::Zamok,
        stats: balance::BuildingStats {
            hp: 600,
            sight_tiles: 1,
            cost_steel: 0,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: GOLEM_ONLY,
        build_requires: &[],
    },
    BuildingDef {
        kind: EntityKind::Depot,
        stats: balance::BuildingStats {
            hp: 110,
            sight_tiles: 1,
            cost_steel: 100,
            cost_oil: 0,
            foot_w: 2,
            foot_h: 2,
            build_ticks: 300,
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
            sight_tiles: 1,
            cost_steel: 150,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 200,
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
            sight_tiles: 1,
            cost_steel: 100,
            cost_oil: 50,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 560,
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
            sight_tiles: 1,
            cost_steel: 125,
            cost_oil: 125,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 749,
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
            sight_tiles: 1,
            cost_steel: 100,
            cost_oil: 100,
            foot_w: 3,
            foot_h: 3,
            build_ticks: balance::TICK_HZ * 15,
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
            sight_tiles: 1,
            cost_steel: 150,
            cost_oil: 100,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 599,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: STEELWORKS_UNITS,
        build_requires: FACTORY_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::TankTrap,
        stats: balance::BuildingStats {
            hp: 120,
            sight_tiles: 0,
            cost_steel: 30,
            cost_oil: 0,
            foot_w: 1,
            foot_h: 1,
            build_ticks: balance::TICK_HZ * 10,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Armored,
        weapon: WeaponClass::None,
        trains: &[],
        build_requires: TRAINING_CENTRE_REQUIRED,
    },
    BuildingDef {
        kind: EntityKind::PumpJack,
        stats: balance::BuildingStats {
            hp: 50,
            sight_tiles: 1,
            cost_steel: 50,
            cost_oil: 0,
            foot_w: 1,
            foot_h: 1,
            build_ticks: balance::TICK_HZ * 20,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        armor_class: ArmorClass::Small,
        weapon: WeaponClass::None,
        trains: &[],
        build_requires: &[],
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
    fn current_faction_catalog_matches_phase0_inventory() {
        let units: Vec<_> = UNITS.iter().map(|d| d.kind).collect();
        assert_eq!(
            units,
            vec![
                EntityKind::Worker,
                EntityKind::Golem,
                EntityKind::Rifleman,
                EntityKind::MachineGunner,
                EntityKind::AntiTankGun,
                EntityKind::MortarTeam,
                EntityKind::Artillery,
                EntityKind::Tank,
                EntityKind::ScoutCar,
                EntityKind::ScoutPlane,
                EntityKind::CommandCar,
                EntityKind::Ekat,
            ]
        );

        let buildings: Vec<_> = BUILDINGS.iter().map(|d| d.kind).collect();
        assert_eq!(
            buildings,
            vec![
                EntityKind::CityCentre,
                EntityKind::Zamok,
                EntityKind::Depot,
                EntityKind::Barracks,
                EntityKind::TrainingCentre,
                EntityKind::Factory,
                EntityKind::ResearchComplex,
                EntityKind::Steelworks,
                EntityKind::TankTrap,
                EntityKind::PumpJack,
            ]
        );

        assert_eq!(
            building_def(EntityKind::CityCentre).unwrap().trains,
            CITY_CENTRE_UNITS
        );
        assert_eq!(
            building_def(EntityKind::Barracks).unwrap().trains,
            BARRACKS_UNITS
        );
        assert_eq!(
            building_def(EntityKind::Factory).unwrap().trains,
            FACTORY_UNITS
        );
        assert_eq!(
            building_def(EntityKind::Steelworks).unwrap().trains,
            STEELWORKS_UNITS
        );
        assert_eq!(
            node_def(EntityKind::Steel).unwrap().amount,
            balance::STEEL_PATCH_AMOUNT
        );
        assert_eq!(
            node_def(EntityKind::Oil).unwrap().amount,
            balance::OIL_GEYSER_AMOUNT
        );
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
        let worker_speed = unit_def(EntityKind::Worker)
            .expect("worker def")
            .stats
            .speed;
        let tank_speed = unit_def(EntityKind::Tank).expect("tank def").stats.speed;

        assert_eq!(worker_speed, tank_speed);
    }

    #[test]
    fn artillery_moves_at_anti_tank_gun_speed() {
        let artillery_speed = unit_def(EntityKind::Artillery)
            .expect("artillery def")
            .stats
            .speed;
        let anti_tank_gun_speed = unit_def(EntityKind::AntiTankGun)
            .expect("anti-tank gun def")
            .stats
            .speed;

        assert_eq!(artillery_speed, anti_tank_gun_speed);
    }

    #[test]
    fn artillery_and_command_car_use_raw_hp_without_special_armor() {
        let artillery = unit_def(EntityKind::Artillery).expect("artillery def");
        let command_car = unit_def(EntityKind::CommandCar).expect("command car def");

        assert_eq!(artillery.stats.hp, 200);
        assert_eq!(artillery.armor_class, ArmorClass::Small);
        assert_eq!(command_car.stats.hp, 150);
        assert_eq!(command_car.armor_class, ArmorClass::Small);
    }

    #[test]
    fn gun_works_uses_square_vehicle_tech_footprint() {
        let stats = building_def(EntityKind::Steelworks)
            .expect("gun works def")
            .stats;

        assert_eq!((stats.foot_w, stats.foot_h), (3, 3));
    }

    #[test]
    fn non_obstacle_buildings_grant_only_local_sight() {
        for building in BUILDINGS {
            if building.kind == EntityKind::TankTrap {
                continue;
            }
            assert_eq!(
                building.stats.sight_tiles, 1,
                "{:?} should grant one-tile local sight",
                building.kind
            );
        }
    }

    #[test]
    fn tank_trap_uses_active_obstacle_stats() {
        let def = building_def(EntityKind::TankTrap).expect("tank trap def");

        assert_eq!(def.stats.hp, 120);
        assert_eq!(def.stats.sight_tiles, 0);
        assert_eq!((def.stats.cost_steel, def.stats.cost_oil), (30, 0));
        assert_eq!((def.stats.foot_w, def.stats.foot_h), (1, 1));
        assert_eq!(def.stats.build_ticks, balance::TICK_HZ * 10);
        assert_eq!(def.armor_class, ArmorClass::Armored);
        assert_eq!(def.weapon, WeaponClass::None);
        assert!(def.trains.is_empty());
        assert_eq!(def.build_requires, TRAINING_CENTRE_REQUIRED);
    }

    #[test]
    fn pump_jack_uses_contextual_oil_extractor_stats() {
        let def = building_def(EntityKind::PumpJack).expect("pump jack def");

        assert_eq!(def.stats.hp, 50);
        assert_eq!(def.stats.sight_tiles, 1);
        assert_eq!((def.stats.cost_steel, def.stats.cost_oil), (50, 0));
        assert_eq!((def.stats.foot_w, def.stats.foot_h), (1, 1));
        assert_eq!(def.stats.build_ticks, balance::TICK_HZ * 20);
        assert_eq!(def.armor_class, ArmorClass::Small);
        assert_eq!(def.weapon, WeaponClass::None);
        assert!(def.trains.is_empty());
        assert!(def.build_requires.is_empty());
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
