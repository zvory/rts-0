use std::collections::HashMap;

use crate::config;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore, ProdItem, ResearchItem};
use crate::game::map::Map;
use crate::game::production_request::{
    ProductionRequest, ProductionRequestItem, MAX_PRODUCTION_REQUESTS,
};
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::{standability, world_query};
use crate::game::upgrade::{self, UpgradeKind};
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules::{self, economy::ResourceCost};

pub(crate) fn enqueue_unit(
    entities: &EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
    unit: EntityKind,
    quantity: u32,
    automatic: bool,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let Some(ps) = players.iter().find(|candidate| candidate.id == player) else {
        return;
    };
    let faction_id = ps.faction_id.clone();
    let valid_producer = matches!(entities.get(building), Some(entity)
        if entity.owner == player
            && entity.is_building()
            && !entity.under_construction()
            && rules::economy::trainable_units_for_faction(&faction_id, entity.kind).contains(&unit));
    if !valid_producer {
        notice(events, player, "Cannot train that here");
        return;
    }
    let owned = world_query::completed_building_kinds(entities, player);
    if !rules::economy::train_requirement_met_for_faction(&faction_id, unit, &owned)
        || upgrade::required_for_unit(unit).is_some_and(|required| !ps.upgrades.contains(&required))
    {
        notice(events, player, "Requirement not met");
        return;
    }
    if config::unit_stats(unit).is_none() {
        notice(events, player, "Unknown unit");
        return;
    }
    let item = ProductionRequestItem::Unit { building, unit };
    let request = if automatic {
        ProductionRequest::automatic(item)
    } else {
        ProductionRequest::finite(item, quantity)
    };
    push_request(players, player, request, events);
}

fn notice(events: &mut HashMap<u32, Vec<Event>>, player: u32, msg: &str) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
        x: None,
        y: None,
        severity: crate::protocol::NoticeSeverity::Info,
    });
}

