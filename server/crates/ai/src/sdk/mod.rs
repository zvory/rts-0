//! Public authoring surface for custom Rust AI strategies.
//!
//! The SDK is intentionally small. Frames are owned, normalized snapshots of what one player is
//! allowed to know, and action requests still pass through the ordinary simulation command path.

mod frame;
mod strategy;

pub use frame::{
    AiBuildObservation, AiBuildObservationPhase, AiCompletion, AiEconomy, AiEntity, AiEntityState,
    AiFrame, AiHealth, AiMap, AiPlayer, AiProduction, AiRememberedContact, AiResource,
    AiResourceAmount, AiTerrain,
};
pub use rts_rules::faction::UpgradeKind;
pub use rts_rules::EntityKind;
pub use strategy::{AiActionRequest, AiActions, AiStrategy, MAX_ACTIONS_PER_STEP};
