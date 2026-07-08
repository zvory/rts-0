use std::collections::VecDeque;

use super::*;

const VORONOI_NEAREST_SITE_LIMIT: usize = 8;
const VORONOI_MIN_BOUNDARY_DISTANCE_TILES: u16 = 2;
const VORONOI_MAX_PAIR_DISTANCE_DELTA_TILES: u16 = 2;
const VORONOI_MIN_GROUP_TILES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoundarySite {
    id: u32,
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoundaryDistance {
    site: BoundarySite,
    distance: u16,
}

pub(super) fn build_voronoi_skeleton(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
) -> Vec<AiMapVoronoiTile> {
    if width == 0 || height == 0 || passable.is_empty() {
        return Vec::new();
    }

    let mut nearest = vec![Vec::<BoundaryDistance>::new(); passable.len()];
    let blocked_site_ids = build_blocked_site_ids(width, height, passable);
    let mut queue = VecDeque::new();

    for y in 0..height {
        for x in 0..width {
            let tile = AiTile::new(x, y);
            let Some(idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if passable.get(idx).copied() != Some(true) {
                continue;
            }

            for site in adjacent_boundary_sites(width, height, passable, &blocked_site_ids, tile) {
                let entry = BoundaryDistance { site, distance: 1 };
                if record_boundary_distance(&mut nearest[idx], entry) {
                    queue.push_back((tile, entry));
                }
            }
        }
    }

    while let Some((tile, entry)) = queue.pop_front() {
        let next_distance = entry.distance.saturating_add(1);
        for neighbor in cardinal_passable_neighbors(width, height, passable, tile) {
            let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y) else {
                continue;
            };
            let next = BoundaryDistance {
                site: entry.site,
                distance: next_distance,
            };
            if record_boundary_distance(&mut nearest[neighbor_idx], next) {
                queue.push_back((neighbor, next));
            }
        }
    }

    let mut medial_distances = vec![None; passable.len()];
    for y in 0..height {
        for x in 0..width {
            let tile = AiTile::new(x, y);
            let Some(idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if passable.get(idx).copied() != Some(true) {
                continue;
            }
            let tile_clearance = clearance.get(idx).copied().unwrap_or(0);
            if tile_clearance < VORONOI_MIN_BOUNDARY_DISTANCE_TILES {
                continue;
            }
            let Some(boundary_distance_tiles) = best_medial_pair_distance(tile, &nearest[idx])
            else {
                continue;
            };
            medial_distances[idx] = Some(boundary_distance_tiles);
        }
    }

    let mut skeleton = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let tile = AiTile::new(x, y);
            let Some(idx) = tile_index(width, height, x, y) else {
                continue;
            };
            let Some(boundary_distance_tiles) = medial_distances.get(idx).copied().flatten()
            else {
                continue;
            };
            if !is_medial_ridge(width, height, passable, &medial_distances, tile, boundary_distance_tiles) {
                continue;
            }
            skeleton.push(AiMapVoronoiTile {
                tile,
                clearance_tiles: clearance.get(idx).copied().unwrap_or(0),
                boundary_distance_tiles,
            });
        }
    }

    prune_short_skeleton_groups(width, height, passable, skeleton)
}

fn adjacent_boundary_sites(
    width: u32,
    height: u32,
    passable: &[bool],
    blocked_site_ids: &[Option<u32>],
    tile: AiTile,
) -> Vec<BoundarySite> {
    let mut out = Vec::with_capacity(4);
    let x = tile.x as i32;
    let y = tile.y as i32;
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let nx = x + dx;
        let ny = y + dy;
        if nx < 0 || ny < 0 || nx as u32 >= width || ny as u32 >= height {
            out.push(BoundarySite {
                id: edge_site_id(nx, ny, width, height),
                x: nx,
                y: ny,
            });
            continue;
        }
        let Some(idx) = tile_index(width, height, nx as u32, ny as u32) else {
            continue;
        };
        if passable.get(idx).copied() != Some(true) {
            let Some(id) = blocked_site_ids.get(idx).copied().flatten() else {
                continue;
            };
            out.push(BoundarySite { id, x: nx, y: ny });
        }
    }
    out.sort_unstable_by_key(|site| (site.id, site.y, site.x));
    out.dedup();
    out
}

