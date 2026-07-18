use std::collections::HashMap;

use crate::config;
use crate::game::ability::{
    self, AbilityEffectHook, AbilityKind, AbilityQueuePolicy, AbilityTargetMode,
};
use crate::game::ability_runtime::{
    AbilityObjectPayload, AbilityRuntime, AbilityWorldObjectKind, AbilityWorldObjectSpec,
};
use crate::game::entity::{EntityKind, EntityStore, MovePhase, Order};
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
use crate::game::upgrade::UpgradeKind;
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
    ability_runtime: &mut AbilityRuntime,
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
    if !policy_accepts(entities, player, caster, ability) {
        return AbilityOrderResult::Skipped;
    }
    if caster_locked_out(entities, caster, ability, tick) {
        return AbilityOrderResult::Skipped;
    }
    if !tech_requirement_met(entities, player, ability) {
        return AbilityOrderResult::Skipped;
    }
    if caster_in_range(map, entities, caster, ability, x, y) || ability_clamps_world_target(ability)
    {
        if !caster_can_attempt(entities, player, caster, ability)
            || !world_ability_current_facing_ready(entities, caster, ability, x, y)
        {
            let Some((sx, sy)) = entities.get(caster).map(|e| (e.pos_x, e.pos_y)) else {
                return AbilityOrderResult::Skipped;
            };
            if let Some(e) = entities.get_mut(caster) {
                e.replace_active_order(Order::ability(ability, x, y, sx, sy));
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
            ability_runtime,
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
    ability_runtime: &mut AbilityRuntime,
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
        || caster_locked_out(entities, caster, ability, tick)
        || !(caster_in_range(map, entities, caster, ability, x, y)
            || ability_clamps_world_target(ability))
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
                e.set_path_goal(None);
            }
            e.set_attack_cd(mortar_fire_weapon_cooldown_ticks());
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
            let (radius_tiles, duration_ticks) = if ps.has_upgrade(UpgradeKind::SmokePlus) {
                (
                    config::SMOKE_PLUS_CLOUD_RADIUS_TILES,
                    config::SMOKE_PLUS_CLOUD_DURATION_TICKS,
                )
            } else {
                (
                    config::SMOKE_CLOUD_RADIUS_TILES,
                    config::SMOKE_CLOUD_DURATION_TICKS,
                )
            };
            smokes.schedule(
                x,
                y,
                radius_tiles,
                duration_ticks,
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
        (AbilityEffectHook::DashReturn, AbilityKind::EkatTeleport) => {
            let Some((origin_x, origin_y)) = entities.get(caster).map(|e| (e.pos_x, e.pos_y))
            else {
                return false;
            };
            let Some((blink_x, blink_y)) = hero_abilities::ekat_dash_destination(
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
            ability_runtime.clear_return_markers(player, caster, ability);
            ability_runtime.spawn_world_object(AbilityWorldObjectSpec {
                owner: player,
                caster_id: caster,
                ability,
                kind: AbilityWorldObjectKind::ReturnMarker,
                x: origin_x,
                y: origin_y,
                created_tick: tick,
                expires_tick: tick.saturating_add(config::EKAT_RETURN_MARKER_DURATION_TICKS),
                payload: AbilityObjectPayload::DashReturn {
                    earliest_return_tick: tick.saturating_add(config::EKAT_RETURN_MIN_DELAY_TICKS),
                },
            });
            if !hero_abilities::move_ekat_to(entities, caster, blink_x, blink_y) {
                return false;
            }
            notice_positioned(
                events,
                player,
                "Dash",
                crate::protocol::NoticeSeverity::Info,
                blink_x,
                blink_y,
            );
            true
        }
        (AbilityEffectHook::LineProjectile, AbilityKind::EkatLineShot) => {
            let Some(hero_projectile_spec) = hero_abilities::ekat_line_projectile_spec(
                entities,
                player,
                caster,
                x,
                y,
                definition.range_tiles,
                tick,
            ) else {
                return false;
            };
            let target_x = hero_projectile_spec.endpoint.0;
            let target_y = hero_projectile_spec.endpoint.1;
            let mut projectile_specs = vec![hero_projectile_spec];
            if let Some(anchor) =
                ability_runtime.active_anchor(player, caster, AbilityKind::EkatMagicAnchor, tick)
            {
                if let Some(anchor_projectile_spec) =
                    hero_abilities::ekat_line_projectile_spec_from_origin(
                        player,
                        caster,
                        Some(anchor.id.get()),
                        (anchor.x, anchor.y),
                        (x, y),
                        definition.range_tiles,
                        tick,
                    )
                {
                    projectile_specs.push(anchor_projectile_spec);
                }
            }
            if !ps.spend_cost(definition.cost) {
                return false;
            }
            for projectile_spec in projectile_specs {
                if ability_runtime.spawn_projectile(projectile_spec).is_none() {
                    return false;
                }
            }
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
        (AbilityEffectHook::MagicAnchor, AbilityKind::EkatMagicAnchor) => {
            let Some((anchor_x, anchor_y)) = SmokeCloudStore::clamp_point_to_map(map, x, y) else {
                return false;
            };
            if !ps.spend_cost(definition.cost) {
                return false;
            }
            let Some(e) = entities.get_mut(caster) else {
                return false;
            };
            ability_runtime.clear_active_anchors(player, caster, ability);
            let radius = config::EKAT_MAGIC_ANCHOR_RADIUS_TILES * config::TILE_SIZE as f32;
            if ability_runtime
                .spawn_world_object(AbilityWorldObjectSpec {
                    owner: player,
                    caster_id: caster,
                    ability,
                    kind: AbilityWorldObjectKind::MagicAnchor,
                    x: anchor_x,
                    y: anchor_y,
                    created_tick: tick,
                    expires_tick: tick.saturating_add(config::EKAT_MAGIC_ANCHOR_DURATION_TICKS),
                    payload: AbilityObjectPayload::MagicAnchor { radius },
                })
                .is_none()
            {
                return false;
            }
            e.start_ability_cooldown(ability, definition.cooldown_ticks);
            if !preserve_active_order {
                e.clear_active_order();
            }
            notice_positioned(
                events,
                player,
                "Magic Anchor",
                crate::protocol::NoticeSeverity::Info,
                anchor_x,
                anchor_y,
            );
            true
        }
        _ => false,
    }
}

fn ability_clamps_world_target(ability: AbilityKind) -> bool {
    matches!(
        ability,
        AbilityKind::EkatTeleport | AbilityKind::EkatLineShot
    )
}

fn caster_locked_out(entities: &EntityStore, caster: u32, ability: AbilityKind, tick: u32) -> bool {
    entities
        .get(caster)
        .and_then(|e| e.ability_lockout_until_tick(ability, tick))
        .is_some()
}

pub(crate) fn launch_self_ability(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
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
                e.start_breakthrough_aura(config::BREAKTHROUGH_DURATION_TICKS);
                e.start_ability_cooldown(ability, definition.cooldown_ticks);
            }
            true
        }
        (AbilityEffectHook::ConsumeGolem, AbilityKind::EkatConsumeGolem) => {
            let Some((caster_x, caster_y)) = entities.get(caster).map(|e| (e.pos_x, e.pos_y))
            else {
                return false;
            };
            let Some(golem) = nearest_owned_golem_for_consume(entities, player, caster_x, caster_y)
            else {
                return false;
            };
            entities.release_miner(golem);
            if entities.remove(golem).is_none() {
                return false;
            }
            let Some(e) = entities.get_mut(caster) else {
                return false;
            };
            let missing_hp = e.max_hp.saturating_sub(e.hp);
            e.restore_hp(missing_hp);
            e.start_ability_cooldown(ability, definition.cooldown_ticks);
            notice_positioned(
                events,
                player,
                "Consumed Golem",
                crate::protocol::NoticeSeverity::Info,
                caster_x,
                caster_y,
            );
            true
        }
        _ => false,
    }
}

