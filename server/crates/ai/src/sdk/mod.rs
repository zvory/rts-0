//! Public authoring surface for custom Rust AI strategies.
//!
//! The SDK is intentionally small. Frames are owned, normalized snapshots of what one player is
//! allowed to know, and action requests still pass through the ordinary simulation command path.

pub(crate) mod actions;
mod frame;
mod rulebook;
mod strategy;
mod world_queries;

pub use actions::{
    ActionBlocker, ActionBudgetSnapshot, ActionError, AiActions, ReservationCounts,
    ReservationNamespace, UnitGroup, MAX_ACTIONS_PER_STEP,
};
pub use frame::{
    AiAbilityState, AiBuildObservation, AiBuildObservationPhase, AiCompletion, AiEconomy, AiEntity,
    AiEntityState, AiFrame, AiHealth, AiMap, AiPlayer, AiProduction, AiRememberedContact,
    AiResource, AiResourceAmount, AiSmokeCloud, AiTerrain,
};
pub(crate) use rts_rules::balance::unit_placement_radius;
pub use rts_rules::faction::UpgradeKind;
pub use rts_rules::EntityKind;
pub use rulebook::{AiCost, AiEntityRule, AiFootprint, AiPrerequisites, AiRulebook};
pub use strategy::AiStrategy;
pub(crate) use world_queries::unit_circle_touches_rect;
pub use world_queries::{
    AiTile, AiWorldPoint, KnownBuildSite, KnownBuildSiteBlocker, KnownBuildSiteExclusions,
    KnownResourceState, WorldQueries,
};
