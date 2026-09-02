use super::*;
use crate::game::entity::EntityStore;
use crate::protocol::{terrain, MapDoodad};

#[test]
fn scout_car_finalization_applies_live_three_tile_segment_limit() {
    let map = Map {
        width: 16,
        height: 16,
        terrain: vec![terrain::GRASS; 16 * 16],
        ..Default::default()
    };
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let start = map.tile_center(2, 8);
    let goal = map.tile_center(12, 8);
    let tile_path: Vec<_> = (3..=12).map(|x| (x, 8)).collect();
    let raw = crate::game::pathfinding::to_world_waypoints(&tile_path);
    let finalized = finalize_reverse_waypoints(
        &map,
        &occupancy,
        EntityKind::ScoutCar,
        start,
        goal,
        RouteFinalizationMode::new(
            RouteShape::VehicleClearance,
            RoutePolicy::LegacyShape,
        ),
        raw.clone(),
    )
    .expect("open route should finalize");

    assert!(finalized.len() < raw.len());
    let mut forward = finalized;
    forward.reverse();
    let mut from = start;
    for to in forward {
        assert!(distance_between(from, to) <= SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX + 0.01);
        from = to;
    }
    assert_eq!(from, goal);
}

#[test]
fn every_oriented_vehicle_keeps_raw_route_when_tree_refinement_is_bounded_out() {
    let mut map = Map {
        width: 16,
        height: 16,
        terrain: vec![terrain::GRASS; 16 * 16],
        ..Default::default()
    };
    let trunk = map.tile_center(8, 8);
    map.doodads = (1..=9)
        .map(|id| MapDoodad {
            id,
            type_id: "tree.alder".to_string(),
            x: trunk.0 as u32,
            y: trunk.1 as u32,
            color: None,
        })
        .collect();
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let start = map.tile_center(6, 8);
    let goal = map.tile_center(10, 8);
    let raw = vec![goal];

    for kind in EntityKind::ALL
        .into_iter()
        .filter(|kind| uses_oriented_vehicle_body(*kind))
    {
        assert!(finalize_reverse_waypoints(
            &map,
            &occupancy,
            kind,
            start,
            goal,
            RouteFinalizationMode::new(
                RouteShape::VehicleClearance,
                RoutePolicy::LegacyShape,
            ),
            raw.clone(),
        )
        .is_none());
        assert_eq!(
            finalize_reverse_waypoints_or_raw(
                &map,
                &occupancy,
                kind,
                start,
                goal,
                RouteFinalizationMode::new(
                    RouteShape::VehicleClearance,
                    RoutePolicy::LegacyShape,
                ),
                raw.clone(),
            ),
            raw,
            "{kind} lost its resolved tile route"
        );
    }
}

#[test]
fn fastest_terrain_finalizer_keeps_a_faster_offset_road_anchor() {
    let mut map = Map {
        width: 20,
        height: 20,
        terrain: vec![terrain::GRASS; 20 * 20],
        ..Default::default()
    };
    for tx in 4..=14 {
        let index = map.index(tx, 7);
        map.terrain[index] = terrain::ROAD_HORIZONTAL;
    }
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let start = map.tile_center(2, 9);
    let goal = map.tile_center(16, 9);
    let forward = [(3, 8), (4, 7), (14, 7), (15, 8), (16, 9)];
    let raw = crate::game::pathfinding::to_world_waypoints(&forward);
    let finalized = finalize_reverse_waypoints(
        &map,
        &occupancy,
        EntityKind::Rifleman,
        start,
        goal,
        RouteFinalizationMode::new(RouteShape::Normal, RoutePolicy::FastestTerrainTime),
        raw,
    )
    .expect("road route should finalize");

    assert!(
        finalized.len() > 1,
        "direct grass route must not erase faster road anchors"
    );
    assert!(finalized.iter().any(|&(x, y)| map.tile_of(x, y).1 == 7));
}

#[test]
fn fastest_terrain_finalizer_collapses_equal_cost_open_route_once() {
    let map = Map {
        width: 20,
        height: 20,
        terrain: vec![terrain::GRASS; 20 * 20],
        ..Default::default()
    };
    let entities = EntityStore::new();
    let occupancy = Occupancy::build(&map, &entities);
    let start = map.tile_center(2, 9);
    let goal = map.tile_center(16, 9);
    let raw = crate::game::pathfinding::to_world_waypoints(
        &(3..=16).map(|tx| (tx, 9)).collect::<Vec<_>>(),
    );
    let finalized = finalize_reverse_waypoints(
        &map,
        &occupancy,
        EntityKind::Rifleman,
        start,
        goal,
        RouteFinalizationMode::new(RouteShape::Normal, RoutePolicy::FastestTerrainTime),
        raw,
    )
    .expect("open route should finalize");
    assert_eq!(finalized, vec![goal]);
}
