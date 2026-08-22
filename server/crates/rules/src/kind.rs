use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Worker,
    Golem,
    Rifleman,
    Panzerfaust,
    MachineGunner,
    AntiTankGun,
    MortarTeam,
    Artillery,
    RocketLauncher,
    ScoutCar,
    ScoutPlane,
    Tank,
    CommandCar,
    Ekat,
    ResourceDepot,
    Zamok,
    Depot,
    Barracks,
    TrainingCentre,
    EngineeringComplex,
    Factory,
    Steelworks,
    TankTrap,
    SteelMine,
    PumpJack,
    Steel,
    Oil,
}

impl EntityKind {
    pub const ALL: [EntityKind; 27] = [
        EntityKind::Worker,
        EntityKind::Golem,
        EntityKind::Rifleman,
        EntityKind::Panzerfaust,
        EntityKind::MachineGunner,
        EntityKind::AntiTankGun,
        EntityKind::MortarTeam,
        EntityKind::Artillery,
        EntityKind::RocketLauncher,
        EntityKind::ScoutCar,
        EntityKind::ScoutPlane,
        EntityKind::Tank,
        EntityKind::CommandCar,
        EntityKind::Ekat,
        EntityKind::ResourceDepot,
        EntityKind::Zamok,
        EntityKind::Depot,
        EntityKind::Barracks,
        EntityKind::TrainingCentre,
        EntityKind::EngineeringComplex,
        EntityKind::Factory,
        EntityKind::Steelworks,
        EntityKind::TankTrap,
        EntityKind::SteelMine,
        EntityKind::PumpJack,
        EntityKind::Steel,
        EntityKind::Oil,
    ];

    pub fn is_unit(self) -> bool {
        crate::defs::unit_def(self).is_some()
    }

    pub fn is_building(self) -> bool {
        crate::defs::building_def(self).is_some()
    }

    pub fn is_node(self) -> bool {
        crate::defs::node_def(self).is_some()
    }

    pub fn is_resource_extractor(self) -> bool {
        matches!(self, EntityKind::SteelMine | EntityKind::PumpJack)
    }

    pub const fn extracted_resource_kind(self) -> Option<EntityKind> {
        match self {
            EntityKind::SteelMine => Some(EntityKind::Steel),
            EntityKind::PumpJack => Some(EntityKind::Oil),
            _ => None,
        }
    }

    pub fn stable_id(self) -> &'static str {
        match self {
            EntityKind::Worker => "worker",
            EntityKind::Golem => "golem",
            EntityKind::Rifleman => "rifleman",
            EntityKind::Panzerfaust => "panzerfaust",
            EntityKind::MachineGunner => "machine_gunner",
            EntityKind::AntiTankGun => "anti_tank_gun",
            EntityKind::MortarTeam => "mortar_team",
            EntityKind::Artillery => "artillery",
            EntityKind::RocketLauncher => "rocket_launcher",
            EntityKind::ScoutCar => "scout_car",
            EntityKind::ScoutPlane => "scout_plane",
            EntityKind::Tank => "tank",
            EntityKind::CommandCar => "command_car",
            EntityKind::Ekat => "ekat",
            EntityKind::ResourceDepot => "resource_depot",
            EntityKind::Zamok => "zamok",
            EntityKind::Depot => "depot",
            EntityKind::Barracks => "barracks",
            EntityKind::TrainingCentre => "training_centre",
            EntityKind::EngineeringComplex => "engineering_complex",
            EntityKind::Factory => "factory",
            EntityKind::Steelworks => "steelworks",
            EntityKind::TankTrap => "tank_trap",
            EntityKind::SteelMine => "steel_mine",
            EntityKind::PumpJack => "pump_jack",
            EntityKind::Steel => "steel",
            EntityKind::Oil => "oil",
        }
    }
}

impl FromStr for EntityKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "worker" => Ok(EntityKind::Worker),
            "golem" => Ok(EntityKind::Golem),
            "rifleman" => Ok(EntityKind::Rifleman),
            "panzerfaust" => Ok(EntityKind::Panzerfaust),
            "machine_gunner" => Ok(EntityKind::MachineGunner),
            "anti_tank_gun" => Ok(EntityKind::AntiTankGun),
            "mortar_team" => Ok(EntityKind::MortarTeam),
            "artillery" => Ok(EntityKind::Artillery),
            "rocket_launcher" => Ok(EntityKind::RocketLauncher),
            "scout_car" => Ok(EntityKind::ScoutCar),
            "scout_plane" => Ok(EntityKind::ScoutPlane),
            "tank" => Ok(EntityKind::Tank),
            "command_car" => Ok(EntityKind::CommandCar),
            "ekat" => Ok(EntityKind::Ekat),
            "resource_depot" => Ok(EntityKind::ResourceDepot),
            "zamok" => Ok(EntityKind::Zamok),
            "depot" => Ok(EntityKind::Depot),
            "barracks" => Ok(EntityKind::Barracks),
            "training_centre" => Ok(EntityKind::TrainingCentre),
            "engineering_complex" => Ok(EntityKind::EngineeringComplex),
            "factory" => Ok(EntityKind::Factory),
            "steelworks" => Ok(EntityKind::Steelworks),
            "tank_trap" => Ok(EntityKind::TankTrap),
            "steel_mine" => Ok(EntityKind::SteelMine),
            "pump_jack" => Ok(EntityKind::PumpJack),
            "steel" => Ok(EntityKind::Steel),
            "oil" => Ok(EntityKind::Oil),
            _ => Err(()),
        }
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.stable_id())
    }
}

