use std::collections::VecDeque;

use super::*;

#[derive(Clone, Debug)]
struct RegionSeed {
    id: u32,
    component_id: Option<u32>,
    core_tiles: Vec<AiTile>,
    bounds: AiTileBounds,
    representative: AiTile,
    max_clearance_tiles: u16,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct OverlayTileRect {
    pub(super) tile_x: u32,
    pub(super) tile_y: u32,
    pub(super) tile_w: u32,
    pub(super) tile_h: u32,
}

pub(super) fn build_regions(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
    component_by_tile: &[Option<u32>],
) -> (Vec<Option<u32>>, Vec<AiMapRegion>) {
    let mut region_by_tile = vec![None; passable.len()];
    let mut visited = vec![false; passable.len()];
    let mut seeds = Vec::new();
    let mut queue = VecDeque::new();

    for y in 0..height {
        for x in 0..width {
            let Some(start_idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if visited.get(start_idx).copied() == Some(true)
                || passable.get(start_idx).copied() != Some(true)
                || clearance.get(start_idx).copied().unwrap_or(0) < REGION_CORE_CLEARANCE_TILES
            {
                continue;
            }

            let start_tile = AiTile::new(x, y);
            let mut seed = RegionSeed {
                id: 0,
                component_id: component_by_tile.get(start_idx).copied().flatten(),
                core_tiles: Vec::new(),
                bounds: AiTileBounds::new(start_tile),
                representative: start_tile,
                max_clearance_tiles: 0,
            };
            visited[start_idx] = true;
            queue.push_back(start_tile);

            while let Some(tile) = queue.pop_front() {
                let Some(idx) = tile_index(width, height, tile.x, tile.y) else {
                    continue;
                };
                let tile_clearance = clearance.get(idx).copied().unwrap_or(0);
                seed.bounds.include(tile);
                if region_representative_better(
                    tile,
                    tile_clearance,
                    seed.representative,
                    seed.max_clearance_tiles,
                ) {
                    seed.representative = tile;
                }
                seed.max_clearance_tiles = seed.max_clearance_tiles.max(tile_clearance);
                seed.core_tiles.push(tile);

                for neighbor in passable_neighbors(width, height, passable, tile) {
                    let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y)
                    else {
                        continue;
                    };
                    if visited.get(neighbor_idx).copied() == Some(true)
                        || clearance.get(neighbor_idx).copied().unwrap_or(0)
                            < REGION_CORE_CLEARANCE_TILES
                    {
                        continue;
                    }
                    visited[neighbor_idx] = true;
                    queue.push_back(neighbor);
                }
            }

            if seed.core_tiles.len() as u32 >= REGION_MIN_CORE_TILES {
                seed.id = seeds.len() as u32;
                seeds.push(seed);
            }
        }
    }

    if seeds.is_empty() {
        return (region_by_tile, Vec::new());
    }

    let mut grow_queue = VecDeque::new();
    for seed in &seeds {
        for tile in &seed.core_tiles {
            if let Some(idx) = tile_index(width, height, tile.x, tile.y) {
                region_by_tile[idx] = Some(seed.id);
                grow_queue.push_back((*tile, seed.id));
            }
        }
    }

    while let Some((tile, region_id)) = grow_queue.pop_front() {
        for neighbor in passable_neighbors(width, height, passable, tile) {
            let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y) else {
                continue;
            };
            if region_by_tile
                .get(neighbor_idx)
                .copied()
                .flatten()
                .is_some()
                || clearance.get(neighbor_idx).copied().unwrap_or(0) < REGION_BODY_CLEARANCE_TILES
            {
                continue;
            }
            region_by_tile[neighbor_idx] = Some(region_id);
            grow_queue.push_back((neighbor, region_id));
        }
    }

    let mut regions: Vec<_> = seeds
        .iter()
        .map(|seed| AiMapRegion {
            id: seed.id,
            component_id: seed.component_id,
            tile_count: 0,
            core_tile_count: seed.core_tiles.len() as u32,
            bounds: AiTileBounds::new(seed.representative),
            representative: seed.representative,
            max_clearance_tiles: seed.max_clearance_tiles,
        })
        .collect();