fn build_blocked_site_ids(width: u32, height: u32, passable: &[bool]) -> Vec<Option<u32>> {
    let mut site_ids = vec![None; passable.len()];
    let mut next_id = 4_u32;
    let mut queue = VecDeque::new();

    for y in 0..height {
        for x in 0..width {
            let Some(start_idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if passable.get(start_idx).copied() == Some(true) || site_ids[start_idx].is_some() {
                continue;
            }

            let site_id = next_id;
            next_id = next_id.saturating_add(1);
            site_ids[start_idx] = Some(site_id);
            queue.push_back(AiTile::new(x, y));
            while let Some(tile) = queue.pop_front() {
                for neighbor in cardinal_grid_neighbors(width, height, tile) {
                    let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y)
                    else {
                        continue;
                    };
                    if passable.get(neighbor_idx).copied() == Some(true)
                        || site_ids[neighbor_idx].is_some()
                    {
                        continue;
                    }
                    site_ids[neighbor_idx] = Some(site_id);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    site_ids
}

fn edge_site_id(x: i32, y: i32, width: u32, height: u32) -> u32 {
    if y < 0 {
        0
    } else if y >= height as i32 {
        1
    } else if x < 0 {
        2
    } else if x >= width as i32 {
        3
    } else {
        0
    }
}

fn record_boundary_distance(
    entries: &mut Vec<BoundaryDistance>,
    candidate: BoundaryDistance,
) -> bool {
    if let Some(existing) = entries
        .iter_mut()
        .find(|entry| entry.site.id == candidate.site.id)
    {
        if boundary_distance_key(candidate) >= boundary_distance_key(*existing) {
            return false;
        }
        *existing = candidate;
        sort_boundary_distances(entries);
        return true;
    }

    if entries.len() < VORONOI_NEAREST_SITE_LIMIT {
        entries.push(candidate);
        sort_boundary_distances(entries);
        return true;
    }

    sort_boundary_distances(entries);
    let Some(worst) = entries.last().copied() else {
        return false;
    };
    if boundary_distance_key(candidate) >= boundary_distance_key(worst) {
        return false;
    }
    if let Some(last) = entries.last_mut() {
        *last = candidate;
    }
    sort_boundary_distances(entries);
    true
}

fn sort_boundary_distances(entries: &mut Vec<BoundaryDistance>) {
    entries.sort_unstable_by_key(|entry| boundary_distance_key(*entry));
    entries.truncate(VORONOI_NEAREST_SITE_LIMIT);
}

fn boundary_distance_key(entry: BoundaryDistance) -> (u16, u32, i32, i32) {
    (entry.distance, entry.site.id, entry.site.y, entry.site.x)
}

fn best_medial_pair_distance(tile: AiTile, entries: &[BoundaryDistance]) -> Option<u16> {
    let mut best = None;
    for (left_idx, left) in entries.iter().enumerate() {
        if left.distance < VORONOI_MIN_BOUNDARY_DISTANCE_TILES {
            continue;
        }
        for right in entries.iter().skip(left_idx + 1) {
            if right.distance < VORONOI_MIN_BOUNDARY_DISTANCE_TILES {
                continue;
            }
            let delta = left.distance.abs_diff(right.distance);
            if delta > VORONOI_MAX_PAIR_DISTANCE_DELTA_TILES {
                continue;
            }
            if !sites_face_across_tile(tile, left.site, right.site) {
                continue;
            }
            let pair_distance = left.distance.min(right.distance);
            best = Some(best.map_or(pair_distance, |current: u16| {
                current.max(pair_distance)
            }));
        }
    }
    best
}

fn sites_face_across_tile(tile: AiTile, a: BoundarySite, b: BoundarySite) -> bool {
    let tile_x = tile.x as i32;
    let tile_y = tile.y as i32;
    let ax = a.x - tile_x;
    let ay = a.y - tile_y;
    let bx = b.x - tile_x;
    let by = b.y - tile_y;
    let dot = ax.saturating_mul(bx).saturating_add(ay.saturating_mul(by));
    dot <= 0
}

fn is_medial_ridge(
    width: u32,
    height: u32,
    passable: &[bool],
    medial_distances: &[Option<u16>],
    tile: AiTile,
    distance: u16,
) -> bool {
    let mut touches_lower_or_edge = false;
    for neighbor in adjacent_grid_neighbors(width, height, tile) {
        let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y) else {
            continue;
        };
        if passable.get(neighbor_idx).copied() != Some(true) {
            touches_lower_or_edge = true;
            continue;
        }
        let neighbor_distance = medial_distances.get(neighbor_idx).copied().flatten();
        if neighbor_distance.is_some_and(|neighbor_distance| neighbor_distance > distance) {
            return false;
        }
        match neighbor_distance {
            Some(neighbor_distance) if neighbor_distance < distance => {
                touches_lower_or_edge = true;
            }
            None => {
                touches_lower_or_edge = true;
            }
            _ => {}
        }
    }
    touches_lower_or_edge
}

fn prune_short_skeleton_groups(
    width: u32,
    height: u32,
    passable: &[bool],
    skeleton: Vec<AiMapVoronoiTile>,
) -> Vec<AiMapVoronoiTile> {
    let mut by_tile = vec![None; passable.len()];
    for entry in skeleton {
        if let Some(idx) = tile_index(width, height, entry.tile.x, entry.tile.y) {
            by_tile[idx] = Some(entry);
        }
    }

    let mut visited = vec![false; passable.len()];
    let mut queue = VecDeque::new();
    let mut kept = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let Some(start_idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if visited[start_idx] || by_tile[start_idx].is_none() {
                continue;
            }

            let mut group = Vec::new();
            visited[start_idx] = true;
            queue.push_back(AiTile::new(x, y));
            while let Some(tile) = queue.pop_front() {
                let Some(idx) = tile_index(width, height, tile.x, tile.y) else {
                    continue;
                };
                group.push(idx);
                for neighbor in adjacent_grid_neighbors(width, height, tile) {
                    let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y)
                    else {
                        continue;
                    };
                    if visited[neighbor_idx] || by_tile[neighbor_idx].is_none() {
                        continue;
                    }
                    visited[neighbor_idx] = true;
                    queue.push_back(neighbor);
                }
            }

            if group.len() < VORONOI_MIN_GROUP_TILES {
                continue;
            }
            for idx in group {
                if let Some(entry) = by_tile[idx].take() {
                    kept.push(entry);
                }
            }
        }
    }

    kept.sort_unstable_by_key(|entry| (entry.tile.y, entry.tile.x));
    kept
}

fn cardinal_passable_neighbors(
    width: u32,
    height: u32,
    passable: &[bool],
    tile: AiTile,
) -> Vec<AiTile> {
    let mut out = Vec::with_capacity(4);
    let x = tile.x as i32;
    let y = tile.y as i32;
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let nx = x + dx;
        let ny = y + dy;
        if nx < 0 || ny < 0 || nx as u32 >= width || ny as u32 >= height {
            continue;
        }
        let Some(idx) = tile_index(width, height, nx as u32, ny as u32) else {
            continue;
        };
        if passable.get(idx).copied() == Some(true) {
            out.push(AiTile::new(nx as u32, ny as u32));
        }
    }
    out
}

fn adjacent_grid_neighbors(width: u32, height: u32, tile: AiTile) -> Vec<AiTile> {
    let mut out = Vec::with_capacity(8);
    let x = tile.x as i32;
    let y = tile.y as i32;
    for (dx, dy) in NEIGHBORS {
        let nx = x + dx;
        let ny = y + dy;
        if nx < 0 || ny < 0 || nx as u32 >= width || ny as u32 >= height {
            continue;
        }
        out.push(AiTile::new(nx as u32, ny as u32));
    }
    out
}

fn cardinal_grid_neighbors(width: u32, height: u32, tile: AiTile) -> Vec<AiTile> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medial_pair_rejects_same_side_wall_sites() {
        let tile = AiTile::new(10, 10);
        assert!(!sites_face_across_tile(
            tile,
            BoundarySite { id: 4, x: 9, y: 7 },
            BoundarySite {
                id: 5,
                x: 11,
                y: 7
            }
        ));
        assert!(sites_face_across_tile(
            tile,
            BoundarySite {
                id: 4,
                x: 10,
                y: 7
            },
            BoundarySite {
                id: 5,
                x: 10,
                y: 13
            }
        ));
    }
}
