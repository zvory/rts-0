use std::collections::HashMap;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::production_request::{
    ProductionRequest, ProductionRequestItem, MAX_PRODUCTION_REQUESTS,
};
use crate::game::services::world_query;
use crate::game::upgrade::{self, UpgradeKind};
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules;

pub(crate) struct UnitRequest {
    pub(crate) player: u32,
    pub(crate) building: u32,
    pub(crate) unit: EntityKind,
    pub(crate) quantity: u32,
    pub(crate) automatic: bool,
}

pub(crate) fn enqueue_unit(
    entities: &EntityStore,
    players: &mut [PlayerState],
    request: UnitRequest,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let UnitRequest {
        player,
        building,
        unit,
        quantity,
        automatic,
    } = request;
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
    if automatic
        && ps.production_requests.iter().any(|request| {
            request.remaining.is_none()
                && matches!(
                    request.item,
                    ProductionRequestItem::Unit {
                        building: queued_building,
                        unit: queued_unit,
                    } if queued_building == building && queued_unit == unit
                )
        })
    {
        notice(events, player, "Already queued automatically");
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

fn notice(events: &mut HashMap<u32, Vec<Event>>, player: u32, msg: &str) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
        x: None,
        y: None,
        severity: crate::protocol::NoticeSeverity::Info,
    });
}
