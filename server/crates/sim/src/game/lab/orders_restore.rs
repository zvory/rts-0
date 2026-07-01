use std::collections::HashMap;

use crate::game::entity::{Entity, EntityKind, EntityStore, Order};
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::order_execution::targeting::{
    stored_artillery_point_fire_target, ArtilleryPointFireAcceptance,
};
use crate::game::services::order_execution::{
    start_artillery_fire_command_order, ArtilleryFireMode,
};
use crate::game::services::pathing::PathingService;
use crate::game::teams::TeamRelations;

use super::super::scenario::{LabScenarioEntity, LabScenarioOrder};
use super::super::{validate_world_position, LabError};
use super::{
    queued_order_from_scenario, required_ability, required_build, required_point, required_target,
    ORDER_ATTACK, ORDER_ATTACK_MOVE, ORDER_BLANKET_FIRE, ORDER_BUILD, ORDER_DECONSTRUCT,
    ORDER_GATHER, ORDER_HOLD_POSITION, ORDER_IDLE, ORDER_MOVE, ORDER_POINT_FIRE,
    ORDER_WORLD_ABILITY,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::game::lab) fn restore_lab_entity_orders(
    map: &Map,
    entities: &mut EntityStore,
    pathing: &mut PathingService,
    tick: u32,
    teams: &TeamRelations,
    scenario_entity: &LabScenarioEntity,
    restored_id: u32,
    id_map: &HashMap<u32, u32>,
) -> Result<(), LabError> {
    if scenario_entity.order.is_none() && scenario_entity.queued_orders.is_empty() {
        return Ok(());
    }
    let Some(restored) = entities.get(restored_id) else {
        return Ok(());
    };
    if !restored.is_unit() {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} kind {} cannot have orders",
                scenario_entity.id, scenario_entity.kind
            ),
        });
    }
    if let Some(order) = scenario_entity.order.as_ref() {
        restore_active_order_from_scenario(
            map,
            entities,
            pathing,
            tick,
            teams,
            scenario_entity,
            restored_id,
            order,
            id_map,
        )?;
    }
    if !scenario_entity.queued_orders.is_empty() {
        if let Some(restored) = entities.get_mut(restored_id) {
            restored.clear_queued_orders();
        }
    }
    for order in &scenario_entity.queued_orders {
        let intent = queued_order_from_scenario(map, scenario_entity, order, id_map)?;
        if !entities
            .get_mut(restored_id)
            .is_some_and(|restored| restored.append_queued_order(intent))
        {
            return Err(LabError::InvalidScenario {
                reason: format!("entity {} cannot accept queued order", scenario_entity.id),
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn restore_active_order_from_scenario(
    map: &Map,
    entities: &mut EntityStore,
    pathing: &mut PathingService,
    tick: u32,
    teams: &TeamRelations,
    entity: &LabScenarioEntity,
    restored_id: u32,
    order: &LabScenarioOrder,
    id_map: &HashMap<u32, u32>,
) -> Result<(), LabError> {
    match order.kind.as_str() {
        ORDER_IDLE => {
            with_restored_entity_mut(entities, restored_id, entity)?.clear_active_order();
        }
        ORDER_HOLD_POSITION => {
            with_restored_entity_mut(entities, restored_id, entity)?
                .replace_active_order(Order::HoldPosition);
        }
        ORDER_MOVE => {
            let (x, y) = required_point(map, entity, order)?;
            let restored = with_restored_entity_mut(entities, restored_id, entity)?;
            restored.replace_active_order(Order::move_to(x, y));
            restored.set_path_goal(Some((x, y)));
            restored.reset_gather_state();
            restored.reset_stuck(restored.pos_x, restored.pos_y);
        }
        ORDER_ATTACK_MOVE => {
            let (x, y) = required_point(map, entity, order)?;
            let restored = with_restored_entity_mut(entities, restored_id, entity)?;
            restored.replace_active_order(Order::attack_move_to(x, y));
            restored.set_path_goal(Some((x, y)));
            restored.reset_gather_state();
            restored.reset_stuck(restored.pos_x, restored.pos_y);
        }
        ORDER_ATTACK => {
            let target = required_target(entity, order, id_map)?;
            let target_pos = target_position(entities, entity, order, target)?;
            let restored = with_restored_entity_mut(entities, restored_id, entity)?;
            restored.replace_active_order(Order::attack(target));
            restored.set_target_id(Some(target));
            restored.set_path_goal(Some(target_pos));
            restored.reset_gather_state();
            restored.reset_stuck(restored.pos_x, restored.pos_y);
        }
        ORDER_GATHER => {
            let target = required_target(entity, order, id_map)?;
            let target_pos = target_position(entities, entity, order, target)?;
            let restored = with_restored_entity_mut(entities, restored_id, entity)?;
            restored.replace_active_order(Order::gather(target));
            restored.set_target_id(Some(target));
            restored.set_path_goal(Some(target_pos));
            restored.clear_worker_carry();
            restored.reset_stuck(restored.pos_x, restored.pos_y);
        }
        ORDER_BUILD => {
            let (kind, tile_x, tile_y) = required_build(entity, order, map)?;
            restore_active_build_order(
                map,
                entities,
                pathing,
                tick,
                teams,
                entity,
                restored_id,
                kind,
                tile_x,
                tile_y,
            )?;
        }
        ORDER_DECONSTRUCT => {
            let target = required_target(entity, order, id_map)?;
            restore_active_deconstruct_order(
                map,
                entities,
                pathing,
                tick,
                teams,
                entity,
                restored_id,
                target,
            )?;
        }
        ORDER_WORLD_ABILITY => {
            let ability = required_ability(entity, order)?;
            let (x, y) = required_point(map, entity, order)?;
            let staging_x = order.staging_x.unwrap_or(x);
            let staging_y = order.staging_y.unwrap_or(y);
            validate_world_position(map, staging_x, staging_y)?;
            let restored = with_restored_entity_mut(entities, restored_id, entity)?;
            restored.replace_active_order(Order::ability(ability, x, y, staging_x, staging_y));
            restored.set_path_goal(Some((staging_x, staging_y)));
            restored.reset_gather_state();
            restored.reset_stuck(restored.pos_x, restored.pos_y);
        }
        ORDER_POINT_FIRE => {
            let (x, y) = required_point(map, entity, order)?;
            restore_active_artillery_fire(
                map,
                entities,
                entity,
                restored_id,
                x,
                y,
                ArtilleryFireMode::Point,
            )?;
        }
        ORDER_BLANKET_FIRE => {
            let (x, y) = required_point(map, entity, order)?;
            restore_active_artillery_fire(
                map,
                entities,
                entity,
                restored_id,
                x,
                y,
                ArtilleryFireMode::Blanket,
            )?;
        }
        _ => {
            return Err(LabError::InvalidScenario {
                reason: format!("entity {} has unsupported order kind {:?}", entity.id, order.kind),
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn restore_active_build_order(
    map: &Map,
    entities: &mut EntityStore,
    pathing: &mut PathingService,
    tick: u32,
    teams: &TeamRelations,
    scenario_entity: &LabScenarioEntity,
    restored_id: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Result<(), LabError> {
    let occ = Occupancy::build(map, entities);
    let mut coordinator = MoveCoordinator::new_with_teams(pathing, map, &occ, tick, teams.clone());
    if coordinator.order_build(entities, restored_id, kind, tile_x, tile_y) {
        Ok(())
    } else {
        Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} cannot restore active build order for {kind}",
                scenario_entity.id
            ),
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn restore_active_deconstruct_order(
    map: &Map,
    entities: &mut EntityStore,
    pathing: &mut PathingService,
    tick: u32,
    teams: &TeamRelations,
    scenario_entity: &LabScenarioEntity,
    restored_id: u32,
    target: u32,
) -> Result<(), LabError> {
    let occ = Occupancy::build(map, entities);
    let mut coordinator = MoveCoordinator::new_with_teams(pathing, map, &occ, tick, teams.clone());
    if coordinator.order_deconstruct(entities, restored_id, target) {
        Ok(())
    } else {
        Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} cannot restore active deconstruct order",
                scenario_entity.id
            ),
        })
    }
}

fn restore_active_artillery_fire(
    map: &Map,
    entities: &mut EntityStore,
    scenario_entity: &LabScenarioEntity,
    restored_id: u32,
    x: f32,
    y: f32,
    mode: ArtilleryFireMode,
) -> Result<(), LabError> {
    let owner = entities
        .get(restored_id)
        .map(|entity| entity.owner)
        .ok_or_else(|| LabError::InvalidScenario {
            reason: format!("entity {} was not restored", scenario_entity.id),
        })?;
    let Some(target) = stored_artillery_point_fire_target(
        map,
        entities,
        owner,
        restored_id,
        x,
        y,
        ArtilleryPointFireAcceptance::Command,
    ) else {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} cannot restore active artillery fire order",
                scenario_entity.id
            ),
        });
    };
    if start_artillery_fire_command_order(entities, restored_id, target, mode) {
        Ok(())
    } else {
        Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} cannot start active artillery fire order",
                scenario_entity.id
            ),
        })
    }
}

fn with_restored_entity_mut<'a>(
    entities: &'a mut EntityStore,
    restored_id: u32,
    scenario_entity: &LabScenarioEntity,
) -> Result<&'a mut Entity, LabError> {
    entities.get_mut(restored_id).ok_or_else(|| LabError::InvalidScenario {
        reason: format!("entity {} was not restored", scenario_entity.id),
    })
}

fn target_position(
    entities: &EntityStore,
    entity: &LabScenarioEntity,
    order: &LabScenarioOrder,
    target: u32,
) -> Result<(f32, f32), LabError> {
    entities
        .get(target)
        .map(|target| (target.pos_x, target.pos_y))
        .ok_or_else(|| LabError::InvalidScenario {
            reason: format!(
                "entity {} order {:?} references missing restored target {}",
                entity.id, order.kind, target
            ),
        })
}