pub fn uses_oriented_vehicle_body(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::AntiTankGun
            | EntityKind::MortarTeam
            | EntityKind::Artillery
            | EntityKind::RocketLauncher
            | EntityKind::ScoutCar
            | EntityKind::Tank
            | EntityKind::CommandCar
    )
}

pub fn supports_manual_emplacement(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::AntiTankGun | EntityKind::Artillery)
}

pub fn is_anti_tank_gun(kind: EntityKind) -> bool {
    kind == EntityKind::AntiTankGun
}

pub fn is_rifle_infantry(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Rifleman | EntityKind::Panzerfaust)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MovementBodyClass {
    InfantryLike,
    VehicleBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StaticBlockerClass {
    None,
    AllGround,
    VehicleBodyOnly,
}

pub fn movement_body_class(kind: EntityKind) -> MovementBodyClass {
    if uses_oriented_vehicle_body(kind) {
        MovementBodyClass::VehicleBody
    } else {
        MovementBodyClass::InfantryLike
    }
}

pub fn static_blocker_class(kind: EntityKind) -> StaticBlockerClass {
    if kind == EntityKind::TankTrap {
        StaticBlockerClass::VehicleBodyOnly
    } else if matches!(kind, EntityKind::SteelMine | EntityKind::PumpJack) {
        StaticBlockerClass::None
    } else if kind.is_building() {
        StaticBlockerClass::AllGround
    } else {
        StaticBlockerClass::None
    }
}

pub fn blocks_line_of_sight(kind: EntityKind) -> bool {
    kind.is_building()
        && !matches!(
            kind,
            EntityKind::TankTrap | EntityKind::SteelMine | EntityKind::PumpJack
        )
}

pub fn uses_pivot_vehicle_movement(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::AntiTankGun | EntityKind::MortarTeam | EntityKind::Artillery | EntityKind::Tank
    )
}

pub fn uses_car_movement_semantics(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::ScoutCar | EntityKind::CommandCar | EntityKind::RocketLauncher
    )
}

pub fn fires_while_moving(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Tank | EntityKind::ScoutCar)
}

/// Whether this unit's tracked running gear leaves durable tread marks while moving.
pub fn leaves_tank_treads(kind: EntityKind) -> bool {
    kind == EntityKind::Tank
}

/// Durable terrain-mark presentation category for an entity death.
pub fn death_ground_decal_class(kind: EntityKind) -> Option<&'static str> {
    match kind {
        EntityKind::Worker
        | EntityKind::Rifleman
        | EntityKind::Panzerfaust
        | EntityKind::MachineGunner
        | EntityKind::MortarTeam
        | EntityKind::Ekat => Some("infantry"),
        EntityKind::ScoutCar
        | EntityKind::Tank
        | EntityKind::CommandCar
        | EntityKind::AntiTankGun
        | EntityKind::Artillery => Some("scorch"),
        EntityKind::RocketLauncher => Some("scorch"),
        kind if kind.is_building() => Some("buildingScorch"),
        _ => None,
    }
}

pub fn mortar_ground_decal_source_kind() -> EntityKind {
    EntityKind::MortarTeam
}

pub fn artillery_ground_decal_source_kind() -> EntityKind {
    EntityKind::Artillery
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_kind_ids_round_trip() {
        for kind in EntityKind::ALL {
            assert_eq!(kind.stable_id().parse::<EntityKind>(), Ok(kind));
            assert_eq!(kind.to_string(), kind.stable_id());
        }
    }

    #[test]
    fn movement_body_class_names_vehicle_body_units() {
        let vehicle_body_kinds = [
            EntityKind::AntiTankGun,
            EntityKind::MortarTeam,
            EntityKind::Artillery,
            EntityKind::ScoutCar,
            EntityKind::Tank,
            EntityKind::CommandCar,
            EntityKind::RocketLauncher,
        ];
        for kind in EntityKind::ALL {
            let expected = if vehicle_body_kinds.contains(&kind) {
                MovementBodyClass::VehicleBody
            } else {
                MovementBodyClass::InfantryLike
            };
            assert_eq!(movement_body_class(kind), expected, "{kind:?}");
        }
    }

    #[test]
    fn only_tracked_tanks_leave_tread_marks() {
        for kind in EntityKind::ALL {
            assert_eq!(
                leaves_tank_treads(kind),
                kind == EntityKind::Tank,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn special_field_buildings_keep_static_and_los_blocking_explicit() {
        assert_eq!(
            static_blocker_class(EntityKind::TankTrap),
            StaticBlockerClass::VehicleBodyOnly
        );
        assert_eq!(
            static_blocker_class(EntityKind::Depot),
            StaticBlockerClass::AllGround
        );
        assert_eq!(
            static_blocker_class(EntityKind::PumpJack),
            StaticBlockerClass::None
        );
        assert_eq!(
            static_blocker_class(EntityKind::Worker),
            StaticBlockerClass::None
        );
        assert!(!blocks_line_of_sight(EntityKind::TankTrap));
        assert!(!blocks_line_of_sight(EntityKind::PumpJack));
        assert!(blocks_line_of_sight(EntityKind::Depot));
    }
}
