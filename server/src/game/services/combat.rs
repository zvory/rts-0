use std::collections::HashMap;

use crate::config;
use crate::game::entity::{
    fires_while_moving, AttackPhase, Entity, EntityKind, EntityStore, Order, WeaponSetup,
};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::geometry::{
    building_rect_for_entity, segment_intersects_rect, segment_intersects_unit_body,
    unit_body_for_entity,
};
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::movement::{angle_delta, rotate_toward};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::PlayerState;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules::combat as combat_rules;
use crate::rules::projection;
use crate::rules::terrain::TerrainKind;
use rand::rngs::SmallRng;
use rand::Rng;

/// Extra slack (px) added to attack range checks so units don't dance at the exact boundary.
const RANGE_SLACK: f32 = 4.0;
const TANK_TURRET_TURN_RATE_RAD_PER_TICK: f32 = 0.070;
const TANK_TURRET_FIRE_TOLERANCE_RAD: f32 = 0.18;
const AT_GUN_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
const AT_GUN_FIRE_TOLERANCE_RAD: f32 = 0.12;
const TANK_STANDOFF_BUFFER_PX: f32 = config::TILE_SIZE as f32;
const TANK_STANDOFF_REPATH_DELTA_PX: f32 = config::TILE_SIZE as f32;

