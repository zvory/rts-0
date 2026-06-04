use std::collections::HashMap;

use crate::config;
use crate::game::entity::{
    AttackPhase, BuildPhase, Entity, EntityKind, EntityStore, Order, WeaponSetup,
};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::movement::{angle_delta, rotate_toward};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules::combat as combat_rules;
use crate::rules::projection;
use crate::rules::terrain::TerrainKind;
use rand::rngs::SmallRng;
use rand::Rng;

/// Extra slack (px) added to attack range checks so units don't dance at the exact boundary.
const RANGE_SLACK: f32 = 4.0;
const WORKER_DIRECT_HIT_RETREAT_TILES: f32 = 5.0;
const TANK_TURRET_TURN_RATE_RAD_PER_TICK: f32 = 0.070;
const TANK_TURRET_FIRE_TOLERANCE_RAD: f32 = 0.18;
const TANK_STANDOFF_BUFFER_PX: f32 = config::TILE_SIZE as f32;
const TANK_STANDOFF_REPATH_DELTA_PX: f32 = config::TILE_SIZE as f32;

/// Combat: acquire targets for aggressive / attack-move units, let eligible idle units
/// auto-acquire enemies, and deal damage when off cooldown. Damage is applied immediately and
/// emits an `Attack` event (for tracers). Cooldowns tick down here too.
#[allow(clippy::too_many_arguments)]
pub(crate) fn combat_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    _occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    rng: &mut SmallRng,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let los = LineOfSight::new(map);
    // Tick down cooldowns first.
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.tick_attack_cd();
            tick_machine_gunner_setup(e);
        }
    }

    for id in entities.ids() {
        // Determine this attacker's combat parameters.
        let (owner, px, py, range_px, acquire_px, dmg, cd_reset, mode, is_unit) = {
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
            let mode = combat_mode(e);
            let acquire_px = if mode == CombatMode::Opportunistic {
                range_px
            } else {
                aggro_px
            };
            (
                e.owner,
                e.pos_x,
                e.pos_y,
                range_px,
                acquire_px,
                dmg,
                cd,
                mode,
                e.is_unit(),
            )
        };
        if dmg == 0 {
            continue;
        }

        // Resolve / acquire a target id based on the current order semantics.
        let target = resolve_target(entities, spatial, &los, id, owner, px, py, acquire_px, mode);
        let Some(tid) = target else {
            // No target: clear stale combat target id for opportunistic-combat orders.
            if let Some(e) = entities.get_mut(id) {
                if matches!(
                    e.order(),
                    Order::AttackMove(_) | Order::Move(_) | Order::Idle
                ) {
                    e.set_target_id(None);
                    begin_idle_machine_gunner_setup(e);
                }
                if e.kind == EntityKind::Tank {
                    relax_tank_weapon_toward_body(e);
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
        let target_angle = (ty - py).atan2(tx - px);
        let clear_shot = los.clear_between_world_points((px, py), (tx, ty));

        if dist <= range_px && clear_shot {
            // In range: aim, stop, deploy if needed, and fire if off cooldown.
            let mut weapon_aligned = true;
            if let Some(e) = entities.get_mut(id) {
                if e.kind == EntityKind::Tank {
                    weapon_aligned = rotate_tank_weapon_for_combat(e, target_angle);
                } else if target_angle.is_finite() {
                    e.set_facing(target_angle);
                    mirror_weapon_to_body(e, target_angle);
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Firing);
                // Most units hold position while firing. Tanks have independent turret facing,
                // so they can keep driving along their current path while the weapon tracks.
                if e.kind != EntityKind::Tank {
                    e.clear_path();
                }
            }
            if !weapon_aligned {
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
                    players,
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
            // Out of weapon range but within aggro: chase. Tanks route to a standoff point
            // inside firing range; other units still route toward the target center.
            let chase_goal =
                chase_goal_for_target(map, entities, id, (px, py), (tx, ty), range_px, dist);
            let want_repath = entities
                .get(id)
                .map(|e| chase_path_needs_refresh(e, chase_goal))
                .unwrap_or(false);
            if let Some(e) = entities.get_mut(id) {
                if e.kind == EntityKind::Tank {
                    rotate_tank_weapon_for_combat(e, target_angle);
                } else if target_angle.is_finite() {
                    mirror_weapon_to_body(e, e.facing());
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Chasing);
            }
            if !machine_gunner_ready_to_move(entities, id) {
                continue;
            }
            if want_repath {
                coordinator.request_chase_path(entities, id, chase_goal);
            }
        }
    }
}

fn chase_goal_for_target(
    map: &Map,
    entities: &EntityStore,
    attacker_id: u32,
    attacker_pos: (f32, f32),
    target_pos: (f32, f32),
    range_px: f32,
    dist: f32,
) -> (f32, f32) {
    let is_out_of_range_tank = entities
        .get(attacker_id)
        .map(|e| e.kind == EntityKind::Tank && dist > range_px)
        .unwrap_or(false);
    if !is_out_of_range_tank {
        return target_pos;
    }
    tank_standoff_goal(map, attacker_pos, target_pos, range_px).unwrap_or(target_pos)
}

fn tank_standoff_goal(
    map: &Map,
    attacker_pos: (f32, f32),
    target_pos: (f32, f32),
    range_px: f32,
) -> Option<(f32, f32)> {
    let (px, py) = attacker_pos;
    let (tx, ty) = target_pos;
    if !px.is_finite()
        || !py.is_finite()
        || !tx.is_finite()
        || !ty.is_finite()
        || !range_px.is_finite()
    {
        return None;
    }
    let dx = px - tx;
    let dy = py - ty;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= f32::EPSILON || !dist.is_finite() {
        return None;
    }
    let buffer = TANK_STANDOFF_BUFFER_PX.min(range_px * 0.5);
    let desired_dist = (range_px - buffer).max(0.0);
    let ux = dx / dist;
    let uy = dy / dist;
    let max = map.world_size_px() - 0.01;
    Some((
        (tx + ux * desired_dist).clamp(0.0, max),
        (ty + uy * desired_dist).clamp(0.0, max),
    ))
}

fn chase_path_needs_refresh(e: &Entity, chase_goal: (f32, f32)) -> bool {
    if e.path_is_empty() {
        return true;
    }
    if e.kind != EntityKind::Tank {
        return false;
    }
    e.path_goal()
        .map(|goal| {
            (goal.0 - chase_goal.0).abs() > TANK_STANDOFF_REPATH_DELTA_PX
                || (goal.1 - chase_goal.1).abs() > TANK_STANDOFF_REPATH_DELTA_PX
        })
        .unwrap_or(true)
}

fn rotate_tank_weapon_for_combat(e: &mut Entity, target_angle: f32) -> bool {
    if !target_angle.is_finite() {
        return false;
    }
    e.set_desired_weapon_facing(target_angle);
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or_else(|| e.facing());
    let rotated = rotate_toward(current, target_angle, TANK_TURRET_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_weapon_facing(rotated);
    }
    angle_delta(rotated, target_angle).abs() <= TANK_TURRET_FIRE_TOLERANCE_RAD
}

fn relax_tank_weapon_toward_body(e: &mut Entity) {
    let body = e.facing();
    if !body.is_finite() {
        return;
    }
    e.set_desired_weapon_facing(body);
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or(body);
    let rotated = rotate_toward(current, body, TANK_TURRET_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_weapon_facing(rotated);
    }
}

fn mirror_weapon_to_body(e: &mut Entity, angle: f32) {
    if !angle.is_finite() {
        return;
    }
    e.set_desired_weapon_facing(angle);
    e.set_weapon_facing(angle);
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
    /// Engages and chases any enemy within acquisition range.
    Aggressive,
    /// Engages enemies already in weapon range, without chasing them.
    Opportunistic,
    /// Ignores nearby enemies unless explicitly ordered to attack.
    Passive,
}

fn combat_mode(e: &Entity) -> CombatMode {
    match e.order() {
        Order::Attack(_) => CombatMode::Ordered,
        Order::AttackMove(_) => CombatMode::Aggressive,
        Order::Move(_) if e.kind == EntityKind::Tank => CombatMode::Opportunistic,
        Order::Idle if e.is_building() => CombatMode::Aggressive,
        Order::Idle if e.is_unit() && e.kind != EntityKind::Worker => CombatMode::Aggressive,
        _ => CombatMode::Passive,
    }
}

/// Resolve which entity an attacker should engage this tick.
#[allow(clippy::too_many_arguments)]
fn resolve_target(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
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

    if matches!(mode, CombatMode::Passive) {
        return None;
    }

    // AT teams prefer tanks over all other targets; fall back to nearest enemy if no tank
    // is in range.
    let prefers_armored = entities
        .get(self_id)
        .map(|e| combat_rules::prefers_armored_targets(e.kind))
        .unwrap_or(false);
    if prefers_armored {
        if let Some(id) = world_query::nearest_tank_in_range_filtered(
            entities,
            spatial,
            self_id,
            owner,
            px,
            py,
            acquire_px,
            |target| los.clear_between_world_points((px, py), (target.pos_x, target.pos_y)),
        ) {
            return Some(id);
        }
    }

    // Aggressive acquisition: the nearest enemy within the acquire radius (weapon range for
    // buildings, sight range for mobile units so they chase).
    world_query::nearest_enemy_in_range_filtered(
        entities,
        spatial,
        self_id,
        owner,
        px,
        py,
        acquire_px,
        |target| los.clear_between_world_points((px, py), (target.pos_x, target.pos_y)),
    )
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
    players: &[PlayerState],
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
    let victim_facing = entities.get(victim).map(|e| e.facing());
    // Roll for miss before computing damage.
    if let (Some(ak), Some(vk)) = (attacker_kind, victim_kind) {
        let mc = combat_rules::miss_chance(ak, vk);
        if mc > 0.0 && rng.gen::<f32>() < mc {
            return;
        }
    }
    let effective_dmg = match (attacker_kind, victim_kind) {
        (Some(ak), Some(vk)) => combat_rules::effective_damage_with_facing(
            ak,
            vk,
            dmg,
            Some(TerrainKind::Open),
            victim_facing,
            (vx, vy),
            (ax, ay),
        ),
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
    retreat_worker_from_direct_hit(
        map,
        entities,
        coordinator,
        players,
        victim,
        attacker_owner,
        (ax, ay),
    );
    apply_overpenetration(
        map,
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
    players: &[PlayerState],
    victim: u32,
    attacker_owner: u32,
    attacker_pos: (f32, f32),
) {
    let (owner, vx, vy) = match entities.get(victim) {
        Some(worker)
            if worker.kind == EntityKind::Worker
                && worker.owner != attacker_owner
                && player_is_ai(players, worker.owner)
                && worker.hp > 0
                && !matches!(worker.build_phase(), Some(BuildPhase::Constructing { .. })) =>
        {
            (worker.owner, worker.pos_x, worker.pos_y)
        }
        _ => return,
    };
    let dx = vx - attacker_pos.0;
    let dy = vy - attacker_pos.1;
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

fn player_is_ai(players: &[PlayerState], player_id: u32) -> bool {
    players
        .iter()
        .find(|player| player.id == player_id)
        .map(|player| player.is_ai)
        .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
fn apply_overpenetration(
    map: &Map,
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
    // A tank's armour stops the round dead: hitting a tank never overpenetrates, no exceptions.
    if entities
        .get(primary_victim)
        .map(|e| e.kind == EntityKind::Tank)
        .unwrap_or(false)
    {
        return;
    }
    let dx = vx - ax;
    let dy = vy - ay;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= f32::EPSILON {
        return;
    }
    // AT teams are built to punch through: their rounds carry twice the normal depth past the
    // primary target. Everyone else gets the base 25% of weapon range.
    let overpenetration_factor = match entities.get(attacker).map(|e| e.kind) {
        Some(EntityKind::AtTeam) => 0.50,
        _ => 0.25,
    };
    let overpenetration_limit = dist + range_px * overpenetration_factor;
    let ux = dx / dist;
    let uy = dy / dist;
    let perpendicular_slack = RANGE_SLACK + 8.0;
    let splash_dmg = primary_dmg / 2;
    if splash_dmg == 0 {
        return;
    }

    let player_ids: Vec<u32> = events.keys().copied().collect();
    let mut hits: Vec<(u32, f32, f32, f32, f32)> = Vec::new();
    let los = LineOfSight::new(map);
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
        if !los.clear_between_world_points((ax, ay), (target.pos_x, target.pos_y)) {
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
                Some(ak) => combat_rules::effective_damage_with_facing(
                    ak,
                    e.kind,
                    splash_dmg,
                    Some(TerrainKind::Open),
                    Some(e.facing()),
                    (e.pos_x, e.pos_y),
                    (ax, ay),
                ),
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
    use crate::game::ScoreState;
    use crate::protocol::terrain;
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

    fn open_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4), (size - 5, size - 5)],
            expansion_sites: Vec::new(),
        }
    }

    fn map_with_rock_at(tile: (u32, u32)) -> Map {
        let mut map = open_map(12);
        map.terrain[(tile.1 * map.size + tile.0) as usize] = terrain::ROCK;
        map
    }

    fn player_state(id: u32, is_ai: bool) -> PlayerState {
        PlayerState {
            id,
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (4, 4),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            supply_cap: 20,
            is_ai,
            score: ScoreState::default(),
        }
    }

    fn run_combat_tick(entities: &mut EntityStore) -> HashMap<u32, Vec<Event>> {
        run_combat_tick_with_players(entities, &[player_state(1, false), player_state(2, false)])
    }

    fn run_combat_tick_with_players(
        entities: &mut EntityStore,
        players: &[PlayerState],
    ) -> HashMap<u32, Vec<Event>> {
        let map = Map::generate(2, 0x00C0_FFEE);
        run_combat_tick_on_map(entities, players, &map)
    }

    fn run_combat_tick_on_map(
        entities: &mut EntityStore,
        players: &[PlayerState],
        map: &Map,
    ) -> HashMap<u32, Vec<Event>> {
        let occ = Occupancy::build(map, entities);
        let spatial = SpatialIndex::build(entities, map.size);
        let mut pathing = PathingService::new(256, 64);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 10);
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], entities, map);
        let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

        let mut rng = SmallRng::seed_from_u64(0);
        combat_system(
            map,
            entities,
            players,
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
            &[player_state(1, false), player_state(2, false)],
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
        let map = open_map(8);
        let los = LineOfSight::new(&map);
        let spatial = SpatialIndex::build(&entities, map.size);
        let attacker = entities.get(self_id).expect("attacker should exist");

        let target = resolve_target(
            &entities,
            &spatial,
            &los,
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
        let map = open_map(8);
        let los = LineOfSight::new(&map);
        let spatial = SpatialIndex::build(&entities, map.size);
        let attacker = entities.get_mut(self_id).expect("attacker should exist");
        attacker.set_order(Order::move_to(300.0, 300.0));

        let target = resolve_target(
            &entities,
            &spatial,
            &los,
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
        let map = open_map(8);
        let los = LineOfSight::new(&map);
        let spatial = SpatialIndex::build(&entities, map.size);
        let attacker = entities.get_mut(self_id).expect("attacker should exist");
        attacker.set_order(Order::attack_move_to(300.0, 300.0));

        let target = resolve_target(
            &entities,
            &spatial,
            &los,
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
    fn stone_blocks_attack_move_auto_acquisition() {
        let map = map_with_rock_at((3, 4));
        let mut entities = EntityStore::new();
        let attacker_pos = map.tile_center(2, 4);
        let enemy_pos = map.tile_center(4, 4);
        let self_id = entities
            .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
            .expect("attacker should spawn");
        entities
            .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
            .expect("enemy should spawn");
        let los = LineOfSight::new(&map);
        let spatial = SpatialIndex::build(&entities, map.size);
        entities
            .get_mut(self_id)
            .expect("attacker should exist")
            .set_order(Order::attack_move_to(300.0, 300.0));
        let attacker = entities.get(self_id).expect("attacker should exist");

        let target = resolve_target(
            &entities,
            &spatial,
            &los,
            self_id,
            attacker.owner,
            attacker.pos_x,
            attacker.pos_y,
            128.0,
            combat_mode(attacker),
        );

        assert_eq!(target, None);
    }

    #[test]
    fn stone_blocks_explicit_attack_damage_until_shot_is_clear() {
        let map = map_with_rock_at((3, 4));
        let mut entities = EntityStore::new();
        let attacker_pos = map.tile_center(2, 4);
        let enemy_pos = map.tile_center(4, 4);
        let attacker_id = entities
            .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
            .expect("attacker should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
            .expect("enemy should spawn");
        entities
            .get_mut(attacker_id)
            .expect("attacker should exist")
            .set_order(Order::attack(enemy_id));
        let before_hp = entities.get(enemy_id).expect("enemy should exist").hp;

        let events = run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );

        assert_eq!(
            entities.get(enemy_id).expect("enemy should exist").hp,
            before_hp
        );
        assert!(
            events
                .values()
                .flatten()
                .all(|event| !matches!(event, Event::Attack { .. })),
            "blocked shots should not emit attack tracers"
        );
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
        fog.recompute(&[1], &entities, &map);
        let mut events = HashMap::from([(1, Vec::new())]);

        let mut rng = SmallRng::seed_from_u64(0);
        combat_system(
            &map,
            &mut entities,
            &[player_state(1, false)],
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
    fn tank_keeps_moving_path_while_firing() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_facing(0.0);
            tank.set_weapon_facing(0.0);
            tank.set_order(Order::attack_move_to(300.0, 100.0));
            tank.set_path(vec![(300.0, 100.0)]);
            tank.set_path_goal(Some((300.0, 100.0)));
        }

        run_combat_tick(&mut entities);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert_eq!(tank.target_id(), Some(enemy_id));
        assert!(
            !tank.path_is_empty(),
            "tank should keep its movement path while firing"
        );
        assert_eq!(tank.next_waypoint(), Some((300.0, 100.0)));
    }

    #[test]
    fn tank_move_order_fires_without_leaving_move_path() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_facing(0.0);
            tank.set_weapon_facing(0.0);
            tank.set_order(Order::move_to(300.0, 100.0));
            tank.set_path(vec![(300.0, 100.0)]);
            tank.set_path_goal(Some((300.0, 100.0)));
        }

        run_combat_tick(&mut entities);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert_eq!(tank.target_id(), Some(enemy_id));
        assert!(
            tank.attack_cd() > 0,
            "aligned moving tank turret should fire"
        );
        assert!(
            !tank.path_is_empty(),
            "moving tank should keep its movement path while firing"
        );
        assert_eq!(tank.next_waypoint(), Some((300.0, 100.0)));
    }

    #[test]
    fn tank_move_order_does_not_chase_targets_outside_weapon_range() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        entities
            .spawn_unit(2, EntityKind::Rifleman, 260.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_order(Order::move_to(300.0, 100.0));
            tank.set_path(vec![(300.0, 100.0)]);
            tank.set_path_goal(Some((300.0, 100.0)));
        }

        run_combat_tick(&mut entities);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert_eq!(tank.target_id(), None);
        assert_eq!(tank.path_goal(), Some((300.0, 100.0)));
        assert_eq!(tank.next_waypoint(), Some((300.0, 100.0)));
    }

    #[test]
    fn tank_chases_to_standoff_range_instead_of_target_center() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 280.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_order(Order::attack_move_to(400.0, 100.0));
            tank.set_path(Vec::new());
            tank.set_path_goal(Some((400.0, 100.0)));
        }

        let map = open_map(20);
        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );

        let tank = entities.get(tank_id).expect("tank should exist");
        let enemy = entities.get(enemy_id).expect("enemy should exist");
        let goal = tank.path_goal().expect("tank should request a chase path");
        let profile = combat_rules::attack_profile(EntityKind::Tank);
        let range_px =
            profile.range_tiles as f32 * config::TILE_SIZE as f32 + tank.radius() + RANGE_SLACK;
        let goal_to_enemy = dist2(goal.0, goal.1, enemy.pos_x, enemy.pos_y).sqrt();

        assert_ne!(goal, (enemy.pos_x, enemy.pos_y));
        assert!(
            goal_to_enemy < range_px,
            "standoff goal should be comfortably inside weapon range"
        );
    }

    #[test]
    fn tank_chase_refreshes_stale_standoff_goal() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 320.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_order(Order::attack_move_to(500.0, 100.0));
            tank.set_path(vec![(192.0, 100.0)]);
            tank.set_path_goal(Some((192.0, 100.0)));
            tank.set_last_repath_tick(10);
        }

        let map = open_map(20);
        let old_goal = entities
            .get(tank_id)
            .expect("tank should exist")
            .path_goal()
            .expect("old goal should exist");

        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );

        let tank = entities.get(tank_id).expect("tank should exist");
        let enemy = entities.get(enemy_id).expect("enemy should exist");
        let goal = tank.path_goal().expect("tank should keep a chase goal");

        assert_ne!(goal, old_goal);
        assert!(
            goal.0 < enemy.pos_x,
            "tank should route to the near side of the target, not the target center"
        );
    }

    #[test]
    fn non_tank_attack_move_still_holds_position_while_firing() {
        let mut entities = EntityStore::new();
        let rifleman_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy should spawn");
        if let Some(rifleman) = entities.get_mut(rifleman_id) {
            rifleman.set_order(Order::attack_move_to(300.0, 100.0));
            rifleman.set_path(vec![(300.0, 100.0)]);
            rifleman.set_path_goal(Some((300.0, 100.0)));
        }

        run_combat_tick(&mut entities);

        let rifleman = entities.get(rifleman_id).expect("rifleman should exist");
        assert_eq!(rifleman.target_id(), Some(enemy_id));
        assert!(
            rifleman.path_is_empty(),
            "non-tank units should still stop while firing"
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
        let map = open_map(8);
        let los = LineOfSight::new(&map);
        let spatial = SpatialIndex::build(&entities, map.size);
        let worker = entities.get(worker_id).expect("worker should exist");

        let target = resolve_target(
            &entities,
            &spatial,
            &los,
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
    fn directly_hit_ai_workers_retreat_from_attacker() {
        let mut entities = EntityStore::new();
        let worker_id = entities
            .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
            .expect("worker should spawn");
        entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
            .expect("enemy rifleman should spawn");

        run_combat_tick_with_players(
            &mut entities,
            &[player_state(1, true), player_state(2, false)],
        );

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
    fn directly_hit_human_workers_do_not_retreat_from_attacker() {
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
            matches!(worker.order(), Order::Idle),
            "human workers should not receive a forced retreat order"
        );
        assert_eq!(
            worker.path_goal(),
            None,
            "human workers should not receive a retreat path goal"
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
    fn tank_combat_keeps_body_stable_and_rotates_turret() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0, 140.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_facing(0.0);
            tank.set_weapon_facing(0.0);
            tank.set_order(Order::attack(enemy_id));
        }

        run_combat_tick(&mut entities);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert_eq!(
            tank.facing(),
            0.0,
            "tank combat should not rotate the hull once turret state exists"
        );
        assert!(
            tank.weapon_facing().unwrap_or(0.0) > 0.0
                && tank.weapon_facing().unwrap_or(0.0)
                    <= TANK_TURRET_TURN_RATE_RAD_PER_TICK + 0.0001,
            "tank turret should rotate gradually toward target, got {:.4}",
            tank.weapon_facing().unwrap_or(0.0)
        );
        assert_eq!(
            tank.attack_cd(),
            0,
            "misaligned turret should not fire on the same tick it starts turning"
        );
    }

    #[test]
    fn tank_cannot_fire_until_turret_aligned() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_facing(std::f32::consts::FRAC_PI_2);
            tank.set_weapon_facing(std::f32::consts::FRAC_PI_2);
            tank.set_order(Order::attack(enemy_id));
        }
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

        for _ in 0..10 {
            run_combat_tick(&mut entities);
        }
        assert_eq!(
            entities.get(enemy_id).expect("enemy should exist").hp,
            enemy_hp,
            "tank should not damage the target while turret aim is outside tolerance"
        );
        assert_eq!(
            entities
                .get(tank_id)
                .expect("tank should exist")
                .attack_cd(),
            0,
            "tank cooldown should remain ready while firing is gated by turret alignment"
        );
        assert_eq!(
            entities.get(tank_id).expect("tank should exist").facing(),
            std::f32::consts::FRAC_PI_2,
            "turret aiming must not rotate the hull"
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
            "tank should fire once its turret rotates inside tolerance"
        );
    }

    #[test]
    fn tank_can_fire_outside_hull_facing_once_turret_aligned() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_facing(std::f32::consts::PI);
            tank.set_weapon_facing(0.08);
            tank.set_order(Order::attack(enemy_id));
        }
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;

        run_combat_tick(&mut entities);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert_eq!(
            tank.facing(),
            std::f32::consts::PI,
            "hull may remain pointed away from the target"
        );
        assert!(
            tank.attack_cd() > 0,
            "aligned turret should allow firing even when hull faces away"
        );
        assert!(
            entities.get(enemy_id).expect("enemy should exist").hp < enemy_hp,
            "target should take tank damage once turret is aligned"
        );
    }

    #[test]
    fn tank_front_and_rear_hits_take_different_damage() {
        fn tank_hp_after_at_hit(attacker_pos: (f32, f32)) -> u32 {
            let mut entities = EntityStore::new();
            let attacker = entities
                .spawn_unit(1, EntityKind::AtTeam, attacker_pos.0, attacker_pos.1)
                .expect("attacker should spawn");
            let victim = entities
                .spawn_unit(2, EntityKind::Tank, 100.0, 100.0)
                .expect("victim tank should spawn");
            entities
                .get_mut(victim)
                .expect("victim tank should exist")
                .set_facing(0.0);
            let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
            events.insert(1, Vec::new());
            events.insert(2, Vec::new());

            apply_test_damage(
                &mut entities,
                &mut events,
                attacker,
                victim,
                48,
                1,
                attacker_pos.0,
                attacker_pos.1,
                100.0,
                100.0,
                128.0,
            );

            entities.get(victim).expect("victim tank should exist").hp
        }

        let front_hp = tank_hp_after_at_hit((140.0, 100.0));
        let rear_hp = tank_hp_after_at_hit((60.0, 100.0));

        assert_eq!(front_hp, 342);
        assert_eq!(rear_hp, 306);
        assert!(
            front_hp > rear_hp,
            "rear AT hits should deal more damage than front hits"
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

    #[test]
    fn attacking_a_tank_never_overpenetrates() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let tank = entities
            .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
            .expect("tank primary target should spawn");
        // Directly behind the tank, well inside the normal overpenetration band so the only
        // reason it survives is the tank stopping the round.
        let behind = entities
            .spawn_unit(2, EntityKind::Rifleman, 165.0, 100.0)
            .expect("unit behind the tank should spawn");
        let behind_hp_before = entities.get(behind).expect("behind should exist").hp;
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        events.insert(1, Vec::new());
        events.insert(2, Vec::new());

        apply_test_damage(
            &mut entities,
            &mut events,
            attacker,
            tank,
            20,
            1,
            100.0,
            100.0,
            140.0,
            100.0,
            128.0,
        );

        assert_eq!(
            entities.get(behind).expect("behind should exist").hp,
            behind_hp_before,
            "a shot whose primary target is a tank must not overpenetrate"
        );
    }

    #[test]
    fn at_teams_overpenetrate_twice_as_far() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("AT team should spawn");
        // An armored, non-tank primary: AT teams never miss armored targets, and a building does
        // not trigger the tank-stops-the-round rule, so the shot reliably overpenetrates.
        let primary = entities
            .spawn_building(2, EntityKind::Barracks, 140.0, 100.0, true)
            .expect("primary target should spawn");
        // 90px along the shot line: past the 72px base band (dist 40 + 0.25*128) but inside the
        // 104px AT band (dist 40 + 0.50*128). A normal attacker would miss it.
        let deep = entities
            .spawn_unit(2, EntityKind::Rifleman, 190.0, 100.0)
            .expect("deep target should spawn");
        let deep_hp_before = entities.get(deep).expect("deep should exist").hp;
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        events.insert(1, Vec::new());
        events.insert(2, Vec::new());

        apply_test_damage(
            &mut entities,
            &mut events,
            attacker,
            primary,
            20,
            1,
            100.0,
            100.0,
            140.0,
            100.0,
            128.0,
        );

        assert!(
            entities.get(deep).expect("deep should exist").hp < deep_hp_before,
            "AT teams should overpenetrate to twice the normal depth"
        );
    }
}
