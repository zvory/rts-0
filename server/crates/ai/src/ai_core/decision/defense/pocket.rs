use std::collections::BTreeMap;

use super::*;

const CENTRAL_BASE_APPROACH_MIN_ALIGNMENT: f32 = 0.9;
const CROSSROADS_MAP_TILES: (u32, u32) = (126, 126);
const CROSSROADS_STARTS: [(u32, u32); 2] = [(47, 8), (117, 78)];
// (forward, lateral) offsets from the starting Resource Depot centre. These reproduce the
// approved River pocket, then rotate as one shape toward each base's central approach.
const RIFLE_SLOTS: [(f32, f32); 4] = [(4.25, 2.83), (4.25, -2.83), (8.15, -0.78), (8.15, 0.78)];
const MACHINE_GUNNER_SLOTS: [(f32, f32); 2] = [(7.5, -2.25), (7.5, 2.25)];
/// Four defensive guns hold one line across the approach: the two pocket posts plus one more on
/// each wing, half a tile farther forward, so an attack that stands off the centre guns is in
/// reach of all four at once rather than meeting them one pair at a time. A burst's scattered
/// bullets stray about a tile at combat range, so the posts stay 1.5 tiles apart.
const WIDE_MACHINE_GUNNER_SLOTS: [(f32, f32); 4] =
    [(7.5, -2.25), (7.5, 2.25), (8.0, -3.75), (8.0, 3.75)];

/// With `forest_posts`, a pocket Rifleman may hold a forest edge within this many tiles of its post
/// instead: hidden from the enemy until it fires, taking a quarter less damage, and dug in.
const FOREST_POST_SEARCH_TILES: i32 = 4;
/// Fog sight crosses at most four forest tiles, so a post inside a forest keeps watch only where its
/// sight lines leave the trees quickly; two leaves margin against the simulation's exact ray.
const FOREST_POST_MAX_FOREST_ON_SIGHT_LINE: u32 = 2;
/// The ground a post must see: its own pocket slot and this far beyond it toward the threat.
const FOREST_POST_WATCH_TILES: f32 = 5.0;
/// A forest post may sit at most this much farther back from the threat than its slot.
const FOREST_POST_MAX_RETREAT_TILES: f32 = 1.0;
const FOREST_POST_SPACING_TILES: f32 = 1.5;

/// `forest_posts` lets the four pocket Riflemen hold nearby forest edges instead of open ground.
pub(in crate::ai_core::decision) fn stage_home_defensive_pocket_riflemen(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    forest_posts: bool,
) -> Option<Vec<u32>> {
    let assignments = pocket_rifle_assignments(
        observation,
        map_analysis,
        ready_units,
        enemy_base,
        forest_posts,
    )?;
    stage_home_rifleman_assignments(actions, observation, assignments)
}

pub(super) fn home_defensive_pocket_rifle_assignments(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
) -> Option<Vec<DefensiveLineAssignment>> {
    pocket_rifle_assignments(observation, map_analysis, ready_units, enemy_base, false)
}

