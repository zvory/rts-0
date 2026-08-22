#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct PointPromotionKey {
    pub(super) owner: u32,
    pub(super) attack_move: bool,
    x_bits: u32,
    y_bits: u32,
}

impl PointPromotionKey {
    pub(super) fn new(owner: u32, attack_move: bool, x: f32, y: f32) -> Option<Self> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        Some(PointPromotionKey {
            owner,
            attack_move,
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
        })
    }

    pub(super) fn point(self) -> (f32, f32) {
        (f32::from_bits(self.x_bits), f32::from_bits(self.y_bits))
    }
}
