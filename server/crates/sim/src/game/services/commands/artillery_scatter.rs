use crate::config;
use crate::game::services::dist2;

type WorldPoint = (f32, f32);

pub(super) fn artillery_scattered_point(
    unit: u32,
    tick: u32,
    origin: WorldPoint,
    target: WorldPoint,
    shot_number: u16,
    ballistic_tables_researched: bool,
) -> (f32, f32) {
    let error_tiles =
        artillery_error_tiles(origin, target, shot_number, ballistic_tables_researched);
    let radius_px = error_tiles.max(0.0) * config::TILE_SIZE as f32;
    if radius_px <= f32::EPSILON {
        return target;
    }
    let seed = unit
        .wrapping_mul(1_103_515_245)
        .wrapping_add(tick)
        .wrapping_add((shot_number as u32).wrapping_mul(97_531));
    let angle = (seed as f32 * 1.618_034).rem_euclid(std::f32::consts::TAU);
    let radial = (((seed.rotate_left(13) >> 8) & 1023) as f32 / 1023.0).sqrt() * radius_px;
    let (x, y) = target;
    (x + angle.cos() * radial, y + angle.sin() * radial)
}

pub(super) fn artillery_blanket_point(
    unit: u32,
    owner: u32,
    tick: u32,
    center: WorldPoint,
    shot_number: u16,
) -> (f32, f32) {
    let radius_px = config::ARTILLERY_BLANKET_RADIUS_TILES.max(0.0) * config::TILE_SIZE as f32;
    if radius_px <= f32::EPSILON {
        return center;
    }
    let seed = mix32(
        unit.wrapping_mul(0x9E37_79B9)
            ^ owner.wrapping_mul(0x85EB_CA6B)
            ^ tick.rotate_left(7)
            ^ (shot_number as u32).wrapping_mul(0xC2B2_AE35),
    );
    let angle_unit = unit_float(seed);
    let radius_unit = unit_float(mix32(seed ^ 0xA5A5_5A5A));
    let angle = angle_unit * std::f32::consts::TAU;
    let radial = radius_unit.sqrt() * radius_px;
    let (x, y) = center;
    (x + angle.cos() * radial, y + angle.sin() * radial)
}

pub(super) fn artillery_error_tiles(
    origin: WorldPoint,
    target: WorldPoint,
    shot_number: u16,
    ballistic_tables_researched: bool,
) -> f32 {
    let (origin_x, origin_y) = origin;
    let (x, y) = target;
    let distance_tiles = dist2(origin_x, origin_y, x, y).sqrt() / config::TILE_SIZE as f32;
    let range_span =
        (config::ARTILLERY_MAX_RANGE_TILES - config::ARTILLERY_MIN_RANGE_TILES).max(1) as f32;
    let range_progress = if distance_tiles.is_finite() {
        ((distance_tiles - config::ARTILLERY_MIN_RANGE_TILES as f32) / range_span).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let starting_error = config::ARTILLERY_MIN_RANGE_ERROR_TILES
        + (config::ARTILLERY_MAX_RANGE_ERROR_TILES - config::ARTILLERY_MIN_RANGE_ERROR_TILES)
            * range_progress;
    if !ballistic_tables_researched {
        return starting_error;
    }
    let max_step = config::ARTILLERY_ACCURACY_SHOTS_TO_MIN
        .saturating_sub(1)
        .max(1) as f32;
    let progress = (shot_number.saturating_sub(1) as f32 / max_step).clamp(0.0, 1.0);
    starting_error + (config::ARTILLERY_MIN_ERROR_TILES - starting_error) * progress
}

fn mix32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^ (x >> 16)
}

fn unit_float(x: u32) -> f32 {
    ((x >> 8) as f32) / 16_777_215.0
}
