use std::collections::HashMap;

use crate::config;
use crate::game::entity::{AttackPhase, Entity, EntityStore, Order};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::protocol::Event;

/// Extra slack (px) added to attack range checks so units don't dance at the exact boundary.
const RANGE_SLACK: f32 = 4.0;

/// Combat: acquire targets for aggressive / attack-move units, let eligible idle units
/// auto-acquire enemies, and deal damage when off cooldown. Damage is applied immediately and
/// emits an `Attack` event (for tracers). Cooldowns tick down here too.
pub(crate) fn combat_system(
    _map: &Map,
    entities: &mut EntityStore,
    _occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
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
            // Workers executing a Gather order ignore nearby enemies: chasing aggro would
            // drag them off the resource node and stall the economy. An explicit Attack
            // order overrides Gather upstream, so this only suppresses auto-acquisition.
            if matches!(e.order(), Order::Gather(_)) {
                continue;
            }
            let (range_tiles, dmg, cd) = attack_profile(e);
            let range_px = range_tiles as f32 * config::TILE_SIZE as f32 + e.radius() + RANGE_SLACK;
            // Aggro radius: mobile units detect and chase enemies out to their sight radius so
            // attack-move / auto-defend actually close the gap. Buildings never move, so they
            // only ever engage within their firing range.
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

        // Resolve / acquire a target id based on the current order semantics.
        let target = resolve_target(entities, spatial, id, owner, px, py, aggro_px, mode);
        let Some(tid) = target else {
            // No target: clear stale combat target id for opportunistic-combat orders.
            if let Some(e) = entities.get_mut(id) {
                if matches!(e.order(), Order::AttackMove(_) | Order::Idle) {
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
                e.mark_attack_phase(AttackPhase::Firing);
                // Hold position while a target is in weapon range (don't overshoot it).
                e.clear_path();
            }
            if ready {
                apply_damage(entities, events, fog, id, tid, dmg, owner, px, py, tx, ty);
                if let Some(e) = entities.get_mut(id) {
                    e.set_attack_cd(cd_reset);
                }
            }
        } else if is_unit {
            // Out of weapon range but within aggro: chase. Re-path toward the target tile
            // when we have no path, so units route around obstacles rather than stalling.
            let want_repath = entities.get(id).map(|e| e.path_is_empty()).unwrap_or(false);
            if let Some(e) = entities.get_mut(id) {
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Chasing);
            }
            if want_repath {
                coordinator.request_chase_path(entities, id, (tx, ty));
            }
        }
    }
}

/// Attack profile (range_tiles, dmg, cooldown) for a combat-capable entity.
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
    /// Engages any enemy within range.
    Aggressive,
    /// Ignores nearby enemies unless explicitly ordered to attack.
    Passive,
}

fn combat_mode(e: &Entity) -> CombatMode {
    match e.order() {
        Order::Attack(_) => CombatMode::Ordered,
        Order::AttackMove(_) => CombatMode::Aggressive,
        Order::Idle if e.is_building() => CombatMode::Aggressive,
        Order::Idle if e.is_unit() && e.kind != crate::game::entity::EntityKind::Worker => {
            CombatMode::Aggressive
        }
        _ => CombatMode::Passive,
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
            if let Some(target) = e.order().attack_target() {
                if entities.get(target).map(|t| t.hp > 0).unwrap_or(false) {
                    return Some(target);
                }
            }
        }
        // Explicit target gone → fall through to acquisition so we don't stand idle.
    }

    if mode == CombatMode::Passive {
        return None;
    }

    // Aggressive acquisition: the nearest enemy within the acquire radius (weapon range for
    // buildings, sight range for mobile units so they chase).
    world_query::nearest_enemy_in_range(entities, spatial, self_id, owner, px, py, acquire_px)
}

/// Apply `dmg` to `victim` from `attacker`, emitting an `Attack` event to the attacker's
/// owner. Death itself is handled by the death system (we only zero hp here).
#[allow(clippy::too_many_arguments)]
fn apply_damage(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    attacker: u32,
    victim: u32,
    dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
) {
    let attacker_is_ap = entities.get(attacker).map(|e| e.kind.is_ap()).unwrap_or(false);
    let victim_is_armored = entities.get(victim).map(|e| e.kind.is_armored()).unwrap_or(false);
    let effective_dmg = if victim_is_armored && !attacker_is_ap {
        dmg / 2
    } else {
        dmg
    };
    if let Some(v) = entities.get_mut(victim) {
        v.hp = v.hp.saturating_sub(effective_dmg);
    }
    // Send the Attack event to every player who can either see the attacker or the victim, so
    // friendly fire tracers + enemy muzzle flashes both render. Attacker's owner always gets it.
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        let visible = pid == attacker_owner
            || fog.is_visible_world(pid, ax, ay)
            || fog.is_visible_world(pid, vx, vy);
        if !visible {
            continue;
        }
        events.entry(pid).or_default().push(Event::Attack {
            from: attacker,
            to: victim,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, Order};
    use crate::game::services::spatial::SpatialIndex;

    fn rifleman_with_enemy() -> (EntityStore, u32, u32) {
        let mut entities = EntityStore::new();
        let self_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy rifleman should spawn");
        (entities, self_id, enemy_id)
    }

    #[test]
    fn idle_army_units_auto_acquire_targets() {
        let (entities, self_id, enemy_id) = rifleman_with_enemy();
        let spatial = SpatialIndex::build(&entities, 32);
        let attacker = entities.get(self_id).expect("attacker should exist");

        let target = resolve_target(
            &entities,
            &spatial,
            self_id,
            attacker.owner,
            attacker.pos_x,
            attacker.pos_y,
            128.0,
            combat_mode(attacker),
        );

        assert_eq!(target, Some(enemy_id));
    }

    #[test]
    fn move_orders_ignore_nearby_enemies() {
        let (mut entities, self_id, _) = rifleman_with_enemy();
        let spatial = SpatialIndex::build(&entities, 32);
        let attacker = entities.get_mut(self_id).expect("attacker should exist");
        attacker.set_order(Order::move_to(300.0, 300.0));

        let target = resolve_target(
            &entities,
            &spatial,
            self_id,
            1,
            100.0,
            100.0,
            128.0,
            combat_mode(entities.get(self_id).expect("attacker should exist")),
        );

        assert_eq!(target, None);
    }

    #[test]
    fn attack_move_keeps_auto_acquisition() {
        let (mut entities, self_id, enemy_id) = rifleman_with_enemy();
        let spatial = SpatialIndex::build(&entities, 32);
        let attacker = entities.get_mut(self_id).expect("attacker should exist");
        attacker.set_order(Order::attack_move_to(300.0, 300.0));

        let target = resolve_target(
            &entities,
            &spatial,
            self_id,
            1,
            100.0,
            100.0,
            128.0,
            combat_mode(entities.get(self_id).expect("attacker should exist")),
        );

        assert_eq!(target, Some(enemy_id));
    }

    #[test]
    fn idle_workers_do_not_auto_acquire_targets() {
        let mut entities = EntityStore::new();
        let worker_id = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("worker should spawn");
        entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy rifleman should spawn");
        let spatial = SpatialIndex::build(&entities, 32);
        let worker = entities.get(worker_id).expect("worker should exist");

        let target = resolve_target(
            &entities,
            &spatial,
            worker_id,
            worker.owner,
            worker.pos_x,
            worker.pos_y,
            128.0,
            combat_mode(worker),
        );

        assert_eq!(target, None);
    }
}
