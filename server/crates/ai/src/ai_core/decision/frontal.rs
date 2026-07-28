use super::geometry::{clamp_to_map, dist2, normalized_direction, tile_center};
use super::*;

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

fn issue_expansion_containment_wave(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    plan: &FrontalWavePlan,
    enemy_base: EnemyBaseFact,
    policy: ExpansionContainmentPolicy,
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
    update_enemy_natural_state(observation, natural_objective, enemy_base, &scouts, memory);
    let objective = if memory.enemy_natural_destroyed {
        (enemy_base.x, enemy_base.y)
    } else {
        natural_objective
    };
    let (tank_point, scout_point) =
        containment_points(own_base, objective, observation.map, policy)?;
    if !memory.containment_wave_launched && tanks.len() >= policy.minimum_tanks_to_continue {
        memory
            .containment_opening_tanks
            .extend(tanks.iter().copied());
        memory.containment_wave_launched = true;
    }
    if memory.containment_wave_launched && tanks.len() < policy.minimum_tanks_to_continue {
        memory.containment_stationary_since = None;
        let fallback = own_base;
        actions::move_units(actions, tanks.iter().copied(), fallback.0, fallback.1);
        actions::move_units(actions, scouts.iter().copied(), fallback.0, fallback.1);
        tanks.extend(scouts);
        tanks.sort_unstable();
        tanks.dedup();
        return Some(AiIntent::Stage { units: tanks });
    }

    // Multiple Tanks cannot occupy the same point. A two-tile arrival radius
    // lets the formation settle and charge its stationary range instead of
    // repeatedly translating because of collision separation.
    let tolerance = tile_size * 2.0;
    let tolerance2 = tolerance * tolerance;
    let tanks_in_position = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id))
        .all(|unit| dist2(unit.x, unit.y, tank_point.0, tank_point.1) <= tolerance2);
    let tank_center = group_center(observation, &tanks)?;
    let trailing_point = scout_trailing_point(
        tank_center,
        own_base,
        objective,
        observation.map,
        policy.scout_trailing_tiles,
    )?;
    let contact_threat =
        visible_anti_armor_target_within_tiles(observation, &tanks, policy.contact_stop_tiles);
    let should_stop = tanks_in_position || contact_threat.is_some();
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
            if let Some(target) = contact_threat
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
                        policy.tank_standoff_tiles,
                    )
                })
            {
                actions::attack_units(actions, tanks.iter().copied(), target);
            } else {
                actions::hold_position_units(actions, tanks.iter().copied());
            }
        } else {
            actions::hold_position_units(actions, tanks.iter().copied());
        }
    } else {
        // Attack-move handles interceptors without converting them into a chase
        // target, so the formation continues toward the containment anchor.
        actions::attack_move_units(actions, tanks.iter().copied(), tank_point.0, tank_point.1);
    }
    let scout_point = if stationary_range_ready {
        if tanks_in_position {
            scout_point
        } else {
            scout_forward_from_tanks(
                tank_center,
                own_base,
                objective,
                observation.map,
                policy.scout_forward_tiles,
            )?
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
    let main_exclusion2 = (config::CC_RESOURCE_MAX_DIST_TILES * tile_size)
        * (config::CC_RESOURCE_MAX_DIST_TILES * tile_size);
    let visible_natural = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::CityCentre)
        .filter(|enemy| dist2(enemy.x, enemy.y, enemy_base.x, enemy_base.y) > main_exclusion2)
        .filter(|enemy| dist2(enemy.x, enemy.y, natural.0, natural.1) <= natural_radius2)
        .min_by_key(|enemy| enemy.id);
    if let Some(city_centre) = visible_natural {
        memory.enemy_natural_city_centre = Some(city_centre.id);
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

fn enemy_natural_edge(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
) -> Option<(f32, f32)> {
    let tile_size = observation.map.tile_size as f32;
    let start_exclusion = (config::CC_RESOURCE_MAX_DIST_TILES + 1.5) * tile_size;
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
                EntityKind::CityCentre => 0,
                EntityKind::Factory | EntityKind::Steelworks => 1,
                EntityKind::ResearchComplex | EntityKind::TrainingCentre => 2,
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
}
