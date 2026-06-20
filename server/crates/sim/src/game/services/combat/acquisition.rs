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
use crate::rules::terrain::TerrainKind;

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
    // Ordered attackers keep their explicit target if it still exists.
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

    let attacker_kind = entities.get(self_id).map(|e| e.kind);
    if attacker_kind == Some(EntityKind::Tank) {
        if let Some(id) = preferred_target_for_tank(
            map, entities, teams, spatial, los, fog, smokes, self_id, owner, px, py,
        ) {
            return Some(id);
        }
    }

    if let Some(target) = retained_firing_target_for_shoot_while_moving_unit(
        map, entities, teams, los, fog, smokes, self_id, owner, px, py, acquire_px,
    ) {
        return Some(target);
    }

    // Anti-Tank Guns prefer tanks over all other targets; fall back to nearest enemy if no tank
    // is in range.
    let prefers_armored = attacker_kind
        .map(combat_rules::prefers_armored_targets)
        .unwrap_or(false);
    if prefers_armored {
        if let Some(id) = world_query::nearest_tank_in_range_filtered(
            entities,
            teams,
            spatial,
            self_id,
            owner,
            px,
            py,
            acquire_px,
            |target| {
                target_currently_fireable(
                    map, entities, los, fog, smokes, self_id, owner, px, py, target,
                )
            },
        ) {
            return Some(id);
        }
    }

    // Units prefer engaging enemy units over buildings; fall back to any enemy if no unit in range.
    let attacker_is_unit = entities.get(self_id).map(|e| e.is_unit()).unwrap_or(false);
    if attacker_is_unit {
        if let Some(id) = world_query::nearest_enemy_unit_in_range_filtered(
            entities,
            teams,
            spatial,
            self_id,
            owner,
            px,
            py,
            acquire_px,
            |target| {
                target_currently_fireable(
                    map, entities, los, fog, smokes, self_id, owner, px, py, target,
                )
            },
        ) {
            return Some(id);
        }
    }

    // Aggressive acquisition: the nearest enemy within the acquire radius (weapon range for
    // buildings, sight range for mobile units so they chase).
    world_query::nearest_enemy_in_range_filtered(
        entities,
        teams,
        spatial,
        self_id,
        owner,
        px,
        py,
        acquire_px,
        |target| {
            attacker_kind.is_some_and(|kind| target_relevant_for_auto_acquisition(kind, target))
                &&
            target_currently_fireable(
                map, entities, los, fog, smokes, self_id, owner, px, py, target,
            )
        },
    )
}

fn target_relevant_for_auto_acquisition(attacker: EntityKind, target: &Entity) -> bool {
    !(movement_body_class(attacker) == MovementBodyClass::InfantryLike
        && target.kind == EntityKind::TankTrap)
}

#[allow(clippy::too_many_arguments)]
fn preferred_target_for_tank(
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
) -> Option<u32> {
    let attacker = entities.get(self_id)?;
    let weapon_range_px = weapon_range_px(attacker);
    for kind in combat_rules::TANK_TARGET_PRIORITY_ORDER {
        if let Some(id) = world_query::nearest_enemy_kind_in_range_filtered(
            entities,
            teams,
            spatial,
            self_id,
            owner,
            px,
            py,
            weapon_range_px,
            kind,
            |target| {
                target_currently_fireable(
                    map, entities, los, fog, smokes, self_id, owner, px, py, target,
                )
            },
        ) {
            return Some(id);
        }
    }
    None
}

fn weapon_range_px(attacker: &Entity) -> f32 {
    let profile = effective_attack_profile(attacker);
    profile.range_tiles as f32 * config::TILE_SIZE as f32 + attacker.radius() + super::RANGE_SLACK
}

#[allow(clippy::too_many_arguments)]
fn retained_firing_target_for_shoot_while_moving_unit(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
) -> Option<u32> {
    let attacker = entities.get(self_id)?;
    if !can_fire_while_moving(attacker) {
        return None;
    }
    let target_id = attacker.target_id()?;
    let target = entities.get(target_id)?;
    if !world_query::is_enemy_targetable(target, teams, owner, self_id) {
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
    if !target_currently_fireable(
        map, entities, los, fog, smokes, self_id, owner, px, py, target,
    ) {
        return None;
    }
    Some(target_id)
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
