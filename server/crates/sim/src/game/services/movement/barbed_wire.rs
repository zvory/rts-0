//! Tile-entry hazards. Phase-local positions avoid adding checkpoint or replay state.
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;
use std::collections::BTreeMap;

pub(super) fn tiles(map: &Map, entities: &EntityStore) -> BTreeMap<(u32, u32), u32> {
    entities
        .iter()
        .filter(|e| e.kind == EntityKind::BarbedWire && e.hp > 0 && !e.under_construction())
        .map(|e| (map.tile_of(e.pos_x(), e.pos_y()), e.id))
        .collect()
}

pub(super) fn positions(map: &Map, entities: &EntityStore) -> BTreeMap<u32, (u32, u32)> {
    if !entities
        .iter()
        .any(|e| e.kind == EntityKind::BarbedWire && e.hp > 0 && !e.under_construction())
    {
        return BTreeMap::new();
    }
    entities
        .iter()
        .filter(|e| e.is_unit() && e.kind != EntityKind::ScoutPlane && e.hp > 0)
        .map(|e| (e.id, map.tile_of(e.pos_x(), e.pos_y())))
        .collect()
}

/// Returns whether death cleanup is required. Sorted ids resolve simultaneous entries deterministically.
pub(super) fn apply_entries(
    map: &Map,
    entities: &mut EntityStore,
    before: &BTreeMap<u32, (u32, u32)>,
) -> bool {
    if before.is_empty() {
        return false;
    }
    let wire = tiles(map, entities);
    let mut killed = false;
    for (&id, &previous) in before {
        let Some(unit) = entities.get(id).filter(|e| e.hp > 0) else {
            continue;
        };
        let current = map.tile_of(unit.pos_x(), unit.pos_y());
        if previous == current {
            continue;
        }
        let Some(&wire_id) = wire.get(&current) else {
            continue;
        };
        let hurts = rts_rules::barbed_wire_affects(unit.kind);
        let Some(segment) = entities.get_mut(wire_id).filter(|e| e.hp > 0) else {
            continue;
        };
        segment.consume_wire_crossing();
        killed |= segment.hp == 0;
        if hurts {
            if let Some(unit) = entities.get_mut(id) {
                unit.apply_damage(10, None);
                killed |= unit.hp == 0;
            }
        }
    }
    killed
}
