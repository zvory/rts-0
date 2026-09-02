use super::*;

pub(super) fn containment_regroup_radius_tiles(
    policy: ExpansionContainmentPolicy,
    required_tanks: usize,
) -> f32 {
    policy.repush_regroup_radius_tiles
        + required_tanks.saturating_sub(policy.recovery_tanks_to_continue) as f32 * 1.5
}

pub(super) fn compact_group_near(
    observation: &AiObservation,
    unit_ids: &[u32],
    rally_point: (f32, f32),
    radius: f32,
) -> bool {
    let Some(center) = group_center(observation, unit_ids) else {
        return false;
    };
    let radius2 = radius * radius;
    dist2(center.0, center.1, rally_point.0, rally_point.1) <= radius2
        && observation
            .owned
            .iter()
            .filter(|unit| unit_ids.contains(&unit.id))
            .all(|unit| dist2(unit.x, unit.y, center.0, center.1) <= radius2)
}

fn beta_unit_position(observation: &AiObservation, unit_id: u32) -> Option<(f32, f32)> {
    observation
        .owned
        .iter()
        .find(|unit| unit.id == unit_id)
        .map(|unit| (unit.x, unit.y))
}

fn select_beta_rifle_escorts(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    anchor: (f32, f32),
) -> Vec<u32> {
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    let mut riflemen =
        actions::select_ready_combat_units(&observation.owned, &[EntityKind::Rifleman]);
    let escort_count = riflemen.len().div_ceil(2);
    riflemen.sort_by(|left, right| {
        memory
            .estimated_entrenchment_ticks(observation, *left)
            .cmp(&memory.estimated_entrenchment_ticks(observation, *right))
            .then_with(|| {
                let left_distance = by_id.get(left).map_or(f32::INFINITY, |unit| {
                    dist2(unit.x, unit.y, anchor.0, anchor.1)
                });
                let right_distance = by_id.get(right).map_or(f32::INFINITY, |unit| {
                    dist2(unit.x, unit.y, anchor.0, anchor.1)
                });
                left_distance.total_cmp(&right_distance)
            })
            .then_with(|| left.cmp(right))
    });
    riflemen.truncate(escort_count);
    riflemen
}

