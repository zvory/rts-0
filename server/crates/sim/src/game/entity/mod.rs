//! Entities and their storage. See `docs/design/server-sim.md` (`entity`).
//!
//! An [`Entity`] is the single mutable record for any unit, building, or resource node
//! in the simulation. The simulation services (`services/`) read and mutate these records
//! every tick; the snapshot layer (`game::mod`) projects them into `protocol::EntityView`.
//!
//! Storage is an [`EntityStore`]: a `HashMap<u32, Entity>` keyed by a stable, monotonically
//! increasing id. Ids are never reused, so a stale id (an entity that has died) simply
//! misses the map — every lookup is fallible and the tick loop tolerates misses (no panics).

mod armor_reaction;
#[allow(clippy::module_inception)]
mod entity;
mod kind;
mod order;
mod state;
mod store;

#[cfg(test)]
mod tests;

use crate::config;

pub use entity::Entity;
pub use kind::EntityKind;
pub(crate) use kind::{
    blocks_line_of_sight, fires_while_moving, movement_body_class, static_blocker_class,
    uses_car_movement_semantics, uses_oriented_vehicle_body, uses_pivot_vehicle_movement,
    MovementBodyClass, StaticBlockerClass,
};
pub(crate) use order::{tank_trap_deconstruction_ticks, FootprintRouting};
#[allow(unused_imports)]
pub use order::{
    AbilityExecution, AbilityIntent, AbilityOrder, AttackExecution, AttackOrder, AttackPhase,
    BuildExecution, BuildIntent, BuildOrder, BuildPhase, DeconstructExecution, DeconstructOrder,
    DeconstructPhase, GatherExecution, GatherIntent, GatherOrder, GatherPhase, MoveExecution,
    MoveOrder, MovePhase, Order, OrderIntent, PointIntent, RallyIntent, RallyKind, TargetIntent,
    MAX_QUEUED_ORDERS,
};
#[cfg(test)]
pub use state::EntityStateGroups;
pub(in crate::game) use state::ScoutPlaneState;
#[allow(unused_imports)]
pub use state::{
    CarryState, CombatState, ConstructionState, MovementState, PanzerfaustState, ProdItem,
    ProductionState, ResearchItem, ResourceExtractorState, ResourceNodeState, WeaponSetup,
    WorkerState,
};
pub use store::EntityStore;

/// Neutral owner id used for resource nodes (steel / oil nodes).
pub const NEUTRAL: u32 = 0;

pub(crate) fn active_trench_occupation(entity: &Entity) -> Option<u32> {
    if entity.hp == 0 || !config::is_entrenchment_eligible_infantry(entity.kind) {
        return None;
    }
    entity.movement.as_ref().and_then(|m| m.occupied_trench_id)
}
