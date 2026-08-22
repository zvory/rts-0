use super::tree_detours::{expand_reverse_waypoints, tree_detour_between};
use super::*;
use crate::game::entity::EntityStore;
use crate::protocol::{terrain, MapDoodad};

#[test]
fn rifleman_tile_path_routes_around_tree_trunk_tile() {
    let size = 12;
    let mut map = Map {
        width: size,
        height: size,
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
            policy: RoutePolicy::LegacyShape,
            budget: None,
        },
    );
    assert_eq!(path.last(), Some(&(7, 5)));
    assert!(!path.contains(&(5, 5)), "tree tile appeared in {path:?}");
}

#[test]
fn infantry_may_cross_trunks_while_vehicle_routes_still_detour() {
    let size = 12;
    let mut map = Map {
        width: size,
        height: size,
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

    assert!(
        detour.is_empty(),
        "infantry route should use soft local avoidance: {detour:?}"
    );
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
    .expect("infantry tree-center hint should remain reachable");
    assert!(expanded.contains(&trunk));

    let tank_from = (trunk.0 - 48.0, trunk.1);
    let tank_to = (trunk.0 + 48.0, trunk.1);
    let tank_detour = tree_detour_between(&map, &occupancy, EntityKind::Tank, tank_from, tank_to)
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
    .expect("consecutive tree-center hints should remain reachable for infantry");
    assert!(expanded.contains(&trunk));
    assert!(expanded.contains(&second_trunk));

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
    .is_some());
}
