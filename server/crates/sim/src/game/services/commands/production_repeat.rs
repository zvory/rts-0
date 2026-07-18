use crate::game::entity::{EntityKind, EntityStore};
use crate::rules;

use super::guards::dedupe_units;

pub(super) fn adjust(
    entities: &mut EntityStore,
    faction_id: &str,
    player: u32,
    buildings: Vec<u32>,
    unit: EntityKind,
    delta: i8,
    max_buildings: usize,
) {
    if !matches!(delta, -1 | 1) {
        return;
    }

    // Keep live command admission consistent with Lab replay validation: a raw list beyond the
    // selected cap rejects the whole command rather than silently applying its prefix.
    if buildings.len() > max_buildings {
        return;
    }

    let candidate = dedupe_units(buildings).into_iter().filter_map(|building| {
        let producer = entities.get(building)?;
        if producer.owner != player || !producer.is_building() || producer.under_construction() {
            return None;
        }
        let repeat_units = &producer.production.as_ref()?.repeat_units;
        let repeats_unit = repeat_units.contains(&unit);
        if delta > 0 {
            let compatible = rules::economy::trainable_units_for_faction(faction_id, producer.kind)
                .contains(&unit);
            if !compatible || repeats_unit {
                return None;
            }
        } else if !repeats_unit {
            return None;
        }
        Some((repeat_units.len(), building))
    });

    // Additions spread standing orders across the least-loaded compatible producers. Removals
    // peel from the most-loaded producer so it keeps another standing order whenever possible.
    // Opposite id tie-breaks make repeated additions followed by removals naturally reversible.
    let building = if delta > 0 {
        candidate.min_by_key(|&(repeat_count, building)| (repeat_count, building))
    } else {
        candidate.max_by_key(|&(repeat_count, building)| (repeat_count, building))
    }
    .map(|(_, building)| building);

    if let Some(producer) = building.and_then(|building| entities.get_mut(building)) {
        producer.set_repeat_production(Some(unit), delta > 0);
    }
}
