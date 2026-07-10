//! Entrenchment trench creation, occupation, and slotting.
//!
//! This service owns unit-facing trench lifecycle state. The neutral trench terrain itself lives
//! in [`crate::game::trench::TrenchStore`].

use std::collections::BTreeMap;

use crate::config;
use crate::game::entity::{
    active_trench_occupation, AttackPhase, Entity, EntityStore, MovePhase, Order,
};
use crate::game::map::Map;
use crate::game::services::geometry::{
    building_rect_for_entity, unit_bodies_intersect, unit_body_for_entity,
    unit_body_intersects_rect, unit_body_with_facing, UnitBody,
};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::standability;
use crate::game::trench::{Trench, TrenchStore};

mod spatial;

use spatial::{EntrenchmentEntityIndex, EntrenchmentIndexes, TrenchSpatialIndex};

const STATIONARY_EPS_PX: f32 = 0.05;
const SLOT_EXTRA_RADIUS_PX: f32 = config::TILE_SIZE as f32;
const SLOT_MAX_CORRECTION_PX: f32 = config::TILE_SIZE as f32;

type OccupiedTrenchCounts = BTreeMap<u32, usize>;

#[derive(Clone, Copy)]
struct OccupationCandidate {
    trench_id: u32,
    slot: Option<(f32, f32)>,
}

#[derive(Clone, Copy)]
struct RankedOccupationCandidate {
    dist_sq: f32,
    candidate: OccupationCandidate,
}

pub(crate) fn entrenchment_system(
    map: &Map,
    entities: &mut EntityStore,
    has_entrenchment: &dyn Fn(u32) -> bool,
    pre_collision_position: &dyn Fn(u32) -> Option<(f32, f32)>,
    occ: &Occupancy<'_>,
    trenches: &mut TrenchStore,
) {
    // Keep single-occupant trench checks cheap while this system mutates occupation state.
    let mut occupied_trench_counts = build_occupied_trench_counts(entities);
    // Entrenchment runs after collision resolution, so these indexes start from the current
    // positions rather than a phase-earlier derived spatial index. Keep them in sync below as
    // units slot and new trenches are dug; subsequent ids must observe those mutations.
    let mut indexes = EntrenchmentIndexes::build(map, entities, trenches);
    for id in entities.ids() {
        let Some(snapshot) = entities.get(id).cloned() else {
            continue;
        };

        if let Some(candidate) = occupation_candidate(
            map,
            entities,
            pre_collision_position,
            occ,
            &indexes,
            &occupied_trench_counts,
            &snapshot,
        ) {
            if let Some(e) = entities.get_mut(id) {
                if let Some((x, y)) = candidate.slot {
                    let old_position = (e.pos_x, e.pos_y);
                    e.set_position(x, y);
                    indexes.entities.relocate(id, old_position, (x, y));
                }
                set_active_trench_occupation_tracked(
                    e,
                    &mut occupied_trench_counts,
                    Some(candidate.trench_id),
                );
                reset_entrenchment_dig(e);
            }
            continue;
        }

        let can_create = can_create_trench(has_entrenchment, &snapshot);
        let should_dig = can_create
            && stationary_for_digging(pre_collision_position, &snapshot)
            && !standing_in_trench(&indexes.trenches, snapshot.pos_x, snapshot.pos_y);
        if should_dig {
            let completed = entities.get_mut(id).is_some_and(|e| {
                set_active_trench_occupation_tracked(e, &mut occupied_trench_counts, None);
                increment_entrenchment_dig(e) >= config::ENTRENCHMENT_DIG_IN_TICKS
            });
            if completed {
                let created = trenches.create(map, snapshot.pos_x, snapshot.pos_y);
                if let (Some(created), Some(trench)) = (created, trenches.all().last().copied()) {
                    if trench.id == created {
                        indexes.trenches.insert(trench);
                    }
                }
                if let Some(e) = entities.get_mut(id) {
                    reset_entrenchment_dig(e);
                    set_active_trench_occupation_tracked(e, &mut occupied_trench_counts, created);
                }
            }
        } else if let Some(e) = entities.get_mut(id) {
            set_active_trench_occupation_tracked(e, &mut occupied_trench_counts, None);
            reset_entrenchment_dig(e);
        }
    }
}

