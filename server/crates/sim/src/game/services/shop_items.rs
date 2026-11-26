//! Shop purchase admission and one-use escape resolution.
use std::collections::HashMap;

use super::{dist2, occupancy::Occupancy, standability};
use crate::config;
use crate::game::ability::{self, AbilityKind};
use crate::game::ability_projectile::{AbilityProjectileReturnTarget, AbilityProjectileSpec};
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::teams::TeamRelations;
use crate::game::PlayerState;
use crate::protocol::Event;

pub(in crate::game) fn is_shop_item(ability: AbilityKind) -> bool {
    matches!(
        ability,
        AbilityKind::Regen
            | AbilityKind::EscapeTreasure
            | AbilityKind::HealingSeal
            | AbilityKind::Haste
            | AbilityKind::DoubleXp
            | AbilityKind::BreakthroughTreasure
            | AbilityKind::DoubleDamage
            | AbilityKind::ArmorTreasure
            | AbilityKind::VitalityTreasure
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn purchase(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    teams: &TeamRelations,
    fog: &Fog,
    runtime: &mut AbilityRuntime,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    shops: &[u32],
    ability: AbilityKind,
    target_id: Option<u32>,
    _point: (Option<f32>, Option<f32>),
    tick: u32,
) {
    if !is_shop_item(ability) {
        return;
    }
    if ability == AbilityKind::HealingSeal {
        return;
    }
    // A Shop plus selected fighters requests one automatic purchase. Resolve against
    // authoritative inventory and in-flight reservations, even for rapid keypresses.
    let has_shop = shops
        .iter()
        .filter_map(|id| entities.get(*id))
        .any(|e| e.kind == EntityKind::Shop);
    let target_id = if target_id.is_none() && has_shop {
        auto_recipient(entities, runtime, fog, player, shops, ability)
    } else {
        target_id
    };
    if ability == AbilityKind::EscapeTreasure && target_id.is_none() && !has_shop {
        // A removed source Shop must not turn a pending purchase into manual escape.
        if shops
            .iter()
            .any(|id| entities.get(*id).is_none_or(|e| !e.is_unit()))
        {
            return;
        }
        activate_escape(map, entities, teams, player, shops);
        return;
    }
    let Some(target_id) = target_id else {
        return;
    };
    let Some(target) = entities.get(target_id) else {
        return;
    };
    if target.hp == 0
        || !target.is_unit()
        || !teams.same_team_or_same_owner(player, target.owner)
        || !fog.is_visible_world(player, target.pos_x(), target.pos_y())
    {
        return;
    }
    if ability == AbilityKind::VitalityTreasure && target.max_hp.checked_add(150).is_none() {
        return;
    }
    let endpoint = (target.pos_x(), target.pos_y());
    let definition = ability::definition(ability);
    let range = definition
        .range_tiles
        .map(|tiles| tiles * config::TILE_SIZE as f32);
    let Some(shop) = shops
        .iter()
        .filter_map(|id| entities.get(*id))
        .find(|shop| {
            shop.owner == player
                && shop.kind == EntityKind::Shop
                && shop.hp > 0
                && !shop.under_construction()
                && range.is_none_or(|range| {
                    dist2(shop.pos_x(), shop.pos_y(), endpoint.0, endpoint.1) <= range * range
                })
        })
    else {
        return;
    };
    let Some(ps) = players.iter_mut().find(|p| p.id == player) else {
        return;
    };
    if !crate::rules::faction::catalog_for(&ps.faction_id)
        .is_some_and(|catalog| catalog.allows_ability(ability, EntityKind::Shop))
    {
        return;
    }
    let already_equipped = if ability == AbilityKind::EscapeTreasure {
        target.escape_treasure.is_some()
            || runtime.projectiles().any(|p| {
                p.ability == ability
                    && matches!(p.return_target,
                AbilityProjectileReturnTarget::Entity { id } if id == target_id)
            })
    } else {
        is_buff_item(ability) && (target.shop_item_active(ability)
            || runtime.projectiles().any(|p| p.ability == ability
                && matches!(p.return_target, AbilityProjectileReturnTarget::Entity { id } if id == target_id)))
    };
    if already_equipped {
        events.entry(player).or_default().push(Event::Notice {
            msg: "Item already equipped or being delivered".into(),
            x: None,
            y: None,
            severity: crate::protocol::NoticeSeverity::Info,
        });
        return;
    }
    // Reserve slots on launch so concurrent deliveries cannot overfill a carrier.
    if ability == AbilityKind::EscapeTreasure || is_buff_item(ability) {
        let incoming = runtime.projectiles().filter(|p| {
            (p.ability == AbilityKind::EscapeTreasure || is_buff_item(p.ability))
                && matches!(p.return_target, AbilityProjectileReturnTarget::Entity { id } if id == target_id)
        }).count();
        if target.carried_item_capacity().is_some_and(|capacity| {
            usize::from(target.carried_item_count()) + incoming >= usize::from(capacity)
        }) {
            events.entry(player).or_default().push(Event::Notice {
                msg: "Carried item slots are full".into(),
                x: None,
                y: None,
                severity: crate::protocol::NoticeSeverity::Info,
            });
            return;
        }
    }
    if ability == AbilityKind::EscapeTreasure
        && ps.ability_cooldowns.get(&ability).copied().unwrap_or(0) > 0
    {
        return;
    }
    if !ps.can_afford(definition.cost.steel, definition.cost.oil) {
        events.entry(player).or_default().push(Event::Notice {
            msg: crate::rules::economy::resource_shortage_notice_for_cost(
                ps.steel,
                ps.oil,
                definition.cost,
            )
            .to_string(),
            x: None,
            y: None,
            severity: crate::protocol::NoticeSeverity::Info,
        });
        return;
    }
    if !map.contains_world_point(endpoint.0, endpoint.1) {
        return;
    }
    if runtime.instances().count()
        + runtime.world_objects().count()
        + runtime.projectiles().count()
        + 2
        > crate::game::ability_runtime::MAX_ACTIVE_ABILITY_OBJECTS
    {
        return;
    }
    let origin = (shop.pos_x(), shop.pos_y());
    let shop_id = shop.id;
    let Some(body_id) = entities.spawn_unit(player, EntityKind::ShopDelivery, origin.0, origin.1)
    else {
        return;
    };
    if runtime
        .spawn_projectile(AbilityProjectileSpec {
            owner: player,
            caster_id: shop_id,
            source_object_id: Some(body_id),
            ability,
            origin,
            endpoint,
            return_target: AbilityProjectileReturnTarget::Entity { id: target_id },
            speed_px_per_tick: 7.5 * config::TILE_SIZE as f32 / config::TICK_HZ as f32,
            width_px: 0.0,
            damage: 0,
            created_tick: tick,
            // Unlimited range also means no arbitrary flight timeout.
            expires_tick: u32::MAX,
        })
        .is_some()
    {
        ps.spend_cost(definition.cost);
        if ability == AbilityKind::EscapeTreasure {
            ps.ability_cooldowns
                .insert(ability, definition.cooldown_ticks);
        }
    } else {
        entities.remove(body_id);
    }
}

/// Stable rank/insight/inventory preference; no hidden or foreign units may be candidates.
fn auto_recipient(
    entities: &EntityStore,
    runtime: &AbilityRuntime,
    fog: &Fog,
    player: u32,
    selected: &[u32],
    ability: AbilityKind,
) -> Option<u32> {
    selected
        .iter()
        .filter_map(|id| entities.get(*id))
        .filter_map(|unit| {
            let capacity = unit.carried_item_capacity()?;
            if unit.owner != player
                || unit.hp == 0
                || !fog.is_visible_world(player, unit.pos_x(), unit.pos_y())
                || unit.shop_item_active(ability)
                || (ability == AbilityKind::EscapeTreasure && unit.escape_treasure.is_some())
            {
                return None;
            }
            let mut incoming = 0_usize;
            for delivery in runtime.projectiles().filter(|p|
            matches!(p.return_target, AbilityProjectileReturnTarget::Entity { id } if id == unit.id)
                && (is_buff_item(p.ability) || p.ability == AbilityKind::EscapeTreasure))
        {
            if delivery.ability == ability { return None; }
            incoming += 1;
        }
            let count = usize::from(unit.carried_item_count()) + incoming;
            if count >= usize::from(capacity)
                || (ability == AbilityKind::VitalityTreasure
                    && unit.max_hp.checked_add(150).is_none())
            {
                return None;
            }
            let rank = if unit.dao_master {
                2
            } else if unit.kind != EntityKind::Warrior {
                1
            } else {
                0
            };
            Some((
                (rank, unit.insight, count, std::cmp::Reverse(unit.id)),
                unit.id,
            ))
        })
        .max_by_key(|(priority, _)| *priority)
        .map(|(_, id)| id)
}

pub(in crate::game) fn is_buff_item(ability: AbilityKind) -> bool {
    matches!(
        ability,
        AbilityKind::Regen
            | AbilityKind::Haste
            | AbilityKind::DoubleXp
            | AbilityKind::BreakthroughTreasure
            | AbilityKind::DoubleDamage
            | AbilityKind::ArmorTreasure
            | AbilityKind::VitalityTreasure
    )
}
/// Resolve before death attribution/removal so every damage source observes the same charge.
/// A destroyed source shop cancels its treasure. A completely blocked shop cannot rescue a unit.
pub(super) fn resolve_escapes(map: &Map, entities: &mut EntityStore, teams: &TeamRelations) {
    resolve_selected_escapes(map, entities, teams, None);
}

fn activate_escape(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    player: u32,
    units: &[u32],
) {
    let owned: Vec<_> = units
        .iter()
        .copied()
        .filter(|id| {
            entities
                .get(*id)
                .is_some_and(|unit| unit.owner == player && unit.hp > 0 && unit.is_unit())
        })
        .collect();
    resolve_selected_escapes(map, entities, teams, Some(&owned));
}

fn resolve_selected_escapes(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    manual: Option<&[u32]>,
) {
    let ids = entities.ids();
    let mut occupancy = None;
    for id in ids {
        let Some(unit) = entities.get(id) else {
            continue;
        };
        let Some(shop_id) = unit.escape_treasure else {
            continue;
        };
        let source = entities
            .get(shop_id)
            .filter(|shop| {
                shop.kind == EntityKind::Shop
                    && shop.hp > 0
                    && !shop.under_construction()
                    && teams.same_team_or_same_owner(unit.owner, shop.owner)
            })
            .map(|shop| (shop.pos_x(), shop.pos_y()));
        let Some(source) = source else {
            if let Some(unit) = entities.get_mut(id) {
                unit.escape_treasure = None;
            }
            continue;
        };
        if let Some(selected) = manual {
            if !selected.contains(&id) {
                continue;
            }
        } else if unit.hp > 0 || unit.escape_autocast_disabled {
            continue;
        }
        let hp = unit.hp.max(1);
        let occ = occupancy.get_or_insert_with(|| Occupancy::build(map, entities));
        let tile = config::TILE_SIZE as f32;
        // Even-sized Shop footprints have centers on tile edges. Also search tile centers
        // so a one-tile clearing is reachable without losing exits aligned with the Shop.
        let origin = (
            (source.0 / tile).floor() + 0.5,
            (source.1 / tile).floor() + 0.5,
        );
        let mut destination = None;
        // Stable nearest ring search around the Shop, including room for large unit bodies.
        'search: for ring in 1_i32..=map.width.max(map.height).min(i32::MAX as u32) as i32 {
            let perimeter = (-ring..=ring)
                .map(|dx| (dx, -ring))
                .chain((-ring + 1..=ring).map(|dy| (ring, dy)))
                .chain((-ring..ring).rev().map(|dx| (dx, ring)))
                .chain((-ring + 1..ring).rev().map(|dy| (-ring, dy)));
            for (dx, dy) in perimeter {
                for (x, y) in [
                    (source.0 + dx as f32 * tile, source.1 + dy as f32 * tile),
                    ((origin.0 + dx as f32) * tile, (origin.1 + dy as f32) * tile),
                ] {
                    if standability::unit_spawn_standable_for_entity(map, occ, entities, unit, x, y)
                    {
                        destination = Some((x, y));
                        break 'search;
                    }
                }
            }
        }
        if destination.is_some() {
            entities.release_miner(id);
        }
        if let Some(unit) = entities.get_mut(id) {
            if let Some((x, y)) = destination {
                unit.rescue_shop_escape(x, y, hp);
            } else {
                unit.escape_treasure = None;
            }
        }
    }
}

pub(super) fn set_autocast(
    entities: &mut EntityStore,
    player: u32,
    units: &[u32],
    ability: AbilityKind,
    enabled: bool,
) {
    if ability != AbilityKind::EscapeTreasure {
        return;
    }
    for id in units {
        if let Some(unit) = entities
            .get_mut(*id)
            .filter(|unit| unit.owner == player && unit.hp > 0 && unit.escape_treasure.is_some())
        {
            unit.escape_autocast_disabled = !enabled;
        }
    }
}

pub(in crate::game) fn heal_seals(entities: &mut EntityStore, teams: &TeamRelations, tick: u32) {
    if !tick.is_multiple_of(config::HEALING_SEAL_PULSE_TICKS) {
        return;
    }
    let mut seals: Vec<_> = entities
        .iter()
        .filter(|e| e.kind == EntityKind::HealingSeal && e.hp > 0 && !e.under_construction())
        .map(|e| (e.id, e.owner, e.pos_x(), e.pos_y(), e.hp))
        .collect();
    seals.sort_by_key(|e| e.0);
    let radius = config::HEALING_SEAL_RADIUS_TILES * config::TILE_SIZE as f32;
    let mut units = entities.ids();
    units.sort_unstable();
    for (id, owner, x, y, mut remaining) in seals {
        for unit_id in &units {
            if remaining == 0 {
                break;
            }
            let Some(unit) = entities.get_mut(*unit_id) else {
                continue;
            };
            if unit.hp > 0
                && unit.hp < unit.max_hp
                && unit.is_unit()
                && unit.kind != EntityKind::ScoutPlane
                && teams.same_team_or_same_owner(owner, unit.owner)
                && dist2(x, y, unit.pos_x(), unit.pos_y()) <= radius * radius
                && unit.restore_hp(1)
            {
                remaining -= 1;
            }
        }
        if remaining == 0 {
            // Expiry is not a combat death: no wreck, score, lingering vision, or death event.
            entities.remove(id);
        } else if let Some(seal) = entities.get_mut(id) {
            seal.spend_seal_healing(seal.hp.saturating_sub(remaining));
        }
    }
}

/// Permanent Regen heals one HP every ten simulation ticks, capped at max HP.
pub(in crate::game) fn heal_regen(entities: &mut EntityStore, tick: u32) {
    if !tick.is_multiple_of(config::REGEN_PULSE_TICKS) {
        return;
    }
    for id in entities.ids() {
        if let Some(unit) = entities.get_mut(id) {
            if unit.hp > 0 && unit.shop_item_active(AbilityKind::Regen) {
                unit.restore_hp(1);
            }
        }
    }
}

/// Rebuild a non-stacking movement aura from the same pre-movement positions for all units.
pub(in crate::game) fn refresh_breakthrough_auras(
    entities: &mut EntityStore,
    teams: &TeamRelations,
) {
    let carriers: Vec<_> = entities
        .iter()
        .filter(|e| {
            e.hp > 0 && e.is_unit() && e.shop_item_active(AbilityKind::BreakthroughTreasure)
        })
        .map(|e| (e.owner, e.pos_x(), e.pos_y()))
        .collect();
    let radius = config::BREAKTHROUGH_ITEM_RADIUS_TILES * config::TILE_SIZE as f32;
    for id in entities.ids() {
        if let Some(unit) = entities.get_mut(id) {
            unit.breakthrough_aura = unit.hp > 0
                && unit.is_unit()
                && carriers.iter().any(|&(owner, x, y)| {
                    teams.same_team_or_same_owner(owner, unit.owner)
                        && dist2(x, y, unit.pos_x(), unit.pos_y()) <= radius * radius
                });
        }
    }
}
