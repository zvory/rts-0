use super::*;
use crate::config;

const CANDIDATES_PER_TREE: usize = 16;
const MAX_LOCAL_TREES: usize = 8;
const DETOUR_CLEARANCE_PX: f32 = 1.0;
const TREE_TILE_AVOIDANCE_COST: u32 = 40;

pub(super) fn movement_cost(pass: &TerrainPassability<'_>, tx: i32, ty: i32) -> u32 {
    let tree_cost = pass
        .occupancy
        .tree_path_avoidance_cost(tx, ty)
        .saturating_mul(TREE_TILE_AVOIDANCE_COST);
    if pass.route_shape != RouteShape::VehicleClearance || !uses_oriented_vehicle_body(pass.kind) {
        return tree_cost;
    }
    tree_cost
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::EntityStore;
    use crate::protocol::{terrain, MapDoodad};

    #[test]
    fn rifleman_tile_path_routes_around_tree_trunk_tile() {
        let size = 12;
        let mut map = Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            ..Default::default()
        };
        let trunk = map.tile_center(5, 5);
        map.doodads.push(MapDoodad {
            id: 1,
            type_id: "tree.spruce".to_string(),
            x: trunk.0 as u32,
            y: trunk.1 as u32,
            color: None,
        });
        let occupancy = Occupancy::build(&map, &EntityStore::new());
        let path = PathingService::new(2_000, 16).request_tile_path(
            &map,
            &occupancy,
            PathRequest {
                kind: EntityKind::Rifleman,
                start: (3, 5),
                goal: (7, 5),
                radius_tiles: 0,
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );
        assert_eq!(path.last(), Some(&(7, 5)));
        assert!(!path.contains(&(5, 5)), "tree tile appeared in {path:?}");
    }

    #[test]
    fn local_visibility_route_crosses_a_tree_tile_without_crossing_its_trunk() {
        let size = 12;
        let mut map = Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            ..Default::default()
        };
        let trunk = map.tile_center(5, 5);
        map.doodads.push(MapDoodad {
            id: 1,
            type_id: "tree.oak".to_string(),
            x: trunk.0 as u32,
            y: trunk.1 as u32,
            color: None,
        });
        let occupancy = Occupancy::build(&map, &EntityStore::new());
        let from = (trunk.0 - 15.0, trunk.1);
        let to = (trunk.0 + 15.0, trunk.1);
        let detour = tree_detour_between(&map, &occupancy, EntityKind::Rifleman, from, to)
            .expect("same-tile trunk should have a local route");

        assert!(detour.len() >= 2, "route was {detour:?}");
        let route = std::iter::once(from)
            .chain(detour)
            .chain(std::iter::once(to))
            .collect::<Vec<_>>();
        assert!(route.windows(2).all(|step| {
            standability::unit_static_segment_standable(
                &map,
                &occupancy,
                EntityKind::Rifleman,
                step[0],
                step[1],
            )
        }));

        let expanded = expand_reverse_waypoints(
            &map,
            &occupancy,
            EntityKind::Rifleman,
            from,
            vec![to, trunk],
        )
        .expect("blocked tree-center hint should be replaced");
        assert!(!expanded.contains(&trunk));

        let tank_from = (trunk.0 - 48.0, trunk.1);
        let tank_to = (trunk.0 + 48.0, trunk.1);
        let tank_detour =
            tree_detour_between(&map, &occupancy, EntityKind::Tank, tank_from, tank_to)
                .expect("oriented vehicle should use its bounding radius for trunk detours");
        assert!(!tank_detour.is_empty());

        let second_trunk = map.tile_center(6, 5);
        map.doodads.push(MapDoodad {
            id: 2,
            type_id: "tree.pine".to_string(),
            x: second_trunk.0 as u32,
            y: second_trunk.1 as u32,
            color: None,
        });
        let occupancy = Occupancy::build(&map, &EntityStore::new());
        let beyond = map.tile_center(7, 5);
        let expanded = expand_reverse_waypoints(
            &map,
            &occupancy,
            EntityKind::Rifleman,
            map.tile_center(4, 5),
            vec![beyond, second_trunk, trunk],
        )
        .expect("consecutive blocked tree-center hints should be replaced");
        assert!(!expanded.contains(&trunk));
        assert!(!expanded.contains(&second_trunk));

        let mut dense_map = map.clone();
        dense_map.doodads = (1..=9)
            .map(|id| MapDoodad {
                id,
                type_id: "tree.alder".to_string(),
                x: trunk.0 as u32,
                y: trunk.1 as u32,
                color: None,
            })
            .collect();
        let dense_occupancy = Occupancy::build(&dense_map, &EntityStore::new());
        assert!(expand_reverse_waypoints(
            &dense_map,
            &dense_occupancy,
            EntityKind::Rifleman,
            from,
            vec![to],
        )
        .is_none());
    }
}
