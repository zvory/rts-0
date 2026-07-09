use std::collections::BTreeSet;

use super::{standability, Occupancy};
use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, EntityKind};
use crate::game::map::Map;
use crate::protocol::TrenchView;

mod goal_search;
mod reachability;

pub(super) use reachability::FormationReachability;

const FORMATION_NEAR_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 4.0;
const FORMATION_FAR_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 18.0;
const FORMATION_MAX_OFFSET_PX: f32 = config::TILE_SIZE as f32 * 4.0;
const FORMATION_TRENCH_PREFERENCE_RADIUS_PX: f32 = config::TILE_SIZE as f32 * 2.0;
pub(super) const VEHICLE_BODY_FORMATION_GAP_TILES: u32 = 1;

pub(super) struct PlayerKnownTrenches {
    pub(super) player: u32,
    pub(super) trenches: Vec<KnownTrench>,
    pub(super) occupied_trenches: BTreeSet<u32>,
}

#[derive(Clone, Copy)]
pub(super) struct KnownTrench {
    pub(super) id: u32,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) radius_tiles: f32,
}

#[derive(Clone, Copy)]
pub(super) struct FormationUnit {
    pub(super) id: u32,
    pub(super) kind: EntityKind,
    pub(super) pos: (f32, f32),
}

#[derive(Clone, Copy)]
pub(super) struct FormationAssignment {
    pub(super) kind: EntityKind,
    pub(super) tile: (u32, u32),
    pub(super) trench_id: Option<u32>,
}

#[derive(Clone, Copy)]
struct FormationGoal {
    point: (f32, f32),
    tile: (u32, u32),
    trench_id: Option<u32>,
}

struct FormationGoalContext<'a> {
    map: &'a Map,
    occ: &'a Occupancy<'a>,
    known_trenches: &'a [KnownTrench],
    occupied_trenches: &'a BTreeSet<u32>,
    assigned: &'a [FormationAssignment],
}

pub(super) fn known_trenches_from_views(views: Vec<TrenchView>) -> Vec<KnownTrench> {
    views
        .into_iter()
        .map(|view| KnownTrench {
            id: view.id,
            x: view.x,
            y: view.y,
            radius_tiles: view.radius_tiles,
        })
        .collect()
}

/// Assign formation-aware path goals in the same order as `units`.
#[cfg(test)]
pub(super) fn formation_goals(
    map: &Map,
    occ: &Occupancy,
    units: &[FormationUnit],
    goal: (f32, f32),
) -> Vec<(f32, f32)> {
    formation_goals_with_known_trenches(map, occ, units, goal, &[], &BTreeSet::new())
}

/// Assign formation-aware path goals in the same order as `units`, preferring nearby known
/// unoccupied trench terrain for trench-eligible infantry.
#[cfg(test)]
pub(super) fn formation_goals_with_known_trenches(
    map: &Map,
    occ: &Occupancy,
    units: &[FormationUnit],
    goal: (f32, f32),
    known_trenches: &[KnownTrench],
    occupied_trenches: &BTreeSet<u32>,
) -> Vec<(f32, f32)> {
    formation_goals_with_known_trenches_and_reachability(
        map,
        occ,
        units,
        goal,
        known_trenches,
        occupied_trenches,
        |_, _| true,
    )
}

