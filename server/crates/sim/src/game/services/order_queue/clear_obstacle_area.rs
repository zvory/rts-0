use std::collections::BTreeMap;

use crate::game::entity::EntityStore;

use super::PromotedIntent;

#[derive(Default)]
pub(super) struct PromotionGroups(BTreeMap<(u32, u32, u32, u32), Vec<u32>>);

impl PromotionGroups {
    pub(super) fn push(
        &mut self,
        entities: &EntityStore,
        unit: u32,
        anchor: u32,
        center: (f32, f32),
    ) {
        let Some(owner) = entities.get(unit).map(|entity| entity.owner) else {
            return;
        };
        self.0
            .entry((owner, anchor, center.0.to_bits(), center.1.to_bits()))
            .or_default()
            .push(unit);
    }

    pub(super) fn into_groups(self) -> BTreeMap<(u32, u32, u32, u32), Vec<u32>> {
        self.0
    }
}

pub(super) fn promoted(anchor: u32, center_x: f32, center_y: f32) -> Option<PromotedIntent> {
    (center_x.is_finite() && center_y.is_finite()).then_some(PromotedIntent::ClearObstacleArea(
        anchor,
        (center_x, center_y),
    ))
}
