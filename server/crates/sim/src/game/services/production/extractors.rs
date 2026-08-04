use crate::config;
use crate::game::entity::{EntityKind, EntityStore};

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
    if scaffold_id(entities, producer_id, kind).is_some() {
        return true;
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
        true
    });
    if !associated {
        entities.remove(scaffold_id);
        return false;
    }
    // A destroyed in-progress scaffold restarts the already-paid build from zero.
    entities
        .get_mut(producer_id)
        .is_some_and(|producer| producer.set_front_production_progress(0))
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
