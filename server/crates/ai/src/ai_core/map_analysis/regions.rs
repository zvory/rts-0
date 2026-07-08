use std::collections::{BTreeMap, VecDeque};

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

#[derive(Clone, Copy, Debug)]
struct RegionContact {
    region_tile: AiTile,
    portal_tile: AiTile,
}

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

pub(super) fn build_chokes(
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
                || !is_choke_seed_tile(
                    width,
                    height,
                    passable,
                    region_by_tile,
                    start_idx,
                    start_tile,
                )
            {
                continue;
            }

            let mut tiles = Vec::new();
            let mut bounds = AiTileBounds::new(start_tile);
            let mut min_clearance_tiles = u16::MAX;
            let mut max_clearance_tiles = 0;

            visited[start_idx] = true;
            queue.push_back(start_tile);
            while let Some(tile) = queue.pop_front() {
                let Some(idx) = tile_index(width, height, tile.x, tile.y) else {
                    continue;
                };
                let tile_clearance = clearance.get(idx).copied().unwrap_or(0);
                bounds.include(tile);
                min_clearance_tiles = min_clearance_tiles.min(tile_clearance);
                max_clearance_tiles = max_clearance_tiles.max(tile_clearance);
                tiles.push(tile);

                for neighbor in cardinal_neighbors(width, height, tile) {
                    let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y)
                    else {
                        continue;
                    };
                    if visited.get(neighbor_idx).copied() == Some(true)
                        || !is_choke_seed_tile(
                            width,
                            height,
                            passable,
                            region_by_tile,
                            neighbor_idx,
                            neighbor,
                        )
                    {
                        continue;
                    }
                    visited[neighbor_idx] = true;
                    queue.push_back(neighbor);
                }
            }

            if tiles.len() as u32 > CHOKE_MAX_BAND_TILES {
                continue;
            }

            let contacts = choke_contacts(width, height, passable, region_by_tile, &tiles);
            if contacts.len() < 2 || contacts.len() > CHOKE_MAX_ADJACENT_REGIONS {
                continue;
            }

            let center_tile = center_tile_for_tiles(&tiles);
            let (endpoint_a_tile, endpoint_b_tile, width_tiles) =
                choke_segment_endpoints(&tiles, bounds);
            let adjacent_regions: Vec<_> = contacts.keys().copied().collect();
            for (idx, &region_a_id) in adjacent_regions.iter().enumerate() {
                for &region_b_id in adjacent_regions.iter().skip(idx + 1) {
                    let Some(contact_a) = contacts
                        .get(&region_a_id)
                        .and_then(|items| best_region_contact(items, center_tile))
                    else {
                        continue;
                    };
                    let Some(contact_b) = contacts
                        .get(&region_b_id)
                        .and_then(|items| best_region_contact(items, center_tile))
                    else {
                        continue;
                    };
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
                        tiles: tiles.clone(),
                        bounds,
                        min_clearance_tiles,
                        max_clearance_tiles,
                    });
                }
            }
        }
    }

    chokes
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

fn is_choke_band_tile(passable: &[bool], region_by_tile: &[Option<u32>], idx: usize) -> bool {
    passable.get(idx).copied() == Some(true) && region_by_tile.get(idx).copied().flatten().is_none()
}

fn is_choke_seed_tile(
    width: u32,
    height: u32,
    passable: &[bool],
    region_by_tile: &[Option<u32>],
    idx: usize,
    tile: AiTile,
) -> bool {
    is_choke_band_tile(passable, region_by_tile, idx)
        && nearby_region_contacts(width, height, passable, region_by_tile, tile).len() >= 2
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

fn choke_segment_endpoints(tiles: &[AiTile], bounds: AiTileBounds) -> (AiTile, AiTile, u16) {
    let span_x = bounds.max.x.saturating_sub(bounds.min.x).saturating_add(1);
    let span_y = bounds.max.y.saturating_sub(bounds.min.y).saturating_add(1);
    if span_x >= span_y {
        let x = bounds.min.x + span_x / 2;
        (
            nearest_tile_to(tiles, AiTile::new(x, bounds.min.y)),
            nearest_tile_to(tiles, AiTile::new(x, bounds.max.y)),
            span_y.min(u32::from(u16::MAX)) as u16,
        )
    } else {
        let y = bounds.min.y + span_y / 2;
        (
            nearest_tile_to(tiles, AiTile::new(bounds.min.x, y)),
            nearest_tile_to(tiles, AiTile::new(bounds.max.x, y)),
            span_x.min(u32::from(u16::MAX)) as u16,
        )
    }
}

fn nearest_tile_to(tiles: &[AiTile], target: AiTile) -> AiTile {
    tiles
        .iter()
        .copied()
        .min_by_key(|tile| (tile_distance2(*tile, target), tile.y, tile.x))
        .unwrap_or(target)
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

pub(super) fn region_tile_rects(
    width: u32,
    height: u32,
    region_by_tile: &[Option<u32>],
    region_id: u32,
) -> Vec<OverlayTileRect> {
    let mut rects: Vec<OverlayTileRect> = Vec::new();
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let Some(idx) = tile_index(width, height, x, y) else {
                x = x.saturating_add(1);
                continue;
            };
            if region_by_tile.get(idx).copied().flatten() != Some(region_id) {
                x = x.saturating_add(1);
                continue;
            }

            let start_x = x;
            while x < width {
                let Some(idx) = tile_index(width, height, x, y) else {
                    break;
                };
                if region_by_tile.get(idx).copied().flatten() != Some(region_id) {
                    break;
                }
                x = x.saturating_add(1);
            }
            let tile_w = x.saturating_sub(start_x);
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
    }
    rects
}

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

        while idx < sorted.len() && sorted[idx].y == y && sorted[idx].x == end_x.saturating_add(1)
        {
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
