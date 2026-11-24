use crate::game::entity::EntityKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuildPlacementPolicy {
    AllGround,
    GroundField,
    VehicleBodyOnly,
    PumpJackOilOnly,
}

pub(super) fn build_placement_policy(building: EntityKind) -> BuildPlacementPolicy {
    match building {
        EntityKind::HealingSeal => BuildPlacementPolicy::GroundField,
        EntityKind::TankTrap | EntityKind::BarbedWire => BuildPlacementPolicy::VehicleBodyOnly,
        EntityKind::PumpJack => BuildPlacementPolicy::PumpJackOilOnly,
        _ => BuildPlacementPolicy::AllGround,
    }
}
