use super::route_cost::RouteCostModel;
use super::*;
use crate::config;

const MAX_TERRAIN_SHORTCUT_CANDIDATES: usize = 64;
const MAX_RAW_WAYPOINTS_PER_ANCHOR: usize = 64;

pub(super) fn simplify_fastest_terrain_route(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    start: (f32, f32),
    route_shape: RouteShape,
    reverse_waypoints: Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    if reverse_waypoints.len() <= 1 {
        return reverse_waypoints;
    }
    let forward = reverse_waypoints.into_iter().rev().collect::<Vec<_>>();
    let mut positions = Vec::with_capacity(forward.len() + 1);
    positions.push(start);
    positions.extend(forward.iter().copied());
    let vehicle_authored_corridor =
        route_shape == RouteShape::VehicleClearance && uses_oriented_vehicle_body(kind);
    if vehicle_authored_corridor {
        return simplify_fastest_vehicle_corridor(map, occupancy, kind, start, &forward);
    }
    let model = RouteCostModel::new(map);
    let mut prefix_cost = vec![0_u64; positions.len()];
    for index in 1..positions.len() {
        let Some(cost) = model.segment_cost(positions[index - 1], positions[index]) else {
            return forward.into_iter().rev().collect();
        };
        prefix_cost[index] = prefix_cost[index - 1].saturating_add(cost);
    }

    let mut protected_prefix = vec![0_usize; forward.len() + 1];
    for (index, waypoint) in forward.iter().enumerate() {
        let (tx, ty) = map.tile_of(waypoint.0, waypoint.1);
        protected_prefix[index + 1] = protected_prefix[index]
            + usize::from(occupancy.tree_path_avoidance_cost(tx as i32, ty as i32) > 0);
    }

    // Search among semantic anchors rather than every tile center. Direction changes preserve
    // obstacle geometry, cost-rate changes preserve terrain transitions, and shaping/tree anchors
    // are explicit. The periodic anchor keeps the candidate scan bounded on very long routes.
    let mut anchor_indices = Vec::new();
    for index in 0..forward.len() {
        let position = index + 1;
        let tree_protected = protected_prefix[index + 1] > protected_prefix[index];
        let direction_changes = position + 1 < positions.len()
            && segment_direction(positions[position - 1], positions[position])
                != segment_direction(positions[position], positions[position + 1]);
        let cost_rate_changes = position + 1 < positions.len()
            && segment_cost_rate_changed(
                positions[position - 1],
                positions[position],
                positions[position + 1],
                prefix_cost[position].saturating_sub(prefix_cost[position - 1]),
                prefix_cost[position + 1].saturating_sub(prefix_cost[position]),
            );
        if index + 1 == forward.len()
            || tree_protected
            || direction_changes
            || cost_rate_changes
            || (index + 1) % MAX_RAW_WAYPOINTS_PER_ANCHOR == 0
        {
            anchor_indices.push(index);
        }
    }

    let mut selected = Vec::with_capacity(anchor_indices.len());
    let mut position_index = 0_usize;
    let mut anchor_cursor = 0_usize;
    while anchor_cursor < anchor_indices.len() {
        let last_candidate =
            (anchor_cursor + MAX_TERRAIN_SHORTCUT_CANDIDATES).min(anchor_indices.len() - 1);
        let mut chosen_cursor = None;
        for candidate_cursor in (anchor_cursor..=last_candidate).rev() {
            let candidate = anchor_indices[candidate_cursor];
            let skipped_protected = protected_prefix[candidate] > protected_prefix[position_index];
            if skipped_protected {
                continue;
            }
            let retained_cost =
                prefix_cost[candidate + 1].saturating_sub(prefix_cost[position_index]);
            if !model
                .segment_cost(positions[position_index], forward[candidate])
                .is_some_and(|direct_cost| direct_cost <= retained_cost)
            {
                continue;
            }
            if !body_sweep_legal(
                map,
                occupancy,
                kind,
                positions[position_index],
                forward[candidate],
            ) {
                continue;
            }
            chosen_cursor = Some(candidate_cursor);
            break;
        }
        let Some(chosen_cursor) = chosen_cursor else {
            if position_index >= forward.len() {
                break;
            }
            selected.push(forward[position_index]);
            position_index += 1;
            while anchor_cursor < anchor_indices.len()
                && anchor_indices[anchor_cursor] < position_index
            {
                anchor_cursor += 1;
            }
            continue;
        };
        let chosen = anchor_indices[chosen_cursor];
        selected.push(forward[chosen]);
        position_index = chosen + 1;
        anchor_cursor = chosen_cursor + 1;
    }
    selected.reverse();
    selected
}

