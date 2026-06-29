use crate::config;
use crate::game::entity::{Entity, EntityKind};

pub(crate) const STATIONARY_RANGE_MAX_TILES: f32 = 14.0;
pub(crate) const STATIONARY_RANGE_RAMP_TICKS: u16 = config::TICK_HZ as u16 * 3;

pub(crate) fn effective_range_tiles(entity: &Entity, base_range_tiles: f32) -> f32 {
    if entity.kind != EntityKind::Tank {
        return base_range_tiles;
    }
    let ramp_ticks = STATIONARY_RANGE_RAMP_TICKS.max(1);
    let ticks = entity.tank_stationary_range_ticks().min(ramp_ticks);
    if ticks == 0 {
        return base_range_tiles;
    }
    let progress = ticks as f32 / ramp_ticks as f32;
    base_range_tiles + (STATIONARY_RANGE_MAX_TILES - base_range_tiles) * progress
}
