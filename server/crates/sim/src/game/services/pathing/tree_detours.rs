use super::*;
use crate::config;
use crate::rules::terrain;

const CANDIDATES_PER_TREE: usize = 16;
const MAX_LOCAL_TREES: usize = 8;
const DETOUR_CLEARANCE_PX: f32 = 1.0;
const TREE_TILE_AVOIDANCE_COST: u32 = 40;

pub(super) fn movement_cost(
    pass: &TerrainPassability<'_>,
    tx: i32,
    ty: i32,
    base_step_cost: u32,
) -> u32 {
    let tree_cost = pass
        .occupancy
        .tree_path_avoidance_cost(tx, ty)
        .saturating_mul(TREE_TILE_AVOIDANCE_COST);
    let slow_cost = if movement_body_class(pass.kind) == MovementBodyClass::InfantryLike
        && pass.map.in_bounds(tx, ty)
    {
        terrain::slow_movement_path_cost_surcharge(
            base_step_cost,
            pass.map.is_slow_movement_tile(tx as u32, ty as u32),
        )
    } else {
        0
    };
    if pass.route_shape != RouteShape::VehicleClearance || !uses_oriented_vehicle_body(pass.kind) {
        return tree_cost.saturating_add(slow_cost);
    }
    tree_cost
        .saturating_add(slow_cost)
        .saturating_add(vehicle_clearance_cost(
            pass.occupancy.clearance_at_tile_for_movement_body(
                tx,
                ty,
                movement_body_class(pass.kind),
            ),
        ))
        .saturating_add(pass.vehicle_corner_cost(tx, ty))
}

pub(in crate::game::services) fn expand_reverse_waypoints(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    start: (f32, f32),
    reverse_waypoints: Vec<(f32, f32)>,
) -> Option<Vec<(f32, f32)>> {
    let forward = reverse_waypoints.into_iter().rev().collect::<Vec<_>>();
    let mut expanded = Vec::with_capacity(forward.len());
    let mut from = start;
    let mut index = 0;
    while index < forward.len() {
        let mut target = forward[index];
        while !standability::unit_static_standable(map, occupancy, kind, target.0, target.1) {
            if index + 1 == forward.len() {
                return None;
            }
            index += 1;
            target = forward[index];
        }
        if !standability::unit_tree_segment_clear(occupancy, kind, from, target) {
            let detour = tree_detour_between(map, occupancy, kind, from, target)?;
            expanded.extend(detour);
        }
        expanded.push(target);
        from = target;
        index += 1;
    }
    expanded.reverse();
    Some(expanded)
}

pub(in crate::game::services) fn tree_detour_between(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
) -> Option<Vec<(f32, f32)>> {
    if standability::unit_static_segment_standable(map, occupancy, kind, from, to) {
        return Some(Vec::new());
    }
    if standability::unit_tree_segment_clear(occupancy, kind, from, to) {
        return None;
    }
    let trunks = nearby_trunks(occupancy, from, to)?;
    let body_radius = standability::unit_bounding_radius(kind)?;
    let candidate_radius =
        body_radius + crate::game::map::doodads::TREE_TRUNK_RADIUS_PX + DETOUR_CLEARANCE_PX;
    let mut nodes = vec![from];
    for &(x, y) in &trunks {
        for index in 0..CANDIDATES_PER_TREE {
            let angle = std::f32::consts::TAU * index as f32 / CANDIDATES_PER_TREE as f32;
            let point = (
                x + angle.cos() * candidate_radius,
                y + angle.sin() * candidate_radius,
            );
            if standability::unit_static_standable(map, occupancy, kind, point.0, point.1) {
                nodes.push(point);
            }
        }
    }
    nodes.push(to);
    shortest_visible_route(map, occupancy, kind, &nodes)
}

fn nearby_trunks(
    occupancy: &Occupancy<'_>,
    from: (f32, f32),
    to: (f32, f32),
) -> Option<Vec<(f32, f32)>> {
    let ts = config::TILE_SIZE as f32;
    let min_tx = (from.0.min(to.0) / ts).floor() as i32 - 1;
    let min_ty = (from.1.min(to.1) / ts).floor() as i32 - 1;
    let max_tx = (from.0.max(to.0) / ts).floor() as i32 + 1;
    let max_ty = (from.1.max(to.1) / ts).floor() as i32 + 1;
    let mut trunks = Vec::new();
    for trunk in occupancy.tree_trunks_in_tile_rect(min_tx, min_ty, max_tx, max_ty) {
        trunks.push(trunk);
        if trunks.len() > MAX_LOCAL_TREES {
            return None;
        }
    }
    (!trunks.is_empty()).then_some(trunks)
}

fn shortest_visible_route(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    nodes: &[(f32, f32)],
) -> Option<Vec<(f32, f32)>> {
    let goal = nodes.len().checked_sub(1)?;
    let mut distances = vec![f32::INFINITY; nodes.len()];
    let mut previous = vec![None; nodes.len()];
    let mut visited = vec![false; nodes.len()];
    distances[0] = 0.0;
    for _ in 0..nodes.len() {
        let current = (0..nodes.len())
            .filter(|&index| !visited[index])
            .min_by(|&a, &b| distances[a].total_cmp(&distances[b]))?;
        if !distances[current].is_finite() {
            break;
        }
        if current == goal {
            break;
        }
        visited[current] = true;
        for next in 1..nodes.len() {
            if visited[next]
                || !standability::unit_static_segment_standable(
                    map,
                    occupancy,
                    kind,
                    nodes[current],
                    nodes[next],
                )
            {
                continue;
            }
            let dx = nodes[next].0 - nodes[current].0;
            let dy = nodes[next].1 - nodes[current].1;
            let candidate = distances[current] + (dx * dx + dy * dy).sqrt();
            if candidate < distances[next] {
                distances[next] = candidate;
                previous[next] = Some(current);
            }
        }
    }
    if !distances[goal].is_finite() {
        return None;
    }
    let mut route = Vec::new();
    let mut current = goal;
    while let Some(parent) = previous[current] {
        if current != goal {
            route.push(nodes[current]);
        }
        current = parent;
    }
    route.reverse();
    Some(route)
}
