use super::*;
use rts_sim::game::map::Map;
use rts_sim::game::{Game, MapMetadata, PlayerInit};
use std::collections::{BTreeSet, VecDeque};

const FIXTURE_SEED: u32 = 0x1234_5678;

#[derive(Clone, Copy)]
struct ExpectedFixture {
    name: &'static str,
    component_count: usize,
    passable_tiles: u32,
    blocked_tiles: u32,
    largest_component_tiles: u32,
    resource_clusters: usize,
}

#[derive(Clone, Copy)]
struct ExpectedRegionFixture {
    name: &'static str,
    regions: usize,
    chokes_min: usize,
    chokes_max: usize,
}

fn player_inits(count: u32) -> Vec<PlayerInit> {
    (1..=count)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("P{id}"),
            color: format!("#{id}{id}{id}"),
            is_ai: true,
        })
        .collect()
}

fn fixture_analysis(map_name: &str) -> AiMapAnalysisDebugSnapshot {
    let players = player_inits(2);
    let player_slots: Vec<_> = players
        .iter()
        .map(|player| (player.id, player.team_id))
        .collect();
    let map = Map::load_for_players(map_name, &player_slots, FIXTURE_SEED)
        .expect("fixture map should load");
    let metadata = Map::metadata_for_name(map_name).unwrap_or_else(|_| MapMetadata {
        name: map_name.to_string(),
        schema_version: rts_sim::game::map::CURRENT_MAP_VERSION,
        content_hash: "test".to_string(),
    });
    let game =
        Game::new_with_random_ai_profiles_and_map_metadata(&players, FIXTURE_SEED, map, metadata);
    AiMapAnalysis::analyze(&game.start_payload()).debug_snapshot()
}

fn resource_at(id: u32, kind: &str, tile_x: u32, tile_y: u32) -> ResourceNode {
    let tile_size = config::TILE_SIZE as f32;
    ResourceNode {
        id,
        kind: kind.to_string(),
        x: (tile_x as f32 + 0.5) * tile_size,
        y: (tile_y as f32 + 0.5) * tile_size,
    }
}

fn start_payload_for_terrain(
    width: u32,
    height: u32,
    terrain: Vec<u8>,
    starts: &[(u32, u32)],
) -> StartPayload {
    StartPayload {
        player_id: 1,
        spectator: false,
        prediction_build_id: None,
        prediction_version: 0,
        match_run_id: None,
        observation_tick_limit: None,
        capabilities: Default::default(),
        diagnostics: Default::default(),
        replay: None,
        lab: None,
        tick: 0,
        map: MapInfo {
            width,
            height,
            tile_size: config::TILE_SIZE,
            terrain,
            resources: Vec::new(),
        },
        players: starts
            .iter()
            .enumerate()
            .map(|(idx, &(x, y))| {
                let id = idx as u32 + 1;
                PlayerStart {
                    id,
                    team_id: id,
                    faction_id: "kriegsia".to_string(),
                    name: format!("P{id}"),
                    color: format!("#{id}{id}{id}"),
                    start_tile_x: x,
                    start_tile_y: y,
                }
            })
            .collect(),
    }
}

fn grass_terrain(width: u32, height: u32) -> Vec<u8> {
    vec![terrain::GRASS; (width * height) as usize]
}

fn set_rock(terrain: &mut [u8], width: u32, x: u32, y: u32) {
    terrain[(y * width + x) as usize] = terrain::ROCK;
}

fn set_grass(terrain: &mut [u8], width: u32, x: u32, y: u32) {
    terrain[(y * width + x) as usize] = terrain::GRASS;
}

fn fill_grass_rect(terrain: &mut [u8], width: u32, x: u32, y: u32, w: u32, h: u32) {
    for ty in y..y.saturating_add(h) {
        for tx in x..x.saturating_add(w) {
            set_grass(terrain, width, tx, ty);
        }
    }
}

fn add_vertical_wall_with_gaps(
    terrain: &mut [u8],
    width: u32,
    height: u32,
    wall_x: u32,
    wall_w: u32,
    gaps: &[(u32, u32)],
) {
    for y in 0..height {
        let in_gap = gaps
            .iter()
            .any(|&(gap_y, gap_h)| y >= gap_y && y < gap_y.saturating_add(gap_h));
        if in_gap {
            continue;
        }
        for x in wall_x..wall_x.saturating_add(wall_w) {
            set_rock(terrain, width, x, y);
        }
    }
}