fn nearest_owned_golem_for_consume(
    entities: &EntityStore,
    player: u32,
    x: f32,
    y: f32,
) -> Option<u32> {
    let range_px = config::EKAT_CONSUME_GOLEM_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let range2 = range_px * range_px + 0.01;
    entities
        .iter()
        .filter(|candidate| {
            candidate.owner == player
                && candidate.kind == EntityKind::Golem
                && candidate.hp > 0
                && dist2(x, y, candidate.pos_x, candidate.pos_y) <= range2
        })
        .map(|candidate| (candidate.id, dist2(x, y, candidate.pos_x, candidate.pos_y)))
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(id, _)| id)
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
            && ability_weapon_cycle_ready(e, ability)
            && ability_launch_ready(e.kind, ability))
}

pub(crate) fn caster_can_promote_queued_world_ability(
    entities: &EntityStore,
    player: u32,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    match ability::definition(ability).queue_policy {
        AbilityQueuePolicy::QueueWaitUntilReady => {
            caster_can_accept_waiting_order(entities, player, caster, ability)
        }
        _ => caster_can_attempt(entities, player, caster, ability),
    }
}

pub(crate) fn caster_can_accept_waiting_order(
    entities: &EntityStore,
    player: u32,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    matches!(entities.get(caster),
        Some(e) if base_eligible(e, player, ability) && ability_order_ready(e.kind, ability))
}

