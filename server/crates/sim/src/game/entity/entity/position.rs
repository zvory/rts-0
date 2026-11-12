use super::Entity;
use rts_rules::math::Coord;

impl Entity {
    /// Current authoritative mortar maximum range; the five-tile dead zone is independent.
    pub(crate) fn mortar_range_tiles(&self) -> f32 {
        let ticks = self
            .combat
            .as_ref()
            .map_or(0, |c| c.mortar_stationary_range_ticks);
        let ramp = crate::config::MORTAR_STATIONARY_RANGE_RAMP_TICKS;
        crate::config::MORTAR_RANGE_TILES as f32 * ticks.min(ramp) as f32 / ramp as f32
    }

    /// World-coordinate projection for geometry and snapshots; storage stays fixed point.
    pub fn pos_x(&self) -> f32 {
        self.pos_x.to_world()
    }
    pub fn pos_y(&self) -> f32 {
        self.pos_y.to_world()
    }

    /// Quantize at every authoritative write, including spawn, collision, pull, and teleport.
    /// Invalid coordinates leave the entity in place rather than poisoning the room.
    pub fn set_position(&mut self, x: f32, y: f32) {
        if let (Some(x), Some(y)) = (Coord::try_from_world(x), Coord::try_from_world(y)) {
            if self.kind == crate::game::entity::EntityKind::MortarTeam
                && (self.pos_x != x || self.pos_y != y)
            {
                if let Some(combat) = self.combat.as_mut() {
                    combat.mortar_stationary_range_ticks = 0;
                    combat.mortar_range_reset_this_tick = true;
                }
            }
            self.pos_x = x;
            self.pos_y = y;
        }
    }
}

#[cfg(test)]
mod tests;
