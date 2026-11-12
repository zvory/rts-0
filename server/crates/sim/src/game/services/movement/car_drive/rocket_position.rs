use super::*;
use crate::game::ability::{self, AbilityKind};
use crate::game::entity::{MovePhase, Order};

// Range approach still uses ordinary pathfinding. Inside range (or at the approach's
// end), choose legal driving arcs for the actual firing pose instead of a point.
pub(in crate::game::services::movement) fn barrage_maneuver_target(
    e: &Entity,
) -> Option<(f32, f32)> {
    let Order::Ability(order) = e.order() else {
        return None;
    };
    if e.kind != EntityKind::RocketLauncher || order.intent.ability != AbilityKind::Barrage {
        return None;
    }
    let target = (order.intent.x, order.intent.y);
    let range =
        ability::definition(AbilityKind::Barrage).range_tiles? as f32 * config::TILE_SIZE as f32;
    let min = ability::definition(AbilityKind::Barrage)
        .min_range_tiles
        .unwrap_or(0) as f32
        * config::TILE_SIZE as f32;
    let distance = distance_between((e.pos_x(), e.pos_y()), target);
    ((distance >= min && distance <= range)
        || (e.path_is_empty()
            && matches!(
                order.execution.phase,
                MovePhase::Arrived | MovePhase::Moving
            )))
    .then_some(target)
}

pub(in crate::game::services::movement) fn barrage_pose_ready(
    e: &Entity,
    target: (f32, f32),
) -> bool {
    let definition = ability::definition(AbilityKind::Barrage);
    let range = definition.range_tiles.unwrap_or(0) as f32 * config::TILE_SIZE as f32;
    let min = definition.min_range_tiles.unwrap_or(0) as f32 * config::TILE_SIZE as f32;
    let distance = distance_between((e.pos_x(), e.pos_y()), target);
    distance >= min
        && distance <= range
        && ability::barrage_facing_ready(
            e.facing(),
            (target.1 - e.pos_y()).det_atan2(target.0 - e.pos_x()),
        )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::game::services::movement) fn plan_barrage_maneuver(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    spatial: &SpatialIndex,
    id: u32,
    e: &Entity,
    current: (f32, f32),
    budget: f32,
    target: (f32, f32),
    directional_speed_multiplier: impl Fn((f32, f32)) -> f32,
) -> Option<ScoutCarMotionPlan> {
    let profile = car_motion_profile(e.kind)?;
    let definition = ability::definition(AbilityKind::Barrage);
    let range = definition.range_tiles? as f32 * config::TILE_SIZE as f32;
    let min = definition.min_range_tiles.unwrap_or(0) as f32 * config::TILE_SIZE as f32;
    let mut best = None;
    let mut best_score = f32::INFINITY;
    // Include reverse steering for tight turns and targets immediately behind the rack.
    for sign in [1.0, -1.0] {
        for mut primitive in scout_car_primitives(profile, false, false) {
            primitive.travel_sign = sign;
            // Probe the arc's travel direction, including reverse, for the same
            // directional terrain and ability modifiers used by routed movement.
            let (probe, _) = sample_primitive(current, e.facing(), primitive, budget)?;
            let direction = unit_direction(current, probe)?;
            let step_budget = budget
                * directional_speed_multiplier((
                    direction.0 * config::TILE_SIZE as f32,
                    direction.1 * config::TILE_SIZE as f32,
                ));
            let Some(candidate) =
                scout_car_candidate(profile, map, occ, e, current, primitive, step_budget)
            else {
                continue;
            };
            let distance = distance_between(candidate.pos, target);
            let bearing = (target.1 - candidate.pos.1).det_atan2(target.0 - candidate.pos.0);
            let range_error = (distance - range).max(0.0) + (min - distance).max(0.0);
            let score = angle_delta(candidate.facing, bearing).abs() * profile.min_turn_radius_px
                + range_error * 2.0
                + traffic_penalty(entities, spatial, id, &candidate)
                + if sign < 0.0 { 0.01 } else { 0.0 };
            if score + profile.score_eps < best_score {
                best_score = score;
                best = Some(candidate);
            }
        }
    }
    Some(ScoutCarMotionPlan {
        pos: best.map_or(current, |c| c.pos),
        facing: best.map(|c| c.facing),
        reverse_waypoint: None,
        static_blocked: best.is_none(),
        pop_waypoints: 0,
    })
}