pub(super) fn pocket_rifle_assignments(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    forest_posts: bool,
) -> Option<Vec<DefensiveLineAssignment>> {
    let mut units = ready_units.to_vec();
    units.sort_unstable();
    units.dedup();
    if units.is_empty() {
        return None;
    }

    let (anchor, direction) = defensive_pocket_basis(observation, map_analysis, enemy_base)?;
    let forest_analysis = map_analysis.filter(|_| forest_posts);
    let mut forest_taken: Vec<(f32, f32)> = Vec::new();
    let mut assignments = Vec::new();
    for (unit_id, slot) in units
        .iter()
        .take(RIFLE_SLOTS.len())
        .copied()
        .zip(RIFLE_SLOTS)
    {
        let desired = slot_target(observation, anchor, direction, slot);
        let forest = forest_analysis.and_then(|analysis| {
            forest_post(
                observation,
                analysis,
                (anchor, direction),
                desired,
                &forest_taken,
            )
        });
        let point = match forest {
            Some(point) => {
                forest_taken.push(point);
                Some(point)
            }
            None => clear_mobile_defensive_position(observation, map_analysis, desired),
        };
        if let Some((x, y)) = point {
            assignments.push(DefensiveLineAssignment { unit_id, x, y });
        }
    }

    // The four oldest home Riflemen own the pocket. Later surplus Riflemen retain the broader
    // envelope coverage so a large late-game group does not collapse into the six opening slots.
    if units.len() > RIFLE_SLOTS.len() {
        if let Some(supplemental) = home_rifleman_envelope_coverage_assignments(
            observation,
            map_analysis,
            &units[RIFLE_SLOTS.len()..],
            enemy_base,
        ) {
            let mut reserved: Vec<_> = assignments.iter().map(|slot| (slot.x, slot.y)).collect();
            for mut slot in supplemental {
                if let Some(point) = separated_rifle_position(
                    observation,
                    map_analysis,
                    (slot.x, slot.y),
                    direction,
                    &reserved,
                ) {
                    slot.x = point.0;
                    slot.y = point.1;
                    reserved.push(point);
                    assignments.push(slot);
                }
            }
        }
    }

    (!assignments.is_empty()).then_some(assignments)
}

pub(in crate::ai_core::decision) fn stage_defensive_pocket_machine_gunners(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    target_count: usize,
) -> Option<Vec<u32>> {
    let assignments = if target_count > MACHINE_GUNNER_SLOTS.len() {
        wide_pocket_machine_gunner_assignments(observation, map_analysis, ready_units, enemy_base)
    } else {
        defensive_pocket_machine_gunner_assignments(
            observation,
            map_analysis,
            ready_units,
            enemy_base,
        )
    }?;
    let units_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let close_enough =
        EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * observation.map.tile_size as f32;
    let close_enough2 = squared(close_enough);
    let mut staged = Vec::new();
    for assignment in assignments {
        let Some(unit) = units_by_id.get(&assignment.unit_id).copied() else {
            continue;
        };
        if dist2(unit.x, unit.y, assignment.x, assignment.y) <= close_enough2 {
            continue;
        }
        if let Some(units) =
            actions::attack_move_units(actions, [assignment.unit_id], assignment.x, assignment.y)
        {
            staged.extend(units);
        }
    }
    (!staged.is_empty()).then_some(staged)
}

pub(super) fn defensive_pocket_machine_gunner_assignments(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
) -> Option<Vec<DefensiveLineAssignment>> {
    let (anchor, direction) = defensive_pocket_basis(observation, map_analysis, enemy_base)?;
    let mut units = ready_units.to_vec();
    units.sort_unstable();
    units.dedup();
    let assignments = units
        .into_iter()
        .take(MACHINE_GUNNER_SLOTS.len())
        .zip(MACHINE_GUNNER_SLOTS)
        .filter_map(|(unit_id, slot)| {
            let desired = slot_target(observation, anchor, direction, slot);
            clear_machine_gunner_position(observation, map_analysis, desired, direction)
                .map(|(x, y)| DefensiveLineAssignment { unit_id, x, y })
        })
        .collect::<Vec<_>>();
    (!assignments.is_empty()).then_some(assignments)
}

