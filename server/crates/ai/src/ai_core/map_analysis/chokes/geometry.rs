use super::super::{tile_distance2, AiTile, AiTileBounds};
use super::nearest_tile_to;

pub(super) fn choke_line_geometry(
    tiles: &[AiTile],
    center_tile: AiTile,
    approach_a_tile: AiTile,
    approach_b_tile: AiTile,
    bounds: AiTileBounds,
) -> (AiTile, AiTile, u16) {
    let width_tiles =
        choke_cross_section_width(tiles, center_tile, approach_a_tile, approach_b_tile)
            .unwrap_or_else(|| {
                let span_x = bounds.max.x.saturating_sub(bounds.min.x).saturating_add(1);
                let span_y = bounds.max.y.saturating_sub(bounds.min.y).saturating_add(1);
                span_x.min(span_y).min(u32::from(u16::MAX)).max(1) as u16
            });

    if let Some(geometry) = projected_choke_line_geometry(tiles, approach_a_tile, approach_b_tile) {
        return (geometry.0, geometry.1, width_tiles);
    }

    if let Some((endpoint_a_tile, endpoint_b_tile, _)) = bounds_choke_line_geometry(tiles, bounds) {
        return (endpoint_a_tile, endpoint_b_tile, width_tiles);
    }

    {
        let endpoint_a_tile = nearest_tile_to(tiles, approach_a_tile);
        let endpoint_b_tile = nearest_tile_to(tiles, approach_b_tile);
        (endpoint_a_tile, endpoint_b_tile, width_tiles)
    }
}

fn projected_choke_line_geometry(
    tiles: &[AiTile],
    approach_a_tile: AiTile,
    approach_b_tile: AiTile,
) -> Option<(AiTile, AiTile, u16)> {
    let dx = approach_b_tile.x as f32 - approach_a_tile.x as f32;
    let dy = approach_b_tile.y as f32 - approach_a_tile.y as f32;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() || len <= f32::EPSILON {
        return None;
    }

    // The choke band can be a stepped or diagonal group of passable tiles. Publish one tactical
    // line for consumers by projecting every evidence tile onto the axis perpendicular to the
    // two region approaches, then spanning the extremes on that axis.
    projected_line_for_axis(tiles, (-dy / len, dx / len))
}

fn bounds_choke_line_geometry(
    tiles: &[AiTile],
    bounds: AiTileBounds,
) -> Option<(AiTile, AiTile, u16)> {
    if tiles.is_empty() {
        return None;
    }
    let span_x = bounds.max.x.saturating_sub(bounds.min.x).saturating_add(1);
    let span_y = bounds.max.y.saturating_sub(bounds.min.y).saturating_add(1);
    if span_x >= span_y {
        projected_line_for_axis(tiles, (1.0, 0.0))
    } else {
        projected_line_for_axis(tiles, (0.0, 1.0))
    }
}

fn choke_cross_section_width(
    tiles: &[AiTile],
    center_tile: AiTile,
    approach_a_tile: AiTile,
    approach_b_tile: AiTile,
) -> Option<u16> {
    if tiles.is_empty() {
        return None;
    }
    let dx = approach_a_tile.x.abs_diff(approach_b_tile.x);
    let dy = approach_a_tile.y.abs_diff(approach_b_tile.y);
    if dx >= dy {
        let x = tiles
            .iter()
            .map(|tile| tile.x)
            .min_by_key(|x| x.abs_diff(center_tile.x))?;
        let values: Vec<_> = tiles
            .iter()
            .filter_map(|tile| (tile.x == x).then_some(tile.y))
            .collect();
        longest_contiguous_span(values)
    } else {
        let y = tiles
            .iter()
            .map(|tile| tile.y)
            .min_by_key(|y| y.abs_diff(center_tile.y))?;
        let values: Vec<_> = tiles
            .iter()
            .filter_map(|tile| (tile.y == y).then_some(tile.x))
            .collect();
        longest_contiguous_span(values)
    }
}

fn longest_contiguous_span(mut values: Vec<u32>) -> Option<u16> {
    values.sort_unstable();
    values.dedup();
    let (&first, rest) = values.split_first()?;
    let mut best = 1_u32;
    let mut current = 1_u32;
    let mut previous = first;
    for value in rest {
        if *value == previous.saturating_add(1) {
            current = current.saturating_add(1);
        } else {
            best = best.max(current);
            current = 1;
        }
        previous = *value;
    }
    best = best.max(current);
    Some(best.min(u32::from(u16::MAX)) as u16)
}

#[derive(Clone, Copy, Debug)]
struct ProjectedChokeTile {
    tile: AiTile,
    projection: f32,
}

fn projected_line_for_axis(
    tiles: &[AiTile],
    line_axis: (f32, f32),
) -> Option<(AiTile, AiTile, u16)> {
    if tiles.is_empty() || !line_axis.0.is_finite() || !line_axis.1.is_finite() {
        return None;
    }

    let mut projected_tiles = Vec::with_capacity(tiles.len());
    for &tile in tiles {
        let x = tile.x as f32 + 0.5;
        let y = tile.y as f32 + 0.5;
        let projection = x * line_axis.0 + y * line_axis.1;
        projected_tiles.push(ProjectedChokeTile { tile, projection });
    }
    let min_projection = projected_tiles
        .iter()
        .map(|tile| tile.projection)
        .min_by(f32::total_cmp)?;
    let max_projection = projected_tiles
        .iter()
        .map(|tile| tile.projection)
        .max_by(f32::total_cmp)?;
    let projection_epsilon = 2.0_f32;
    let min_candidates: Vec<_> = projected_tiles
        .iter()
        .copied()
        .filter(|tile| (tile.projection - min_projection).abs() <= projection_epsilon)
        .collect();
    let max_candidates: Vec<_> = projected_tiles
        .iter()
        .copied()
        .filter(|tile| (tile.projection - max_projection).abs() <= projection_epsilon)
        .collect();
    let (min_tile, max_tile) = farthest_projected_pair(&min_candidates, &max_candidates)?;
    let width_tiles = (max_tile.projection - min_tile.projection)
        .abs()
        .ceil()
        .max(0.0) as u32
        + 1;
    Some((
        min_tile.tile,
        max_tile.tile,
        width_tiles.min(u32::from(u16::MAX)).max(1) as u16,
    ))
}

fn farthest_projected_pair(
    min_candidates: &[ProjectedChokeTile],
    max_candidates: &[ProjectedChokeTile],
) -> Option<(ProjectedChokeTile, ProjectedChokeTile)> {
    let mut best: Option<(ProjectedChokeTile, ProjectedChokeTile, u32)> = None;
    for &left in min_candidates {
        for &right in max_candidates {
            let distance = tile_distance2(left.tile, right.tile);
            let better = best.is_none_or(|(best_left, best_right, best_distance)| {
                distance > best_distance
                    || (distance == best_distance
                        && (left.tile.y, left.tile.x, right.tile.y, right.tile.x)
                            < (
                                best_left.tile.y,
                                best_left.tile.x,
                                best_right.tile.y,
                                best_right.tile.x,
                            ))
            });
            if better {
                best = Some((left, right, distance));
            }
        }
    }
    best.map(|(left, right, _)| (left, right))
}
