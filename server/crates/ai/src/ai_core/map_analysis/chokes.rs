use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::*;

mod geometry;
mod min_cut;
mod region_pair;
use geometry::choke_line_geometry;
use min_cut::{linearity_score, local_min_vertex_cut, tile_in_bounds};
use region_pair::candidate_split_region_pair;

#[derive(Clone, Copy, Debug)]
struct RegionContact {
    region_tile: AiTile,
    portal_tile: AiTile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RegionDistance {
    region_id: u32,
    distance: u32,
}

const GAMEPLAY_CHOKE_TARGET_COUNT: usize = 12;
const GAMEPLAY_BROAD_CHOKE_COUNT: usize = 4;
const GAMEPLAY_BASIN_CLEARANCE_TILES: u16 = 10;
const GAMEPLAY_SADDLE_MIN_CLEARANCE_TILES: u16 = 4;
const GAMEPLAY_BASIN_RADIUS_TILES: u32 = 10;
const GAMEPLAY_BROAD_MIN_TILES: usize = 32;
const GAMEPLAY_BROAD_MAX_TILES: usize = 700;
const GAMEPLAY_GAP_MIN_TILES: u32 = 4;
const GAMEPLAY_GAP_MAX_TILES: u32 = 24;
const GAMEPLAY_CUT_MIN_SPACING_TILES: u32 = 8;
const GAMEPLAY_LOCAL_CUT_PADDING_TILES: u32 = 14;
const GAMEPLAY_LOCAL_CUT_MIN_SIDE_TILES: usize = 24;
const GAMEPLAY_MIN_CUT_PADDING_TILES: u32 = 16;
const GAMEPLAY_MIN_CUT_PROTECTED_CLEARANCE_TILES: u16 = 10;
const GAMEPLAY_MIN_CUT_MAX_TILES: usize = 96;
const GAMEPLAY_FLOW_INF: i32 = 1_000_000;

#[derive(Clone, Debug)]
struct ChokeCandidate {
    center: AiTile,
    bounds: AiTileBounds,
    tiles: Vec<AiTile>,
    score: i32,
    basin_pair: Option<(u32, u32)>,
}

struct ChokeBuildContext<'a> {
    width: u32,
    height: u32,
    passable: &'a [bool],
    clearance: &'a [u16],
    region_by_tile: &'a [Option<u32>],
    regions: &'a [AiMapRegion],
}

#[derive(Clone, Copy, Debug)]
struct LinearCutDirection {
    dx: i32,
    dy: i32,
    normal_x: i32,
    normal_y: i32,
    thicken_diagonal: bool,
}

const LINEAR_CUT_DIRECTIONS: [LinearCutDirection; 4] = [
    LinearCutDirection {
        dx: 1,
        dy: 0,
        normal_x: 0,
        normal_y: 1,
        thicken_diagonal: false,
    },
    LinearCutDirection {
        dx: 0,
        dy: 1,
        normal_x: 1,
        normal_y: 0,
        thicken_diagonal: false,
    },
    LinearCutDirection {
        dx: 1,
        dy: 1,
        normal_x: -1,
        normal_y: 1,
        thicken_diagonal: true,
    },
    LinearCutDirection {
        dx: 1,
        dy: -1,
        normal_x: 1,
        normal_y: 1,
        thicken_diagonal: true,
    },
];

pub(super) fn build_chokes(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
    region_by_tile: &[Option<u32>],
    regions: &[AiMapRegion],
) -> Vec<AiMapChoke> {
    let gameplay_chokes =
        build_gameplay_chokes(width, height, passable, clearance, region_by_tile, regions);
    if gameplay_chokes.len() >= GAMEPLAY_CHOKE_TARGET_COUNT {
        return gameplay_chokes;
    }
    build_region_band_chokes(width, height, passable, clearance, region_by_tile, regions)
}

fn build_region_band_chokes(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
    region_by_tile: &[Option<u32>],
    regions: &[AiMapRegion],
) -> Vec<AiMapChoke> {
    if regions.len() < 2 {
        return Vec::new();
    }

    let mut visited = vec![false; passable.len()];
    let mut queue = VecDeque::new();
    let mut chokes = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let Some(start_idx) = tile_index(width, height, x, y) else {
                continue;
            };
            let start_tile = AiTile::new(x, y);
            if visited.get(start_idx).copied() == Some(true)
                || !is_choke_band_tile(passable, region_by_tile, start_idx)
            {
                continue;
            }

            let mut band_tiles = Vec::new();

            visited[start_idx] = true;
            queue.push_back(start_tile);
            while let Some(tile) = queue.pop_front() {
                if tile_index(width, height, tile.x, tile.y).is_none() {
                    continue;
                }
                band_tiles.push(tile);

                for neighbor in cardinal_neighbors(width, height, tile) {
                    let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y)
                    else {
                        continue;
                    };
                    if visited.get(neighbor_idx).copied() == Some(true)
                        || !is_choke_band_tile(passable, region_by_tile, neighbor_idx)
                    {
                        continue;
                    }
                    visited[neighbor_idx] = true;
                    queue.push_back(neighbor);
                }
            }

            build_chokes_for_band(
                width,
                height,
                passable,
                clearance,
                region_by_tile,
                &band_tiles,
                &mut chokes,
            );
        }
    }

    chokes
}

