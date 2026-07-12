use crate::config;
use crate::game::entity::{EntityStore, ProdItem, ResearchItem};
use crate::game::production_request::{ProductionRequest, ProductionRequestItem};
use crate::game::upgrade;
use crate::game::PlayerState;
use crate::rules::{self, economy::ResourceCost};

#[derive(Clone, Copy)]
enum Assessment {
    Ready(ResourceCost),
    ResourceBlocked(ResourceCost),
    OtherBlocked,
    Invalid,
}

pub(crate) fn run_scheduler(entities: &mut EntityStore, players: &mut [PlayerState]) {
    for player_index in 0..players.len() {
        remove_invalid_requests(entities, &mut players[player_index]);

        let requests = players[player_index]
            .production_requests
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let stock = (players[player_index].steel, players[player_index].oil);
        let mut protected_costs = Vec::new();
        let mut selected = None;
        for (index, request) in requests.iter().enumerate() {
            match assess(request, entities, &players[player_index]) {
                Assessment::Ready(cost)
                    if preserves_earlier_deficits(stock, cost, &protected_costs) =>
                {
                    selected = Some((index, request.clone()));
                    break;
                }
                Assessment::ResourceBlocked(cost) => protected_costs.push(cost),
                Assessment::Ready(_) | Assessment::OtherBlocked | Assessment::Invalid => {}
            }
        }
        let Some((index, request)) = selected else {
            continue;
        };
        if start_request(entities, players, player_index, &request) {
            rotate_after_start(&mut players[player_index], index);
        }
    }
}

fn remove_invalid_requests(entities: &EntityStore, player: &mut PlayerState) {
    let invalid_indices = player
        .production_requests
        .iter()
        .enumerate()
        .filter_map(|(index, request)| {
            matches!(assess(request, entities, player), Assessment::Invalid).then_some(index)
        })
        .collect::<Vec<_>>();
    for index in invalid_indices.into_iter().rev() {
        player.production_requests.remove(index);
    }
}

fn assess(request: &ProductionRequest, entities: &EntityStore, player: &PlayerState) -> Assessment {
    match request.item {
        ProductionRequestItem::Unit { building, unit } => {
            let Some(producer) = entities.get(building) else {
                return Assessment::Invalid;
            };
            let valid_producer = producer.owner == player.id
                && producer.is_building()
                && !producer.under_construction()
                && rules::economy::trainable_units_for_faction(&player.faction_id, producer.kind)
                    .contains(&unit)
                && config::unit_stats(unit).is_some();
            if !valid_producer {
                return Assessment::Invalid;
            }
            if !producer.prod_queue().is_empty() || !producer.research_queue().is_empty() {
                return Assessment::OtherBlocked;
            }
            let supply = rules::economy::supply_cost(unit);
            let supply_ready = player
                .supply_used
                .checked_add(supply)
                .is_some_and(|used| used <= player.supply_cap);
            if !supply_ready {
                return Assessment::OtherBlocked;
            }
            affordability(player, rules::economy::resource_cost(unit))
        }
        ProductionRequestItem::Research { building, upgrade } => {
            if player.upgrades.contains(&upgrade) {
                return Assessment::Invalid;
            }
            let definition = upgrade::definition(upgrade);
            let Some(producer) = entities.get(building) else {
                return Assessment::Invalid;
            };
            let valid_producer = producer.owner == player.id
                && producer.is_building()
                && !producer.under_construction()
                && producer.kind == definition.researched_at
                && rules::economy::can_research_for_faction(
                    &player.faction_id,
                    upgrade.to_protocol_str(),
                    producer.kind,
                );
            if !valid_producer {
                return Assessment::Invalid;
            }
            if !producer.prod_queue().is_empty() || !producer.research_queue().is_empty() {
                return Assessment::OtherBlocked;
            }
            affordability(
                player,
                ResourceCost::new(definition.cost_steel, definition.cost_oil),
            )
        }
    }
}

fn affordability(player: &PlayerState, cost: ResourceCost) -> Assessment {
    if player.can_afford(cost.steel, cost.oil) {
        Assessment::Ready(cost)
    } else {
        Assessment::ResourceBlocked(cost)
    }
}

fn preserves_earlier_deficits(
    stock: (u32, u32),
    candidate: ResourceCost,
    earlier: &[ResourceCost],
) -> bool {
    let after = (
        stock.0.saturating_sub(candidate.steel),
        stock.1.saturating_sub(candidate.oil),
    );
    earlier.iter().all(|cost| {
        cost.steel.saturating_sub(after.0) <= cost.steel.saturating_sub(stock.0)
            && cost.oil.saturating_sub(after.1) <= cost.oil.saturating_sub(stock.1)
    })
}

fn start_request(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player_index: usize,
    request: &ProductionRequest,
) -> bool {
    match request.item {
        ProductionRequestItem::Unit { building, unit } => {
            let Some(stats) = config::unit_stats(unit) else {
                return false;
            };
            let cost = rules::economy::resource_cost(unit);
            let supply = rules::economy::supply_cost(unit);
            if !players[player_index].spend_cost(cost) {
                return false;
            }
            if !players[player_index].reserve_supply(supply) {
                players[player_index].refund_cost(cost);
                return false;
            }
            let started = entities.get_mut(building).is_some_and(|entity| {
                entity.push_production(ProdItem {
                    unit,
                    progress: 0,
                    total: stats.build_ticks,
                })
            });
            if !started {
                players[player_index].refund_cost(cost);
                players[player_index].release_supply(supply);
            }
            started
        }
        ProductionRequestItem::Research { building, upgrade } => {
            let definition = upgrade::definition(upgrade);
            let cost = ResourceCost::new(definition.cost_steel, definition.cost_oil);
            if !players[player_index].spend_cost(cost) {
                return false;
            }
            let started = entities.get_mut(building).is_some_and(|entity| {
                entity.push_research(ResearchItem {
                    upgrade,
                    progress: 0,
                    total: definition.research_ticks,
                })
            });
            if !started {
                players[player_index].refund_cost(cost);
            }
            started
        }
    }
}

fn rotate_after_start(player: &mut PlayerState, index: usize) {
    let Some(mut request) = player.production_requests.remove(index) else {
        return;
    };
    match request.remaining {
        Some(0 | 1) => {}
        Some(remaining) => {
            request.remaining = Some(remaining - 1);
            player.production_requests.push_back(request);
        }
        None => player.production_requests.push_back(request),
    }
}

pub(crate) fn cancel_latest_for_producer(player: &mut PlayerState, building: u32) -> bool {
    let Some(index) = player
        .production_requests
        .iter()
        .rposition(|request| request.item.producer_id() == building)
    else {
        return false;
    };
    player.production_requests.remove(index).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_spend_may_use_a_resource_the_earlier_request_does_not_lack() {
        let stock = (100, 0);
        let earlier = [ResourceCost::new(75, 25)];

        assert!(preserves_earlier_deficits(
            stock,
            ResourceCost::new(25, 0),
            &earlier,
        ));
    }

    #[test]
    fn later_spend_cannot_create_or_increase_an_earlier_deficit() {
        let earlier = [ResourceCost::new(75, 25)];

        assert!(!preserves_earlier_deficits(
            (90, 0),
            ResourceCost::new(25, 0),
            &earlier,
        ));
        assert!(!preserves_earlier_deficits(
            (50, 0),
            ResourceCost::new(25, 0),
            &earlier,
        ));
    }
}
