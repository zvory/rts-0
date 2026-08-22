use super::*;
use crate::config;
use crate::game::services::occupancy::footprint_center;
use crate::protocol::{terrain, MapDoodad};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

fn rich_map() -> Map {
    let size = 18;
    let mut map = Map {
        width: size,
        height: size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        elevation: vec![0; (size * size) as usize],
        ..Default::default()
    };
    for tile in [(4, 4), (4, 5), (9, 8), (12, 3)] {
        let index = map.index(tile.0, tile.1);
        map.terrain[index] = terrain::ROCK;
    }
    map.no_vehicle_tiles = vec![(7, 7), (7, 8)];
    map.slow_movement_tiles = vec![(5, 9), (6, 9), (7, 9)];
    let trunk = map.tile_center(10, 10);
    map.doodads.push(MapDoodad {
        id: 1,
        type_id: "tree.oak".to_string(),
        x: trunk.0 as u32,
        y: trunk.1 as u32,
        color: None,
    });
    map
}

fn assert_matches_reference(
    graph: &mut PathGraph,
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    route_shape: RouteShape,
) {
    let profile = RoutingProfile::for_request(kind, 0, route_shape, RoutePolicy::LegacyShape);
    let reference = terrain_passability(profile, map, occupancy);
    let table = graph.view(
        map,
        occupancy,
        kind,
        0,
        route_shape,
        RoutePolicy::LegacyShape,
    );
    for ty in -1..=map.height as i32 {
        for tx in -1..=map.width as i32 {
            assert_eq!(
                table.passable(tx, ty),
                reference.passable(tx, ty),
                "standability mismatch for {profile:?} at ({tx}, {ty})"
            );
            if map.in_bounds(tx, ty) {
                for &(dx, dy, step) in &DIRECTIONS {
                    assert_eq!(
                        table.edge_cost(tx, ty, dx, dy, step),
                        reference.edge_cost(tx, ty, dx, dy, step),
                        "edge mismatch for {profile:?} at ({tx}, {ty}) -> ({dx}, {dy})"
                    );
                }
            }
        }
    }
}

#[test]
fn precomputed_edges_match_rich_reference_for_every_live_profile() {
    let map = rich_map();
    let mut entities = EntityStore::new();
    let depot = footprint_center(&map, EntityKind::Depot, 7, 11);
    entities
        .spawn_building(1, EntityKind::Depot, depot.0, depot.1, true)
        .expect("depot should spawn");
    let trap = footprint_center(&map, EntityKind::TankTrap, 13, 12);
    entities
        .spawn_building(2, EntityKind::TankTrap, trap.0, trap.1, true)
        .expect("tank trap should spawn");
    let occupancy = Occupancy::build(&map, &entities);
    let mut graph = PathGraph::default();

    assert_matches_reference(
        &mut graph,
        &map,
        &occupancy,
        EntityKind::Rifleman,
        RouteShape::Normal,
    );
    assert_matches_reference(
        &mut graph,
        &map,
        &occupancy,
        EntityKind::Tank,
        RouteShape::Normal,
    );
    assert_matches_reference(
        &mut graph,
        &map,
        &occupancy,
        EntityKind::Tank,
        RouteShape::VehicleClearance,
    );

    let (base_bytes, dynamic_bytes) = graph.retained_bytes();
    assert_eq!(base_bytes, 3 * 18 * 18 * 33);
    assert!(
        (3 * 18 * 18 * 33..=3 * 18 * 18 * 34).contains(&dynamic_bytes),
        "unexpected dynamic bytes: {dynamic_bytes}"
    );
}

#[test]
fn precomputed_weighted_edges_match_vehicle_reference_profiles() {
    let map = rich_map();
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let mut graph = PathGraph::default();
    for kind in [
        EntityKind::ScoutCar,
        EntityKind::Tank,
        EntityKind::AntiTankGun,
    ] {
        let radius_tiles = config::unit_radius_tiles(kind);
        let profile = RoutingProfile::for_request(
            kind,
            radius_tiles,
            RouteShape::VehicleClearance,
            RoutePolicy::FastestTerrainTime,
        );
        let reference = terrain_passability(profile, &map, &occupancy);
        let table = graph.view(
            &map,
            &occupancy,
            kind,
            radius_tiles,
            RouteShape::VehicleClearance,
            RoutePolicy::FastestTerrainTime,
        );
        for ty in 0..map.height as i32 {
            for tx in 0..map.width as i32 {
                assert_eq!(
                    table.passable(tx, ty),
                    reference.passable(tx, ty),
                    "{kind:?}"
                );
                for &(dx, dy, step) in &DIRECTIONS {
                    assert_eq!(
                        table.edge_cost(tx, ty, dx, dy, step),
                        reference.edge_cost(tx, ty, dx, dy, step),
                        "{kind:?} at ({tx}, {ty}) -> ({dx}, {dy})"
                    );
                }
            }
        }
    }
}