fn build_gameplay_chokes(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
    region_by_tile: &[Option<u32>],
    regions: &[AiMapRegion],
) -> Vec<AiMapChoke> {
    if regions.len() < 2 {
        return Vec::new();
    }
    let candidates = build_linear_graph_cut_candidates(width, height, passable, clearance);
    if candidates.len() < GAMEPLAY_CHOKE_TARGET_COUNT {
        return Vec::new();
    }

    let context = ChokeBuildContext {
        width,
        height,
        passable,
        clearance,
        region_by_tile,
        regions,
    };
    let mut chokes = Vec::new();
    for candidate in &candidates {
        let Some(choke) = choke_from_candidate(&context, candidate, chokes.len() as u32) else {
            continue;
        };
        chokes.push(choke);
        if chokes.len() >= GAMEPLAY_CHOKE_TARGET_COUNT {
            break;
        }
    }
    if chokes.len() >= GAMEPLAY_CHOKE_TARGET_COUNT {
        chokes
    } else {
        Vec::new()
    }
}

fn build_linear_graph_cut_candidates(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
) -> Vec<ChokeCandidate> {
    let broad_seeds = build_broad_saddle_candidates(width, height, passable, clearance);
    if broad_seeds.len() < GAMEPLAY_BROAD_CHOKE_COUNT {
        return Vec::new();
    }

    let mut candidates =
        saddle_min_cut_candidates(width, height, passable, clearance, &broad_seeds);
    candidates.extend(direct_linear_cut_candidates(
        width, height, passable, clearance,
    ));
    non_max_choke_candidates(
        candidates,
        GAMEPLAY_CHOKE_TARGET_COUNT,
        GAMEPLAY_CUT_MIN_SPACING_TILES,
    )
}

fn build_broad_saddle_candidates(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
) -> Vec<ChokeCandidate> {
    let basins = high_clearance_components(
        width,
        height,
        passable,
        clearance,
        GAMEPLAY_BASIN_CLEARANCE_TILES,
    );
    if basins.len() < 2 {
        return Vec::new();
    }
    let nearest = nearest_basin_distances(
        width,
        height,
        passable,
        &basins,
        GAMEPLAY_BASIN_RADIUS_TILES,
    );
    let mut by_pair: BTreeMap<(u32, u32), BTreeSet<AiTile>> = BTreeMap::new();
    for y in 0..height {
        for x in 0..width {
            let Some(idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if passable.get(idx).copied() != Some(true) {
                continue;
            }
            let tile_clearance = clearance.get(idx).copied().unwrap_or(0);
            if !(GAMEPLAY_SADDLE_MIN_CLEARANCE_TILES..GAMEPLAY_BASIN_CLEARANCE_TILES)
                .contains(&tile_clearance)
            {
                continue;
            }
            let Some(entries) = nearest.get(idx) else {
                continue;
            };
            if entries.len() < 2 {
                continue;
            }
            let pair = ordered_pair(entries[0].region_id, entries[1].region_id);
            by_pair.entry(pair).or_default().insert(AiTile::new(x, y));
        }
    }

    let mut candidates = Vec::new();
    for (&pair, tiles) in &by_pair {
        for group in connected_tile_groups_8(width, height, tiles) {
            if group.len() < GAMEPLAY_BROAD_MIN_TILES || group.len() > GAMEPLAY_BROAD_MAX_TILES {
                continue;
            }
            let score = group.len() as i32;
            if let Some(mut candidate) = candidate_from_tiles(group, score) {
                candidate.basin_pair = Some(pair);
                candidates.push(candidate);
            }
        }
    }
    non_max_choke_candidates(candidates, GAMEPLAY_BROAD_CHOKE_COUNT, 18)
}

fn saddle_min_cut_candidates(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
    broad_seeds: &[ChokeCandidate],
) -> Vec<ChokeCandidate> {
    let basins = high_clearance_components(
        width,
        height,
        passable,
        clearance,
        GAMEPLAY_BASIN_CLEARANCE_TILES,
    );
    let mut candidates = Vec::new();
    for seed in broad_seeds {
        let Some((basin_a_id, basin_b_id)) = seed.basin_pair else {
            continue;
        };
        let Some(basin_a) = basins.get(basin_a_id as usize) else {
            continue;
        };
        let Some(basin_b) = basins.get(basin_b_id as usize) else {
            continue;
        };
        let bounds = expanded_bounds(width, height, seed.bounds, GAMEPLAY_MIN_CUT_PADDING_TILES);
        let cut_tiles =
            local_min_vertex_cut(width, height, passable, clearance, bounds, basin_a, basin_b);
        if cut_tiles.len() < GAMEPLAY_GAP_MIN_TILES as usize
            || cut_tiles.len() > GAMEPLAY_MIN_CUT_MAX_TILES
        {
            continue;
        }
        let score = 2_500_i32
            .saturating_add((100_i32.saturating_sub(cut_tiles.len() as i32)).saturating_mul(6))
            .saturating_add(linearity_score(&cut_tiles));
        if let Some(mut candidate) = candidate_from_tiles(cut_tiles, score) {
            candidate.basin_pair = seed.basin_pair;
            candidates.push(candidate);
        }
    }
    candidates
}

fn direct_linear_cut_candidates(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
) -> Vec<ChokeCandidate> {
    let mut candidates = Vec::new();
    for direction in LINEAR_CUT_DIRECTIONS {
        for start in linear_cut_starts(width, height, direction) {
            let line = linear_cut_line(width, height, start, direction);
            let mut cursor = 0_usize;
            while cursor < line.len() {
                let tile = line[cursor];
                if !passable_at_tile(width, height, passable, tile.x as i32, tile.y as i32) {
                    cursor = cursor.saturating_add(1);
                    continue;
                }
                let run_start = cursor;
                while cursor < line.len() {
                    let tile = line[cursor];
                    if !passable_at_tile(width, height, passable, tile.x as i32, tile.y as i32) {
                        break;
                    }
                    cursor = cursor.saturating_add(1);
                }
                let run_end = cursor.saturating_sub(1);
                let run_len = run_end.saturating_sub(run_start).saturating_add(1);
                if run_len < GAMEPLAY_GAP_MIN_TILES as usize
                    || run_len > GAMEPLAY_GAP_MAX_TILES as usize
                {
                    continue;
                }
                if line
                    .get(run_start.saturating_sub(1))
                    .filter(|_| run_start > 0)
                    .is_some_and(|tile| {
                        passable_at_tile(width, height, passable, tile.x as i32, tile.y as i32)
                    })
                {
                    continue;
                }
                if line.get(run_end.saturating_add(1)).is_some_and(|tile| {
                    passable_at_tile(width, height, passable, tile.x as i32, tile.y as i32)
                }) {
                    continue;
                }

                let cut_tiles = thicken_linear_cut(
                    width,
                    height,
                    passable,
                    &line[run_start..=run_end],
                    direction,
                );
                let Some(split_score) = local_cut_split_score(
                    width,
                    height,
                    passable,
                    &cut_tiles,
                    (direction.normal_x, direction.normal_y),
                ) else {
                    continue;
                };
                let Some(mut candidate) = candidate_from_tiles(cut_tiles, 0) else {
                    continue;
                };
                if candidate_touches_map_edge(width, height, &candidate) {
                    continue;
                }
                let length_score = (28_usize.saturating_sub(run_len) as i32).saturating_mul(8);
                let clearance_score =
                    low_clearance_score(width, height, clearance, &candidate.tiles);
                candidate.score = split_score
                    .saturating_add(length_score)
                    .saturating_add(clearance_score)
                    .saturating_add(direct_cut_score(width, height, &candidate));
                candidates.push(candidate);
            }
        }
    }
    candidates
}

fn linear_cut_starts(width: u32, height: u32, direction: LinearCutDirection) -> Vec<AiTile> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    if direction.dx == 1 && direction.dy == 0 {
        return (0..height).map(|y| AiTile::new(0, y)).collect();
    }
    if direction.dx == 0 && direction.dy == 1 {
        return (0..width).map(|x| AiTile::new(x, 0)).collect();
    }
    if direction.dx == 1 && direction.dy == 1 {
        let mut starts: Vec<_> = (0..width).map(|x| AiTile::new(x, 0)).collect();
        starts.extend((1..height).map(|y| AiTile::new(0, y)));
        return starts;
    }
    let mut starts: Vec<_> = (0..width)
        .map(|x| AiTile::new(x, height.saturating_sub(1)))
        .collect();
    starts.extend(
        (0..height.saturating_sub(1))
            .rev()
            .map(|y| AiTile::new(0, y)),
    );
    starts
}

