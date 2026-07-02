use std::collections::BTreeSet;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, OrderIntent, MAX_QUEUED_ORDERS};
use crate::game::map::Map;

const TWO_PI: f32 = std::f32::consts::PI * 2.0;
const ORBIT_PHASE_EPS: f32 = 0.001;

pub(crate) fn launch_from_building(
    map: &Map,
    entities: &mut EntityStore,
    owner: u32,
    building: u32,
) -> Option<u32> {
    let (launch_x, launch_y, target) = {
        let building = entities.get(building)?;
        let launch = (building.pos_x, building.pos_y);
        let target = building
            .rally_plan()
            .first()
            .map(|rally| (rally.point.x, rally.point.y))
            .unwrap_or(launch);
        (launch.0, launch.1, target)
    };

    let target = clamp_world_point(map, target.0, target.1).unwrap_or((launch_x, launch_y));
    let spawned = entities.spawn_unit(owner, EntityKind::ScoutPlane, launch_x, launch_y)?;
    if let Some(plane) = entities.get_mut(spawned) {
        let _ = plane.retarget_scout_plane(target.0, target.1);
    }
    Some(spawned)
}

pub(crate) fn retarget(
    map: &Map,
    entities: &mut EntityStore,
    unit: u32,
    x: f32,
    y: f32,
    clear_queued: bool,
) -> bool {
    let Some((x, y)) = clamp_world_point(map, x, y) else {
        return false;
    };
    let Some(plane) = entities.get_mut(unit) else {
        return false;
    };
    if plane.kind != EntityKind::ScoutPlane || plane.hp == 0 {
        return false;
    }
    if clear_queued {
        plane.clear_queued_orders();
    }
    plane.clear_path();
    plane.set_path_goal(None);
    plane.clear_active_order();
    plane.retarget_scout_plane(x, y)
}

pub(crate) fn append_queued_retarget(
    map: &Map,
    entities: &mut EntityStore,
    unit: u32,
    x: f32,
    y: f32,
) -> bool {
    let Some((x, y)) = clamp_world_point(map, x, y) else {
        return false;
    };
    let Some(plane) = entities.get_mut(unit) else {
        return false;
    };
    if plane.kind != EntityKind::ScoutPlane || plane.hp == 0 {
        return false;
    }
    plane.append_queued_order(OrderIntent::move_to(x, y))
}

pub(in crate::game::services) fn dismiss(
    entities: &mut EntityStore,
    owner: u32,
    unit: u32,
) -> bool {
    let Some(plane) = entities.get(unit) else {
        return false;
    };
    if plane.kind != EntityKind::ScoutPlane || plane.owner != owner {
        return false;
    }
    entities.remove(unit).is_some()
}

pub(crate) fn advance_scout_planes(map: &Map, entities: &mut EntityStore) {
    let world_max = (map.world_size_px() - 0.01).max(0.0);
    let speed = config::unit_stats(EntityKind::ScoutPlane)
        .map(|stats| stats.speed)
        .unwrap_or(config::SCOUT_PLANE_SPEED_PX_PER_TICK)
        .max(0.0);
    let orbit_radius = config::SCOUT_PLANE_ORBIT_RADIUS_TILES as f32 * config::TILE_SIZE as f32;
    if speed <= 0.0 || orbit_radius <= 0.0 || !orbit_radius.is_finite() {
        return;
    }

    for id in entities.ids() {
        ensure_state(entities, id);
        let Some(snapshot) = scout_plane_snapshot(map, entities, id) else {
            continue;
        };
        let step = advance_one(snapshot, speed, orbit_radius, world_max);
        if let Some(plane) = entities.get_mut(id) {
            plane.clear_path();
            plane.set_path_goal(None);
            plane.set_position(step.x, step.y);
            plane.set_movement_delta(step.x - snapshot.x, step.y - snapshot.y);
            if let Some(facing) = step.facing {
                plane.set_facing(facing);
            }
            let _ = plane.update_scout_plane_runtime(step.center, step.phase, step.orbiting);
        }
        if step.orbiting {
            promote_next_queued_center(map, entities, id);
        }
    }
}

