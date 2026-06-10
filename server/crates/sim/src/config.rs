//! Compatibility shim for balance constants and stat helpers.
//!
//! `rts-rules` owns these values. `client/src/config.js` mirrors the UI/render/fog subset
//! (costs, supply, sight, sizes, colors), so keep that mirror in sync with player-facing balance
//! edits.

pub use rts_rules::balance::*;
pub use rts_rules::balance::{
    AT_GUN_UNLOCK_COST_OIL, AT_GUN_UNLOCK_COST_STEEL, AT_GUN_UNLOCK_RESEARCH_TICKS,
    METHAMPHETAMINES_ATTACK_COOLDOWN_DENOMINATOR, METHAMPHETAMINES_ATTACK_COOLDOWN_NUMERATOR,
    METHAMPHETAMINES_COST_OIL, METHAMPHETAMINES_COST_STEEL, METHAMPHETAMINES_RESEARCH_TICKS,
    TANK_UNLOCK_COST_OIL, TANK_UNLOCK_COST_STEEL, TANK_UNLOCK_RESEARCH_TICKS,
};
