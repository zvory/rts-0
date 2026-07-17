pub use rts_rules::{
    blocks_line_of_sight, fires_while_moving, movement_body_class, static_blocker_class,
    uses_car_movement_semantics, uses_oriented_vehicle_body, uses_pivot_vehicle_movement,
    EntityKind, MovementBodyClass, StaticBlockerClass,
};

pub(crate) fn supports_manual_emplacement(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::AntiTankGun | EntityKind::MortarTeam | EntityKind::Artillery
    )
}
