use crate::game::entity::{Entity, EntityStore};
use crate::game::map::Map;

use super::{TANK_STANDOFF_BUFFER_PX, TANK_STANDOFF_REPATH_DELTA_PX};

fn uses_vehicle_standoff_policy(e: &Entity) -> bool {
    crate::game::entity::fires_while_moving(e.kind)
}

pub(super) fn chase_goal_for_target(
    map: &Map,
    entities: &EntityStore,
    attacker_id: u32,
    attacker_pos: (f32, f32),
    target_pos: (f32, f32),
    range_px: f32,
    dist: f32,
) -> (f32, f32) {
    let is_out_of_range_tank = entities
        .get(attacker_id)
        .map(|e| uses_vehicle_standoff_policy(e) && dist > range_px)
        .unwrap_or(false);
    if !is_out_of_range_tank {
        return target_pos;
    }
    tank_standoff_goal(map, attacker_pos, target_pos, range_px).unwrap_or(target_pos)
}

fn tank_standoff_goal(
    map: &Map,
    attacker_pos: (f32, f32),
    target_pos: (f32, f32),
    range_px: f32,
) -> Option<(f32, f32)> {
    let (px, py) = attacker_pos;
    let (tx, ty) = target_pos;
    if !px.is_finite()
        || !py.is_finite()
        || !tx.is_finite()
        || !ty.is_finite()
        || !range_px.is_finite()
    {
        return None;
    }
    let dx = px - tx;
    let dy = py - ty;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= f32::EPSILON || !dist.is_finite() {
        return None;
    }
    let buffer = TANK_STANDOFF_BUFFER_PX.min(range_px * 0.5);
    let desired_dist = (range_px - buffer).max(0.0);
    let ux = dx / dist;
    let uy = dy / dist;
    let max = map.world_size_px() - 0.01;
    Some((
        (tx + ux * desired_dist).clamp(0.0, max),
        (ty + uy * desired_dist).clamp(0.0, max),
    ))
}

pub(super) fn chase_path_needs_refresh(e: &Entity, chase_goal: (f32, f32)) -> bool {
    if e.path_is_empty() {
        return true;
    }
    if !uses_vehicle_standoff_policy(e) {
        return false;
    }
    e.path_goal()
        .map(|goal| {
            (goal.0 - chase_goal.0).abs() > TANK_STANDOFF_REPATH_DELTA_PX
                || (goal.1 - chase_goal.1).abs() > TANK_STANDOFF_REPATH_DELTA_PX
        })
        .unwrap_or(true)
}