/// The four-gun line. A gun already on a post keeps it, so a loss never walks a dug-in gun out of
/// its trench to close the gap; the other guns take the free posts centre first, in id order.
pub(super) fn wide_pocket_machine_gunner_assignments(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
) -> Option<Vec<DefensiveLineAssignment>> {
    let (anchor, direction) = defensive_pocket_basis(observation, map_analysis, enemy_base)?;
    let mut posts: Vec<Option<(f32, f32)>> = WIDE_MACHINE_GUNNER_SLOTS
        .iter()
        .map(|slot| {
            let desired = slot_target(observation, anchor, direction, *slot);
            clear_machine_gunner_position(observation, map_analysis, desired, direction)
        })
        .collect();
    let mut units = ready_units.to_vec();
    units.sort_unstable();
    units.dedup();
    let positions: BTreeMap<u32, (f32, f32)> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, (entity.x, entity.y)))
        .collect();
    let on_post2 =
        squared(EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * observation.map.tile_size as f32);
    let mut assignments = Vec::new();
    let mut waiting = Vec::new();
    for unit_id in units {
        let held = positions.get(&unit_id).and_then(|&(x, y)| {
            posts
                .iter_mut()
                .find(|post| post.is_some_and(|(px, py)| dist2(x, y, px, py) <= on_post2))
                .and_then(Option::take)
        });
        match held {
            Some((x, y)) => assignments.push(DefensiveLineAssignment { unit_id, x, y }),
            None => waiting.push(unit_id),
        }
    }
    for unit_id in waiting {
        let Some((x, y)) = posts.iter_mut().find_map(Option::take) else {
            break;
        };
        assignments.push(DefensiveLineAssignment { unit_id, x, y });
    }
    (!assignments.is_empty()).then_some(assignments)
}

pub(super) fn defensive_pocket_basis(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    enemy_base: EnemyBaseFact,
) -> Option<((f32, f32), (f32, f32))> {
    let anchor = tile_center(observation.own_start_tile, observation.map.tile_size);
    let tile_size = observation.map.tile_size as f32;
    let map_center = (
        observation.map.width as f32 * tile_size * 0.5,
        observation.map.height as f32 * tile_size * 0.5,
    );
    if let Some(direction) = crossroads_wall_aware_direction_for_observation(observation) {
        return Some((anchor, direction));
    }
    // Face the side an attack actually arrives from: where the enemy's shortest ground route
    // enters the base. The map centre can point away from it; on Schone Tage raids come down the
    // northern corridor while the centre lies to the south, so the pocket sat behind the Depot.
    if let Some(direction) = map_analysis
        .and_then(|analysis| analysis.base_route_entry(observation.player_id))
        .and_then(|entry| normalized_direction(anchor, entry))
    {
        return Some((anchor, direction));
    }
    let target = map_analysis
        .and_then(|analysis| {
            central_base_approach(observation.player_id, analysis, anchor, map_center)
        })
        .unwrap_or(map_center);
    let direction = normalized_direction(anchor, target)
        .or_else(|| normalized_direction(anchor, (enemy_base.x, enemy_base.y)))?;
    Some((anchor, direction))
}

pub(super) fn crossroads_wall_aware_direction_for_observation(
    observation: &AiObservation,
) -> Option<(f32, f32)> {
    crossroads_wall_aware_direction(
        (observation.map.width, observation.map.height),
        observation.own_start_tile,
        observation.players.iter().map(|player| player.start_tile),
    )
}

fn crossroads_wall_aware_direction(
    map_tiles: (u32, u32),
    own_start_tile: (u32, u32),
    player_starts: impl IntoIterator<Item = (u32, u32)>,
) -> Option<(f32, f32)> {
    if map_tiles != CROSSROADS_MAP_TILES {
        return None;
    }
    let mut seen = [false; CROSSROADS_STARTS.len()];
    let mut start_count = 0_usize;
    for start in player_starts {
        let index = CROSSROADS_STARTS
            .iter()
            .position(|expected| *expected == start)?;
        if seen[index] {
            return None;
        }
        seen[index] = true;
        start_count += 1;
    }
    if start_count != CROSSROADS_STARTS.len() || seen.iter().any(|present| !present) {
        return None;
    }

    // Crossroads' two water walls block the direct spawn-to-spawn diagonal. These vectors retain
    // the approved six-slot shape while facing the southwest road-and-ground corridor from which
    // an attack can actually enter each base. They are the reviewed route-facing proposal plus
    // the requested additional 45-degree turn in the same direction at each spawn.
    let target = match own_start_tile {
        (47, 8) => (-2.0, 1.0),
        (117, 78) => (-5.0, 7.0),
        _ => return None,
    };
    normalized_direction((0.0, 0.0), target)
}