fn policy_accepts(store: &EntityStore, player: u32, caster: u32, ability: AbilityKind) -> bool {
    match ability::definition(ability).queue_policy {
        AbilityQueuePolicy::QueueWaitUntilReady => {
            caster_can_accept_waiting_order(store, player, caster, ability)
        }
        _ => caster_can_accept_order(store, player, caster, ability),
    }
}

pub(crate) fn caster_can_accept_order(
    entities: &EntityStore,
    player: u32,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    matches!(entities.get(caster),
        Some(e) if caster_base_ready(e, player, ability)
            && ability_order_ready(e.kind, ability))
}

pub(crate) fn caster_allowed_by_faction(
    entities: &EntityStore,
    faction_id: &str,
    caster: u32,
    ability: AbilityKind,
) -> bool {
    matches!(entities.get(caster), Some(e)
        if rules::faction::catalog_for(faction_id)
            .is_some_and(|catalog| catalog.allows_ability(ability, e.kind)))
}

fn caster_base_ready(e: &crate::game::entity::Entity, player: u32, ability: AbilityKind) -> bool {
    base_eligible(e, player, ability) && e.ability_cooldown_ticks(ability) == 0
}

fn base_eligible(e: &crate::game::entity::Entity, player: u32, ability: AbilityKind) -> bool {
    e.owner == player
        && e.hp > 0
        && e.is_unit()
        && !e.under_construction()
        && ability::carried_by(ability, e.kind)
        && e.ability_uses_remaining(ability).unwrap_or(1) > 0
}

fn ability_weapon_cycle_ready(e: &crate::game::entity::Entity, ability: AbilityKind) -> bool {
    ability != AbilityKind::MortarFire || e.attack_cd() == 0
}

fn mortar_fire_weapon_cooldown_ticks() -> u32 {
    config::unit_stats(EntityKind::MortarTeam).map_or(0, |stats| stats.cooldown)
}

fn ability_order_ready(kind: EntityKind, ability: AbilityKind) -> bool {
    ability != AbilityKind::MortarFire || kind == EntityKind::MortarTeam
}