pub(super) fn issue_expansion_containment_wave(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    plan: &FrontalWavePlan,
    enemy_base: EnemyBaseFact,
    policy: ExpansionContainmentPolicy,
    tight_formation: bool,
    memory: &mut AiDecisionMemory,
) -> Option<AiIntent> {
    let natural_objective = enemy_natural_edge(observation, enemy_base)?;
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let tile_size = observation.map.tile_size as f32;

    let mut tanks = Vec::new();
    let mut scouts = Vec::new();
    for unit in observation
        .owned
        .iter()
        .filter(|unit| plan.ready_units.contains(&unit.id))
    {
        match unit.kind {
            EntityKind::Tank => tanks.push(unit.id),
            EntityKind::ScoutCar => scouts.push(unit.id),
            _ => {}
        }
    }
    if tanks.is_empty() || scouts.is_empty() {
        return None;
    }
    tanks.sort_unstable();
    scouts.sort_unstable();
    let owned: BTreeSet<u32> = observation.owned.iter().map(|unit| unit.id).collect();
    memory
        .containment_active_riflemen
        .retain(|rifleman| owned.contains(rifleman));
    if !memory.containment_wave_launched {
        if tanks.len() < policy.minimum_tanks_to_continue {
            return None;
        }
        tanks.truncate(policy.minimum_tanks_to_continue);
        scouts.truncate(1);
        memory.containment_opening_tanks = tanks.iter().copied().collect();
        memory.containment_active_tanks = tanks.iter().copied().collect();
        memory.containment_active_scout = scouts.first().copied();
        let escort_anchor = group_center(observation, &tanks).unwrap_or(own_base);
        memory.containment_active_riflemen =
            select_beta_rifle_escorts(observation, memory, escort_anchor)
                .into_iter()
                .collect();
        memory.containment_wave_launched = true;
    } else if memory.containment_recovery_active {
        let required = containment_repush_tank_count(policy, memory.containment_repush_count);
        let forward_rally = containment_regroup_point(own_base, enemy_base, observation.map)?;
        select_nearest_units(observation, &mut tanks, forward_rally, required);
        select_nearest_units(observation, &mut scouts, forward_rally, 1);
        let regroup_point = tanks
            .first()
            .and_then(|tank| beta_unit_position(observation, *tank))?;
        let regroup_radius = containment_regroup_radius_tiles(policy, required) * tile_size;
        let mut cohort = tanks.clone();
        cohort.extend(scouts.iter().copied());
        let grouped_at_home = tanks.len() == required
            && !scouts.is_empty()
            && compact_group_near(observation, &cohort, regroup_point, regroup_radius);
        if !grouped_at_home {
            if tight_formation {
                let toward_enemy = normalized_direction(own_base, (enemy_base.x, enemy_base.y))?;
                for (tank_id, point) in compact_tank_formation_assignments(
                    observation,
                    &tanks,
                    regroup_point,
                    toward_enemy,
                    observation.map,
                    1.5,
                ) {
                    actions::move_units(actions, [tank_id], point.0, point.1);
                }
            } else {
                actions::move_units(
                    actions,
                    tanks.iter().copied(),
                    regroup_point.0,
                    regroup_point.1,
                );
            }
            let scout_regroup = if tight_formation {
                scout_trailing_point(
                    regroup_point,
                    own_base,
                    (enemy_base.x, enemy_base.y),
                    observation.map,
                    policy.scout_trailing_tiles,
                )?
            } else {
                regroup_point
            };
            actions::move_units(
                actions,
                scouts.iter().copied(),
                scout_regroup.0,
                scout_regroup.1,
            );
            cohort.sort_unstable();
            cohort.dedup();
            return Some(AiIntent::Stage { units: cohort });
        }
        memory.containment_active_tanks = tanks.iter().copied().collect();
        memory.containment_active_scout = scouts.first().copied();
        memory.containment_active_riflemen =
            select_beta_rifle_escorts(observation, memory, regroup_point)
                .into_iter()
                .collect();
        memory.containment_recovery_active = false;
        memory.containment_stationary_since = None;
    } else {
        tanks.retain(|tank| memory.containment_active_tanks.contains(tank));
        scouts.retain(|scout| Some(*scout) == memory.containment_active_scout);
        if tanks.is_empty() || scouts.is_empty() {
            return None;
        }
    }
    update_enemy_natural_state(observation, natural_objective, enemy_base, &scouts, memory);
    if memory.enemy_natural_destroyed {
        update_enemy_main_state(observation, enemy_base, &tanks, &scouts, memory);
    }
    let endgame_search_active = memory.enemy_main_destroyed;
    let objective = if endgame_search_active {
        endgame_search_point(
            own_base,
            enemy_base,
            observation.map,
            memory.endgame_search_waypoint,
        )
    } else if memory.enemy_natural_destroyed {
        (enemy_base.x, enemy_base.y)
    } else {
        natural_objective
    };
    let (tank_point, legacy_scout_point) = if endgame_search_active {
        let scout_point = scout_forward_from_tanks(
            objective,
            own_base,
            objective,
            observation.map,
            policy.scout_forward_tiles,
        )?;
        (objective, scout_point)
    } else {
        containment_points(own_base, objective, observation.map, policy)?
    };
    let toward_objective = normalized_direction(own_base, objective)?;
    let tank_assignments = if tight_formation {
        compact_tank_formation_assignments(
            observation,
            &tanks,
            tank_point,
            toward_objective,
            observation.map,
            1.5,
        )
    } else {
        tanks.iter().map(|tank_id| (*tank_id, tank_point)).collect()
    };
    let tolerance = tile_size * if tight_formation { 1.0 } else { 2.0 };
    let tolerance2 = tolerance * tolerance;
    let tanks_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id))
        .map(|unit| (unit.id, unit))
        .collect();
    let tanks_in_position = tank_assignments.iter().all(|(tank_id, point)| {
        tanks_by_id
            .get(tank_id)
            .is_some_and(|tank| dist2(tank.x, tank.y, point.0, point.1) <= tolerance2)
    });
    let tank_anchor = if tight_formation {
        frontmost_unit_position(observation, &tanks, toward_objective)?
    } else {
        group_center(observation, &tanks)?
    };
    let trailing_point = scout_trailing_point(
        tank_anchor,
        own_base,
        objective,
        observation.map,
        policy.scout_trailing_tiles,
    )?;
    let contact_target =
        visible_combat_target_within_tiles(observation, &tanks, policy.contact_stop_tiles);
    let should_stop = tanks_in_position || contact_target.is_some();
    let stationary_range_ready = if should_stop {
        let since = memory
            .containment_stationary_since
            .get_or_insert(observation.tick);
        observation.tick.saturating_sub(*since) >= config::TICK_HZ * 3
    } else {
        memory.containment_stationary_since = None;
        false
    };

    if should_stop {
        if stationary_range_ready {
            let target = contact_target
                .or_else(|| {
                    visible_combat_target_within_tiles(
                        observation,
                        &tanks,
                        policy.tank_standoff_tiles,
                    )
                })
                .or_else(|| {
                    visible_strategic_building_target_within_tiles(
                        observation,
                        &tanks,
                        policy.contact_stop_tiles,
                    )
                });
            if let Some(target) = target {
                actions::attack_units(actions, tanks.iter().copied(), target);
            } else if endgame_search_active {
                memory.endgame_search_waypoint =
                    (memory.endgame_search_waypoint + 1) % ENDGAME_SEARCH_OFFSETS.len();
                memory.containment_stationary_since = None;
                let next = endgame_search_point(
                    own_base,
                    enemy_base,
                    observation.map,
                    memory.endgame_search_waypoint,
                );
                actions::attack_move_units(actions, tanks.iter().copied(), next.0, next.1);
            } else {
                actions::hold_position_units(actions, tanks.iter().copied());
            }
        } else {
            actions::hold_position_units(actions, tanks.iter().copied());
        }
    } else if tight_formation {
        for (tank_id, point) in &tank_assignments {
            actions::attack_move_units(actions, [*tank_id], point.0, point.1);
        }
    } else {
        actions::attack_move_units(actions, tanks.iter().copied(), tank_point.0, tank_point.1);
    }
    let scout_point = if stationary_range_ready && tanks_in_position {
        if tight_formation {
            scout_forward_from_tanks(
                tank_anchor,
                own_base,
                objective,
                observation.map,
                policy.scout_forward_tiles,
            )?
        } else {
            legacy_scout_point
        }
    } else {
        trailing_point
    };
    actions::move_units(
        actions,
        scouts.iter().copied(),
        scout_point.0,
        scout_point.1,
    );

    let riflemen: Vec<u32> = memory.containment_active_riflemen.iter().copied().collect();
    if !riflemen.is_empty() {
        let screen_points =
            rifle_screen_points(tank_anchor, objective, observation.map, riflemen.len());
        for (rifleman, screen_point) in riflemen.iter().zip(screen_points) {
            actions::attack_move_units(actions, [*rifleman], screen_point.0, screen_point.1);
        }
    }

    tanks.extend(scouts);
    tanks.extend(riflemen);
    tanks.sort_unstable();
    tanks.dedup();
    Some(AiIntent::Attack { units: tanks })
}
