use std::collections::HashMap;

use crate::config;
use crate::game::entity::{
    AttackPhase, BuildPhase, Entity, EntityKind, EntityStore, Order, WeaponSetup,
};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::movement::{
    angle_delta, rotate_toward, TANK_BODY_FIRE_TOLERANCE_RAD, TANK_BODY_TURN_RATE_RAD_PER_TICK,
};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::protocol::Event;
use crate::rules::combat as combat_rules;
use crate::rules::projection;
use crate::rules::terrain::TerrainKind;
use rand::rngs::SmallRng;
use rand::Rng;

/// Extra slack (px) added to attack range checks so units don't dance at the exact boundary.
const RANGE_SLACK: f32 = 4.0;
const WORKER_DIRECT_HIT_RETREAT_TILES: f32 = 5.0;

/// Combat: acquire targets for aggressive / attack-move units, let eligible idle units
/// auto-acquire enemies, and deal damage when off cooldown. Damage is applied immediately and
/// emits an `Attack` event (for tracers). Cooldowns tick down here too.
#[allow(clippy::too_many_arguments)]
pub(crate) fn combat_system(
    map: &Map,
    entities: &mut EntityStore,
    _occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    rng: &mut SmallRng,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    // Tick down cooldowns first.
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.tick_attack_cd();
            tick_machine_gunner_setup(e);
        }
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
            let profile = combat_rules::attack_profile(e.kind);
            let (range_tiles, dmg, cd) = (profile.range_tiles, profile.dmg, profile.cooldown);
            let range_px = range_tiles as f32 * config::TILE_SIZE as f32 + e.radius() + RANGE_SLACK;
            // Aggro radius: mobile units detect and chase enemies out to their sight radius so
            // attack-move / auto-defend actually close the gap. Idle machine gunners are the
            // exception: they hold position and only auto-acquire enemies already in weapon
            // range. Buildings never move, so they only ever engage within their firing range.
            let aggro_px = if e.is_unit() {
                if e.kind == EntityKind::MachineGunner && matches!(e.order(), Order::Idle) {
                    range_px
                } else {
                    (e.sight_tiles() as f32 * config::TILE_SIZE as f32).max(range_px)
                }
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
                    begin_idle_machine_gunner_setup(e);
                }
            }
            if matches!(mode, CombatMode::Aggressive) {
                if let Some(goal) = entities.get(id).and_then(|e| e.move_intent()) {
                    let needs_resume = entities
                        .get(id)
                        .and_then(|e| e.path_goal())
                        .map(|path_goal| {
                            (path_goal.0 - goal.0).abs() > f32::EPSILON
                                || (path_goal.1 - goal.1).abs() > f32::EPSILON
                        })
                        .unwrap_or(true);
                    if needs_resume {
                        if let Some(e) = entities.get_mut(id) {
                            e.set_target_id(None);
                        }
                        coordinator.request_chase_path(entities, id, goal);
                    }
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
            // In range: face it, stop, deploy if needed, and fire if off cooldown.
            let target_angle = (ty - py).atan2(tx - px);
            let mut body_aligned = true;
            if let Some(e) = entities.get_mut(id) {
                if e.kind == EntityKind::Tank {
                    body_aligned = rotate_tank_body_for_combat(e, target_angle);
                } else if target_angle.is_finite() {
                    e.set_facing(target_angle);
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Firing);
                // Hold position while a target is in weapon range (don't overshoot it).
                e.clear_path();
            }
            if !body_aligned {
                continue;
            }
            if !machine_gunner_ready_to_fire(entities, id) {
                continue;
            }
            let ready = matches!(entities.get(id), Some(e) if e.attack_cd() == 0);
            if ready {
                apply_damage(
                    map,
                    entities,
                    events,
                    fog,
                    coordinator,
                    rng,
                    id,
                    tid,
                    dmg,
                    owner,
                    px,
                    py,
                    tx,
                    ty,
                    range_px,
                );
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
            if !machine_gunner_ready_to_move(entities, id) {
                continue;
            }
            if want_repath {
                coordinator.request_chase_path(entities, id, (tx, ty));
            }
        }
    }
}

