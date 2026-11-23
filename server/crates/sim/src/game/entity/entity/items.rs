use super::{Entity, Order};

impl Entity {
    /// Consume a delivered escape charge and reset all movement at the validated Shop exit.
    pub(in crate::game) fn rescue_shop_escape(&mut self, x: f32, y: f32, hp: u32) {
        self.escape_treasure = None;
        self.escape_autocast_disabled = false;
        self.hp = hp.max(1);
        self.clear_orders();
        self.replace_active_order(Order::Idle);
        self.reset_stuck(x, y);
        self.set_position(x, y);
    }
}