fn linear_cut_line(
    width: u32,
    height: u32,
    start: AiTile,
    direction: LinearCutDirection,
) -> Vec<AiTile> {
    let mut out = Vec::new();
    let mut x = start.x as i32;
    let mut y = start.y as i32;
    while x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
        out.push(AiTile::new(x as u32, y as u32));
        x += direction.dx;
        y += direction.dy;
    }
    out
}

fn thicken_linear_cut(
    width: u32,
    height: u32,
    passable: &[bool],
    run: &[AiTile],
    direction: LinearCutDirection,
) -> Vec<AiTile> {
    let mut tiles = BTreeSet::new();
    for &tile in run {
        add_passable_cut_tile(
            width,
            height,
            passable,
            tile.x as i32,
            tile.y as i32,
            &mut tiles,
        );
        if direction.thicken_diagonal {
            add_passable_cut_tile(
                width,
                height,
                passable,
                tile.x as i32 + direction.normal_x,
                tile.y as i32 + direction.normal_y,
                &mut tiles,
            );
        }
    }
    tiles.into_iter().collect()
}

fn add_passable_cut_tile(
    width: u32,
    height: u32,
    passable: &[bool],
    x: i32,
    y: i32,
    out: &mut BTreeSet<AiTile>,
) {
    if passable_at_tile(width, height, passable, x, y) {
        out.insert(AiTile::new(x as u32, y as u32));
    }
}

fn low_clearance_score(width: u32, height: u32, clearance: &[u16], tiles: &[AiTile]) -> i32 {
    tiles
        .iter()
        .map(|tile| {
            tile_index(width, height, tile.x, tile.y)
                .and_then(|idx| clearance.get(idx).copied())
                .map(|value| (12_i32 - i32::from(value)).max(0))
                .unwrap_or(0)
        })
        .sum()
}

