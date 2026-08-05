use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::rules;

const AUTOMATIC_KINDS: [EntityKind; 2] = [EntityKind::SteelMine, EntityKind::PumpJack];

/// Advance both free depot-local extractor jobs independently. Each kind pauses when its matching
/// patches are saturated and waits through its combat restart delay after an extractor is killed.
pub(super) fn advance_automatic(
    entities: &mut EntityStore,
    producer_id: u32,
    faction_id: &str,
    tick: u32,
) -> Vec<(u32, EntityKind)> {
    let Some((owner, producer_kind, active)) = entities.get(producer_id).map(|producer| {
        (
            producer.owner,
            producer.kind,
            producer.hp > 0 && !producer.under_construction(),
        )
    }) else {
        return Vec::new();
    };
    let trainable = rules::economy::trainable_units_for_faction(faction_id, producer_kind);
    if !active || !AUTOMATIC_KINDS.iter().all(|kind| trainable.contains(kind)) {
        return Vec::new();
    }

    let mut completed = Vec::new();
    for kind in AUTOMATIC_KINDS {
        if entities.get(producer_id).is_some_and(|producer| {
            producer
                .automatic_extractor_restart_at(kind)
                .is_some_and(|restart_at| tick < restart_at)
        }) {
            continue;
        }
        // Production runs before death cleanup. A killed extractor must remain authoritative for
        // this tick so a replacement cannot appear before death records the restart deadline.
        if entities.iter().any(|entity| {
            entity.hp == 0
                && entity.kind == kind
                && entity.resource_extractor_producer_id() == Some(producer_id)
        }) {
            continue;
        }
        // Old replay/checkpoint queues remain authoritative until their extractor item finishes;
        // the permanent background job takes over afterward.
        if entities
            .get(producer_id)
            .is_some_and(|producer| producer.prod_queue().iter().any(|item| item.unit == kind))
        {
            continue;
        }
        let scaffold = scaffold_id(entities, producer_id, kind).or_else(|| {
            let (x, y) = target(entities, producer_id, kind)?;
            let scaffold = entities.spawn_building(owner, kind, x, y, false)?;
            let associated = entities.get_mut(scaffold).is_some_and(|entity| {
                let Some(construction) = entity.construction.as_mut() else {
                    return false;
                };
                construction.producer_id = Some(producer_id);
                let Some(extractor) = entity.resource_extractor.as_mut() else {
                    return false;
                };
                extractor.producer_id = Some(producer_id);
                true
            });
            if !associated {
                entities.remove(scaffold);
                return None;
            }
            Some(scaffold)
        });
        if scaffold.is_some_and(|scaffold| {
            entities
                .get_mut(scaffold)
                .and_then(|entity| entity.advance_construction())
                == Some(true)
        }) {
            completed.push((owner, kind));
        }
    }
    completed
}

pub(super) fn ensure_scaffold(entities: &mut EntityStore, producer_id: u32) -> bool {
    let Some((owner, kind, paid)) = entities.get(producer_id).and_then(|producer| {
        producer
            .prod_queue()
            .first()
            .map(|front| (producer.owner, front.unit, front.paid))
    }) else {
        return true;
    };
    if !kind.is_resource_extractor() || !paid {
        return true;
    }
    if let Some(scaffold) = entities.iter().find(|entity| {
        entity.kind == kind && entity.construction_producer_id() == Some(producer_id)
    }) {
        // Death cleanup owns settling a destroyed scaffold. Do not replace or advance it earlier
        // in the tick, or the already-paid production item would survive the destruction.
        return scaffold.hp > 0;
    }
    let Some((x, y)) = target(entities, producer_id, kind) else {
        return false;
    };
    let Some(scaffold_id) = entities.spawn_building(owner, kind, x, y, false) else {
        return false;
    };
    let associated = entities.get_mut(scaffold_id).is_some_and(|scaffold| {
        let Some(construction) = scaffold.construction.as_mut() else {
            return false;
        };
        construction.producer_id = Some(producer_id);
        let Some(extractor) = scaffold.resource_extractor.as_mut() else {
            return false;
        };
        extractor.producer_id = Some(producer_id);
        true
    });
    if !associated {
        entities.remove(scaffold_id);
        return false;
    }
    true
}

