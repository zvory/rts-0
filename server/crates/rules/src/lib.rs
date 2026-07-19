//! Gameplay domain vocabulary, balance data, and pure rules.
//!
//! This crate deliberately owns no simulation state and imports no protocol, lobby, server,
//! Tokio, or Axum code.

pub mod balance;
pub mod combat;
pub mod defs;
pub mod economy;
pub mod faction;
pub mod target;
pub mod terrain;

mod kind;

pub use kind::{
    blocks_line_of_sight, fires_while_moving, is_anti_tank_gun, is_rifle_infantry,
    movement_body_class, static_blocker_class, supports_manual_emplacement,
    uses_car_movement_semantics, uses_oriented_vehicle_body, uses_pivot_vehicle_movement,
    EntityKind, MovementBodyClass, StaticBlockerClass,
};