fn occupation_candidate(
    map: &Map,
    entities: &EntityStore,
    pre_collision_position: &dyn Fn(u32) -> Option<(f32, f32)>,
    occ: &Occupancy<'_>,
    indexes: &EntrenchmentIndexes,
    occupied_trench_counts: &OccupiedTrenchCounts,
    entity: &Entity,
) -> Option<OccupationCandidate> {
    if !eligible_living_infantry(entity) || !stopped_for_occupation(pre_collision_position, entity)
    {
        return None;
    }
    best_occupation_candidate(
        map,
        entities,
        occ,
        indexes,
        occupied_trench_counts,
        entity,
    )
}

fn can_create_trench(has_entrenchment: &dyn Fn(u32) -> bool, entity: &Entity) -> bool {
    eligible_living_infantry(entity) && has_entrenchment(entity.owner)
}

fn eligible_living_infantry(entity: &Entity) -> bool {
    entity.hp > 0 && entity.is_unit() && config::is_entrenchment_eligible_infantry(entity.kind)
}

fn stopped_for_occupation(
    pre_collision_position: &dyn Fn(u32) -> Option<(f32, f32)>,
    entity: &Entity,
) -> bool {
    holds_ground(entity)
        && movement_delta_distance(entity) <= STATIONARY_EPS_PX
        && forced_movement_delta_distance(pre_collision_position, entity) <= STATIONARY_EPS_PX
}

fn stationary_for_digging(
    pre_collision_position: &dyn Fn(u32) -> Option<(f32, f32)>,
    entity: &Entity,
) -> bool {
    stopped_for_occupation(pre_collision_position, entity)
}

fn holds_ground(entity: &Entity) -> bool {
    if !entity.path_is_empty() {
        return false;
    }
    match entity.order() {
        Order::Idle | Order::HoldPosition => true,
        Order::Attack(order) => order.execution.phase == AttackPhase::Firing,
        Order::AttackMove(_) => entity.move_phase() == Some(MovePhase::Arrived),
        Order::Move(_)
        | Order::Gather(_)
        | Order::Build(_)
        | Order::Deconstruct(_)
        | Order::Ability(_)
        | Order::ArtilleryPointFire(_) | Order::ArtilleryBlanketFire(_) => false,
    }
}

fn movement_delta_distance(entity: &Entity) -> f32 {
    let (dx, dy) = entity.movement_delta();
    (dx * dx + dy * dy).sqrt()
}

fn forced_movement_delta_distance(
    pre_collision_position: &dyn Fn(u32) -> Option<(f32, f32)>,
    entity: &Entity,
) -> f32 {
    let Some((before_x, before_y)) = pre_collision_position(entity.id) else {
        return 0.0;
    };
    distance((before_x, before_y), (entity.pos_x, entity.pos_y))
}

fn best_occupation_candidate(
    map: &Map,
    entities: &EntityStore,
    occ: &Occupancy<'_>,
    indexes: &EntrenchmentIndexes,
    occupied_trench_counts: &OccupiedTrenchCounts,
    entity: &Entity,
) -> Option<OccupationCandidate> {
    let mut best: Option<RankedOccupationCandidate> = None;
    for trench in indexes.trenches.occupation_candidates(entity) {
        if trench_occupied_by_other(occupied_trench_counts, entity, trench.id) {
            continue;
        }
        let Some(dist_sq) = occupation_search_distance_sq(trench, entity) else {
            continue;
        };
        let Some(slot) = slot_candidate(map, entities, occ, &indexes.entities, entity, trench)
        else {
            continue;
        };
        if best
            .map(|best| {
                dist_sq > best.dist_sq
                    || ((dist_sq - best.dist_sq).abs() <= f32::EPSILON
                        && trench.id > best.candidate.trench_id)
            })
            .unwrap_or(false)
        {
            continue;
        }
        best = Some(RankedOccupationCandidate {
            dist_sq,
            candidate: OccupationCandidate {
                trench_id: trench.id,
                slot,
            },
        });
    }
    best.map(|best| best.candidate)
}

fn occupation_search_distance_sq(trench: Trench, entity: &Entity) -> Option<f32> {
    let dx = entity.pos_x - trench.x;
    let dy = entity.pos_y - trench.y;
    let dist_sq = dx * dx + dy * dy;
    if !dist_sq.is_finite() {
        return None;
    }
    let radius = trench_radius_px(trench) + SLOT_EXTRA_RADIUS_PX;
    (dist_sq <= radius * radius).then_some(dist_sq)
}

