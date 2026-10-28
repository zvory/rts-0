//! Player supply cap.

pub const STARTING_SUPPLY_CAP: u32 = 100;
pub const INDUSTRIAL_CENTER_SUPPLY: u32 = 100;
pub const INDUSTRIAL_CENTER_LIMIT: usize = 2;
pub const INDUSTRIAL_CENTER_COST_STEEL: u32 = 800;
pub const INDUSTRIAL_CENTER_COST_OIL: u32 = 300;
pub const INDUSTRIAL_CENTER_RESEARCH_TICKS: u32 = super::TICK_HZ * 60;

/// Absolute global supply ceiling.
pub const PLAYER_SUPPLY_CAP: u32 = 300;
