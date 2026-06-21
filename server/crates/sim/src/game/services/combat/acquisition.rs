use crate::config;
use crate::game::entity::{
    movement_body_class, Entity, EntityKind, EntityStore, MovementBodyClass, Order,
};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::rules::combat as combat_rules;
use crate::rules::terrain::{self, TerrainKind};

use super::priority::{self, AttackPriorityContext, TargetCandidate};
use super::projection::friendly_hard_blocker_between;
use super::weapons::{can_fire_while_moving, effective_attack_profile};

/// How a combatant chooses targets.
#[derive(Copy, Clone, PartialEq)]
pub(super) enum CombatMode {
    /// Has an explicit attack target id.
    Ordered,
    /// Engages and chases any enemy within acquisition range.
    Aggressive,
    /// Engages enemies already in weapon range, without chasing them.
    Opportunistic,
    /// Ignores nearby enemies unless explicitly ordered to attack.
    Passive,
}

pub(super) fn combat_mode(e: &Entity) -> CombatMode {
    match e.order() {
        Order::Attack(_) => CombatMode::Ordered,
        Order::HoldPosition => CombatMode::Opportunistic,
        Order::AttackMove(_) => CombatMode::Aggressive,
        Order::Move(_) if can_fire_while_moving(e) => CombatMode::Opportunistic,
        Order::Idle if e.is_building() => CombatMode::Aggressive,
        Order::Idle if e.is_unit() && e.kind != EntityKind::Worker => CombatMode::Aggressive,
        _ => CombatMode::Passive,
    }
}

/// Resolve which entity an attacker should engage this tick.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_target(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    tank_trap_obstructs_vehicle_route: &dyn Fn(&Entity, &Entity) -> bool,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
) -> Option<u32> {
    if smokes.point_inside(px, py) {
        return None;
    }
    // Ordered attackers keep command intent outside the ranker. If the target is
    // still hostile and visible, the combat system may chase for a fireable shot
    // instead of letting auto-acquisition steal focus.
    if mode == CombatMode::Ordered {
        if let Some(e) = entities.get(self_id) {
            if let Some(target) = e.order().attack_target() {
                if entities
                    .get(target)
                    .map(|t| {
                        world_query::is_enemy_targetable(t, teams, owner, self_id)
                            && target_visible_to_owner(fog, smokes, owner, t)
                    })
                    .unwrap_or(false)
                {
                    return Some(target);
                }
            }
        }
        // Explicit target gone → fall through to acquisition so we don't stand idle.
    }

    if matches!(mode, CombatMode::Passive) {
        return None;
    }

    let attacker = entities.get(self_id)?;
    let context = AttackPriorityContext {
        attacker_kind: attacker.kind,
        attacker_is_unit: attacker.is_unit(),
        attacker_is_vehicle_body: movement_body_class(attacker.kind)
            == MovementBodyClass::VehicleBody,
        attacker_weapon_class: combat_rules::weapon_class(attacker.kind),
        can_retain_moving_target: can_fire_while_moving(attacker),
    };
    let weapon_range_px = weapon_range_px(attacker);
    let candidates = legal_target_candidates(
        map,
        entities,
        teams,
        spatial,
        los,
        fog,
        smokes,
        self_id,
        owner,
        px,
        py,
        acquire_px,
        weapon_range_px,
        &context,
        tank_trap_obstructs_vehicle_route,
        attacker.target_id(),
    );
    priority::choose_target(&context, &candidates)
}

fn target_relevant_for_auto_acquisition(attacker: EntityKind, target: &Entity) -> bool {
    !(movement_body_class(attacker) == MovementBodyClass::InfantryLike
        && target.kind == EntityKind::TankTrap)
}

fn weapon_range_px(attacker: &Entity) -> f32 {
    let profile = effective_attack_profile(attacker);
    profile.range_tiles as f32 * config::TILE_SIZE as f32 + attacker.radius() + super::RANGE_SLACK
}