/// Combat: acquire targets for aggressive / attack-move units, let eligible idle units
/// auto-acquire enemies, and deal damage when off cooldown. Damage is applied immediately and
/// emits an `Attack` event (for tracers). Cooldowns tick down here too.
#[allow(clippy::too_many_arguments)]
pub(crate) fn combat_system(
    map: &Map,
    entities: &mut EntityStore,
    _players: &[PlayerState],
    _occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    rng: &mut SmallRng,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
) {
    let los = LineOfSight::new(map);
    // Tick down cooldowns first.
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            e.tick_attack_cd();
            tick_deployed_weapon_setup(e);
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
            let profile = effective_attack_profile(e);
            let (range_tiles, dmg, cd) = (profile.range_tiles, profile.dmg, profile.cooldown);
            let range_px = range_tiles as f32 * config::TILE_SIZE as f32 + e.radius() + RANGE_SLACK;
            // Aggro radius: mobile units detect and chase enemies out to their sight radius so
            // attack-move / auto-defend actually close the gap. Idle deployed weapons are the
            // exception: they hold position and only auto-acquire enemies already in weapon
            // range. Buildings never move, so they only ever engage within their firing range.
            let aggro_px = if e.is_unit() {
                if uses_stationary_weapon_aggro(e) && matches!(e.order(), Order::Idle) {
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
                    begin_idle_deployed_weapon_setup(e);
                }
                if fires_while_moving(e.kind) {
                    relax_vehicle_weapon_toward_body(e);
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
                if fires_while_moving(e.kind) {
                    weapon_aligned = rotate_vehicle_weapon_for_combat(e, target_angle);
                } else if e.kind == EntityKind::AtTeam {
                    weapon_aligned = rotate_at_gun_for_combat(e, target_angle);
                } else if target_angle.is_finite() {
                    e.set_facing(target_angle);
                    mirror_weapon_to_body(e, target_angle);
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Firing);
                // Most units hold position while firing. Vehicle weapons can track independently,
                // so those units keep driving along their current path while the weapon tracks.
                if !fires_while_moving(e.kind) {
                    e.clear_path();
                }
            }
            if !weapon_aligned {
                continue;
            }
            if !deployed_weapon_ready_to_fire(entities, id) {
                continue;
            }
            let ready = matches!(entities.get(id), Some(e) if e.attack_cd() == 0);
            if ready {
                apply_damage(
                    map, entities, events, fog, rng, id, tid, dmg, owner, px, py, tx, ty, range_px,
                    tick,
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
            let mut can_chase = true;
            if let Some(e) = entities.get_mut(id) {
                if fires_while_moving(e.kind) {
                    rotate_vehicle_weapon_for_combat(e, target_angle);
                } else if e.kind == EntityKind::AtTeam {
                    rotate_at_gun_for_combat(e, target_angle);
                } else if target_angle.is_finite() {
                    mirror_weapon_to_body(e, e.facing());
                }
                e.set_target_id(Some(tid));
                e.mark_attack_phase(AttackPhase::Chasing);
                can_chase = at_gun_can_chase(e);
            }
            if !can_chase {
                continue;
            }
            if !deployed_weapon_ready_to_move(entities, id) {
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
        .map(|e| fires_while_moving(e.kind) && dist > range_px)
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
    if !fires_while_moving(e.kind) {
        return false;
    }
    e.path_goal()
        .map(|goal| {
            (goal.0 - chase_goal.0).abs() > TANK_STANDOFF_REPATH_DELTA_PX
                || (goal.1 - chase_goal.1).abs() > TANK_STANDOFF_REPATH_DELTA_PX
        })
        .unwrap_or(true)
}

fn rotate_vehicle_weapon_for_combat(e: &mut Entity, target_angle: f32) -> bool {
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

fn relax_vehicle_weapon_toward_body(e: &mut Entity) {
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

fn rotate_at_gun_for_combat(e: &mut Entity, target_angle: f32) -> bool {
    if !target_angle.is_finite() {
        return false;
    }
    let desired = deployed_at_gun_desired_facing(e, target_angle);
    e.set_desired_weapon_facing(desired);
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or_else(|| {
            let facing = e.facing();
            if facing.is_finite() {
                facing
            } else {
                0.0
            }
        });
    let rotated = rotate_toward(current, desired, AT_GUN_TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_facing(rotated);
        e.set_weapon_facing(rotated);
    } else {
        return false;
    }
    at_gun_target_inside_field_of_fire(e, target_angle)
        && angle_delta(rotated, target_angle).abs() <= AT_GUN_FIRE_TOLERANCE_RAD
}

fn tick_deployed_weapon_setup(e: &mut Entity) {
    if !requires_weapon_setup(e.kind) {
        return;
    }
    e.tick_weapon_setup();
}

fn begin_idle_deployed_weapon_setup(e: &mut Entity) {
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

fn deployed_weapon_ready_to_fire(entities: &mut EntityStore, id: u32) -> bool {
    let Some(e) = entities.get_mut(id) else {
        return false;
    };
    if !requires_weapon_setup(e.kind) || e.kind == EntityKind::AtTeam {
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

fn deployed_weapon_ready_to_move(entities: &mut EntityStore, id: u32) -> bool {
    let Some(e) = entities.get_mut(id) else {
        return false;
    };
    if !requires_weapon_setup(e.kind) {
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

fn requires_weapon_setup(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::MachineGunner | EntityKind::AtTeam)
}

fn uses_stationary_weapon_aggro(e: &Entity) -> bool {
    matches!(e.kind, EntityKind::MachineGunner)
        || (e.kind == EntityKind::AtTeam && !matches!(e.weapon_setup(), WeaponSetup::Packed))
}

fn at_gun_can_chase(e: &Entity) -> bool {
    e.kind != EntityKind::AtTeam || matches!(e.weapon_setup(), WeaponSetup::Packed)
}

fn effective_attack_profile(e: &Entity) -> combat_rules::AttackProfile {
    let mut profile = combat_rules::attack_profile(e.kind);
    if e.kind != EntityKind::AtTeam {
        return profile;
    }
    match e.weapon_setup() {
        WeaponSetup::Packed => {
            profile.range_tiles = config::AT_GUN_PACKED_RANGE_TILES;
            profile.dmg =
                ((profile.dmg as f32) * config::AT_GUN_PACKED_DAMAGE_MULTIPLIER).round() as u32;
        }
        WeaponSetup::Deployed => {
            profile.range_tiles = config::AT_GUN_DEPLOYED_RANGE_TILES;
        }
        WeaponSetup::SettingUp { .. } | WeaponSetup::TearingDown { .. } => {
            profile.range_tiles = config::AT_GUN_PACKED_RANGE_TILES;
            profile.dmg = 0;
        }
    }
    profile
}

fn deployed_at_gun_desired_facing(e: &Entity, target_angle: f32) -> f32 {
    if !matches!(e.weapon_setup(), WeaponSetup::Deployed) {
        return target_angle;
    }
    let Some(center) = at_gun_field_center(e) else {
        return target_angle;
    };
    let half = config::AT_GUN_FIELD_OF_FIRE_RAD * 0.5;
    let delta = angle_delta(center, target_angle);
    if delta.abs() <= half {
        target_angle
    } else {
        center + delta.signum() * half
    }
}

fn at_gun_target_inside_field_of_fire(e: &Entity, target_angle: f32) -> bool {
    if !matches!(e.weapon_setup(), WeaponSetup::Deployed) {
        return true;
    }
    let Some(center) = at_gun_field_center(e) else {
        return true;
    };
    angle_delta(center, target_angle).abs() <= config::AT_GUN_FIELD_OF_FIRE_RAD * 0.5
}

fn at_gun_field_center(e: &Entity) -> Option<f32> {
    e.emplacement_facing()
        .or_else(|| e.weapon_facing())
        .filter(|facing| facing.is_finite())
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
        Order::Move(_) if fires_while_moving(e.kind) => CombatMode::Opportunistic,
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

    if let Some(target) = retained_firing_target_for_shoot_while_moving_unit(
        entities, los, self_id, owner, px, py, acquire_px,
    ) {
        return Some(target);
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

fn retained_firing_target_for_shoot_while_moving_unit(
    entities: &EntityStore,
    los: &LineOfSight<'_>,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
) -> Option<u32> {
    let attacker = entities.get(self_id)?;
    if !fires_while_moving(attacker.kind) {
        return None;
    }
    let target_id = attacker.target_id()?;
    let target = entities.get(target_id)?;
    if !world_query::is_enemy_targetable(target, owner, self_id) {
        return None;
    }
    let concealment =
        crate::rules::terrain::concealment_modifier(target.kind, TerrainKind::Open).max(0.0);
    let effective_acquire_px = acquire_px * concealment;
    if !effective_acquire_px.is_finite() {
        return None;
    }
    let dx = target.pos_x - px;
    let dy = target.pos_y - py;
    if dx * dx + dy * dy > effective_acquire_px * effective_acquire_px {
        return None;
    }
    if !los.clear_between_world_points((px, py), (target.pos_x, target.pos_y)) {
        return None;
    }
    Some(target_id)
}

/// Apply `dmg` to `victim` from `attacker`, emitting an `Attack` event for every fired shot.
/// Death itself is handled by the death system (we only zero hp here).
#[allow(clippy::too_many_arguments)]
fn apply_damage(
    map: &Map,
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
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
    tick: u32,
) {
    if entities.get(victim).map(|e| e.is_node()).unwrap_or(false) {
        return;
    }
    let shot_victim = resolve_shot_victim(map, entities, attacker, victim, attacker_owner, ax, ay);
    let Some(shot_victim) = shot_victim else {
        return;
    };
    let shot_victim_pos = entities
        .get(shot_victim)
        .map(|e| (e.pos_x, e.pos_y))
        .unwrap_or((vx, vy));
    let attacker_kind = entities.get(attacker).map(|e| e.kind);
    let victim_kind = entities.get(shot_victim).map(|e| e.kind);
    let victim_facing = entities.get(shot_victim).map(|e| e.facing());
    let victim_owner = entities.get(shot_victim).map(|e| e.owner).unwrap_or(0);
    emit_attack_event(
        events,
        fog,
        attacker,
        shot_victim,
        attacker_owner,
        ax,
        ay,
        shot_victim_pos.0,
        shot_victim_pos.1,
    );

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
            shot_victim_pos,
            (ax, ay),
        ),
        _ => dmg,
    };
    let damaged = if let Some(v) = entities.get_mut(shot_victim) {
        if v.hp > 0 && effective_dmg > 0 {
            v.hp = v.hp.saturating_sub(effective_dmg);
            if v.owner != attacker_owner {
                v.set_last_damage_owner(Some(attacker_owner));
                v.record_damage_from((ax, ay), tick);
            }
            true
        } else {
            false
        }
    } else {
        false
    };
    if damaged {
        apply_overpenetration(
            map,
            entities,
            events,
            fog,
            attacker,
            shot_victim,
            effective_dmg,
            attacker_owner,
            ax,
            ay,
            shot_victim_pos.0,
            shot_victim_pos.1,
            range_px,
            tick,
        );
        push_under_attack_notices_for_visible_attack(
            events,
            fog,
            victim_owner,
            attacker_owner,
            ax,
            ay,
            shot_victim_pos.0,
            shot_victim_pos.1,
        );
    }
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
    tick: u32,
) {
    if entities
        .get(primary_victim)
        .map(|e| e.kind == EntityKind::Tank || e.is_building())
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

    let overpenetration_factor = match entities.get(attacker).map(|e| e.kind) {
        Some(EntityKind::AtTeam) => 0.50,
        _ => 0.25,
    };
    let overpenetration_limit = dist + range_px * overpenetration_factor;
    let ux = dx / dist;
    let uy = dy / dist;
    let shot_end = (
        ax + ux * overpenetration_limit,
        ay + uy * overpenetration_limit,
    );
    let perpendicular_slack = RANGE_SLACK + 8.0;
    let splash_dmg = primary_dmg / 2;
    if splash_dmg == 0 {
        return;
    }

    let player_ids: Vec<u32> = events.keys().copied().collect();
    let mut hits: Vec<(u32, f32, f32, f32)> = Vec::new();
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
        let along = if target.kind == EntityKind::Tank || target.is_building() {
            let Some(hit_t) = shot_blocker_intersection(map, target, (ax, ay), shot_end) else {
                continue;
            };
            hit_t * overpenetration_limit
        } else {
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
            along
        };
        if along <= dist || along > overpenetration_limit {
            continue;
        }
        if !los.clear_between_world_points((ax, ay), (target.pos_x, target.pos_y)) {
            continue;
        }
        hits.push((id, target.pos_x, target.pos_y, along));
    }

    hits.sort_by(|a, b| a.3.total_cmp(&b.3).then_with(|| a.0.cmp(&b.0)));
    for (id, tx, ty, _) in hits {
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
        let victim_owner = entities.get(id).map(|e| e.owner).unwrap_or(0);
        let shot_blocked = entities
            .get(id)
            .map(|e| e.kind == EntityKind::Tank || e.is_building())
            .unwrap_or(false);
        if let Some(v) = entities.get_mut(id) {
            if v.hp > 0 {
                v.hp = v.hp.saturating_sub(effective_dmg);
                v.set_last_damage_owner(Some(attacker_owner));
                v.record_damage_from((ax, ay), tick);
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
            push_under_attack_notice(events, *pid, victim_owner, attacker_owner, tx, ty);
        }
        if shot_blocked {
            break;
        }
    }
}

fn resolve_shot_victim(
    map: &Map,
    entities: &EntityStore,
    attacker: u32,
    intended_victim: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
) -> Option<u32> {
    let victim = entities.get(intended_victim)?;
    let end = (victim.pos_x, victim.pos_y);
    if !ax.is_finite() || !ay.is_finite() || !end.0.is_finite() || !end.1.is_finite() {
        return Some(intended_victim);
    }

    let mut best = (intended_victim, 1.0f32);
    for candidate in entities.iter() {
        if candidate.id == attacker
            || candidate.is_node()
            || candidate.owner == attacker_owner
            || candidate.hp == 0
        {
            continue;
        }
        let Some(hit_t) = shot_blocker_intersection(map, candidate, (ax, ay), end) else {
            continue;
        };
        if hit_t <= best.1 + f32::EPSILON
            && (hit_t < best.1 - f32::EPSILON || candidate.id < best.0)
        {
            best = (candidate.id, hit_t);
        }
    }
    Some(best.0)
}

fn shot_blocker_intersection(
    map: &Map,
    entity: &Entity,
    start: (f32, f32),
    end: (f32, f32),
) -> Option<f32> {
    if entity.kind == EntityKind::Tank {
        return unit_body_for_entity(entity)
            .and_then(|body| segment_intersects_unit_body(start, end, body));
    }
    if entity.is_building() {
        return building_rect_for_entity(map, entity)
            .and_then(|rect| segment_intersects_rect(start, end, rect));
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn emit_attack_event(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    attacker: u32,
    victim: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
) {
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

#[allow(clippy::too_many_arguments)]
fn push_under_attack_notices_for_visible_attack(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    victim_owner: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if !projection::attack_event_visible_to(pid, ax, ay, vx, vy, attacker_owner, fog) {
            continue;
        }
        push_under_attack_notice(events, pid, victim_owner, attacker_owner, vx, vy);
    }
}

fn push_under_attack_notice(
    events: &mut HashMap<u32, Vec<Event>>,
    recipient: u32,
    victim_owner: u32,
    attacker_owner: u32,
    x: f32,
    y: f32,
) {
    if victim_owner == 0 || victim_owner == attacker_owner || recipient != victim_owner {
        return;
    }
    events.entry(recipient).or_default().push(Event::Notice {
        msg: "alert:under_attack".to_string(),
        x: Some(x),
        y: Some(y),
        severity: NoticeSeverity::Alert,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{BuildPhase, EntityKind, EntityStore, Order, WeaponSetup};
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
            10,
        );
        events
    }

    fn run_movement_tick(entities: &mut EntityStore) {
        let map = Map::generate(2, 0x00C0_FFEE);
        let occ = Occupancy::build(&map, entities);
        let spatial = SpatialIndex::build(entities, map.size);
        movement_system(&map, entities, &mut [], &occ, &spatial, 0);
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
        let fog = Fog::new(map.size);
        let mut rng = SmallRng::seed_from_u64(0);
        apply_damage(
            &map,
            entities,
            events,
            &fog,
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
            10,
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
    fn visible_damage_emits_positioned_under_attack_alert_to_victim_owner() {
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let victim_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("victim should spawn");
        entities
            .get_mut(attacker_id)
            .expect("attacker should exist")
            .set_order(Order::attack(victim_id));

        let events = run_combat_tick(&mut entities);
        let victim_events = events
            .get(&2)
            .expect("victim owner should have an event queue");

        assert!(
            victim_events
                .iter()
                .any(|event| matches!(event, Event::Attack { from, to } if *from == attacker_id && *to == victim_id)),
            "victim owner should receive the visible attack event"
        );
        assert!(
            victim_events.iter().any(|event| matches!(
                event,
                Event::Notice {
                    msg,
                    x: Some(x),
                    y: Some(y),
                    severity: NoticeSeverity::Alert,
                } if msg == "alert:under_attack" && (*x - 120.0).abs() < 0.001 && (*y - 100.0).abs() < 0.001
            )),
            "victim owner should receive a positioned under-attack alert"
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
            10,
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
            .spawn_unit(2, EntityKind::Rifleman, 300.0, 100.0)
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
    fn shoot_while_moving_units_keep_existing_valid_target() {
        for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
            let mut entities = EntityStore::new();
            let attacker_id = entities
                .spawn_unit(1, kind, 100.0, 100.0)
                .expect("attacker should spawn");
            let retained_target_id = entities
                .spawn_unit(2, EntityKind::Worker, 150.0, 100.0)
                .expect("retained target should spawn");
            entities
                .spawn_unit(2, EntityKind::Worker, 120.0, 130.0)
                .expect("closer target should spawn");
            if let Some(attacker) = entities.get_mut(attacker_id) {
                attacker.set_order(Order::move_to(300.0, 100.0));
                attacker.set_target_id(Some(retained_target_id));
            }

            let map = open_map(8);
            let los = LineOfSight::new(&map);
            let spatial = SpatialIndex::build(&entities, map.size);
            let attacker = entities
                .get(attacker_id)
                .expect("attacker should still exist");

            let target = resolve_target(
                &entities,
                &spatial,
                &los,
                attacker_id,
                attacker.owner,
                attacker.pos_x,
                attacker.pos_y,
                192.0,
                combat_mode(attacker),
            );

            assert_eq!(
                target,
                Some(retained_target_id),
                "{kind} should stay focused"
            );
        }
    }

    #[test]
    fn shoot_while_moving_units_reacquire_when_existing_target_is_dead() {
        for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
            let mut entities = EntityStore::new();
            let attacker_id = entities
                .spawn_unit(1, kind, 100.0, 100.0)
                .expect("attacker should spawn");
            let dead_target_id = entities
                .spawn_unit(2, EntityKind::Worker, 150.0, 100.0)
                .expect("dead target should spawn");
            let new_target_id = entities
                .spawn_unit(2, EntityKind::Worker, 120.0, 130.0)
                .expect("new target should spawn");
            if let Some(dead_target) = entities.get_mut(dead_target_id) {
                dead_target.hp = 0;
            }
            if let Some(attacker) = entities.get_mut(attacker_id) {
                attacker.set_order(Order::move_to(300.0, 100.0));
                attacker.set_target_id(Some(dead_target_id));
            }

            let map = open_map(8);
            let los = LineOfSight::new(&map);
            let spatial = SpatialIndex::build(&entities, map.size);
            let attacker = entities
                .get(attacker_id)
                .expect("attacker should still exist");

            let target = resolve_target(
                &entities,
                &spatial,
                &los,
                attacker_id,
                attacker.owner,
                attacker.pos_x,
                attacker.pos_y,
                192.0,
                combat_mode(attacker),
            );

            assert_eq!(target, Some(new_target_id), "{kind} should reacquire");
        }
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
            .spawn_unit(2, EntityKind::Rifleman, 288.0, 100.0)
            .expect("enemy should spawn");
        if let Some(tank) = entities.get_mut(tank_id) {
            tank.set_order(Order::attack_move_to(500.0, 100.0));
            tank.set_path(vec![(96.0, 100.0)]);
            tank.set_path_goal(Some((96.0, 100.0)));
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
    fn direct_hits_record_damage_signal_on_victim() {
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
        let pos = worker
            .last_damage_pos()
            .expect("victim should record attacker position");
        assert!(
            pos.0 < worker.pos_x,
            "recorded attacker position should be on the attacker's side"
        );
        assert!(
            worker.last_damage_tick().is_some(),
            "victim should record damage tick so AI can react"
        );
    }

    #[test]
    fn combat_no_longer_issues_retreat_orders() {
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
            matches!(worker.order(), Order::Idle),
            "combat must not mutate orders; retreat is now an AI command"
        );
        assert_eq!(worker.path_goal(), None, "combat must not issue path goals");
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
    fn idle_at_team_does_not_auto_setup() {
        let mut entities = EntityStore::new();
        let at_id = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at team should spawn");

        run_combat_tick(&mut entities);

        assert_eq!(
            entities
                .get(at_id)
                .expect("at team should exist")
                .weapon_setup(),
            WeaponSetup::Packed
        );
    }

    #[test]
    fn packed_at_team_fires_with_shorter_range_and_reduced_damage() {
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at team should spawn");
        let tank_id = entities
            .spawn_unit(2, EntityKind::Tank, 220.0, 100.0)
            .expect("enemy tank should spawn");
        entities
            .get_mut(tank_id)
            .expect("tank should exist")
            .set_facing(std::f32::consts::PI);
        let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

        run_combat_tick(&mut entities);

        assert_eq!(
            entities.get(tank_id).expect("enemy should exist").hp,
            enemy_hp - 36,
            "packed AT gun should deal 75% of its deployed 48 damage"
        );
    }

    #[test]
    fn deployed_at_team_fires_at_long_range() {
        let mut entities = EntityStore::new();
        let at_id = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at team should spawn");
        let tank_id = entities
            .spawn_unit(2, EntityKind::Tank, 310.0, 100.0)
            .expect("enemy tank should spawn");
        if let Some(at) = entities.get_mut(at_id) {
            at.set_weapon_setup(WeaponSetup::Deployed);
            at.set_emplacement_facing(Some(0.0));
            at.set_facing(0.0);
            at.set_weapon_facing(0.0);
        }
        entities
            .get_mut(tank_id)
            .expect("tank should exist")
            .set_facing(std::f32::consts::PI);
        let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

        run_combat_tick(&mut entities);

        assert!(
            entities.get(tank_id).expect("enemy should exist").hp < enemy_hp,
            "deployed AT team should fire at range 7"
        );
    }

    #[test]
    fn at_team_turns_slowly_before_firing() {
        let mut entities = EntityStore::new();
        let at_id = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at team should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Tank, 100.0, 20.0)
            .expect("enemy tank should spawn");
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
        if let Some(at) = entities.get_mut(at_id) {
            at.set_facing(0.0);
            at.set_weapon_facing(0.0);
            at.set_weapon_setup(WeaponSetup::Deployed);
        }

        run_combat_tick(&mut entities);

        let at = entities.get(at_id).expect("at should exist");
        assert!(
            at.facing().abs() <= AT_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
            "AT gun should only slew by its turn-rate cap, got {:.4}",
            at.facing()
        );
        assert_eq!(
            entities.get(enemy_id).expect("enemy should exist").hp,
            enemy_hp,
            "AT gun should not fire until its barrel is aligned"
        );
    }

    #[test]
    fn deployed_at_team_clamps_to_field_edge_and_does_not_fire_outside_arc() {
        let mut entities = EntityStore::new();
        let at_id = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at team should spawn");
        let enemy_id = entities
            .spawn_unit(2, EntityKind::Tank, 100.0, 180.0)
            .expect("enemy tank should spawn");
        let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
        if let Some(at) = entities.get_mut(at_id) {
            at.set_weapon_setup(WeaponSetup::Deployed);
            at.set_emplacement_facing(Some(0.0));
            at.set_facing(0.0);
            at.set_weapon_facing(0.0);
        }

        for _ in 0..20 {
            run_combat_tick(&mut entities);
        }

        let at = entities.get(at_id).expect("at should exist");
        let edge = config::AT_GUN_FIELD_OF_FIRE_RAD * 0.5;
        assert!(
            (at.facing() - edge).abs() <= AT_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
            "AT gun should clamp to the nearest arc edge, got {:.4}",
            at.facing()
        );
        assert_eq!(
            entities.get(enemy_id).expect("enemy should exist").hp,
            enemy_hp,
            "AT gun should not fire outside its deployed field of fire"
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
    fn shots_overpenetrate_past_non_blocking_primary_target() {
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
    fn missed_primary_shot_still_emits_attack_event() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("attacker should spawn");
        let victim = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("victim should spawn");
        let victim_hp = entities.get(victim).expect("victim should exist").hp;
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
            100.0,
            100.0,
            140.0,
            100.0,
            128.0,
        );

        assert_eq!(
            entities.get(victim).expect("victim should exist").hp,
            victim_hp,
            "seeded AT shot should miss the infantry target"
        );
        assert!(
            events
                .get(&1)
                .expect("attacker owner events should exist")
                .iter()
                .any(|event| matches!(event, Event::Attack { from, to } if *from == attacker && *to == victim)),
            "missed shots should still emit attack feedback for gun audio"
        );
        assert!(
            events
                .get(&2)
                .expect("victim owner events should exist")
                .iter()
                .all(|event| !matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack")),
            "misses should not emit under-attack damage alerts"
        );
    }

    #[test]
    fn shots_do_not_continue_into_resource_nodes() {
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
    fn tank_behind_primary_target_blocks_overpenetration() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let primary = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("primary target should spawn");
        let blocker = entities
            .spawn_unit(2, EntityKind::Tank, 165.0, 100.0)
            .expect("blocking tank should spawn");
        let behind = entities
            .spawn_unit(2, EntityKind::Worker, 190.0, 100.0)
            .expect("unit behind blocker should spawn");
        let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
        let behind_hp_before = entities.get(behind).expect("behind should exist").hp;
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
            entities.get(blocker).expect("blocker should exist").hp < blocker_hp_before,
            "tank behind the primary target should take overpenetration damage"
        );
        assert_eq!(
            entities.get(behind).expect("behind should exist").hp,
            behind_hp_before,
            "overpenetration should stop at the tank"
        );
    }

    #[test]
    fn tank_between_attacker_and_target_blocks_the_shot() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let blocker = entities
            .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
            .expect("blocking tank should spawn");
        let intended = entities
            .spawn_unit(2, EntityKind::Worker, 190.0, 100.0)
            .expect("intended target should spawn");
        let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
        let intended_hp_before = entities.get(intended).expect("intended should exist").hp;
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        events.insert(1, Vec::new());
        events.insert(2, Vec::new());

        apply_test_damage(
            &mut entities,
            &mut events,
            attacker,
            intended,
            20,
            1,
            100.0,
            100.0,
            190.0,
            100.0,
            128.0,
        );

        assert_eq!(
            entities.get(intended).expect("intended should exist").hp,
            intended_hp_before,
            "target behind the blocking tank should not be damaged"
        );
        assert!(
            entities.get(blocker).expect("blocker should exist").hp < blocker_hp_before,
            "blocking tank should take the shot damage"
        );
        assert!(
            events
                .get(&1)
                .expect("attacker owner events should exist")
                .iter()
                .any(|event| matches!(event, Event::Attack { from, to } if *from == attacker && *to == blocker)),
            "attack event should point at the blocking tank"
        );
    }

    #[test]
    fn building_between_attacker_and_target_blocks_the_shot() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("attacker should spawn");
        let blocker = entities
            .spawn_building(2, EntityKind::Depot, 160.0, 100.0, true)
            .expect("blocking building should spawn");
        let intended = entities
            .spawn_unit(2, EntityKind::Worker, 230.0, 100.0)
            .expect("intended target should spawn");
        let blocker_hp_before = entities.get(blocker).expect("blocker should exist").hp;
        let intended_hp_before = entities.get(intended).expect("intended should exist").hp;
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        events.insert(1, Vec::new());
        events.insert(2, Vec::new());

        apply_test_damage(
            &mut entities,
            &mut events,
            attacker,
            intended,
            20,
            1,
            100.0,
            100.0,
            230.0,
            100.0,
            128.0,
        );

        assert_eq!(
            entities.get(intended).expect("intended should exist").hp,
            intended_hp_before,
            "target behind the blocking building should not be damaged"
        );
        assert!(
            entities.get(blocker).expect("blocker should exist").hp < blocker_hp_before,
            "blocking building should take the shot damage"
        );
    }
}
