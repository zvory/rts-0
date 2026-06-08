use std::collections::HashMap;

use crate::config;
use crate::game::ability::{self, AbilityKind, AbilityTargetMode};
use crate::game::entity::{EntityStore, MovePhase, Order};
use crate::game::map::Map;
use crate::game::services::commands::{notice, notice_positioned};
use crate::game::services::dist2;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::world_query;
use crate::game::smoke::SmokeCloudStore;
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbilityOrderResult {
    Launched,
    Moving,
    Skipped,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn order_or_launch_world_ability(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    coordinator: &mut MoveCoordinator<'_>,
    smokes: &mut SmokeCloudStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    caster: u32,
    ability: AbilityKind,
    x: f32,
    y: f32,
    tick: u32,
    emit_resource_notice: bool,
) -> AbilityOrderResult {
    let Some((x, y)) = SmokeCloudStore::clamp_point_to_map(map, x, y) else {
        return AbilityOrderResult::Skipped;
    };
    if !caster_can_attempt(entities, player, caster, ability) {
        return AbilityOrderResult::Skipped;
    }
    if !tech_requirement_met(entities, player, ability) {
        return AbilityOrderResult::Skipped;
    }
    if caster_in_range(map, entities, caster, ability, x, y) {
        if launch_world_ability(
            map,
            entities,
            players,
            smokes,
            events,
            player,
            caster,
            ability,
            x,
            y,
            tick,
            emit_resource_notice,
        ) {
            return AbilityOrderResult::Launched;
        }
        return AbilityOrderResult::Skipped;
    }

    let Some(staging) = staging_point(map, entities, caster, ability, x, y) else {
        return AbilityOrderResult::Skipped;
    };
    coordinator.order_ability(entities, caster, ability, (x, y), staging);
    AbilityOrderResult::Moving
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn launch_world_ability(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    smokes: &mut SmokeCloudStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    caster: u32,
    ability: AbilityKind,
    x: f32,
    y: f32,
    tick: u32,
    emit_resource_notice: bool,
) -> bool {
    let Some((x, y)) = SmokeCloudStore::clamp_point_to_map(map, x, y) else {
        return false;
    };
    if !caster_can_attempt(entities, player, caster, ability)
        || !tech_requirement_met(entities, player, ability)
        || !caster_in_range(map, entities, caster, ability, x, y)
    {
        return false;
    }
    let definition = ability::definition(ability);
    if definition.target_mode != AbilityTargetMode::WorldPoint {
        return false;
    }
    let Some(ps) = players.iter_mut().find(|p| p.id == player) else {
        return false;
    };
    if ps.steel < definition.cost.steel || ps.oil < definition.cost.oil {
        if emit_resource_notice {
            notice(
                events,
                player,
                rules::economy::resource_shortage_notice(
                    ps.steel,
                    ps.oil,
                    definition.cost.steel,
                    definition.cost.oil,
                ),
            );
        }
        return false;
    }

    match ability {
        AbilityKind::Charge => false,
        AbilityKind::Smoke => {
            let Some(e) = entities.get_mut(caster) else {
                return false;
            };
            if !e.consume_ability_use(ability) {
                return false;
            }
            ps.steel = ps.steel.saturating_sub(definition.cost.steel);
            ps.oil = ps.oil.saturating_sub(definition.cost.oil);
            e.start_ability_cooldown(ability, definition.cooldown_ticks);
            e.clear_active_order();
            smokes.spawn(
                x,
                y,
                config::SMOKE_CLOUD_RADIUS_TILES,
                config::SMOKE_CLOUD_DURATION_TICKS,
                tick,
            );
            notice_positioned(
                events,
                player,
                "Smoke",
                crate::protocol::NoticeSeverity::Info,
                x,
                y,
            );
            true
        }
    }
}

pub(crate) fn caster_can_attempt(
    entities: &EntityStore,
    player: u32,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    matches!(entities.get(caster),
        Some(e) if e.owner == player
            && e.hp > 0
            && e.is_unit()
            && !e.under_construction()
            && ability::carried_by(ability, e.kind)
            && e.ability_uses_remaining(ability).unwrap_or(1) > 0
            && e.ability_cooldown_ticks(ability) == 0)
}

pub(crate) fn tech_requirement_met(
    entities: &EntityStore,
    player: u32,
    ability: AbilityKind,
) -> bool {
    match ability::definition(ability).tech_requirement {
        Some(required) => {
            world_query::completed_building_kinds(entities, player).contains(&required)
        }
        None => true,
    }
}

pub(crate) fn caster_in_range(
    map: &Map,
    entities: &EntityStore,
    caster: u32,
    ability: AbilityKind,
    x: f32,
    y: f32,
) -> bool {
    let Some(e) = entities.get(caster) else {
        return false;
    };
    let Some(range_tiles) = ability::definition(ability).range_tiles else {
        return true;
    };
    if SmokeCloudStore::clamp_point_to_map(map, x, y).is_none() {
        return false;
    }
    let range_px = range_tiles as f32 * config::TILE_SIZE as f32;
    dist2(e.pos_x, e.pos_y, x, y) <= range_px * range_px
}

fn staging_point(
    map: &Map,
    entities: &EntityStore,
    caster: u32,
    ability: AbilityKind,
    x: f32,
    y: f32,
) -> Option<(f32, f32)> {
    let caster = entities.get(caster)?;
    let range_tiles = ability::definition(ability).range_tiles?;
    let range_px = range_tiles as f32 * config::TILE_SIZE as f32;
    let dx = caster.pos_x - x;
    let dy = caster.pos_y - y;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() {
        return None;
    }
    let margin = (caster.radius() * 0.25).max(1.0);
    let staging_distance = (range_px - margin).max(0.0);
    let (sx, sy) = if len <= f32::EPSILON {
        (caster.pos_x, caster.pos_y)
    } else {
        (
            x + dx / len * staging_distance,
            y + dy / len * staging_distance,
        )
    };
    SmokeCloudStore::clamp_point_to_map(map, sx, sy)
}

pub(crate) fn active_ability_order_ready(
    order: &Order,
) -> Option<(AbilityKind, f32, f32, MovePhase)> {
    match order {
        Order::Ability(order) => Some((
            order.intent.ability,
            order.intent.x,
            order.intent.y,
            order.execution.phase,
        )),
        _ => None,
    }
}
