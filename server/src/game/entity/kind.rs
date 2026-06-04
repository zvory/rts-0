use crate::rules;

// ---------------------------------------------------------------------------
// Typed entity kinds (internal simulation only; protocol strings live in
// `protocol::kinds` and conversion happens only at the wire boundary).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    Worker,
    Rifleman,
    MachineGunner,
    AtTeam,
    ScoutCar,
    Tank,
    CityCentre,
    Depot,
    Barracks,
    TrainingCentre,
    Factory,
    Steelworks,
    Steel,
    Oil,
}

impl EntityKind {
    #[cfg(test)]
    pub const ALL: [EntityKind; 14] = [
        EntityKind::Worker,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::AtTeam,
        EntityKind::ScoutCar,
        EntityKind::Tank,
        EntityKind::CityCentre,
        EntityKind::Depot,
        EntityKind::Barracks,
        EntityKind::TrainingCentre,
        EntityKind::Factory,
        EntityKind::Steelworks,
        EntityKind::Steel,
        EntityKind::Oil,
    ];

    pub fn is_unit(self) -> bool {
        rules::defs::unit_def(self).is_some()
    }

    pub fn is_building(self) -> bool {
        rules::defs::building_def(self).is_some()
    }

    pub fn is_node(self) -> bool {
        rules::defs::node_def(self).is_some()
    }

    pub fn to_protocol_str(self) -> &'static str {
        use crate::protocol::kinds;
        match self {
            EntityKind::Worker => kinds::WORKER,
            EntityKind::Rifleman => kinds::RIFLEMAN,
            EntityKind::MachineGunner => kinds::MACHINE_GUNNER,
            EntityKind::AtTeam => kinds::AT_TEAM,
            EntityKind::ScoutCar => kinds::SCOUT_CAR,
            EntityKind::Tank => kinds::TANK,
            EntityKind::CityCentre => kinds::CITY_CENTRE,
            EntityKind::Depot => kinds::DEPOT,
            EntityKind::Barracks => kinds::BARRACKS,
            EntityKind::TrainingCentre => kinds::TRAINING_CENTRE,
            EntityKind::Factory => kinds::FACTORY,
            EntityKind::Steelworks => kinds::STEELWORKS,
            EntityKind::Steel => kinds::STEEL,
            EntityKind::Oil => kinds::OIL,
        }
    }
}

impl std::str::FromStr for EntityKind {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use crate::protocol::kinds;
        match s {
            kinds::WORKER => Ok(EntityKind::Worker),
            kinds::RIFLEMAN => Ok(EntityKind::Rifleman),
            kinds::MACHINE_GUNNER => Ok(EntityKind::MachineGunner),
            kinds::AT_TEAM => Ok(EntityKind::AtTeam),
            kinds::SCOUT_CAR => Ok(EntityKind::ScoutCar),
            kinds::TANK => Ok(EntityKind::Tank),
            kinds::CITY_CENTRE => Ok(EntityKind::CityCentre),
            kinds::DEPOT => Ok(EntityKind::Depot),
            kinds::BARRACKS => Ok(EntityKind::Barracks),
            kinds::TRAINING_CENTRE => Ok(EntityKind::TrainingCentre),
            kinds::FACTORY => Ok(EntityKind::Factory),
            kinds::STEELWORKS => Ok(EntityKind::Steelworks),
            kinds::STEEL => Ok(EntityKind::Steel),
            kinds::OIL => Ok(EntityKind::Oil),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_protocol_str())
    }
}

pub(crate) fn uses_oriented_vehicle_body(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Tank | EntityKind::ScoutCar)
}

pub(crate) fn uses_tank_movement_semantics(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Tank)
}

pub(crate) fn uses_car_movement_semantics(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::ScoutCar)
}

pub(crate) fn fires_while_moving(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Tank | EntityKind::ScoutCar)
}
