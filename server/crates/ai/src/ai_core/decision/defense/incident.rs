use super::*;

pub(super) const SEARCH_TICKS: u32 = config::TICK_HZ * 2;
const REACQUIRE_TILES: f32 = 1.5;

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
        );
        let interceptors = select_defensive_interceptors(
            observation,
            memory,
            eligible_local_defenders(observation, local_defenders),
            contact.intercept,
            contact.threat_value,
        );
        return actions::attack_move_units(
            actions,
            interceptors,
            contact.intercept.0,
            contact.intercept.1,
        );
    }

    let incident = memory.defensive_incident(observation.tick, SEARCH_TICKS)?;
    let candidates = local_defense_units_with_plans(observation, local_defenders);
    let reacquire2 = squared(REACQUIRE_TILES * observation.map.tile_size as f32);
    let reached_last_contact = candidates.iter().any(|id| {
        observation.owned.iter().any(|unit| {
            unit.id == *id
                && dist2(
                    unit.x,
                    unit.y,
                    incident.position.0,
                    incident.position.1,
                ) <= reacquire2
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
    );
    actions::attack_move_units(
        actions,
        interceptors,
        incident.position.0,
        incident.position.1,
    )
}

fn eligible_local_defenders(
    observation: &AiObservation,
    local_defenders: &[u32],
) -> Vec<u32> {
    local_defense_units_with_plans(observation, local_defenders)
        .into_iter()
        .filter(|id| {
            observation.owned.iter().any(|unit| {
                unit.id == *id
                    && matches!(
                        unit.kind,
                        EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::ScoutCar
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
) -> Vec<u32> {
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    candidates.sort_by(|left, right| {
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
        left_entrenchment
            .cmp(&right_entrenchment)
            .then_with(|| left_dist.total_cmp(&right_dist))
            .then_with(|| left.cmp(right))
    });
    candidates.dedup();

    // A two-to-one observed-value response clears a small penetration without uprooting every
    // entrenched edge guard. Larger forces naturally exhaust the available home reserve.
    let required_value = threat_value.saturating_mul(2).max(1);
    let mut selected = Vec::new();
    let mut selected_value: u32 = 0;
    for unit_id in candidates {
        let Some(unit) = by_id.get(&unit_id) else {
            continue;
        };
        selected.push(unit_id);
        let (steel, oil) = rts_rules::economy::cost(unit.kind);
        selected_value = selected_value.saturating_add(steel.saturating_add(oil).max(1));
        if selected_value >= required_value {
            break;
        }
    }
    selected
}
