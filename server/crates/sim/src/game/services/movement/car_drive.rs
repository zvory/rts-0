use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::geometry::{unit_body_for_entity, unit_body_with_facing};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::standability as static_standability;

use super::pivot_drive::{
    angle_delta, distance_between, normalize_angle, vehicle_traffic_adjustment,
    VEHICLE_REVERSE_GOAL_DISTANCE_PX,
};
use super::vehicle_profiles::{car_motion_profile, CarMotionProfile};
#[cfg(test)]
pub(super) use super::vehicle_profiles::{
    SCOUT_CAR_MIN_TURN_RADIUS_PX, SCOUT_CAR_ROUTE_LOOKAHEAD_PX,
};
#[cfg(test)]
use super::vehicle_route::vehicle_desired_path_point;
use super::vehicle_route::{
    along_track_error, lateral_error, route_accepts_waypoint, static_clearance_px,
    static_swept_segment_legal, unit_direction, vehicle_route_context, VehicleRouteContext,
};
use super::{ARRIVE_EPS, MAX_UNIT_BOUNDING_RADIUS_PX, STEERING_MAX_NEIGHBORS};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ScoutCarMotionPlan {
    pub(super) pos: (f32, f32),
    pub(super) facing: Option<f32>,
    pub(super) reverse_waypoint: Option<(f32, f32)>,
    pub(super) static_blocked: bool,
    pub(super) pop_waypoints: usize,
}

#[derive(Clone, Copy)]
struct Primitive {
    curvature: f32,
    travel_sign: f32,
    ordinal: u8,
}

