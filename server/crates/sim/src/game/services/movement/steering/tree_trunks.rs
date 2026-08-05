use crate::config;
use crate::game::entity::{movement_body_class, EntityKind, MovementBodyClass};

const RADIUS_PX: f32 = config::TILE_SIZE as f32 * 1.5;
const STRENGTH: f32 = 0.55;
const MAX_NEIGHBORS: usize = 32;

pub(super) fn query_bounds(x: f32, y: f32) -> (i32, i32, i32, i32) {
    let tile_radius = (RADIUS_PX / config::TILE_SIZE as f32).ceil() as i32;
    let tx = (x / config::TILE_SIZE as f32).floor() as i32;
    let ty = (y / config::TILE_SIZE as f32).floor() as i32;
    (
        tx - tile_radius,
        ty - tile_radius,
        tx + tile_radius,
        ty + tile_radius,
    )
}

pub(super) fn apply_bias(
    trunks: impl Iterator<Item = (f32, f32)>,
    id: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    base_dir: (f32, f32),
    path_dir: (f32, f32),
) -> (f32, f32) {
    let bias = bias(trunks, id, kind, x, y, path_dir);
    normalize_or(
        (
            base_dir.0 + bias.0 * STRENGTH,
            base_dir.1 + bias.1 * STRENGTH,
        ),
        path_dir,
    )
}

fn bias(
    trunks: impl Iterator<Item = (f32, f32)>,
    id: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    path_dir: (f32, f32),
) -> (f32, f32) {
    if movement_body_class(kind) != MovementBodyClass::InfantryLike
        || !path_dir.0.is_finite()
        || !path_dir.1.is_finite()
    {
        return (0.0, 0.0);
    }
    let perp = (-path_dir.1, path_dir.0);
    let mut result = (0.0_f32, 0.0_f32);
    for (index, (trunk_x, trunk_y)) in trunks.take(MAX_NEIGHBORS).enumerate() {
        let rel = (x - trunk_x, y - trunk_y);
        let distance_sq = rel.0 * rel.0 + rel.1 * rel.1;
        if !distance_sq.is_finite() || distance_sq > RADIUS_PX * RADIUS_PX {
            continue;
        }
        let along = (trunk_x - x) * path_dir.0 + (trunk_y - y) * path_dir.1;
        let forward = ((along + config::TILE_SIZE as f32 * 0.25)
            / (RADIUS_PX + config::TILE_SIZE as f32 * 0.25))
            .clamp(0.0, 1.0);
        if forward <= 0.0 {
            continue;
        }
        let lateral = rel.0 * perp.0 + rel.1 * perp.1;
        let side = if lateral.abs() > 0.5 {
            lateral.signum()
        } else if (id as usize).wrapping_add(index) & 1 == 0 {
            1.0
        } else {
            -1.0
        };
        let proximity = ((RADIUS_PX - distance_sq.sqrt()) / RADIUS_PX).clamp(0.0, 1.0);
        let weight = proximity * proximity * forward;
        result.0 += perp.0 * side * weight;
        result.1 += perp.1 * side * weight;
    }
    normalize_or(result, (0.0, 0.0))
}

fn normalize_or(vector: (f32, f32), fallback: (f32, f32)) -> (f32, f32) {
    let length = (vector.0 * vector.0 + vector.1 * vector.1).sqrt();
    if !length.is_finite() || length <= 1e-4 {
        fallback
    } else {
        (vector.0 / length, vector.1 / length)
    }
}