fn rotate_tank_body_for_combat(e: &mut Entity, target_angle: f32) -> bool {
    if !target_angle.is_finite() {
        return false;
    }
    let rotated = rotate_toward(e.facing(), target_angle, TANK_BODY_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_facing(rotated);
    }
    angle_delta(rotated, target_angle).abs() <= TANK_BODY_FIRE_TOLERANCE_RAD
}

fn tick_machine_gunner_setup(e: &mut Entity) {
    if e.kind != EntityKind::MachineGunner {
        return;
    }
    e.tick_weapon_setup();
}

fn begin_idle_machine_gunner_setup(e: &mut Entity) {
    if e.kind != EntityKind::MachineGunner {
        return;
    }
    if !e.path_is_empty() {
        return;
    }
    if !matches!(
        e.order(),
        Order::Idle | Order::Attack(_) | Order::AttackMove(_)
    ) {
        return;
    }
    if matches!(e.weapon_setup(), WeaponSetup::Packed) {
        e.set_weapon_setup(WeaponSetup::SettingUp {
            ticks: config::MACHINE_GUNNER_SETUP_TICKS,
        });
    }
}

fn machine_gunner_ready_to_fire(entities: &mut EntityStore, id: u32) -> bool {
    let Some(e) = entities.get_mut(id) else {
        return false;
    };
    if e.kind != EntityKind::MachineGunner {
        return true;
    }
    match e.weapon_setup() {
        WeaponSetup::Deployed => true,
        WeaponSetup::Packed => {
            e.set_weapon_setup(WeaponSetup::SettingUp {
                ticks: config::MACHINE_GUNNER_SETUP_TICKS,
            });
            false
        }
        WeaponSetup::SettingUp { .. } | WeaponSetup::TearingDown { .. } => false,
    }
}

fn machine_gunner_ready_to_move(entities: &mut EntityStore, id: u32) -> bool {
    let Some(e) = entities.get_mut(id) else {
        return false;
    };
    if e.kind != EntityKind::MachineGunner {
        return true;
    }
    match e.weapon_setup() {
        WeaponSetup::Packed => true,
        WeaponSetup::Deployed | WeaponSetup::SettingUp { .. } => {
            e.set_weapon_setup(WeaponSetup::TearingDown {
                ticks: config::MACHINE_GUNNER_SETUP_TICKS,
            });
            false
        }
        WeaponSetup::TearingDown { .. } => false,
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

    // AT teams prefer tanks over all other targets; fall back to nearest enemy if no tank
    // is in range.
    let prefers_armored = entities
        .get(self_id)
        .map(|e| combat_rules::prefers_armored_targets(e.kind))
        .unwrap_or(false);
    if prefers_armored {
        if let Some(id) = world_query::nearest_tank_in_range(
            entities, spatial, self_id, owner, px, py, acquire_px,
        ) {
            return Some(id);
        }
    }

    // Aggressive acquisition: the nearest enemy within the acquire radius (weapon range for
    // buildings, sight range for mobile units so they chase).
    world_query::nearest_enemy_in_range(entities, spatial, self_id, owner, px, py, acquire_px)
}

/// Apply `dmg` to `victim` from `attacker`, emitting an `Attack` event to the attacker's
/// owner. Death itself is handled by the death system (we only zero hp here).
#[allow(clippy::too_many_arguments)]
fn apply_damage(
    map: &Map,
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    coordinator: &mut MoveCoordinator<'_>,
    rng: &mut SmallRng,
    attacker: u32,
    victim: u32,
    dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    range_px: f32,
) {
    if entities.get(victim).map(|e| e.is_node()).unwrap_or(false) {
        return;
    }
    let attacker_kind = entities.get(attacker).map(|e| e.kind);
    let victim_kind = entities.get(victim).map(|e| e.kind);
    // Roll for miss before computing damage.
    if let (Some(ak), Some(vk)) = (attacker_kind, victim_kind) {
        let mc = combat_rules::miss_chance(ak, vk);
        if mc > 0.0 && rng.gen::<f32>() < mc {
            return;
        }
    }
    let effective_dmg = match (attacker_kind, victim_kind) {
        (Some(ak), Some(vk)) => {
            combat_rules::effective_damage(ak, vk, dmg, Some(TerrainKind::Open))
        }
        _ => dmg,
    };
    if let Some(v) = entities.get_mut(victim) {
        if v.hp > 0 && effective_dmg > 0 {
            v.hp = v.hp.saturating_sub(effective_dmg);
            if v.owner != attacker_owner {
                v.set_last_damage_owner(Some(attacker_owner));
            }
        }
    }
    retreat_worker_from_direct_hit(map, entities, coordinator, victim, attacker_owner, ax, ay);
    apply_overpenetration(
        entities,
        events,
        fog,
        attacker,
        victim,
        effective_dmg,
        attacker_owner,
        ax,
        ay,
        vx,
        vy,
        range_px,
    );
    // Send the Attack event to every player who can either see the attacker or the victim, so
    // friendly fire tracers + enemy muzzle flashes both render. Attacker's owner always gets it.
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if !projection::attack_event_visible_to(pid, ax, ay, vx, vy, attacker_owner, fog) {
            continue;
        }
        events.entry(pid).or_default().push(Event::Attack {
            from: attacker,
            to: victim,
        });
    }
}

