use std::collections::HashMap;
use std::str::FromStr;

use crate::game::ability::AbilityKind;
use crate::game::entity::{Entity, EntityKind, Order, OrderIntent};
use crate::game::map::Map;

use super::scenario::{LabScenarioEntity, LabScenarioOrder};
use super::{validate_world_position, LabError};

#[path = "orders_restore.rs"]
mod restore;

pub(super) use restore::restore_lab_entity_orders;

const ORDER_IDLE: &str = "idle";
const ORDER_HOLD_POSITION: &str = "holdPosition";
const ORDER_MOVE: &str = "move";
const ORDER_ATTACK_MOVE: &str = "attackMove";
const ORDER_ATTACK: &str = "attack";
const ORDER_GATHER: &str = "gather";
const ORDER_BUILD: &str = "build";
const ORDER_DECONSTRUCT: &str = "deconstruct";
const ORDER_WORLD_ABILITY: &str = "worldAbility";
const ORDER_SELF_ABILITY: &str = "selfAbility";
const ORDER_SETUP_ANTI_TANK_GUNS: &str = "setupAntiTankGuns";
const ORDER_POINT_FIRE: &str = "pointFire";
const ORDER_BLANKET_FIRE: &str = "blanketFire";

pub(super) fn lab_entity_active_order(entity: &Entity) -> Option<LabScenarioOrder> {
    scenario_order_from_active(&entity.order())
}

pub(super) fn lab_entity_queued_orders(entity: &Entity) -> Vec<LabScenarioOrder> {
    entity
        .queued_orders()
        .iter()
        .map(scenario_order_from_intent)
        .collect()
}

pub(super) fn clear_lab_production_state(entity: &mut Entity) {
    if let Some(production) = entity.production.as_mut() {
        production.queue.clear();
        production.research_queue.clear();
        production.rally_point = None;
        production.rally_queue.clear();
    }
}

pub(super) fn order_references_entity(order: &Order, entity_id: u32) -> bool {
    order.attack_target() == Some(entity_id)
        || order.gather_node() == Some(entity_id)
        || order.build_site() == Some(entity_id)
        || order.deconstruct_target() == Some(entity_id)
}

pub(super) fn order_intent_references_entity(intent: &OrderIntent, entity_id: u32) -> bool {
    match intent {
        OrderIntent::Attack(target) => target.target == entity_id,
        OrderIntent::Gather(gather) => gather.node == entity_id,
        OrderIntent::Deconstruct(target) => target.target == entity_id,
        _ => false,
    }
}

fn scenario_order_from_active(order: &Order) -> Option<LabScenarioOrder> {
    match order {
        Order::Idle => None,
        Order::HoldPosition => Some(empty_order(ORDER_HOLD_POSITION)),
        Order::Move(order) => Some(point_order(ORDER_MOVE, order.intent.x, order.intent.y)),
        Order::AttackMove(order) => {
            Some(point_order(ORDER_ATTACK_MOVE, order.intent.x, order.intent.y))
        }
        Order::Attack(order) => Some(target_order(ORDER_ATTACK, order.intent.target)),
        Order::Gather(order) => Some(target_order(ORDER_GATHER, order.intent.node)),
        Order::Build(order) => Some(build_order(
            order.intent.kind,
            order.intent.tile_x,
            order.intent.tile_y,
        )),
        Order::Deconstruct(order) => Some(target_order(ORDER_DECONSTRUCT, order.intent.target)),
        Order::Ability(order) => {
            let mut scenario_order = point_order(
                ORDER_WORLD_ABILITY,
                order.intent.x,
                order.intent.y,
            );
            scenario_order.ability = Some(order.intent.ability.to_protocol_str().to_string());
            scenario_order.staging_x = Some(order.execution.staging.x);
            scenario_order.staging_y = Some(order.execution.staging.y);
            Some(scenario_order)
        }
        Order::ArtilleryPointFire(order) => {
            Some(point_order(ORDER_POINT_FIRE, order.intent.x, order.intent.y))
        }
        Order::ArtilleryBlanketFire(order) => {
            Some(point_order(ORDER_BLANKET_FIRE, order.intent.x, order.intent.y))
        }
    }
}