#[test]
fn local_updates_match_close_overlap_partial_and_full_reopen_reference() {
    let map = rich_map();
    let mut entities = EntityStore::new();
    let mut graph = PathGraph::default();
    let empty = Occupancy::build(&map, &entities);
    assert_matches_reference(
        &mut graph,
        &map,
        &empty,
        EntityKind::Tank,
        RouteShape::VehicleClearance,
    );
    let initial_generation = graph.generation();

    let center = footprint_center(&map, EntityKind::Depot, 8, 8);
    let first = entities
        .spawn_building(1, EntityKind::Depot, center.0, center.1, true)
        .expect("first depot should spawn");
    let closed = Occupancy::build(&map, &entities);
    assert_matches_reference(
        &mut graph,
        &map,
        &closed,
        EntityKind::Tank,
        RouteShape::VehicleClearance,
    );
    assert!(graph.generation() > initial_generation);
    assert!(graph
        .last_update_origins()
        .is_some_and(|count| count < 18 * 18));

    let second = entities
        .spawn_building(2, EntityKind::Depot, center.0, center.1, true)
        .expect("overlapping depot should spawn");
    let overlapping = Occupancy::build(&map, &entities);
    assert_matches_reference(
        &mut graph,
        &map,
        &overlapping,
        EntityKind::Tank,
        RouteShape::VehicleClearance,
    );

    entities.remove(first);
    let partial_reopen = Occupancy::build(&map, &entities);
    assert_matches_reference(
        &mut graph,
        &map,
        &partial_reopen,
        EntityKind::Tank,
        RouteShape::VehicleClearance,
    );
    assert!(!partial_reopen.passable_for_kind(8, 8, EntityKind::Tank));

    entities.remove(second);
    let reopened = Occupancy::build(&map, &entities);
    assert_matches_reference(
        &mut graph,
        &map,
        &reopened,
        EntityKind::Tank,
        RouteShape::VehicleClearance,
    );
    assert!(reopened.passable_for_kind(8, 8, EntityKind::Tank));
}

#[test]
fn base_and_empty_dynamic_tables_share_every_edge_page() {
    let map = rich_map();
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let mut graph = PathGraph::default();

    graph.view(
        &map,
        &occupancy,
        EntityKind::Tank,
        0,
        RouteShape::VehicleClearance,
        RoutePolicy::LegacyShape,
    );

    let profile = &graph.profiles[0];
    assert_eq!(profile.base.pages.len(), profile.dynamic.pages.len());
    assert!(profile
        .base
        .pages
        .iter()
        .zip(&profile.dynamic.pages)
        .all(|(base, dynamic)| Arc::ptr_eq(base, dynamic)));
}

#[test]
fn graph_search_matches_rich_reference_for_profiles_caps_and_fallbacks() {
    let map = rich_map();
    let mut entities = EntityStore::new();
    let depot = footprint_center(&map, EntityKind::Depot, 8, 8);
    entities
        .spawn_building(1, EntityKind::Depot, depot.0, depot.1, true)
        .expect("depot should spawn");
    let occupancy = Occupancy::build(&map, &entities);
    let profiles = [
        (EntityKind::Rifleman, RouteShape::Normal),
        (EntityKind::ScoutCar, RouteShape::Normal),
        (EntityKind::Tank, RouteShape::VehicleClearance),
    ];
    let caps = [0, 1, 2, 8, 64, 4_096, 32_768];
    let mut graph = PathGraph::default();
    for (query_index, start, goal) in [
        (0usize, (1, 1), (16, 16)),
        (1, (2, 14), (15, 2)),
        (2, (5, 9), (12, 9)),
        (3, (7, 8), (8, 8)),
        (4, (10, 10), (4, 4)),
    ] {
        for &(kind, route_shape) in &profiles {
            for &cap in &caps {
                let profile =
                    RoutingProfile::for_request(kind, 0, route_shape, RoutePolicy::LegacyShape);
                let reference = terrain_passability(profile, &map, &occupancy);
                let table = graph.view(
                    &map,
                    &occupancy,
                    kind,
                    0,
                    route_shape,
                    RoutePolicy::LegacyShape,
                );
                let mut reference_scratch = crate::game::pathfinding::SearchScratch::default();
                let mut table_scratch = crate::game::pathfinding::SearchScratch::default();
                let expected = crate::game::pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                    &reference,
                    start,
                    goal,
                    cap,
                    route_shape.turn_penalty(RoutePolicy::LegacyShape),
                    &mut reference_scratch,
                );
                let actual = crate::game::pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                    &table,
                    start,
                    goal,
                    cap,
                    route_shape.turn_penalty(RoutePolicy::LegacyShape),
                    &mut table_scratch,
                );
                assert_eq!(
                    actual, expected,
                    "query {query_index}, {kind:?}, {route_shape:?}, cap {cap}"
                );
            }
        }
    }
}

