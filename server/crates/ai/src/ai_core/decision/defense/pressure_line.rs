//! Pressure's home Machine Gunners cover the ground approach, including bends in the route.
use super::*;

pub(in crate::ai_core::decision) fn stage_pressure_machine_gunners(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    profile: &AiProfile,
    ready_units: &[u32],
    has_home_tank: bool,
) -> Option<Vec<u32>> {
    let policy = profile.defensive_machine_gunners?;
    let screen = profile.home_anti_tank.filter(|_| has_home_tank);
    let distance = policy.perimeter_distance_tiles
        + screen.map_or(0.0, |policy| policy.machine_gunner_screen_tiles);
    let spacing = screen.map_or(policy.lateral_spacing_tiles, |policy| {
        policy.lateral_spacing_tiles
    });
    let analysis = map_analysis?;
    // Cached routes run from the enemy main into our original main. Walk them outward.
    let route: Vec<_> = analysis
        .base_route_tiles(observation.player_id)?
        .iter()
        .rev()
        .map(|tile| tile_center((tile.x, tile.y), observation.map.tile_size))
        .collect();
    let (center, direction) = route_basis(&route, distance * observation.map.tile_size as f32)?;
    let assignments = line::defensive_line_assignments_at(
        observation,
        ready_units,
        center,
        direction,
        spacing,
        policy.target_count,
    )?
    .into_iter()
    .filter_map(|assignment| {
        // Keep clearance searches in the route's frame instead of turning back toward the
        // enemy's straight-line bearing. Prefer the original slot, then nearby clear ground.
        let ts = observation.map.tile_size as f32;
        (0..=DEFENSIVE_FIRING_POSITION_SEARCH_TILES).find_map(|forward| {
            let point = (
                assignment.x + direction.0 * forward as f32 * ts,
                assignment.y + direction.1 * forward as f32 * ts,
            );
            (defensive_position_is_open(observation, map_analysis, point.0, point.1)
                && analysis.open_ground_near(center, point, 1, 0).is_some()
                && defensive_firing_sector_is_clear(
                    observation,
                    map_analysis,
                    point,
                    direction,
                    DEFENSIVE_FIRING_LANE_TILES,
                ))
            .then_some(DefensiveLineAssignment {
                unit_id: assignment.unit_id,
                x: point.0,
                y: point.1,
            })
        })
    })
    .collect();
    if has_home_tank {
        move_and_hold_machine_gunner_slots(actions, observation, assignments)
    } else {
        line::attack_move_to_line_slots(actions, observation, assignments)
    }
}

/// Interpolate the requested walking distance and use the local segment's outward tangent.
fn route_basis(route: &[(f32, f32)], distance: f32) -> Option<((f32, f32), (f32, f32))> {
    let mut remaining = distance.max(0.0);
    let mut last = None;
    for segment in route.windows(2) {
        let (from, to) = (segment[0], segment[1]);
        let Some(direction) = normalized_direction(from, to) else {
            continue;
        };
        let length = (to.0 - from.0).hypot(to.1 - from.1);
        if remaining <= length {
            return Some((
                (
                    from.0 + direction.0 * remaining,
                    from.1 + direction.1 * remaining,
                ),
                direction,
            ));
        }
        remaining -= length;
        last = Some((to, direction));
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bent_approach_uses_walking_distance_and_local_tangent() {
        let route = [(0.0, 0.0), (0.0, 100.0), (100.0, 100.0)];
        assert_eq!(route_basis(&route, 60.0), Some(((0.0, 60.0), (0.0, 1.0))));
        assert_eq!(
            route_basis(&route, 140.0),
            Some(((40.0, 100.0), (1.0, 0.0)))
        );
        assert_eq!(
            route_basis(&route, 300.0),
            Some(((100.0, 100.0), (1.0, 0.0)))
        );
        assert_eq!(route_basis(&[], 10.0), None);
    }
}