fn scenario_order_from_intent(intent: &OrderIntent) -> LabScenarioOrder {
    match intent {
        OrderIntent::Move(point) => point_order(ORDER_MOVE, point.x, point.y),
        OrderIntent::AttackMove(point) => point_order(ORDER_ATTACK_MOVE, point.x, point.y),
        OrderIntent::Attack(target) => target_order(ORDER_ATTACK, target.target),
        OrderIntent::Gather(gather) => target_order(ORDER_GATHER, gather.node),
        OrderIntent::Build(build) => build_order(build.kind, build.tile_x, build.tile_y),
        OrderIntent::Deconstruct(target) => target_order(ORDER_DECONSTRUCT, target.target),
        OrderIntent::WorldAbility(ability) => {
            let mut order = point_order(ORDER_WORLD_ABILITY, ability.x, ability.y);
            order.ability = Some(ability.ability.to_protocol_str().to_string());
            order
        }
        OrderIntent::SelfAbility(ability) => {
            let mut order = empty_order(ORDER_SELF_ABILITY);
            order.ability = Some(ability.ability.to_protocol_str().to_string());
            order
        }
        OrderIntent::SetupAntiTankGuns(point) => {
            point_order(ORDER_SETUP_ANTI_TANK_GUNS, point.x, point.y)
        }
        OrderIntent::PointFire(point) => point_order(ORDER_POINT_FIRE, point.x, point.y),
        OrderIntent::BlanketFire(point) => point_order(ORDER_BLANKET_FIRE, point.x, point.y),
    }
}

fn queued_order_from_scenario(
    map: &Map,
    entity: &LabScenarioEntity,
    order: &LabScenarioOrder,
    id_map: &HashMap<u32, u32>,
) -> Result<OrderIntent, LabError> {
    Ok(match order.kind.as_str() {
        ORDER_MOVE => {
            let (x, y) = required_point(map, entity, order)?;
            OrderIntent::move_to(x, y)
        }
        ORDER_ATTACK_MOVE => {
            let (x, y) = required_point(map, entity, order)?;
            OrderIntent::attack_move_to(x, y)
        }
        ORDER_ATTACK => OrderIntent::attack(required_target(entity, order, id_map)?),
        ORDER_GATHER => OrderIntent::gather(required_target(entity, order, id_map)?),
        ORDER_BUILD => {
            let (kind, tile_x, tile_y) = required_build(entity, order, map)?;
            OrderIntent::build(kind, tile_x, tile_y)
        }
        ORDER_DECONSTRUCT => OrderIntent::deconstruct(required_target(entity, order, id_map)?),
        ORDER_WORLD_ABILITY => {
            let ability = required_ability(entity, order)?;
            let (x, y) = required_point(map, entity, order)?;
            OrderIntent::ability(ability, x, y)
        }
        ORDER_SELF_ABILITY => OrderIntent::self_ability(required_ability(entity, order)?),
        ORDER_SETUP_ANTI_TANK_GUNS => {
            let (x, y) = required_point(map, entity, order)?;
            OrderIntent::setup_anti_tank_guns(x, y)
        }
        ORDER_POINT_FIRE => {
            let (x, y) = required_point(map, entity, order)?;
            OrderIntent::point_fire(x, y)
        }
        ORDER_BLANKET_FIRE => {
            let (x, y) = required_point(map, entity, order)?;
            OrderIntent::blanket_fire(x, y)
        }
        _ => {
            return Err(LabError::InvalidScenario {
                reason: format!(
                    "entity {} has unsupported queued order kind {:?}",
                    entity.id, order.kind
                ),
            })
        }
    })
}