    for y in 0..height {
        for x in 0..width {
            let Some(idx) = tile_index(width, height, x, y) else {
                continue;
            };
            let Some(region_id) = region_by_tile.get(idx).copied().flatten() else {
                continue;
            };
            let Some(region) = regions.get_mut(region_id as usize) else {
                continue;
            };
            let tile = AiTile::new(x, y);
            let tile_clearance = clearance.get(idx).copied().unwrap_or(0);
            if region.tile_count == 0 {
                region.bounds = AiTileBounds::new(tile);
            } else {
                region.bounds.include(tile);
            }
            region.tile_count = region.tile_count.saturating_add(1);
            if region_representative_better(
                tile,
                tile_clearance,
                region.representative,
                region.max_clearance_tiles,
            ) {
                region.representative = tile;
            }
            region.max_clearance_tiles = region.max_clearance_tiles.max(tile_clearance);
        }
    }

    (region_by_tile, regions)
}

pub(super) fn region_id_for_tile(
    width: u32,
    height: u32,
    region_by_tile: &[Option<u32>],
    tile: AiTile,
) -> Option<u32> {
    tile_index(width, height, tile.x, tile.y)
        .and_then(|idx| region_by_tile.get(idx).copied().flatten())
}

pub(super) fn nearest_region(
    tile: AiTile,
    component_id: Option<u32>,
    regions: &[AiMapRegion],
) -> Option<(u32, u32)> {
    let same_component = regions
        .iter()
        .filter(|region| same_component_or_unknown(component_id, region.component_id))
        .map(|region| {
            (
                region.id,
                tile_distance2(tile, region.representative),
                region.representative,
            )
        })
        .min_by_key(|(id, distance2, representative)| {
            (*distance2, representative.y, representative.x, *id)
        });
    same_component
        .or_else(|| {
            regions
                .iter()
                .map(|region| {
                    (
                        region.id,
                        tile_distance2(tile, region.representative),
                        region.representative,
                    )
                })
                .min_by_key(|(id, distance2, representative)| {
                    (*distance2, representative.y, representative.x, *id)
                })
        })
        .map(|(id, distance2, _)| (id, distance2))
}

fn region_representative_better(
    tile: AiTile,
    tile_clearance: u16,
    representative: AiTile,
    representative_clearance: u16,
) -> bool {
    tile_clearance > representative_clearance
        || (tile_clearance == representative_clearance
            && (tile.y, tile.x) < (representative.y, representative.x))
}

#[cfg(test)]
pub(super) fn tile_rects_for_tiles(tiles: &[AiTile]) -> Vec<OverlayTileRect> {
    let mut sorted = tiles.to_vec();
    sorted.sort_unstable_by_key(|tile| (tile.y, tile.x));
    sorted.dedup();

    let mut rects: Vec<OverlayTileRect> = Vec::new();
    let mut idx = 0;
    while idx < sorted.len() {
        let y = sorted[idx].y;
        let start_x = sorted[idx].x;
        let mut end_x = start_x;
        idx += 1;

        while idx < sorted.len() && sorted[idx].y == y && sorted[idx].x == end_x.saturating_add(1) {
            end_x = sorted[idx].x;
            idx += 1;
        }

        let tile_w = end_x.saturating_sub(start_x).saturating_add(1);
        if let Some(last) = rects.last_mut() {
            if last.tile_x == start_x
                && last.tile_w == tile_w
                && last.tile_y.saturating_add(last.tile_h) == y
            {
                last.tile_h = last.tile_h.saturating_add(1);
                continue;
            }
        }
        rects.push(OverlayTileRect {
            tile_x: start_x,
            tile_y: y,
            tile_w,
            tile_h: 1,
        });
    }

    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_rects_for_tiles_compacts_scrambled_tiles_row_major() {
        let rects = tile_rects_for_tiles(&[
            AiTile::new(5, 4),
            AiTile::new(3, 3),
            AiTile::new(4, 3),
            AiTile::new(3, 4),
            AiTile::new(4, 4),
            AiTile::new(4, 4),
            AiTile::new(8, 4),
            AiTile::new(8, 5),
        ]);

        let actual: Vec<_> = rects
            .iter()
            .map(|rect| (rect.tile_x, rect.tile_y, rect.tile_w, rect.tile_h))
            .collect();
        assert_eq!(actual, vec![(3, 3, 2, 1), (3, 4, 3, 1), (8, 4, 1, 2)]);
    }
}
