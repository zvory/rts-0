use super::*;

/// Build deterministic requested slots along a freehand polyline. A line long enough for the
/// selection uses one rank and the entire stroke; shorter lines grow parallel ranks instead of
/// stacking units onto the same point.
pub(in crate::game) fn slots(
    units: &[FormationUnit],
    points: &[(f32, f32)],
) -> Vec<(u32, (f32, f32))> {
    if units.is_empty() || points.len() < 2 {
        return Vec::new();
    }
    let mut cumulative = Vec::with_capacity(points.len());
    cumulative.push(0.0);
    for segment in points.windows(2) {
        let dx = segment[1].0 - segment[0].0;
        let dy = segment[1].1 - segment[0].1;
        let length = (dx * dx + dy * dy).sqrt();
        cumulative.push(cumulative.last().copied().unwrap_or(0.0) + length);
    }
    let total = cumulative.last().copied().unwrap_or(0.0);
    if !total.is_finite() || total <= f32::EPSILON {
        return Vec::new();
    }

    let spacing = units
        .iter()
        .fold(config::TILE_SIZE as f32, |spacing, unit| {
            let diameter = config::unit_stats(unit.kind)
                .map(|stats| stats.radius * 2.0)
                .unwrap_or(config::TILE_SIZE as f32);
            spacing.max(diameter + config::TILE_SIZE as f32 * 0.25)
        });
    let per_rank = (((total / spacing).floor() as usize).saturating_add(1)).clamp(1, units.len());
    let rank_count = units.len().div_ceil(per_rank);
    let mut slots = Vec::with_capacity(units.len());
    for index in 0..units.len() {
        let rank = index / per_rank;
        let in_rank = index % per_rank;
        let rank_size = (units.len() - rank * per_rank).min(per_rank);
        let distance = if rank_size <= 1 {
            total * 0.5
        } else {
            total * in_rank as f32 / (rank_size - 1) as f32
        };
        let (point, tangent) = point_and_tangent_at_distance(points, &cumulative, distance);
        let normal = (-tangent.1, tangent.0);
        let rank_offset = (rank as f32 - (rank_count - 1) as f32 * 0.5) * spacing;
        slots.push((
            point.0 + normal.0 * rank_offset,
            point.1 + normal.1 * rank_offset,
        ));
    }

    let mut ordered_units = units.to_vec();
    ordered_units.sort_by_key(|unit| unit.id);
    let mut available = (0..slots.len()).collect::<Vec<_>>();
    let mut assigned = Vec::with_capacity(units.len());
    for unit in ordered_units {
        let Some((available_index, _)) = available
            .iter()
            .enumerate()
            .map(|(available_index, slot_index)| {
                (
                    available_index,
                    point_distance_sq(unit.pos, slots[*slot_index]),
                )
            })
            .min_by(|(a_index, a_dist), (b_index, b_dist)| {
                a_dist
                    .total_cmp(b_dist)
                    .then_with(|| available[*a_index].cmp(&available[*b_index]))
            })
        else {
            break;
        };
        let slot_index = available.remove(available_index);
        assigned.push((unit.id, slots[slot_index]));
    }
    assigned.sort_by_key(|(id, _)| *id);
    assigned
}

fn point_and_tangent_at_distance(
    points: &[(f32, f32)],
    cumulative: &[f32],
    distance: f32,
) -> ((f32, f32), (f32, f32)) {
    let segment_index = cumulative
        .windows(2)
        .position(|window| distance <= window[1])
        .unwrap_or(points.len().saturating_sub(2));
    let start = points[segment_index];
    let end = points[segment_index + 1];
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= f32::EPSILON {
        return (start, (1.0, 0.0));
    }
    let t = ((distance - cumulative[segment_index]) / length).clamp(0.0, 1.0);
    (
        (start.0 + dx * t, start.1 + dy * t),
        (dx / length, dy / length),
    )
}

pub(in crate::game) fn formation_goals_with_reachability<F>(
    map: &Map,
    occ: &Occupancy,
    units: &[FormationUnit],
    desired_by_id: &[(u32, (f32, f32))],
    known_trenches: &[KnownTrench],
    occupied_trenches: &BTreeSet<u32>,
    mut is_goal_reachable: F,
) -> Vec<(u32, (f32, f32))>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    let inputs = FormationInputContext {
        map,
        occ,
        known_trenches,
        occupied_trenches,
    };
    let max = map.world_size_px() - 1.0;
    let mut assigned = Vec::new();
    let mut out = Vec::with_capacity(units.len());
    for unit in units {
        let Some((_, requested)) = desired_by_id.iter().find(|(id, _)| *id == unit.id) else {
            continue;
        };
        let desired = (requested.0.clamp(0.0, max), requested.1.clamp(0.0, max));
        let anchor = map.tile_of(desired.0, desired.1);
        let context = inputs.with_assigned(&assigned);
        if let Some(goal) = assign_formation_goal(
            &context,
            unit,
            anchor,
            desired,
            desired,
            &mut is_goal_reachable,
        ) {
            assigned.push(FormationAssignment {
                kind: unit.kind,
                tile: goal.tile,
                trench_id: goal.trench_id,
            });
            out.push((unit.id, goal.point));
        } else {
            out.push((unit.id, unit.pos));
        }
    }
    out
}