pub(in crate::game) fn dismiss_inactive_or_duplicate_planes(entities: &mut EntityStore) {
    let mut seen_owners = BTreeSet::new();
    let mut dismissals = Vec::new();
    for id in entities.ids() {
        let Some(plane) = entities.get(id) else {
            continue;
        };
        if plane.kind != EntityKind::ScoutPlane {
            continue;
        }
        let empty_fuel = plane
            .scout_plane_state()
            .is_some_and(|state| state.fuel_oil == 0);
        if plane.hp == 0 || plane.owner == 0 || empty_fuel || !seen_owners.insert(plane.owner) {
            dismissals.push(id);
        }
    }
    for id in dismissals {
        let _ = entities.remove(id);
    }
}

pub(in crate::game) fn active_scout_planes(entities: &EntityStore) -> Vec<(u32, u32)> {
    entities
        .iter()
        .filter(|plane| plane.kind == EntityKind::ScoutPlane && plane.hp > 0 && plane.owner != 0)
        .map(|plane| (plane.id, plane.owner))
        .collect()
}

pub(in crate::game) fn tick_upkeep_timer(entities: &mut EntityStore, id: u32) -> bool {
    let Some(plane) = entities.get_mut(id) else {
        return false;
    };
    let Some(state) = plane.scout_plane_state_mut() else {
        return false;
    };
    let interval = config::SCOUT_PLANE_UPKEEP_INTERVAL_TICKS.max(1);
    if state.upkeep_ticks_until_due == 0 {
        state.upkeep_ticks_until_due = interval;
    }
    state.upkeep_ticks_until_due = state.upkeep_ticks_until_due.saturating_sub(1);
    if state.upkeep_ticks_until_due == 0 {
        state.upkeep_ticks_until_due = interval;
        true
    } else {
        false
    }
}

pub(in crate::game) fn scout_plane_fuel(entities: &EntityStore, id: u32) -> Option<u8> {
    entities
        .get(id)?
        .scout_plane_state()
        .map(|state| state.fuel_oil)
}

pub(in crate::game) fn drain_fuel(entities: &mut EntityStore, id: u32) -> bool {
    let Some(plane) = entities.get_mut(id) else {
        return true;
    };
    let Some(state) = plane.scout_plane_state_mut() else {
        return true;
    };
    state.fuel_oil = state
        .fuel_oil
        .saturating_sub(config::SCOUT_PLANE_UPKEEP_OIL);
    state.fuel_oil == 0
}

pub(in crate::game) fn missing_fuel_oil(entities: &EntityStore, id: u32) -> u32 {
    let Some(fuel) = scout_plane_fuel(entities, id) else {
        return 0;
    };
    config::SCOUT_PLANE_FUEL_RESERVE_OIL.saturating_sub(fuel) as u32
}

pub(in crate::game) fn refill_fuel(entities: &mut EntityStore, id: u32, amount: u8) {
    if amount == 0 {
        return;
    }
    let Some(plane) = entities.get_mut(id) else {
        return;
    };
    let Some(state) = plane.scout_plane_state_mut() else {
        return;
    };
    state.fuel_oil = state
        .fuel_oil
        .saturating_add(amount)
        .min(config::SCOUT_PLANE_FUEL_RESERVE_OIL);
}

fn ensure_state(entities: &mut EntityStore, id: u32) {
    let Some(plane) = entities.get_mut(id) else {
        return;
    };
    plane.ensure_scout_plane_state();
}

#[derive(Clone, Copy)]
struct ScoutPlaneSnapshot {
    x: f32,
    y: f32,
    center: (f32, f32),
    phase: f32,
    orbiting: bool,
}

fn scout_plane_snapshot(map: &Map, entities: &EntityStore, id: u32) -> Option<ScoutPlaneSnapshot> {
    let plane = entities.get(id)?;
    if plane.kind != EntityKind::ScoutPlane || plane.hp == 0 {
        return None;
    }
    let state = plane.scout_plane_state()?;
    let center = clamp_world_point(map, state.orbit_center.0, state.orbit_center.1)
        .unwrap_or((plane.pos_x, plane.pos_y));
    let phase = if state.orbit_phase.is_finite() {
        normalize_angle(state.orbit_phase)
    } else {
        0.0
    };
    Some(ScoutPlaneSnapshot {
        x: plane.pos_x,
        y: plane.pos_y,
        center,
        phase,
        orbiting: state.orbiting,
    })
}