#[test]
fn fastest_terrain_raw_graph_cost_equals_dijkstra() {
    let mut map = rich_map();
    for tx in 1..17 {
        let index = map.index(tx, 12);
        map.terrain[index] = terrain::ROAD_HORIZONTAL;
    }
    for ty in 1..17 {
        let index = map.index(13, ty);
        map.elevation[index] = if ty < 9 { 4 } else { 1 };
    }
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let mut graph = PathGraph::default();
    for (start, goal) in [((1, 1), (16, 16)), ((16, 16), (1, 1)), ((2, 14), (15, 2))] {
        let pass = graph.view(
            &map,
            &occupancy,
            EntityKind::Rifleman,
            config::unit_radius_tiles(EntityKind::Rifleman),
            RouteShape::Normal,
            RoutePolicy::FastestTerrainTime,
        );
        let path = crate::game::pathfinding::find_path_with_budget_and_turn_cost(
            &pass,
            start.0,
            start.1,
            goal.0,
            goal.1,
            (map.width * map.height) as usize,
            0,
        );
        assert_eq!(path.last().copied(), Some(goal));
        let mut actual_cost = 0_u64;
        let mut from = start;
        for to in path {
            let dx = to.0 - from.0;
            let dy = to.1 - from.1;
            let base = if dx != 0 && dy != 0 { 14 } else { 10 };
            actual_cost += u64::from(
                pass.edge_cost(from.0, from.1, dx, dy, base)
                    .expect("production path edge should be legal"),
            );
            from = to;
        }
        assert_eq!(Some(actual_cost), dijkstra_cost(&pass, start, goal));
    }
}

#[test]
fn fastest_terrain_vehicle_direction_state_cost_equals_dijkstra() {
    let mut map = rich_map();
    for tx in 1..17 {
        let index = map.index(tx, 12);
        map.terrain[index] = terrain::ROAD_HORIZONTAL;
    }
    for ty in 1..17 {
        let index = map.index(13, ty);
        map.elevation[index] = if ty < 9 { 4 } else { 1 };
    }
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let mut graph = PathGraph::default();
    for kind in [
        EntityKind::ScoutCar,
        EntityKind::Tank,
        EntityKind::AntiTankGun,
    ] {
        for (start, goal) in [((1, 1), (16, 16)), ((16, 16), (1, 1)), ((2, 14), (15, 2))] {
            let route_shape = RouteShape::VehicleClearance;
            let turn_penalty = route_shape.turn_penalty(RoutePolicy::FastestTerrainTime);
            let pass = graph.view(
                &map,
                &occupancy,
                kind,
                config::unit_radius_tiles(kind),
                route_shape,
                RoutePolicy::FastestTerrainTime,
            );
            let path = crate::game::pathfinding::find_path_with_budget_and_turn_cost(
                &pass,
                start.0,
                start.1,
                goal.0,
                goal.1,
                (map.width * map.height * 9) as usize,
                turn_penalty,
            );
            assert_eq!(path.last().copied(), Some(goal), "{kind:?}");
            let mut actual_cost = 0_u64;
            let mut from = start;
            let mut incoming = None;
            for to in path {
                let dx = to.0 - from.0;
                let dy = to.1 - from.1;
                let direction = EdgeTable::direction(dx, dy).expect("path step direction");
                let base = if dx != 0 && dy != 0 { 14 } else { 10 };
                actual_cost += u64::from(
                    pass.edge_cost(from.0, from.1, dx, dy, base)
                        .expect("production path edge should be legal"),
                );
                if incoming.is_some_and(|previous| previous != direction) {
                    actual_cost += u64::from(turn_penalty);
                }
                incoming = Some(direction);
                from = to;
            }
            assert_eq!(
                Some(actual_cost),
                direction_state_dijkstra_cost(&pass, start, goal, turn_penalty),
                "{kind:?} {start:?}->{goal:?}"
            );
        }
    }
}

