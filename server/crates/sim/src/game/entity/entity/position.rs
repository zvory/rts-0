use super::Entity;
use rts_rules::math::Coord;

impl Entity {
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
            self.pos_x = x;
            self.pos_y = y;
        }
    }
}

#[cfg(test)]
mod tests;
