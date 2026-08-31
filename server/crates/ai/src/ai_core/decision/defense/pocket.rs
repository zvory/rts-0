use std::collections::BTreeMap;

use super::*;

const CENTRAL_BASE_APPROACH_MIN_ALIGNMENT: f32 = 0.9;
// (forward, lateral) offsets from the starting Resource Depot centre. These reproduce the
// approved River pocket, then rotate as one shape toward each base's central approach.
const RIFLE_SLOTS: [(f32, f32); 4] = [(4.25, 2.83), (4.25, -2.83), (8.15, -0.78), (8.15, 0.78)];
const MACHINE_GUNNER_SLOTS: [(f32, f32); 2] = [(7.5, -2.25), (7.5, 2.25)];

pub(in crate::ai_core::decision) fn stage_home_defensive_pocket_riflemen(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
) -> Option<Vec<u32>> {
    let assignments = home_defensive_pocket_rifle_assignments(
        observation,
        map_analysis,
        ready_units,
        enemy_base,
    )?;
    stage_home_rifleman_assignments(actions, observation, assignments)
}

pub(super) fn home_defensive_pocket_rifle_assignments(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
) -> Option<Vec<DefensiveLineAssignment>> {
    let mut units = ready_units.to_vec();
    units.sort_unstable();
    units.dedup();
    if units.is_empty() {
        return None;
    }

    let (anchor, direction) = defensive_pocket_basis(observation, map_analysis, enemy_base)?;
    let mut assignments = units
        .iter()
        .take(RIFLE_SLOTS.len())
        .copied()
        .zip(RIFLE_SLOTS)
        .filter_map(|(unit_id, slot)| {
            let desired = slot_target(observation, anchor, direction, slot);
            clear_mobile_defensive_position(observation, map_analysis, desired)
                .map(|(x, y)| DefensiveLineAssignment { unit_id, x, y })
        })
        .collect::<Vec<_>>();

    // The four oldest home Riflemen own the pocket. Later surplus Riflemen retain the broader
    // envelope coverage so a large late-game group does not collapse into the six opening slots.
    if units.len() > RIFLE_SLOTS.len() {
        if let Some(mut supplemental) = home_rifleman_envelope_coverage_assignments(
            observation,
            map_analysis,
            &units[RIFLE_SLOTS.len()..],
            enemy_base,
        ) {
            assignments.append(&mut supplemental);
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
) -> Option<Vec<u32>> {
    let assignments = defensive_pocket_machine_gunner_assignments(
        observation,
        map_analysis,
        ready_units,
        enemy_base,
    )?;
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
    let target = map_analysis
        .and_then(|analysis| {
            central_base_approach(observation.player_id, analysis, anchor, map_center)
        })
        .unwrap_or(map_center);
    let direction = normalized_direction(anchor, target)
        .or_else(|| normalized_direction(anchor, (enemy_base.x, enemy_base.y)))?;
    Some((anchor, direction))
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

    #[test]
    fn central_base_approach_classifies_the_river_but_not_crossroads() {
        assert!(fixture_approaches("The River")
            .into_iter()
            .all(|approach| approach.is_some()));
        assert!(fixture_approaches("Crossroads")
            .into_iter()
            .all(|approach| approach.is_none()));
    }
}
