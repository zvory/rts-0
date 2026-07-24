use super::*;
use crate::game::entity::{EntityKind, EntityStore};
use crate::protocol::terrain;

fn fixture() -> (Map, EntityStore, u32, (f32, f32)) {
    let size = 64;
    let map = Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4)],
        base_sites: Vec::new(),
    };
    let origin = map.tile_center(20, 20);
    let mut entities = EntityStore::new();
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, origin.0, origin.1)
        .expect("artillery should spawn");
    (map, entities, artillery, origin)
}

#[test]
fn raw_target_preserves_an_inside_range_click() {
    let (map, entities, artillery, origin) = fixture();
    let clicked = (
        origin.0 + config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32 + 16.0,
        origin.1,
    );

    let target = artillery_point_fire_target(
        &map,
        &entities,
        1,
        artillery,
        clicked.0,
        clicked.1,
        ArtilleryPointFireAcceptance::Command,
    )
    .expect("inside-range target should be accepted");

    assert!(target.in_range);
    assert!((target.x - clicked.0).abs() < 0.001);
    assert!((target.y - clicked.1).abs() < 0.001);
}

#[test]
fn raw_target_preserves_out_of_range_clicks_for_repositioning() {
    let (map, entities, artillery, origin) = fixture();
    for clicked in [
        (origin.0 + 16.0, origin.1),
        (
            origin.0 + (config::ARTILLERY_MAX_RANGE_TILES as f32 + 4.0) * config::TILE_SIZE as f32,
            origin.1,
        ),
    ] {
        let target = artillery_point_fire_target(
            &map,
            &entities,
            1,
            artillery,
            clicked.0,
            clicked.1,
            ArtilleryPointFireAcceptance::Command,
        )
        .expect("in-map target should be accepted for repositioning");

        assert!(!target.in_range);
        assert!((target.x - clicked.0).abs() < 0.001);
        assert!((target.y - clicked.1).abs() < 0.001);
    }
}