fn high_clearance_components(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
    min_clearance: u16,
) -> Vec<Vec<AiTile>> {
    let mut visited = vec![false; passable.len()];
    let mut components = Vec::new();
    let mut queue = VecDeque::new();
    for y in 0..height {
        for x in 0..width {
            let Some(start_idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if visited.get(start_idx).copied() == Some(true)
                || passable.get(start_idx).copied() != Some(true)
                || clearance.get(start_idx).copied().unwrap_or(0) < min_clearance
            {
                continue;
            }
            let start = AiTile::new(x, y);
            visited[start_idx] = true;
            queue.push_back(start);
            let mut tiles = Vec::new();
            while let Some(tile) = queue.pop_front() {
                tiles.push(tile);
                for neighbor in cardinal_neighbors(width, height, tile) {
                    let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y)
                    else {
                        continue;
                    };
                    if visited.get(neighbor_idx).copied() == Some(true)
                        || passable.get(neighbor_idx).copied() != Some(true)
                        || clearance.get(neighbor_idx).copied().unwrap_or(0) < min_clearance
                    {
                        continue;
                    }
                    visited[neighbor_idx] = true;
                    queue.push_back(neighbor);
                }
            }
            if tiles.len() as u32 >= REGION_MIN_CORE_TILES {
                components.push(tiles);
            }
        }
    }
    components
}

fn nearest_basin_distances(
    width: u32,
    height: u32,
    passable: &[bool],
    basins: &[Vec<AiTile>],
    max_distance: u32,
) -> Vec<Vec<RegionDistance>> {
    let mut nearest = vec![Vec::new(); passable.len()];
    let mut queue = VecDeque::new();
    for (basin_id, tiles) in basins.iter().enumerate() {
        let basin_id = basin_id as u32;
        for &tile in tiles {
            let Some(idx) = tile_index(width, height, tile.x, tile.y) else {
                continue;
            };
            record_nearest_distance(&mut nearest[idx], basin_id, 0);
            queue.push_back((tile, basin_id, 0_u32));
        }
    }
    while let Some((tile, basin_id, distance)) = queue.pop_front() {
        if distance >= max_distance {
            continue;
        }
        for neighbor in cardinal_neighbors(width, height, tile) {
            let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y) else {
                continue;
            };
            if passable.get(neighbor_idx).copied() != Some(true) {
                continue;
            }
            let next_distance = distance.saturating_add(1);
            if record_nearest_distance(&mut nearest[neighbor_idx], basin_id, next_distance) {
                queue.push_back((neighbor, basin_id, next_distance));
            }
        }
    }
    for entries in &mut nearest {
        entries.sort_unstable_by_key(|entry| (entry.distance, entry.region_id));
        entries.truncate(2);
    }
    nearest
}

fn record_nearest_distance(
    entries: &mut Vec<RegionDistance>,
    region_id: u32,
    distance: u32,
) -> bool {
    if let Some(existing) = entries
        .iter_mut()
        .find(|entry| entry.region_id == region_id)
    {
        if distance >= existing.distance {
            return false;
        }
        existing.distance = distance;
        entries.sort_unstable_by_key(|entry| (entry.distance, entry.region_id));
        return true;
    }
    if entries.len() >= 2
        && entries
            .last()
            .is_some_and(|entry| distance >= entry.distance)
    {
        return false;
    }
    entries.push(RegionDistance {
        region_id,
        distance,
    });
    entries.sort_unstable_by_key(|entry| (entry.distance, entry.region_id));
    entries.truncate(2);
    true
}

fn candidate_touches_map_edge(width: u32, height: u32, candidate: &ChokeCandidate) -> bool {
    candidate.bounds.min.x == 0
        || candidate.bounds.min.y == 0
        || candidate.bounds.max.x >= width.saturating_sub(1)
        || candidate.bounds.max.y >= height.saturating_sub(1)
}

