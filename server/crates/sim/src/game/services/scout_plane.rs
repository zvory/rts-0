use std::collections::BTreeSet;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, ScoutPlaneState};
use crate::game::map::Map;
use crate::game::services::dist2;

const TWO_PI: f32 = std::f32::consts::PI * 2.0;
const ORBIT_PHASE_EPS: f32 = 0.001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScoutPlaneLaunchError {
    Active,
    NoCityCentre,
}

pub(crate) fn launch_ability(
    map: &Map,
    entities: &mut EntityStore,
    owner: u32,
    source_command_car: u32,
    launch_x: f32,
    launch_y: f32,
    x: f32,
    y: f32,
) -> Result<u32, ScoutPlaneLaunchError> {
    if active_scout_plane_for_command_car(entities, owner, source_command_car).is_some() {
        return Err(ScoutPlaneLaunchError::Active);
    }
    let Some((target_x, target_y)) = clamp_world_point(map, x, y) else {
        return Err(ScoutPlaneLaunchError::NoCityCentre);
    };
    let Some((launch_x, launch_y)) = clamp_world_point(map, launch_x, launch_y) else {
        return Err(ScoutPlaneLaunchError::NoCityCentre);
    };
    let Some((return_city_centre, _, _)) =
        nearest_owned_completed_city_centre(entities, owner, target_x, target_y)
    else {
        return Err(ScoutPlaneLaunchError::NoCityCentre);
    };
    let spawned = entities
        .spawn_unit(owner, EntityKind::ScoutPlane, launch_x, launch_y)
        .ok_or(ScoutPlaneLaunchError::NoCityCentre)?;
    if let Some(plane) = entities.get_mut(spawned) {
        if let Some(state) = plane.scout_plane_state_mut() {
            *state = ScoutPlaneState::launched_from_command_car(
                return_city_centre,
                source_command_car,
                target_x,
                target_y,
            );
        }
    }
    Ok(spawned)
}

pub(crate) fn active_scout_plane_for_command_car(
    entities: &EntityStore,
    owner: u32,
    source_command_car: u32,
) -> Option<u32> {
    entities
        .iter()
        .filter(|plane| {
            plane.kind == EntityKind::ScoutPlane
                && plane.owner == owner
                && plane.hp > 0
                && plane
                    .scout_plane_state()
                    .is_some_and(|state| state.source_command_car == Some(source_command_car))
        })
        .map(|plane| plane.id)
        .min()
}

pub(crate) fn advance_scout_planes(map: &Map, entities: &mut EntityStore) {
    dismiss_inactive_or_duplicate_planes(entities);

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

        if snapshot.returning {
            let Some(home) = snapshot.home_position else {
                removals.push(id);
                continue;
            };
            let step = advance_return(snapshot.flight.x, snapshot.flight.y, home, speed, world_max);
            if let Some(plane) = entities.get_mut(id) {
                plane.clear_path();
                plane.set_path_goal(None);
                plane.set_position(step.x, step.y);
                plane.set_movement_delta(step.x - snapshot.flight.x, step.y - snapshot.flight.y);
                if let Some(facing) = step.facing {
                    plane.set_facing(facing);
                }
            }
            if step.arrived {
                removals.push(id);
            }
            continue;
        }

        let step = advance_one(snapshot.flight, speed, orbit_radius, world_max);
        let mut station_expired = false;
        if let Some(plane) = entities.get_mut(id) {
            plane.clear_path();
            plane.set_path_goal(None);
            plane.set_position(step.x, step.y);
            plane.set_movement_delta(step.x - snapshot.flight.x, step.y - snapshot.flight.y);
            if let Some(facing) = step.facing {
                plane.set_facing(facing);
            }
            let _ = plane.update_scout_plane_runtime(step.center, step.phase, step.orbiting);
            if step.orbiting {
                let just_arrived = !snapshot.flight.orbiting;
                if let Some(state) = plane.scout_plane_state_mut() {
                    if !just_arrived {
                        state.station_ticks_remaining =
                            state.station_ticks_remaining.saturating_sub(1);
                    }
                    station_expired = state.station_ticks_remaining == 0;
                }
            }
        }

        if station_expired {
            match snapshot.home_position {
                Some(home) => {
                    if let Some(plane) = entities.get_mut(id) {
                        if let Some(state) = plane.scout_plane_state_mut() {
                            state.returning = true;
                            state.orbiting = false;
                            state.orbit_center = home;
                        }
                    }
                }
                None => removals.push(id),
            }
        }
    }

    for id in removals {
        let _ = entities.remove(id);
    }
}

