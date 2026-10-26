//! The main's defensive line: slots across the way to the enemy, placed from the main's steel or
//! from a given centre, and the orders that fill them.

use super::*;

pub(in crate::ai_core::decision) fn stage_main_steel_defensive_line(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
) -> Option<Vec<u32>> {
    stage_main_steel_defensive_line_with_spacing(
        actions,
        observation,
        ready_units,
        enemy_base,
        distance_tiles,
        EXPANSION_DEFENSIVE_LINE_SPACING_TILES,
        ready_units.len(),
    )
}

/// Units attack-move to their slots on a line through `line_center` facing `direction`; units
/// already on their slot get no order.
pub(in crate::ai_core::decision) fn stage_defensive_line_at(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    ready_units: &[u32],
    line_center: (f32, f32),
    direction: (f32, f32),
) -> Option<Vec<u32>> {
    let assignments = defensive_line_assignments_at(
        observation,
        ready_units,
        line_center,
        direction,
        EXPANSION_DEFENSIVE_LINE_SPACING_TILES,
        ready_units.len(),
    )?;
    attack_move_to_line_slots(actions, observation, assignments)
}

pub(in crate::ai_core::decision) fn main_steel_defensive_line_assignments(
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
    lateral_spacing_tiles: f32,
    formation_slots: usize,
) -> Option<Vec<DefensiveLineAssignment>> {
    if ready_units.is_empty() {
        return None;
    }
    let steel_center = main_steel_cluster_center(observation)?;
    let line_center =
        main_steel_line_center(observation, steel_center, enemy_base, distance_tiles)?;
    let direction = normalized_direction(steel_center, (enemy_base.x, enemy_base.y))?;
    defensive_line_assignments_at(
        observation,
        ready_units,
        line_center,
        direction,
        lateral_spacing_tiles,
        formation_slots,
    )
}

/// The centre of the main's defensive line: `distance_tiles` from the main's steel toward the
/// enemy base.
pub(in crate::ai_core::decision) fn main_steel_line_center(
    observation: &AiObservation,
    steel_center: (f32, f32),
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
) -> Option<(f32, f32)> {
    let (dir_x, dir_y) = normalized_direction(steel_center, (enemy_base.x, enemy_base.y))?;
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return None;
    }
    let front_distance = distance_tiles.max(1.0) * tile_size;
    Some(clamp_to_map(
        (
            steel_center.0 + dir_x * front_distance,
            steel_center.1 + dir_y * front_distance,
        ),
        observation.map,
    ))
}

pub(super) fn stage_main_steel_defensive_line_with_spacing(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
    lateral_spacing_tiles: f32,
    formation_slots: usize,
) -> Option<Vec<u32>> {
    let assignments = main_steel_defensive_line_assignments(
        observation,
        ready_units,
        enemy_base,
        distance_tiles,
        lateral_spacing_tiles,
        formation_slots,
    )?;
    attack_move_to_line_slots(actions, observation, assignments)
}

pub(super) fn attack_move_to_line_slots(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    assignments: Vec<DefensiveLineAssignment>,
) -> Option<Vec<u32>> {
    let units_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let close_enough_px =
        EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * observation.map.tile_size as f32;
    let close_enough2 = squared(close_enough_px);
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

/// Slots on a line through `line_center`, across `direction` (the way the line faces), spaced
/// `lateral_spacing_tiles` apart and centred on the middle of `formation_slots`.
pub(super) fn defensive_line_assignments_at(
    observation: &AiObservation,
    ready_units: &[u32],
    line_center: (f32, f32),
    direction: (f32, f32),
    lateral_spacing_tiles: f32,
    formation_slots: usize,
) -> Option<Vec<DefensiveLineAssignment>> {
    if ready_units.is_empty() {
        return None;
    }
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return None;
    }
    let (dir_x, dir_y) = direction;
    let perp = (-dir_y, dir_x);
    let spacing = lateral_spacing_tiles.max(0.0) * tile_size;
    let mut units = ready_units.to_vec();
    units.sort_unstable();
    units.dedup();
    let center_index = (formation_slots.max(units.len()).saturating_sub(1)) as f32 * 0.5;

    let assignments = units
        .into_iter()
        .enumerate()
        .map(|(index, unit_id)| {
            let offset = (index as f32 - center_index) * spacing;
            let (x, y) = clamp_to_map(
                (
                    line_center.0 + perp.0 * offset,
                    line_center.1 + perp.1 * offset,
                ),
                observation.map,
            );
            DefensiveLineAssignment { unit_id, x, y }
        })
        .collect();
    Some(assignments)
}
