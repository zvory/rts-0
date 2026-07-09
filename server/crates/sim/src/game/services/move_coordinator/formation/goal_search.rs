use std::collections::BTreeSet;

use super::{is_free_goal, point_distance_sq, FormationAssignment, FormationUnit, Occupancy};
use crate::game::map::Map;

struct GoalSearchContext<'state, 'unit, 'assigned> {
    map: &'state Map,
    occ: &'state Occupancy<'state>,
    unit: &'unit FormationUnit,
    assigned: &'assigned [FormationAssignment],
    formation_center: (f32, f32),
}

/// Search outward from `anchor` in deterministic ring order and return the first body-standable
/// tile not already assigned. Some unit kinds prefer additional spacing and get a strict first
/// pass before falling back to the ordinary unique-tile rule.
pub(super) fn find_unique_tile_near<F>(
    map: &Map,
    occ: &Occupancy,
    unit: &FormationUnit,
    anchor: (u32, u32),
    assigned: &[FormationAssignment],
    formation_center: (f32, f32),
    is_goal_reachable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    let context = GoalSearchContext {
        map,
        occ,
        unit,
        assigned,
        formation_center,
    };
    if let Some(tile) = find_unique_tile_near_with_spacing(
        &context,
        anchor,
        is_goal_reachable,
        true,
    ) {
        return Some(tile);
    }
    find_unique_tile_near_with_spacing(
        &context,
        anchor,
        is_goal_reachable,
        false,
    )
    .or_else(|| find_free_goal_near_with_spacing(map, occ, unit, anchor, assigned, true))
    .or_else(|| find_free_goal_near_with_spacing(map, occ, unit, anchor, assigned, false))
}

fn find_unique_tile_near_with_spacing<F>(
    context: &GoalSearchContext<'_, '_, '_>,
    anchor: (u32, u32),
    is_goal_reachable: &mut F,
    require_preferred_spacing: bool,
) -> Option<(u32, u32)>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    let anchor_free = is_free_goal(
        context.map,
        context.occ,
        context.unit,
        anchor,
        context.assigned,
        require_preferred_spacing,
    );
    if anchor_free && is_goal_reachable(context.unit, anchor) {
        return Some(anchor);
    }
    if anchor_free {
        if let Some(tile) = find_center_biased_tile_with_spacing(
            context,
            anchor,
            is_goal_reachable,
            require_preferred_spacing,
        ) {
            return Some(tile);
        }
    }
    if let Some(tile) = find_tile_near(anchor, false, |tile| {
        is_reachable_free_goal(
            context,
            tile,
            require_preferred_spacing,
            is_goal_reachable,
        )
    }) {
        return Some(tile);
    }

    if !anchor_free {
        if let Some(tile) = find_center_biased_tile_with_spacing(
            context,
            anchor,
            is_goal_reachable,
            require_preferred_spacing,
        ) {
            return Some(tile);
        }
    }

    None
}

fn find_center_biased_tile_with_spacing<F>(
    context: &GoalSearchContext<'_, '_, '_>,
    anchor: (u32, u32),
    is_goal_reachable: &mut F,
    require_preferred_spacing: bool,
) -> Option<(u32, u32)>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    let anchor_center = context.map.tile_center(anchor.0, anchor.1);
    let original_dist_sq = point_distance_sq(anchor_center, context.formation_center);
    if !original_dist_sq.is_finite() || original_dist_sq <= f32::EPSILON {
        return None;
    }

    let center_tile = context
        .map
        .tile_of(context.formation_center.0, context.formation_center.1);
    let steps = anchor
        .0
        .abs_diff(center_tile.0)
        .max(anchor.1.abs_diff(center_tile.1));
    if steps == 0 {
        return None;
    }

    let mut anchors = Vec::new();
    let mut seen = BTreeSet::new();
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let point = (
            anchor_center.0 + (context.formation_center.0 - anchor_center.0) * t,
            anchor_center.1 + (context.formation_center.1 - anchor_center.1) * t,
        );
        let fallback_anchor = context.map.tile_of(point.0, point.1);
        if fallback_anchor == anchor || !seen.insert(fallback_anchor) {
            continue;
        }
        anchors.push(fallback_anchor);
    }

    for &fallback_anchor in &anchors {
        if center_biased_tile_closer(
            context.map,
            fallback_anchor,
            context.formation_center,
            original_dist_sq,
        )
            && is_reachable_free_goal(
                context,
                fallback_anchor,
                require_preferred_spacing,
                is_goal_reachable,
            )
        {
            return Some(fallback_anchor);
        }
    }

    for fallback_anchor in anchors {
        if let Some(tile) = find_reachable_free_goal_near_with_spacing(
            context,
            fallback_anchor,
            require_preferred_spacing,
            is_goal_reachable,
            |candidate| {
                center_biased_tile_closer(
                    context.map,
                    candidate,
                    context.formation_center,
                    original_dist_sq,
                )
            },
        ) {
            return Some(tile);
        }
    }

    None
}

fn center_biased_tile_closer(
    map: &Map,
    tile: (u32, u32),
    formation_center: (f32, f32),
    original_dist_sq: f32,
) -> bool {
    point_distance_sq(map.tile_center(tile.0, tile.1), formation_center) < original_dist_sq
}

fn find_reachable_free_goal_near_with_spacing<F, A>(
    context: &GoalSearchContext<'_, '_, '_>,
    anchor: (u32, u32),
    require_preferred_spacing: bool,
    is_goal_reachable: &mut F,
    mut accept_tile: A,
) -> Option<(u32, u32)>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
    A: FnMut((u32, u32)) -> bool,
{
    find_tile_near(anchor, true, |tile| {
        accept_tile(tile)
            && is_reachable_free_goal(
                context,
                tile,
                require_preferred_spacing,
                is_goal_reachable,
            )
    })
}

fn is_reachable_free_goal<F>(
    context: &GoalSearchContext<'_, '_, '_>,
    tile: (u32, u32),
    require_preferred_spacing: bool,
    is_goal_reachable: &mut F,
) -> bool
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    is_free_goal(
        context.map,
        context.occ,
        context.unit,
        tile,
        context.assigned,
        require_preferred_spacing,
    ) && is_goal_reachable(context.unit, tile)
}

fn find_free_goal_near_with_spacing(
    map: &Map,
    occ: &Occupancy,
    unit: &FormationUnit,
    anchor: (u32, u32),
    assigned: &[FormationAssignment],
    require_preferred_spacing: bool,
) -> Option<(u32, u32)> {
    find_tile_near(anchor, true, |tile| {
        is_free_goal(map, occ, unit, tile, assigned, require_preferred_spacing)
    })
}

fn find_tile_near<V>(
    anchor: (u32, u32),
    include_anchor: bool,
    mut is_candidate_valid: V,
) -> Option<(u32, u32)>
where
    V: FnMut((u32, u32)) -> bool,
{
    if include_anchor && is_candidate_valid(anchor) {
        return Some(anchor);
    }
    for r in 1i32..=6 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let tx = anchor.0 as i32 + dx;
                let ty = anchor.1 as i32 + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let tile = (tx as u32, ty as u32);
                if is_candidate_valid(tile) {
                    return Some(tile);
                }
            }
        }
    }
    None
}
