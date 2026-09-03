use super::*;

pub(super) fn active_smoke_focus(observation: &AiObservation, memory: &mut AiDecisionMemory) -> Option<u32> {
    let active = memory
        .containment_smoke_expires_tick
        .is_some_and(|expires| observation.tick < expires);
    if !active {
        memory.containment_smoke_target = None;
        memory.containment_smoke_focus_target = None;
        memory.containment_smoke_expires_tick = None;
        return None;
    }
    let focus = memory.containment_smoke_focus_target?;
    if observation
        .visible_enemies
        .iter()
        .any(|enemy| enemy.id == focus)
    {
        Some(focus)
    } else {
        // The exposed target can die before the cloud expires. Keep excluding the obscured Tank
        // for the cloud's lifetime, but release the obsolete focus lock immediately.
        memory.containment_smoke_focus_target = None;
        None
    }
}

pub(super) fn issue_hp_aware_tank_volley(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    tanks: &[u32],
    primary_target: u32,
    range_tiles: f32,
    excluded_target: Option<u32>,
) {
    let mut remaining_tanks = tanks.to_vec();
    let mut targets =
        shared_stationary_tank_targets(observation, tanks, range_tiles, excluded_target);
    if let Some(index) = targets
        .iter()
        .position(|target| target.id == primary_target)
    {
        let primary = targets.remove(index);
        targets.insert(0, primary);
    }
    for target in targets {
        if remaining_tanks.is_empty() {
            break;
        }
        remaining_tanks.sort_by(|left, right| {
            let distance = |tank_id: &u32| {
                observation
                    .owned
                    .iter()
                    .find(|unit| unit.id == *tank_id)
                    .map(|tank| dist2(tank.x, tank.y, target.x, target.y))
                    .unwrap_or(f32::MAX)
            };
            distance(left)
                .total_cmp(&distance(right))
                .then_with(|| left.cmp(right))
        });
        let shots = target.hp.max(1).div_ceil(TANK_VOLLEY_DAMAGE) as usize;
        let assigned_count = shots.min(remaining_tanks.len());
        let assigned = remaining_tanks.drain(..assigned_count).collect::<Vec<_>>();
        actions::attack_units(actions, assigned, target.id);
    }
    if !remaining_tanks.is_empty() {
        actions::hold_position_units(actions, remaining_tanks);
    }
}