fn retreat_worker_from_direct_hit(
    map: &Map,
    entities: &mut EntityStore,
    coordinator: &mut MoveCoordinator<'_>,
    victim: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
) {
    let (owner, vx, vy) = match entities.get(victim) {
        Some(worker)
            if worker.kind == EntityKind::Worker
                && worker.owner != attacker_owner
                && worker.hp > 0
                && !matches!(worker.build_phase(), Some(BuildPhase::Constructing { .. })) =>
        {
            (worker.owner, worker.pos_x, worker.pos_y)
        }
        _ => return,
    };
    let dx = vx - ax;
    let dy = vy - ay;
    let dist = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = if dist > f32::EPSILON && dist.is_finite() {
        (dx / dist, dy / dist)
    } else {
        (1.0, 0.0)
    };
    let retreat_px = WORKER_DIRECT_HIT_RETREAT_TILES * config::TILE_SIZE as f32;
    let max = map.world_size_px() - 0.01;
    let target = (
        (vx + ux * retreat_px).clamp(0.0, max),
        (vy + uy * retreat_px).clamp(0.0, max),
    );
    coordinator.order_group_move(entities, owner, &[victim], target, false);
}

#[allow(clippy::too_many_arguments)]
fn apply_overpenetration(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    attacker: u32,
    primary_victim: u32,
    primary_dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    range_px: f32,
) {
    let dx = vx - ax;
    let dy = vy - ay;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= f32::EPSILON {
        return;
    }
    let overpenetration_limit = dist + range_px * 0.25;
    let ux = dx / dist;
    let uy = dy / dist;
    let perpendicular_slack = RANGE_SLACK + 8.0;
    let splash_dmg = primary_dmg / 2;
    if splash_dmg == 0 {
        return;
    }

    let player_ids: Vec<u32> = events.keys().copied().collect();
    let mut hits: Vec<(u32, f32, f32, f32, f32)> = Vec::new();
    for id in entities.ids() {
        if id == attacker || id == primary_victim {
            continue;
        }
        let Some(target) = entities.get(id) else {
            continue;
        };
        if target.is_node() || target.owner == attacker_owner || target.hp == 0 {
            continue;
        }
        let tx = target.pos_x - ax;
        let ty = target.pos_y - ay;
        let along = tx * ux + ty * uy;
        if along <= dist || along > overpenetration_limit {
            continue;
        }
        let perp = (tx * uy - ty * ux).abs();
        if perp > target.radius() + perpendicular_slack {
            continue;
        }
        hits.push((id, target.pos_x, target.pos_y, along, target.radius()));
    }

    hits.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));
    for (id, tx, ty, _, _) in hits {
        let attacker_kind = entities.get(attacker).map(|e| e.kind);
        let effective_dmg = entities
            .get(id)
            .map(|e| match attacker_kind {
                Some(ak) => {
                    combat_rules::effective_damage(ak, e.kind, splash_dmg, Some(TerrainKind::Open))
                }
                None => splash_dmg,
            })
            .unwrap_or(0);
        if effective_dmg == 0 {
            continue;
        }
        if let Some(v) = entities.get_mut(id) {
            if v.hp > 0 {
                v.hp = v.hp.saturating_sub(effective_dmg);
                v.set_last_damage_owner(Some(attacker_owner));
            }
        }
        for pid in &player_ids {
            if !projection::attack_event_visible_to(*pid, ax, ay, tx, ty, attacker_owner, fog) {
                continue;
            }
            events.entry(*pid).or_default().push(Event::Attack {
                from: attacker,
                to: id,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, Order, WeaponSetup};
    use crate::game::fog::Fog;
    use crate::game::services::move_coordinator::MoveCoordinator;
    use crate::game::services::movement::movement_system;
    use crate::game::services::occupancy::Occupancy;
    use crate::game::services::pathing::PathingService;
    use crate::game::services::spatial::SpatialIndex;
    use rand::SeedableRng;

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

    fn run_combat_tick(entities: &mut EntityStore) -> HashMap<u32, Vec<Event>> {
        let map = Map::generate(2, 0x00C0_FFEE);
        let occ = Occupancy::build(&map, entities);
        let spatial = SpatialIndex::build(entities, config::TILE_SIZE);
        let mut pathing = PathingService::new(256, 64);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 10);
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], entities);
        let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

        let mut rng = SmallRng::seed_from_u64(0);
        combat_system(
            &map,
            entities,
            &occ,
            &spatial,
            &mut coordinator,
            &fog,
            &mut rng,
            &mut events,
        );
        events
    }

    fn run_movement_tick(entities: &mut EntityStore) {
        let map = Map::generate(2, 0x00C0_FFEE);
        let occ = Occupancy::build(&map, entities);
        let spatial = SpatialIndex::build(entities, map.size);
        movement_system(&map, entities, &occ, &spatial, 0);
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_test_damage(
        entities: &mut EntityStore,
        events: &mut HashMap<u32, Vec<Event>>,
        attacker: u32,
        victim: u32,
        dmg: u32,
        attacker_owner: u32,
        ax: f32,
        ay: f32,
        vx: f32,
        vy: f32,
        range_px: f32,
    ) {
        let map = Map::generate(2, 0x00C0_FFEE);
        let occ = Occupancy::build(&map, entities);
        let mut pathing = PathingService::new(256, 64);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 10);
        let fog = Fog::new(map.size);
        let mut rng = SmallRng::seed_from_u64(0);
        apply_damage(
            &map,
            entities,
            events,
            &fog,
            &mut coordinator,
            &mut rng,
            attacker,
            victim,
            dmg,
            attacker_owner,
            ax,
            ay,
            vx,
            vy,
            range_px,
        );
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
    fn attack_move_resumes_original_destination_after_target_is_gone() {
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let attacker = entities
            .get_mut(attacker_id)
            .expect("attacker should exist");
        attacker.set_order(Order::attack_move_to(300.0, 300.0));
        attacker.set_path_goal(Some((270.0, 100.0)));
        attacker.set_path(Vec::new());

        let map = Map::generate(2, 0x00C0_FFEE);
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        let mut pathing = PathingService::new(256, 64);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 0);
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities);
        let mut events = HashMap::from([(1, Vec::new())]);

        let mut rng = SmallRng::seed_from_u64(0);
        combat_system(
            &map,
            &mut entities,
            &occ,
            &spatial,
            &mut coordinator,
            &fog,
            &mut rng,
            &mut events,
        );
        assert_eq!(
            entities
                .get(attacker_id)
                .expect("attacker should exist")
                .path_goal(),
            Some((300.0, 300.0))
        );
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

    #[test]
    fn directly_hit_workers_retreat_from_attacker() {
        let mut entities = EntityStore::new();
        let worker_id = entities
            .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
            .expect("worker should spawn");
        entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
            .expect("enemy rifleman should spawn");

        run_combat_tick(&mut entities);

        let worker = entities.get(worker_id).expect("worker should exist");
        assert!(
            worker.hp < worker.max_hp,
            "worker should have taken direct damage"
        );
        assert!(
            matches!(worker.order(), Order::Move(_)),
            "direct damage should issue a temporary move-away order"
        );
        let goal = worker.path_goal().expect("retreat should set a path goal");
        assert!(
            goal.0 > worker.pos_x,
            "retreat should move away from the attacker on the left"
        );
    }

    #[test]
    fn direct_hits_do_not_pull_workers_off_active_construction() {
        let mut entities = EntityStore::new();
        let worker_id = entities
            .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
            .expect("worker should spawn");
        entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
            .expect("enemy rifleman should spawn");
        let site = entities
            .spawn_building(1, EntityKind::Depot, 160.0, 100.0, false)
            .expect("scaffold should spawn");
        if let Some(worker) = entities.get_mut(worker_id) {
            worker.set_order(Order::build(EntityKind::Depot, 4, 4));
            worker.mark_build_phase(BuildPhase::Constructing { site });
        }

        run_combat_tick(&mut entities);

        let worker = entities.get(worker_id).expect("worker should exist");
        assert!(
            matches!(worker.build_phase(), Some(BuildPhase::Constructing { .. })),
            "active builders remain latched so scaffolds are not stranded"
        );
    }

    #[test]
    fn idle_machine_gunner_deploys_after_stationary_delay() {
        let mut entities = EntityStore::new();
        let mg_id = entities
            .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
            .expect("machine gunner should spawn");

        run_combat_tick(&mut entities);
        assert!(matches!(
            entities.get(mg_id).expect("mg should exist").weapon_setup(),
            WeaponSetup::SettingUp { .. }
        ));

        for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
            run_combat_tick(&mut entities);
        }

        assert_eq!(
            entities.get(mg_id).expect("mg should exist").weapon_setup(),
            WeaponSetup::Deployed
        );
    }

    #[test]
    fn idle_machine_gunner_does_not_chase_distant_enemies() {
        let mut entities = EntityStore::new();
        let mg_id = entities
            .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
            .expect("machine gunner should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 330.0, 100.0)
            .expect("enemy should spawn");
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

        run_combat_tick(&mut entities);

        let mg = entities.get(mg_id).expect("mg should exist");
        assert_eq!(mg.target_id(), None);
        assert!(mg.path_is_empty(), "idle machine gunner should not chase");
        assert_eq!(
            entities.get(enemy_id).expect("enemy should exist").hp,
            enemy_hp,
            "distant enemies should not be attacked or chased"
        );
    }

    #[test]
    fn machine_gunner_waits_to_deploy_before_first_shot() {
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
            .expect("machine gunner should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy should spawn");
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

        run_combat_tick(&mut entities);
        assert_eq!(
            entities.get(enemy_id).expect("enemy should exist").hp,
            enemy_hp
        );

        for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
            run_combat_tick(&mut entities);
        }

        assert!(
            entities.get(enemy_id).expect("enemy should exist").hp < enemy_hp,
            "machine gunner should fire once deployment completes"
        );
    }

    #[test]
    fn deployed_machine_gunner_can_fire_immediately() {
        let mut entities = EntityStore::new();
        let mg_id = entities
            .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
            .expect("machine gunner should spawn");

        run_combat_tick(&mut entities);
        for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
            run_combat_tick(&mut entities);
        }
        assert_eq!(
            entities.get(mg_id).expect("mg should exist").weapon_setup(),
            WeaponSetup::Deployed
        );

        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy should spawn");
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

        run_combat_tick(&mut entities);

        assert!(
            entities.get(enemy_id).expect("enemy should exist").hp < enemy_hp,
            "deployed machine gunner should not wait for another setup cycle"
        );
    }

    #[test]
    fn machine_gunner_tears_down_before_moving() {
        let mut entities = EntityStore::new();
        let mg_id = entities
            .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
            .expect("machine gunner should spawn");
        let start_x = entities.get(mg_id).expect("mg should exist").pos_x;

        {
            let mg = entities.get_mut(mg_id).expect("mg should exist");
            mg.set_weapon_setup(WeaponSetup::TearingDown {
                ticks: config::MACHINE_GUNNER_SETUP_TICKS,
            });
            mg.set_order(Order::move_to(120.0, 100.0));
            mg.set_path(vec![(120.0, 100.0)]);
            mg.set_path_goal(Some((120.0, 100.0)));
        }

        run_movement_tick(&mut entities);
        assert_eq!(entities.get(mg_id).expect("mg should exist").pos_x, start_x);

        for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
            run_combat_tick(&mut entities);
        }
        assert_eq!(
            entities.get(mg_id).expect("mg should exist").weapon_setup(),
            WeaponSetup::Packed
        );

        run_movement_tick(&mut entities);
        assert!(
            entities.get(mg_id).expect("mg should exist").pos_x > start_x,
            "machine gunner should move after teardown completes"
        );
    }

    #[test]
    fn tank_combat_does_not_snap_body_to_target() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0, 140.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_facing(0.0);
            tank.set_order(Order::attack(enemy_id));
        }

        run_combat_tick(&mut entities);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert!(
            tank.facing() > 0.0 && tank.facing() <= TANK_BODY_TURN_RATE_RAD_PER_TICK + 0.0001,
            "tank body should rotate gradually toward target, got {:.4}",
            tank.facing()
        );
        assert_eq!(
            tank.attack_cd(),
            0,
            "misaligned tank should not fire on the same tick it starts turning"
        );
    }

    #[test]
    fn tank_cannot_fire_until_body_aligned_before_turrets_exist() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_facing(std::f32::consts::FRAC_PI_2);
            tank.set_order(Order::attack(enemy_id));
        }
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

        for _ in 0..10 {
            run_combat_tick(&mut entities);
        }
        assert_eq!(
            entities.get(enemy_id).expect("enemy should exist").hp,
            enemy_hp,
            "tank should not damage the target while hull aim is outside tolerance"
        );
        assert_eq!(
            entities
                .get(tank_id)
                .expect("tank should exist")
                .attack_cd(),
            0,
            "tank cooldown should remain ready while firing is gated by hull alignment"
        );

        let mut fired = false;
        for _ in 0..80 {
            run_combat_tick(&mut entities);
            if entities
                .get(tank_id)
                .expect("tank should exist")
                .attack_cd()
                > 0
            {
                fired = true;
                break;
            }
        }

        assert!(
            fired,
            "tank should fire once its hull rotates inside the temporary no-turret tolerance"
        );
    }

    #[test]
    fn shots_overpenetrate_past_the_primary_target() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let primary = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("primary target should spawn");
        let secondary = entities
            .spawn_unit(2, EntityKind::Worker, 165.0, 100.0)
            .expect("secondary target should spawn");
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        events.insert(1, Vec::new());
        events.insert(2, Vec::new());

        apply_test_damage(
            &mut entities,
            &mut events,
            attacker,
            primary,
            10,
            1,
            100.0,
            100.0,
            140.0,
            100.0,
            128.0,
        );

        assert_eq!(entities.get(primary).expect("primary should exist").hp, 35);
        let secondary = entities.get(secondary).expect("secondary should exist");
        assert_eq!(secondary.hp, 35);
        assert!(
            matches!(secondary.order(), Order::Idle),
            "overpenetration damage must not trigger worker retreat"
        );
    }

    #[test]
    fn overpenetration_does_not_damage_resource_nodes() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let primary = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("primary target should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, 165.0, 100.0)
            .expect("resource node should spawn");
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        events.insert(1, Vec::new());
        events.insert(2, Vec::new());

        apply_test_damage(
            &mut entities,
            &mut events,
            attacker,
            primary,
            10,
            1,
            100.0,
            100.0,
            140.0,
            100.0,
            128.0,
        );

        assert_eq!(entities.get(primary).expect("primary should exist").hp, 35);
        assert_eq!(
            entities.get(node).expect("node should exist").remaining(),
            Some(1500)
        );
        assert_eq!(entities.get(node).expect("node should exist").hp, 1);
    }
}
