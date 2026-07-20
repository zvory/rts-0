use crate::config;

type WorldPoint = (f32, f32);

pub(super) fn artillery_blanket_point(
    unit: u32,
    owner: u32,
    tick: u32,
    center: WorldPoint,
    shot_number: u16,
    fire_radius_tiles: f32,
) -> (f32, f32) {
    let radius_px = fire_radius_tiles
        .clamp(
            config::ARTILLERY_MIN_FIRE_RADIUS_TILES,
            config::ARTILLERY_BLANKET_RADIUS_TILES,
        ) * config::TILE_SIZE as f32;
    let seed = mix32(
        unit.wrapping_mul(0x9E37_79B9)
            ^ owner.wrapping_mul(0x85EB_CA6B)
            ^ tick.rotate_left(7)
            ^ (shot_number as u32).wrapping_mul(0xC2B2_AE35),
    );
    offset_inside_circle(center, radius_px, seed)
}

fn offset_inside_circle(center: WorldPoint, radius_px: f32, seed: u32) -> WorldPoint {
    if radius_px <= f32::EPSILON {
        return center;
    }
    let angle = unit_float(seed) * std::f32::consts::TAU;
    let radial = unit_float(mix32(seed ^ 0xA5A5_5A5A)).sqrt() * radius_px;
    (center.0 + angle.cos() * radial, center.1 + angle.sin() * radial)
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
