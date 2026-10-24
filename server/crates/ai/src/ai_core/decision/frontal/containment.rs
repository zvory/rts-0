//! The current push controller: forming the push at the regroup point, launching it, marching it
//! to the enemy natural and holding there, and falling back.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn issue_expansion_containment_wave(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    plan: &FrontalWavePlan,
    enemy_base: EnemyBaseFact,
    policy: ExpansionContainmentPolicy,
    tight_formation: bool,
    lead_anchor_tank_catchup: bool,
    push_uses_available_armor: bool,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
) -> Option<AiIntent> {
    let natural_objective = enemy_natural_edge(observation, enemy_base)?;
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let tile_size = observation.map.tile_size as f32;
    let owned: BTreeSet<u32> = observation.owned.iter().map(|unit| unit.id).collect();
    memory
        .containment
        .active_tanks
        .retain(|tank| owned.contains(tank));
    if memory
        .containment
        .active_scout
        .is_some_and(|scout| !owned.contains(&scout))
    {
        memory.containment.active_scout = None;
    }
    memory
        .containment
        .active_riflemen
        .retain(|rifleman| owned.contains(rifleman));
    // A Tank seen moving or attacking was given another order since the push held it.
    let still_standing: BTreeSet<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.state == AiEntityState::Idle)
        .map(|unit| unit.id)
        .collect();
    let active_tanks = memory.containment.active_tanks.clone();
    memory
        .containment
        .held_tanks
        .retain(|tank| active_tanks.contains(tank) && still_standing.contains(tank));

    let assembling = !memory.containment.wave_launched || memory.containment.recovery_active;
    // On Crossroads the push walks out along the enemy's own road; small pushes died there to
    // AI 2.1's larger Tank groups, while Tanks held inside the exit traded 1 for 5.
    let crossroads = defense::crossroads_wall_aware_approach_direction(observation).is_some();
    if assembling {
        let required_tanks = if memory.containment.recovery_active {
            containment_repush_tank_count(policy, memory.containment.repush_count)
        } else {
            policy.minimum_tanks_to_continue
        };
        // The Crossroads push also grows to keep its lead over the enemy Tanks seen recently;
        // with a fixed size it could never show that lead and stayed home at 21 Tanks against 5.
        let required_tanks = if crossroads {
            required_tanks
                .max(CROSSROADS_PUSH_MIN_TANKS)
                .max(memory.recent_enemy_tanks() + CROSSROADS_PUSH_TANK_LEAD)
        } else {
            required_tanks
        };
        let rally = containment_regroup_point(own_base, enemy_base, observation.map)?;
        // The current Jeff forms its push once from most of the Tanks ready now, and only forms it
        // again if losses before the launch take it below its minimum. Other profiles keep a push
        // of exactly the required size.
        let reform = if push_uses_available_armor {
            memory.containment.active_tanks.len() < required_tanks
        } else {
            memory.containment.active_tanks.len() != required_tanks
        };
        if reform || memory.containment.active_scout.is_none() {
            let tank_exclusions: BTreeSet<u32> = memory
                .home_defensive_tank
                .into_iter()
                .chain(memory.later_bases.guards.iter().copied())
                .collect();
            let mut tanks = actions::select_ready_combat_units_excluding(
                &observation.owned,
                &[EntityKind::Tank],
                &tank_exclusions,
            );
            let mut scouts =
                actions::select_ready_combat_units(&observation.owned, &[EntityKind::ScoutCar]);
            if !memory.containment.wave_launched {
                tanks.retain(|tank| plan.ready_units.contains(tank));
                scouts.retain(|scout| plan.ready_units.contains(scout));
            }
            let push_size = if push_uses_available_armor {
                push_tank_count(
                    tanks.len(),
                    required_tanks,
                    push_keep_home(observation, memory),
                )?
            } else {
                required_tanks
            };
            select_nearest_units(observation, &mut tanks, rally, push_size);
            select_nearest_units(observation, &mut scouts, rally, 1);
            if tanks.len() != push_size || scouts.is_empty() {
                return None;
            }
            memory.containment.active_tanks = tanks.iter().copied().collect();
            memory.containment.active_scout = scouts.first().copied();
            memory.containment.active_riflemen = select_rifle_escorts(observation, memory, rally)
                .into_iter()
                .collect();
            reset_containment_route(memory);
            memory.containment.last_formation_command_tick = None;
            memory.containment.assembly_started_tick = Some(observation.tick);
        } else if push_uses_available_armor {
            // Until it leaves, the push keeps taking in Tanks that become ready, so it stays most
            // of the army. A push that formed with 3 Tanks otherwise waited all game while 20 more
            // were built behind it.
            let mut exclusions: BTreeSet<u32> = memory
                .home_defensive_tank
                .into_iter()
                .chain(memory.later_bases.guards.iter().copied())
                .collect();
            exclusions.extend(memory.containment.active_tanks.iter().copied());
            let mut newcomers = actions::select_ready_combat_units_excluding(
                &observation.owned,
                &[EntityKind::Tank],
                &exclusions,
            );
            if !memory.containment.wave_launched {
                newcomers.retain(|tank| plan.ready_units.contains(tank));
            }
            let pushing = memory.containment.active_tanks.len();
            let wanted = push_tank_count(
                pushing + newcomers.len(),
                required_tanks,
                push_keep_home(observation, memory),
            )
            .unwrap_or(pushing);
            if wanted > pushing {
                select_nearest_units(observation, &mut newcomers, rally, wanted - pushing);
                memory
                    .containment
                    .active_tanks
                    .extend(newcomers.iter().copied());
            }
        }

        let tanks: Vec<u32> = memory.containment.active_tanks.iter().copied().collect();
        let scout = memory.containment.active_scout?;
        let formation_center = group_center(observation, &tanks).unwrap_or(rally);
        let assembly_started = *memory
            .containment
            .assembly_started_tick
            .get_or_insert(observation.tick);
        let assembly_elapsed = observation.tick.saturating_sub(assembly_started);
        let assembly_timed_out = assembly_elapsed >= CONTAINMENT_ASSEMBLY_TIMEOUT_TICKS;
        let assembly_hard_timed_out = assembly_elapsed >= CONTAINMENT_ASSEMBLY_HARD_TIMEOUT_TICKS;
        if assembly_timed_out {
            memory.containment.active_riflemen =
                select_rifle_escorts(observation, memory, formation_center)
                    .into_iter()
                    .collect();
        }
        let riflemen: Vec<u32> = memory.containment.active_riflemen.iter().copied().collect();
        let assembly_shape = if push_uses_available_armor {
            MarchShape::TIGHT
        } else {
            MarchShape::LEGACY
        };
        let formation = containment_formation(
            observation,
            &tanks,
            scout,
            &riflemen,
            formation_center,
            own_base,
            (enemy_base.x, enemy_base.y),
            policy,
            assembly_shape,
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
            assembly_shape,
        );
        let vehicle_core_grouped = formation_vehicle_core_is_grouped(
            observation,
            &formation,
            own_base,
            (enemy_base.x, enemy_base.y),
            assembly_shape,
        );
        let nearby_rifles = nearby_rifle_escort_count(observation, &riflemen, formation_center);
        let assembled = exact_assembly_ready
            || (assembly_timed_out
                && core_grouped
                && nearby_rifles >= MIN_CONTAINMENT_RIFLE_ESCORTS)
            || (assembly_hard_timed_out && vehicle_core_grouped);
        let river_opening_guard = !memory.containment.wave_launched
            && expansion::has_jeff_river_expansion_site(observation)
            && river_opening_guard_active(observation, &tanks, assembly_started, memory);
        // Do not send the push out while more enemy Tanks than it has were seen recently: it would
        // only lose them one or two at a time outside the base. It waits at the regroup point.
        // Crossroads needs a clear lead, since Jeff sees only part of AI 2.1's army.
        let lead_needed = if crossroads {
            CROSSROADS_PUSH_TANK_LEAD
        } else {
            0
        };
        let launch_outnumbered = memory.recent_enemy_tanks() + lead_needed > tanks.len();
        // Nor while enemy Tanks shelling a base outnumber the home Tanks near them.
        let home_outgunned = push_uses_available_armor && memory.home_outgunned;
        let assembly_ready =
            assembled && !river_opening_guard && !launch_outnumbered && !home_outgunned;
        if !assembly_ready {
            if formation_command_due(memory, observation.tick) {
                issue_containment_formation(actions, observation, &formation, false);
                if river_opening_guard && assembled {
                    hold_containment_tanks(actions, observation, memory, tanks.iter().copied());
                }
                note_formation_command(memory, observation.tick);
            }
            return Some(AiIntent::Assemble {
                units: formation.unit_ids(),
            });
        }

        if !memory.containment.wave_launched {
            memory.containment.opening_tanks = tanks.iter().copied().collect();
            memory.containment.wave_launched = true;
        }
        memory.containment.launch_tanks = tanks.len();
        memory.containment.recovery_active = false;
        memory.approach.start_push();
        memory.containment.stationary_since = None;
        reset_containment_route(memory);
        memory.containment.last_formation_command_tick = None;
        memory.containment.assembly_started_tick = None;
    }

    let tanks: Vec<u32> = memory.containment.active_tanks.iter().copied().collect();
    let scouts = memory
        .containment
        .active_scout
        .into_iter()
        .collect::<Vec<_>>();
    let riflemen: Vec<u32> = memory.containment.active_riflemen.iter().copied().collect();
    if tanks.is_empty() || scouts.is_empty() {
        return None;
    }

    // Outnumbered in Tanks out in the field: fall back to the regroup point and rebuild one Tank
    // larger, instead of losing the push a Tank at a time.
    if !memory.enemy_main_destroyed && push_outnumbered(observation, &tanks) {
        if push_uses_available_armor {
            memory
                .approach
                .note_failed_push(observation.player_id, observation.tick);
        }
        begin_containment_recovery(memory);
        let rally = containment_regroup_point(own_base, enemy_base, observation.map)?;
        let mut units = tanks;
        units.extend(scouts);
        units.extend(riflemen);
        actions::move_units(actions, units.iter().copied(), rally.0, rally.1);
        return Some(AiIntent::Assemble { units });
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
    let contact_target = if push_uses_available_armor {
        march_contact_target(observation, &tanks, policy.contact_stop_tiles)
    } else {
        visible_combat_target_within_tiles(observation, &tanks, policy.contact_stop_tiles)
    };
    if contact_target.is_some() {
        memory.containment.contact_last_tick = Some(observation.tick);
    }
    let contact_active = memory.containment.contact_last_tick.is_some_and(|last| {
        observation.tick.saturating_sub(last) <= CONTAINMENT_CONTACT_MEMORY_TICKS
    });
    // The current Jeff travels loosely to a staging point short of the target, reforms there, then
    // closes in tight. After a failed push it may come at the target from a side instead.
    // A small push (the two-Tank opening) marches as before: it is one rank, which never stalled,
    // and its two Tanks decide a duel by where they stand when they meet the enemy's.
    let legs = if push_uses_available_armor
        && !endgame_search_active
        && tanks.len() >= super::approach::MIN_TANKS_FOR_LEGS
    {
        containment_regroup_point(own_base, enemy_base, observation.map).map(|rally| {
            let legs = super::approach::push_legs(
                memory,
                map_analysis,
                observation,
                rally,
                own_base,
                (enemy_base.x, enemy_base.y),
                objective,
                tank_point,
                policy.tank_standoff_tiles,
            );
            super::approach::leg_orders(
                memory,
                observation,
                legs,
                group_center(observation, &tanks),
                contact_active,
                own_base,
                objective,
            )
        })
    } else {
        None
    };
    let (tank_point, face_from, face_to) = match legs {
        Some(orders) => (orders.destination, orders.face_from, orders.face_to),
        None => (tank_point, own_base, objective),
    };
    let shape = match legs.map(|orders| orders.phase) {
        Some(super::approach::PushPhase::Travel) => MarchShape::TRAVEL,
        _ if push_uses_available_armor => MarchShape::TIGHT,
        _ => MarchShape::LEGACY,
    };
    let toward_objective = normalized_direction(face_from, face_to)?;
    let tank_assignments = if tight_formation {
        ranked_tank_formation_assignments(
            observation,
            &tanks,
            tank_point,
            toward_objective,
            observation.map,
            shape.tank_spacing_tiles,
            shape.rank_width,
        )
    } else {
        tanks.iter().map(|tank_id| (*tank_id, tank_point)).collect()
    };
    let tolerance = tile_size
        * if tight_formation {
            shape.in_position_tiles
        } else {
            2.0
        };
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
        face_from,
        face_to,
        observation.map,
        policy.scout_trailing_tiles,
    )?;
    // Reformed at the staging point: in its slots with most of its Riflemen up. It closes in next.
    if legs.is_some_and(|orders| orders.phase == super::approach::PushPhase::Reform) {
        let reformed = tanks_in_position
            && nearby_rifle_escort_count(observation, &riflemen, tank_point)
                >= riflemen.len().div_ceil(2);
        memory.approach.note_reform(observation.tick, reformed);
    }
    let should_stop = tanks_in_position || contact_active;
    // A Tank Trap across the way is cleared before marching on, while nothing hostile is near.
    if push_uses_available_armor && !contact_active && !tanks_in_position {
        if let Some(trap) = obstacles::trap_across_push(
            observation,
            &tanks,
            tank_point,
            (enemy_base.x, enemy_base.y),
        ) {
            let refresh = memory.containment.trap_order_tick.is_none_or(|last| {
                observation.tick.saturating_sub(last) >= obstacles::TRAP_ORDER_REFRESH_TICKS
            });
            if refresh {
                actions::clear_obstacle_area(actions, tanks.iter().copied(), trap);
                memory.containment.trap_order_tick = Some(observation.tick);
            }
            return Some(AiIntent::Attack { units: tanks });
        }
    }
    let stationary_range_ready = if should_stop {
        let since = memory
            .containment
            .stationary_since
            .get_or_insert(observation.tick);
        observation.tick.saturating_sub(*since) >= config::TICK_HZ * 3
    } else {
        memory.containment.stationary_since = None;
        false
    };

    // Only a Tank under an attack order chases its target out of position. A holding Tank picked
    // its target from where it stands, inside its current range, so it is left to shoot.
    let current_tank_target_outside_leash = stationary_range_ready
        && tanks.iter().any(|tank_id| {
            tanks_by_id
                .get(tank_id)
                .filter(|_| !tank_is_holding(observation, memory, *tank_id))
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
        hold_containment_tanks(actions, observation, memory, tanks.iter().copied());
        memory.containment.focus_target = None;
        memory.containment.focus_stable_since = None;
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
                    memory.containment.focus_stable_since = Some(observation.tick);
                }
                let target = shared_stationary_tank_target(
                    observation,
                    &tanks,
                    policy.tank_standoff_tiles,
                    locked_focus
                        .or(consensus_target)
                        .or(memory.containment.focus_target),
                    memory.containment.smoke_target,
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
                        let smoke_expiry_before = memory.containment.smoke_expires_tick;
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
                            && memory.containment.smoke_expires_tick.is_some();
                        let holding: Vec<u32> = tanks
                            .iter()
                            .copied()
                            .filter(|tank| tank_is_holding(observation, memory, *tank))
                            .collect();
                        issue_hp_aware_tank_volley(
                            actions,
                            observation,
                            &tanks,
                            &holding,
                            target,
                            policy.tank_standoff_tiles,
                            memory.containment.smoke_target,
                        );
                    } else if target_is_in_shared_tank_range(
                        observation,
                        &tanks,
                        target,
                        policy.tank_standoff_tiles,
                    ) {
                        actions::attack_units(actions, tanks.iter().copied(), target);
                    } else {
                        hold_containment_tanks(actions, observation, memory, tanks.iter().copied());
                    }
                } else if endgame_search_active {
                    memory.endgame_search_waypoint =
                        (memory.endgame_search_waypoint + 1) % ENDGAME_SEARCH_OFFSETS.len();
                    memory.containment.stationary_since = None;
                    let next = endgame_search_point(
                        own_base,
                        enemy_base,
                        observation.map,
                        memory.endgame_search_waypoint,
                    );
                    actions::attack_move_units(actions, tanks.iter().copied(), next.0, next.1);
                } else {
                    hold_containment_tanks(actions, observation, memory, tanks.iter().copied());
                    memory.containment.focus_target = None;
                    memory.containment.focus_stable_since = None;
                }
            } else {
                hold_containment_tanks(actions, observation, memory, tanks.iter().copied());
            }

            let scout_point = if let Some(smoke_launch_point) = smoke_reposition {
                smoke_launch_point
            } else if stationary_range_ready && tanks_in_position {
                if tight_formation {
                    scout_forward_from_tanks(
                        tank_anchor,
                        face_from,
                        face_to,
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
                face_from,
                face_to,
                policy,
                shape,
            )?;
            let waypoint_timed_out =
                memory
                    .containment
                    .waypoint_started_tick
                    .is_some_and(|started| {
                        observation.tick.saturating_sub(started)
                            >= CONTAINMENT_WAYPOINT_TIMEOUT_TICKS
                    });
            if waypoint_timed_out {
                let tank_center = group_center(observation, &tanks).unwrap_or(current_waypoint);
                retain_nearby_rifle_escorts(
                    observation,
                    &mut memory.containment.active_riflemen,
                    tank_center,
                );
            }
            // The current Jeff's Tanks and Scout Car lead: the next waypoint is ordered once they
            // are in place, and the Riflemen catch up rather than holding every step.
            let vehicles_placed = push_uses_available_armor
                && formation_vehicles_in_position(observation, &formation, shape.arrival_tiles);
            // Travelling, the push moves on once its Tanks' centre reaches the waypoint.
            let center_arrived = shape.advance_on_center
                && group_center(observation, &tanks).is_some_and(|center| {
                    dist2(center.0, center.1, current_waypoint.0, current_waypoint.1)
                        <= (shape.arrival_tiles * tile_size).powi(2)
                });
            if vehicles_placed
                || center_arrived
                || formation_units_in_position(observation, &formation, shape.arrival_tiles)
                || (waypoint_timed_out
                    && formation_vehicle_core_is_grouped(
                        observation,
                        &formation,
                        face_from,
                        face_to,
                        shape,
                    ))
            {
                memory.containment.march_waypoint = None;
                memory.containment.last_formation_command_tick = None;
                memory.containment.waypoint_started_tick = None;
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
            let tanks_are_cohesive =
                tank_group_is_cohesive(observation, &tanks, toward_objective, shape);
            if !tanks_are_cohesive && lead_anchor_tank_catchup {
                let lead_tank = frontmost_unit_id(observation, &tanks, toward_objective)?;
                let rear_tank = rearmost_unit_id(observation, &tanks, toward_objective)?;
                let lead_position = unit_position(observation, lead_tank)?;
                let rear_position = unit_position(observation, rear_tank)?;
                let direct_catch_up_point =
                    tank_catch_up_point(lead_position, face_from, face_to, observation.map)?;
                let route_catch_up_point =
                    defense::crossroads_wall_aware_approach_direction(observation).and_then(|_| {
                        map_analysis.and_then(|analysis| {
                            tank_catch_up_point_on_route(analysis, rear_position, lead_position)
                        })
                    });
                let catch_up_point = route_catch_up_point.unwrap_or(direct_catch_up_point);

                // Freeze the forward Tank at its current progress and give only the rear Tank a
                // fresh point behind it. Centering a whole formation on the rear Tank's current
                // position gives that Tank no forward destination and can make the pair wait
                // forever when pathing is obstructed.
                // On Crossroads, a direct recovery point can cut through a water wall. Use the
                // compact-group route when it is available and preserve the lead Tank's existing
                // attack-move so it does not stall inside the narrow approach corridor.
                if route_catch_up_point.is_none() {
                    hold_containment_tanks(actions, observation, memory, [lead_tank]);
                }
                actions::attack_move_units(
                    actions,
                    [rear_tank],
                    catch_up_point.0,
                    catch_up_point.1,
                );
                note_formation_command(memory, observation.tick);
                return Some(AiIntent::Attack { units: tanks });
            }
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
                    shape.step_tiles,
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
                face_from,
                face_to,
                policy,
                shape,
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