fn simplify_fastest_vehicle_corridor(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    start: (f32, f32),
    forward: &[(f32, f32)],
) -> Vec<(f32, f32)> {
    let mut selected = Vec::with_capacity(forward.len());
    let mut next_index = 0_usize;
    let mut from = start;
    while next_index < forward.len() {
        let last_candidate = (next_index + MAX_TERRAIN_SHORTCUT_CANDIDATES).min(forward.len() - 1);
        let mut run_end = next_index;
        while run_end < last_candidate {
            let candidate = run_end + 1;
            if !same_forward_line(from, forward[next_index], forward[candidate]) {
                break;
            }
            run_end = candidate;
        }
        let mut chosen = next_index;
        for candidate in (next_index..=run_end).rev() {
            if (next_index..=candidate).any(|index| {
                let (tx, ty) = map.tile_of(forward[index].0, forward[index].1);
                occupancy.tree_path_avoidance_cost(tx as i32, ty as i32) > 0
            }) {
                continue;
            }
            if kind == EntityKind::ScoutCar
                && (forward[candidate].0 - from.0).hypot(forward[candidate].1 - from.1)
                    > super::route_finalize::SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX
            {
                continue;
            }
            // An exactly collinear span traverses the same directed terrain intervals and carries
            // no omitted turn/clearance/corner event, so its composite cost is identical.
            if !body_sweep_legal(map, occupancy, kind, from, forward[candidate]) {
                continue;
            }
            chosen = candidate;
            break;
        }
        selected.push(forward[chosen]);
        from = forward[chosen];
        next_index = chosen + 1;
    }
    selected.reverse();
    selected
}

fn same_forward_line(from: (f32, f32), first: (f32, f32), candidate: (f32, f32)) -> bool {
    let first_delta = (first.0 - from.0, first.1 - from.1);
    let candidate_delta = (candidate.0 - from.0, candidate.1 - from.1);
    let cross = first_delta.0 * candidate_delta.1 - first_delta.1 * candidate_delta.0;
    let dot = first_delta.0 * candidate_delta.0 + first_delta.1 * candidate_delta.1;
    cross.abs() <= 0.001 && dot > 0.0
}

fn segment_direction(from: (f32, f32), to: (f32, f32)) -> (i8, i8) {
    let axis = |delta: f32| {
        if delta > 0.0 {
            1
        } else if delta < 0.0 {
            -1
        } else {
            0
        }
    };
    (axis(to.0 - from.0), axis(to.1 - from.1))
}

fn segment_cost_rate_changed(
    from: (f32, f32),
    at: (f32, f32),
    to: (f32, f32),
    before_cost: u64,
    after_cost: u64,
) -> bool {
    let before_distance = f64::from((at.0 - from.0).hypot(at.1 - from.1));
    let after_distance = f64::from((to.0 - at.0).hypot(to.1 - at.1));
    if before_distance <= f64::EPSILON || after_distance <= f64::EPSILON {
        return true;
    }
    let before_rate = before_cost as f64 / before_distance;
    let after_rate = after_cost as f64 / after_distance;
    (before_rate - after_rate).abs() > before_rate.max(after_rate) * 1.0e-9
}

/// Conservative exact swept-circle test against authored tile/building rectangles. Expanding the
/// rectangles by the body's bounding radius may retain extra anchors at corners, but any accepted
/// segment is body-legal. Tree trunks are checked by their independent geometry oracle.
fn body_sweep_legal(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
) -> bool {
    if !standability::unit_static_standable(map, occupancy, kind, from.0, from.1)
        || !standability::unit_static_standable(map, occupancy, kind, to.0, to.1)
        || !standability::unit_tree_segment_clear(occupancy, kind, from, to)
    {
        return false;
    }
    let Some(radius) = standability::unit_bounding_radius(kind) else {
        return false;
    };
    let world_width = map.world_width_px();
    let world_height = map.world_height_px();
    if from.0.min(to.0) - radius < 0.0
        || from.1.min(to.1) - radius < 0.0
        || from.0.max(to.0) + radius >= world_width
        || from.1.max(to.1) + radius >= world_height
    {
        return false;
    }
    let tile_size = config::TILE_SIZE as f32;
    let min_tx = ((from.0.min(to.0) - radius) / tile_size).floor() as i32;
    let min_ty = ((from.1.min(to.1) - radius) / tile_size).floor() as i32;
    let max_tx = ((from.0.max(to.0) + radius) / tile_size).floor() as i32;
    let max_ty = ((from.1.max(to.1) + radius) / tile_size).floor() as i32;
    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            if map.is_passable(tx, ty) && occupancy.passable_for_kind(tx, ty, kind) {
                continue;
            }
            let min = (
                tx as f32 * tile_size - radius,
                ty as f32 * tile_size - radius,
            );
            let max = (
                (tx + 1) as f32 * tile_size + radius,
                (ty + 1) as f32 * tile_size + radius,
            );
            if segment_intersects_aabb(from, to, min, max) {
                return false;
            }
        }
    }
    true
}

fn segment_intersects_aabb(
    from: (f32, f32),
    to: (f32, f32),
    min: (f32, f32),
    max: (f32, f32),
) -> bool {
    let delta = (to.0 - from.0, to.1 - from.1);
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for (origin, direction, axis_min, axis_max) in [
        (from.0, delta.0, min.0, max.0),
        (from.1, delta.1, min.1, max.1),
    ] {
        if direction.abs() <= f32::EPSILON {
            if origin >= axis_min && origin <= axis_max {
                continue;
            }
            return false;
        }
        let a = (axis_min - origin) / direction;
        let b = (axis_max - origin) / direction;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return false;
        }
    }
    true
}