pub(crate) fn enqueue_research(
    entities: &EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
    upgrade: UpgradeKind,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let definition = upgrade::definition(upgrade);
    let Some(ps) = players.iter().find(|candidate| candidate.id == player) else {
        return;
    };
    let valid_producer = matches!(entities.get(building), Some(entity)
    if entity.owner == player
        && entity.is_building()
        && !entity.under_construction()
        && entity.kind == definition.researched_at
        && rules::economy::can_research_for_faction(
            &ps.faction_id,
            upgrade.to_protocol_str(),
            entity.kind,
        ));
    if !valid_producer {
        notice(events, player, "Cannot research that here");
        return;
    }
    let already_requested = ps.production_requests.iter().any(|request| {
        matches!(request.item, ProductionRequestItem::Research { upgrade: queued, .. } if queued == upgrade)
    });
    let already_active = entities.iter().any(|entity| {
        entity.owner == player
            && entity
                .research_queue()
                .iter()
                .any(|item| item.upgrade == upgrade)
    });
    if ps.upgrades.contains(&upgrade) || already_requested || already_active {
        notice(events, player, "Already researched or queued");
        return;
    }
    if definition
        .requires_upgrade
        .is_some_and(|required| !ps.upgrades.contains(&required))
    {
        notice(events, player, "Requirement not met");
        return;
    }
    push_request(
        players,
        player,
        ProductionRequest::finite(ProductionRequestItem::Research { building, upgrade }, 1),
        events,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn enqueue_building(
    map: &Map,
    entities: &EntityStore,
    players: &mut [PlayerState],
    player: u32,
    units: Vec<u32>,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    queued: bool,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let Some(ps) = players.iter().find(|candidate| candidate.id == player) else {
        return;
    };
    let valid_worker = units.iter().copied().any(|id| {
        matches!(entities.get(id), Some(entity)
            if entity.owner == player
                && entity.is_unit()
                && rules::economy::can_build_for_faction(&ps.faction_id, entity.kind, building))
    });
    if !valid_worker {
        notice(events, player, "Only workers can build");
        return;
    }
    let owned = world_query::completed_building_kinds(entities, player);
    if !rules::economy::build_requirement_met_for_faction(&ps.faction_id, building, &owned) {
        notice(events, player, "Requirement not met");
        return;
    }
    if config::building_stats(building).is_none() || tile_x >= map.size || tile_y >= map.size {
        notice(events, player, "Cannot build there");
        return;
    }
    let placement_valid = units.iter().copied().any(|worker| {
        matches!(
            standability::building_site_status_for_build_intent(
                map, entities, building, tile_x, tile_y, worker,
            ),
            standability::BuildSiteStatus::Clear | standability::BuildSiteStatus::BlockedByUnit
        )
    });
    if !placement_valid {
        notice(events, player, "Cannot build there");
        return;
    }
    push_request(
        players,
        player,
        ProductionRequest::finite(
            ProductionRequestItem::Building {
                units,
                building,
                tile_x,
                tile_y,
                queued,
            },
            1,
        ),
        events,
    );
}

fn push_request(
    players: &mut [PlayerState],
    player: u32,
    request: ProductionRequest,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let Some(ps) = players.iter_mut().find(|candidate| candidate.id == player) else {
        return;
    };
    if ps.production_requests.len() >= MAX_PRODUCTION_REQUESTS {
        notice(events, player, "Production queue full");
        return;
    }
    ps.production_requests.push_back(request);
}

#[derive(Clone, Copy)]
enum Assessment {
    Ready(ResourceCost),
    ResourceBlocked(ResourceCost),
    OtherBlocked,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_scheduler(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    coordinator: &mut MoveCoordinator<'_>,
) {
    let player_ids = players.iter().map(|player| player.id).collect::<Vec<_>>();
    for player_id in player_ids {
        let Some(player_index) = players.iter().position(|player| player.id == player_id) else {
            continue;
        };
        let requests = players[player_index]
            .production_requests
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let stock = (players[player_index].steel, players[player_index].oil);
        let mut protected_costs = Vec::new();
        let mut selected = None;
        for (index, request) in requests.iter().enumerate() {
            match assess(request, map, entities, &players[player_index]) {
                Assessment::Ready(cost)
                    if preserves_earlier_deficits(stock, cost, &protected_costs) =>
                {
                    selected = Some((index, request.clone()));
                    break;
                }
                Assessment::ResourceBlocked(cost) => protected_costs.push(cost),
                Assessment::Ready(_) | Assessment::OtherBlocked => {}
            }
        }
        let Some((index, request)) = selected else {
            continue;
        };
        if start_request(map, entities, players, player_index, coordinator, &request) {
            rotate_after_start(&mut players[player_index], index);
        }
    }
}

fn assess(
    request: &ProductionRequest,
    map: &Map,
    entities: &EntityStore,
    player: &PlayerState,
) -> Assessment {
    match request.item {
        ProductionRequestItem::Unit { building, unit } => {
            let producer_ready = matches!(entities.get(building), Some(entity)
                if entity.owner == player.id
                    && !entity.under_construction()
                    && entity.prod_queue().is_empty()
                    && entity.research_queue().is_empty());
            let supply = rules::economy::supply_cost(unit);
            let supply_ready = player
                .supply_used
                .checked_add(supply)
                .is_some_and(|used| used <= player.supply_cap);
            if !producer_ready || !supply_ready {
                return Assessment::OtherBlocked;
            }
            affordability(player, rules::economy::resource_cost(unit))
        }
        ProductionRequestItem::Research { building, upgrade } => {
            let producer_ready = matches!(entities.get(building), Some(entity)
                if entity.owner == player.id
                    && !entity.under_construction()
                    && entity.prod_queue().is_empty()
                    && entity.research_queue().is_empty())
                && !player.upgrades.contains(&upgrade);
            if !producer_ready {
                return Assessment::OtherBlocked;
            }
            let definition = upgrade::definition(upgrade);
            affordability(
                player,
                ResourceCost::new(definition.cost_steel, definition.cost_oil),
            )
        }
        ProductionRequestItem::Building {
            ref units,
            building,
            tile_x,
            tile_y,
            ..
        } => {
            let worker_ready = units.iter().copied().any(|id| {
                matches!(entities.get(id), Some(entity)
                    if entity.owner == player.id
                        && !matches!(entity.build_phase(), Some(BuildPhase::Constructing { .. })))
            });
            let site_ready = units.iter().copied().any(|worker| {
                matches!(
                    standability::building_site_status_for_build_intent(
                        map, entities, building, tile_x, tile_y, worker,
                    ),
                    standability::BuildSiteStatus::Clear
                        | standability::BuildSiteStatus::BlockedByUnit
                )
            });
            if worker_ready && site_ready {
                Assessment::Ready(ResourceCost::new(0, 0))
            } else {
                Assessment::OtherBlocked
            }
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
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player_index: usize,
    coordinator: &mut MoveCoordinator<'_>,
    request: &ProductionRequest,
) -> bool {
    let player_id = players[player_index].id;
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
        ProductionRequestItem::Building {
            ref units,
            building,
            tile_x,
            tile_y,
            queued,
        } => {
            let _ = queued;
            let Some(worker) = units.iter().copied().find(|id| {
                matches!(entities.get(*id), Some(entity)
                    if entity.owner == player_id
                        && !matches!(entity.build_phase(), Some(BuildPhase::Constructing { .. })))
                    && matches!(
                        standability::building_site_status_for_build_intent(
                            map, entities, building, tile_x, tile_y, *id,
                        ),
                        standability::BuildSiteStatus::Clear
                            | standability::BuildSiteStatus::BlockedByUnit
                    )
            }) else {
                return false;
            };
            coordinator.order_build(entities, worker, building, tile_x, tile_y)
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
