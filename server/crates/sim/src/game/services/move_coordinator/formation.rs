use std::collections::BTreeSet;

use super::{standability, Occupancy};
use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, EntityKind};
use crate::game::map::Map;
use crate::protocol::TrenchView;

mod goal_search;
mod polyline;
mod reachability;
#[cfg(test)]
mod tests;

pub(super) use polyline::{
    formation_goals_with_reachability as polyline_formation_goals_with_reachability,
    slots as polyline_slots,
};
pub(super) use reachability::FormationReachability;

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

#[derive(Clone, Copy)]
struct FormationInputContext<'state> {
    map: &'state Map,
    occ: &'state Occupancy<'state>,
    known_trenches: &'state [KnownTrench],
    occupied_trenches: &'state BTreeSet<u32>,
}

struct FormationGoalContext<'state, 'assigned> {
    map: &'state Map,
    occ: &'state Occupancy<'state>,
    known_trenches: &'state [KnownTrench],
    occupied_trenches: &'state BTreeSet<u32>,
    assigned: &'assigned [FormationAssignment],
}

impl<'state> FormationInputContext<'state> {
    fn with_assigned<'assigned>(
        &self,
        assigned: &'assigned [FormationAssignment],
    ) -> FormationGoalContext<'state, 'assigned> {
        FormationGoalContext {
            map: self.map,
            occ: self.occ,
            known_trenches: self.known_trenches,
            occupied_trenches: self.occupied_trenches,
            assigned,
        }
    }
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
    let inputs = FormationInputContext {
        map,
        occ,
        known_trenches,
        occupied_trenches,
    };
    let desired_points = compact_formation_points(map, units, goal);
    let mut out = Vec::with_capacity(units.len());
    let mut assigned: Vec<FormationAssignment> = Vec::new();

    for (unit, desired) in units.iter().zip(desired_points) {
        let anchor = map.tile_of(desired.0, desired.1);
        let context = inputs.with_assigned(&assigned);
        if let Some(formation_goal) =
            assign_formation_goal(&context, unit, anchor, desired, &mut is_goal_reachable)
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

/// Build one compact translated layout. Broad rows retain top-to-bottom order, and units within a
/// row retain left-to-right order, but original world-space separation is discarded. Infantry
/// occupies adjacent tiles; a selection containing a vehicle uses a two-tile pitch.
fn compact_formation_points(
    map: &Map,
    units: &[FormationUnit],
    center: (f32, f32),
) -> Vec<(f32, f32)> {
    if units.len() <= 1 {
        return units.iter().map(|_| center).collect();
    }

    let (min_x, max_x, min_y, max_y) = units.iter().fold(
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), unit| {
            (
                min_x.min(unit.pos.0),
                max_x.max(unit.pos.0),
                min_y.min(unit.pos.1),
                max_y.max(unit.pos.1),
            )
        },
    );
    let width = max_x - min_x;
    let height = max_y - min_y;
    let columns = if height <= f32::EPSILON {
        units.len()
    } else if width <= f32::EPSILON {
        1
    } else {
        ((units.len() as f32 * width / height).sqrt().round() as usize).clamp(1, units.len())
    };
    let rows = units.len().div_ceil(columns);
    let pitch_tiles = if units
        .iter()
        .any(|unit| uses_oriented_vehicle_body(unit.kind))
    {
        VEHICLE_BODY_FORMATION_GAP_TILES + 1
    } else {
        1
    };
    let max_row_size = columns.min(units.len()) as u32;
    let width_tiles = max_row_size.saturating_sub(1) * pitch_tiles;
    let height_tiles = (rows as u32).saturating_sub(1) * pitch_tiles;
    let center_tile = map.tile_of(center.0, center.1);
    let start_x = centered_tile_start(center_tile.0, width_tiles, map.size);
    let start_y = centered_tile_start(center_tile.1, height_tiles, map.size);

    let mut ordered = (0..units.len()).collect::<Vec<_>>();
    ordered.sort_by(|&a, &b| {
        units[a]
            .pos
            .1
            .total_cmp(&units[b].pos.1)
            .then_with(|| units[a].pos.0.total_cmp(&units[b].pos.0))
            .then_with(|| units[a].id.cmp(&units[b].id))
    });

    let mut points = vec![center; units.len()];
    for (row, row_units) in ordered.chunks_mut(columns).enumerate() {
        row_units.sort_by(|&a, &b| {
            units[a]
                .pos
                .0
                .total_cmp(&units[b].pos.0)
                .then_with(|| units[a].pos.1.total_cmp(&units[b].pos.1))
                .then_with(|| units[a].id.cmp(&units[b].id))
        });
        let row_start_x = start_x
            + compact_row_start_column(units, row_units, min_x, width, columns) as u32
                * pitch_tiles;
        for (column, &unit_index) in row_units.iter().enumerate() {
            let tile = (
                row_start_x + column as u32 * pitch_tiles,
                start_y + row as u32 * pitch_tiles,
            );
            points[unit_index] = map.tile_center(tile.0, tile.1);
        }
    }
    points
}

fn compact_row_start_column(
    units: &[FormationUnit],
    row_units: &[usize],
    min_x: f32,
    width: f32,
    columns: usize,
) -> usize {
    if row_units.len() >= columns || width <= f32::EPSILON {
        return 0;
    }
    let mean_x = row_units
        .iter()
        .map(|&index| units[index].pos.0)
        .sum::<f32>()
        / row_units.len() as f32;
    let desired_center = ((mean_x - min_x) / width) * (columns - 1) as f32;
    let half_row = (row_units.len() - 1) as f32 * 0.5;
    (desired_center - half_row)
        .round()
        .clamp(0.0, (columns - row_units.len()) as f32) as usize
}

fn centered_tile_start(center: u32, span: u32, map_size: u32) -> u32 {
    center
        .saturating_sub(span.div_ceil(2))
        .min(map_size.saturating_sub(span.saturating_add(1)))
}

fn assign_formation_goal<F>(
    context: &FormationGoalContext<'_, '_>,
    unit: &FormationUnit,
    anchor: (u32, u32),
    desired: (f32, f32),
    is_goal_reachable: &mut F,
) -> Option<FormationGoal>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    if let Some(goal) = find_preferred_trench_goal(context, unit, desired, is_goal_reachable) {
        return Some(goal);
    }
    goal_search::find_unique_tile_near(
        context.map,
        context.occ,
        unit,
        anchor,
        context.assigned,
        is_goal_reachable,
    )
    .map(|tile| FormationGoal {
        point: context.map.tile_center(tile.0, tile.1),
        tile,
        trench_id: None,
    })
}

fn find_preferred_trench_goal<F>(
    context: &FormationGoalContext<'_, '_>,
    unit: &FormationUnit,
    desired: (f32, f32),
    is_goal_reachable: &mut F,
) -> Option<FormationGoal>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    if !config::is_entrenchment_eligible_infantry(unit.kind) {
        return None;
    }
    let mut best: Option<(f32, u32, FormationGoal)> = None;
    for trench in context.known_trenches.iter().copied() {
        if context.occupied_trenches.contains(&trench.id)
            || context
                .assigned
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
        if !formation_goal_point_free(context.map, context.occ, unit, point, context.assigned) {
            continue;
        }
        let tile = context.map.tile_of(point.0, point.1);
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
    if !preferred_goal_spacing_clear(unit, tile, assigned) {
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