fn central_base_approach(
    player_id: u32,
    map_analysis: &AiMapAnalysis,
    anchor: (f32, f32),
    map_center: (f32, f32),
) -> Option<(f32, f32)> {
    let center_direction = normalized_direction(anchor, map_center)?;
    map_analysis
        .base_chokes_for_player(player_id, usize::MAX)
        .into_iter()
        .filter_map(|choke| {
            let approach_direction = normalized_direction(anchor, choke.enemy_approach_world)?;
            let alignment = center_direction.0 * approach_direction.0
                + center_direction.1 * approach_direction.1;
            (alignment >= CENTRAL_BASE_APPROACH_MIN_ALIGNMENT).then_some((
                choke.enemy_approach_world,
                alignment,
                dist2(
                    choke.enemy_approach_world.0,
                    choke.enemy_approach_world.1,
                    map_center.0,
                    map_center.1,
                ),
                choke.id,
            ))
        })
        .min_by(|left, right| {
            left.2
                .total_cmp(&right.2)
                .then_with(|| right.1.total_cmp(&left.1))
                .then_with(|| left.3.cmp(&right.3))
        })
        .map(|(target, _, _, _)| target)
}

fn slot_target(
    observation: &AiObservation,
    anchor: (f32, f32),
    direction: (f32, f32),
    slot: (f32, f32),
) -> (f32, f32) {
    let tile_size = observation.map.tile_size as f32;
    let perpendicular = (-direction.1, direction.0);
    clamp_to_map(
        (
            anchor.0 + direction.0 * slot.0 * tile_size + perpendicular.0 * slot.1 * tile_size,
            anchor.1 + direction.1 * slot.0 * tile_size + perpendicular.1 * slot.1 * tile_size,
        ),
        observation.map,
    )
}

/// The forest tile nearest `slot` that a pocket Rifleman can hold instead of the slot: dug-in
/// ground in a forest, within `FOREST_POST_SEARCH_TILES`, no more than a tile farther from the
/// threat, apart from the other forest posts, and with clear sight of the slot and of the ground
/// beyond it. A Rifleman deep in the trees would be hidden but blind, so posts sit on the edge.
fn forest_post(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    (anchor, direction): ((f32, f32), (f32, f32)),
    slot: (f32, f32),
    taken: &[(f32, f32)],
) -> Option<(f32, f32)> {
    let tile = observation.map.tile_size.max(1) as f32;
    let forward = |point: (f32, f32)| {
        ((point.0 - anchor.0) * direction.0 + (point.1 - anchor.1) * direction.1) / tile
    };
    let watch = clamp_to_map(
        (
            slot.0 + direction.0 * FOREST_POST_WATCH_TILES * tile,
            slot.1 + direction.1 * FOREST_POST_WATCH_TILES * tile,
        ),
        observation.map,
    );
    let (slot_x, slot_y) = world_tile(observation.map, slot.0, slot.1);
    let mut best: Option<((f32, f32), f32)> = None;
    for dy in -FOREST_POST_SEARCH_TILES..=FOREST_POST_SEARCH_TILES {
        for dx in -FOREST_POST_SEARCH_TILES..=FOREST_POST_SEARCH_TILES {
            let (Some(x), Some(y)) = (slot_x.checked_add_signed(dx), slot_y.checked_add_signed(dy))
            else {
                continue;
            };
            if !analysis.tile_is_concealment(x, y) || !analysis.tile_allows_entrenchment(x, y) {
                continue;
            }
            let point = tile_center((x, y), observation.map.tile_size);
            let distance2 = dist2(point.0, point.1, slot.0, slot.1);
            if distance2 > squared(FOREST_POST_SEARCH_TILES as f32 * tile)
                || forward(point) < forward(slot) - FOREST_POST_MAX_RETREAT_TILES
                || taken.iter().any(|other| {
                    dist2(point.0, point.1, other.0, other.1)
                        < squared(FOREST_POST_SPACING_TILES * tile)
                })
                || best.is_some_and(|(_, best_distance2)| distance2 >= best_distance2)
                || !defensive_position_is_open(observation, Some(analysis), point.0, point.1)
            {
                continue;
            }
            let watches = |target: (f32, f32)| {
                forest_tiles_on_sight_line(observation, analysis, point, target)
                    .is_some_and(|forest| forest <= FOREST_POST_MAX_FOREST_ON_SIGHT_LINE)
            };
            if watches(slot) && watches(watch) {
                best = Some((point, distance2));
            }
        }
    }
    best.map(|(point, _)| point)
}

