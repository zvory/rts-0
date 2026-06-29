use crate::game::entity::EntityKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuildPlacementPolicy {
    AllGround,
    VehicleBodyOnly,
    PumpJackOilOnly,
}

pub(super) fn build_placement_policy(building: EntityKind) -> BuildPlacementPolicy {
    match building {
        EntityKind::TankTrap => BuildPlacementPolicy::VehicleBodyOnly,
        EntityKind::PumpJack => BuildPlacementPolicy::PumpJackOilOnly,
        _ => BuildPlacementPolicy::AllGround,
    }
}
