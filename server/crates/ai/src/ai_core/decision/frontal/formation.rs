use super::*;

pub(super) fn containment_repush_tank_count(
    policy: ExpansionContainmentPolicy,
    repush_count: usize,
) -> usize {
    policy.recovery_tanks_to_continue.saturating_add(
        repush_count
            .saturating_sub(1)
            .saturating_mul(policy.additional_tanks_per_repush),
    )
}

pub(super) fn containment_regroup_point(
    own_base: (f32, f32),
    enemy_base: EnemyBaseFact,
    map: AiMapSummary,
) -> Option<(f32, f32)> {
    let direction = normalized_direction(own_base, (enemy_base.x, enemy_base.y))?;
    let forward_distance = map.tile_size as f32 * 8.0;
    Some(clamp_to_map(
        (
            own_base.0 + direction.0 * forward_distance,
            own_base.1 + direction.1 * forward_distance,
        ),
        map,
    ))
}

pub(super) fn select_nearest_units(
    observation: &AiObservation,
    candidates: &mut Vec<u32>,
    point: (f32, f32),
    count: usize,
) {
    let units_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    candidates.sort_by(|left, right| {
        let left_distance = units_by_id.get(left).map_or(f32::INFINITY, |unit| {
            dist2(unit.x, unit.y, point.0, point.1)
        });
        let right_distance = units_by_id.get(right).map_or(f32::INFINITY, |unit| {
            dist2(unit.x, unit.y, point.0, point.1)
        });
        left_distance
            .total_cmp(&right_distance)
            .then_with(|| left.cmp(right))
    });
    candidates.truncate(count);
}

pub(super) fn select_rifle_escorts(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    anchor: (f32, f32),
) -> Vec<u32> {
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    let mut defensive_riflemen = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .map(|unit| unit.id)
        .collect::<Vec<_>>();
    defensive_riflemen.sort_unstable();
    defensive_riflemen.truncate(CONTAINMENT_HOME_RIFLE_RESERVE);
    let radius2 =
        (CONTAINMENT_ESCORT_SELECTION_RADIUS_TILES * observation.map.tile_size as f32).powi(2);
    let mut riflemen: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| {
            unit.kind == EntityKind::Rifleman
                && unit.is_complete
                && unit.hp > 0
                && unit.free_for_combat
                && !defensive_riflemen.contains(&unit.id)
                && dist2(unit.x, unit.y, anchor.0, anchor.1) <= radius2
        })
        .map(|unit| unit.id)
        .collect();
    let escort_count = riflemen
        .len()
        .div_ceil(2)
        .clamp(MIN_CONTAINMENT_RIFLE_ESCORTS, MAX_CONTAINMENT_RIFLE_ESCORTS);
    riflemen.sort_by(|left, right| {
        let left_distance = by_id.get(left).map_or(f32::INFINITY, |unit| {
            dist2(unit.x, unit.y, anchor.0, anchor.1)
        });
        let right_distance = by_id.get(right).map_or(f32::INFINITY, |unit| {
            dist2(unit.x, unit.y, anchor.0, anchor.1)
        });
        left_distance
            .total_cmp(&right_distance)
            .then_with(|| {
                memory
                    .estimated_entrenchment_ticks(observation, *left)
                    .cmp(&memory.estimated_entrenchment_ticks(observation, *right))
            })
            .then_with(|| left.cmp(right))
    });
    riflemen.truncate(escort_count);
    riflemen
}

pub(super) fn rifle_screen_point(
    tank_anchor: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
) -> Option<(f32, f32)> {
    let direction = normalized_direction(tank_anchor, objective)?;
    let forward = RIFLE_SCREEN_FORWARD_TILES * map.tile_size as f32;
    Some(clamp_to_map(
        (
            tank_anchor.0 + direction.0 * forward,
            tank_anchor.1 + direction.1 * forward,
        ),
        map,
    ))
}

