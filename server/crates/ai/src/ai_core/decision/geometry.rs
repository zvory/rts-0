use super::*;

pub(super) fn normalized_direction(from: (f32, f32), to: (f32, f32)) -> Option<(f32, f32)> {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON || !len.is_finite() {
        return None;
    }
    Some((dx / len, dy / len))
}

pub(super) fn clamp_to_map(point: (f32, f32), map: AiMapSummary) -> (f32, f32) {
    let tile_size = map.tile_size as f32;
    let min = tile_size * 0.5;
    let max_x = map.width as f32 * tile_size - min;
    let max_y = map.height as f32 * tile_size - min;
    (
        point.0.clamp(min, max_x.max(min)),
        point.1.clamp(min, max_y.max(min)),
    )
}

pub(super) fn footprint_edge_distance_tiles(
    tile: (u32, u32),
    stats: &config::BuildingStats,
    map_width: u32,
    map_height: u32,
) -> u32 {
    let left = tile.0;
    let top = tile.1;
    let right = map_width.saturating_sub(tile.0.saturating_add(stats.foot_w));
    let bottom = map_height.saturating_sub(tile.1.saturating_add(stats.foot_h));
    left.min(top).min(right).min(bottom)
}

pub(super) fn point_line_distance2(
    point: (f32, f32),
    line_start: (f32, f32),
    line_end: (f32, f32),
) -> f32 {
    let vx = line_end.0 - line_start.0;
    let vy = line_end.1 - line_start.1;
    let line_len2 = vx * vx + vy * vy;
    if line_len2 <= f32::EPSILON {
        return dist2(point.0, point.1, line_start.0, line_start.1);
    }
    let wx = point.0 - line_start.0;
    let wy = point.1 - line_start.1;
    let cross = wx * vy - wy * vx;
    cross * cross / line_len2
}

pub(super) fn building_center(
    tile: (u32, u32),
    kind: EntityKind,
    tile_size: u32,
) -> Option<(f32, f32)> {
    let stats = config::building_stats(kind)?;
    let tile_size = tile_size as f32;
    Some((
        tile.0 as f32 * tile_size + stats.foot_w as f32 * tile_size * 0.5,
        tile.1 as f32 * tile_size + stats.foot_h as f32 * tile_size * 0.5,
    ))
}

pub(super) fn footprint_top_left_for_center(
    center_tile: (u32, u32),
    kind: EntityKind,
) -> Option<(u32, u32)> {
    let stats = config::building_stats(kind)?;
    Some((
        center_tile.0.saturating_sub(stats.foot_w / 2),
        center_tile.1.saturating_sub(stats.foot_h / 2),
    ))
}

pub(super) fn tile_center(tile: (u32, u32), tile_size: u32) -> (f32, f32) {
    (
        tile.0 as f32 * tile_size as f32 + tile_size as f32 * 0.5,
        tile.1 as f32 * tile_size as f32 + tile_size as f32 * 0.5,
    )
}

pub(super) fn dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

pub(super) fn squared(value: f32) -> f32 {
    value * value
}
