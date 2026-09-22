//! Economy and resource-node balance constants.

pub const STARTING_STEEL: u32 = 75;
pub const STARTING_OIL: u32 = 0;
pub const STARTING_WORKERS: u32 = 1;
pub const STARTING_RIFLEMEN: u32 = 4;
pub const STARTING_STEEL_MINES: u32 = 6;
pub const STARTING_PUMP_JACKS: u32 = 2;
/// Free depot construction remains independent of paid Engineer construction.
pub const AUTOMATIC_PUMP_JACK_BUILD_TICKS: u32 = super::TICK_HZ * 18;

pub const STEEL_LOAD: u32 = 2;
pub const OIL_LOAD: u32 = 1;
pub const HARVEST_TICKS: u32 = 40;
pub const STEEL_PATCH_AMOUNT: u32 = 625;
// Twelve steel patches and six oil patches yield the same 2.599:1 Steel/Oil base ratio as the
// former three-patch layout while splitting its oil capacity into smoother, smaller sources.
pub const OIL_GEYSER_AMOUNT: u32 = 481;
pub const STEEL_PATCHES_PER_BASE: u32 = 12;
pub const OIL_PATCHES_PER_BASE: u32 = 6;

pub const START_RESOURCE_MIN_DIST_TILES: f32 = 3.5;
pub const START_RESOURCE_MAX_DIST_TILES: f32 = 7.0;
// Four-tile mining buffer beyond authored/base resource cluster placement.
pub const MINING_ANCHOR_RANGE_TILES: f32 = 11.0;
pub const STEEL_BLOCK_DIST_TILES: f32 = 4.0;
pub const OIL_DIST_TILES: f32 = 6.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoother_oil_keeps_the_established_base_capacity_and_opening_rate() {
        assert_eq!(OIL_PATCHES_PER_BASE * OIL_GEYSER_AMOUNT, 2_886);
        assert_eq!(STARTING_PUMP_JACKS * OIL_LOAD, 2);
        assert_eq!(OIL_GEYSER_AMOUNT / OIL_LOAD, 481);
    }
}
