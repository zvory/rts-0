//! Anchor displacement and scoped friendly contact constraints.
use super::{standability, Occupancy};
use crate::config;
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;
use rts_rules::math::GameplayMath as _;
use std::collections::BTreeSet;

pub(super) fn contact_ids(
    entities: &EntityStore,
    teams: &crate::game::teams::TeamRelations,
    runtime: &AbilityRuntime,
    tick: u32,
) -> Vec<u32> {
    entities
        .iter()
        .filter(|e| {
            e.is_unit()
                && e.kind != EntityKind::ScoutPlane
                && runtime
                    .magic_anchor_pull(e.owner, teams, e.pos_x(), e.pos_y(), tick)
                    .is_some()
        })
        .map(|e| e.id)
        .collect()
}

pub(super) fn displacement(
    e: &crate::game::entity::Entity,
    teams: &crate::game::teams::TeamRelations,
    runtime: &AbilityRuntime,
    tick: u32,
) -> Option<(f32, f32)> {
    let (dir, strength, distance) =
        runtime.magic_anchor_pull(e.owner, teams, e.pos_x(), e.pos_y(), tick)?;
    let resistance = standability::footing_resistance(standability::footing_profile(e)).max(1.0);
    let magnitude =
        (e.movement_speed() * config::EKAT_MAGIC_ANCHOR_PULL_SPEED_MULTIPLIER * strength
            / resistance.det_sqrt())
        .min(distance);
    (magnitude.is_finite()).then_some((dir.0 * magnitude, dir.1 * magnitude))
}

/// Admit combined walking/recovery and pull against the original pose.
#[allow(clippy::too_many_arguments)]
pub(super) fn admit_motion(
    map: &Map,
    occ: &Occupancy,
    blockers: &standability::UnitBlockers,
    entity: &crate::game::entity::Entity,
    teams: &crate::game::teams::TeamRelations,
    desired: (f32, f32),
    facing: f32,
) -> Option<(f32, f32)> {
    let from = (entity.pos_x(), entity.pos_y());
    [desired, (desired.0, from.1), (from.0, desired.1)]
        .into_iter()
        .find(|&to| {
            // Forced sideways motion does not turn a vehicle's hull toward
            // its displacement. Route shortcut checks assume that turn.
            let static_clear = if crate::game::entity::uses_oriented_vehicle_body(entity.kind) {
                super::vehicle_route::static_swept_segment_legal(
                    map,
                    occ,
                    entity.kind,
                    from,
                    to,
                    facing,
                )
            } else {
                crate::game::services::standability::unit_static_segment_standable_with_radius(
                    map,
                    occ,
                    entity.kind,
                    from,
                    to,
                    Some(entity.radius()),
                )
            };
            static_clear && blockers.motion_clear(entity, to, facing, teams)
        })
}

pub(super) fn apply_pull(
    map: &Map,
    entities: &mut EntityStore,
    teams: &crate::game::teams::TeamRelations,
    occ: &Occupancy,
    tick: u32,
    ability_runtime: &AbilityRuntime,
    integrated: &BTreeSet<u32>,
) {
    let mut blockers = crate::game::services::standability::UnitBlockers::with_contact_locks(
        entities,
        contact_ids(entities, teams, ability_runtime, tick),
    );
    let world_max_x = map.world_width_px() - 0.01;
    let world_max_y = map.world_height_px() - 0.01;
    for id in entities.ids() {
        if integrated.contains(&id) {
            continue;
        }
        let Some((kind, x, y, facing, delta)) = entities.get(id).and_then(|e| {
            if !e.is_unit() || e.kind == EntityKind::ScoutPlane {
                return None;
            }
            Some((
                e.kind,
                e.pos_x(),
                e.pos_y(),
                e.facing(),
                displacement(e, teams, ability_runtime, tick)?,
            ))
        }) else {
            continue;
        };
        let next_x = (x + delta.0).clamp(0.0, world_max_x);
        let next_y = (y + delta.1).clamp(0.0, world_max_y);
        if !crate::game::services::standability::unit_static_standable_with_radius(
            map,
            occ,
            kind,
            next_x,
            next_y,
            facing,
            entities.get(id).map(|e| e.radius()),
        ) {
            continue;
        }
        if entities
            .get(id)
            .is_some_and(|e| !blockers.motion_clear(e, (next_x, next_y), facing, teams))
        {
            continue;
        }
        if let Some(e) = entities.get_mut(id) {
            e.set_position(next_x, next_y);
            blockers.update(e);
            let (next_x, next_y) = (e.pos_x(), e.pos_y());
            let previous_delta = e.movement_delta();
            e.set_movement_delta(previous_delta.0 + next_x - x, previous_delta.1 + next_y - y);
            if kind == EntityKind::Tank && (next_x != x || next_y != y) {
                if let Some(combat) = e.combat.as_mut() {
                    combat.tank_stationary_range_ticks = 0;
                    combat.tank_stationary_range_reset_this_tick = true;
                }
            }
        }
    }
}