#[derive(Clone, Copy)]
struct Candidate {
    pos: (f32, f32),
    facing: f32,
    travel_dir: (f32, f32),
    primitive: Primitive,
    min_static_clearance_px: f32,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn plan_scout_car_motion(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    spatial: &SpatialIndex,
    id: u32,
    e: &Entity,
    current: (f32, f32),
    budget: f32,
) -> Option<ScoutCarMotionPlan> {
    if !matches!(
        e.kind,
        EntityKind::ScoutCar | EntityKind::CommandCar | EntityKind::RocketLauncher
    ) || !budget.is_finite()
        || budget <= 0.0
    {
        return None;
    }
    if e.next_waypoint()
        .is_some_and(|next| distance_between(current, next) <= ARRIVE_EPS)
    {
        return Some(ScoutCarMotionPlan {
            pos: current,
            facing: None,
            reverse_waypoint: None,
            static_blocked: false,
            pop_waypoints: 1,
        });
    }
    let profile = car_motion_profile(e.kind)?;
    let route = vehicle_route_context(map, occ, e, current)?;
    let route_reverse_waypoint = if route.pre_pop_count == 0 {
        scout_car_reverse_waypoint(e, current.0, current.1)
    } else {
        None
    };
    if route.pre_pop_count >= e.movement.as_ref()?.path.len() {
        return Some(ScoutCarMotionPlan {
            pos: current,
            facing: None,
            reverse_waypoint: None,
            static_blocked: false,
            pop_waypoints: route.pre_pop_count,
        });
    }

    let traffic = vehicle_traffic_adjustment(
        entities,
        spatial,
        id,
        e.kind,
        current.0,
        current.1,
        e.facing(),
    );
    let step_budget = (budget * traffic.throttle_scale).clamp(0.0, budget);
    let front_blocked = scout_car_front_is_blocked(map, occ, e, current, budget.min(ARRIVE_EPS));
    let mut best = None;
    let mut best_score = f32::INFINITY;

    if step_budget > 0.01 {
        // A behind-the-car intermediate route point is often the pathfinder telling the car to
        // back out of a pocket before continuing. Keep distant final clicks on the established
        // forward-turn behavior, but let reverse compete with forward arcs for this local exit.
        let waypoint_behind = route.next_index > 0
            && !route.direct_goal_is_clear
            && waypoint_is_behind(e.facing(), route.target, current);
        for primitive in
            scout_car_primitives(profile, route_reverse_waypoint.is_some(), waypoint_behind)
        {
            let travel_distance =
                primitive_travel_distance(current, &route, primitive, step_budget, e.facing());
            if travel_distance <= 0.01 {
                continue;
            }
            let Some(candidate) =
                scout_car_candidate(profile, map, occ, e, current, primitive, travel_distance)
            else {
                continue;
            };
            let score = score_candidate(
                entities,
                spatial,
                id,
                current,
                e.facing(),
                &route,
                &candidate,
                profile,
                front_blocked,
            );
            if score + profile.score_eps < best_score {
                best = Some(candidate);
                best_score = score;
            }
        }
    }

    let Some(candidate) = best.or_else(|| {
        scout_car_outward_displacement_candidate(profile, map, occ, e, current, &route, step_budget)
    }) else {
        return Some(ScoutCarMotionPlan {
            pos: current,
            facing: None,
            reverse_waypoint: route_reverse_waypoint,
            static_blocked: true,
            pop_waypoints: route.pre_pop_count,
        });
    };

    let (pos, post_pop_count) = scout_car_post_motion_waypoint_pops(map, occ, e, &route, candidate);
    let reverse_waypoint = if candidate.primitive.travel_sign < 0.0 {
        Some(route.target)
    } else {
        route_reverse_waypoint
    };
    Some(ScoutCarMotionPlan {
        pos,
        facing: Some(candidate.facing),
        reverse_waypoint: reverse_waypoint.filter(|wp| {
            route.pre_pop_count + post_pop_count == 0
                || e.movement
                    .as_ref()
                    .and_then(|m| m.path.get(route.next_index.saturating_sub(post_pop_count)))
                    .is_some_and(|next| distance_between(*next, *wp) <= ARRIVE_EPS)
        }),
        static_blocked: false,
        pop_waypoints: route.pre_pop_count + post_pop_count,
    })
}

pub(super) fn scout_car_final_goal_tolerance() -> f32 {
    config::SCOUT_CAR_FINAL_GOAL_TOLERANCE_PX
}

#[cfg(test)]
pub(super) fn scout_car_desired_path_point(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    x: f32,
    y: f32,
) -> Option<(f32, f32)> {
    vehicle_desired_path_point(map, occ, e, x, y)
}

fn scout_car_primitives(
    profile: CarMotionProfile,
    reverse_only: bool,
    allow_reverse: bool,
) -> Vec<Primitive> {
    if reverse_only {
        return vec![Primitive {
            curvature: 0.0,
            travel_sign: -1.0,
            ordinal: 0,
        }];
    }

    let max = 1.0 / profile.min_turn_radius_px;
    let mut primitives: Vec<_> = [
        0.0,
        max * 0.35,
        -max * 0.35,
        max * 0.65,
        -max * 0.65,
        max,
        -max,
    ]
    .into_iter()
    .enumerate()
    .map(|(ordinal, curvature)| Primitive {
        curvature,
        travel_sign: 1.0,
        ordinal: ordinal as u8,
    })
    .collect();
    if allow_reverse {
        primitives.push(Primitive {
            curvature: 0.0,
            travel_sign: -1.0,
            ordinal: primitives.len() as u8,
        });
    }
    primitives
}

fn primitive_travel_distance(
    current: (f32, f32),
    route: &VehicleRouteContext,
    primitive: Primitive,
    step_budget: f32,
    facing: f32,
) -> f32 {
    if primitive.curvature.abs() > 1.0e-5 {
        return step_budget;
    }
    let forward = unit_direction_for_facing(facing).unwrap_or(route.route_dir);
    let travel_dir = if primitive.travel_sign < 0.0 {
        (-forward.0, -forward.1)
    } else {
        forward
    };
    let delta = (route.target.0 - current.0, route.target.1 - current.1);
    let dist = distance_between(current, route.target);
    if dist <= step_budget && step_can_reach_waypoint(delta, travel_dir, step_budget) {
        dist
    } else {
        step_budget
    }
}

fn scout_car_candidate(
    profile: CarMotionProfile,
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    current: (f32, f32),
    primitive: Primitive,
    travel_distance: f32,
) -> Option<Candidate> {
    let steps = (travel_distance / profile.sweep_sample_step_px)
        .ceil()
        .max(1.0) as u32;
    let mut min_clearance = f32::INFINITY;
    let mut last_pos = current;
    let mut last_facing = e.facing();
    for i in 0..=steps {
        let d = travel_distance * i as f32 / steps as f32;
        let (pos, facing) = sample_primitive(current, e.facing(), primitive, d)?;
        if !static_standability::unit_static_standable_with_facing(
            map, occ, e.kind, pos.0, pos.1, facing,
        ) {
            return None;
        }
        min_clearance =
            min_clearance.min(static_clearance_px(profile, map, occ, e.kind, pos, facing));
        last_pos = pos;
        last_facing = facing;
    }
    let moved = (last_pos.0 - current.0, last_pos.1 - current.1);
    let travel_dir = unit_direction((0.0, 0.0), moved).unwrap_or_else(|| {
        let f = (last_facing.cos(), last_facing.sin());
        if primitive.travel_sign < 0.0 {
            (-f.0, -f.1)
        } else {
            f
        }
    });
    Some(Candidate {
        pos: last_pos,
        facing: last_facing,
        travel_dir,
        primitive,
        min_static_clearance_px: min_clearance,
    })
}

fn scout_car_outward_displacement_candidate(
    profile: CarMotionProfile,
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    current: (f32, f32),
    route: &VehicleRouteContext,
    budget: f32,
) -> Option<Candidate> {
    if !budget.is_finite() || budget <= 0.01 || !e.facing().is_finite() {
        return None;
    }
    let base_clearance = static_clearance_px(profile, map, occ, e.kind, current, e.facing());
    let forward = (e.facing().cos(), e.facing().sin());
    let side = (-forward.1, forward.0);
    let distance = budget.min(config::TILE_SIZE as f32 * 0.25);
    let mut best = None;
    let mut best_score = f32::INFINITY;

    for (ordinal, dir) in [side, (-side.0, -side.1)].into_iter().enumerate() {
        let candidate_pos = (current.0 + dir.0 * distance, current.1 + dir.1 * distance);
        if !static_swept_segment_legal(map, occ, e.kind, current, candidate_pos, e.facing()) {
            continue;
        }
        let clearance = static_clearance_px(profile, map, occ, e.kind, candidate_pos, e.facing());
        if clearance + 0.001 < base_clearance {
            continue;
        }
        let route_dist = distance_between(candidate_pos, route.lookahead);
        let score = route_dist - clearance * 2.0 + ordinal as f32 * 0.001;
        if score + profile.score_eps < best_score {
            best_score = score;
            best = Some(Candidate {
                pos: candidate_pos,
                facing: e.facing(),
                travel_dir: dir,
                primitive: Primitive {
                    curvature: 0.0,
                    travel_sign: 1.0,
                    ordinal: 200 + ordinal as u8,
                },
                min_static_clearance_px: clearance,
            });
        }
    }

    best
}

fn sample_primitive(
    current: (f32, f32),
    facing: f32,
    primitive: Primitive,
    travel_distance: f32,
) -> Option<((f32, f32), f32)> {
    if !facing.is_finite() || !travel_distance.is_finite() || travel_distance < 0.0 {
        return None;
    }
    if primitive.travel_sign < 0.0 {
        let forward = (facing.cos(), facing.sin());
        return Some((
            (
                current.0 - forward.0 * travel_distance,
                current.1 - forward.1 * travel_distance,
            ),
            facing,
        ));
    }
    if primitive.curvature.abs() <= 1.0e-5 {
        let forward = (facing.cos(), facing.sin());
        return Some((
            (
                current.0 + forward.0 * travel_distance,
                current.1 + forward.1 * travel_distance,
            ),
            facing,
        ));
    }

    let yaw = primitive.curvature * travel_distance;
    let local_x = yaw.sin() / primitive.curvature;
    let local_y = (1.0 - yaw.cos()) / primitive.curvature;
    let cos = facing.cos();
    let sin = facing.sin();
    Some((
        (
            current.0 + cos * local_x - sin * local_y,
            current.1 + sin * local_x + cos * local_y,
        ),
        normalize_angle(facing + yaw),
    ))
}

#[allow(clippy::too_many_arguments)]
fn score_candidate(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    id: u32,
    current: (f32, f32),
    current_facing: f32,
    route: &VehicleRouteContext,
    candidate: &Candidate,
    profile: CarMotionProfile,
    front_blocked: bool,
) -> f32 {
    let route_progress = distance_between(current, route.lookahead)
        - distance_between(candidate.pos, route.lookahead);
    let goal_progress = distance_between(current, route.final_goal)
        - distance_between(candidate.pos, route.final_goal);
    let delta = (candidate.pos.0 - current.0, candidate.pos.1 - current.1);
    let along = along_track_error(delta, route.route_dir);
    let lateral = lateral_error(
        (
            route.lookahead.0 - candidate.pos.0,
            route.lookahead.1 - candidate.pos.1,
        ),
        route.route_dir,
    );
    let motion_heading = if candidate.primitive.travel_sign < 0.0 {
        normalize_angle(candidate.facing + std::f32::consts::PI)
    } else {
        candidate.facing
    };
    let route_heading = route.route_dir.1.atan2(route.route_dir.0);
    let current_heading = if candidate.primitive.travel_sign < 0.0 {
        normalize_angle(current_facing + std::f32::consts::PI)
    } else {
        current_facing
    };
    let travel_alignment = angle_delta(motion_heading, route_heading).abs();
    let current_alignment = angle_delta(current_heading, route_heading).abs();
    let alignment_improvement = current_alignment - travel_alignment;
    let clearance_penalty =
        (profile.clearance_score_max_px - candidate.min_static_clearance_px).max(0.0) * 0.85;
    let steering_penalty = candidate.primitive.curvature.abs() * profile.min_turn_radius_px * 1.25;
    let reverse_penalty = if candidate.primitive.travel_sign < 0.0 {
        12.0
    } else {
        0.0
    };
    let blocked_front_penalty = if front_blocked
        && candidate.primitive.travel_sign > 0.0
        && candidate.primitive.curvature.abs() <= 1.0e-5
    {
        6.0
    } else {
        0.0
    };

    -route_progress * 10.0 - goal_progress * 2.5 - along * 0.75
        + lateral * 0.08
        + travel_alignment * 8.0
        - alignment_improvement * 18.0
        + clearance_penalty
        + steering_penalty
        + reverse_penalty
        + traffic_penalty(entities, spatial, id, candidate)
        + blocked_front_penalty
        + candidate.primitive.ordinal as f32 * 0.001
}

fn traffic_penalty(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    id: u32,
    candidate: &Candidate,
) -> f32 {
    let query_radius = config::TILE_SIZE as f32 * 2.0 + MAX_UNIT_BOUNDING_RADIUS_PX;
    let mut neighbors: Vec<u32> = spatial
        .ids_in_circle_bbox(candidate.pos.0, candidate.pos.1, query_radius)
        .filter(|&neighbor_id| neighbor_id != id)
        .collect();
    neighbors.sort_unstable();
    neighbors.truncate(STEERING_MAX_NEIGHBORS);

    let Some(candidate_body) = unit_body_with_facing(
        EntityKind::ScoutCar,
        candidate.pos.0,
        candidate.pos.1,
        candidate.facing,
    ) else {
        return 1000.0;
    };
    let candidate_radius = candidate_body.bounding_radius();
    let mut penalty = 0.0;
    for neighbor_id in neighbors {
        let Some(neighbor) = entities.get(neighbor_id) else {
            continue;
        };
        if neighbor.hp == 0 || !neighbor.is_unit() {
            continue;
        }
        let neighbor_radius = unit_body_for_entity(neighbor)
            .map(|body| body.bounding_radius())
            .unwrap_or_else(|| neighbor.radius());
        let clearance = distance_between(candidate.pos, (neighbor.pos_x, neighbor.pos_y))
            - candidate_radius
            - neighbor_radius;
        if clearance < config::TILE_SIZE as f32 * 0.5 {
            penalty += (config::TILE_SIZE as f32 * 0.5 - clearance).max(0.0) * 0.25;
        }
    }

    penalty
}

fn scout_car_front_is_blocked(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    current: (f32, f32),
    probe_distance: f32,
) -> bool {
    if probe_distance <= 0.0 || !e.facing().is_finite() {
        return false;
    }
    let forward = (e.facing().cos(), e.facing().sin());
    let probe = (
        current.0 + forward.0 * probe_distance,
        current.1 + forward.1 * probe_distance,
    );
    !static_standability::unit_static_standable_with_facing(
        map,
        occ,
        e.kind,
        probe.0,
        probe.1,
        e.facing(),
    )
}

fn scout_car_post_motion_waypoint_pops(
    map: &Map,
    occ: &Occupancy,
    e: &Entity,
    route: &VehicleRouteContext,
    candidate: Candidate,
) -> ((f32, f32), usize) {
    let Some(path) = e.movement.as_ref().map(|m| m.path.as_slice()) else {
        return (candidate.pos, 0);
    };
    if e.path_policy() == crate::game::entity::RoutePolicy::FastestTerrainTime
        && route.pre_pop_count > 0
    {
        return (candidate.pos, 0);
    }
    let mut idx = route.next_index;
    let mut pops = 0usize;
    let mut pos = candidate.pos;

    while idx > 0 {
        let waypoint = path[idx];
        let next_waypoint = path[idx - 1];
        if !route_accepts_waypoint(map, occ, e, pos, waypoint, Some(next_waypoint)) {
            break;
        }
        pops += 1;
        idx -= 1;
        if e.path_policy() == crate::game::entity::RoutePolicy::FastestTerrainTime {
            break;
        }
    }

    if idx == 0 {
        let goal = path[0];
        let dist = distance_between(pos, goal);
        if dist <= ARRIVE_EPS
            && static_standability::unit_static_standable_with_facing(
                map,
                occ,
                e.kind,
                goal.0,
                goal.1,
                candidate.facing,
            )
        {
            pos = goal;
            pops += 1;
        } else if dist <= scout_car_final_goal_tolerance()
            && static_standability::unit_static_standable_with_facing(
                map,
                occ,
                e.kind,
                pos.0,
                pos.1,
                candidate.facing,
            )
        {
            let delta = (goal.0 - pos.0, goal.1 - pos.1);
            let along = along_track_error(delta, candidate.travel_dir);
            let lateral = lateral_error(delta, candidate.travel_dir);
            if lateral > along.abs() && lateral > ARRIVE_EPS {
                pops += 1;
            }
        } else {
            let pass_by_tolerance = if candidate.primitive.travel_sign < 0.0 {
                config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX
            } else {
                scout_car_final_goal_tolerance()
            };
            if dist > pass_by_tolerance {
                return (pos, pops);
            }
            let delta_from_target = (pos.0 - goal.0, pos.1 - goal.1);
            let along = along_track_error(delta_from_target, candidate.travel_dir);
            let lateral = lateral_error(delta_from_target, candidate.travel_dir);
            if along > 0.0 && lateral <= pass_by_tolerance {
                pops += 1;
            }
        }
    }

    (pos, pops)
}

fn scout_car_reverse_waypoint(e: &Entity, x: f32, y: f32) -> Option<(f32, f32)> {
    let movement = e.movement.as_ref()?;
    let next = e.next_waypoint()?;
    let latched = movement
        .scout_car_reverse_waypoint
        .is_some_and(|latched| same_waypoint(latched, next));
    if latched && waypoint_is_behind(e.facing(), next, (x, y)) {
        return Some(next);
    }
    if latched {
        return None;
    }

    if !waypoint_is_behind(e.facing(), next, (x, y)) {
        return None;
    }

    let dx = next.0 - x;
    let dy = next.1 - y;
    let dist = (dx * dx + dy * dy).sqrt();
    if !dist.is_finite() || dist <= 1.0e-4 {
        return None;
    }

    let is_final_waypoint = movement.path.len() == 1;
    if is_final_waypoint && dist <= VEHICLE_REVERSE_GOAL_DISTANCE_PX {
        return Some(next);
    }

    None
}

fn waypoint_is_behind(facing: f32, waypoint: (f32, f32), current: (f32, f32)) -> bool {
    if !facing.is_finite() {
        return false;
    }
    let dx = waypoint.0 - current.0;
    let dy = waypoint.1 - current.1;
    let dist = (dx * dx + dy * dy).sqrt();
    if !dist.is_finite() || dist <= 1.0e-4 {
        return false;
    }
    let forward_desired = dy.atan2(dx);
    angle_delta(facing, forward_desired).abs()
        > car_motion_profile(EntityKind::ScoutCar)
            .map(|profile| profile.reverse_min_behind_angle_rad)
            .unwrap_or(std::f32::consts::FRAC_PI_2)
}

fn same_waypoint(a: (f32, f32), b: (f32, f32)) -> bool {
    distance_between(a, b) <= ARRIVE_EPS
}

fn step_can_reach_waypoint(delta: (f32, f32), step_dir: (f32, f32), budget: f32) -> bool {
    if !budget.is_finite() || budget < 0.0 {
        return false;
    }
    let dist = (delta.0 * delta.0 + delta.1 * delta.1).sqrt();
    if !dist.is_finite() || dist > budget {
        return false;
    }
    let along = delta.0 * step_dir.0 + delta.1 * step_dir.1;
    let lateral = (delta.0 * step_dir.1 - delta.1 * step_dir.0).abs();
    along >= -ARRIVE_EPS && lateral <= ARRIVE_EPS
}

fn unit_direction_for_facing(facing: f32) -> Option<(f32, f32)> {
    if !facing.is_finite() {
        return None;
    }
    Some((facing.cos(), facing.sin()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumed_reverse_latch_does_not_apply_to_following_waypoint() {
        let mut map = Map::generate(1, 0xC0FF_EE01);
        for terrain in &mut map.terrain {
            *terrain = crate::protocol::terrain::GRASS;
        }
        let mut entities = EntityStore::new();
        let (sx, sy) = map.tile_center(20, 20);
        let intermediate = (sx - config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX + 2.0, sy);
        let goal = (sx + config::TILE_SIZE as f32 * 4.0, sy);
        let scout = entities
            .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
            .expect("scout car should spawn");
        let entity = entities.get_mut(scout).expect("scout car should exist");
        entity.set_facing(0.0);
        entity.set_path(vec![goal, intermediate]);
        entity.set_path_goal(Some(goal));
        entity
            .movement
            .as_mut()
            .expect("scout car should have movement state")
            .scout_car_reverse_waypoint = Some(intermediate);

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        let entity = entities
            .get(scout)
            .expect("scout car should still exist")
            .clone();
        let plan = plan_scout_car_motion(
            &map,
            &occ,
            &entities,
            &spatial,
            scout,
            &entity,
            (sx, sy),
            1.0,
        )
        .expect("scout car should produce a movement plan");

        assert_eq!(plan.pop_waypoints, 1);
        assert!(
            plan.pos.0 > sx,
            "consuming the latched intermediate should resume toward the next waypoint"
        );
        assert_eq!(plan.reverse_waypoint, None);
    }
}