fn local_cut_split_score(
    width: u32,
    height: u32,
    passable: &[bool],
    cut_tiles: &[AiTile],
    normal: (i32, i32),
) -> Option<i32> {
    let cut_set: BTreeSet<_> = cut_tiles.iter().copied().collect();
    let bounds = expanded_bounds(
        width,
        height,
        bounds_for_candidate_tiles(cut_tiles)?,
        GAMEPLAY_LOCAL_CUT_PADDING_TILES,
    );
    let mut component_by_tile: BTreeMap<AiTile, usize> = BTreeMap::new();
    let mut component_sizes = Vec::new();
    let mut queue = VecDeque::new();

    for y in bounds.min.y..=bounds.max.y {
        for x in bounds.min.x..=bounds.max.x {
            let tile = AiTile::new(x, y);
            let Some(idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if passable.get(idx).copied() != Some(true)
                || cut_set.contains(&tile)
                || component_by_tile.contains_key(&tile)
            {
                continue;
            }

            let component_id = component_sizes.len();
            let mut component_size = 0_usize;
            component_by_tile.insert(tile, component_id);
            queue.push_back(tile);
            while let Some(current) = queue.pop_front() {
                component_size = component_size.saturating_add(1);
                for neighbor in passable_neighbors(width, height, passable, current) {
                    if !tile_in_bounds(neighbor, bounds)
                        || cut_set.contains(&neighbor)
                        || component_by_tile.contains_key(&neighbor)
                    {
                        continue;
                    }
                    component_by_tile.insert(neighbor, component_id);
                    queue.push_back(neighbor);
                }
            }
            component_sizes.push(component_size);
        }
    }

    let side_a = best_adjacent_component(
        width,
        height,
        passable,
        cut_tiles,
        &component_by_tile,
        &component_sizes,
        normal,
    )?;
    let side_b = best_adjacent_component(
        width,
        height,
        passable,
        cut_tiles,
        &component_by_tile,
        &component_sizes,
        (-normal.0, -normal.1),
    )?;
    if side_a.0 == side_b.0
        || side_a.1 < GAMEPLAY_LOCAL_CUT_MIN_SIDE_TILES
        || side_b.1 < GAMEPLAY_LOCAL_CUT_MIN_SIDE_TILES
    {
        return None;
    }

    let balance = side_a.1.min(side_b.1);
    let total = side_a.1.saturating_add(side_b.1);
    let score = balance
        .saturating_mul(2)
        .saturating_add(total / 5)
        .min(balance.saturating_mul(2).saturating_add(120));
    Some(score.min(i32::MAX as usize) as i32)
}

fn best_adjacent_component(
    width: u32,
    height: u32,
    passable: &[bool],
    cut_tiles: &[AiTile],
    component_by_tile: &BTreeMap<AiTile, usize>,
    component_sizes: &[usize],
    direction: (i32, i32),
) -> Option<(usize, usize)> {
    let mut contacts: BTreeMap<usize, usize> = BTreeMap::new();
    for &tile in cut_tiles {
        for step in 1..=3_i32 {
            let x = tile.x as i32 + direction.0.saturating_mul(step);
            let y = tile.y as i32 + direction.1.saturating_mul(step);
            if !passable_at_tile(width, height, passable, x, y) {
                continue;
            }
            let neighbor = AiTile::new(x as u32, y as u32);
            let Some(&component_id) = component_by_tile.get(&neighbor) else {
                continue;
            };
            *contacts.entry(component_id).or_default() += 1;
            break;
        }
    }
    contacts
        .into_iter()
        .filter_map(|(component_id, contact_count)| {
            component_sizes
                .get(component_id)
                .copied()
                .map(|size| (component_id, contact_count, size))
        })
        .max_by_key(|(component_id, contact_count, size)| {
            (*contact_count, *size, usize::MAX - *component_id)
        })
        .map(|(component_id, _, size)| (component_id, size))
}

fn direct_cut_score(width: u32, height: u32, candidate: &ChokeCandidate) -> i32 {
    let edge_penalty = if candidate.center.x < 15
        || candidate.center.x > width.saturating_sub(15)
        || candidate.center.y < 10
        || candidate.center.y > height.saturating_sub(10)
    {
        -180
    } else {
        0
    };
    let candidate_width = candidate
        .bounds
        .max
        .x
        .saturating_sub(candidate.bounds.min.x)
        .saturating_add(1);
    let candidate_height = candidate
        .bounds
        .max
        .y
        .saturating_sub(candidate.bounds.min.y)
        .saturating_add(1);
    let side_mouth = candidate.center.x < width.saturating_mul(3) / 10
        || candidate.center.x > width.saturating_mul(7) / 10;
    let orientation_score = if side_mouth && candidate_height >= candidate_width {
        40
    } else {
        0
    };
    edge_penalty + orientation_score
}

fn non_max_choke_candidates(
    mut candidates: Vec<ChokeCandidate>,
    max: usize,
    min_spacing: u32,
) -> Vec<ChokeCandidate> {
    candidates.sort_by_key(|candidate| (-candidate.score, candidate.center.y, candidate.center.x));
    let mut kept = Vec::new();
    for candidate in candidates {
        if kept.iter().any(|other: &ChokeCandidate| {
            tile_distance2(other.center, candidate.center) < min_spacing * min_spacing
        }) {
            continue;
        }
        kept.push(candidate);
        if kept.len() >= max {
            break;
        }
    }
    kept
}

fn choke_from_candidate(
    context: &ChokeBuildContext<'_>,
    candidate: &ChokeCandidate,
    id: u32,
) -> Option<AiMapChoke> {
    let (region_a_id, region_b_id, approach_a_tile, approach_b_tile) =
        candidate_region_pair(context, candidate)?;
    if region_a_id == region_b_id {
        return None;
    }
    let (bounds, min_clearance_tiles, max_clearance_tiles) = choke_tile_stats(
        context.width,
        context.height,
        context.clearance,
        &candidate.tiles,
    );
    let (endpoint_a_tile, endpoint_b_tile, width_tiles) = choke_line_geometry(
        &candidate.tiles,
        candidate.center,
        approach_a_tile,
        approach_b_tile,
        bounds,
    );
    Some(AiMapChoke {
        id,
        region_a_id,
        region_b_id,
        center_tile: candidate.center,
        endpoint_a_tile,
        endpoint_b_tile,
        approach_a_tile,
        approach_b_tile,
        width_tiles,
        tile_count: candidate.tiles.len() as u32,
        tiles: candidate.tiles.clone(),
        bounds,
        min_clearance_tiles,
        max_clearance_tiles,
    })
}

fn candidate_region_pair(
    context: &ChokeBuildContext<'_>,
    candidate: &ChokeCandidate,
) -> Option<(u32, u32, AiTile, AiTile)> {
    let contacts = choke_contacts(
        context.width,
        context.height,
        context.passable,
        context.region_by_tile,
        &candidate.tiles,
    );
    let mut ranked_contacts: Vec<_> = contacts
        .iter()
        .filter_map(|(&region_id, contacts)| {
            best_region_contact(contacts, candidate.center).map(|contact| (region_id, contact))
        })
        .collect();
    ranked_contacts.sort_by_key(|(region_id, contact)| {
        (
            tile_distance2(contact.portal_tile, candidate.center),
            tile_distance2(contact.region_tile, candidate.center),
            *region_id,
        )
    });
    if ranked_contacts.len() >= 2 {
        return Some((
            ranked_contacts[0].0,
            ranked_contacts[1].0,
            ranked_contacts[0].1.region_tile,
            ranked_contacts[1].1.region_tile,
        ));
    }

    if let Some(pair) = candidate_split_region_pair(context, candidate) {
        return Some(pair);
    }

    let (basin_a_id, basin_b_id) = candidate.basin_pair?;
    let region_a = context
        .regions
        .iter()
        .find(|region| region.id == basin_a_id)?;
    let region_b = context
        .regions
        .iter()
        .find(|region| region.id == basin_b_id)?;
    Some((
        region_a.id,
        region_b.id,
        region_a.representative,
        region_b.representative,
    ))
}

fn build_chokes_for_band(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
    region_by_tile: &[Option<u32>],
    band_tiles: &[AiTile],
    chokes: &mut Vec<AiMapChoke>,
) {
    let contacts = choke_contacts(width, height, passable, region_by_tile, band_tiles);
    if contacts.len() < 2 {
        return;
    }

    let band_index = band_tile_index(band_tiles);
    let region_distances: BTreeMap<_, _> = contacts
        .iter()
        .map(|(&region_id, contacts)| {
            let sources = contact_portal_tiles(contacts);
            (
                region_id,
                band_distances(width, height, &band_index, band_tiles.len(), &sources),
            )
        })
        .collect();
    let mut pair_tiles: BTreeMap<(u32, u32), Vec<(AiTile, u32)>> = BTreeMap::new();
    for (tile_idx, &tile) in band_tiles.iter().enumerate() {
        let Some((region_a, region_b, pair_distance)) =
            nearest_region_pair_for_band_tile(&region_distances, tile_idx)
        else {
            continue;
        };
        pair_tiles
            .entry(ordered_pair(region_a, region_b))
            .or_default()
            .push((tile, pair_distance));
    }

    for ((region_a_id, region_b_id), tiles_with_distance) in pair_tiles {
        let distance_by_tile: BTreeMap<_, _> = tiles_with_distance.into_iter().collect();
        let pair_tile_set: BTreeSet<_> = distance_by_tile.keys().copied().collect();
        for pair_group in connected_tile_groups(width, height, &pair_tile_set) {
            let Some(shortest_pair_distance) = pair_group
                .iter()
                .filter_map(|tile| distance_by_tile.get(tile).copied())
                .min()
            else {
                continue;
            };
            let corridor_tiles: BTreeSet<_> = pair_group
                .into_iter()
                .filter(|tile| {
                    distance_by_tile.get(tile).copied().is_some_and(|distance| {
                        distance
                            <= shortest_pair_distance.saturating_add(CHOKE_PAIR_PATH_SLACK_TILES)
                    })
                })
                .collect();
            if corridor_tiles.is_empty() {
                continue;
            }

            for tiles in connected_tile_groups(width, height, &corridor_tiles) {
                if tiles.len() as u32 > CHOKE_MAX_BAND_TILES
                    || (tiles.len() as u32) < CHOKE_MIN_BAND_TILES
                {
                    continue;
                }

                let contacts = choke_contacts(width, height, passable, region_by_tile, &tiles);
                let Some(contact_a) = contacts
                    .get(&region_a_id)
                    .and_then(|items| best_region_contact(items, center_tile_for_tiles(&tiles)))
                else {
                    continue;
                };
                let center_tile = center_tile_for_tiles(&tiles);
                let Some(contact_b) = contacts
                    .get(&region_b_id)
                    .and_then(|items| best_region_contact(items, center_tile))
                else {
                    continue;
                };
                let (bounds, min_clearance_tiles, max_clearance_tiles) =
                    choke_tile_stats(width, height, clearance, &tiles);
                let (endpoint_a_tile, endpoint_b_tile, width_tiles) = choke_line_geometry(
                    &tiles,
                    center_tile,
                    contact_a.region_tile,
                    contact_b.region_tile,
                    bounds,
                );
                let id = chokes.len() as u32;
                chokes.push(AiMapChoke {
                    id,
                    region_a_id,
                    region_b_id,
                    center_tile,
                    endpoint_a_tile,
                    endpoint_b_tile,
                    approach_a_tile: contact_a.region_tile,
                    approach_b_tile: contact_b.region_tile,
                    width_tiles,
                    tile_count: tiles.len() as u32,
                    tiles,
                    bounds,
                    min_clearance_tiles,
                    max_clearance_tiles,
                });
            }
        }
    }
}

fn is_choke_band_tile(passable: &[bool], region_by_tile: &[Option<u32>], idx: usize) -> bool {
    passable.get(idx).copied() == Some(true) && region_by_tile.get(idx).copied().flatten().is_none()
}

fn band_tile_index(tiles: &[AiTile]) -> BTreeMap<AiTile, usize> {
    tiles
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, tile)| (tile, idx))
        .collect()
}

