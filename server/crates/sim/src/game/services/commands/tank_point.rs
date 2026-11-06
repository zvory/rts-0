use crate::game::entity::{EntityKind, EntityStore};
use rts_rules::math::GameplayMath as _;

pub(super) fn heading(
    entities: &EntityStore,
    player: u32,
    units: &[u32],
    x: f32,
    y: f32,
) -> Option<(Vec<u32>, f32)> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let mut tanks: Vec<_> = units
        .iter()
        .copied()
        .filter(|id| {
            entities
                .get(*id)
                .is_some_and(|e| e.owner == player && e.kind == EntityKind::Tank && e.hp > 0)
        })
        .collect();
    tanks.sort_unstable();
    if tanks.is_empty() {
        return None;
    }
    let (sx, sy) = tanks
        .iter()
        .filter_map(|id| entities.get(*id))
        .fold((0.0_f64, 0.0_f64), |(x, y), e| {
            (x + e.pos_x() as f64, y + e.pos_y() as f64)
        });
    let dx = x as f64 - sx / tanks.len() as f64;
    let dy = y as f64 - sy / tanks.len() as f64;
    if dx.det_hypot(dy) < 0.001 {
        return None;
    }
    let facing = dy.det_atan2(dx) as f32;
    Some((tanks, facing))
}
