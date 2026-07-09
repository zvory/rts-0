use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::min_cut::tile_in_bounds;
use super::*;

#[derive(Clone, Copy, Debug)]
struct CandidateRegionSide {
    region_id: u32,
    region_tile: AiTile,
    tile_count: usize,
    distance2: u32,
}

pub(super) fn candidate_split_region_pair(
    context: &ChokeBuildContext<'_>,
    candidate: &ChokeCandidate,
) -> Option<(u32, u32, AiTile, AiTile)> {
    let cut_tiles: BTreeSet<_> = candidate.tiles.iter().copied().collect();
    let bounds = expanded_bounds(
        context.width,
        context.height,
        candidate.bounds,
        GAMEPLAY_LOCAL_CUT_PADDING_TILES,
    );
    let mut component_by_tile: BTreeMap<AiTile, usize> = BTreeMap::new();
    let mut sides = Vec::new();
    let mut queue = VecDeque::new();

    for &cut_tile in &candidate.tiles {
        for start in cardinal_neighbors(context.width, context.height, cut_tile) {
            if !tile_in_bounds(start, bounds)
                || cut_tiles.contains(&start)
                || !passable_tile(context.width, context.height, context.passable, start)
                || component_by_tile.contains_key(&start)
            {
                continue;
            }

            let component_id = component_by_tile.len();
            let mut tile_count = 0_usize;
            let mut best_region: Option<(u32, AiTile, u32)> = None;
            component_by_tile.insert(start, component_id);
            queue.push_back(start);

            while let Some(tile) = queue.pop_front() {
                tile_count = tile_count.saturating_add(1);
                if let Some(region_id) =
                    region_id_for_tile(context.width, context.height, context.region_by_tile, tile)
                {
                    let distance2 = tile_distance2(tile, candidate.center);
                    let replace = best_region
                        .map(|(best_id, best_tile, best_distance2)| {
                            (distance2, tile.y, tile.x, region_id)
                                < (best_distance2, best_tile.y, best_tile.x, best_id)
                        })
                        .unwrap_or(true);
                    if replace {
                        best_region = Some((region_id, tile, distance2));
                    }
                }

                for neighbor in cardinal_neighbors(context.width, context.height, tile) {
                    if !tile_in_bounds(neighbor, bounds)
                        || cut_tiles.contains(&neighbor)
                        || !passable_tile(context.width, context.height, context.passable, neighbor)
                        || component_by_tile.contains_key(&neighbor)
                    {
                        continue;
                    }
                    component_by_tile.insert(neighbor, component_id);
                    queue.push_back(neighbor);
                }
            }

            if let Some((region_id, region_tile, distance2)) = best_region {
                sides.push(CandidateRegionSide {
                    region_id,
                    region_tile,
                    tile_count,
                    distance2,
                });
            }
        }
    }

    sides.sort_by_key(|side| {
        (
            usize::MAX.saturating_sub(side.tile_count),
            side.distance2,
            side.region_tile.y,
            side.region_tile.x,
            side.region_id,
        )
    });
    for (idx, side_a) in sides.iter().enumerate() {
        for side_b in sides.iter().skip(idx + 1) {
            if side_a.region_id == side_b.region_id {
                continue;
            }
            return Some((
                side_a.region_id,
                side_b.region_id,
                side_a.region_tile,
                side_b.region_tile,
            ));
        }
    }
    None
}

fn passable_tile(width: u32, height: u32, passable: &[bool], tile: AiTile) -> bool {
    tile_index(width, height, tile.x, tile.y)
        .and_then(|idx| passable.get(idx).copied())
        .unwrap_or(false)
}
