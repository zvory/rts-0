use super::geometry::{clamp_to_map, dist2, normalized_direction, tile_center};
use super::*;

mod formation;
#[cfg(test)]
mod formation_tests;
mod legacy_beta;
pub(super) mod smoke;

use self::formation::*;
#[cfg(test)]
use self::legacy_beta::{compact_group_near, containment_regroup_radius_tiles};
use self::smoke::*;
use rts_rules::faction::AbilityKind;

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
const RIFLE_SCREEN_FORWARD_TILES: f32 = 2.0;
const RIFLE_SCREEN_SPACING_TILES: f32 = 2.0;
const RIFLE_SCREEN_SECOND_RANK_BACK_TILES: f32 = 1.5;
const RIFLE_SCREEN_FIRST_RANK: usize = 4;
const MAX_CONTAINMENT_RIFLE_ESCORTS: usize = 6;
const MIN_CONTAINMENT_RIFLE_ESCORTS: usize = 2;
const CONTAINMENT_HOME_RIFLE_RESERVE: usize = 4;
const CONTAINMENT_ESCORT_SELECTION_RADIUS_TILES: f32 = 12.0;
const CONTAINMENT_TANK_SPACING_TILES: f32 = 1.5;
const CONTAINMENT_ASSEMBLY_TOLERANCE_TILES: f32 = 1.75;
const CONTAINMENT_LONGITUDINAL_SPREAD_TILES: f32 = 2.0;
const CONTAINMENT_LATERAL_SLOP_TILES: f32 = 1.0;
const CONTAINMENT_RIFLE_COHESION_TILES: f32 = 7.0;
const CONTAINMENT_MARCH_STEP_TILES: f32 = 6.0;
const CONTAINMENT_FORMATION_REISSUE_TICKS: u32 = config::TICK_HZ * 2;
const CONTAINMENT_ASSEMBLY_TIMEOUT_TICKS: u32 = config::TICK_HZ * 8;
const CONTAINMENT_ASSEMBLY_HARD_TIMEOUT_TICKS: u32 = config::TICK_HZ * 12;
const RIVER_OPENING_GUARD_TICKS: u32 = config::TICK_HZ * 30;
const RIVER_OPENING_CLEAR_TICKS: u32 = config::TICK_HZ * 5;
const RIVER_OPENING_PRESSURE_RADIUS_TILES: f32 = 22.0;
const CONTAINMENT_WAYPOINT_TIMEOUT_TICKS: u32 = config::TICK_HZ * 4;
const CONTAINMENT_CONTACT_MEMORY_TICKS: u32 = config::TICK_HZ * 2;
const CONTAINMENT_FOCUS_STABLE_TICKS: u32 = 9;
const CONTAINMENT_SMOKE_RANGE_TILES: f32 = 13.5;
const CONTAINMENT_SMOKE_BASE_RADIUS_TILES: f32 = 2.0;
const CONTAINMENT_SMOKE_PLUS_RADIUS_TILES: f32 = 4.0;
const CONTAINMENT_SMOKE_SAFETY_TILES: f32 = 0.5;
const CONTAINMENT_SMOKE_DURATION_TICKS: u32 = config::TICK_HZ * 5;
const CONTAINMENT_SMOKE_AIM_INSET_TILES: f32 = 0.5;
const CONTAINMENT_LOCAL_SMOKE_TARGET_TILES: f32 = 22.0;
const CONTAINMENT_SCOUT_SMOKE_FORWARD_LIMIT_TILES: f32 = 3.5;
const CONTAINMENT_SCOUT_SMOKE_REAR_LIMIT_TILES: f32 = 2.0;
const CONTAINMENT_SCOUT_SMOKE_LATERAL_LIMIT_TILES: f32 = 4.5;
const MAX_LOCAL_DEFENSE_SMOKE_TANKS: usize = 2;
const TANK_VOLLEY_DAMAGE: u32 = 60;
const RIFLE_THREAT_LEASH_TILES: f32 = 4.0;
const RIFLE_THREAT_SECTOR_HALF_WIDTH_TILES: f32 = 3.0;

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