/// Assign formation-aware path goals, rejecting candidate tiles that cannot be exactly reached
/// from the assigned unit.
pub(super) fn formation_goals_with_known_trenches_and_reachability<F>(
    map: &Map,
    occ: &Occupancy,
    units: &[FormationUnit],
    goal: (f32, f32),
    known_trenches: &[KnownTrench],
    occupied_trenches: &BTreeSet<u32>,
    mut is_goal_reachable: F,
) -> Vec<(f32, f32)>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    if units.len() <= 1 {
        let anchor = map.tile_of(goal.0, goal.1);
        return spread_goals_with_known_trenches(
            map,
            occ,
            units,
            anchor,
            goal,
            known_trenches,
            occupied_trenches,
            &mut is_goal_reachable,
        );
    }

    let inv_count = 1.0 / units.len() as f32;
    let centroid = units.iter().fold((0.0f32, 0.0f32), |acc, unit| {
        (
            acc.0 + unit.pos.0 * inv_count,
            acc.1 + unit.pos.1 * inv_count,
        )
    });
    let dx = goal.0 - centroid.0;
    let dy = goal.1 - centroid.1;
    let move_distance = (dx * dx + dy * dy).sqrt();
    let formation_scale = formation_scale_for_distance(move_distance);
    let max = map.world_size_px() - 1.0;
    let mut out = Vec::with_capacity(units.len());
    let mut assigned: Vec<FormationAssignment> = Vec::new();

    for unit in units {
        let offset = clamp_offset(
            unit.pos.0 - centroid.0,
            unit.pos.1 - centroid.1,
            FORMATION_MAX_OFFSET_PX,
        );
        let desired = (
            (goal.0 + offset.0 * formation_scale).clamp(0.0, max),
            (goal.1 + offset.1 * formation_scale).clamp(0.0, max),
        );
        let anchor = map.tile_of(desired.0, desired.1);
        let context = FormationGoalContext {
            map,
            occ,
            known_trenches,
            occupied_trenches,
            assigned: &assigned,
        };
        if let Some(formation_goal) =
            assign_formation_goal(&context, unit, anchor, desired, goal, &mut is_goal_reachable)
        {
            assigned.push(FormationAssignment {
                kind: unit.kind,
                tile: formation_goal.tile,
                trench_id: formation_goal.trench_id,
            });
            out.push(formation_goal.point);
        } else {
            out.push(unit.pos);
        }
    }

    out
}

fn spread_goals_with_known_trenches<F>(
    map: &Map,
    occ: &Occupancy,
    units: &[FormationUnit],
    anchor: (u32, u32),
    desired: (f32, f32),
    known_trenches: &[KnownTrench],
    occupied_trenches: &BTreeSet<u32>,
    is_goal_reachable: &mut F,
) -> Vec<(f32, f32)>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    let mut out = Vec::with_capacity(units.len());
    let mut assigned: Vec<FormationAssignment> = Vec::new();

    for unit in units {
        let context = FormationGoalContext {
            map,
            occ,
            known_trenches,
            occupied_trenches,
            assigned: &assigned,
        };
        if let Some(formation_goal) =
            assign_formation_goal(&context, unit, anchor, desired, desired, is_goal_reachable)
        {
            assigned.push(FormationAssignment {
                kind: unit.kind,
                tile: formation_goal.tile,
                trench_id: formation_goal.trench_id,
            });
            out.push(formation_goal.point);
        } else {
            out.push(unit.pos);
        }
    }

    out
}

fn assign_formation_goal<F>(
    context: &FormationGoalContext<'_>,
    unit: &FormationUnit,
    anchor: (u32, u32),
    desired: (f32, f32),
    formation_center: (f32, f32),
    is_goal_reachable: &mut F,
) -> Option<FormationGoal>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    if let Some(goal) = find_preferred_trench_goal(
        context.map,
        context.occ,
        unit,
        desired,
        context.known_trenches,
        context.occupied_trenches,
        context.assigned,
        is_goal_reachable,
    ) {
        return Some(goal);
    }
    goal_search::find_unique_tile_near(
        context.map,
        context.occ,
        unit,
        anchor,
        context.assigned,
        formation_center,
        is_goal_reachable,
    )
    .map(|tile| FormationGoal {
        point: context.map.tile_center(tile.0, tile.1),
        tile,
        trench_id: None,
    })
}

fn find_preferred_trench_goal<F>(
    map: &Map,
    occ: &Occupancy,
    unit: &FormationUnit,
    desired: (f32, f32),
    known_trenches: &[KnownTrench],
    occupied_trenches: &BTreeSet<u32>,
    assigned: &[FormationAssignment],
    is_goal_reachable: &mut F,
) -> Option<FormationGoal>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    if !config::is_entrenchment_eligible_infantry(unit.kind) {
        return None;
    }
    let mut best: Option<(f32, u32, FormationGoal)> = None;
    for trench in known_trenches.iter().copied() {
        if occupied_trenches.contains(&trench.id)
            || assigned
                .iter()
                .any(|assignment| assignment.trench_id == Some(trench.id))
        {
            continue;
        }
        let point = (trench.x, trench.y);
        let dist_sq = point_distance_sq(desired, point);
        if !dist_sq.is_finite() {
            continue;
        }
        let max_dist =
            FORMATION_TRENCH_PREFERENCE_RADIUS_PX + trench.radius_tiles * config::TILE_SIZE as f32;
        if dist_sq > max_dist * max_dist {
            continue;
        }
        if !formation_goal_point_free(map, occ, unit, point, assigned) {
            continue;
        }
        let tile = map.tile_of(point.0, point.1);
        if !is_goal_reachable(unit, tile) {
            continue;
        }
        let goal = FormationGoal {
            point,
            tile,
            trench_id: Some(trench.id),
        };
        let replace = best
            .map(|(best_dist_sq, best_id, _)| {
                dist_sq < best_dist_sq
                    || ((dist_sq - best_dist_sq).abs() <= f32::EPSILON && trench.id < best_id)
            })
            .unwrap_or(true);
        if replace {
            best = Some((dist_sq, trench.id, goal));
        }
    }
    best.map(|(_, _, goal)| goal)
}

