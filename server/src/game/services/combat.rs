use std::collections::HashMap;

use crate::config;
use crate::game::entity::{Entity, EntityStore, Order};
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::protocol::Event;

/// Extra slack (px) added to attack range checks so units don't dance at the exact boundary.
const RANGE_SLACK: f32 = 4.0;

/// Combat: acquire targets for aggressive / attack-move units, let idle units auto-defend,
/// fire bunkers, and deal damage when off cooldown. Damage is applied immediately and emits an
/// `Attack` event (for tracers). Cooldowns tick down here too.
pub(crate) fn combat_system(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    spatial: &SpatialIndex,
    pathing: &mut PathingService,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    // Tick down cooldowns first.
    for e in entities.iter_mut() {
        e.tick_attack_cd();
    }

    for id in entities.ids() {
        // Determine this attacker's combat parameters.
        let (owner, px, py, range_px, aggro_px, dmg, cd_reset, mode, is_unit) = {
            let e = match entities.get(id) {
                Some(e) => e,
                None => continue,
            };
            if e.hp == 0 || !e.can_attack() {
                continue;
            }
            let (range_tiles, dmg, cd) = attack_profile(e);
            let range_px = range_tiles as f32 * config::TILE_SIZE as f32 + e.radius() + RANGE_SLACK;
            // Aggro radius: mobile units detect and chase enemies out to their sight radius so
            // attack-move / auto-defend actually close the gap. Buildings (bunkers) never move,
            // so they only ever engage within their firing range.
            let aggro_px = if e.is_unit() {
                (e.sight_tiles() as f32 * config::TILE_SIZE as f32).max(range_px)
            } else {
                range_px
            };
            (
                e.owner,
                e.pos_x,
                e.pos_y,
                range_px,
                aggro_px,
                dmg,
                cd,
                combat_mode(e),
                e.is_unit(),
            )
        };
        if dmg == 0 {
            continue;
        }

        // Resolve / acquire a target id (explicit target for Ordered, nearest enemy in aggro
        // radius for Aggressive).
        let target = resolve_target(entities, spatial, id, owner, px, py, aggro_px, mode);
        let Some(tid) = target else {
            // No target: clear stale combat target id for non-attack orders.
            if let Some(e) = entities.get_mut(id) {
                if matches!(e.order(), Order::AttackMove { .. } | Order::Idle) {
                    e.set_target_id(None);
                }
            }
            continue;
        };

        // Distance to chosen target.
        let (tx, ty, t_owner) = match entities.get(tid) {
            Some(t) => (t.pos_x, t.pos_y, t.owner),
            None => continue,
        };
        if t_owner == owner {
            continue; // never friendly fire
        }
        let dist = dist2(px, py, tx, ty).sqrt();

        if dist <= range_px {
            // In range: face it, stop, and fire if off cooldown.
            let ready = matches!(entities.get(id), Some(e) if e.attack_cd() == 0);
            if let Some(e) = entities.get_mut(id) {
                e.set_facing((ty - py).atan2(tx - px));
                e.set_target_id(Some(tid));
                // Hold position while a target is in weapon range (don't overshoot it).
                e.clear_path();
            }
            if ready {
                apply_damage(entities, events, id, tid, dmg, owner);
                if let Some(e) = entities.get_mut(id) {
                    e.set_attack_cd(cd_reset);
                }
            }
        } else if is_unit {
            // Out of weapon range but within aggro: chase. Re-path with A* toward the target
            // tile when we have no path, so units route around obstacles rather than stalling.
            let want_repath = entities.get(id).map(|e| e.path_is_empty()).unwrap_or(false);
            if let Some(e) = entities.get_mut(id) {
                e.set_target_id(Some(tid));
            }
            if want_repath {
                pathing.repath_entity(map, entities, occ, id, tx, ty);
            }
        }
    }
}

/// Attack profile (range_tiles, dmg, cooldown) for a unit or bunker.
fn attack_profile(e: &Entity) -> (u32, u32, u32) {
    if let Some(s) = config::unit_stats(e.kind) {
        (s.range_tiles, s.dmg, s.cooldown)
    } else if let Some(s) = config::building_stats(e.kind) {
        (s.range_tiles, s.dmg, s.cooldown)
    } else {
        (0, 0, 0)
    }
}

/// How a combatant chooses targets.
#[derive(Copy, Clone, PartialEq)]
enum CombatMode {
    /// Has an explicit attack target id.
    Ordered,
    /// Engages any enemy within range (attack-move, bunkers, idle auto-defend).
    Aggressive,
}

fn combat_mode(e: &Entity) -> CombatMode {
    match e.order() {
        Order::Attack { .. } => CombatMode::Ordered,
        _ => CombatMode::Aggressive,
    }
}

/// Resolve which entity an attacker should engage this tick.
#[allow(clippy::too_many_arguments)]
fn resolve_target(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
) -> Option<u32> {
    // Ordered attackers keep their explicit target if it still exists.
    if mode == CombatMode::Ordered {
        if let Some(e) = entities.get(self_id) {
            if let Order::Attack { target } = e.order() {
                if entities.get(target).map(|t| t.hp > 0).unwrap_or(false) {
                    return Some(target);
                }
            }
        }
        // Explicit target gone → fall through to acquisition so we don't stand idle.
    }

    // Aggressive acquisition: the nearest enemy within the acquire radius (weapon range for
    // buildings, sight range for mobile units so they chase).
    spatial
        .nearest(px, py, acquire_px, entities, |e: &Entity| {
            e.id != self_id
                && e.owner != owner
                && e.owner != crate::game::entity::NEUTRAL
                && e.is_targetable()
                && e.hp > 0
        })
        .map(|(cid, _)| cid)
}

/// Apply `dmg` to `victim` from `attacker`, emitting an `Attack` event to the attacker's
/// owner. Death itself is handled by the death system (we only zero hp here).
fn apply_damage(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    attacker: u32,
    victim: u32,
    dmg: u32,
    attacker_owner: u32,
) {
    if let Some(v) = entities.get_mut(victim) {
        v.hp = v.hp.saturating_sub(dmg);
    }
    events
        .entry(attacker_owner)
        .or_default()
        .push(Event::Attack {
            from: attacker,
            to: victim,
        });
}
