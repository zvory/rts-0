use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};

pub(super) struct PumpJackPayout {
    pub(super) owner: u32,
    pub(super) oil: u32,
}

pub(super) fn tick(entities: &mut EntityStore) -> Vec<PumpJackPayout> {
    let pump_ids: Vec<u32> = entities
        .iter()
        .filter(|e| {
            e.kind == EntityKind::PumpJack
                && e.hp > 0
                && !e.under_construction()
                && e.resource_extractor.is_some()
        })
        .map(|e| e.id)
        .collect();
    let mut payouts = Vec::new();

    for pump_id in pump_ids {
        let Some(owner) = entities.get(pump_id).map(|pump| pump.owner) else {
            continue;
        };
        let Some(node_id) = pump_jack_oil_node(entities, pump_id) else {
            if let Some(extractor) = entities
                .get_mut(pump_id)
                .and_then(|pump| pump.resource_extractor.as_mut())
            {
                extractor.progress = 0;
            }
            continue;
        };

        let ready = match entities
            .get_mut(pump_id)
            .and_then(|pump| pump.resource_extractor.as_mut())
        {
            Some(extractor) => {
                extractor.progress = extractor.progress.saturating_add(1);
                extractor.progress >= config::HARVEST_TICKS
            }
            None => false,
        };
        if !ready {
            continue;
        }

        let taken = entities
            .get_mut(node_id)
            .map(|node| node.harvest_resources(config::OIL_LOAD))
            .unwrap_or(0);
        if let Some(extractor) = entities
            .get_mut(pump_id)
            .and_then(|pump| pump.resource_extractor.as_mut())
        {
            extractor.progress = 0;
        }
        if taken > 0 {
            payouts.push(PumpJackPayout { owner, oil: taken });
        }
    }

    payouts
}

fn pump_jack_oil_node(entities: &EntityStore, pump_id: u32) -> Option<u32> {
    let pump = entities.get(pump_id)?;
    if pump.kind != EntityKind::PumpJack {
        return None;
    }
    let rect = building_rect(pump)?;
    entities
        .iter()
        .filter(|node| {
            node.kind == EntityKind::Oil && node.is_node() && node.remaining().unwrap_or(0) > 0
        })
        .find(|node| {
            circle_intersects_rect(
                Circle {
                    x: node.pos_x,
                    y: node.pos_y,
                    radius: config::TILE_SIZE as f32 * 0.5,
                },
                rect,
            )
        })
        .map(|node| node.id)
}

#[derive(Clone, Copy)]
struct Rect {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

#[derive(Clone, Copy)]
struct Circle {
    x: f32,
    y: f32,
    radius: f32,
}

fn building_rect(entity: &Entity) -> Option<Rect> {
    let stats = config::building_stats(entity.kind)?;
    let tile_size = config::TILE_SIZE as f32;
    let half_w = stats.foot_w as f32 * tile_size * 0.5;
    let half_h = stats.foot_h as f32 * tile_size * 0.5;
    Some(Rect {
        min_x: entity.pos_x - half_w,
        min_y: entity.pos_y - half_h,
        max_x: entity.pos_x + half_w,
        max_y: entity.pos_y + half_h,
    })
}

fn circle_intersects_rect(circle: Circle, rect: Rect) -> bool {
    let closest_x = circle.x.clamp(rect.min_x, rect.max_x);
    let closest_y = circle.y.clamp(rect.min_y, rect.max_y);
    let dx = circle.x - closest_x;
    let dy = circle.y - closest_y;
    dx * dx + dy * dy <= circle.radius * circle.radius
}