fn formation_goal_point_free(
    map: &Map,
    occ: &Occupancy,
    unit: &FormationUnit,
    point: (f32, f32),
    assigned: &[FormationAssignment],
) -> bool {
    if !point.0.is_finite() || !point.1.is_finite() {
        return false;
    }
    let world_size = map.world_size_px();
    if point.0 < 0.0 || point.1 < 0.0 || point.0 >= world_size || point.1 >= world_size {
        return false;
    }
    let tile = map.tile_of(point.0, point.1);
    if assigned.iter().any(|assignment| assignment.tile == tile) {
        return false;
    }
    if !map.is_passable(tile.0 as i32, tile.1 as i32) {
        return false;
    }
    if !occ.passable_for_kind(tile.0 as i32, tile.1 as i32, unit.kind) {
        return false;
    }
    let facing = formation_goal_facing(unit, point);
    standability::unit_static_standable_with_facing(map, occ, unit.kind, point.0, point.1, facing)
}

fn point_distance_sq(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

fn formation_scale_for_distance(distance: f32) -> f32 {
    let t = ((distance - FORMATION_NEAR_DISTANCE_PX)
        / (FORMATION_FAR_DISTANCE_PX - FORMATION_NEAR_DISTANCE_PX))
        .clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn clamp_offset(dx: f32, dy: f32, max_len: f32) -> (f32, f32) {
    let len = (dx * dx + dy * dy).sqrt();
    if len <= max_len || len <= f32::EPSILON {
        return (dx, dy);
    }
    let scale = max_len / len;
    (dx * scale, dy * scale)
}

pub(super) fn is_free_goal(
    map: &Map,
    occ: &Occupancy,
    unit: &FormationUnit,
    tile: (u32, u32),
    assigned: &[FormationAssignment],
    require_preferred_spacing: bool,
) -> bool {
    if !map.is_passable(tile.0 as i32, tile.1 as i32) {
        return false;
    }
    if !occ.passable_for_kind(tile.0 as i32, tile.1 as i32, unit.kind) {
        return false;
    }
    if assigned.iter().any(|assignment| assignment.tile == tile) {
        return false;
    }
    if require_preferred_spacing && !preferred_goal_spacing_clear(unit, tile, assigned) {
        return false;
    }
    let center = map.tile_center(tile.0, tile.1);
    let facing = formation_goal_facing(unit, center);
    standability::unit_static_standable_with_facing(map, occ, unit.kind, center.0, center.1, facing)
}

fn preferred_goal_spacing_clear(
    unit: &FormationUnit,
    tile: (u32, u32),
    assigned: &[FormationAssignment],
) -> bool {
    assigned.iter().all(|assignment| {
        let gap_tiles = preferred_gap_tiles(unit.kind).max(preferred_gap_tiles(assignment.kind));
        gap_tiles == 0 || tile_chebyshev_distance(tile, assignment.tile) > gap_tiles
    })
}

fn preferred_gap_tiles(kind: EntityKind) -> u32 {
    if uses_oriented_vehicle_body(kind) {
        VEHICLE_BODY_FORMATION_GAP_TILES
    } else {
        0
    }
}

pub(super) fn tile_chebyshev_distance(a: (u32, u32), b: (u32, u32)) -> u32 {
    a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))
}

pub(super) fn formation_goal_facing(unit: &FormationUnit, center: (f32, f32)) -> f32 {
    if !uses_oriented_vehicle_body(unit.kind) {
        return 0.0;
    }
    let dx = center.0 - unit.pos.0;
    let dy = center.1 - unit.pos.1;
    let dist2 = dx * dx + dy * dy;
    if !dist2.is_finite() || dist2 <= 1.0e-4 {
        0.0
    } else {
        dy.atan2(dx)
    }
}
