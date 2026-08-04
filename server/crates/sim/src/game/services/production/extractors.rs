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
                || extractor_on_node(entities, extractor_kind, node.pos_x, node.pos_y)
            {
                None
            } else {
                Some((node.id, dist2, node.pos_x, node.pos_y))
            }
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(_, _, x, y)| (x, y))
}

fn extractor_on_node(entities: &EntityStore, kind: EntityKind, x: f32, y: f32) -> bool {
    entities.iter().any(|entity| {
        entity.kind == kind
            && entity.hp > 0
            && (entity.pos_x - x).abs() <= 0.001
            && (entity.pos_y - y).abs() <= 0.001
    })
}