fn empty_order(kind: &str) -> LabScenarioOrder {
    LabScenarioOrder {
        kind: kind.to_string(),
        x: None,
        y: None,
        target: None,
        entity_kind: None,
        tile_x: None,
        tile_y: None,
        ability: None,
        staging_x: None,
        staging_y: None,
    }
}

fn point_order(kind: &str, x: f32, y: f32) -> LabScenarioOrder {
    LabScenarioOrder {
        x: Some(x),
        y: Some(y),
        ..empty_order(kind)
    }
}

fn target_order(kind: &str, target: u32) -> LabScenarioOrder {
    LabScenarioOrder {
        target: Some(target),
        ..empty_order(kind)
    }
}

fn build_order(kind: EntityKind, tile_x: u32, tile_y: u32) -> LabScenarioOrder {
    LabScenarioOrder {
        entity_kind: Some(kind.to_string()),
        tile_x: Some(tile_x),
        tile_y: Some(tile_y),
        ..empty_order(ORDER_BUILD)
    }
}

fn required_point(
    map: &Map,
    entity: &LabScenarioEntity,
    order: &LabScenarioOrder,
) -> Result<(f32, f32), LabError> {
    let x = required_f32(entity, order, "x", order.x)?;
    let y = required_f32(entity, order, "y", order.y)?;
    validate_world_position(map, x, y)?;
    Ok((x, y))
}

fn required_target(
    entity: &LabScenarioEntity,
    order: &LabScenarioOrder,
    id_map: &HashMap<u32, u32>,
) -> Result<u32, LabError> {
    let old_id = order.target.ok_or_else(|| LabError::InvalidScenario {
        reason: format!("entity {} order {:?} is missing target", entity.id, order.kind),
    })?;
    id_map.get(&old_id).copied().ok_or_else(|| LabError::InvalidScenario {
        reason: format!(
            "entity {} order {:?} references missing target {}",
            entity.id, order.kind, old_id
        ),
    })
}

fn required_build(
    entity: &LabScenarioEntity,
    order: &LabScenarioOrder,
    map: &Map,
) -> Result<(EntityKind, u32, u32), LabError> {
    let kind_id = order.entity_kind.as_deref().ok_or_else(|| LabError::InvalidScenario {
        reason: format!("entity {} build order is missing entityKind", entity.id),
    })?;
    let kind = EntityKind::from_str(kind_id).map_err(|_| LabError::InvalidKind {
        kind: kind_id.to_string(),
        operation: "restoreScenario",
    })?;
    let tile_x = order.tile_x.ok_or_else(|| LabError::InvalidScenario {
        reason: format!("entity {} build order is missing tileX", entity.id),
    })?;
    let tile_y = order.tile_y.ok_or_else(|| LabError::InvalidScenario {
        reason: format!("entity {} build order is missing tileY", entity.id),
    })?;
    if tile_x >= map.size || tile_y >= map.size {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} build order tile ({tile_x},{tile_y}) is outside the map",
                entity.id
            ),
        });
    }
    Ok((kind, tile_x, tile_y))
}

fn required_ability(
    entity: &LabScenarioEntity,
    order: &LabScenarioOrder,
) -> Result<AbilityKind, LabError> {
    let ability = order.ability.as_deref().ok_or_else(|| LabError::InvalidScenario {
        reason: format!("entity {} order {:?} is missing ability", entity.id, order.kind),
    })?;
    AbilityKind::from_str(ability).map_err(|_| LabError::InvalidScenario {
        reason: format!(
            "entity {} order {:?} has unknown ability {:?}",
            entity.id, order.kind, ability
        ),
    })
}

fn required_f32(
    entity: &LabScenarioEntity,
    order: &LabScenarioOrder,
    field: &'static str,
    value: Option<f32>,
) -> Result<f32, LabError> {
    value.ok_or_else(|| LabError::InvalidScenario {
        reason: format!(
            "entity {} order {:?} is missing {field}",
            entity.id, order.kind
        ),
    })
}