fn direction_state_dijkstra_cost<P: Passability>(
    pass: &P,
    start: (i32, i32),
    goal: (i32, i32),
    turn_penalty: u32,
) -> Option<u64> {
    const START_DIRECTION: usize = DIRECTIONS.len();
    let (width, height) = pass.dimensions();
    let states_per_tile = DIRECTIONS.len() + 1;
    let mut distance = vec![u64::MAX; width.saturating_mul(height) as usize * states_per_tile];
    let index = |tx: i32, ty: i32, direction: usize| {
        ((ty as u32 * width + tx as u32) as usize) * states_per_tile + direction
    };
    distance[index(start.0, start.1, START_DIRECTION)] = 0;
    let mut queue = BinaryHeap::from([Reverse((0_u64, start.1, start.0, START_DIRECTION))]);
    while let Some(Reverse((cost, ty, tx, incoming))) = queue.pop() {
        if (tx, ty) == goal {
            return Some(cost);
        }
        if cost != distance[index(tx, ty, incoming)] {
            continue;
        }
        for (direction, &(dx, dy, base)) in DIRECTIONS.iter().enumerate() {
            let Some(edge_cost) = pass.edge_cost(tx, ty, dx, dy, base) else {
                continue;
            };
            let next = (tx + dx, ty + dy);
            let turn_cost = if incoming != START_DIRECTION && incoming != direction {
                turn_penalty
            } else {
                0
            };
            let next_cost = cost
                .saturating_add(u64::from(edge_cost))
                .saturating_add(u64::from(turn_cost));
            let next_index = index(next.0, next.1, direction);
            if next_cost < distance[next_index] {
                distance[next_index] = next_cost;
                queue.push(Reverse((next_cost, next.1, next.0, direction)));
            }
        }
    }
    None
}

fn dijkstra_cost<P: Passability>(pass: &P, start: (i32, i32), goal: (i32, i32)) -> Option<u64> {
    let (width, height) = pass.dimensions();
    let mut distance = vec![u64::MAX; width.saturating_mul(height) as usize];
    let index = |tx: i32, ty: i32| (ty as u32 * width + tx as u32) as usize;
    distance[index(start.0, start.1)] = 0;
    let mut queue = BinaryHeap::from([Reverse((0_u64, start.1, start.0))]);
    while let Some(Reverse((cost, ty, tx))) = queue.pop() {
        if (tx, ty) == goal {
            return Some(cost);
        }
        if cost != distance[index(tx, ty)] {
            continue;
        }
        for &(dx, dy, base) in &DIRECTIONS {
            let Some(edge_cost) = pass.edge_cost(tx, ty, dx, dy, base) else {
                continue;
            };
            let next = (tx + dx, ty + dy);
            let next_cost = cost.saturating_add(u64::from(edge_cost));
            let next_index = index(next.0, next.1);
            if next_cost < distance[next_index] {
                distance[next_index] = next_cost;
                queue.push(Reverse((next_cost, next.1, next.0)));
            }
        }
    }
    None
}