#[test]
fn no_terrain_fixture_is_one_clear_component() {
    let debug = fixture_analysis("No Terrain");

    assert_eq!(debug.map_width, 126);
    assert_eq!(debug.map_height, 126);
    assert_eq!(debug.passable_tiles, 126 * 126);
    assert_eq!(debug.blocked_tiles, 0);
    assert_eq!(debug.component_count, 1);
    assert_eq!(debug.largest_component_tiles, 126 * 126);
    assert_eq!(debug.max_clearance_tiles, MAX_CLEARANCE_TILES);
    assert_eq!(debug.region_count, 1);
    assert_eq!(debug.choke_count, 0);
    assert!(debug.starts.iter().all(|start| {
        start.component_id == Some(0) && start.clearance_tiles == MAX_CLEARANCE_TILES
    }));
    assert!(debug.starts.iter().all(|start| start.region_id == Some(0)));
}

#[test]
fn bundled_fixture_counts_are_deterministic() {
    let expected = [
        ExpectedFixture {
            name: "Default",
            component_count: 43,
            passable_tiles: 14_634,
            blocked_tiles: 1_242,
            largest_component_tiles: 14_476,
            resource_clusters: 6,
        },
        ExpectedFixture {
            name: "Low Econ",
            component_count: 45,
            passable_tiles: 14_615,
            blocked_tiles: 1_261,
            largest_component_tiles: 14_451,
            resource_clusters: 4,
        },
        ExpectedFixture {
            name: "No Terrain",
            component_count: 1,
            passable_tiles: 126 * 126,
            blocked_tiles: 0,
            largest_component_tiles: 126 * 126,
            resource_clusters: 4,
        },
    ];

    for fixture in expected {
        let debug = fixture_analysis(fixture.name);

        assert_eq!(
            debug.component_count, fixture.component_count,
            "{} component count changed",
            fixture.name
        );
        assert_eq!(
            debug.passable_tiles, fixture.passable_tiles,
            "{} passable tile count changed",
            fixture.name
        );
        assert_eq!(
            debug.blocked_tiles, fixture.blocked_tiles,
            "{} blocked tile count changed",
            fixture.name
        );
        assert_eq!(
            debug.largest_component_tiles, fixture.largest_component_tiles,
            "{} largest component size changed",
            fixture.name
        );
        assert_eq!(
            debug.resource_clusters.len(),
            fixture.resource_clusters,
            "{} resource cluster count changed",
            fixture.name
        );
        assert_eq!(debug.passable_tiles + debug.blocked_tiles, 126 * 126);
    }
}

#[test]
fn bundled_fixture_region_and_choke_counts_are_legible() {
    let expected = [
        ExpectedRegionFixture {
            name: "Default",
            regions: 5,
            chokes_min: 12,
            chokes_max: 12,
        },
        ExpectedRegionFixture {
            name: "Low Econ",
            regions: 5,
            chokes_min: 12,
            chokes_max: 12,
        },
        ExpectedRegionFixture {
            name: "No Terrain",
            regions: 1,
            chokes_min: 0,
            chokes_max: 0,
        },
    ];

    for fixture in expected {
        let debug = fixture_analysis(fixture.name);

        assert_eq!(
            debug.region_count, fixture.regions,
            "{} meaningful region count changed",
            fixture.name
        );
        assert!(
            (fixture.chokes_min..=fixture.chokes_max).contains(&debug.choke_count),
            "{} choke count {} outside expected range {}..={}",
            fixture.name,
            debug.choke_count,
            fixture.chokes_min,
            fixture.chokes_max
        );
        for start in &debug.starts {
            assert!(
                start.region_id.is_some(),
                "{} player {} should map to a meaningful region",
                fixture.name,
                start.player_id
            );
        }
        for choke in &debug.chokes {
            assert_eq!(
                choke.tiles.len(),
                choke.tile_count as usize,
                "{} choke {:?} tile list must match tile count",
                fixture.name,
                choke.id
            );
            assert!(
                choke.tiles.iter().all(|tile| tile.x >= choke.bounds.min.x
                    && tile.x <= choke.bounds.max.x
                    && tile.y >= choke.bounds.min.y
                    && tile.y <= choke.bounds.max.y),
                "{} choke {:?} has a tile outside its bounds",
                fixture.name,
                choke.id
            );
            assert!(
                tiles_are_cardinally_connected(&choke.tiles),
                "{} choke {:?} tiles should not bridge disconnected passable fragments",
                fixture.name,
                choke.id
            );
            assert_ne!(
                choke.region_a_id, choke.region_b_id,
                "{} choke {:?} must connect distinct regions",
                fixture.name, choke.id
            );
            assert!(
                debug
                    .regions
                    .iter()
                    .any(|region| region.id == choke.region_a_id)
                    && debug
                        .regions
                        .iter()
                        .any(|region| region.id == choke.region_b_id),
                "{} choke {:?} references unknown regions",
                fixture.name,
                choke.id
            );
        }
    }
}

