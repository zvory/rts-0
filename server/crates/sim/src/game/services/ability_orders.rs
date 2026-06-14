use std::collections::HashMap;

use crate::config;
use crate::game::ability::{self, AbilityEffectHook, AbilityKind, AbilityTargetMode};
use crate::game::entity::{EntityKind, EntityStore, MovePhase, Order, WeaponSetup};
use crate::game::fog::Fog;
use crate::game::hero_abilities;
use crate::game::map::Map;
use crate::game::mortar::{mortar_current_facing_ready, rotate_mortar_for_fire, MortarShellStore};
use crate::game::services::commands::{notice, notice_positioned};
use crate::game::services::dist2;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::world_query;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
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
    fog: &Fog,
    teams: &TeamRelations,
    coordinator: &mut MoveCoordinator<'_>,
    smokes: &mut SmokeCloudStore,
    mortar_shells: &mut MortarShellStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    faction_id: &str,
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
    if !caster_allowed_by_faction(entities, faction_id, caster, ability) {
        return AbilityOrderResult::Skipped;
    }
    if !caster_can_accept_order(entities, player, caster, ability) {
        return AbilityOrderResult::Skipped;
    }
    if !tech_requirement_met(entities, player, ability) {
        return AbilityOrderResult::Skipped;
    }
    if caster_in_range(map, entities, caster, ability, x, y) {
        if !caster_can_attempt(entities, player, caster, ability)
            || !world_ability_current_facing_ready(entities, caster, ability, x, y)
        {
            let Some((sx, sy)) = entities.get(caster).map(|e| (e.pos_x, e.pos_y)) else {
                return AbilityOrderResult::Skipped;
            };
            if let Some(e) = entities.get_mut(caster) {
                e.set_order(Order::ability(ability, x, y, sx, sy));
                e.set_target_id(None);
                e.set_path(Vec::new());
                e.set_path_goal(None);
                e.mark_move_phase(MovePhase::Arrived);
            }
            return AbilityOrderResult::Moving;
        }
        if launch_world_ability(
            map,
            entities,
            players,
            fog,
            teams,
            smokes,
            mortar_shells,
            events,
            player,
            faction_id,
            caster,
            ability,
            x,
            y,
            tick,
            false,
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
    fog: &Fog,
    teams: &TeamRelations,
    smokes: &mut SmokeCloudStore,
    mortar_shells: &mut MortarShellStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    faction_id: &str,
    caster: u32,
    ability: AbilityKind,
    x: f32,
    y: f32,
    tick: u32,
    preserve_active_order: bool,
    emit_resource_notice: bool,
) -> bool {
    let Some((x, y)) = SmokeCloudStore::clamp_point_to_map(map, x, y) else {
        return false;
    };
    if !caster_can_attempt(entities, player, caster, ability)
        || !caster_allowed_by_faction(entities, faction_id, caster, ability)
        || !tech_requirement_met(entities, player, ability)
        || !caster_in_range(map, entities, caster, ability, x, y)
        || !world_ability_facing_ready(entities, caster, ability, x, y)
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
    if !ps.can_afford(definition.cost.steel, definition.cost.oil) {
        if emit_resource_notice {
            notice(
                events,
                player,
                rules::economy::resource_shortage_notice_for_cost(
                    ps.steel,
                    ps.oil,
                    definition.cost,
                ),
            );
        }
        return false;
    }

    match (definition.effect_hook, ability) {
        (AbilityEffectHook::DelayedWorld, AbilityKind::MortarFire) => {
            let Some((from_x, from_y)) = entities.get(caster).map(|e| (e.pos_x, e.pos_y)) else {
                return false;
            };
            let Some(e) = entities.get_mut(caster) else {
                return false;
            };
            if !ps.spend_cost(definition.cost) {
                return false;
            }
            e.start_ability_cooldown(ability, definition.cooldown_ticks);
            if !preserve_active_order {
                e.clear_active_order();
            }
            mortar_shells.schedule(
                events, fog, teams, player, caster, from_x, from_y, x, y, tick, false,
            );
            true
        }
        (AbilityEffectHook::DelayedWorld, AbilityKind::Smoke) => {
            let Some(caster_pos) = entities.get(caster).map(|e| (e.pos_x, e.pos_y)) else {
                return false;
            };
            let delay_ticks =
                smoke_launch_delay_ticks(map, caster_pos.0, caster_pos.1, ability, x, y);
            let Some(e) = entities.get_mut(caster) else {
                return false;
            };
            if !e.consume_ability_use(ability) {
                return false;
            }
            if !ps.spend_cost(definition.cost) {
                return false;
            }
            e.start_ability_cooldown(ability, definition.cooldown_ticks);
            if !preserve_active_order {
                e.clear_active_order();
            }
            smokes.schedule(
                x,
                y,
                config::SMOKE_CLOUD_RADIUS_TILES,
                config::SMOKE_CLOUD_DURATION_TICKS,
                tick.saturating_add(delay_ticks),
            );
            for pid in events.keys().copied().collect::<Vec<_>>() {
                if !teams.same_team_or_same_owner(pid, player) {
                    continue;
                }
                events.entry(pid).or_default().push(Event::SmokeLaunch {
                    from_x: caster_pos.0,
                    from_y: caster_pos.1,
                    to_x: x,
                    to_y: y,
                    delay_ticks,
                });
            }
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
        (AbilityEffectHook::Teleport, AbilityKind::EkatTeleport) => {
            let Some((blink_x, blink_y)) = hero_abilities::ekat_teleport_destination(
                map,
                entities,
                caster,
                x,
                y,
                definition.range_tiles,
            ) else {
                return false;
            };
            if !ps.spend_cost(definition.cost) {
                return false;
            }
            let Some(e) = entities.get_mut(caster) else {
                return false;
            };
            e.start_ability_cooldown(ability, definition.cooldown_ticks);
            if !hero_abilities::move_ekat_to(entities, caster, blink_x, blink_y) {
                return false;
            }
            notice_positioned(
                events,
                player,
                "Teleport",
                crate::protocol::NoticeSeverity::Info,
                blink_x,
                blink_y,
            );
            true
        }
        (AbilityEffectHook::LineDamage, AbilityKind::EkatLineShot) => {
            if !ps.spend_cost(definition.cost) {
                return false;
            }
            let Some((target_x, target_y)) = hero_abilities::apply_ekat_line_shot(
                entities,
                teams,
                player,
                caster,
                x,
                y,
                definition.range_tiles,
                tick,
            ) else {
                return false;
            };
            if let Some(e) = entities.get_mut(caster) {
                e.start_ability_cooldown(ability, definition.cooldown_ticks);
                if !preserve_active_order {
                    e.clear_active_order();
                }
            }
            notice_positioned(
                events,
                player,
                "Line Shot",
                crate::protocol::NoticeSeverity::Info,
                target_x,
                target_y,
            );
            true
        }
        _ => false,
    }
}

pub(crate) fn launch_self_ability(
    entities: &mut EntityStore,
    faction_id: &str,
    player: u32,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    if !caster_can_attempt(entities, player, caster, ability)
        || !caster_allowed_by_faction(entities, faction_id, caster, ability)
        || !tech_requirement_met(entities, player, ability)
    {
        return false;
    }
    let definition = ability::definition(ability);
    if definition.target_mode != AbilityTargetMode::SelfTarget {
        return false;
    }
    match (definition.effect_hook, ability) {
        (AbilityEffectHook::SelfStatus, AbilityKind::Charge) => {
            let Some(e) = entities.get_mut(caster) else {
                return false;
            };
            e.start_charge(config::RIFLEMAN_CHARGE_TICKS);
            e.start_ability_cooldown(ability, definition.cooldown_ticks);
            true
        }
        (AbilityEffectHook::OwnedAreaStatus, AbilityKind::Breakthrough) => {
            let Some((caster_x, caster_y)) = entities.get(caster).map(|e| (e.pos_x, e.pos_y))
            else {
                return false;
            };
            let radius_px = config::BREAKTHROUGH_RADIUS_TILES * config::TILE_SIZE as f32;
            let radius2 = radius_px * radius_px;
            for id in entities.ids() {
                let Some(unit) = entities.get(id) else {
                    continue;
                };
                if unit.owner != player
                    || unit.hp == 0
                    || !unit.is_unit()
                    || unit.under_construction()
                {
                    continue;
                }
                if dist2(caster_x, caster_y, unit.pos_x, unit.pos_y) > radius2 {
                    continue;
                }
                if let Some(unit) = entities.get_mut(id) {
                    unit.start_breakthrough(config::BREAKTHROUGH_DURATION_TICKS);
                }
            }
            if let Some(e) = entities.get_mut(caster) {
                e.start_ability_cooldown(ability, definition.cooldown_ticks);
            }
            true
        }
        _ => false,
    }
}

fn smoke_launch_delay_ticks(
    map: &Map,
    caster_x: f32,
    caster_y: f32,
    ability: AbilityKind,
    x: f32,
    y: f32,
) -> u32 {
    let Some(range_tiles) = ability::definition(ability).range_tiles else {
        return 0;
    };
    if SmokeCloudStore::clamp_point_to_map(map, x, y).is_none() {
        return 0;
    }
    let range_px = range_tiles as f32 * config::TILE_SIZE as f32;
    if range_px <= f32::EPSILON {
        return 0;
    }
    let distance = dist2(caster_x, caster_y, x, y).sqrt();
    if !distance.is_finite() || distance <= f32::EPSILON {
        return 0;
    }
    let scaled = (distance / range_px).clamp(0.0, 1.0);
    ((config::SMOKE_LAUNCH_MAX_DELAY_TICKS as f32) * scaled).ceil() as u32
}

pub(crate) fn caster_can_attempt(
    entities: &EntityStore,
    player: u32,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    matches!(entities.get(caster),
        Some(e) if caster_base_ready(e, player, ability)
            && ability_launch_ready(e.kind, e.weapon_setup(), e.path_is_empty(), ability))
}

pub(crate) fn caster_can_accept_order(
    entities: &EntityStore,
    player: u32,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    matches!(entities.get(caster),
        Some(e) if caster_base_ready(e, player, ability)
            && ability_order_ready(e.kind, e.weapon_setup(), ability))
}

pub(crate) fn caster_allowed_by_faction(
    entities: &EntityStore,
    faction_id: &str,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    matches!(entities.get(caster), Some(e)
        if rules::faction::catalog_for(faction_id)
            .is_some_and(|catalog| catalog.allows_ability(ability.to_protocol_str(), e.kind)))
}

fn caster_base_ready(e: &crate::game::entity::Entity, player: u32, ability: AbilityKind) -> bool {
    e.owner == player
        && e.hp > 0
        && e.is_unit()
        && !e.under_construction()
        && ability::carried_by(ability, e.kind)
        && e.ability_uses_remaining(ability).unwrap_or(1) > 0
        && e.ability_cooldown_ticks(ability) == 0
}

fn ability_order_ready(kind: EntityKind, setup: WeaponSetup, ability: AbilityKind) -> bool {
    ability != AbilityKind::MortarFire
        || (kind == EntityKind::MortarTeam && setup == WeaponSetup::Deployed)
}

fn ability_launch_ready(
    kind: EntityKind,
    setup: WeaponSetup,
    path_empty: bool,
    ability: AbilityKind,
) -> bool {
    ability != AbilityKind::MortarFire
        || (kind == EntityKind::MortarTeam && path_empty && setup == WeaponSetup::Deployed)
}

pub(crate) fn world_ability_facing_ready(
    entities: &mut EntityStore,
    caster: u32,
    ability: AbilityKind,
    x: f32,
    y: f32,
) -> bool {
    if ability != AbilityKind::MortarFire {
        return true;
    }
    let Some(e) = entities.get_mut(caster) else {
        return false;
    };
    if e.kind != EntityKind::MortarTeam {
        return false;
    }
    let target_angle = (y - e.pos_y).atan2(x - e.pos_x);
    rotate_mortar_for_fire(e, target_angle)
}

pub(crate) fn world_ability_current_facing_ready(
    entities: &EntityStore,
    caster: u32,
    ability: AbilityKind,
    x: f32,
    y: f32,
) -> bool {
    if ability != AbilityKind::MortarFire {
        return true;
    }
    let Some(e) = entities.get(caster) else {
        return false;
    };
    if e.kind != EntityKind::MortarTeam {
        return false;
    }
    let target_angle = (y - e.pos_y).atan2(x - e.pos_x);
    mortar_current_facing_ready(e, target_angle)
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
