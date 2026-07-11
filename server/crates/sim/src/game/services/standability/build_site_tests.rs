use super::*;

#[test]
fn build_site_status_classifies_clear_footprint() {
    let map = flat_map(12);
    let entities = EntityStore::new();

    assert_eq!(
        building_site_status_for_build_intent(&map, &entities, EntityKind::Depot, 4, 4, u32::MAX),
        BuildSiteStatus::Clear
    );
}

#[test]
fn build_site_status_classifies_invalid_footprints_and_terrain() {
    let mut map = flat_map(12);
    let entities = EntityStore::new();
    set_tile(&mut map, 4, 4, crate::protocol::terrain::ROCK);

    assert_eq!(
        building_site_status_for_build_intent(&map, &entities, EntityKind::Depot, 4, 4, u32::MAX),
        BuildSiteStatus::InvalidFootprint
    );
    assert_eq!(
        building_site_status_for_build_intent(&map, &entities, EntityKind::Depot, 11, 11, u32::MAX),
        BuildSiteStatus::InvalidFootprint
    );
    assert_eq!(
        building_site_status_for_build_intent(&map, &entities, EntityKind::Worker, 4, 4, u32::MAX),
        BuildSiteStatus::InvalidFootprint
    );
}

#[test]
fn build_site_status_classifies_building_and_scaffold_blockers() {
    let map = flat_map(12);
    let (x, y) = footprint_center(&map, EntityKind::Depot, 4, 4);

    let mut finished = EntityStore::new();
    finished
        .spawn_building(1, EntityKind::Depot, x, y, true)
        .expect("depot should spawn");
    assert_eq!(
        building_site_status_for_build_intent(&map, &finished, EntityKind::Depot, 4, 4, u32::MAX,),
        BuildSiteStatus::BlockedByBuilding
    );

    let mut scaffold = EntityStore::new();
    scaffold
        .spawn_building(1, EntityKind::Depot, x, y, false)
        .expect("depot scaffold should spawn");
    assert_eq!(
        building_site_status_for_build_intent(&map, &scaffold, EntityKind::Depot, 4, 4, u32::MAX,),
        BuildSiteStatus::BlockedByBuilding
    );
}

#[test]
fn build_site_status_classifies_resource_node_blockers() {
    let map = flat_map(12);
    let mut entities = EntityStore::new();
    let (nx, ny) = map.tile_center(4, 4);
    entities
        .spawn_node(EntityKind::Steel, nx, ny)
        .expect("steel node should spawn");

    assert_eq!(
        building_site_status_for_build_intent(&map, &entities, EntityKind::Depot, 4, 4, u32::MAX),
        BuildSiteStatus::BlockedByResourceNode
    );
}

#[test]
fn pump_jack_build_site_requires_live_oil_patch() {
    let map = flat_map(12);
    let (x, y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);

    let empty = EntityStore::new();
    assert_eq!(
        building_site_status_for_build_intent(&map, &empty, EntityKind::PumpJack, 4, 4, u32::MAX,),
        BuildSiteStatus::InvalidFootprint
    );

    let mut steel = EntityStore::new();
    steel
        .spawn_node(EntityKind::Steel, x, y)
        .expect("steel node should spawn");
    assert_eq!(
        building_site_status_for_build_intent(&map, &steel, EntityKind::PumpJack, 4, 4, u32::MAX,),
        BuildSiteStatus::InvalidFootprint
    );

    let mut oil = EntityStore::new();
    let oil_id = oil
        .spawn_node(EntityKind::Oil, x, y)
        .expect("oil node should spawn");
    assert_eq!(
        building_site_status_for_build_intent(&map, &oil, EntityKind::PumpJack, 4, 4, u32::MAX,),
        BuildSiteStatus::Clear
    );

    oil.spawn_building(1, EntityKind::PumpJack, x, y, false)
        .expect("pump jack scaffold should spawn");
    assert_eq!(
        building_site_status_for_build_intent(&map, &oil, EntityKind::PumpJack, 4, 4, u32::MAX,),
        BuildSiteStatus::BlockedByBuilding
    );

    oil.get_mut(oil_id)
        .expect("oil node should exist")
        .harvest_resources(u32::MAX);
    assert_eq!(
        building_site_status_for_build_intent(&map, &oil, EntityKind::PumpJack, 4, 4, u32::MAX,),
        BuildSiteStatus::InvalidFootprint
    );
}

