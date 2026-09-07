use super::*;

pub(super) const SEARCH_TICKS: u32 = config::TICK_HZ * 2;
const REACQUIRE_TILES: f32 = 1.5;
const EARLY_ENTRENCHED_RESPONSE_TICKS: u32 = config::TICK_HZ * 60 * 4;
const MIN_ENTRENCHED_HOME_GUARDS: usize = 2;
const MAX_ENTRENCHED_INTERCEPTORS: usize = 2;

pub(in crate::ai_core::decision) fn respond_to_local_incident(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    local_defenders: &[u32],
) -> Option<Vec<u32>> {
    if let Some(contact) = local_defense_contact(observation) {
        memory.note_defensive_contact(
            observation.tick,
            contact.intercept,
            contact.threat_value,
            contact.armored_threat,
        );
        let mut interceptors = select_defensive_interceptors(
            observation,
            memory,
            eligible_local_defenders(observation, local_defenders),
            contact.intercept,
            contact.threat_value,
            contact.armored_threat,
        );
        let smoke = crate::ai_core::decision::frontal::smoke::maybe_issue_local_defense_smoke(
            actions,
            observation,
            &interceptors,
            local_defenders,
            &contact.target_ids,
            memory,
        );
        let mut attack_targets = contact.target_ids.clone();
        if let Some(smoke) = smoke {
            let scout = match smoke {
                crate::ai_core::decision::frontal::smoke::LocalDefenseSmokeDirective::Obscure {
                    target,
                    scout,
                } => {
                    attack_targets.retain(|candidate| *candidate != target);
                    scout
                }
                crate::ai_core::decision::frontal::smoke::LocalDefenseSmokeDirective::Reposition {
                    scout,
                } => scout,
            };
            interceptors.retain(|unit| *unit != scout);
        }
        return if let Some(target) = primary_defense_target(observation, &attack_targets) {
            issue_local_interception(
                actions,
                observation,
                interceptors,
                target,
                contact.intercept,
            )
        } else {
            actions::hold_position_units(actions, interceptors)
        };
    }

    let incident = memory.defensive_incident(observation.tick, SEARCH_TICKS)?;
    let candidates = local_defense_units_with_plans(observation, local_defenders);
    let reacquire2 = squared(REACQUIRE_TILES * observation.map.tile_size as f32);
    let reached_last_contact = candidates.iter().any(|id| {
        observation.owned.iter().any(|unit| {
            unit.id == *id
                && dist2(unit.x, unit.y, incident.position.0, incident.position.1) <= reacquire2
        })
    });
    if reached_last_contact {
        memory.clear_defensive_incident();
        return None;
    }
    let interceptors = select_defensive_interceptors(
        observation,
        memory,
        candidates,
        incident.position,
        incident.threat_value,
        incident.armored_threat,
    );
    actions::attack_move_units(
        actions,
        interceptors,
        incident.position.0,
        incident.position.1,
    )
}

fn issue_local_interception(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    interceptors: Vec<u32>,
    target: u32,
    intercept: (f32, f32),
) -> Option<Vec<u32>> {
    let (bounded_riflemen, direct): (Vec<_>, Vec<_>) = interceptors.into_iter().partition(|unit_id| {
        observation.tick < EARLY_ENTRENCHED_RESPONSE_TICKS
            && observation
                .owned
                .iter()
                .any(|unit| unit.id == *unit_id && unit.kind == EntityKind::Rifleman)
    });
    let mut issued = Vec::new();
    if let Some(units) = actions::attack_units(actions, direct, target) {
        issued.extend(units);
    }
    // Early Riflemen move only to the footprint-side intercept. This remains true after released
    // trench guards start moving and no longer qualify as entrenched, so later decisions cannot
    // turn the bounded response into a chase across an open map or around Crossroads walls.
    if let Some(units) =
        actions::attack_move_units(actions, bounded_riflemen, intercept.0, intercept.1)
    {
        issued.extend(units);
    }
    issued.sort_unstable();
    issued.dedup();
    (!issued.is_empty()).then_some(issued)
}

fn eligible_local_defenders(observation: &AiObservation, local_defenders: &[u32]) -> Vec<u32> {
    local_defense_units_with_plans(observation, local_defenders)
        .into_iter()
        .filter(|id| {
            observation.owned.iter().any(|unit| {
                unit.id == *id
                    && matches!(
                        unit.kind,
                        EntityKind::Rifleman
                            | EntityKind::MachineGunner
                            | EntityKind::ScoutCar
                            | EntityKind::Panzerfaust
                            | EntityKind::Tank
                    )
            })
        })
        .collect()
}