#[derive(Clone, Copy)]
struct ScoutPlaneStep {
    x: f32,
    y: f32,
    center: (f32, f32),
    phase: f32,
    orbiting: bool,
    facing: Option<f32>,
}

fn advance_one(
    snapshot: ScoutPlaneSnapshot,
    speed: f32,
    orbit_radius: f32,
    world_max: f32,
) -> ScoutPlaneStep {
    let mut x = snapshot.x.clamp(0.0, world_max);
    let mut y = snapshot.y.clamp(0.0, world_max);
    let center = (
        snapshot.center.0.clamp(0.0, world_max),
        snapshot.center.1.clamp(0.0, world_max),
    );
    let mut phase = snapshot.phase;
    let mut orbiting = snapshot.orbiting;
    let mut budget = speed;
    let mut facing = None;

    if !orbiting {
        let dx = center.0 - x;
        let dy = center.1 - y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist.is_finite() && dist > orbit_radius {
            let travel = budget.min(dist - orbit_radius);
            if travel > 0.0 {
                let inv = 1.0 / dist;
                x = (x + dx * inv * travel).clamp(0.0, world_max);
                y = (y + dy * inv * travel).clamp(0.0, world_max);
                facing = Some(dy.atan2(dx));
                budget = (budget - travel).max(0.0);
            }
            orbiting = dist - travel <= orbit_radius + ORBIT_PHASE_EPS;
            if orbiting {
                phase = phase_from_center((x, y), center);
            }
        } else {
            orbiting = true;
            if dist.is_finite() && dist > ORBIT_PHASE_EPS {
                phase = phase_from_center((x, y), center);
            }
        }
    }

    if orbiting {
        let ring = (
            center.0 + phase.cos() * orbit_radius,
            center.1 + phase.sin() * orbit_radius,
        );
        let dx = ring.0 - x;
        let dy = ring.1 - y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist.is_finite() && dist > ORBIT_PHASE_EPS {
            let travel = budget.min(dist);
            if travel > 0.0 {
                let inv = 1.0 / dist;
                x = (x + dx * inv * travel).clamp(0.0, world_max);
                y = (y + dy * inv * travel).clamp(0.0, world_max);
                facing = Some(dy.atan2(dx));
                budget = (budget - travel).max(0.0);
            }
        }
        if budget > 0.0 {
            phase = normalize_angle(phase + budget / orbit_radius);
            x = (center.0 + phase.cos() * orbit_radius).clamp(0.0, world_max);
            y = (center.1 + phase.sin() * orbit_radius).clamp(0.0, world_max);
            facing = Some(normalize_angle(phase + std::f32::consts::FRAC_PI_2));
        }
    }

    ScoutPlaneStep {
        x,
        y,
        center,
        phase: normalize_angle(phase),
        orbiting,
        facing,
    }
}

fn promote_next_queued_center(map: &Map, entities: &mut EntityStore, id: u32) {
    for _ in 0..MAX_QUEUED_ORDERS {
        let Some(intent) = entities
            .get_mut(id)
            .and_then(|plane| plane.pop_promoted_intent())
        else {
            return;
        };
        let OrderIntent::Move(point) = intent else {
            continue;
        };
        if retarget(map, entities, id, point.x, point.y, false) {
            return;
        }
    }
}

fn clamp_world_point(map: &Map, x: f32, y: f32) -> Option<(f32, f32)> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let world_max = (map.world_size_px() - 0.01).max(0.0);
    Some((x.clamp(0.0, world_max), y.clamp(0.0, world_max)))
}

fn normalize_angle(angle: f32) -> f32 {
    if angle.is_finite() {
        angle.rem_euclid(TWO_PI)
    } else {
        0.0
    }
}

fn phase_from_center(point: (f32, f32), center: (f32, f32)) -> f32 {
    normalize_angle((point.1 - center.1).atan2(point.0 - center.0))
}

#[cfg(test)]
mod tests;