pub(super) fn sync_scaffold_progress(entities: &mut EntityStore, producer_id: u32) {
    let Some((kind, progress)) = entities.get(producer_id).and_then(|producer| {
        producer
            .prod_queue()
            .first()
            .filter(|front| front.unit.is_resource_extractor())
            .map(|front| (front.unit, front.progress))
    }) else {
        return;
    };
    let Some(scaffold_id) = scaffold_id(entities, producer_id, kind) else {
        return;
    };
    if let Some(scaffold) = entities.get_mut(scaffold_id) {
        scaffold.set_construction_progress(progress);
    }
}

pub(super) fn scaffold_id(
    entities: &EntityStore,
    producer_id: u32,
    kind: EntityKind,
) -> Option<u32> {
    entities
        .iter()
        .find(|entity| {
            entity.hp > 0
                && entity.kind == kind
                && entity.construction_producer_id() == Some(producer_id)
        })
        .map(|entity| entity.id)
}

pub(super) fn target(
    entities: &EntityStore,
    producer_id: u32,
    extractor_kind: EntityKind,
) -> Option<(f32, f32)> {
    let node_kind = extractor_kind.extracted_resource_kind()?;
    let producer = entities.get(producer_id)?;
    if !crate::rules::economy::trainable_units(producer.kind).contains(&extractor_kind)
        || producer.hp == 0
        || producer.under_construction()
    {
        return None;
    }
    let range = config::MINING_ANCHOR_RANGE_TILES * config::TILE_SIZE as f32;
    let range2 = range * range;
    entities
        .iter()
        .filter(|node| node.kind == node_kind && node.remaining().unwrap_or(0) > 0)
        .filter_map(|node| {
            let dx = node.pos_x - producer.pos_x;
            let dy = node.pos_y - producer.pos_y;
            let dist2 = dx * dx + dy * dy;
            if dist2 > range2 + 0.01
                || entities.node_slot_holder(node.id).is_some()
                || entities.resource_extractor_for_node(node.id).is_some()
            {
                None
            } else {
                Some((node.id, dist2, node.pos_x, node.pos_y))
            }
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(_, _, x, y)| (x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{GatherPhase, Order};
    use crate::rules::faction::{CATALOGS, CURRENT_CATALOG};

    #[test]
    fn direct_miner_reservation_blocks_extractor_target() {
        let mut entities = EntityStore::new();
        let extractor_kind = EntityKind::SteelMine;
        let producer_kind = CURRENT_CATALOG
            .production_anchors
            .iter()
            .copied()
            .find(|kind| crate::rules::economy::trainable_units(*kind).contains(&extractor_kind))
            .expect("catalog should expose an extractor producer");
        let node_kind = extractor_kind
            .extracted_resource_kind()
            .expect("extractor should identify its resource");
        let gatherer_kind = CATALOGS
            .iter()
            .flat_map(|catalog| catalog.gatherers.iter().copied())
            .next()
            .expect("a catalog should expose a direct gatherer");
        let depot = entities
            .spawn_building(1, producer_kind, 64.0, 64.0, true)
            .expect("depot");
        let node = entities
            .spawn_node(node_kind, 128.0, 64.0)
            .expect("steel node");
        let gatherer = entities
            .spawn_unit(2, gatherer_kind, 128.0, 64.0)
            .expect("gatherer");
        {
            let gatherer = entities.get_mut(gatherer).expect("gatherer");
            gatherer.set_order(Order::gather(node));
            gatherer.mark_gather_phase(GatherPhase::Harvesting);
        }
        assert!(entities.claim_miner(node, gatherer));

        assert_eq!(target(&entities, depot, extractor_kind), None);
    }
}