pub(super) fn rifle_screen_points(
    tank_anchor: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    count: usize,
) -> Vec<(f32, f32)> {
    let Some(center) = rifle_screen_point(tank_anchor, objective, map) else {
        return Vec::new();
    };
    let Some(direction) = normalized_direction(tank_anchor, objective) else {
        return Vec::new();
    };
    let perpendicular = (-direction.1, direction.0);
    let spacing = RIFLE_SCREEN_SPACING_TILES * map.tile_size as f32;
    (0..count)
        .map(|index| {
            let (rank_index, rank_count, rank_back) = if index < RIFLE_SCREEN_FIRST_RANK {
                (index, count.min(RIFLE_SCREEN_FIRST_RANK), 0.0)
            } else {
                (
                    index - RIFLE_SCREEN_FIRST_RANK,
                    count - RIFLE_SCREEN_FIRST_RANK,
                    RIFLE_SCREEN_SECOND_RANK_BACK_TILES * map.tile_size as f32,
                )
            };
            let center_index = (rank_count.saturating_sub(1)) as f32 / 2.0;
            let offset = (rank_index as f32 - center_index) * spacing;
            clamp_to_map(
                (
                    center.0 + perpendicular.0 * offset - direction.0 * rank_back,
                    center.1 + perpendicular.1 * offset - direction.1 * rank_back,
                ),
                map,
            )
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(super) struct ContainmentFormation {
    pub(super) tanks: Vec<(u32, (f32, f32))>,
    pub(super) scout: (u32, (f32, f32)),
    pub(super) riflemen: Vec<(u32, (f32, f32))>,
}

impl ContainmentFormation {
    pub(super) fn unit_ids(&self) -> Vec<u32> {
        let mut units: Vec<u32> = self.tanks.iter().map(|(id, _)| *id).collect();
        units.push(self.scout.0);
        units.extend(self.riflemen.iter().map(|(id, _)| *id));
        units.sort_unstable();
        units.dedup();
        units
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn containment_formation(
    observation: &AiObservation,
    tanks: &[u32],
    scout: u32,
    riflemen: &[u32],
    tank_center: (f32, f32),
    own_base: (f32, f32),
    objective: (f32, f32),
    policy: ExpansionContainmentPolicy,
) -> Option<ContainmentFormation> {
    let toward_objective = normalized_direction(own_base, objective)?;
    let tanks = compact_tank_formation_assignments(
        observation,
        tanks,
        tank_center,
        toward_objective,
        observation.map,
        CONTAINMENT_TANK_SPACING_TILES,
    );
    let scout_point = scout_trailing_point(
        tank_center,
        own_base,
        objective,
        observation.map,
        policy.scout_trailing_tiles,
    )?;
    let rifle_points = rifle_screen_points(tank_center, objective, observation.map, riflemen.len());
    let riflemen = riflemen.iter().copied().zip(rifle_points).collect();
    Some(ContainmentFormation {
        tanks,
        scout: (scout, scout_point),
        riflemen,
    })
}

pub(super) fn formation_units_in_position(
    observation: &AiObservation,
    formation: &ContainmentFormation,
    tolerance_tiles: f32,
) -> bool {
    let tolerance = tolerance_tiles * observation.map.tile_size as f32;
    let tolerance2 = tolerance * tolerance;
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    let assignment_in_position = |(id, point): &(u32, (f32, f32))| {
        by_id
            .get(id)
            .is_some_and(|unit| dist2(unit.x, unit.y, point.0, point.1) <= tolerance2)
    };
    let tanks_ready = formation.tanks.iter().all(assignment_in_position);
    let scout_ready = assignment_in_position(&formation.scout);
    let riflemen_ready = formation.riflemen.is_empty()
        || formation
            .riflemen
            .iter()
            .filter(|assignment| assignment_in_position(assignment))
            .count()
            >= formation.riflemen.len().div_ceil(2);
    tanks_ready && scout_ready && riflemen_ready
}

pub(super) fn issue_containment_formation(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    formation: &ContainmentFormation,
    tanks_attack_move: bool,
) {
    let tolerance = 0.5 * observation.map.tile_size as f32;
    let tolerance2 = tolerance * tolerance;
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    for (tank_id, point) in &formation.tanks {
        if by_id
            .get(tank_id)
            .is_some_and(|tank| dist2(tank.x, tank.y, point.0, point.1) <= tolerance2)
        {
            continue;
        }
        if tanks_attack_move {
            actions::attack_move_units(actions, [*tank_id], point.0, point.1);
        } else {
            actions::move_units(actions, [*tank_id], point.0, point.1);
        }
    }
    if !by_id.get(&formation.scout.0).is_some_and(|scout| {
        dist2(scout.x, scout.y, formation.scout.1 .0, formation.scout.1 .1) <= tolerance2
    }) {
        actions::move_units(
            actions,
            [formation.scout.0],
            formation.scout.1 .0,
            formation.scout.1 .1,
        );
    }
    for (rifleman_id, point) in &formation.riflemen {
        if by_id
            .get(rifleman_id)
            .is_some_and(|rifleman| dist2(rifleman.x, rifleman.y, point.0, point.1) <= tolerance2)
        {
            continue;
        }
        actions::attack_move_units(actions, [*rifleman_id], point.0, point.1);
    }
}

pub(super) fn formation_command_due(memory: &AiDecisionMemory, tick: u32) -> bool {
    memory
        .containment_last_formation_command_tick
        .map(|last| tick.saturating_sub(last) >= CONTAINMENT_FORMATION_REISSUE_TICKS)
        .unwrap_or(true)
}

pub(super) fn note_formation_command(memory: &mut AiDecisionMemory, tick: u32) {
    memory.containment_last_formation_command_tick = Some(tick);
}

pub(super) fn store_waypoint(memory: &mut AiDecisionMemory, point: (f32, f32), tick: u32) {
    memory.containment_march_waypoint = Some((point.0.round() as i32, point.1.round() as i32));
    memory.containment_waypoint_started_tick = Some(tick);
}

pub(super) fn stored_waypoint(memory: &AiDecisionMemory) -> Option<(f32, f32)> {
    memory
        .containment_march_waypoint
        .map(|(x, y)| (x as f32, y as f32))
}

pub(super) fn retain_nearby_rifle_escorts(
    observation: &AiObservation,
    riflemen: &mut BTreeSet<u32>,
    anchor: (f32, f32),
) {
    let radius2 = (CONTAINMENT_RIFLE_COHESION_TILES * observation.map.tile_size as f32).powi(2);
    riflemen.retain(|id| {
        observation.owned.iter().any(|unit| {
            unit.id == *id && unit.hp > 0 && dist2(unit.x, unit.y, anchor.0, anchor.1) <= radius2
        })
    });
}

pub(super) fn nearby_rifle_escort_count(
    observation: &AiObservation,
    riflemen: &[u32],
    anchor: (f32, f32),
) -> usize {
    let radius2 = (CONTAINMENT_RIFLE_COHESION_TILES * observation.map.tile_size as f32).powi(2);
    observation
        .owned
        .iter()
        .filter(|unit| {
            riflemen.contains(&unit.id)
                && unit.hp > 0
                && dist2(unit.x, unit.y, anchor.0, anchor.1) <= radius2
        })
        .count()
}

pub(super) fn reset_containment_route(memory: &mut AiDecisionMemory) {
    memory.containment_march_waypoint = None;
    memory.containment_route.clear();
    memory.containment_route_index = 0;
    memory.containment_route_objective = None;
    memory.containment_waypoint_started_tick = None;
}

pub(super) fn next_containment_route_waypoint(
    memory: &mut AiDecisionMemory,
    analysis: Option<&AiMapAnalysis>,
    from: (f32, f32),
    destination: (f32, f32),
    map: AiMapSummary,
) -> (f32, f32) {
    let objective = (destination.0.round() as i32, destination.1.round() as i32);
    if memory.containment_route_objective != Some(objective)
        || memory.containment_route_index >= memory.containment_route.len()
    {
        let route = analysis
            .map(|analysis| {
                analysis.compact_group_route(
                    from,
                    destination,
                    CONTAINMENT_MARCH_STEP_TILES as usize,
                )
            })
            .unwrap_or_else(|| vec![short_march_waypoint(from, destination, map)]);
        memory.containment_route = route
            .into_iter()
            .map(|point| (point.0.round() as i32, point.1.round() as i32))
            .collect();
        memory.containment_route_index = 0;
        memory.containment_route_objective = Some(objective);
    }
    let point = memory
        .containment_route
        .get(memory.containment_route_index)
        .copied()
        .map(|(x, y)| (x as f32, y as f32))
        .unwrap_or(destination);
    memory.containment_route_index = memory.containment_route_index.saturating_add(1);
    point
}

fn short_march_waypoint(
    from: (f32, f32),
    destination: (f32, f32),
    map: AiMapSummary,
) -> (f32, f32) {
    let max_step = CONTAINMENT_MARCH_STEP_TILES * map.tile_size as f32;
    let distance = dist2(from.0, from.1, destination.0, destination.1).sqrt();
    if distance <= max_step || distance <= f32::EPSILON {
        return clamp_to_map(destination, map);
    }
    let scale = max_step / distance;
    clamp_to_map(
        (
            from.0 + (destination.0 - from.0) * scale,
            from.1 + (destination.1 - from.1) * scale,
        ),
        map,
    )
}

pub(super) fn issue_containment_recall(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    target: u32,
) -> Option<AiIntent> {
    let target_exists = observation
        .visible_enemies
        .iter()
        .any(|enemy| enemy.id == target);
    if !target_exists {
        return None;
    }
    if !memory.containment_recall_active {
        memory.containment_recall_active = true;
        memory.containment_last_formation_command_tick = None;
        reset_containment_route(memory);
    }
    let owned: BTreeSet<u32> = observation.owned.iter().map(|unit| unit.id).collect();
    let mut units = memory
        .containment_active_tanks
        .iter()
        .copied()
        .collect::<Vec<_>>();
    units.extend(memory.containment_active_scout);
    units.extend(memory.containment_active_riflemen.iter().copied());
    units.retain(|id| owned.contains(id));
    if units.is_empty() {
        return None;
    }
    if formation_command_due(memory, observation.tick) {
        actions::attack_units(actions, units.iter().copied(), target);
        note_formation_command(memory, observation.tick);
    }
    Some(AiIntent::Attack { units })
}

pub(super) fn tank_group_is_cohesive(
    observation: &AiObservation,
    tanks: &[u32],
    toward_objective: (f32, f32),
) -> bool {
    if tanks.len() <= 1 {
        return true;
    }
    let perpendicular = (-toward_objective.1, toward_objective.0);
    let positions: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id))
        .map(|unit| (unit.x, unit.y))
        .collect();
    if positions.len() != tanks.len() {
        return false;
    }
    let projection_span = |axis: (f32, f32)| {
        let mut minimum = f32::INFINITY;
        let mut maximum = f32::NEG_INFINITY;
        for position in &positions {
            let projection = position.0 * axis.0 + position.1 * axis.1;
            minimum = minimum.min(projection);
            maximum = maximum.max(projection);
        }
        maximum - minimum
    };
    let tile_size = observation.map.tile_size as f32;
    let lateral_limit = ((tanks.len().saturating_sub(1)) as f32 * CONTAINMENT_TANK_SPACING_TILES
        + CONTAINMENT_LATERAL_SLOP_TILES)
        * tile_size;
    projection_span(toward_objective) <= CONTAINMENT_LONGITUDINAL_SPREAD_TILES * tile_size
        && projection_span(perpendicular) <= lateral_limit
}

pub(super) fn formation_core_is_grouped(
    observation: &AiObservation,
    formation: &ContainmentFormation,
    own_base: (f32, f32),
    objective: (f32, f32),
) -> bool {
    let Some(toward_objective) = normalized_direction(own_base, objective) else {
        return false;
    };
    let tank_ids: Vec<u32> = formation.tanks.iter().map(|(id, _)| *id).collect();
    if !tank_group_is_cohesive(observation, &tank_ids, toward_objective) {
        return false;
    }
    let Some(tank_center) = group_center(observation, &tank_ids) else {
        return false;
    };
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    let tile_size = observation.map.tile_size as f32;
    let scout_close = by_id.get(&formation.scout.0).is_some_and(|scout| {
        dist2(scout.x, scout.y, tank_center.0, tank_center.1) <= (4.0 * tile_size).powi(2)
    });
    let nearby_riflemen = formation
        .riflemen
        .iter()
        .filter(|(id, _)| {
            by_id.get(id).is_some_and(|rifleman| {
                dist2(rifleman.x, rifleman.y, tank_center.0, tank_center.1)
                    <= (CONTAINMENT_RIFLE_COHESION_TILES * tile_size).powi(2)
            })
        })
        .count();
    let rifle_screen_close =
        formation.riflemen.is_empty() || nearby_riflemen >= formation.riflemen.len().div_ceil(2);
    scout_close && rifle_screen_close
}

pub(super) fn formation_vehicle_core_is_grouped(
    observation: &AiObservation,
    formation: &ContainmentFormation,
    own_base: (f32, f32),
    objective: (f32, f32),
) -> bool {
    let Some(toward_objective) = normalized_direction(own_base, objective) else {
        return false;
    };
    let tank_ids: Vec<u32> = formation.tanks.iter().map(|(id, _)| *id).collect();
    if !tank_group_is_cohesive(observation, &tank_ids, toward_objective) {
        return false;
    }
    let Some(tank_center) = group_center(observation, &tank_ids) else {
        return false;
    };
    observation
        .owned
        .iter()
        .find(|unit| unit.id == formation.scout.0)
        .is_some_and(|scout| {
            dist2(scout.x, scout.y, tank_center.0, tank_center.1)
                <= (4.0 * observation.map.tile_size as f32).powi(2)
        })
}

pub(super) fn rearmost_unit_position(
    observation: &AiObservation,
    unit_ids: &[u32],
    toward_objective: (f32, f32),
) -> Option<(f32, f32)> {
    observation
        .owned
        .iter()
        .filter(|unit| unit_ids.contains(&unit.id))
        .min_by(|left, right| {
            let left_progress = left.x * toward_objective.0 + left.y * toward_objective.1;
            let right_progress = right.x * toward_objective.0 + right.y * toward_objective.1;
            left_progress
                .total_cmp(&right_progress)
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|unit| (unit.x, unit.y))
}