fn fold_path(mut hash: u64, path: &[(i32, i32)]) -> u64 {
    for &(tx, ty) in path {
        hash ^= u64::from(tx as u32) << 32 | u64::from(ty as u32);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[test]
#[ignore = "release-only paired rich-edge and local-update evidence"]
fn phase2_release_rich_edge_benchmark() {
    assert!(!cfg!(debug_assertions), "benchmark must use --release");
    let map = Map::generate(4, 0xFA57_0002);
    let mut entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let profiles = [
        (EntityKind::Rifleman, RouteShape::Normal),
        (EntityKind::ScoutCar, RouteShape::Normal),
        (EntityKind::Tank, RouteShape::VehicleClearance),
    ];
    let width = map.width as i32;
    let height = map.height as i32;
    let queries = (0..80)
        .flat_map(|index| {
            profiles.map(move |profile| {
                let start = (
                    2 + (index * 17 % (width as usize - 4)) as i32,
                    2 + (index * 29 % (height as usize - 4)) as i32,
                );
                let goal = (
                    2 + (index * 43 + 31) as i32 % (width - 4),
                    2 + (index * 61 + 47) as i32 % (height - 4),
                );
                (profile.0, profile.1, start, goal)
            })
        })
        .collect::<Vec<_>>();

    let run_reference = || {
        let started = Instant::now();
        let mut scratch = crate::game::pathfinding::SearchScratch::default();
        let mut hash = 0xcbf2_9ce4_8422_2325;
        let mut expanded = 0usize;
        for &(kind, route_shape, start, goal) in &queries {
            let pass = TerrainPassability {
                map: &map,
                occupancy: &occupancy,
                kind,
                radius_tiles: 0,
                route_shape,
                policy: RoutePolicy::LegacyShape,
                avoid_diagonal_pinch: uses_oriented_vehicle_body(kind),
            };
            let (path, count, _) = crate::game::pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                &pass,
                start,
                goal,
                32_768,
                route_shape.turn_penalty(RoutePolicy::LegacyShape),
                &mut scratch,
            );
            hash = fold_path(hash, &path);
            for waypoint in crate::game::pathfinding::to_world_waypoints(&path) {
                hash ^= u64::from(waypoint.0.to_bits()) << 32 | u64::from(waypoint.1.to_bits());
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            expanded += count;
        }
        (started.elapsed().as_nanos(), hash, expanded)
    };

    let mut graph = PathGraph::default();
    for &(kind, route_shape) in &profiles {
        let _ = graph.view(
            &map,
            &occupancy,
            kind,
            0,
            route_shape,
            RoutePolicy::LegacyShape,
        );
    }
    let (base_bytes, dynamic_bytes) = graph.retained_bytes();
    let initialization_ns = graph.initialization_ns();
    let run_graph = |graph: &mut PathGraph| {
        let started = Instant::now();
        let mut scratch = crate::game::pathfinding::SearchScratch::default();
        let mut hash = 0xcbf2_9ce4_8422_2325;
        let mut expanded = 0usize;
        for &(kind, route_shape, start, goal) in &queries {
            let pass = graph.view(
                &map,
                &occupancy,
                kind,
                0,
                route_shape,
                RoutePolicy::LegacyShape,
            );
            let (path, count, _) = crate::game::pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                &pass,
                start,
                goal,
                32_768,
                route_shape.turn_penalty(RoutePolicy::LegacyShape),
                &mut scratch,
            );
            hash = fold_path(hash, &path);
            for waypoint in crate::game::pathfinding::to_world_waypoints(&path) {
                hash ^= u64::from(waypoint.0.to_bits()) << 32 | u64::from(waypoint.1.to_bits());
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            expanded += count;
        }
        (started.elapsed().as_nanos(), hash, expanded)
    };

    // Warm code and allocations before the reported pair.
    let _ = run_reference();
    let _ = run_graph(&mut graph);
    let candidate_first = std::env::var("RTS_PATHING_BENCH_ORDER").as_deref() == Ok("candidate");
    let (reference, candidate) = if candidate_first {
        let candidate = run_graph(&mut graph);
        (run_reference(), candidate)
    } else {
        let reference = run_reference();
        (reference, run_graph(&mut graph))
    };
    assert_eq!((candidate.1, candidate.2), (reference.1, reference.2));

    let center = footprint_center(&map, EntityKind::TankTrap, 20, 20);
    let building = entities
        .spawn_building(1, EntityKind::TankTrap, center.0, center.1, true)
        .expect("benchmark trap should spawn");
    for iteration in 0..101u32 {
        let tx = 20 + iteration % 10;
        let ty = 20 + iteration / 10;
        let position = footprint_center(&map, EntityKind::TankTrap, tx, ty);
        entities
            .get_mut(building)
            .expect("benchmark trap should remain")
            .set_position(position.0, position.1);
        let changed = Occupancy::build(&map, &entities);
        for &(kind, route_shape) in &profiles {
            let _ = graph.view(
                &map,
                &changed,
                kind,
                0,
                route_shape,
                RoutePolicy::LegacyShape,
            );
        }
    }
    let mut updates = graph.local_update_ns().to_vec();
    updates.sort_unstable();
    let percentile = |percent: usize| updates[(updates.len() - 1) * percent / 100];
    println!(
        "PATHING_PHASE2_BENCH_JSON={{\"requests\":{},\"semanticHash\":\"{:016x}\",\"expanded\":{},\"referenceElapsedNs\":{},\"candidateElapsedNs\":{},\"initializationNs\":{},\"baseBytes\":{},\"dynamicBytes\":{},\"updates\":{},\"updateP50Ns\":{},\"updateP95Ns\":{},\"updateWorstNs\":{}}}",
        queries.len(),
        candidate.1,
        candidate.2,
        reference.0,
        candidate.0,
        initialization_ns,
        base_bytes,
        dynamic_bytes,
        updates.len(),
        percentile(50),
        percentile(95),
        updates[updates.len() - 1],
    );
}

#[test]
#[ignore = "release-only paired weighted infantry full-path evidence"]
fn phase3_release_infantry_terrain_benchmark() {
    assert!(!cfg!(debug_assertions), "benchmark must use --release");
    let map = Map::generate(4, 0xFA57_0003);
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let width = map.width as i32;
    let height = map.height as i32;
    let queries = (0..240)
        .map(|index| {
            let start = (
                2 + (index * 17 % (width as usize - 4)) as i32,
                2 + (index * 29 % (height as usize - 4)) as i32,
            );
            let goal = (
                2 + (index * 43 + 31) as i32 % (width - 4),
                2 + (index * 61 + 47) as i32 % (height - 4),
            );
            (start, goal)
        })
        .collect::<Vec<_>>();

    let run_reference = || {
        let started = Instant::now();
        let mut scratch = crate::game::pathfinding::SearchScratch::default();
        let mut hash = 0xcbf2_9ce4_8422_2325;
        let mut expanded = 0usize;
        for &(start, goal) in &queries {
            let pass = TerrainPassability {
                map: &map,
                occupancy: &occupancy,
                kind: EntityKind::Rifleman,
                radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
                route_shape: RouteShape::Normal,
                policy: RoutePolicy::LegacyShape,
                avoid_diagonal_pinch: false,
            };
            let (path, count, _) = crate::game::pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                &pass,
                start,
                goal,
                32_768,
                0,
                &mut scratch,
            );
            let start_world = map.tile_center(start.0 as u32, start.1 as u32);
            let goal_world = map.tile_center(goal.0 as u32, goal.1 as u32);
            let waypoints = super::super::route_finalize::finalize_reverse_waypoints_or_raw(
                &map,
                &occupancy,
                EntityKind::Rifleman,
                start_world,
                goal_world,
                super::super::route_finalize::RouteFinalizationMode::new(
                    RouteShape::Normal,
                    RoutePolicy::LegacyShape,
                ),
                crate::game::pathfinding::to_world_waypoints(&path),
            );
            for waypoint in waypoints {
                hash ^= u64::from(waypoint.0.to_bits()) << 32 | u64::from(waypoint.1.to_bits());
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            expanded += count;
        }
        (started.elapsed().as_nanos(), hash, expanded)
    };

    let mut graph = PathGraph::default();
    let _ = graph.view(
        &map,
        &occupancy,
        EntityKind::Rifleman,
        config::unit_radius_tiles(EntityKind::Rifleman),
        RouteShape::Normal,
        RoutePolicy::FastestTerrainTime,
    );
    let run_candidate = |graph: &mut PathGraph| {
        let started = Instant::now();
        let mut finalization_ns = 0_u128;
        let mut scratch = crate::game::pathfinding::SearchScratch::default();
        let mut hash = 0xcbf2_9ce4_8422_2325;
        let mut expanded = 0usize;
        for &(start, goal) in &queries {
            let pass = graph.view(
                &map,
                &occupancy,
                EntityKind::Rifleman,
                config::unit_radius_tiles(EntityKind::Rifleman),
                RouteShape::Normal,
                RoutePolicy::FastestTerrainTime,
            );
            let (path, count, _) = crate::game::pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                &pass,
                start,
                goal,
                32_768,
                0,
                &mut scratch,
            );
            let start_world = map.tile_center(start.0 as u32, start.1 as u32);
            let goal_world = map.tile_center(goal.0 as u32, goal.1 as u32);
            let finalization_started = Instant::now();
            let waypoints = super::super::route_finalize::finalize_reverse_waypoints_or_raw(
                &map,
                &occupancy,
                EntityKind::Rifleman,
                start_world,
                goal_world,
                super::super::route_finalize::RouteFinalizationMode::new(
                    RouteShape::Normal,
                    RoutePolicy::FastestTerrainTime,
                ),
                crate::game::pathfinding::to_world_waypoints(&path),
            );
            finalization_ns =
                finalization_ns.saturating_add(finalization_started.elapsed().as_nanos());
            for waypoint in waypoints {
                hash ^= u64::from(waypoint.0.to_bits()) << 32 | u64::from(waypoint.1.to_bits());
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            expanded += count;
        }
        (
            started.elapsed().as_nanos(),
            finalization_ns,
            hash,
            expanded,
        )
    };

    let _ = run_reference();
    let _ = run_candidate(&mut graph);
    let candidate_first = std::env::var("RTS_PATHING_BENCH_ORDER").as_deref() == Ok("candidate");
    let (reference, candidate) = if candidate_first {
        let candidate = run_candidate(&mut graph);
        (run_reference(), candidate)
    } else {
        let reference = run_reference();
        (reference, run_candidate(&mut graph))
    };
    println!(
        "PATHING_PHASE3_BENCH_JSON={{\"requests\":{},\"referenceElapsedNs\":{},\"candidateElapsedNs\":{},\"finalizationNs\":{},\"referenceHash\":\"{:016x}\",\"candidateHash\":\"{:016x}\",\"referenceExpanded\":{},\"candidateExpanded\":{}}}",
        queries.len(),
        reference.0,
        candidate.0,
        candidate.1,
        reference.1,
        candidate.2,
        reference.2,
        candidate.3,
    );
}

#[test]
#[ignore = "release-only paired all-profile terrain full-path evidence"]
fn phase4_release_all_profile_terrain_benchmark() {
    assert!(!cfg!(debug_assertions), "benchmark must use --release");
    let map = Map::generate(4, 0xFA57_0004);
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let profiles = [
        (EntityKind::Rifleman, RouteShape::Normal),
        (EntityKind::ScoutCar, RouteShape::VehicleClearance),
        (EntityKind::Tank, RouteShape::VehicleClearance),
    ];
    let width = map.width as i32;
    let height = map.height as i32;
    let queries = (0..80)
        .flat_map(|index| {
            profiles.map(move |profile| {
                let start = (
                    2 + (index * 17 % (width as usize - 4)) as i32,
                    2 + (index * 29 % (height as usize - 4)) as i32,
                );
                let goal = (
                    2 + (index * 43 + 31) as i32 % (width - 4),
                    2 + (index * 61 + 47) as i32 % (height - 4),
                );
                (profile.0, profile.1, start, goal)
            })
        })
        .collect::<Vec<_>>();
    const REPETITIONS: usize = 4;
    const WARM_REPETITIONS: usize = 64;

    let run_reference = || {
        let started = Instant::now();
        let mut scratch = crate::game::pathfinding::SearchScratch::default();
        let mut hash = 0xcbf2_9ce4_8422_2325;
        let mut expanded = 0usize;
        for &(kind, route_shape, start, goal) in
            queries.iter().cycle().take(queries.len() * REPETITIONS)
        {
            let pass = TerrainPassability {
                map: &map,
                occupancy: &occupancy,
                kind,
                radius_tiles: config::unit_radius_tiles(kind),
                route_shape,
                policy: RoutePolicy::LegacyShape,
                avoid_diagonal_pinch: uses_oriented_vehicle_body(kind),
            };
            let (mut path, count, _) = crate::game::pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                &pass,
                start,
                goal,
                32_768,
                route_shape.turn_penalty(RoutePolicy::LegacyShape),
                &mut scratch,
            );
            if crate::game::entity::uses_pivot_vehicle_movement(kind) {
                path =
                    super::super::expand_vehicle_diagonal_steps_to_l_waypoints(start, &path, &pass);
            }
            let waypoints = super::super::route_finalize::finalize_reverse_waypoints_or_raw(
                &map,
                &occupancy,
                kind,
                map.tile_center(start.0 as u32, start.1 as u32),
                map.tile_center(goal.0 as u32, goal.1 as u32),
                super::super::route_finalize::RouteFinalizationMode::new(
                    route_shape,
                    RoutePolicy::LegacyShape,
                ),
                crate::game::pathfinding::to_world_waypoints(&path),
            );
            for waypoint in waypoints {
                hash ^= u64::from(waypoint.0.to_bits()) << 32 | u64::from(waypoint.1.to_bits());
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            expanded += count;
        }
        (started.elapsed().as_nanos(), hash, expanded)
    };

    let mut graph = PathGraph::default();
    for &(kind, route_shape) in &profiles {
        let _ = graph.view(
            &map,
            &occupancy,
            kind,
            config::unit_radius_tiles(kind),
            route_shape,
            RoutePolicy::FastestTerrainTime,
        );
    }
    let (base_bytes, dynamic_bytes) = graph.retained_bytes();
    let initialization_ns = graph.initialization_ns();
    let run_candidate = |graph: &mut PathGraph| {
        let started = Instant::now();
        let mut finalization_ns = 0_u128;
        let mut scratch = crate::game::pathfinding::SearchScratch::default();
        let mut hash = 0xcbf2_9ce4_8422_2325;
        let mut expanded = 0usize;
        for &(kind, route_shape, start, goal) in
            queries.iter().cycle().take(queries.len() * REPETITIONS)
        {
            let pass = graph.view(
                &map,
                &occupancy,
                kind,
                config::unit_radius_tiles(kind),
                route_shape,
                RoutePolicy::FastestTerrainTime,
            );
            let (mut path, count, _) = crate::game::pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                &pass,
                start,
                goal,
                32_768,
                route_shape.turn_penalty(RoutePolicy::FastestTerrainTime),
                &mut scratch,
            );
            if crate::game::entity::uses_pivot_vehicle_movement(kind) {
                path =
                    super::super::expand_vehicle_diagonal_steps_to_l_waypoints(start, &path, &pass);
            }
            let finalization_started = Instant::now();
            let waypoints = super::super::route_finalize::finalize_reverse_waypoints_or_raw(
                &map,
                &occupancy,
                kind,
                map.tile_center(start.0 as u32, start.1 as u32),
                map.tile_center(goal.0 as u32, goal.1 as u32),
                super::super::route_finalize::RouteFinalizationMode::new(
                    route_shape,
                    RoutePolicy::FastestTerrainTime,
                ),
                crate::game::pathfinding::to_world_waypoints(&path),
            );
            finalization_ns =
                finalization_ns.saturating_add(finalization_started.elapsed().as_nanos());
            for waypoint in waypoints {
                hash ^= u64::from(waypoint.0.to_bits()) << 32 | u64::from(waypoint.1.to_bits());
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            expanded += count;
        }
        (
            started.elapsed().as_nanos(),
            finalization_ns,
            hash,
            expanded,
        )
    };

    let _ = run_reference();
    let _ = run_candidate(&mut graph);
    let candidate_first = std::env::var("RTS_PATHING_BENCH_ORDER").as_deref() == Ok("candidate");
    let (reference, candidate) = if candidate_first {
        let candidate = run_candidate(&mut graph);
        (run_reference(), candidate)
    } else {
        let reference = run_reference();
        (reference, run_candidate(&mut graph))
    };
    let run_warm = |policy: RoutePolicy| {
        let mut service = super::super::PathingService::new(32_768, 512);
        let mut execute = |measure: bool| {
            let started = Instant::now();
            let mut hash = 0xcbf2_9ce4_8422_2325;
            for &(kind, route_shape, start, goal) in queries
                .iter()
                .cycle()
                .take(queries.len() * WARM_REPETITIONS)
            {
                let request = super::super::PathRequest {
                    kind,
                    start,
                    goal,
                    radius_tiles: config::unit_radius_tiles(kind),
                    route_shape,
                    policy,
                    budget: None,
                };
                let start_world = map.tile_center(start.0 as u32, start.1 as u32);
                let goal_world = map.tile_center(goal.0 as u32, goal.1 as u32);
                let super::super::PathingRequestOutcome::Resolved {
                    path: waypoints,
                    diagnostics,
                } = service.request_finalized_with_diagnostics(
                    &map,
                    &occupancy,
                    request,
                    (start_world, goal_world),
                    None,
                    true,
                )
                else {
                    panic!("benchmark request unexpectedly deferred");
                };
                if measure {
                    assert_eq!(diagnostics.cache_status, super::super::PathCacheStatus::Hit);
                }
                for waypoint in waypoints {
                    hash ^= u64::from(waypoint.0.to_bits()) << 32 | u64::from(waypoint.1.to_bits());
                    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
            (started.elapsed().as_nanos(), hash)
        };
        let _ = execute(false);
        execute(true)
    };
    let (warm_reference, warm_candidate) = if candidate_first {
        let candidate = run_warm(RoutePolicy::FastestTerrainTime);
        (run_warm(RoutePolicy::LegacyShape), candidate)
    } else {
        let reference = run_warm(RoutePolicy::LegacyShape);
        (reference, run_warm(RoutePolicy::FastestTerrainTime))
    };
    println!(
        "PATHING_PHASE4_BENCH_JSON={{\"requests\":{},\"warmRequests\":{},\"referenceElapsedNs\":{},\"candidateElapsedNs\":{},\"finalizationNs\":{},\"referenceHash\":\"{:016x}\",\"candidateHash\":\"{:016x}\",\"referenceExpanded\":{},\"candidateExpanded\":{},\"warmReferenceElapsedNs\":{},\"warmCandidateElapsedNs\":{},\"warmReferenceHash\":\"{:016x}\",\"warmCandidateHash\":\"{:016x}\",\"initializationNs\":{},\"baseBytes\":{},\"dynamicBytes\":{}}}",
        queries.len() * REPETITIONS,
        queries.len() * WARM_REPETITIONS,
        reference.0,
        candidate.0,
        candidate.1,
        reference.1,
        candidate.2,
        reference.2,
        candidate.3,
        warm_reference.0,
        warm_candidate.0,
        warm_reference.1,
        warm_candidate.1,
        initialization_ns,
        base_bytes,
        dynamic_bytes,
    );
}