#[allow(clippy::too_many_arguments)]
fn legal_target_candidates(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    weapon_range_px: f32,
    context: &AttackPriorityContext,
    tank_trap_obstructs_vehicle_route: &dyn Fn(&Entity, &Entity) -> bool,
    retained_target_id: Option<u32>,
) -> Vec<TargetCandidate> {
    let mut candidates = Vec::new();
    let attacker = entities.get(self_id);
    for id in spatial.ids_in_circle_bbox(px, py, acquire_px) {
        let Some(target) = entities.get(id) else {
            continue;
        };
        // Retained target status is only a ranker fact. It must still pass the
        // same hostile, visible, smoke, LOS, and blocker checks as any other
        // auto-acquired candidate.
        let retained_target = retained_target_id == Some(id);
        let retained_moving_fire_target = context.can_retain_moving_target && retained_target;
        if !world_query::is_enemy_targetable(target, teams, owner, self_id) {
            continue;
        }
        if !retained_moving_fire_target
            && !target_relevant_for_auto_acquisition(context.attacker_kind, target)
        {
            continue;
        }
        let concealment = terrain::concealment_modifier(target.kind, TerrainKind::Open).max(0.0);
        let effective_acquire_px = acquire_px * concealment;
        if !effective_acquire_px.is_finite() {
            continue;
        }
        let dx = target.pos_x - px;
        let dy = target.pos_y - py;
        let distance_sq = dx * dx + dy * dy;
        if distance_sq > effective_acquire_px * effective_acquire_px {
            continue;
        }
        let effective_weapon_range_px = weapon_range_px * concealment;
        let in_weapon_range = effective_weapon_range_px.is_finite()
            && distance_sq <= effective_weapon_range_px * effective_weapon_range_px;
        if !target_currently_fireable(
            map, entities, los, fog, smokes, self_id, owner, px, py, target,
        ) {
            continue;
        }
        let tank_trap_obstructs_vehicle_route =
            if target.kind == EntityKind::TankTrap && context.attacker_is_vehicle_body {
                attacker
                    .map(|attacker| tank_trap_obstructs_vehicle_route(attacker, target))
                    .unwrap_or(false)
            } else {
                false
            };
        candidates.push(TargetCandidate {
            id,
            kind: target.kind,
            owner: target.owner,
            pos_x: target.pos_x,
            pos_y: target.pos_y,
            distance_sq,
            is_unit: target.is_unit(),
            is_building: target.is_building(),
            armor_class: combat_rules::armor_class(target.kind),
            weapon_class: combat_rules::weapon_class(target.kind),
            threat_role: combat_rules::target_threat_role(target.kind),
            in_weapon_range,
            tank_trap_obstructs_vehicle_route,
            retained_target,
        });
    }
    candidates
}

#[allow(clippy::too_many_arguments)]
fn target_currently_fireable(
    map: &Map,
    entities: &EntityStore,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    target: &Entity,
) -> bool {
    !smokes.point_inside(px, py)
        && !smokes.point_inside(target.pos_x, target.pos_y)
        && target_visible_to_owner(fog, smokes, owner, target)
        && (attacker_uses_indirect_fire(entities, self_id)
            || (los.clear_between_world_points((px, py), (target.pos_x, target.pos_y))
                && !friendly_hard_blocker_between(
                    map,
                    entities,
                    self_id,
                    owner,
                    (px, py),
                    (target.pos_x, target.pos_y),
                )))
}

fn attacker_uses_indirect_fire(entities: &EntityStore, self_id: u32) -> bool {
    matches!(
        entities.get(self_id).map(|e| e.kind),
        Some(EntityKind::MortarTeam)
    )
}

fn target_visible_to_owner(
    fog: &Fog,
    smokes: &SmokeCloudStore,
    owner: u32,
    target: &Entity,
) -> bool {
    fog.is_visible_world(owner, target.pos_x, target.pos_y)
        && !smokes.point_inside(target.pos_x, target.pos_y)
}
