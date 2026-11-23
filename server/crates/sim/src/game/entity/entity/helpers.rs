use super::config;
use super::{BUILDING_START_HP_DENOMINATOR, BUILDING_START_HP_NUMERATOR};

pub(super) fn construction_hp_for_progress(max_hp: u32, progress: u32, total: u32) -> u32 {
    if max_hp == 0 {
        return 0;
    }
    if total == 0 || progress >= total {
        return max_hp;
    }
    let start_hp = max_hp
        .saturating_mul(BUILDING_START_HP_NUMERATOR)
        .div_ceil(BUILDING_START_HP_DENOMINATOR)
        .clamp(1, max_hp);
    let remaining_hp = max_hp.saturating_sub(start_hp);
    let gained_hp = (remaining_hp as u64)
        .saturating_mul(progress as u64)
        .checked_div(total as u64)
        .unwrap_or(remaining_hp as u64) as u32;
    start_hp.saturating_add(gained_hp).min(max_hp)
}

impl super::Entity {
    /// The collision/interaction radius in world pixels.
    pub fn radius(&self) -> f32 {
        if let Some(s) = config::unit_stats(self.kind) {
            s.radius
                * if self.dao_master {
                    config::DAO_MASTER_BODY_SCALE
                } else {
                    1.0
                }
        } else if self.is_building() {
            // Footprint half-extent (approx) for range/interaction checks.
            let Some(s) = config::building_stats(self.kind) else {
                return config::TILE_SIZE as f32 * 0.5;
            };
            (s.foot_w.max(s.foot_h) as f32) * config::TILE_SIZE as f32 * 0.5
        } else {
            // Resource node footprint ~1 tile.
            config::TILE_SIZE as f32 * 0.5
        }
    }

    pub fn last_damage_owner(&self) -> Option<u32> {
        self.last_damage_owner
    }

    pub(in crate::game) fn last_damage_entity(&self) -> Option<u32> {
        self.last_damage_entity
    }

    pub fn invulnerable(&self) -> bool {
        self.invulnerable
    }
    pub fn set_invulnerable(&mut self, invulnerable: bool) {
        self.invulnerable = invulnerable;
    }
}
