//! Gameplay domain vocabulary, balance data, and pure rules.
//!
//! This crate deliberately owns no simulation state and imports no protocol, lobby, server,
//! Tokio, or Axum code.

pub mod balance;
pub mod combat;
pub mod defs;
pub mod economy;
pub mod terrain;

mod kind;

pub use kind::{
    fires_while_moving, uses_car_movement_semantics, uses_oriented_vehicle_body,
    uses_pivot_vehicle_movement, EntityKind,
};