pub(super) fn maybe_issue_isolation_smoke(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    tanks: &[u32],
    scout: u32,
    focus_target: &mut u32,
    memory: &mut AiDecisionMemory,
    require_stable_focus: bool,
) -> Option<(f32, f32)> {
    if memory.containment_smoke_expires_tick.is_some() {
        return None;
    }
    if require_stable_focus
        && memory.containment_focus_stable_since.is_none_or(|since| {
            observation.tick.saturating_sub(since) < CONTAINMENT_FOCUS_STABLE_TICKS
        })
    {
        return None;
    }
    if !observation
        .owned
        .iter()
        .any(|entity| entity.kind == EntityKind::EngineeringComplex && entity.is_complete)
    {
        return None;
    }
    let smoke_ready = observation.ability_states.iter().any(|ability| {
        ability.entity_id == scout
            && ability.kind == AbilityKind::Smoke
            && ability.cooldown_left == 0
            && ability.remaining_uses.is_none_or(|uses| uses > 0)
            && ability
                .available_tick
                .is_none_or(|tick| tick <= observation.tick)
            && ability
                .lockout_until_tick
                .is_none_or(|tick| tick <= observation.tick)
    });
    if !smoke_ready {
        return None;
    }
    let mut focus = observation
        .visible_enemies
        .iter()
        .find(|enemy| enemy.id == *focus_target)?;
    let center = group_center(observation, tanks).unwrap_or((focus.x, focus.y));
    let tile_size = observation.map.tile_size as f32;
    let local_target_radius2 = (CONTAINMENT_LOCAL_SMOKE_TARGET_TILES * tile_size).powi(2);
    let mut smoke_candidates = observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            enemy.kind == EntityKind::Tank
                && enemy.id != *focus_target
                && dist2(center.0, center.1, enemy.x, enemy.y) <= local_target_radius2
        })
        .collect::<Vec<_>>();
    let mut singleton_suppression = false;
    let candidate = if smoke_candidates.is_empty() && focus.kind == EntityKind::Tank {
        // Prefer switching the volley to another exposed unit while the lone local Tank is hidden.
        let alternate = shared_stationary_tank_targets(
            observation,
            tanks,
            CONTAINMENT_SMOKE_RANGE_TILES,
            Some(focus.id),
        )
        .into_iter()
        .find(|target| target.kind != EntityKind::Tank);
        if let Some(alternate) = alternate {
            let candidate = focus;
            focus = alternate;
            *focus_target = alternate.id;
            memory.containment_focus_target = Some(alternate.id);
            memory.containment_focus_stable_since = Some(observation.tick);
            candidate
        } else {
            // Compact fronts sometimes expose one Tank and nothing else in shared range. Blind it
            // for one cloud duration so the rifle screen can advance while our grouped Tanks hold.
            singleton_suppression = true;
            focus
        }
    } else {
        if smoke_candidates.is_empty() {
            return None;
        }
        let focus_distance = dist2(center.0, center.1, focus.x, focus.y);
        let forward_distance = smoke_candidates
            .iter()
            .map(|tank| dist2(center.0, center.1, tank.x, tank.y))
            .fold(f32::MAX, f32::min);
        if focus.kind == EntityKind::Tank && focus_distance > forward_distance {
            // If our Tanks have committed to the rear Tank, obey the focus rather than smoking it:
            // isolate the unfocused forward Tank instead.
            smoke_candidates.sort_by(|left, right| {
                dist2(center.0, center.1, left.x, left.y)
                    .total_cmp(&dist2(center.0, center.1, right.x, right.y))
                    .then_with(|| left.id.cmp(&right.id))
            });
        } else {
            // Normal case: obscure the healthy rear Tank and leave the forward target exposed.
            smoke_candidates.sort_by(|left, right| {
                (left.hp < 263)
                    .cmp(&(right.hp < 263))
                    .then_with(|| {
                        dist2(center.0, center.1, right.x, right.y)
                            .total_cmp(&dist2(center.0, center.1, left.x, left.y))
                    })
                    .then_with(|| right.hp.cmp(&left.hp))
                    .then_with(|| left.id.cmp(&right.id))
            });
        }
        smoke_candidates[0]
    };
    let scout_entity = observation.owned.iter().find(|unit| unit.id == scout)?;
    let smoke_radius_tiles = if observation.upgrades.contains(&UpgradeKind::SmokePlus) {
        CONTAINMENT_SMOKE_PLUS_RADIUS_TILES
    } else {
        CONTAINMENT_SMOKE_BASE_RADIUS_TILES
    };
    let clearance = (smoke_radius_tiles + CONTAINMENT_SMOKE_SAFETY_TILES) * tile_size;
    let aim_offset = (smoke_radius_tiles - CONTAINMENT_SMOKE_AIM_INSET_TILES).max(0.0) * tile_size;
    let away_from_focus = normalized_direction((focus.x, focus.y), (candidate.x, candidate.y));
    let perpendicular = away_from_focus.map(|direction| (-direction.1, direction.0));
    let mut aim_points = vec![(candidate.x, candidate.y)];
    if let Some(direction) = away_from_focus {
        aim_points.push((
            candidate.x + direction.0 * aim_offset,
            candidate.y + direction.1 * aim_offset,
        ));
    }
    if let Some(direction) = perpendicular {
        aim_points.push((
            candidate.x + direction.0 * aim_offset,
            candidate.y + direction.1 * aim_offset,
        ));
        aim_points.push((
            candidate.x - direction.0 * aim_offset,
            candidate.y - direction.1 * aim_offset,
        ));
    }
    aim_points = aim_points
        .into_iter()
        .map(|point| clamp_to_map(point, observation.map))
        .filter(|point| {
            (singleton_suppression || dist2(focus.x, focus.y, point.0, point.1) > clearance.powi(2))
                && !observation.smokes.iter().any(|cloud| {
                    let combined =
                        (cloud.radius_tiles + CONTAINMENT_SMOKE_SAFETY_TILES) * tile_size;
                    dist2(cloud.x, cloud.y, point.0, point.1) <= combined.powi(2)
                })
                && (singleton_suppression
                    || tanks.iter().all(|tank_id| {
                        observation
                            .owned
                            .iter()
                            .find(|unit| unit.id == *tank_id)
                            .is_none_or(|tank| {
                                point_segment_distance2(
                                    *point,
                                    (tank.x, tank.y),
                                    (focus.x, focus.y),
                                ) > clearance.powi(2)
                            })
                    }))
        })
        .collect();
    aim_points.sort_by(|left, right| {
        dist2(scout_entity.x, scout_entity.y, left.0, left.1).total_cmp(&dist2(
            scout_entity.x,
            scout_entity.y,
            right.0,
            right.1,
        ))
    });
    let smoke_point = aim_points.first().copied()?;
    if dist2(scout_entity.x, scout_entity.y, smoke_point.0, smoke_point.1)
        > (CONTAINMENT_SMOKE_RANGE_TILES * tile_size).powi(2)
    {
        return bounded_scout_smoke_launch_point(
            observation,
            center,
            (focus.x, focus.y),
            smoke_point,
        );
    }
    actions::use_world_ability(
        actions,
        scout,
        AbilityKind::Smoke,
        smoke_point.0,
        smoke_point.1,
    );
    memory.containment_smoke_target = Some(candidate.id);
    memory.containment_smoke_focus_target = (!singleton_suppression).then_some(*focus_target);
    let smoke_duration = if observation.upgrades.contains(&UpgradeKind::SmokePlus) {
        CONTAINMENT_SMOKE_DURATION_TICKS * 2
    } else {
        CONTAINMENT_SMOKE_DURATION_TICKS
    };
    memory.containment_smoke_expires_tick = Some(
        observation
            .tick
            .saturating_add(smoke_duration + config::TICK_HZ),
    );
    None
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ai_core::decision) enum LocalDefenseSmokeDirective {
    Obscure { target: u32, scout: u32 },
    Reposition { scout: u32 },
}

