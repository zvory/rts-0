//! Entities and their storage. See `docs/design/server-sim.md` (`entity`).
//!
//! An [`Entity`] is the single mutable record for any unit, building, or resource node
//! in the simulation. The simulation services (`services/`) read and mutate these records
//! every tick; the snapshot layer (`game::mod`) projects them into `protocol::EntityView`.
//!
//! Storage is an [`EntityStore`]: a `HashMap<u32, Entity>` keyed by a stable, monotonically
//! increasing id. Ids are never reused, so a stale id (an entity that has died) simply
//! misses the map — every lookup is fallible and the tick loop tolerates misses (no panics).

#[allow(clippy::module_inception)]
mod entity;
mod kind;
mod order;
mod state;
mod store;

#[cfg(test)]
mod tests;

pub use entity::Entity;
pub use kind::EntityKind;
pub(crate) use kind::{
    fires_while_moving, uses_car_movement_semantics, uses_oriented_vehicle_body,
    uses_pivot_vehicle_movement,
};
#[allow(unused_imports)]
pub use order::{
    AttackExecution, AttackOrder, AttackPhase, BuildExecution, BuildIntent, BuildOrder, BuildPhase,
    GatherExecution, GatherIntent, GatherOrder, GatherPhase, MoveExecution, MoveOrder, MovePhase,
    Order, OrderIntent, PointIntent, TargetIntent, MAX_QUEUED_ORDERS,
};
#[cfg(test)]
pub use state::EntityStateGroups;
#[allow(unused_imports)]
pub use state::{
    CarryState, CombatState, ConstructionState, MovementState, ProdItem, ProductionState,
    ResourceNodeState, WeaponSetup, WorkerState,
};
pub use store::EntityStore;

/// Neutral owner id used for resource nodes (steel / oil nodes).
pub const NEUTRAL: u32 = 0;
