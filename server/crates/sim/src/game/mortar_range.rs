use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::services::dist2;

pub(crate) fn tick_stationary_ranges(entities: &mut EntityStore) {
    for id in entities.ids() {
        let Some(entity) = entities.get_mut(id) else {
            continue;
        };
        if entity.kind != EntityKind::MortarTeam || entity.hp == 0 {
            continue;
        }
        if let Some(combat) = entity.combat.as_mut() {
            if combat.mortar_range_reset_this_tick {
                combat.mortar_range_reset_this_tick = false;
            } else {
                combat.mortar_stationary_range_ticks = combat
                    .mortar_stationary_range_ticks
                    .saturating_add(1)
                    .min(config::MORTAR_STATIONARY_RANGE_RAMP_TICKS);
            }
        }
    }
}

pub(crate) fn point_in_current_range(entity: &Entity, x: f32, y: f32) -> bool {
    let distance_sq = dist2(entity.pos_x(), entity.pos_y(), x, y);
    let min = config::MORTAR_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let max = entity.mortar_range_tiles() * config::TILE_SIZE as f32;
    distance_sq >= min * min && distance_sq <= max * max
}