fn standing_in_trench(trenches: &TrenchSpatialIndex, x: f32, y: f32) -> bool {
    trenches
        .containing_candidates(x, y)
        .any(|trench| trench_contains_point(trench, x, y))
}

fn slot_candidate(
    map: &Map,
    entities: &EntityStore,
    occ: &Occupancy<'_>,
    entity_index: &EntrenchmentEntityIndex,
    entity: &Entity,
    trench: Trench,
) -> Option<Option<(f32, f32)>> {
    if trench_contains_point(trench, entity.pos_x, entity.pos_y)
        && slot_position_legal(
            map,
            entities,
            occ,
            entity_index,
            entity,
            trench,
            (entity.pos_x, entity.pos_y),
        )
    {
        return Some(None);
    }

    slot_positions(entity, trench)
        .into_iter()
        .filter(|candidate| {
            distance((entity.pos_x, entity.pos_y), *candidate) <= SLOT_MAX_CORRECTION_PX
        })
        .filter(|candidate| {
            slot_position_legal(map, entities, occ, entity_index, entity, trench, *candidate)
        })
        .min_by(|a, b| {
            distance_sq((entity.pos_x, entity.pos_y), *a)
                .total_cmp(&distance_sq((entity.pos_x, entity.pos_y), *b))
                .then_with(|| a.0.total_cmp(&b.0))
                .then_with(|| a.1.total_cmp(&b.1))
        })
        .map(Some)
}

fn slot_positions(entity: &Entity, trench: Trench) -> Vec<(f32, f32)> {
    let mut candidates = Vec::new();
    let from_center = (entity.pos_x - trench.x, entity.pos_y - trench.y);
    let dist = (from_center.0 * from_center.0 + from_center.1 * from_center.1).sqrt();
    if dist.is_finite() && dist > 0.001 {
        let target_dist = trench_radius_px(trench) * 0.55;
        let step = (dist - target_dist).clamp(0.0, SLOT_MAX_CORRECTION_PX);
        candidates.push((
            entity.pos_x - from_center.0 / dist * step,
            entity.pos_y - from_center.1 / dist * step,
        ));
    }

    candidates.push((trench.x, trench.y));
    let base_angle = if dist.is_finite() && dist > 0.001 {
        from_center.1.atan2(from_center.0)
    } else {
        (entity.id as f32 * 0.618_034).rem_euclid(std::f32::consts::TAU)
    };
    for radius in [
        trench_radius_px(trench) * 0.45,
        trench_radius_px(trench) * 0.75,
        trench_radius_px(trench) * 0.85,
    ] {
        for offset in [
            0.0,
            std::f32::consts::FRAC_PI_4,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_2,
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
        ] {
            let angle = base_angle + offset;
            candidates.push((
                trench.x + angle.cos() * radius,
                trench.y + angle.sin() * radius,
            ));
        }
    }
    candidates
}

fn slot_position_legal(
    map: &Map,
    entities: &EntityStore,
    occ: &Occupancy<'_>,
    entity_index: &EntrenchmentEntityIndex,
    entity: &Entity,
    trench: Trench,
    candidate: (f32, f32),
) -> bool {
    if !trench_contains_point(trench, candidate.0, candidate.1) {
        return false;
    }
    if !standability::unit_static_standable_with_facing(
        map,
        occ,
        entity.kind,
        candidate.0,
        candidate.1,
        entity.facing(),
    ) {
        return false;
    }
    if distance_sq((entity.pos_x, entity.pos_y), candidate) > STATIONARY_EPS_PX * STATIONARY_EPS_PX
        && !standability::unit_static_segment_standable(
            map,
            occ,
            entity.kind,
            (entity.pos_x, entity.pos_y),
            candidate,
        )
    {
        return false;
    }
    if slot_intersects_building(map, entities, entity_index, entity, candidate) {
        return false;
    }
    !slot_overlaps_other_unit(entities, entity_index, entity, candidate)
}

