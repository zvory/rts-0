use crate::config;
use crate::game::entity::{blocks_line_of_sight, Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::geometry::{
    building_rect_for_entity, segment_intersects_rect, segment_intersects_unit_body,
    unit_body_for_entity,
};
use crate::game::teams::TeamRelations;

use super::shot_blocker_index::{ShotBlockerBounds, ShotBlockerIndex};

const TANK_STATIONARY_RANGE_MAX_TILES: f32 = 14.0;
const TANK_STATIONARY_RANGE_RAMP_TICKS: u16 = config::TICK_HZ as u16 * 3;

impl ShotBlockerIndex {
    pub(super) fn build(map: &Map, entities: &EntityStore) -> Self {
        Self::from_entries(entities.iter().filter_map(|entity| {
            shot_blocker_bounds(map, entity).map(|bounds| (entity.id, entity.owner, bounds))
        }))
    }
}

fn shot_blocker_bounds(map: &Map, entity: &Entity) -> Option<ShotBlockerBounds> {
    if entity.kind == EntityKind::Tank {
        let radius = unit_body_for_entity(entity)?.bounding_radius();
        return Some(ShotBlockerBounds {
            min_x: entity.pos_x - radius,
            min_y: entity.pos_y - radius,
            max_x: entity.pos_x + radius,
            max_y: entity.pos_y + radius,
        });
    }
    if entity.is_building() && blocks_line_of_sight(entity.kind) {
        let rect = building_rect_for_entity(map, entity)?;
        return Some(ShotBlockerBounds {
            min_x: rect.min_x,
            min_y: rect.min_y,
            max_x: rect.max_x,
            max_y: rect.max_y,
        });
    }
    None
}

pub(super) fn tank_effective_range_tiles(e: &Entity, base_range_tiles: f32) -> f32 {
    if e.kind != EntityKind::Tank {
        return base_range_tiles;
    }
    let ramp_ticks = TANK_STATIONARY_RANGE_RAMP_TICKS.max(1);
    let ticks = e
        .combat
        .as_ref()
        .map_or(0, |c| c.tank_stationary_range_ticks)
        .min(ramp_ticks);
    let progress = ticks as f32 / ramp_ticks as f32;
    base_range_tiles + (TANK_STATIONARY_RANGE_MAX_TILES - base_range_tiles) * progress
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_shot_victim(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
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
    for entry in blockers.all() {
        if !entry.bounds.overlaps_segment_bounds((ax, ay), end) {
            continue;
        }
        let Some(candidate) = entities.get(entry.id) else {
            continue;
        };
        if candidate.id == attacker
            || candidate.is_node()
            || !teams.is_enemy_owner(attacker_owner, candidate.owner)
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

pub(super) fn friendly_hard_blocker_between(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    attacker: u32,
    attacker_owner: u32,
    start: (f32, f32),
    end: (f32, f32),
) -> bool {
    friendly_hard_blocker_between_except(
        map,
        entities,
        blockers,
        attacker,
        attacker_owner,
        start,
        end,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn friendly_hard_blocker_between_except(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    attacker: u32,
    attacker_owner: u32,
    start: (f32, f32),
    end: (f32, f32),
    ignored_target: Option<u32>,
) -> bool {
    if !start.0.is_finite() || !start.1.is_finite() || !end.0.is_finite() || !end.1.is_finite() {
        return true;
    }
    blockers
        .owned_by(attacker_owner)
        .iter()
        .filter(|entry| entry.bounds.overlaps_segment_bounds(start, end))
        .filter_map(|entry| entities.get(entry.id))
        .any(|candidate| {
            candidate.id != attacker
                && Some(candidate.id) != ignored_target
                && candidate.hp > 0
                && shot_blocker_intersection(map, candidate, start, end).is_some()
        })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn shot_hits_intended_target(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    attacker: u32,
    attacker_owner: u32,
    intended_victim: u32,
    start: (f32, f32),
) -> bool {
    let Some(target) = entities.get(intended_victim) else {
        return false;
    };
    let end = (target.pos_x, target.pos_y);
    if friendly_hard_blocker_between_except(
        map,
        entities,
        blockers,
        attacker,
        attacker_owner,
        start,
        end,
        Some(intended_victim),
    ) {
        return false;
    }
    resolve_shot_victim(
        map,
        entities,
        blockers,
        teams,
        attacker,
        intended_victim,
        attacker_owner,
        start.0,
        start.1,
    )
    .is_some_and(|victim| victim == intended_victim)
}

pub(super) fn shot_blocker_intersection(
    map: &Map,
    entity: &Entity,
    start: (f32, f32),
    end: (f32, f32),
) -> Option<f32> {
    if entity.kind == EntityKind::Tank {
        return unit_body_for_entity(entity)
            .and_then(|body| segment_intersects_unit_body(start, end, body));
    }
    if entity.is_building() && blocks_line_of_sight(entity.kind) {
        return building_rect_for_entity(map, entity)
            .and_then(|rect| segment_intersects_rect(start, end, rect));
    }
    None
}
