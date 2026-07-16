use super::{
    formation, standability, unit_bodies_intersect, unit_body_for_entity, unit_body_with_facing,
    Occupancy, UnitBody,
};
use crate::config;
use crate::game::entity::{Entity, EntityStore, MovePhase};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::rules::projection;

/// Resolve a produced unit's rally location to the closest reachable, unoccupied,
/// body-standable tile. The producer retains the original rally coordinate; this is only the new
/// unit's invisible arrival goal. Ties use deterministic ring order.
#[allow(clippy::too_many_arguments)]
pub(super) fn nearest_free_goal(
    map: &Map,
    occ: &Occupancy<'_>,
    entities: &EntityStore,
    teams: &TeamRelations,
    player: u32,
    unit_id: u32,
    rally: (f32, f32),
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> Option<(f32, f32)> {
    if !rally.0.is_finite() || !rally.1.is_finite() {
        return None;
    }
    let unit = entities.get(unit_id)?;
    if !unit.is_unit() {
        return None;
    }
    let formation_unit = formation::FormationUnit {
        id: unit_id,
        kind: unit.kind,
        pos: (unit.pos_x, unit.pos_y),
    };
    let occupied_unit_bodies =
        occupied_and_reserved_bodies(entities, teams, player, unit_id, fog, smokes);
    let mut reachability = formation::FormationReachability::new(map, occ);
    let max_tile = map.size.checked_sub(1)?;
    let anchor = map.tile_of(rally.0, rally.1);
    let max_radius = anchor
        .0
        .max(anchor.1)
        .max(max_tile.saturating_sub(anchor.0))
        .max(max_tile.saturating_sub(anchor.1));
    let mut best: Option<(f32, (f32, f32))> = None;

    for radius in 0..=max_radius {
        let min_x = anchor.0.saturating_sub(radius);
        let max_x = anchor.0.saturating_add(radius).min(max_tile);
        let min_y = anchor.1.saturating_sub(radius);
        let max_y = anchor.1.saturating_add(radius).min(max_tile);
        for ty in min_y..=max_y {
            for tx in min_x..=max_x {
                if tx.abs_diff(anchor.0).max(ty.abs_diff(anchor.1)) != radius {
                    continue;
                }
                let point = map.tile_center(tx, ty);
                let facing = formation::formation_goal_facing(&formation_unit, point);
                if !standability::unit_static_standable_with_facing(
                    map, occ, unit.kind, point.0, point.1, facing,
                ) {
                    continue;
                }
                let Some(candidate_body) =
                    unit_body_with_facing(unit.kind, point.0, point.1, facing)
                else {
                    continue;
                };
                if occupied_unit_bodies
                    .iter()
                    .copied()
                    .any(|body| unit_bodies_intersect(candidate_body, body))
                    || !reachability.can_reach(&formation_unit, (tx, ty))
                {
                    continue;
                }

                let distance_sq = (point.0 - rally.0).powi(2) + (point.1 - rally.1).powi(2);
                if !distance_sq.is_finite() {
                    continue;
                }
                if best.is_none_or(|(best_distance_sq, _)| distance_sq < best_distance_sq) {
                    best = Some((distance_sq, point));
                }
            }
        }

        let Some((best_distance_sq, best_point)) = best else {
            continue;
        };
        if nearest_unsearched_tile_distance_sq(map, rally, min_x, max_x, min_y, max_y)
            .is_none_or(|lower_bound_sq| best_distance_sq <= lower_bound_sq)
        {
            return Some(best_point);
        }
    }

    best.map(|(_, point)| point)
}

fn occupied_and_reserved_bodies(
    entities: &EntityStore,
    teams: &TeamRelations,
    player: u32,
    unit_id: u32,
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> Vec<UnitBody> {
    let mut bodies = Vec::new();
    let mut rally_viewers = teams.same_team_player_ids(player);
    if !rally_viewers.contains(&player) {
        rally_viewers.push(player);
    }
    for other in entities.iter() {
        if other.id == unit_id || other.hp == 0 || !other.is_unit() {
            continue;
        }
        let friendly = teams.same_team_or_same_owner(player, other.owner);
        if !friendly && !blocker_visible_to_team(other, &rally_viewers, fog, smokes) {
            continue;
        }
        if let Some(body) = unit_body_for_entity(other) {
            bodies.push(body);
        }
        if !friendly
            || !matches!(
                other.move_phase(),
                Some(MovePhase::AwaitingPath | MovePhase::Moving)
            )
        {
            continue;
        }
        let Some(goal) = other.path_goal() else {
            continue;
        };
        let other_unit = formation::FormationUnit {
            id: other.id,
            kind: other.kind,
            pos: (other.pos_x, other.pos_y),
        };
        let facing = formation::formation_goal_facing(&other_unit, goal);
        if let Some(body) = unit_body_with_facing(other.kind, goal.0, goal.1, facing) {
            bodies.push(body);
        }
    }
    bodies
}

fn blocker_visible_to_team(
    entity: &Entity,
    viewers: &[u32],
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> bool {
    viewers
        .iter()
        .copied()
        .any(|viewer| projection::entity_visible_to_with_smoke(viewer, entity, fog, smokes))
}

/// A lower bound on the squared distance from `rally` to any tile center outside the scanned
/// rectangle. A candidate at or below this value is necessarily closest on the whole map.
fn nearest_unsearched_tile_distance_sq(
    map: &Map,
    rally: (f32, f32),
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
) -> Option<f32> {
    let max_tile = map.size.checked_sub(1)?;
    let tile_size = config::TILE_SIZE as f32;
    let mut lower_bound_sq = f32::INFINITY;
    let mut consider_axis_distance = |tile_center: f32, rally_coordinate: f32| {
        lower_bound_sq = lower_bound_sq.min((tile_center - rally_coordinate).powi(2));
    };

    if min_x > 0 {
        consider_axis_distance((min_x as f32 - 0.5) * tile_size, rally.0);
    }
    if max_x < max_tile {
        consider_axis_distance((max_x as f32 + 1.5) * tile_size, rally.0);
    }
    if min_y > 0 {
        consider_axis_distance((min_y as f32 - 0.5) * tile_size, rally.1);
    }
    if max_y < max_tile {
        consider_axis_distance((max_y as f32 + 1.5) * tile_size, rally.1);
    }

    lower_bound_sq.is_finite().then_some(lower_bound_sq)
}
