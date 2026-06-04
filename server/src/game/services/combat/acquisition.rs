use crate::game::entity::{fires_while_moving, Entity, EntityKind, EntityStore, Order};
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::rules::combat as combat_rules;
use crate::rules::terrain::TerrainKind;

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
        Order::AttackMove(_) => CombatMode::Aggressive,
        Order::Move(_) if fires_while_moving(e.kind) => CombatMode::Opportunistic,
        Order::Idle if e.is_building() => CombatMode::Aggressive,
        Order::Idle if e.is_unit() && e.kind != EntityKind::Worker => CombatMode::Aggressive,
        _ => CombatMode::Passive,
    }
}

/// Resolve which entity an attacker should engage this tick.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_target(
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

    // Units prefer engaging enemy units over buildings; fall back to any enemy if no unit in range.
    let attacker_is_unit = entities.get(self_id).map(|e| e.is_unit()).unwrap_or(false);
    if attacker_is_unit {
        if let Some(id) = world_query::nearest_enemy_unit_in_range_filtered(
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