fn ability_launch_ready(kind: EntityKind, ability: AbilityKind) -> bool {
    ability != AbilityKind::MortarFire || kind == EntityKind::MortarTeam
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_recast_return(
    map: &Map,
    entities: &mut EntityStore,
    ability_runtime: &mut AbilityRuntime,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    faction_id: &str,
    ability: AbilityKind,
    units: Vec<u32>,
    target_object_id: Option<u32>,
    tick: u32,
) -> bool {
    let Some((caster, marker_id, marker_pos)) = recast_return_candidate(
        map,
        entities,
        ability_runtime,
        player,
        faction_id,
        ability,
        units,
        target_object_id,
        tick,
    ) else {
        return false;
    };
    if ability_runtime
        .consume_active_return_marker(player, caster, ability, Some(marker_id), tick)
        .is_none()
    {
        return false;
    }
    if !hero_abilities::move_ekat_to(entities, caster, marker_pos.0, marker_pos.1) {
        return false;
    }
    notice_positioned(
        events,
        player,
        "Return",
        crate::protocol::NoticeSeverity::Info,
        marker_pos.0,
        marker_pos.1,
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn recast_return_candidate(
    map: &Map,
    entities: &EntityStore,
    ability_runtime: &AbilityRuntime,
    player: u32,
    faction_id: &str,
    ability: AbilityKind,
    units: Vec<u32>,
    target_object_id: Option<u32>,
    tick: u32,
) -> Option<(u32, u32, (f32, f32))> {
    let definition = ability::definition(ability);
    if definition.target_mode != AbilityTargetMode::WorldPoint {
        return None;
    }
    for caster in units {
        let Some(entity) = entities.get(caster) else {
            continue;
        };
        if entity.owner != player || entity.hp == 0 {
            continue;
        }
        if !caster_allowed_by_faction(entities, faction_id, caster, ability)
            || !tech_requirement_met(entities, player, ability)
        {
            continue;
        }
        let Some(marker) =
            ability_runtime.active_return_marker(player, caster, ability, target_object_id, tick)
        else {
            continue;
        };
        let AbilityObjectPayload::DashReturn {
            earliest_return_tick,
        } = marker.payload
        else {
            continue;
        };
        if tick < earliest_return_tick {
            continue;
        }
        if !hero_abilities::ekat_return_destination_valid(map, entities, caster, marker.x, marker.y)
        {
            continue;
        }
        return Some((caster, marker.id.get(), (marker.x, marker.y)));
    }
    None
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
    let definition = ability::definition(ability);
    let Some(range_tiles) = definition.range_tiles else {
        return true;
    };
    if SmokeCloudStore::clamp_point_to_map(map, x, y).is_none() {
        return false;
    }
    let range_px = range_tiles as f32 * config::TILE_SIZE as f32;
    let min_range_px = definition.min_range_tiles.unwrap_or(0) as f32 * config::TILE_SIZE as f32;
    let distance_sq = dist2(e.pos_x, e.pos_y, x, y);
    distance_sq >= min_range_px * min_range_px && distance_sq <= range_px * range_px
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
    let definition = ability::definition(ability);
    let range_tiles = definition.range_tiles?;
    let range_px = range_tiles as f32 * config::TILE_SIZE as f32;
    let min_range_px = definition.min_range_tiles.unwrap_or(0) as f32 * config::TILE_SIZE as f32;
    let dx = caster.pos_x - x;
    let dy = caster.pos_y - y;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() {
        return None;
    }
    let margin = (caster.radius() * 0.25).max(1.0);
    let staging_distance = if len < min_range_px {
        (min_range_px + margin).min(range_px)
    } else {
        (range_px - margin).max(min_range_px)
    };
    let (dir_x, dir_y) = if len > f32::EPSILON {
        (dx / len, dy / len)
    } else {
        let map_center = map.world_size_px() * 0.5;
        let center_dx = map_center - x;
        let center_dy = map_center - y;
        let center_len = center_dx.hypot(center_dy);
        if center_len > f32::EPSILON {
            (center_dx / center_len, center_dy / center_len)
        } else {
            let facing = caster.facing();
            if facing.is_finite() {
                (facing.cos(), facing.sin())
            } else {
                (1.0, 0.0)
            }
        }
    };
    let (sx, sy) = (x + dir_x * staging_distance, y + dir_y * staging_distance);
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