fn contact_portal_tiles(contacts: &[RegionContact]) -> Vec<AiTile> {
    let mut portals: Vec<_> = contacts.iter().map(|contact| contact.portal_tile).collect();
    portals.sort_unstable_by_key(|tile| (tile.y, tile.x));
    portals.dedup();
    portals
}

fn band_distances(
    width: u32,
    height: u32,
    band_index: &BTreeMap<AiTile, usize>,
    tile_count: usize,
    sources: &[AiTile],
) -> Vec<Option<u32>> {
    let mut distances: Vec<Option<u32>> = vec![None; tile_count];
    let mut queue = VecDeque::new();

    for &source in sources {
        let Some(&source_idx) = band_index.get(&source) else {
            continue;
        };
        if distances[source_idx].is_some() {
            continue;
        }
        distances[source_idx] = Some(0);
        queue.push_back(source);
    }

    while let Some(tile) = queue.pop_front() {
        let Some(&tile_idx) = band_index.get(&tile) else {
            continue;
        };
        let Some(distance) = distances[tile_idx] else {
            continue;
        };
        for neighbor in cardinal_neighbors(width, height, tile) {
            let Some(&neighbor_idx) = band_index.get(&neighbor) else {
                continue;
            };
            if distances[neighbor_idx].is_some() {
                continue;
            }
            distances[neighbor_idx] = Some(distance.saturating_add(1));
            queue.push_back(neighbor);
        }
    }

    distances
}

