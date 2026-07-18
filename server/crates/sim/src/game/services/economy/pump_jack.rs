use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::teams::TeamRelations;

const POINT_IN_RECT_EPS_PX: f32 = 0.001;

pub(super) struct PumpJackPayout {
    pub(super) owner: u32,
    pub(super) oil: u32,
}

pub(super) fn tick(entities: &mut EntityStore, teams: &TeamRelations) -> Vec<PumpJackPayout> {
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
        let Some(node_id) = oil_node(entities, pump_id) else {
            let _ = entities.remove(pump_id);
            continue;
        };
        if !super::pump_jack_has_completed_friendly_mining_anchor(entities, teams, owner, node_id) {
            continue;
        }

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

        let Some((taken, depleted)) = entities.get_mut(node_id).map(|node| {
            let taken = node.harvest_resources(config::OIL_LOAD);
            (taken, node.remaining().unwrap_or(0) == 0)
        }) else {
            let _ = entities.remove(pump_id);
            continue;
        };
        if let Some(extractor) = entities
            .get_mut(pump_id)
            .and_then(|pump| pump.resource_extractor.as_mut())
        {
            extractor.progress = 0;
        }
        if taken > 0 {
            payouts.push(PumpJackPayout { owner, oil: taken });
        }
        // A Pump Jack is bound to the oil patch it just extracted from. Remove it in the same
        // tick as the final payout so it cannot retarget another patch in its footprint and both
        // the Pump Jack and depleted patch disappear from the following snapshot.
        if depleted {
            let _ = entities.remove(pump_id);
        }
    }

    payouts
}

pub(super) fn oil_node(entities: &EntityStore, pump_id: u32) -> Option<u32> {
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
        .find(|node| point_inside_rect((node.pos_x, node.pos_y), rect))
        .map(|node| node.id)
}

#[derive(Clone, Copy)]
struct Rect {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
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

fn point_inside_rect(point: (f32, f32), rect: Rect) -> bool {
    point.0 >= rect.min_x - POINT_IN_RECT_EPS_PX
        && point.0 <= rect.max_x + POINT_IN_RECT_EPS_PX
        && point.1 >= rect.min_y - POINT_IN_RECT_EPS_PX
        && point.1 <= rect.max_y + POINT_IN_RECT_EPS_PX
}