fn dismiss_inactive_or_duplicate_planes(entities: &mut EntityStore) {
    let mut seen_sorties = BTreeSet::new();
    let mut dismissals = Vec::new();
    for id in entities.ids() {
        let Some(plane) = entities.get(id) else {
            continue;
        };
        if plane.kind != EntityKind::ScoutPlane {
            continue;
        }
        let source_command_car = plane
            .scout_plane_state()
            .and_then(|state| state.source_command_car);
        if plane.hp == 0
            || plane.owner == 0
            || !seen_sorties.insert((plane.owner, source_command_car))
        {
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
    home_position: Option<(f32, f32)>,
    returning: bool,
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
    let home_position = state
        .home_city_centre
        .and_then(|home| home_position(entities, plane.owner, home));
    Some(ScoutPlaneRuntimeSnapshot {
        flight: ScoutPlaneSnapshot {
            x: plane.pos_x,
            y: plane.pos_y,
            center,
            phase,
            orbiting: state.orbiting,
        },
        home_position,
        returning: state.returning,
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

#[derive(Clone, Copy)]
struct ReturnStep {
    x: f32,
    y: f32,
    facing: Option<f32>,
    arrived: bool,
}

fn advance_return(x: f32, y: f32, target: (f32, f32), speed: f32, world_max: f32) -> ReturnStep {
    let x = x.clamp(0.0, world_max);
    let y = y.clamp(0.0, world_max);
    let target = (
        target.0.clamp(0.0, world_max),
        target.1.clamp(0.0, world_max),
    );
    let dx = target.0 - x;
    let dy = target.1 - y;
    let dist = (dx * dx + dy * dy).sqrt();
    if !dist.is_finite() || dist <= speed.max(0.0) + ORBIT_PHASE_EPS {
        return ReturnStep {
            x: target.0,
            y: target.1,
            facing: (dist > ORBIT_PHASE_EPS).then_some(dy.atan2(dx)),
            arrived: true,
        };
    }
    let travel = speed.max(0.0);
    let inv = 1.0 / dist;
    ReturnStep {
        x: (x + dx * inv * travel).clamp(0.0, world_max),
        y: (y + dy * inv * travel).clamp(0.0, world_max),
        facing: Some(dy.atan2(dx)),
        arrived: false,
    }
}

fn nearest_owned_completed_city_centre(
    entities: &EntityStore,
    owner: u32,
    x: f32,
    y: f32,
) -> Option<(u32, f32, f32)> {
    entities
        .iter()
        .filter(|candidate| {
            candidate.owner == owner
                && candidate.kind == EntityKind::CityCentre
                && candidate.hp > 0
                && !candidate.under_construction()
        })
        .map(|candidate| {
            (
                candidate.id,
                candidate.pos_x,
                candidate.pos_y,
                dist2(x, y, candidate.pos_x, candidate.pos_y),
            )
        })
        .min_by(|a, b| a.3.total_cmp(&b.3).then_with(|| a.0.cmp(&b.0)))
        .map(|(id, pos_x, pos_y, _)| (id, pos_x, pos_y))
}

fn home_position(entities: &EntityStore, owner: u32, home: u32) -> Option<(f32, f32)> {
    let building = entities.get(home)?;
    (building.owner == owner
        && building.kind == EntityKind::CityCentre
        && building.hp > 0
        && !building.under_construction())
    .then_some((building.pos_x, building.pos_y))
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