/// The forest tiles a fog sight ray from `from` to `to` enters, not counting the tile it starts on,
/// sampled every quarter tile; `None` when rock or a building blocks it.
pub(super) fn forest_tiles_on_sight_line(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    from: (f32, f32),
    to: (f32, f32),
) -> Option<u32> {
    let tile = observation.map.tile_size.max(1) as f32;
    let samples = ((to.0 - from.0).hypot(to.1 - from.1) / tile * 4.0)
        .ceil()
        .max(1.0) as usize;
    let origin = world_tile(observation.map, from.0, from.1);
    let blockers = dynamic_los_blocking_tiles(observation);
    let mut last = origin;
    let mut forest = 0;
    for step in 1..=samples {
        let t = step as f32 / samples as f32;
        let current = world_tile(
            observation.map,
            from.0 + (to.0 - from.0) * t,
            from.1 + (to.1 - from.1) * t,
        );
        if current == last {
            continue;
        }
        last = current;
        if analysis.tile_blocks_line_of_sight(current.0, current.1) || blockers.contains(&current) {
            return None;
        }
        if analysis.tile_is_concealment(current.0, current.1) {
            forest += 1;
        }
    }
    Some(forest)
}

fn clear_machine_gunner_position(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    desired: (f32, f32),
    direction: (f32, f32),
) -> Option<(f32, f32)> {
    let perpendicular = (-direction.1, direction.0);
    let tile_size = observation.map.tile_size as f32;
    let mut offsets = vec![(0.0, 0.0)];
    for radius in 1..=DEFENSIVE_FIRING_POSITION_SEARCH_TILES {
        let radius = radius as f32;
        offsets.extend([
            (perpendicular.0 * radius, perpendicular.1 * radius),
            (-perpendicular.0 * radius, -perpendicular.1 * radius),
            (-direction.0 * radius, -direction.1 * radius),
            (
                (perpendicular.0 - direction.0) * radius,
                (perpendicular.1 - direction.1) * radius,
            ),
            (
                (-perpendicular.0 - direction.0) * radius,
                (-perpendicular.1 - direction.1) * radius,
            ),
        ]);
    }
    offsets.into_iter().find_map(|offset| {
        let candidate = clamp_to_map(
            (
                desired.0 + offset.0 * tile_size,
                desired.1 + offset.1 * tile_size,
            ),
            observation.map,
        );
        (defensive_position_is_open(observation, map_analysis, candidate.0, candidate.1)
            && defensive_firing_sector_is_clear(
                observation,
                map_analysis,
                candidate,
                direction,
                DEFENSIVE_FIRING_LANE_TILES,
            ))
        .then_some(candidate)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, MapMetadata, PlayerInit};

    const FIXTURE_SEED: u32 = 0x1234_5678;

    fn fixture(map_name: &str) -> (AiMapAnalysis, rts_sim::protocol::StartPayload) {
        let players = (1..=2)
            .map(|id| PlayerInit {
                id,
                team_id: id,
                faction_id: "kriegsia".to_string(),
                name: format!("P{id}"),
                color: format!("#{id}{id}{id}"),
                is_ai: true,
            })
            .collect::<Vec<_>>();
        let player_slots = players
            .iter()
            .map(|player| (player.id, player.team_id))
            .collect::<Vec<_>>();
        let map = Map::load_for_players(map_name, &player_slots, FIXTURE_SEED)
            .expect("fixture map should load");
        let metadata = Map::metadata_for_name(map_name).unwrap_or_else(|_| MapMetadata {
            name: map_name.to_string(),
            schema_version: rts_sim::game::map::CURRENT_MAP_VERSION,
            content_hash: "test".to_string(),
        });
        let game = Game::new_with_random_ai_profiles_and_map_metadata(
            &players,
            FIXTURE_SEED,
            map,
            metadata,
        );
        let start = game.start_payload();
        (AiMapAnalysis::analyze(&start), start)
    }

    fn fixture_approaches(map_name: &str) -> Vec<Option<(f32, f32)>> {
        let (analysis, start) = fixture(map_name);
        let tile_size = start.map.tile_size;
        let map_center = (
            start.map.width as f32 * tile_size as f32 * 0.5,
            start.map.height as f32 * tile_size as f32 * 0.5,
        );
        start
            .players
            .iter()
            .map(|player| {
                let anchor = tile_center(
                    (player.start_tile_x, player.start_tile_y),
                    start.map.tile_size,
                );
                central_base_approach(player.id, &analysis, anchor, map_center)
            })
            .collect()
    }

    /// Per start: the route-facing direction and the previous centre-facing direction.
    fn route_facing(map_name: &str) -> Vec<((f32, f32), (f32, f32))> {
        let (analysis, start) = fixture(map_name);
        let tile_size = start.map.tile_size as f32;
        let map_center = (
            start.map.width as f32 * tile_size * 0.5,
            start.map.height as f32 * tile_size * 0.5,
        );
        start
            .players
            .iter()
            .map(|player| {
                let anchor = tile_center(
                    (player.start_tile_x, player.start_tile_y),
                    start.map.tile_size,
                );
                let route = analysis
                    .base_route_entry(player.id)
                    .and_then(|entry| normalized_direction(anchor, entry))
                    .expect("every start has a route entry");
                let central = central_base_approach(player.id, &analysis, anchor, map_center)
                    .and_then(|approach| normalized_direction(anchor, approach))
                    .or_else(|| normalized_direction(anchor, map_center))
                    .expect("a start is never the map centre");
                (route, central)
            })
            .collect()
    }

    #[test]
    fn schone_tage_pocket_faces_the_northern_corridor_raids_arrive_through() {
        // Both mains sit on the east-west axis; attacks come down the northern corridor while the
        // map centre lies to the south.
        for (route, _) in route_facing("Schone Tage") {
            assert!(
                route.1 < -0.5,
                "Schone pocket should face north, got {route:?}"
            );
        }
    }

    #[test]
    fn route_facing_keeps_the_river_and_classic_pockets() {
        for map_name in ["The River", "Classic"] {
            for (route, central) in route_facing(map_name) {
                let alignment = route.0 * central.0 + route.1 * central.1;
                assert!(
                    alignment > 0.99,
                    "{map_name} pocket moved: {route:?} vs {central:?}"
                );
            }
        }
    }

    #[test]
    fn central_base_approach_classifies_the_river_but_not_crossroads() {
        assert!(fixture_approaches("The River")
            .into_iter()
            .all(|approach| approach.is_some()));
        assert!(fixture_approaches("Crossroads")
            .into_iter()
            .all(|approach| approach.is_none()));
    }

    #[test]
    fn crossroads_wall_aware_directions_match_the_approved_extra_rotation() {
        let (_, start) = fixture("Crossroads");
        let player_starts = start
            .players
            .iter()
            .map(|player| (player.start_tile_x, player.start_tile_y))
            .collect::<Vec<_>>();
        let p1 = normalized_direction((0.0, 0.0), (-2.0, 1.0)).unwrap();
        let p2 = normalized_direction((0.0, 0.0), (-5.0, 7.0)).unwrap();
        for player in &start.players {
            let start_tile = (player.start_tile_x, player.start_tile_y);
            let direction = crossroads_wall_aware_direction(
                (start.map.width, start.map.height),
                start_tile,
                player_starts.iter().copied(),
            )
            .expect("Crossroads should have a wall-aware pocket direction");
            let expected = match start_tile {
                (47, 8) => p1,
                (117, 78) => p2,
                _ => panic!("unexpected Crossroads start {start_tile:?}"),
            };
            assert!((direction.0 - expected.0).abs() < 0.0001);
            assert!((direction.1 - expected.1).abs() < 0.0001);
        }
    }
}
