//! Manual front-entry payment and the remaining resource budget protected from repeat production.
use super::*;
use crate::rules::economy::ResourceCost;
use std::collections::HashMap;

pub(super) fn pay_and_protect(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    pending: &mut HashMap<u32, ResourceCost>,
) {
    if players.iter().all(|player| player.is_ai) {
        return;
    }
    for id in entities.ids() {
        let active_producer = entities.get(id).is_some_and(|producer| {
            producer.hp > 0 && producer.is_building() && !producer.under_construction()
        });
        if entities.get(id).is_some_and(|building| {
            players
                .iter()
                .any(|player| player.id == building.owner && player.is_ai)
        }) {
            continue;
        }
        if !active_producer {
            continue;
        }
        pay_front(entities, players, id);
        // Claim a paid extractor's patch before another front can pay against it.
        // Scaffolds used to be created between payments in the per-producer loop.
        extractors::ensure_scaffold(entities, id);

        let Some(building) = entities.get(id) else {
            continue;
        };
        let research_cost = building
            .research_queue()
            .first()
            .filter(|item| !item.paid)
            .map(|item| {
                let definition = upgrade::definition(item.upgrade);
                ResourceCost::new(definition.cost_steel, definition.cost_oil)
            })
            .unwrap_or(ResourceCost::new(0, 0));
        let unit_cost = building
            .prod_queue()
            .first()
            .filter(|item| !item.paid)
            .filter(|item| {
                !item.unit.is_resource_extractor()
                    || extractors::target(entities, id, item.unit).is_some()
            })
            .map(|item| rules::economy::resource_cost(item.unit))
            .unwrap_or(ResourceCost::new(0, 0));
        let total = pending
            .entry(building.owner)
            .or_insert(ResourceCost::new(0, 0));
        *total = ResourceCost::new(
            total
                .steel
                .saturating_add(research_cost.steel)
                .saturating_add(unit_cost.steel),
            total
                .oil
                .saturating_add(research_cost.oil)
                .saturating_add(unit_cost.oil),
        );
    }
}

// AI repeat scheduling retains its existing per-producer payment order.
pub(super) fn pay_front(entities: &mut EntityStore, players: &mut [PlayerState], id: u32) {
    let active_producer = entities.get(id).is_some_and(|producer| {
        producer.hp > 0 && producer.is_building() && !producer.under_construction()
    });
    if let Some((owner, research)) =
        entities
            .get(id)
            .filter(|_| active_producer)
            .and_then(|building| {
                building
                    .research_queue()
                    .first()
                    .filter(|item| !item.paid)
                    .map(|item| (building.owner, item.upgrade))
            })
    {
        let definition = upgrade::definition(research);
        let cost = rules::economy::ResourceCost::new(definition.cost_steel, definition.cost_oil);
        if let Some(player) = players.iter_mut().find(|player| player.id == owner) {
            if player.spend_cost(cost)
                && !entities
                    .get_mut(id)
                    .is_some_and(|building| building.mark_front_research_paid())
            {
                player.refund_cost(cost);
            }
        }
    }
    if let Some((owner, unit)) = entities
        .get(id)
        .filter(|_| active_producer)
        .and_then(|building| {
            building
                .prod_queue()
                .first()
                .filter(|item| !item.paid)
                .map(|item| (building.owner, item.unit))
        })
    {
        if unit.is_resource_extractor() && extractors::target(entities, id, unit).is_none() {
            return;
        }
        let cost = rules::economy::resource_cost(unit);
        let supply = rules::economy::supply_cost(unit);
        if let Some(player) = players.iter_mut().find(|player| player.id == owner) {
            let supply_available = player.can_reserve_supply(supply);
            if supply_available && player.spend_cost(cost) {
                if player.reserve_supply(supply) {
                    if !entities
                        .get_mut(id)
                        .is_some_and(|building| building.mark_front_production_paid())
                    {
                        player.refund_cost(cost);
                        player.release_supply(supply);
                    }
                } else {
                    player.refund_cost(cost);
                }
            }
        }
    }
}
