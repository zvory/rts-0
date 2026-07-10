use super::*;

fn map_center_projection_tiles(
    map: AiMapSummary,
    base_center: (f32, f32),
    point: (f32, f32),
) -> f32 {
    let tile_size = map.tile_size as f32;
    let map_center = (
        map.width as f32 * tile_size * 0.5,
        map.height as f32 * tile_size * 0.5,
    );
    let dir = normalized_direction(base_center, map_center)
        .expect("base should not overlap the map center");
    ((point.0 - base_center.0) * dir.0 + (point.1 - base_center.1) * dir.1) / tile_size
}

#[test]
fn main_steel_cluster_center_uses_forward_split_field() {
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 10,
        },
        Vec::new(),
    );
    observation.resources =
        base_site_resources(100, observation.own_start_tile, observation.map.width);

    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let steel_center =
        main_steel_cluster_center(&observation).expect("main steel cluster should be found");
    let projection = map_center_projection_tiles(observation.map, own_base, steel_center);

    assert!(
        (config::STEEL_BLOCK_DIST_TILES - 0.25..=config::STEEL_BLOCK_DIST_TILES + 0.25)
            .contains(&projection),
        "main steel center should stay on the forward split field, got {projection:.2} tiles"
    );
}
