use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    Worker,
    Rifleman,
    MachineGunner,
    AntiTankGun,
    MortarTeam,
    Artillery,
    ScoutCar,
    Tank,
    CommandCar,
    EkaterinaEngineer,
    EkaterinaConscript,
    CityCentre,
    Depot,
    Barracks,
    TrainingCentre,
    ResearchComplex,
    Factory,
    Steelworks,
    EkaterinaCommandPost,
    EkaterinaSupplyCache,
    EkaterinaWorkshop,
    Steel,
    Oil,
}

impl EntityKind {
    pub const ALL: [EntityKind; 23] = [
        EntityKind::Worker,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::AntiTankGun,
        EntityKind::MortarTeam,
        EntityKind::Artillery,
        EntityKind::ScoutCar,
        EntityKind::Tank,
        EntityKind::CommandCar,
        EntityKind::EkaterinaEngineer,
        EntityKind::EkaterinaConscript,
        EntityKind::CityCentre,
        EntityKind::Depot,
        EntityKind::Barracks,
        EntityKind::TrainingCentre,
        EntityKind::ResearchComplex,
        EntityKind::Factory,
        EntityKind::Steelworks,
        EntityKind::EkaterinaCommandPost,
        EntityKind::EkaterinaSupplyCache,
        EntityKind::EkaterinaWorkshop,
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

    pub fn is_worker(self) -> bool {
        matches!(self, EntityKind::Worker | EntityKind::EkaterinaEngineer)
    }

    pub fn stable_id(self) -> &'static str {
        match self {
            EntityKind::Worker => "worker",
            EntityKind::Rifleman => "rifleman",
            EntityKind::MachineGunner => "machine_gunner",
            EntityKind::AntiTankGun => "anti_tank_gun",
            EntityKind::MortarTeam => "mortar_team",
            EntityKind::Artillery => "artillery",
            EntityKind::ScoutCar => "scout_car",
            EntityKind::Tank => "tank",
            EntityKind::CommandCar => "command_car",
            EntityKind::EkaterinaEngineer => "ekaterina_engineer",
            EntityKind::EkaterinaConscript => "ekaterina_conscript",
            EntityKind::CityCentre => "city_centre",
            EntityKind::Depot => "depot",
            EntityKind::Barracks => "barracks",
            EntityKind::TrainingCentre => "training_centre",
            EntityKind::ResearchComplex => "research_complex",
            EntityKind::Factory => "factory",
            EntityKind::Steelworks => "steelworks",
            EntityKind::EkaterinaCommandPost => "ekaterina_command_post",
            EntityKind::EkaterinaSupplyCache => "ekaterina_supply_cache",
            EntityKind::EkaterinaWorkshop => "ekaterina_workshop",
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
            "rifleman" => Ok(EntityKind::Rifleman),
            "machine_gunner" => Ok(EntityKind::MachineGunner),
            "anti_tank_gun" => Ok(EntityKind::AntiTankGun),
            "mortar_team" => Ok(EntityKind::MortarTeam),
            "artillery" => Ok(EntityKind::Artillery),
            "scout_car" => Ok(EntityKind::ScoutCar),
            "tank" => Ok(EntityKind::Tank),
            "command_car" => Ok(EntityKind::CommandCar),
            "ekaterina_engineer" => Ok(EntityKind::EkaterinaEngineer),
            "ekaterina_conscript" => Ok(EntityKind::EkaterinaConscript),
            "city_centre" => Ok(EntityKind::CityCentre),
            "depot" => Ok(EntityKind::Depot),
            "barracks" => Ok(EntityKind::Barracks),
            "training_centre" => Ok(EntityKind::TrainingCentre),
            "research_complex" => Ok(EntityKind::ResearchComplex),
            "factory" => Ok(EntityKind::Factory),
            "steelworks" => Ok(EntityKind::Steelworks),
            "ekaterina_command_post" => Ok(EntityKind::EkaterinaCommandPost),
            "ekaterina_supply_cache" => Ok(EntityKind::EkaterinaSupplyCache),
            "ekaterina_workshop" => Ok(EntityKind::EkaterinaWorkshop),
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
            | EntityKind::ScoutCar
            | EntityKind::Tank
            | EntityKind::CommandCar
    )
}

pub fn uses_pivot_vehicle_movement(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::AntiTankGun | EntityKind::MortarTeam | EntityKind::Artillery | EntityKind::Tank
    )
}

pub fn uses_car_movement_semantics(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::ScoutCar | EntityKind::CommandCar)
}

pub fn fires_while_moving(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Tank | EntityKind::ScoutCar)
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
}