pub(in crate::ai_core::decision) fn select_defensive_interceptors(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    mut candidates: Vec<u32>,
    contact: (f32, f32),
    threat_value: u32,
    armored_threat: bool,
) -> Vec<u32> {
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    candidates.sort_by(|left, right| {
        let left_counter_rank = by_id.get(left).map_or(u8::MAX, |unit| {
            defensive_counter_rank(unit.kind, armored_threat)
        });
        let right_counter_rank = by_id.get(right).map_or(u8::MAX, |unit| {
            defensive_counter_rank(unit.kind, armored_threat)
        });
        let left_entrenchment = memory.estimated_entrenchment_ticks(observation, *left);
        let right_entrenchment = memory.estimated_entrenchment_ticks(observation, *right);
        let left_dist = by_id
            .get(left)
            .map(|unit| dist2(unit.x, unit.y, contact.0, contact.1))
            .unwrap_or(f32::INFINITY);
        let right_dist = by_id
            .get(right)
            .map(|unit| dist2(unit.x, unit.y, contact.0, contact.1))
            .unwrap_or(f32::INFINITY);
        left_counter_rank
            .cmp(&right_counter_rank)
            .then_with(|| left_entrenchment.cmp(&right_entrenchment))
            .then_with(|| left_dist.total_cmp(&right_dist))
            .then_with(|| left.cmp(right))
    });
    candidates.dedup();
    let entrenched: Vec<u32> = candidates
        .iter()
        .copied()
        .filter(|unit_id| entrenched_rifleman(observation, memory, *unit_id))
        .collect();
    let released_entrenched = if observation.tick < EARLY_ENTRENCHED_RESPONSE_TICKS {
        entrenched
            .len()
            .saturating_sub(MIN_ENTRENCHED_HOME_GUARDS)
            .min(MAX_ENTRENCHED_INTERCEPTORS)
    } else {
        0
    };
    let released_entrenched: BTreeSet<u32> =
        entrenched.into_iter().take(released_entrenched).collect();
    candidates.retain(|unit_id| {
        !entrenched_rifleman(observation, memory, *unit_id) || released_entrenched.contains(unit_id)
    });
    // A two-to-one observed-value response clears a small penetration without uprooting every
    // entrenched edge guard. Unentrenched units sort first, but entrenched Riflemen remain valid
    // interceptors when the mobile reserve is insufficient. The deterministic home-pocket planner
    // sends them back to their original slots as soon as the incident clears.
    let required_value = threat_value.saturating_mul(2).max(1);
    let mut selected = Vec::new();
    let mut selected_value: u32 = 0;
    let mut has_anti_armor = false;
    let eligible_candidates = candidates.clone();
    for unit_id in candidates {
        let Some(unit) = by_id.get(&unit_id) else {
            continue;
        };
        selected.push(unit_id);
        has_anti_armor |= matches!(unit.kind, EntityKind::Tank | EntityKind::Panzerfaust);
        let (steel, oil) = rts_rules::economy::cost(unit.kind);
        selected_value = selected_value.saturating_add(steel.saturating_add(oil).max(1));
        if selected_value >= required_value {
            break;
        }
    }
    let selected_has_tank = selected.iter().any(|unit_id| {
        by_id
            .get(unit_id)
            .is_some_and(|unit| unit.kind == EntityKind::Tank)
    });
    let river_natural =
        crate::ai_core::decision::expansion::has_jeff_river_expansion_site(observation);
    if selected_has_tank && river_natural {
        for kind in [EntityKind::Tank, EntityKind::Rifleman] {
            for unit_id in eligible_candidates
                .iter()
                .copied()
                .filter(|unit_id| by_id.get(unit_id).is_some_and(|unit| unit.kind == kind))
            {
                if !selected.contains(&unit_id) {
                    selected.push(unit_id);
                }
                let selected_of_kind = selected
                    .iter()
                    .filter(|selected_id| {
                        by_id.get(selected_id).is_some_and(|unit| unit.kind == kind)
                    })
                    .count();
                if selected_of_kind >= 2 {
                    break;
                }
            }
        }
    }
    if selected_value < required_value
        && !has_anti_armor
        && (armored_threat || observation.tick >= EARLY_ENTRENCHED_RESPONSE_TICKS)
    {
        Vec::new()
    } else {
        selected
    }
}

fn entrenched_rifleman(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    unit_id: u32,
) -> bool {
    observation.owned.iter().any(|unit| {
        unit.id == unit_id
            && unit.kind == EntityKind::Rifleman
            && memory.estimated_entrenchment_ticks(observation, unit_id)
                >= rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
    })
}

fn defensive_counter_rank(kind: EntityKind, armored_threat: bool) -> u8 {
    if armored_threat {
        match kind {
            EntityKind::Tank | EntityKind::Panzerfaust => 0,
            EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::ScoutCar => 1,
            _ => 2,
        }
    } else {
        0
    }
}

fn primary_defense_target(observation: &AiObservation, target_ids: &[u32]) -> Option<u32> {
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| target_ids.contains(&enemy.id))
        .max_by(|left, right| {
            let left_armored = matches!(left.kind, EntityKind::Tank | EntityKind::ScoutCar);
            let right_armored = matches!(right.kind, EntityKind::Tank | EntityKind::ScoutCar);
            left_armored
                .cmp(&right_armored)
                .then_with(|| unit_value(left.kind).cmp(&unit_value(right.kind)))
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|enemy| enemy.id)
}
