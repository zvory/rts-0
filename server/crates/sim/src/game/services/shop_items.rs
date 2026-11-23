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
        AbilityKind::HealingPotion | AbilityKind::EscapeTreasure
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
    tick: u32,
) {
    if !is_shop_item(ability) {
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
    if ability == AbilityKind::EscapeTreasure
        && (target.escape_treasure.is_some()
            || runtime.projectiles().any(|p| {
                p.ability == ability
                    && matches!(p.return_target,
            AbilityProjectileReturnTarget::Entity { id } if id == target_id)
            }))
    {
        return;
    }
    let endpoint = (target.pos_x(), target.pos_y());
    let definition = ability::definition(ability);
    let range = definition.range_tiles.unwrap_or(0) as f32 * config::TILE_SIZE as f32;
    let Some(shop) = shops
        .iter()
        .filter_map(|id| entities.get(*id))
        .find(|shop| {
            shop.owner == player
                && shop.kind == EntityKind::Shop
                && shop.hp > 0
                && !shop.under_construction()
                && dist2(shop.pos_x(), shop.pos_y(), endpoint.0, endpoint.1) <= range * range
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
    if runtime
        .spawn_projectile(AbilityProjectileSpec {
            owner: player,
            caster_id: shop.id,
            source_object_id: None,
            ability,
            origin: (shop.pos_x(), shop.pos_y()),
            endpoint,
            return_target: AbilityProjectileReturnTarget::Entity { id: target_id },
            speed_px_per_tick: 10.0 * config::TILE_SIZE as f32 / config::TICK_HZ as f32,
            width_px: 0.0,
            damage: 0,
            created_tick: tick,
            // A pursued unit can run; cap delivery lifetime instead of retaining it indefinitely.
            expires_tick: tick.saturating_add(config::TICK_HZ * 60),
        })
        .is_some()
    {
        ps.spend_cost(definition.cost);
    }
}

/// Resolve before death attribution/removal so every damage source observes the same charge.
/// A destroyed source shop cancels its treasure. A completely blocked shop cannot rescue a unit.
pub(super) fn resolve_escapes(map: &Map, entities: &mut EntityStore, teams: &TeamRelations) {
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
        if unit.hp > 0 {
            continue;
        }
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
                unit.rescue_shop_escape(x, y);
            } else {
                unit.escape_treasure = None;
            }
        }
    }
}
