use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;

use super::standability::{
    footing_profile, footing_resistance, unit_static_standable, FootingProfile,
};
use super::STEERING_MAX_NEIGHBORS;

const STEERING_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 1.5;
const STEERING_RADIUS_PX: f32 = config::TILE_SIZE as f32 * 2.5;
const STEERING_STRENGTH: f32 = 0.65;
const TANK_TRAP_STEERING_RADIUS_PX: f32 = config::TILE_SIZE as f32 * 2.0;
const TANK_TRAP_STEERING_STRENGTH: f32 = 0.35;

pub(super) fn steering_path_dir(e: &Entity, x: f32, y: f32, fallback: (f32, f32)) -> (f32, f32) {
    let Some(path) = e.movement.as_ref().map(|m| &m.path) else {
        return fallback;
    };
    let Some((tx, ty)) = path
        .iter()
        .rev()
        .copied()
        .find(|(px, py)| {
            let dx = *px - x;
            let dy = *py - y;
            (dx * dx + dy * dy).sqrt() >= STEERING_LOOKAHEAD_PX
        })
        .or_else(|| path.last().copied())
    else {
        return fallback;
    };
    let dx = tx - x;
    let dy = ty - y;
    let d = (dx * dx + dy * dy).sqrt();
    if d <= 1e-4 || !d.is_finite() {
        fallback
    } else {
        (dx / d, dy / d)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn steered_candidate(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    occ: &Occupancy,
    map: &Map,
    id: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    path_dir: (f32, f32),
    budget: f32,
) -> Option<(f32, f32)> {
    let steer_dir = local_steering_dir(entities, spatial, id, x, y, path_dir);
    if (steer_dir.0 - path_dir.0).abs() <= 1e-4 && (steer_dir.1 - path_dir.1).abs() <= 1e-4 {
        return None;
    }

    let nx = x + steer_dir.0 * budget;
    let ny = y + steer_dir.1 * budget;
    let facing = if uses_oriented_vehicle_body(kind) {
        path_dir.1.atan2(path_dir.0)
    } else {
        0.0
    };
    unit_static_standable(occ, map, kind, nx, ny, facing).then_some((nx, ny))
}

fn local_steering_dir(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    id: u32,
    x: f32,
    y: f32,
    path_dir: (f32, f32),
) -> (f32, f32) {
    let Some(unit) = entities.get(id) else {
        return path_dir;
    };
    let unit_radius = unit.radius();

    let mut neighbors: Vec<u32> = spatial
        .ids_in_circle_bbox(x, y, STEERING_RADIUS_PX)
        .filter(|&neighbor_id| neighbor_id != id)
        .filter(|&neighbor_id| {
            let Some(neighbor) = entities.get(neighbor_id) else {
                return false;
            };
            neighbor.hp > 0
                && neighbor.is_unit()
                && !matches!(
                    footing_profile(neighbor),
                    FootingProfile::Ghost | FootingProfile::Soft
                )
                && {
                    let dx = x - neighbor.pos_x;
                    let dy = y - neighbor.pos_y;
                    dx * dx + dy * dy <= STEERING_RADIUS_PX * STEERING_RADIUS_PX
                }
        })
        .collect();
    neighbors.sort_unstable();
    neighbors.truncate(STEERING_MAX_NEIGHBORS);

    let mut sep_x = 0.0_f32;
    let mut sep_y = 0.0_f32;
    for neighbor_id in neighbors {
        let Some(neighbor) = entities.get(neighbor_id) else {
            continue;
        };
        let dx = x - neighbor.pos_x;
        let dy = y - neighbor.pos_y;
        let d2 = dx * dx + dy * dy;
        let (away_x, away_y, d) = if d2 <= 1e-4 {
            if id < neighbor_id {
                (-path_dir.1, path_dir.0, 0.0)
            } else {
                (path_dir.1, -path_dir.0, 0.0)
            }
        } else {
            let d = d2.sqrt();
            (dx / d, dy / d, d)
        };

        let min_d = unit_radius + neighbor.radius();
        let proximity = ((STEERING_RADIUS_PX - d) / STEERING_RADIUS_PX).clamp(0.0, 1.0);
        let overlap_boost = if d < min_d {
            1.0 + ((min_d - d) / min_d.max(1.0))
        } else {
            1.0
        };
        let footing_weight = footing_resistance(footing_profile(neighbor)).sqrt();
        let weight = proximity * proximity * overlap_boost * footing_weight;
        sep_x += away_x * weight;
        sep_y += away_y * weight;
    }

    let trap_bias = tank_trap_steering_bias(entities, spatial, id, x, y, path_dir);
    let sep_len = (sep_x * sep_x + sep_y * sep_y).sqrt();
    let trap_bias_len = (trap_bias.0 * trap_bias.0 + trap_bias.1 * trap_bias.1).sqrt();
    if sep_len <= 1e-4 && trap_bias_len <= 1e-4 {
        return path_dir;
    }

    let mut desired_x = path_dir.0;
    let mut desired_y = path_dir.1;
    if sep_len > 1e-4 {
        desired_x += (sep_x / sep_len) * STEERING_STRENGTH;
        desired_y += (sep_y / sep_len) * STEERING_STRENGTH;
    }
    desired_x += trap_bias.0 * TANK_TRAP_STEERING_STRENGTH;
    desired_y += trap_bias.1 * TANK_TRAP_STEERING_STRENGTH;
    let desired_len = (desired_x * desired_x + desired_y * desired_y).sqrt();
    if desired_len <= 1e-4 {
        path_dir
    } else {
        (desired_x / desired_len, desired_y / desired_len)
    }
}

fn tank_trap_steering_bias(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    id: u32,
    x: f32,
    y: f32,
    path_dir: (f32, f32),
) -> (f32, f32) {
    if !path_dir.0.is_finite() || !path_dir.1.is_finite() {
        return (0.0, 0.0);
    }
    let perp = (-path_dir.1, path_dir.0);

    let mut traps: Vec<u32> = spatial
        .ids_in_circle_bbox(x, y, TANK_TRAP_STEERING_RADIUS_PX)
        .filter(|&trap_id| {
            entities
                .get(trap_id)
                .is_some_and(|trap| trap.hp > 0 && trap.kind == EntityKind::TankTrap)
        })
        .collect();
    traps.sort_unstable();
    traps.truncate(STEERING_MAX_NEIGHBORS);

    let mut bias_x = 0.0_f32;
    let mut bias_y = 0.0_f32;
    for trap_id in traps {
        let Some(trap) = entities.get(trap_id) else {
            continue;
        };
        let rel_x = x - trap.pos_x;
        let rel_y = y - trap.pos_y;
        let d2 = rel_x * rel_x + rel_y * rel_y;
        if !d2.is_finite() || d2 > TANK_TRAP_STEERING_RADIUS_PX * TANK_TRAP_STEERING_RADIUS_PX {
            continue;
        }
        let d = d2.sqrt();

        let along = (trap.pos_x - x) * path_dir.0 + (trap.pos_y - y) * path_dir.1;
        let forward = ((along + config::TILE_SIZE as f32 * 0.5)
            / (TANK_TRAP_STEERING_RADIUS_PX + config::TILE_SIZE as f32 * 0.5))
            .clamp(0.0, 1.0);
        if forward <= 0.0 {
            continue;
        }

        let lateral = rel_x * perp.0 + rel_y * perp.1;
        let side = if lateral.abs() > 1.0 {
            lateral.signum()
        } else if id < trap_id {
            1.0
        } else {
            -1.0
        };
        let proximity =
            ((TANK_TRAP_STEERING_RADIUS_PX - d) / TANK_TRAP_STEERING_RADIUS_PX).clamp(0.0, 1.0);
        let weight = proximity * proximity * forward;
        bias_x += perp.0 * side * weight;
        bias_y += perp.1 * side * weight;
    }

    let bias_len = (bias_x * bias_x + bias_y * bias_y).sqrt();
    if bias_len <= 1e-4 {
        (0.0, 0.0)
    } else {
        (bias_x / bias_len, bias_y / bias_len)
    }
}

/// Inject a perpendicular detour waypoint so a stuck mid-path unit can shimmy free.
/// Direction is derived from repulsion away from neighbors (deterministic).
/// `repulsion_dir` is the pre-computed normalized repulsion vector (or (0,0) if no neighbors).
#[allow(clippy::too_many_arguments)]
pub(super) fn inject_sidestep(
    e: &mut Entity,
    entity_id: u32,
    x: f32,
    y: f32,
    map: &Map,
    occ: &Occupancy,
    repulsion_dir: (f32, f32),
    tick: u32,
) {
    // Heading toward next waypoint; fall back to facing angle if no waypoint.
    let (hx, hy) = if let Some((wx, wy)) = e.next_waypoint() {
        let dx = wx - x;
        let dy = wy - y;
        let d = (dx * dx + dy * dy).sqrt();
        if d > 1e-4 {
            (dx / d, dy / d)
        } else {
            (e.facing().cos(), e.facing().sin())
        }
    } else {
        (e.facing().cos(), e.facing().sin())
    };

    // Use repulsion direction if meaningful; otherwise fall back to id-parity perpendicular.
    let (bx, by) = if repulsion_dir.0 != 0.0 || repulsion_dir.1 != 0.0 {
        repulsion_dir
    } else if entity_id & 1 == 0 {
        (-hy, hx)
    } else {
        (hy, -hx)
    };

    // Deterministic jitter seeded from both entity_id and tick so repeated sidestepping
    // explores different directions rather than always re-entering the same blocked spot.
    let seed = entity_id.wrapping_add(tick);
    let jitter_angle = ((seed % 5) as f32 - 2.0) * (std::f32::consts::PI / 12.0); // ±30°
    let (cos_j, sin_j) = (jitter_angle.cos(), jitter_angle.sin());
    let (px, py) = (bx * cos_j - by * sin_j, bx * sin_j + by * cos_j);

    // Distance jitter: 0.5×–0.75× of SIDESTEP_DISTANCE_PX (half the original average).
    let d = config::SIDESTEP_DISTANCE_PX * (0.5 + (seed % 3) as f32 * 0.125);
    let tx = x + px * d;
    let ty = y + py * d;

    let facing = e.facing();
    let point_clear = |cx: f32, cy: f32| unit_static_standable(occ, map, e.kind, cx, cy, facing);

    let detour = if point_clear(tx, ty) {
        Some((tx, ty))
    } else {
        // Try opposite side.
        let tx2 = x - px * d;
        let ty2 = y - py * d;
        if point_clear(tx2, ty2) {
            Some((tx2, ty2))
        } else {
            None
        }
    };

    if let Some(waypoint) = detour {
        // path is reverse-ordered; push makes it the *next* waypoint.
        e.push_waypoint(waypoint);
        if let Some(m) = e.movement.as_mut() {
            m.sidestep_cooldown = config::SIDESTEP_COOLDOWN_TICKS;
            m.stuck_ticks = 0;
        }
    }
}
