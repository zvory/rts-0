use super::geometry::{clamp_to_map, dist2, normalized_direction, tile_center};
use super::*;

const ENDGAME_SEARCH_OFFSETS: [(f32, f32); 17] = [
    (0.0, 0.0),
    (8.0, 0.0),
    (8.0, 8.0),
    (0.0, 8.0),
    (-8.0, 8.0),
    (-8.0, 0.0),
    (-8.0, -8.0),
    (0.0, -8.0),
    (8.0, -8.0),
    (16.0, 0.0),
    (16.0, 16.0),
    (0.0, 16.0),
    (-16.0, 16.0),
    (-16.0, 0.0),
    (-16.0, -16.0),
    (0.0, -16.0),
    (16.0, -16.0),
];

pub(super) const OUTBOUND_WAVE_VISIBLE_TARGET_RADIUS_TILES: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FrontalWaveBlocker {
    WaitingForUnits,
    WaitingForTank,
    WaitingForMethamphetamines,
    Staging,
    AttackCadence,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FrontalWavePlan {
    pub(super) ready_units: Vec<u32>,
    pub(super) desired_size: usize,
    pub(super) attack_due: bool,
    pub(super) required_unit_ready: bool,
    pub(super) methamphetamines_ready: bool,
    pub(super) blockers: Vec<FrontalWaveBlocker>,
}

impl FrontalWavePlan {
    pub(super) fn should_attack(&self) -> bool {
        self.blockers.is_empty()
    }

    pub(super) fn should_stage(&self) -> bool {
        !self.ready_units.is_empty() && !self.should_attack()
    }
}

pub(super) fn plan_frontal_wave(
    observation: &AiObservation,
    attack: AttackPolicy,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    excluded_units: &BTreeSet<u32>,
) -> FrontalWavePlan {
    let owned_units: BTreeSet<u32> = observation.owned.iter().map(|entity| entity.id).collect();
    let launched_units =
        memory.launched_frontal_unit_exclusions(profile, observation.tick, &owned_units);
    let mut excluded_units = excluded_units.clone();
    excluded_units.extend(launched_units);
    let ready_units = actions::select_ready_combat_units_excluding(
        &observation.owned,
        attack.unit_kinds,
        &excluded_units,
    );
    let desired_size = memory.desired_attack_size_for(profile, attack, observation.tick);
    let attack_due = memory.attack_due_for(profile, attack, observation.tick);
    let required_unit_ready = attack
        .required_unit
        .map(|kind| {
            observation
                .owned
                .iter()
                .any(|entity| entity.kind == kind && ready_units.contains(&entity.id))
        })
        .unwrap_or(true);
    let methamphetamines_ready = profile.fast_tank_timing.is_some()
        || !attack.unit_kinds.contains(&EntityKind::Tank)
        || observation
            .upgrades
            .contains(&UpgradeKind::Methamphetamines);

    let mut blockers = Vec::new();
    if ready_units.len() < desired_size {
        blockers.push(FrontalWaveBlocker::WaitingForUnits);
    }
    if !required_unit_ready && attack.required_unit == Some(EntityKind::Tank) {
        blockers.push(FrontalWaveBlocker::WaitingForTank);
    } else if !required_unit_ready {
        blockers.push(FrontalWaveBlocker::WaitingForUnits);
    }
    if !methamphetamines_ready {
        blockers.push(FrontalWaveBlocker::WaitingForMethamphetamines);
    }
    if !attack_due {
        blockers.push(FrontalWaveBlocker::AttackCadence);
    }
    blockers.sort();
    blockers.dedup();

    FrontalWavePlan {
        ready_units,
        desired_size,
        attack_due,
        required_unit_ready,
        methamphetamines_ready,
        blockers,
    }
}

pub(super) fn issue_frontal_wave(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    profile: &AiProfile,
    attack: AttackPolicy,
    plan: &FrontalWavePlan,
    enemy_base: EnemyBaseFact,
    memory: &mut AiDecisionMemory,
) -> Option<AiIntent> {
    if plan.should_attack() {
        if let Some(containment) = profile.expansion_containment {
            if let Some(intent) = issue_expansion_containment_wave(
                actions,
                observation,
                plan,
                enemy_base,
                containment,
                profile.id == JEFFS_AI_ID,
                memory,
            ) {
                return Some(intent);
            }
        }
        let attack_units =
            if let Some(target) = visible_combat_target_for_wave(observation, &plan.ready_units) {
                actions::attack_units(actions, plan.ready_units.clone(), target)
            } else {
                actions::attack_move_units(
                    actions,
                    plan.ready_units.clone(),
                    enemy_base.x,
                    enemy_base.y,
                )
            };
        return attack_units.map(|units| AiIntent::Attack { units });
    }

    if !plan.should_stage() {
        return None;
    }

    let staged = if profile.frontal_wave.line_staging {
        stage_main_steel_defensive_line(
            actions,
            observation,
            &plan.ready_units,
            enemy_base,
            attack.stage_distance_tiles,
        )
    } else {
        let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
        actions::stage_units_toward(
            actions,
            plan.ready_units.clone(),
            own_base,
            (enemy_base.x, enemy_base.y),
            observation.map.tile_size,
            attack.stage_distance_tiles,
        )
    };
    staged.map(|units| AiIntent::Stage { units })
}

pub(super) fn sync_containment_recovery(
    observation: &AiObservation,
    profile: &AiProfile,
    memory: &mut AiDecisionMemory,
) {
    let Some(_) = profile.expansion_containment else {
        memory.containment_recovery_active = false;
        memory.containment_active_tanks.clear();
        memory.containment_active_scout = None;
        return;
    };
    if !memory.containment_wave_launched || memory.enemy_main_destroyed {
        return;
    }
    if memory.containment_recovery_active || memory.containment_active_tanks.is_empty() {
        return;
    }
    let owned: BTreeSet<u32> = observation.owned.iter().map(|entity| entity.id).collect();
    let tanks_intact = memory
        .containment_active_tanks
        .iter()
        .all(|tank| owned.contains(tank));
    let scout_intact = memory
        .containment_active_scout
        .is_some_and(|scout| owned.contains(&scout));
    if tanks_intact && scout_intact {
        return;
    }
    memory.containment_repush_count = memory.containment_repush_count.saturating_add(1);
    memory.containment_recovery_active = true;
    memory.containment_active_tanks.clear();
    memory.containment_active_scout = None;
    memory.containment_stationary_since = None;
}

fn containment_repush_tank_count(policy: ExpansionContainmentPolicy, repush_count: usize) -> usize {
    policy.recovery_tanks_to_continue.saturating_add(
        repush_count
            .saturating_sub(1)
            .saturating_mul(policy.additional_tanks_per_repush),
    )
}

fn containment_regroup_radius_tiles(
    policy: ExpansionContainmentPolicy,
    required_tanks: usize,
) -> f32 {
    policy.repush_regroup_radius_tiles
        + required_tanks.saturating_sub(policy.recovery_tanks_to_continue) as f32 * 1.5
}

fn containment_regroup_point(
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

fn select_nearest_units(
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

fn unit_position(observation: &AiObservation, unit_id: u32) -> Option<(f32, f32)> {
    observation
        .owned
        .iter()
        .find(|unit| unit.id == unit_id)
        .map(|unit| (unit.x, unit.y))
}

fn compact_group_near(
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

fn issue_expansion_containment_wave(
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
    if !memory.containment_wave_launched {
        if tanks.len() < policy.minimum_tanks_to_continue {
            return None;
        }
        tanks.truncate(policy.minimum_tanks_to_continue);
        scouts.truncate(1);
        memory.containment_opening_tanks = tanks.iter().copied().collect();
        memory.containment_active_tanks = tanks.iter().copied().collect();
        memory.containment_active_scout = scouts.first().copied();
        memory.containment_wave_launched = true;
    } else if memory.containment_recovery_active {
        let required = containment_repush_tank_count(policy, memory.containment_repush_count);
        let forward_rally = containment_regroup_point(own_base, enemy_base, observation.map)?;
        select_nearest_units(observation, &mut tanks, forward_rally, required);
        select_nearest_units(observation, &mut scouts, forward_rally, 1);
        let regroup_point = tanks
            .first()
            .and_then(|tank| unit_position(observation, *tank))?;
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
        endgame_search_point(enemy_base, observation.map, memory.endgame_search_waypoint)
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
    } else {
        // Attack-move handles interceptors without converting them into a chase
        // target, so the formation continues toward the containment anchor.
        if tight_formation {
            for (tank_id, point) in &tank_assignments {
                actions::attack_move_units(actions, [*tank_id], point.0, point.1);
            }
        } else {
            actions::attack_move_units(actions, tanks.iter().copied(), tank_point.0, tank_point.1);
        }
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

    tanks.extend(scouts);
    tanks.sort_unstable();
    tanks.dedup();
    Some(AiIntent::Attack { units: tanks })
}

fn update_enemy_main_state(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
    tanks: &[u32],
    scouts: &[u32],
    memory: &mut AiDecisionMemory,
) {
    if memory.enemy_main_destroyed {
        return;
    }
    let tile_size = observation.map.tile_size as f32;
    let main_radius2 = (8.0 * tile_size).powi(2);
    let visible_main = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::ResourceDepot)
        .filter(|enemy| dist2(enemy.x, enemy.y, enemy_base.x, enemy_base.y) <= main_radius2)
        .min_by_key(|enemy| enemy.id);
    if let Some(resource_depot) = visible_main {
        memory.enemy_main_resource_depot = Some(resource_depot.id);
        return;
    }
    if memory.enemy_main_resource_depot.is_none() {
        return;
    }
    let confirmation_radius2 = (14.0 * tile_size).powi(2);
    let force_confirms_site = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id) || scouts.contains(&unit.id))
        .any(|unit| dist2(unit.x, unit.y, enemy_base.x, enemy_base.y) <= confirmation_radius2);
    if force_confirms_site {
        memory.enemy_main_destroyed = true;
        memory.endgame_search_waypoint = 0;
        memory.containment_stationary_since = None;
    }
}

fn endgame_search_point(
    enemy_base: EnemyBaseFact,
    map: AiMapSummary,
    waypoint: usize,
) -> (f32, f32) {
    let offset = ENDGAME_SEARCH_OFFSETS[waypoint % ENDGAME_SEARCH_OFFSETS.len()];
    let tile_size = map.tile_size as f32;
    clamp_to_map(
        (
            enemy_base.x + offset.0 * tile_size,
            enemy_base.y + offset.1 * tile_size,
        ),
        map,
    )
}

fn update_enemy_natural_state(
    observation: &AiObservation,
    natural: (f32, f32),
    enemy_base: EnemyBaseFact,
    scouts: &[u32],
    memory: &mut AiDecisionMemory,
) {
    if memory.enemy_natural_destroyed {
        return;
    }
    let tile_size = observation.map.tile_size as f32;
    let natural_radius2 = (8.0 * tile_size) * (8.0 * tile_size);
    let main_exclusion2 = (config::START_RESOURCE_MAX_DIST_TILES * tile_size)
        * (config::START_RESOURCE_MAX_DIST_TILES * tile_size);
    let visible_natural = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::ResourceDepot)
        .filter(|enemy| dist2(enemy.x, enemy.y, enemy_base.x, enemy_base.y) > main_exclusion2)
        .filter(|enemy| dist2(enemy.x, enemy.y, natural.0, natural.1) <= natural_radius2)
        .min_by_key(|enemy| enemy.id);
    if let Some(resource_depot) = visible_natural {
        memory.enemy_natural_resource_depot = Some(resource_depot.id);
        return;
    }
    // At the containment anchor the Tanks sit 13.5 tiles from the resource
    // edge and the Scout moves two tiles ahead. Allow one tile of formation
    // separation so that the intended 11.5-tile observation point can
    // confirm that a destroyed (or absent) natural is clear.
    let scout_confirmation_tiles = 12.5;
    let scout_confirms_site = observation
        .owned
        .iter()
        .filter(|unit| scouts.contains(&unit.id))
        .any(|unit| {
            dist2(unit.x, unit.y, natural.0, natural.1)
                <= (scout_confirmation_tiles * tile_size).powi(2)
        });
    if scout_confirms_site {
        memory.enemy_natural_destroyed = true;
        memory.containment_stationary_since = None;
    }
}

fn containment_points(
    own_base: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    policy: ExpansionContainmentPolicy,
) -> Option<((f32, f32), (f32, f32))> {
    let toward_expansion = normalized_direction(own_base, objective)?;
    let tile_size = map.tile_size as f32;
    let perpendicular = (-toward_expansion.1, toward_expansion.0);
    let flank_sign = if own_base.0 + own_base.1 <= objective.0 + objective.1 {
        1.0
    } else {
        -1.0
    };
    let approach_origin = (
        own_base.0 + perpendicular.0 * policy.flank_tiles * tile_size * flank_sign,
        own_base.1 + perpendicular.1 * policy.flank_tiles * tile_size * flank_sign,
    );
    let toward_expansion = normalized_direction(approach_origin, objective)?;
    let tank_point = clamp_to_map(
        (
            objective.0 - toward_expansion.0 * policy.tank_standoff_tiles * tile_size,
            objective.1 - toward_expansion.1 * policy.tank_standoff_tiles * tile_size,
        ),
        map,
    );
    let scout_point = clamp_to_map(
        (
            tank_point.0 + toward_expansion.0 * policy.scout_forward_tiles * tile_size,
            tank_point.1 + toward_expansion.1 * policy.scout_forward_tiles * tile_size,
        ),
        map,
    );
    Some((tank_point, scout_point))
}

fn scout_trailing_point(
    tank_center: (f32, f32),
    own_base: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    trailing_tiles: f32,
) -> Option<(f32, f32)> {
    let toward_expansion = normalized_direction(own_base, objective)?;
    let tile_size = map.tile_size as f32;
    Some(clamp_to_map(
        (
            tank_center.0 - toward_expansion.0 * trailing_tiles * tile_size,
            tank_center.1 - toward_expansion.1 * trailing_tiles * tile_size,
        ),
        map,
    ))
}

fn scout_forward_from_tanks(
    tank_center: (f32, f32),
    own_base: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    forward_tiles: f32,
) -> Option<(f32, f32)> {
    let toward_expansion = normalized_direction(own_base, objective)?;
    let tile_size = map.tile_size as f32;
    Some(clamp_to_map(
        (
            tank_center.0 + toward_expansion.0 * forward_tiles * tile_size,
            tank_center.1 + toward_expansion.1 * forward_tiles * tile_size,
        ),
        map,
    ))
}

fn compact_tank_formation_assignments(
    observation: &AiObservation,
    tank_ids: &[u32],
    center: (f32, f32),
    toward_objective: (f32, f32),
    map: AiMapSummary,
    spacing_tiles: f32,
) -> Vec<(u32, (f32, f32))> {
    let mut tank_ids = tank_ids.to_vec();
    let perpendicular = (-toward_objective.1, toward_objective.0);
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    tank_ids.sort_by(|left, right| {
        let lateral_position = |id: &u32| {
            by_id
                .get(id)
                .map(|unit| unit.x * perpendicular.0 + unit.y * perpendicular.1)
                .unwrap_or(0.0)
        };
        lateral_position(left)
            .total_cmp(&lateral_position(right))
            .then_with(|| left.cmp(right))
    });
    let tile_size = map.tile_size as f32;
    let middle = tank_ids.len().saturating_sub(1) as f32 / 2.0;
    tank_ids
        .into_iter()
        .enumerate()
        .map(|(index, tank_id)| {
            let offset = (index as f32 - middle) * spacing_tiles * tile_size;
            (
                tank_id,
                clamp_to_map(
                    (
                        center.0 + perpendicular.0 * offset,
                        center.1 + perpendicular.1 * offset,
                    ),
                    map,
                ),
            )
        })
        .collect()
}

fn frontmost_unit_position(
    observation: &AiObservation,
    unit_ids: &[u32],
    toward_objective: (f32, f32),
) -> Option<(f32, f32)> {
    observation
        .owned
        .iter()
        .filter(|unit| unit_ids.contains(&unit.id))
        .max_by(|left, right| {
            let left_progress = left.x * toward_objective.0 + left.y * toward_objective.1;
            let right_progress = right.x * toward_objective.0 + right.y * toward_objective.1;
            left_progress
                .total_cmp(&right_progress)
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|unit| (unit.x, unit.y))
}

fn enemy_natural_edge(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
) -> Option<(f32, f32)> {
    let tile_size = observation.map.tile_size as f32;
    let start_exclusion = (config::START_RESOURCE_MAX_DIST_TILES + 1.5) * tile_size;
    let start_exclusion2 = start_exclusion * start_exclusion;
    observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
        .filter(|resource| {
            dist2(resource.x, resource.y, enemy_base.x, enemy_base.y) > start_exclusion2
        })
        .min_by(|left, right| {
            dist2(left.x, left.y, enemy_base.x, enemy_base.y)
                .total_cmp(&dist2(right.x, right.y, enemy_base.x, enemy_base.y))
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|resource| (resource.x, resource.y))
}

pub(super) fn visible_combat_target_for_wave(
    observation: &AiObservation,
    unit_ids: &[u32],
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = OUTBOUND_WAVE_VISIBLE_TARGET_RADIUS_TILES * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .map(|enemy| {
            let distance2 = geometry::dist2(center.0, center.1, enemy.x, enemy.y);
            (
                enemy.id,
                outbound_wave_target_priority(enemy.kind),
                distance2,
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn visible_combat_target_within_tiles(
    observation: &AiObservation,
    unit_ids: &[u32],
    radius_tiles: f32,
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = radius_tiles * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .map(|enemy| {
            (
                enemy.id,
                outbound_wave_target_priority(enemy.kind),
                geometry::dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn visible_anti_armor_target_within_tiles(
    observation: &AiObservation,
    unit_ids: &[u32],
    radius_tiles: f32,
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = radius_tiles * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            matches!(
                enemy.kind,
                EntityKind::Tank | EntityKind::AntiTankGun | EntityKind::Panzerfaust
            )
        })
        .map(|enemy| {
            (
                enemy.id,
                outbound_wave_target_priority(enemy.kind),
                geometry::dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn visible_strategic_building_target_within_tiles(
    observation: &AiObservation,
    unit_ids: &[u32],
    radius_tiles: f32,
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = radius_tiles * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_building())
        .map(|enemy| {
            let priority = match enemy.kind {
                EntityKind::ResourceDepot => 0,
                EntityKind::Factory | EntityKind::Steelworks => 1,
                EntityKind::EngineeringComplex | EntityKind::TrainingCentre => 2,
                _ => 3,
            };
            (
                enemy.id,
                priority,
                geometry::dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn outbound_wave_target_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Tank | EntityKind::AntiTankGun | EntityKind::Panzerfaust => 0,
        EntityKind::Artillery | EntityKind::MortarTeam => 1,
        EntityKind::MachineGunner | EntityKind::Rifleman | EntityKind::ScoutCar => 2,
        _ => 3,
    }
}

fn group_center(observation: &AiObservation, unit_ids: &[u32]) -> Option<(f32, f32)> {
    let (sum_x, sum_y, count) = observation
        .owned
        .iter()
        .filter(|entity| unit_ids.contains(&entity.id))
        .fold((0.0, 0.0, 0usize), |(sum_x, sum_y, count), entity| {
            (sum_x + entity.x, sum_y + entity.y, count + 1)
        });
    (count > 0).then_some((sum_x / count as f32, sum_y / count as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::observation::AiEconomy;
    use crate::ai_core::profiles::JEFFS_AI;

    fn target_test_entity(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: if id == 1 { 1 } else { 2 },
            kind,
            x,
            y,
            hp: 300,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: true,
        }
    }

    fn regroup_test_observation(owned: Vec<AiEntitySummary>) -> AiObservation {
        AiObservation {
            player_id: 1,
            tick: 0,
            map: AiMapSummary {
                width: 64,
                height: 64,
                tile_size: 32,
            },
            economy: AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: owned.len() as u32,
                supply_cap: 100,
            },
            own_start_tile: (10, 10),
            players: Vec::new(),
            owned,
            resources: Vec::new(),
            visible_allies: Vec::new(),
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        }
    }

    #[test]
    fn containment_uses_stationary_tank_range_and_forward_scout_vision() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };
        let policy = ExpansionContainmentPolicy {
            tank_standoff_tiles: 13.5,
            scout_trailing_tiles: 1.5,
            scout_forward_tiles: 2.0,
            flank_tiles: 5.0,
            contact_stop_tiles: 18.0,
            minimum_tanks_to_continue: 2,
            recovery_tanks_to_continue: 3,
            additional_tanks_per_repush: 1,
            repush_regroup_radius_tiles: 5.0,
        };
        let objective = (2_000.0, 1_000.0);
        let (tank, scout) = containment_points((200.0, 1_000.0), objective, map, policy).unwrap();

        let tank_distance = dist2(objective.0, objective.1, tank.0, tank.1).sqrt() / 32.0;
        let scout_distance = dist2(scout.0, scout.1, tank.0, tank.1).sqrt() / 32.0;
        assert!((tank_distance - 13.5).abs() < 0.001);
        assert!((scout_distance - 2.0).abs() < 0.001);
        assert!(scout.0 < objective.0);

        let trailing =
            scout_trailing_point((1_000.0, 1_000.0), (200.0, 1_000.0), objective, map, 1.5)
                .unwrap();
        assert_eq!((1_000.0 - trailing.0) / 32.0, 1.5);
    }

    #[test]
    fn each_repush_adds_one_tank_to_the_grouped_cohort() {
        let policy = JEFFS_AI.expansion_containment.unwrap();
        assert_eq!(containment_repush_tank_count(policy, 1), 3);
        assert_eq!(containment_repush_tank_count(policy, 2), 4);
        assert_eq!(containment_repush_tank_count(policy, 3), 5);
        assert_eq!(containment_regroup_radius_tiles(policy, 3), 3.0);
        assert_eq!(containment_regroup_radius_tiles(policy, 4), 4.5);
        assert_eq!(containment_regroup_radius_tiles(policy, 5), 6.0);
    }

    #[test]
    fn repush_selects_units_nearest_the_forward_rally_point() {
        let observation = regroup_test_observation(vec![
            target_test_entity(1, EntityKind::Tank, 100.0, 100.0),
            target_test_entity(2, EntityKind::Tank, 500.0, 500.0),
            target_test_entity(3, EntityKind::Tank, 515.0, 500.0),
            target_test_entity(4, EntityKind::Tank, 530.0, 500.0),
        ]);
        let mut candidates = vec![1, 2, 3, 4];

        select_nearest_units(&observation, &mut candidates, (520.0, 500.0), 3);

        assert_eq!(candidates, vec![3, 4, 2]);
    }

    #[test]
    fn repush_requires_a_compact_group_near_its_rally_point() {
        let compact = regroup_test_observation(vec![
            target_test_entity(1, EntityKind::Tank, 490.0, 500.0),
            target_test_entity(2, EntityKind::Tank, 510.0, 500.0),
            target_test_entity(3, EntityKind::Tank, 500.0, 510.0),
            target_test_entity(4, EntityKind::ScoutCar, 500.0, 490.0),
        ]);
        let scattered = regroup_test_observation(vec![
            target_test_entity(1, EntityKind::Tank, 300.0, 500.0),
            target_test_entity(2, EntityKind::Tank, 700.0, 500.0),
            target_test_entity(3, EntityKind::Tank, 500.0, 300.0),
            target_test_entity(4, EntityKind::ScoutCar, 500.0, 700.0),
        ]);
        let cohort = [1, 2, 3, 4];

        assert!(compact_group_near(
            &compact,
            &cohort,
            (500.0, 500.0),
            5.0 * 32.0
        ));
        assert!(!compact_group_near(
            &scattered,
            &cohort,
            (500.0, 500.0),
            5.0 * 32.0
        ));
    }

    #[test]
    fn anti_armor_threats_outrank_every_economic_target() {
        assert_eq!(outbound_wave_target_priority(EntityKind::Tank), 0);
        assert_eq!(outbound_wave_target_priority(EntityKind::AntiTankGun), 0);
        assert_eq!(outbound_wave_target_priority(EntityKind::Panzerfaust), 0);
        assert!(
            outbound_wave_target_priority(EntityKind::MachineGunner)
                > outbound_wave_target_priority(EntityKind::Tank)
        );
        assert!(
            outbound_wave_target_priority(EntityKind::Worker)
                > outbound_wave_target_priority(EntityKind::Panzerfaust)
        );
    }

    #[test]
    fn main_resource_depot_is_acquired_outside_nominal_standoff_radius() {
        let tile_size = 32;
        let tank = target_test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0);
        let resource_depot =
            target_test_entity(2, EntityKind::ResourceDepot, 25.0 * 32.0, 10.0 * 32.0);
        let observation = AiObservation {
            player_id: 1,
            tick: 0,
            map: AiMapSummary {
                width: 64,
                height: 64,
                tile_size,
            },
            economy: AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 1,
                supply_cap: 100,
            },
            own_start_tile: (10, 10),
            players: Vec::new(),
            owned: vec![tank],
            resources: Vec::new(),
            visible_allies: Vec::new(),
            visible_enemies: vec![resource_depot],
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        };

        assert_eq!(
            visible_strategic_building_target_within_tiles(&observation, &[1], 13.5),
            None
        );
        assert_eq!(
            visible_strategic_building_target_within_tiles(&observation, &[1], 18.0),
            Some(2)
        );
    }

    #[test]
    fn endgame_search_visits_inner_and_outer_base_rings() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };
        let enemy_base = EnemyBaseFact {
            player_id: 2,
            start_tile: (50, 50),
            x: 50.5 * 32.0,
            y: 50.5 * 32.0,
        };
        assert_eq!(endgame_search_point(enemy_base, map, 0), (1616.0, 1616.0));
        assert_eq!(endgame_search_point(enemy_base, map, 1), (1872.0, 1616.0));
        assert_eq!(endgame_search_point(enemy_base, map, 9), (2128.0, 1616.0));
        assert_eq!(
            endgame_search_point(enemy_base, map, ENDGAME_SEARCH_OFFSETS.len()),
            endgame_search_point(enemy_base, map, 0)
        );
    }
}
