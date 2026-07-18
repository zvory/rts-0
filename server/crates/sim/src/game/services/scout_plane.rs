use crate::config;
use crate::game::entity::{EntityKind, EntityStore, ScoutPlaneState};
use crate::game::map::Map;

const TWO_PI: f32 = std::f32::consts::PI * 2.0;
const ORBIT_PHASE_EPS: f32 = 0.001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScoutPlaneLaunchError {
    InvalidLaunch,
}

pub(crate) fn launch_ability(
    map: &Map,
    entities: &mut EntityStore,
    owner: u32,
    source_command_car: u32,
    x: f32,
    y: f32,
) -> Result<u32, ScoutPlaneLaunchError> {
    let Some((target_x, target_y)) = clamp_world_point(map, x, y) else {
        return Err(ScoutPlaneLaunchError::InvalidLaunch);
    };
    let Some((launch_x, launch_y)) = entities
        .get(source_command_car)
        .filter(|source| {
            source.owner == owner && source.kind == EntityKind::CommandCar && source.hp > 0
        })
        .map(|source| (source.pos_x, source.pos_y))
    else {
        return Err(ScoutPlaneLaunchError::InvalidLaunch);
    };
    let Some((launch_x, launch_y)) = clamp_world_point(map, launch_x, launch_y) else {
        return Err(ScoutPlaneLaunchError::InvalidLaunch);
    };
    let spawned = entities
        .spawn_unit(owner, EntityKind::ScoutPlane, launch_x, launch_y)
        .ok_or(ScoutPlaneLaunchError::InvalidLaunch)?;
    if let Some(plane) = entities.get_mut(spawned) {
        if let Some(state) = plane.scout_plane_state_mut() {
            *state =
                ScoutPlaneState::launched_from_command_car(source_command_car, target_x, target_y);
        }
    }
    Ok(spawned)
}

pub(crate) fn advance_scout_planes(map: &Map, entities: &mut EntityStore) {
    dismiss_inactive_planes(entities);

    let world_max = (map.world_size_px() - 0.01).max(0.0);
    let speed = config::unit_stats(EntityKind::ScoutPlane)
        .map(|stats| stats.speed)
        .unwrap_or(config::SCOUT_PLANE_SPEED_PX_PER_TICK)
        .max(0.0);
    let orbit_radius = config::SCOUT_PLANE_ORBIT_RADIUS_TILES as f32 * config::TILE_SIZE as f32;
    if speed <= 0.0 || orbit_radius <= 0.0 || !orbit_radius.is_finite() {
        return;
    }

    let mut removals = Vec::new();
    for id in entities.ids() {
        ensure_state(entities, id);
        let Some(snapshot) = scout_plane_runtime_snapshot(map, entities, id) else {
            continue;
        };

        let step = advance_one(snapshot.flight, speed, orbit_radius, world_max);
        let mut lifetime_expired = false;
        if let Some(plane) = entities.get_mut(id) {
            plane.clear_path();
            plane.set_path_goal(None);
            plane.set_position(step.x, step.y);
            plane.set_movement_delta(step.x - snapshot.flight.x, step.y - snapshot.flight.y);
            if let Some(facing) = step.facing {
                plane.set_facing(facing);
            }
            let _ = plane.update_scout_plane_runtime(step.center, step.phase, step.orbiting);
            if let Some(state) = plane.scout_plane_state_mut() {
                state.lifetime_ticks_remaining = state.lifetime_ticks_remaining.saturating_sub(1);
                lifetime_expired = state.lifetime_ticks_remaining == 0;
            }
        }

        if lifetime_expired {
            removals.push(id);
        }
    }

    for id in removals {
        let _ = entities.remove(id);
    }
}

fn dismiss_inactive_planes(entities: &mut EntityStore) {
    let mut dismissals = Vec::new();
    for id in entities.ids() {
        let Some(plane) = entities.get(id) else {
            continue;
        };
        if plane.kind != EntityKind::ScoutPlane {
            continue;
        }
        if plane.hp == 0 || plane.owner == 0 {
            dismissals.push(id);
        }
    }
    for id in dismissals {
        let _ = entities.remove(id);
    }
}

fn ensure_state(entities: &mut EntityStore, id: u32) {
    let Some(plane) = entities.get_mut(id) else {
        return;
    };
    plane.ensure_scout_plane_state();
}

#[derive(Clone, Copy)]
struct ScoutPlaneRuntimeSnapshot {
    flight: ScoutPlaneSnapshot,
}

#[derive(Clone, Copy)]
struct ScoutPlaneSnapshot {
    x: f32,
    y: f32,
    center: (f32, f32),
    phase: f32,
    orbiting: bool,
}

fn scout_plane_runtime_snapshot(
    map: &Map,
    entities: &EntityStore,
    id: u32,
) -> Option<ScoutPlaneRuntimeSnapshot> {
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
    Some(ScoutPlaneRuntimeSnapshot {
        flight: ScoutPlaneSnapshot {
            x: plane.pos_x,
            y: plane.pos_y,
            center,
            phase,
            orbiting: state.orbiting,
        },
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
