use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore, OrderIntent};

const CONVERGENCE_END_TILES: f32 = 14.0;
const PARALLEL_DISTANCE_TILES: f32 = 20.0;
const FULL_FAN_DISTANCE_TILES: f32 = 25.0;
const FULL_FAN_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_4;
const FACING_RAY_TILES: f32 = 1_000.0;

pub(super) struct SetupTargetGroup {
    pub(super) units: Vec<u32>,
    pub(super) x: f32,
    pub(super) y: f32,
}

/// Resolve the AT-gun formation from one admitted player command. Keeping this authoritative means
/// the action is budgeted once and every command source (human, AI, replay, or Lab) gets the same
/// setup behavior. Mortars and artillery retain the literal requested point.
pub(super) fn target_groups(
    entities: &EntityStore,
    player: u32,
    units: &[u32],
    raw_x: f32,
    raw_y: f32,
    queued: bool,
) -> Vec<SetupTargetGroup> {
    if !raw_x.is_finite() || !raw_y.is_finite() {
        return vec![literal_group(units, raw_x, raw_y)];
    }

    let mut guns: Vec<(u32, f32, f32)> = units
        .iter()
        .filter_map(|id| {
            let entity = entities.get(*id)?;
            (entity.owner == player && entity.kind == EntityKind::AntiTankGun).then(|| {
                let (x, y) = setup_origin(entity, queued);
                (*id, x, y)
            })
        })
        .collect();
    if guns.is_empty() {
        return vec![literal_group(units, raw_x, raw_y)];
    }

    let centroid_x = guns.iter().map(|(_, x, _)| x).sum::<f32>() / guns.len() as f32;
    let centroid_y = guns.iter().map(|(_, _, y)| y).sum::<f32>() / guns.len() as f32;
    let dx = raw_x - centroid_x;
    let dy = raw_y - centroid_y;
    let distance = dx.hypot(dy);
    let tile_size = config::TILE_SIZE as f32;
    if distance <= CONVERGENCE_END_TILES * tile_size {
        return vec![literal_group(units, raw_x, raw_y)];
    }

    let forward = dy.atan2(dx);
    let right_x = -forward.sin();
    let right_y = forward.cos();
    guns.sort_by(|left, right| {
        let left_projection = left.1 * right_x + left.2 * right_y;
        let right_projection = right.1 * right_x + right.2 * right_y;
        left_projection
            .total_cmp(&right_projection)
            .then_with(|| left.0.cmp(&right.0))
    });
    let convergence = smoothstep(remap01(
        distance,
        CONVERGENCE_END_TILES * tile_size,
        PARALLEL_DISTANCE_TILES * tile_size,
    ));
    let fan = smoothstep(remap01(
        distance,
        PARALLEL_DISTANCE_TILES * tile_size,
        FULL_FAN_DISTANCE_TILES * tile_size,
    ));
    let ray_length = FACING_RAY_TILES * tile_size;
    let mut groups = Vec::with_capacity(guns.len() + 1);
    for (index, (id, x, y)) in guns.iter().copied().enumerate() {
        let literal_facing = (raw_y - y).atan2(raw_x - x);
        let parallel_facing = lerp_angle(literal_facing, forward, convergence);
        let rank = if guns.len() == 1 {
            0.0
        } else {
            index as f32 / (guns.len() - 1) as f32 * 2.0 - 1.0
        };
        let facing = parallel_facing + rank * FULL_FAN_HALF_ANGLE * fan;
        groups.push(SetupTargetGroup {
            units: vec![id],
            x: x + facing.cos() * ray_length,
            y: y + facing.sin() * ray_length,
        });
    }

    let other_units: Vec<u32> = units
        .iter()
        .copied()
        .filter(|id| !guns.iter().any(|(gun_id, _, _)| gun_id == id))
        .collect();
    if !other_units.is_empty() {
        groups.push(literal_group(&other_units, raw_x, raw_y));
    }
    groups
}

fn setup_origin(entity: &Entity, queued: bool) -> (f32, f32) {
    let mut origin = (entity.pos_x, entity.pos_y);
    if !queued {
        return origin;
    }
    if let Some((x, y)) = entity
        .move_intent()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
    {
        origin = (x, y);
    }
    for intent in entity.queued_orders() {
        if let OrderIntent::Move(point) | OrderIntent::AttackMove(point) = intent {
            if point.x.is_finite() && point.y.is_finite() {
                origin = (point.x, point.y);
            }
        }
    }
    origin
}

fn literal_group(units: &[u32], x: f32, y: f32) -> SetupTargetGroup {
    SetupTargetGroup {
        units: units.to_vec(),
        x,
        y,
    }
}

fn remap01(value: f32, start: f32, end: f32) -> f32 {
    ((value - start) / (end - start)).clamp(0.0, 1.0)
}

fn smoothstep(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn lerp_angle(from: f32, to: f32, amount: f32) -> f32 {
    let delta = (to - from).sin().atan2((to - from).cos());
    from + delta * amount
}