#[allow(clippy::too_many_arguments)]
pub(super) fn issue_frontal_wave(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    profile: &AiProfile,
    attack: AttackPolicy,
    plan: &FrontalWavePlan,
    enemy_base: EnemyBaseFact,
    map_analysis: Option<&AiMapAnalysis>,
    recall_target: Option<u32>,
    memory: &mut AiDecisionMemory,
) -> Option<AiIntent> {
    let cohesive_containment = uses_cohesive_containment(observation, profile.id);
    let containment_active = memory.containment_wave_launched
        || memory.containment_recovery_active
        || !memory.containment_active_tanks.is_empty();
    if let Some(containment) = profile.expansion_containment {
        if cohesive_containment {
            if let Some(target) = recall_target {
                if let Some(intent) = issue_containment_recall(actions, observation, memory, target)
                {
                    return Some(intent);
                }
            } else if memory.containment_recall_active {
                memory.containment_recall_active = false;
                memory.containment_last_formation_command_tick = None;
                reset_containment_route(memory);
            }
        }
        let relaxed_start = !containment_active
            && plan.attack_due
            && plan.required_unit_ready
            && plan.methamphetamines_ready
            && plan.ready_units.len() >= containment.minimum_tanks_to_continue + 3;
        if !cohesive_containment && plan.should_attack() {
            if let Some(intent) = legacy_beta::issue_expansion_containment_wave(
                actions,
                observation,
                plan,
                enemy_base,
                containment,
                true,
                memory,
            ) {
                return Some(intent);
            }
        } else if cohesive_containment
            && (plan.should_attack() || relaxed_start || containment_active)
        {
            if let Some(intent) = issue_expansion_containment_wave(
                actions,
                observation,
                plan,
                enemy_base,
                containment,
                is_jeffs_ai_profile(profile.id),
                map_analysis,
                memory,
            ) {
                return Some(intent);
            }
        }
    }

    if plan.should_attack() {
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

pub(super) fn uses_cohesive_containment(observation: &AiObservation, profile_id: &str) -> bool {
    if profile_id == JEFFS_AI_ID {
        return expansion::uses_jeff_river_layout(observation);
    }
    profile_id != JEFFS_AI_BETA_ID
}

pub(super) fn containment_wave_needs_control(memory: &AiDecisionMemory) -> bool {
    memory.containment_wave_launched
        || memory.containment_recovery_active
        || !memory.containment_active_tanks.is_empty()
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
        memory.containment_active_riflemen.clear();
        memory.containment_march_waypoint = None;
        memory.containment_route.clear();
        memory.containment_route_index = 0;
        memory.containment_route_objective = None;
        memory.containment_last_formation_command_tick = None;
        memory.containment_assembly_started_tick = None;
        memory.containment_waypoint_started_tick = None;
        memory.containment_recall_active = false;
        memory.containment_contact_last_tick = None;
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
    memory.containment_active_riflemen.clear();
    memory.containment_march_waypoint = None;
    memory.containment_route.clear();
    memory.containment_route_index = 0;
    memory.containment_route_objective = None;
    memory.containment_last_formation_command_tick = None;
    memory.containment_assembly_started_tick = None;
    memory.containment_waypoint_started_tick = None;
    memory.containment_stationary_since = None;
    memory.containment_contact_last_tick = None;
}

#[allow(clippy::too_many_arguments)]
fn issue_expansion_containment_wave(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    plan: &FrontalWavePlan,
    enemy_base: EnemyBaseFact,
    policy: ExpansionContainmentPolicy,
    tight_formation: bool,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
) -> Option<AiIntent> {
    let natural_objective = enemy_natural_edge(observation, enemy_base)?;
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let tile_size = observation.map.tile_size as f32;
    let owned: BTreeSet<u32> = observation.owned.iter().map(|unit| unit.id).collect();
    memory
        .containment_active_tanks
        .retain(|tank| owned.contains(tank));
    if memory
        .containment_active_scout
        .is_some_and(|scout| !owned.contains(&scout))
    {
        memory.containment_active_scout = None;
    }
    memory
        .containment_active_riflemen
        .retain(|rifleman| owned.contains(rifleman));

    let assembling = !memory.containment_wave_launched || memory.containment_recovery_active;
    if assembling {
        let required_tanks = if memory.containment_recovery_active {
            containment_repush_tank_count(policy, memory.containment_repush_count)
        } else {
            policy.minimum_tanks_to_continue
        };
        let rally = containment_regroup_point(own_base, enemy_base, observation.map)?;
        if memory.containment_active_tanks.len() != required_tanks
            || memory.containment_active_scout.is_none()
        {
            let tank_exclusions: BTreeSet<u32> = memory.home_defensive_tank.into_iter().collect();
            let mut tanks = actions::select_ready_combat_units_excluding(
                &observation.owned,
                &[EntityKind::Tank],
                &tank_exclusions,
            );
            let mut scouts =
                actions::select_ready_combat_units(&observation.owned, &[EntityKind::ScoutCar]);
            if !memory.containment_wave_launched {
                tanks.retain(|tank| plan.ready_units.contains(tank));
                scouts.retain(|scout| plan.ready_units.contains(scout));
            }
            select_nearest_units(observation, &mut tanks, rally, required_tanks);
            select_nearest_units(observation, &mut scouts, rally, 1);
            if tanks.len() != required_tanks || scouts.is_empty() {
                return None;
            }
            memory.containment_active_tanks = tanks.iter().copied().collect();
            memory.containment_active_scout = scouts.first().copied();
            memory.containment_active_riflemen = select_rifle_escorts(observation, memory, rally)
                .into_iter()
                .collect();
            reset_containment_route(memory);
            memory.containment_last_formation_command_tick = None;
            memory.containment_assembly_started_tick = Some(observation.tick);
        }

        let tanks: Vec<u32> = memory.containment_active_tanks.iter().copied().collect();
        let scout = memory.containment_active_scout?;
        let formation_center = group_center(observation, &tanks).unwrap_or(rally);
        let assembly_started = *memory
            .containment_assembly_started_tick
            .get_or_insert(observation.tick);
        let assembly_elapsed = observation.tick.saturating_sub(assembly_started);
        let assembly_timed_out = assembly_elapsed >= CONTAINMENT_ASSEMBLY_TIMEOUT_TICKS;
        let assembly_hard_timed_out = assembly_elapsed >= CONTAINMENT_ASSEMBLY_HARD_TIMEOUT_TICKS;
        if assembly_timed_out {
            memory.containment_active_riflemen =
                select_rifle_escorts(observation, memory, formation_center)
                    .into_iter()
                    .collect();
        }
        let riflemen: Vec<u32> = memory.containment_active_riflemen.iter().copied().collect();
        let formation = containment_formation(
            observation,
            &tanks,
            scout,
            &riflemen,
            formation_center,
            own_base,
            (enemy_base.x, enemy_base.y),
            policy,
        )?;
        let exact_assembly_ready = formation_units_in_position(
            observation,
            &formation,
            CONTAINMENT_ASSEMBLY_TOLERANCE_TILES,
        );
        let core_grouped = formation_core_is_grouped(
            observation,
            &formation,
            own_base,
            (enemy_base.x, enemy_base.y),
        );
        let vehicle_core_grouped = formation_vehicle_core_is_grouped(
            observation,
            &formation,
            own_base,
            (enemy_base.x, enemy_base.y),
        );
        let nearby_rifles = nearby_rifle_escort_count(observation, &riflemen, formation_center);
        let assembled = exact_assembly_ready
            || (assembly_timed_out
                && core_grouped
                && nearby_rifles >= MIN_CONTAINMENT_RIFLE_ESCORTS)
            || (assembly_hard_timed_out && vehicle_core_grouped);
        let river_opening_guard = !memory.containment_wave_launched
            && expansion::has_jeff_river_expansion_site(observation)
            && river_opening_guard_active(observation, &tanks, assembly_started, memory);
        let assembly_ready = assembled && !river_opening_guard;
        if !assembly_ready {
            if formation_command_due(memory, observation.tick) {
                issue_containment_formation(actions, observation, &formation, false);
                if river_opening_guard && assembled {
                    actions::hold_position_units(actions, tanks.iter().copied());
                }
                note_formation_command(memory, observation.tick);
            }
            return Some(AiIntent::Stage {
                units: formation.unit_ids(),
            });
        }

        if !memory.containment_wave_launched {
            memory.containment_opening_tanks = tanks.iter().copied().collect();
            memory.containment_wave_launched = true;
        }
        memory.containment_recovery_active = false;
        memory.containment_stationary_since = None;
        reset_containment_route(memory);
        memory.containment_last_formation_command_tick = None;
        memory.containment_assembly_started_tick = None;
    }

    let tanks: Vec<u32> = memory.containment_active_tanks.iter().copied().collect();
    let scouts = memory
        .containment_active_scout
        .into_iter()
        .collect::<Vec<_>>();
    let riflemen: Vec<u32> = memory.containment_active_riflemen.iter().copied().collect();
    if tanks.is_empty() || scouts.is_empty() {
        return None;
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
            CONTAINMENT_TANK_SPACING_TILES,
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
    if contact_target.is_some() {
        memory.containment_contact_last_tick = Some(observation.tick);
    }
    let contact_active = memory.containment_contact_last_tick.is_some_and(|last| {
        observation.tick.saturating_sub(last) <= CONTAINMENT_CONTACT_MEMORY_TICKS
    });
    let should_stop = tanks_in_position || contact_active;
    let stationary_range_ready = if should_stop {
        let since = memory
            .containment_stationary_since
            .get_or_insert(observation.tick);
        observation.tick.saturating_sub(*since) >= config::TICK_HZ * 3
    } else {
        memory.containment_stationary_since = None;
        false
    };

    let current_tank_target_outside_leash = stationary_range_ready
        && tanks.iter().any(|tank_id| {
            tanks_by_id
                .get(tank_id)
                .and_then(|tank| tank.target_id)
                .is_some_and(|target_id| {
                    !tank_can_fire_at_visible_target(
                        observation,
                        *tank_id,
                        target_id,
                        policy.tank_standoff_tiles,
                    )
                })
        });
    if current_tank_target_outside_leash {
        actions::hold_position_units(actions, tanks.iter().copied());
        memory.containment_focus_target = None;
        memory.containment_focus_stable_since = None;
    }

    if should_stop {
        let mut smoke_reposition = None;
        let mut smoke_issued = false;
        if tanks_in_position && !contact_active {
            reset_containment_route(memory);
        }
        if formation_command_due(memory, observation.tick) || current_tank_target_outside_leash {
            if stationary_range_ready {
                let locked_focus = active_smoke_focus(observation, memory);
                let current_targets = tanks
                    .iter()
                    .filter_map(|tank_id| tanks_by_id.get(tank_id).and_then(|tank| tank.target_id))
                    .collect::<BTreeSet<_>>();
                let consensus_target = (current_targets.len() == 1)
                    .then(|| current_targets.iter().next().copied())
                    .flatten();
                if current_targets.len() > 1 {
                    memory.containment_focus_stable_since = Some(observation.tick);
                }
                let target = shared_stationary_tank_target(
                    observation,
                    &tanks,
                    policy.tank_standoff_tiles,
                    locked_focus
                        .or(consensus_target)
                        .or(memory.containment_focus_target),
                    memory.containment_smoke_target,
                )
                .or_else(|| {
                    visible_strategic_building_target_within_tiles(
                        observation,
                        &tanks,
                        policy.tank_standoff_tiles,
                    )
                });
                if let Some(mut target) = target {
                    note_containment_focus(memory, observation.tick, target);
                    let target_is_unit = observation
                        .visible_enemies
                        .iter()
                        .find(|enemy| enemy.id == target)
                        .is_some_and(|enemy| enemy.kind.is_unit());
                    if target_is_unit {
                        let smoke_expiry_before = memory.containment_smoke_expires_tick;
                        smoke_reposition = maybe_issue_isolation_smoke(
                            actions,
                            observation,
                            &tanks,
                            scouts[0],
                            &mut target,
                            memory,
                            true,
                        );
                        smoke_issued = smoke_expiry_before.is_none()
                            && memory.containment_smoke_expires_tick.is_some();
                        issue_hp_aware_tank_volley(
                            actions,
                            observation,
                            &tanks,
                            target,
                            policy.tank_standoff_tiles,
                            memory.containment_smoke_target,
                        );
                    } else if target_is_in_shared_tank_range(
                        observation,
                        &tanks,
                        target,
                        policy.tank_standoff_tiles,
                    ) {
                        actions::attack_units(actions, tanks.iter().copied(), target);
                    } else {
                        actions::hold_position_units(actions, tanks.iter().copied());
                    }
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
                    memory.containment_focus_target = None;
                    memory.containment_focus_stable_since = None;
                }
            } else {
                actions::hold_position_units(actions, tanks.iter().copied());
            }

            let scout_point = if let Some(smoke_launch_point) = smoke_reposition {
                smoke_launch_point
            } else if stationary_range_ready && tanks_in_position {
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
            if !smoke_issued {
                actions::move_units(
                    actions,
                    scouts.iter().copied(),
                    scout_point.0,
                    scout_point.1,
                );
            }
            let screen_points =
                rifle_screen_points(tank_anchor, objective, observation.map, riflemen.len());
            for (rifleman, screen_point) in riflemen.iter().zip(screen_points) {
                if let Some(target) = rifle_sector_target(
                    observation,
                    *rifleman,
                    screen_point,
                    tank_anchor,
                    objective,
                ) {
                    actions::attack_units(actions, [*rifleman], target);
                } else {
                    actions::attack_move_units(
                        actions,
                        [*rifleman],
                        screen_point.0,
                        screen_point.1,
                    );
                }
            }
            note_formation_command(memory, observation.tick);
        }
    } else {
        let mut waypoint = stored_waypoint(memory);
        if let Some(current_waypoint) = waypoint {
            let formation = containment_formation(
                observation,
                &tanks,
                scouts[0],
                &riflemen,
                current_waypoint,
                own_base,
                objective,
                policy,
            )?;
            let waypoint_timed_out =
                memory
                    .containment_waypoint_started_tick
                    .is_some_and(|started| {
                        observation.tick.saturating_sub(started)
                            >= CONTAINMENT_WAYPOINT_TIMEOUT_TICKS
                    });
            if waypoint_timed_out {
                let tank_center = group_center(observation, &tanks).unwrap_or(current_waypoint);
                retain_nearby_rifle_escorts(
                    observation,
                    &mut memory.containment_active_riflemen,
                    tank_center,
                );
            }
            if formation_units_in_position(
                observation,
                &formation,
                CONTAINMENT_ASSEMBLY_TOLERANCE_TILES,
            ) || (waypoint_timed_out
                && formation_vehicle_core_is_grouped(observation, &formation, own_base, objective))
            {
                memory.containment_march_waypoint = None;
                memory.containment_last_formation_command_tick = None;
                memory.containment_waypoint_started_tick = None;
                waypoint = None;
            } else {
                if formation_command_due(memory, observation.tick) {
                    issue_containment_formation(actions, observation, &formation, true);
                    note_formation_command(memory, observation.tick);
                }
                return Some(AiIntent::Attack {
                    units: formation.unit_ids(),
                });
            }
        }

        if waypoint.is_none() {
            let tanks_are_cohesive = tank_group_is_cohesive(observation, &tanks, toward_objective);
            let current_center = if tanks_are_cohesive {
                group_center(observation, &tanks)?
            } else {
                rearmost_unit_position(observation, &tanks, toward_objective)?
            };
            let next = if tanks_are_cohesive {
                next_containment_route_waypoint(
                    memory,
                    map_analysis,
                    current_center,
                    tank_point,
                    observation.map,
                )
            } else {
                current_center
            };
            store_waypoint(memory, next, observation.tick);
            let formation = containment_formation(
                observation,
                &tanks,
                scouts[0],
                &riflemen,
                next,
                own_base,
                objective,
                policy,
            )?;
            issue_containment_formation(actions, observation, &formation, true);
            note_formation_command(memory, observation.tick);
            return Some(AiIntent::Attack {
                units: formation.unit_ids(),
            });
        }
    }

    let mut units = tanks;
    units.extend(scouts);
    units.extend(riflemen);
    units.sort_unstable();
    units.dedup();
    Some(AiIntent::Attack { units })
}

fn river_opening_guard_active(
    observation: &AiObservation,
    tanks: &[u32],
    assembly_started: u32,
    memory: &mut AiDecisionMemory,
) -> bool {
    let pressure_visible =
        visible_combat_target_within_tiles(observation, tanks, RIVER_OPENING_PRESSURE_RADIUS_TILES)
            .is_some();
    if pressure_visible {
        memory.containment_contact_last_tick = Some(observation.tick);
    }
    let pressure_recent = memory
        .containment_contact_last_tick
        .is_some_and(|last| observation.tick.saturating_sub(last) <= RIVER_OPENING_CLEAR_TICKS);
    observation.tick.saturating_sub(assembly_started) < RIVER_OPENING_GUARD_TICKS || pressure_recent
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
    own_base: (f32, f32),
    enemy_base: EnemyBaseFact,
    map: AiMapSummary,
    waypoint: usize,
) -> (f32, f32) {
    let offset = ENDGAME_SEARCH_OFFSETS[waypoint % ENDGAME_SEARCH_OFFSETS.len()];
    let orientation = canonical_half_turn_orientation(own_base, (enemy_base.x, enemy_base.y));
    let tile_size = map.tile_size as f32;
    clamp_to_map(
        (
            enemy_base.x + offset.0 * tile_size * orientation,
            enemy_base.y + offset.1 * tile_size * orientation,
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
    let approach_origin = (
        own_base.0 + perpendicular.0 * policy.flank_tiles * tile_size,
        own_base.1 + perpendicular.1 * policy.flank_tiles * tile_size,
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

/// Express half-turn-sensitive search offsets in Jeff's local own-base-to-enemy frame. A
/// rotationally mirrored start flips both axes, so the same local search pattern mirrors instead
/// of retaining a global top-left bias.
fn canonical_half_turn_orientation(from: (f32, f32), to: (f32, f32)) -> f32 {
    if from.0 < to.0 || (from.0 == to.0 && from.1 <= to.1) {
        1.0
    } else {
        -1.0
    }
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

fn tank_can_fire_at_visible_target(
    observation: &AiObservation,
    tank_id: u32,
    target_id: u32,
    range_tiles: f32,
) -> bool {
    let Some(tank) = observation.owned.iter().find(|unit| unit.id == tank_id) else {
        return false;
    };
    let Some(target) = observation
        .visible_enemies
        .iter()
        .find(|enemy| enemy.id == target_id)
    else {
        return false;
    };
    dist2(tank.x, tank.y, target.x, target.y)
        <= (range_tiles * observation.map.tile_size as f32).powi(2)
}

fn target_is_in_shared_tank_range(
    observation: &AiObservation,
    tanks: &[u32],
    target_id: u32,
    range_tiles: f32,
) -> bool {
    !tanks.is_empty()
        && tanks
            .iter()
            .all(|tank| tank_can_fire_at_visible_target(observation, *tank, target_id, range_tiles))
}

fn shared_stationary_tank_targets<'a>(
    observation: &'a AiObservation,
    tanks: &[u32],
    range_tiles: f32,
    excluded_target: Option<u32>,
) -> Vec<&'a AiEntitySummary> {
    let center = group_center(observation, tanks).unwrap_or((0.0, 0.0));
    let mut targets = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| Some(enemy.id) != excluded_target)
        .filter(|enemy| target_is_in_shared_tank_range(observation, tanks, enemy.id, range_tiles))
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        stationary_tank_target_priority(left.kind)
            .cmp(&stationary_tank_target_priority(right.kind))
            .then_with(|| {
                (left.hp > TANK_VOLLEY_DAMAGE * tanks.len() as u32)
                    .cmp(&(right.hp > TANK_VOLLEY_DAMAGE * tanks.len() as u32))
            })
            .then_with(|| left.hp.cmp(&right.hp))
            .then_with(|| {
                dist2(center.0, center.1, left.x, left.y)
                    .total_cmp(&dist2(center.0, center.1, right.x, right.y))
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    targets
}

fn shared_stationary_tank_target(
    observation: &AiObservation,
    tanks: &[u32],
    range_tiles: f32,
    preferred_target: Option<u32>,
    excluded_target: Option<u32>,
) -> Option<u32> {
    let targets = shared_stationary_tank_targets(observation, tanks, range_tiles, excluded_target);
    preferred_target
        .filter(|preferred| targets.iter().any(|target| target.id == *preferred))
        .or_else(|| targets.first().map(|target| target.id))
}

fn stationary_tank_target_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::AntiTankGun => 0,
        EntityKind::Tank => 1,
        EntityKind::Panzerfaust => 2,
        EntityKind::Artillery | EntityKind::MortarTeam => 3,
        EntityKind::MachineGunner => 4,
        EntityKind::ScoutCar | EntityKind::Rifleman => 5,
        _ => 6,
    }
}

fn note_containment_focus(memory: &mut AiDecisionMemory, tick: u32, target: u32) {
    if memory.containment_focus_target == Some(target) {
        return;
    }
    memory.containment_focus_target = Some(target);
    memory.containment_focus_stable_since = Some(tick);
}

fn rifle_sector_target(
    observation: &AiObservation,
    rifleman: u32,
    screen_point: (f32, f32),
    tank_anchor: (f32, f32),
    objective: (f32, f32),
) -> Option<u32> {
    let rifle = observation.owned.iter().find(|unit| unit.id == rifleman)?;
    let direction = normalized_direction(tank_anchor, objective)?;
    let perpendicular = (-direction.1, direction.0);
    let tile_size = observation.map.tile_size as f32;
    if dist2(rifle.x, rifle.y, screen_point.0, screen_point.1)
        > (RIFLE_THREAT_LEASH_TILES * tile_size).powi(2)
    {
        return None;
    }
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            matches!(
                enemy.kind,
                EntityKind::Panzerfaust
                    | EntityKind::MachineGunner
                    | EntityKind::Rifleman
                    | EntityKind::ScoutCar
            )
        })
        .filter_map(|enemy| {
            let delta = (enemy.x - screen_point.0, enemy.y - screen_point.1);
            let lateral = (delta.0 * perpendicular.0 + delta.1 * perpendicular.1).abs();
            let longitudinal = delta.0 * direction.0 + delta.1 * direction.1;
            (lateral <= RIFLE_THREAT_SECTOR_HALF_WIDTH_TILES * tile_size
                && longitudinal >= -tile_size
                && longitudinal <= RIFLE_THREAT_LEASH_TILES * tile_size)
                .then_some((
                    enemy.id,
                    match enemy.kind {
                        EntityKind::Panzerfaust => 0,
                        EntityKind::MachineGunner => 1,
                        EntityKind::Rifleman => 2,
                        _ => 3,
                    },
                    dist2(rifle.x, rifle.y, enemy.x, enemy.y),
                ))
        })
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|target| target.0)
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
mod tests;