fn nearest_region_pair_for_band_tile(
    region_distances: &BTreeMap<u32, Vec<Option<u32>>>,
    tile_idx: usize,
) -> Option<(u32, u32, u32)> {
    let mut nearest: Vec<_> = region_distances
        .iter()
        .filter_map(|(&region_id, distances)| {
            distances
                .get(tile_idx)
                .copied()
                .flatten()
                .map(|distance| RegionDistance {
                    region_id,
                    distance,
                })
        })
        .collect();
    if nearest.len() < 2 {
        return None;
    }
    nearest.sort_unstable_by_key(|entry| (entry.distance, entry.region_id));
    let a = nearest[0];
    let b = nearest[1];
    Some((
        a.region_id,
        b.region_id,
        a.distance.saturating_add(b.distance),
    ))
}

fn ordered_pair(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn connected_tile_groups(width: u32, height: u32, tiles: &BTreeSet<AiTile>) -> Vec<Vec<AiTile>> {
    let mut groups = Vec::new();
    let mut remaining = tiles.clone();
    let mut queue = VecDeque::new();

    while let Some(&start) = remaining.iter().next() {
        remaining.remove(&start);
        let mut group = Vec::new();
        queue.push_back(start);
        while let Some(tile) = queue.pop_front() {
            group.push(tile);
            for neighbor in cardinal_neighbors(width, height, tile) {
                if remaining.remove(&neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        group.sort_unstable_by_key(|tile| (tile.y, tile.x));
        groups.push(group);
    }

    groups.sort_by_key(|group| {
        let center = center_tile_for_tiles(group);
        (center.y, center.x, group.len())
    });
    groups
}

fn connected_tile_groups_8(width: u32, height: u32, tiles: &BTreeSet<AiTile>) -> Vec<Vec<AiTile>> {
    let mut groups = Vec::new();
    let mut remaining = tiles.clone();
    let mut queue = VecDeque::new();

    while let Some(&start) = remaining.iter().next() {
        remaining.remove(&start);
        let mut group = Vec::new();
        queue.push_back(start);
        while let Some(tile) = queue.pop_front() {
            group.push(tile);
            for neighbor in all_neighbors(width, height, tile) {
                if remaining.remove(&neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        group.sort_unstable_by_key(|tile| (tile.y, tile.x));
        groups.push(group);
    }

    groups.sort_by_key(|group| {
        let center = center_tile_for_tiles(group);
        (center.y, center.x, group.len())
    });
    groups
}

fn all_neighbors(width: u32, height: u32, tile: AiTile) -> Vec<AiTile> {
    let mut out = Vec::with_capacity(8);
    let x = tile.x as i32;
    let y = tile.y as i32;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || ny < 0 || nx as u32 >= width || ny as u32 >= height {
                continue;
            }
            out.push(AiTile::new(nx as u32, ny as u32));
        }
    }
    out
}

fn candidate_from_tiles(mut tiles: Vec<AiTile>, score: i32) -> Option<ChokeCandidate> {
    tiles.sort_unstable_by_key(|tile| (tile.y, tile.x));
    tiles.dedup();
    let bounds = bounds_for_candidate_tiles(&tiles)?;
    let center = nearest_tile_to(
        &tiles,
        AiTile::new(
            (bounds.min.x.saturating_add(bounds.max.x)) / 2,
            (bounds.min.y.saturating_add(bounds.max.y)) / 2,
        ),
    );
    Some(ChokeCandidate {
        center,
        bounds,
        tiles,
        score,
        basin_pair: None,
    })
}

fn bounds_for_candidate_tiles(tiles: &[AiTile]) -> Option<AiTileBounds> {
    let (&first, rest) = tiles.split_first()?;
    let mut bounds = AiTileBounds::new(first);
    for &tile in rest {
        bounds.include(tile);
    }
    Some(bounds)
}

fn expanded_bounds(width: u32, height: u32, bounds: AiTileBounds, amount: u32) -> AiTileBounds {
    AiTileBounds {
        min: AiTile::new(
            bounds.min.x.saturating_sub(amount),
            bounds.min.y.saturating_sub(amount),
        ),
        max: AiTile::new(
            bounds
                .max
                .x
                .saturating_add(amount)
                .min(width.saturating_sub(1)),
            bounds
                .max
                .y
                .saturating_add(amount)
                .min(height.saturating_sub(1)),
        ),
    }
}

fn passable_at_tile(width: u32, height: u32, passable: &[bool], x: i32, y: i32) -> bool {
    if x < 0 || y < 0 {
        return false;
    }
    tile_index(width, height, x as u32, y as u32)
        .and_then(|idx| passable.get(idx).copied())
        .unwrap_or(false)
}

fn choke_tile_stats(
    width: u32,
    height: u32,
    clearance: &[u16],
    tiles: &[AiTile],
) -> (AiTileBounds, u16, u16) {
    let Some((&first, rest)) = tiles.split_first() else {
        return (AiTileBounds::new(AiTile::new(0, 0)), 0, 0);
    };
    let mut bounds = AiTileBounds::new(first);
    let mut min_clearance_tiles = u16::MAX;
    let mut max_clearance_tiles = 0;
    for &tile in std::iter::once(&first).chain(rest.iter()) {
        bounds.include(tile);
        let tile_clearance = tile_index(width, height, tile.x, tile.y)
            .and_then(|idx| clearance.get(idx).copied())
            .unwrap_or(0);
        min_clearance_tiles = min_clearance_tiles.min(tile_clearance);
        max_clearance_tiles = max_clearance_tiles.max(tile_clearance);
    }
    (bounds, min_clearance_tiles, max_clearance_tiles)
}

fn choke_contacts(
    width: u32,
    height: u32,
    passable: &[bool],
    region_by_tile: &[Option<u32>],
    tiles: &[AiTile],
) -> BTreeMap<u32, Vec<RegionContact>> {
    let mut contacts: BTreeMap<u32, Vec<RegionContact>> = BTreeMap::new();
    for &portal_tile in tiles {
        for (region_id, contact) in
            nearby_region_contacts(width, height, passable, region_by_tile, portal_tile)
        {
            contacts.entry(region_id).or_default().push(contact);
        }
    }
    contacts
}

fn nearby_region_contacts(
    width: u32,
    height: u32,
    passable: &[bool],
    region_by_tile: &[Option<u32>],
    portal_tile: AiTile,
) -> BTreeMap<u32, RegionContact> {
    let mut contacts = BTreeMap::new();
    let mut visited = vec![false; passable.len()];
    let mut queue = VecDeque::new();
    let Some(start_idx) = tile_index(width, height, portal_tile.x, portal_tile.y) else {
        return contacts;
    };
    if passable.get(start_idx).copied() != Some(true) {
        return contacts;
    }
    visited[start_idx] = true;
    queue.push_back((portal_tile, 0_u16));

    while let Some((tile, distance)) = queue.pop_front() {
        let Some(idx) = tile_index(width, height, tile.x, tile.y) else {
            continue;
        };
        if let Some(region_id) = region_by_tile.get(idx).copied().flatten() {
            contacts.entry(region_id).or_insert(RegionContact {
                region_tile: tile,
                portal_tile,
            });
            continue;
        }
        if distance >= CHOKE_CONTACT_RADIUS_TILES {
            continue;
        }
        for neighbor in cardinal_neighbors(width, height, tile) {
            let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y) else {
                continue;
            };
            if visited.get(neighbor_idx).copied() == Some(true)
                || passable.get(neighbor_idx).copied() != Some(true)
            {
                continue;
            }
            visited[neighbor_idx] = true;
            queue.push_back((neighbor, distance.saturating_add(1)));
        }
    }

    contacts
}

fn best_region_contact(contacts: &[RegionContact], center_tile: AiTile) -> Option<RegionContact> {
    contacts.iter().copied().min_by_key(|contact| {
        (
            tile_distance2(contact.portal_tile, center_tile),
            tile_distance2(contact.region_tile, center_tile),
            contact.region_tile.y,
            contact.region_tile.x,
            contact.portal_tile.y,
            contact.portal_tile.x,
        )
    })
}

fn cardinal_neighbors(width: u32, height: u32, tile: AiTile) -> Vec<AiTile> {
    let mut out = Vec::with_capacity(4);
    let x = tile.x as i32;
    let y = tile.y as i32;
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let nx = x + dx;
        let ny = y + dy;
        if nx < 0 || ny < 0 || nx as u32 >= width || ny as u32 >= height {
            continue;
        }
        out.push(AiTile::new(nx as u32, ny as u32));
    }
    out
}

fn center_tile_for_tiles(tiles: &[AiTile]) -> AiTile {
    if tiles.is_empty() {
        return AiTile::new(0, 0);
    }
    let sum_x: u64 = tiles.iter().map(|tile| u64::from(tile.x)).sum();
    let sum_y: u64 = tiles.iter().map(|tile| u64::from(tile.y)).sum();
    let len = tiles.len() as u64;
    let target = AiTile::new((sum_x / len) as u32, (sum_y / len) as u32);
    nearest_tile_to(tiles, target)
}

fn nearest_tile_to(tiles: &[AiTile], target: AiTile) -> AiTile {
    tiles
        .iter()
        .copied()
        .min_by_key(|tile| (tile_distance2(*tile, target), tile.y, tile.x))
        .unwrap_or(target)
}
