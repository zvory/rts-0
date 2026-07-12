use crate::game::entity::{EntityKind, EntityStore};
use crate::rules;

use super::guards::dedupe_cap_units;

pub(super) fn set(
    entities: &mut EntityStore,
    faction_id: &str,
    player: u32,
    buildings: Vec<u32>,
    unit: EntityKind,
    enabled: bool,
    max_buildings: usize,
) {
    for building in dedupe_cap_units(buildings, max_buildings) {
        let eligible = matches!(entities.get(building), Some(producer)
            if producer.owner == player
                && producer.is_building()
                && !producer.under_construction()
                && (!enabled
                    || rules::economy::trainable_units_for_faction(faction_id, producer.kind)
                        .contains(&unit)));
        if !eligible {
            continue;
        }
        if let Some(producer) = entities.get_mut(building) {
            if enabled {
                producer.set_repeat_production(Some(unit));
            } else if producer.repeat_production() == Some(unit) {
                producer.set_repeat_production(None);
            }
        }
    }
}
