//! Compatibility shim for balance constants and stat helpers.
//!
//! `rts-rules` owns these values. `client/src/config.js` mirrors the UI/render/fog subset
//! (costs, supply, sight, sizes, colors), so keep that mirror in sync with player-facing balance
//! edits.

pub use rts_rules::balance::*;
pub use rts_rules::balance::{
    ARTILLERY_UNLOCK_COST_OIL, ARTILLERY_UNLOCK_COST_STEEL, ARTILLERY_UNLOCK_RESEARCH_TICKS,
    AT_GUN_UNLOCK_COST_OIL, AT_GUN_UNLOCK_COST_STEEL, AT_GUN_UNLOCK_RESEARCH_TICKS,
    METHAMPHETAMINES_ATTACK_COOLDOWN_DENOMINATOR, METHAMPHETAMINES_ATTACK_COOLDOWN_NUMERATOR,
    METHAMPHETAMINES_COST_OIL, METHAMPHETAMINES_COST_STEEL, METHAMPHETAMINES_RESEARCH_TICKS,
    TANK_UNLOCK_COST_OIL, TANK_UNLOCK_COST_STEEL, TANK_UNLOCK_RESEARCH_TICKS,
};

pub const MORTAR_TURN_RATE_RAD_PER_TICK: f32 = 0.035 * 3.0;
pub const MORTAR_FIRE_TOLERANCE_RAD: f32 = 15.0_f32.to_radians();