#[test]
fn adjacent_oil_patch_edges_do_not_block_pump_jack_sites() {
    let map = flat_map(12);
    let (first_x, first_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    let (second_x, second_y) = footprint_center(&map, EntityKind::PumpJack, 5, 4);
    let mut entities = EntityStore::new();
    entities
        .spawn_node(EntityKind::Oil, first_x, first_y)
        .expect("first oil node should spawn");
    entities
        .spawn_node(EntityKind::Oil, second_x, second_y)
        .expect("second oil node should spawn");

    assert_eq!(
        building_site_status_for_build_intent(
            &map,
            &entities,
            EntityKind::PumpJack,
            4,
            4,
            u32::MAX,
        ),
        BuildSiteStatus::Clear,
        "neighboring oil circles may touch the footprint without invalidating the selected oil site"
    );

    entities
        .spawn_building(1, EntityKind::PumpJack, first_x, first_y, false)
        .expect("first pump jack should spawn");
    assert_eq!(
        building_site_status_for_build_intent(
            &map,
            &entities,
            EntityKind::PumpJack,
            5,
            4,
            u32::MAX,
        ),
        BuildSiteStatus::Clear,
        "a Pump Jack on one oil patch must not reserve an adjacent patch by edge contact"
    );
}

#[test]
fn build_site_status_classifies_relevant_unit_blockers() {
    let map = flat_map(12);
    let mut entities = EntityStore::new();
    let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
    let radius = config::unit_stats(EntityKind::Tank)
        .expect("tank stats")
        .radius;
    entities
        .spawn_unit(
            1,
            EntityKind::Tank,
            rect.max_x + radius - 1.0,
            rect.min_y + 32.0,
        )
        .expect("tank should spawn");

    assert_eq!(
        building_site_status_for_build_intent(&map, &entities, EntityKind::Depot, 4, 4, u32::MAX),
        BuildSiteStatus::BlockedByUnit
    );
}

#[test]
fn build_site_status_preserves_tank_trap_unit_policy() {
    let map = flat_map(12);
    let (x, y) = footprint_center(&map, EntityKind::TankTrap, 4, 4);
    let mut infantry = EntityStore::new();
    infantry
        .spawn_unit(1, EntityKind::Rifleman, x, y)
        .expect("rifleman should spawn");

    assert_eq!(
        building_site_status_for_build_intent(
            &map,
            &infantry,
            EntityKind::TankTrap,
            4,
            4,
            u32::MAX,
        ),
        BuildSiteStatus::Clear
    );

    let mut vehicle = EntityStore::new();
    vehicle
        .spawn_unit(1, EntityKind::Tank, x, y)
        .expect("tank should spawn");

    assert_eq!(
        building_site_status_for_build_intent(&map, &vehicle, EntityKind::TankTrap, 4, 4, u32::MAX,),
        BuildSiteStatus::BlockedByUnit
    );
}

#[test]
fn spatial_building_site_clear_matches_full_scan_for_blockers() {
    let map = flat_map(12);
    let mut cases = Vec::new();

    let mut blocked_by_building = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
    blocked_by_building
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("depot should spawn");
    cases.push(blocked_by_building);

    let mut blocked_by_unit = EntityStore::new();
    let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
    let radius = config::unit_stats(EntityKind::Tank)
        .expect("tank stats")
        .radius;
    blocked_by_unit
        .spawn_unit(
            1,
            EntityKind::Tank,
            rect.max_x + radius - 1.0,
            rect.min_y + 32.0,
        )
        .expect("tank should spawn");
    cases.push(blocked_by_unit);

    let mut blocked_by_resource = EntityStore::new();
    let (nx, ny) = map.tile_center(4, 4);
    blocked_by_resource
        .spawn_node(EntityKind::Steel, nx, ny)
        .expect("steel node should spawn");
    cases.push(blocked_by_resource);

    cases.push(EntityStore::new());

    for entities in cases {
        let spatial = SpatialIndex::build(&entities, map.size);
        assert_eq!(
            building_site_clear_spatial(&map, &entities, &spatial, EntityKind::Depot, 4, 4),
            building_site_clear(&map, &entities, EntityKind::Depot, 4, 4)
        );
    }
}

#[test]
fn spatial_building_site_clear_matches_pump_jack_contextual_policy() {
    let map = flat_map(12);
    let (x, y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);

    let empty = EntityStore::new();
    let spatial = SpatialIndex::build(&empty, map.size);
    assert_eq!(
        building_site_clear_spatial(&map, &empty, &spatial, EntityKind::PumpJack, 4, 4),
        building_site_clear(&map, &empty, EntityKind::PumpJack, 4, 4)
    );

    let mut with_oil = EntityStore::new();
    with_oil
        .spawn_node(EntityKind::Oil, x, y)
        .expect("oil node should spawn");
    let spatial = SpatialIndex::build(&with_oil, map.size);
    assert_eq!(
        building_site_clear_spatial(&map, &with_oil, &spatial, EntityKind::PumpJack, 4, 4),
        building_site_clear(&map, &with_oil, EntityKind::PumpJack, 4, 4)
    );

    with_oil
        .spawn_building(1, EntityKind::PumpJack, x, y, false)
        .expect("pump jack scaffold should spawn");
    let spatial = SpatialIndex::build(&with_oil, map.size);
    assert_eq!(
        building_site_clear_spatial(&map, &with_oil, &spatial, EntityKind::PumpJack, 4, 4),
        building_site_clear(&map, &with_oil, EntityKind::PumpJack, 4, 4)
    );
}

fn flat_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
        starts: vec![],
        base_sites: vec![],
    }
}

fn footprint_center(_map: &Map, kind: EntityKind, tile_x: u32, tile_y: u32) -> (f32, f32) {
    let stats = config::building_stats(kind).expect("building stats");
    let tile_size = config::TILE_SIZE as f32;
    (
        (tile_x as f32 + stats.foot_w as f32 * 0.5) * tile_size,
        (tile_y as f32 + stats.foot_h as f32 * 0.5) * tile_size,
    )
}

fn set_tile(map: &mut Map, x: u32, y: u32, terrain: u8) {
    let index = map.index(x, y);
    map.terrain[index] = terrain;
}