fn slot_intersects_building(
    map: &Map,
    entities: &EntityStore,
    entity_index: &EntrenchmentEntityIndex,
    entity: &Entity,
    candidate: (f32, f32),
) -> bool {
    let Some(candidate_body) =
        unit_body_with_facing(entity.kind, candidate.0, candidate.1, entity.facing())
    else {
        return true;
    };
    entity_index.ids_near(candidate.0, candidate.1, entity_index.building_query_radius(candidate_body))
        .any(|other_id| {
            entities.get(other_id).is_some_and(|other| {
                other.hp > 0
                    && other.is_building()
                    && building_rect_for_entity(map, other)
                        .is_some_and(|rect| unit_body_intersects_rect(candidate_body, rect))
            })
        })
}

fn slot_overlaps_other_unit(
    entities: &EntityStore,
    entity_index: &EntrenchmentEntityIndex,
    entity: &Entity,
    candidate: (f32, f32),
) -> bool {
    let Some(candidate_body) =
        unit_body_with_facing(entity.kind, candidate.0, candidate.1, entity.facing())
    else {
        return true;
    };
    entity_index
        .ids_near(candidate.0, candidate.1, entity_index.unit_query_radius(candidate_body))
        .any(|other_id| {
            other_id != entity.id
                && entities.get(other_id).is_some_and(|other| {
                    other.hp > 0
                        && other.is_unit()
                        && unit_body_for_entity(other).is_some_and(|other_body| {
                            unit_bodies_intersect(candidate_body, other_body)
                        })
                })
        })
}

fn reset_entrenchment_dig(entity: &mut Entity) {
    if let Some(movement) = entity.movement.as_mut() {
        movement.entrenchment_dig_ticks = 0;
    }
}

fn increment_entrenchment_dig(entity: &mut Entity) -> u32 {
    let Some(movement) = entity.movement.as_mut() else {
        return 0;
    };
    movement.entrenchment_dig_ticks = movement.entrenchment_dig_ticks.saturating_add(1);
    movement.entrenchment_dig_ticks
}

fn set_active_trench_occupation(entity: &mut Entity, trench_id: Option<u32>) {
    if let Some(movement) = entity.movement.as_mut() {
        movement.occupied_trench_id = trench_id;
    }
}

fn build_occupied_trench_counts(entities: &EntityStore) -> OccupiedTrenchCounts {
    let mut counts = OccupiedTrenchCounts::new();
    for entity in entities.iter() {
        if let Some(trench_id) = active_trench_occupation(entity) {
            increment_occupied_trench_count(&mut counts, trench_id);
        }
    }
    counts
}

fn trench_occupied_by_other(
    occupied_trench_counts: &OccupiedTrenchCounts,
    entity: &Entity,
    trench_id: u32,
) -> bool {
    let count = occupied_trench_counts
        .get(&trench_id)
        .copied()
        .unwrap_or_default();
    let self_count = usize::from(active_trench_occupation(entity) == Some(trench_id));
    count > self_count
}

fn set_active_trench_occupation_tracked(
    entity: &mut Entity,
    occupied_trench_counts: &mut OccupiedTrenchCounts,
    trench_id: Option<u32>,
) {
    let previous = active_trench_occupation(entity);
    set_active_trench_occupation(entity, trench_id);
    let current = active_trench_occupation(entity);
    if previous == current {
        return;
    }
    if let Some(previous) = previous {
        decrement_occupied_trench_count(occupied_trench_counts, previous);
    }
    if let Some(current) = current {
        increment_occupied_trench_count(occupied_trench_counts, current);
    }
}

fn increment_occupied_trench_count(
    occupied_trench_counts: &mut OccupiedTrenchCounts,
    trench_id: u32,
) {
    *occupied_trench_counts.entry(trench_id).or_default() += 1;
}

fn decrement_occupied_trench_count(
    occupied_trench_counts: &mut OccupiedTrenchCounts,
    trench_id: u32,
) {
    let Some(count) = occupied_trench_counts.get_mut(&trench_id) else {
        return;
    };
    *count = count.saturating_sub(1);
    if *count == 0 {
        occupied_trench_counts.remove(&trench_id);
    }
}

fn trench_radius_px(trench: Trench) -> f32 {
    trench.radius_tiles * config::TILE_SIZE as f32
}

fn trench_contains_point(trench: Trench, x: f32, y: f32) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    let dx = x - trench.x;
    let dy = y - trench.y;
    let radius = trench_radius_px(trench);
    dx * dx + dy * dy <= radius * radius
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    distance_sq(a, b).sqrt()
}

fn distance_sq(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests;