pub(in crate::ai_core::decision) fn maybe_issue_local_defense_smoke(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    local_interceptors: &[u32],
    local_defenders: &[u32],
    local_targets: &[u32],
    memory: &mut AiDecisionMemory,
) -> Option<LocalDefenseSmokeDirective> {
    let _ = active_smoke_focus(observation, memory);
    let tanks = local_interceptors
        .iter()
        .filter(|id| {
            observation.owned.iter().any(|unit| {
                unit.id == **id && unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0
            })
        })
        .copied()
        .collect::<Vec<_>>();
    if tanks.is_empty() || tanks.len() > MAX_LOCAL_DEFENSE_SMOKE_TANKS {
        return None;
    }
    let center = group_center(observation, &tanks)?;
    let scout = local_defenders
        .iter()
        .filter_map(|id| {
            observation
                .owned
                .iter()
                .find(|unit| {
                    unit.id == *id
                        && unit.kind == EntityKind::ScoutCar
                        && unit.is_complete
                        && unit.hp > 0
                })
                .map(|unit| (*id, dist2(center.0, center.1, unit.x, unit.y)))
        })
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        })?
        .0;

    if let Some(target) = memory
        .containment_smoke_target
        .filter(|target| local_targets.contains(target))
    {
        return Some(LocalDefenseSmokeDirective::Obscure { target, scout });
    }

    let mut focus = observation
        .visible_enemies
        .iter()
        .filter(|enemy| local_targets.contains(&enemy.id) && enemy.kind == EntityKind::Tank)
        .min_by(|left, right| {
            dist2(center.0, center.1, left.x, left.y)
                .total_cmp(&dist2(center.0, center.1, right.x, right.y))
                .then_with(|| left.id.cmp(&right.id))
        })?
        .id;
    let launch_point = maybe_issue_isolation_smoke(
        actions,
        observation,
        &tanks,
        scout,
        &mut focus,
        memory,
        false,
    );
    if let Some(point) = launch_point {
        actions::move_units(actions, [scout], point.0, point.1);
        return Some(LocalDefenseSmokeDirective::Reposition { scout });
    }
    memory
        .containment_smoke_target
        .filter(|target| local_targets.contains(target))
        .map(|target| LocalDefenseSmokeDirective::Obscure { target, scout })
}

fn bounded_scout_smoke_launch_point(
    observation: &AiObservation,
    tank_center: (f32, f32),
    focus: (f32, f32),
    smoke_point: (f32, f32),
) -> Option<(f32, f32)> {
    let forward = normalized_direction(tank_center, focus)?;
    let lateral_axis = (-forward.1, forward.0);
    let toward_tanks = normalized_direction(smoke_point, tank_center)?;
    let tile_size = observation.map.tile_size as f32;
    let cast_buffer = (CONTAINMENT_SMOKE_RANGE_TILES - 0.5) * tile_size;
    let raw = (
        smoke_point.0 + toward_tanks.0 * cast_buffer,
        smoke_point.1 + toward_tanks.1 * cast_buffer,
    );
    let relative = (raw.0 - tank_center.0, raw.1 - tank_center.1);
    let longitudinal = (relative.0 * forward.0 + relative.1 * forward.1).clamp(
        -CONTAINMENT_SCOUT_SMOKE_REAR_LIMIT_TILES * tile_size,
        CONTAINMENT_SCOUT_SMOKE_FORWARD_LIMIT_TILES * tile_size,
    );
    let lateral = (relative.0 * lateral_axis.0 + relative.1 * lateral_axis.1).clamp(
        -CONTAINMENT_SCOUT_SMOKE_LATERAL_LIMIT_TILES * tile_size,
        CONTAINMENT_SCOUT_SMOKE_LATERAL_LIMIT_TILES * tile_size,
    );
    let launch_point = clamp_to_map(
        (
            tank_center.0 + forward.0 * longitudinal + lateral_axis.0 * lateral,
            tank_center.1 + forward.1 * longitudinal + lateral_axis.1 * lateral,
        ),
        observation.map,
    );
    (dist2(launch_point.0, launch_point.1, smoke_point.0, smoke_point.1)
        <= (CONTAINMENT_SMOKE_RANGE_TILES * tile_size).powi(2))
    .then_some(launch_point)
}

fn point_segment_distance2(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let segment = (end.0 - start.0, end.1 - start.1);
    let length2 = segment.0 * segment.0 + segment.1 * segment.1;
    if length2 <= f32::EPSILON {
        return dist2(point.0, point.1, start.0, start.1);
    }
    let projection = (((point.0 - start.0) * segment.0 + (point.1 - start.1) * segment.1)
        / length2)
        .clamp(0.0, 1.0);
    let nearest = (
        start.0 + segment.0 * projection,
        start.1 + segment.1 * projection,
    );
    dist2(point.0, point.1, nearest.0, nearest.1)
}