fn tiles_are_cardinally_connected(tiles: &[AiTile]) -> bool {
    let Some(&start) = tiles.first() else {
        return false;
    };
    let all: BTreeSet<_> = tiles.iter().copied().collect();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(tile) = queue.pop_front() {
        if !seen.insert(tile) {
            continue;
        }
        for (dx, dy) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
            let nx = tile.x as i32 + dx;
            let ny = tile.y as i32 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let neighbor = AiTile::new(nx as u32, ny as u32);
            if all.contains(&neighbor) && !seen.contains(&neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    seen.len() == all.len()
}

#[derive(Clone, Copy, Debug)]
struct TargetBox {
    id: &'static str,
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
}

#[test]
fn default_chokes_cover_marked_gameplay_passages() {
    let debug = fixture_analysis("Default");
    let targets = [
        TargetBox {
            id: "T0",
            min_x: 66,
            min_y: 12,
            max_x: 81,
            max_y: 28,
        },
        TargetBox {
            id: "T1",
            min_x: 52,
            min_y: 13,
            max_x: 64,
            max_y: 28,
        },
        TargetBox {
            id: "T2",
            min_x: 24,
            min_y: 29,
            max_x: 54,
            max_y: 53,
        },
        TargetBox {
            id: "T3",
            min_x: 74,
            min_y: 30,
            max_x: 97,
            max_y: 54,
        },
        TargetBox {
            id: "T4",
            min_x: 13,
            min_y: 46,
            max_x: 29,
            max_y: 58,
        },
        TargetBox {
            id: "T5",
            min_x: 101,
            min_y: 52,
            max_x: 114,
            max_y: 62,
        },
        TargetBox {
            id: "T6",
            min_x: 11,
            min_y: 61,
            max_x: 29,
            max_y: 72,
        },
        TargetBox {
            id: "T7",
            min_x: 100,
            min_y: 63,
            max_x: 113,
            max_y: 73,
        },
        TargetBox {
            id: "T8",
            min_x: 75,
            min_y: 72,
            max_x: 101,
            max_y: 97,
        },
        TargetBox {
            id: "T9",
            min_x: 26,
            min_y: 73,
            max_x: 54,
            max_y: 99,
        },
        TargetBox {
            id: "T10",
            min_x: 65,
            min_y: 98,
            max_x: 78,
            max_y: 113,
        },
        TargetBox {
            id: "T11",
            min_x: 52,
            min_y: 99,
            max_x: 63,
            max_y: 115,
        },
    ];

    let missing: Vec<_> = targets
        .iter()
        .filter(|target| {
            !debug
                .chokes
                .iter()
                .any(|choke| choke_covers_target(choke, **target))
        })
        .map(|target| target.id)
        .collect();

    assert!(
        missing.is_empty(),
        "Default gameplay chokes should cover marked passages; missing {missing:?}; chokes {:?}",
        debug
            .chokes
            .iter()
            .map(|choke| (
                choke.id,
                choke.center_tile,
                choke.bounds.min,
                choke.bounds.max
            ))
            .collect::<Vec<_>>()
    );
}

fn choke_covers_target(choke: &AiMapChoke, target: TargetBox) -> bool {
    choke
        .tiles
        .iter()
        .copied()
        .any(|tile| point_in_target(tile, target))
}

fn point_in_target(tile: AiTile, target: TargetBox) -> bool {
    tile.x >= target.min_x
        && tile.x <= target.max_x
        && tile.y >= target.min_y
        && tile.y <= target.max_y
}

#[test]
fn open_field_has_one_region_and_no_chokes() {
    let width = 48;
    let height = 32;
    let start = start_payload_for_terrain(
        width,
        height,
        grass_terrain(width, height),
        &[(10, 16), (38, 16)],
    );
    let debug = AiMapAnalysis::analyze(&start).debug_snapshot();

    assert_eq!(debug.component_count, 1);
    assert_eq!(debug.region_count, 1);
    assert_eq!(debug.choke_count, 0);
    assert!(debug.starts.iter().all(|start| start.region_id == Some(0)));
}

#[test]
fn single_gap_wall_extracts_one_choke_between_two_regions() {
    let width = 64;
    let height = 36;
    let mut terrain = grass_terrain(width, height);
    add_vertical_wall_with_gaps(&mut terrain, width, height, 31, 2, &[(16, 4)]);
    let start = start_payload_for_terrain(width, height, terrain, &[(12, 18), (52, 18)]);
    let debug = AiMapAnalysis::analyze(&start).debug_snapshot();

    assert_eq!(debug.region_count, 2);
    assert_eq!(debug.choke_count, 1);
    let choke = &debug.chokes[0];
    assert_ne!(choke.region_a_id, choke.region_b_id);
    assert!(
        choke.width_tiles >= 3 && choke.width_tiles <= 8,
        "unexpected choke width: {:?}",
        choke
    );
    assert!(debug.starts.iter().all(|start| start.region_id.is_some()));
}

#[test]
fn long_corridor_choke_keeps_middle_tiles_between_regions() {
    let width = 80;
    let height = 40;
    let mut terrain = vec![terrain::ROCK; (width * height) as usize];
    fill_grass_rect(&mut terrain, width, 0, 0, 24, height);
    fill_grass_rect(&mut terrain, width, 56, 0, 24, height);
    fill_grass_rect(&mut terrain, width, 24, 18, 32, 4);
    let start = start_payload_for_terrain(width, height, terrain, &[(12, 20), (68, 20)]);
    let debug = AiMapAnalysis::analyze(&start).debug_snapshot();

    assert_eq!(debug.region_count, 2);
    assert_eq!(debug.choke_count, 1);
    let choke = &debug.chokes[0];
    assert!(
        choke.tiles.contains(&AiTile::new(40, 19)),
        "long choke should include the corridor middle, not just the ends: {:?}",
        choke
    );
    assert!(
        choke.tile_count > u32::from(CHOKE_CONTACT_RADIUS_TILES) * 2,
        "long choke should retain a full passage band: {:?}",
        choke
    );
    assert!(
        choke.width_tiles <= 6,
        "corridor width should be measured by cross-section, not bounds: {:?}",
        choke
    );
}

#[test]
fn two_gap_wall_extracts_two_alternate_chokes() {
    let width = 64;
    let height = 44;
    let mut terrain = grass_terrain(width, height);
    add_vertical_wall_with_gaps(&mut terrain, width, height, 31, 2, &[(10, 4), (30, 4)]);
    let start = start_payload_for_terrain(width, height, terrain, &[(12, 22), (52, 22)]);
    let debug = AiMapAnalysis::analyze(&start).debug_snapshot();

    assert_eq!(debug.region_count, 2);
    assert_eq!(debug.choke_count, 2);
    assert!(debug
        .chokes
        .iter()
        .all(|choke| choke.region_a_id != choke.region_b_id));
    assert!(
        debug.chokes[0]
            .center_tile
            .y
            .abs_diff(debug.chokes[1].center_tile.y)
            >= 12,
        "alternate chokes should remain separate: {:?}",
        debug.chokes
    );
}

#[test]
fn resource_clusters_cover_all_static_nodes_with_expected_base_shape() {
    let expected_nodes_per_cluster =
        (config::STEEL_PATCHES_PER_BASE + config::OIL_PATCHES_PER_BASE) as usize;

    for map_name in ["Default", "Low Econ", "No Terrain"] {
        let debug = fixture_analysis(map_name);
        let total_clustered_nodes: usize = debug
            .resource_clusters
            .iter()
            .map(|cluster| cluster.resource_ids.len())
            .sum();

        assert_eq!(
            total_clustered_nodes,
            debug.resource_clusters.len() * expected_nodes_per_cluster,
            "{map_name} should assign every static resource to full base clusters"
        );
        for cluster in &debug.resource_clusters {
            assert_eq!(
                cluster.resource_ids.len(),
                expected_nodes_per_cluster,
                "{map_name} cluster {:?} should keep one base resource group",
                cluster.id
            );
            assert_eq!(cluster.steel_nodes, config::STEEL_PATCHES_PER_BASE as u16);
            assert_eq!(cluster.oil_nodes, config::OIL_PATCHES_PER_BASE as u16);
            assert!(
                cluster.component_id.is_some(),
                "{map_name} cluster {:?} should map to passable terrain",
                cluster.id
            );
            assert!(
                cluster.region_id.is_some(),
                "{map_name} cluster {:?} should map to a meaningful region",
                cluster.id
            );
        }
    }
}

#[test]
fn player_starts_map_to_components_and_nearby_resource_clusters() {
    for map_name in ["Default", "Low Econ", "No Terrain"] {
        let debug = fixture_analysis(map_name);

        assert_eq!(debug.starts.len(), 2);
        for start in &debug.starts {
            assert!(
                start.component_id.is_some(),
                "{map_name} player {} start should map to a passable component",
                start.player_id
            );
            assert!(
                start.region_id.is_some(),
                "{map_name} player {} start should map to a meaningful region",
                start.player_id
            );
            assert!(
                start.clearance_tiles >= 8,
                "{map_name} player {} start clearance was {}",
                start.player_id,
                start.clearance_tiles
            );
            assert!(
                start.nearest_resource_cluster_id.is_some(),
                "{map_name} player {} should have a nearest resource cluster",
                start.player_id
            );
        }
    }
}

#[test]
fn resource_mappings_prefer_reachable_components_over_cross_wall_distance() {
    let width = 40;
    let height = 10;
    let mut terrain = vec![terrain::GRASS; (width * height) as usize];
    for y in 0..height {
        terrain[(y * width + 20) as usize] = terrain::ROCK;
    }
    let start = StartPayload {
        player_id: 1,
        spectator: false,
        prediction_build_id: None,
        prediction_version: 0,
        match_run_id: None,
        observation_tick_limit: None,
        capabilities: Default::default(),
        diagnostics: Default::default(),
        replay: None,
        lab: None,
        tick: 0,
        map: MapInfo {
            width,
            height,
            tile_size: config::TILE_SIZE,
            terrain,
            resources: vec![
                resource_at(1, kinds::STEEL, 2, 5),
                resource_at(2, kinds::STEEL, 21, 5),
            ],
        },
        players: vec![
            PlayerStart {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "P1".to_string(),
                color: "#111".to_string(),
                start_tile_x: 19,
                start_tile_y: 5,
            },
            PlayerStart {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "P2".to_string(),
                color: "#222".to_string(),
                start_tile_x: 39,
                start_tile_y: 5,
            },
        ],
    };

    let debug = AiMapAnalysis::analyze(&start).debug_snapshot();
    let p1 = debug
        .starts
        .iter()
        .find(|start| start.player_id == 1)
        .expect("player 1 start should be present");
    let p2 = debug
        .starts
        .iter()
        .find(|start| start.player_id == 2)
        .expect("player 2 start should be present");
    assert_ne!(p1.component_id, p2.component_id);

    let p1_cluster = debug
        .resource_clusters
        .iter()
        .find(|cluster| Some(cluster.id) == p1.nearest_resource_cluster_id)
        .expect("player 1 should have a nearest cluster");
    assert_eq!(p1_cluster.component_id, p1.component_id);
    assert!(
        p1_cluster.resource_ids.contains(&1),
        "player 1 should map to the same-component resource, not the closer cross-wall one"
    );

    let right_cluster = debug
        .resource_clusters
        .iter()
        .find(|cluster| cluster.resource_ids.contains(&2))
        .expect("right-side resource should be clustered");
    assert_eq!(right_cluster.component_id, p2.component_id);
    assert_eq!(right_cluster.nearest_start_player_id, Some(2));
}

#[test]
fn analysis_key_tracks_static_map_start_and_resource_identity() {
    let mut start = StartPayload {
        player_id: 1,
        spectator: false,
        prediction_build_id: None,
        prediction_version: 0,
        match_run_id: None,
        observation_tick_limit: None,
        capabilities: Default::default(),
        diagnostics: Default::default(),
        replay: None,
        lab: None,
        tick: 0,
        map: MapInfo {
            width: 4,
            height: 4,
            tile_size: config::TILE_SIZE,
            terrain: vec![terrain::GRASS; 16],
            resources: Vec::new(),
        },
        players: vec![PlayerStart {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "P1".to_string(),
            color: "#111".to_string(),
            start_tile_x: 1,
            start_tile_y: 1,
        }],
    };

    let original = AiMapAnalysisKey::from_start(&start);
    start.players[0].start_tile_x = 2;
    let moved_start = AiMapAnalysisKey::from_start(&start);
    start.map.terrain[0] = terrain::ROCK;
    let changed_terrain = AiMapAnalysisKey::from_start(&start);

    assert_ne!(original, moved_start);
    assert_ne!(moved_start, changed_terrain);
}
