use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::teams::TeamRelations;

const POINT_IN_RECT_EPS_PX: f32 = 0.001;

pub(super) struct PumpJackPayout {
    pub(super) owner: u32,
    pub(super) kind: EntityKind,
    pub(super) amount: u32,
}

pub(super) fn tick(entities: &mut EntityStore, teams: &TeamRelations) -> Vec<PumpJackPayout> {
    let pump_ids: Vec<u32> = entities
        .iter()
        .filter(|e| {
            e.kind.is_resource_extractor()
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
        let Some(node_id) = resource_node(entities, pump_id) else {
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

        let Some((node_kind, taken, depleted)) = entities.get_mut(node_id).map(|node| {
            let amount = if node.kind == EntityKind::Oil {
                config::OIL_LOAD
            } else {
                config::STEEL_LOAD
            };
            let kind = node.kind;
            let taken = node.harvest_resources(amount);
            (kind, taken, node.remaining().unwrap_or(0) == 0)
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
            payouts.push(PumpJackPayout {
                owner,
                kind: node_kind,
                amount: taken,
            });
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

pub(super) fn resource_node(entities: &EntityStore, pump_id: u32) -> Option<u32> {
    let pump = entities.get(pump_id)?;
    let node_kind = match pump.kind {
        EntityKind::SteelMine => EntityKind::Steel,
        EntityKind::PumpJack => EntityKind::Oil,
        _ => return None,
    };
    entities
        .iter()
        .filter(|node| {
            node.kind == node_kind && node.is_node() && node.remaining().unwrap_or(0) > 0
        })
        .find(|node| {
            (node.pos_x - pump.pos_x).abs() <= POINT_IN_RECT_EPS_PX
                && (node.pos_y - pump.pos_y).abs() <= POINT_IN_RECT_EPS_PX
        })
        .map(|node| node.id)
}
