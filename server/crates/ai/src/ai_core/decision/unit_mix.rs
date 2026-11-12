//! Which units production aims for this decision: the profile's priorities adjusted for upgrades,
//! Jeff's fast-Tank timing, Turtle's opening and entrenchment, and defensive Machine Gunners, and
//! the unit caps that go with them.

use super::*;

pub(super) fn turtle_should_delay_tech_for_entrenchment(
    profile: &AiProfile,
    memory: &AiDecisionMemory,
    facts: &AiFacts,
    kind: EntityKind,
) -> bool {
    if profile.turtle_defense.is_none() {
        return false;
    }
    if matches!(kind, EntityKind::Barracks | EntityKind::TrainingCentre) {
        return false;
    }
    if facts.complete_building_count(EntityKind::TrainingCentre) == 0 {
        return true;
    }
    if !turtle_entrenchment_started_or_done(memory, facts) {
        return true;
    }
    false
}

pub(super) fn turtle_barracks_target(
    profile: &AiProfile,
    facts: &AiFacts,
    base_target: usize,
) -> usize {
    let Some(policy) = profile.turtle_defense else {
        return base_target;
    };
    if facts.complete_building_count(EntityKind::TrainingCentre) == 0 {
        return base_target.min(1);
    }
    base_target.max(policy.support_barracks_target)
}

pub(super) fn effective_unit_priorities_for_upgrades(
    profile: &AiProfile,
    unit_priorities: &[EntityKind],
    completed_upgrades: &[UpgradeKind],
) -> Vec<EntityKind> {
    if profile.fast_tank_timing.is_some() {
        return unit_priorities.to_vec();
    }
    let methamphetamines_ready = completed_upgrades.contains(&UpgradeKind::Methamphetamines);
    unit_priorities
        .iter()
        .copied()
        .filter(|unit| *unit != EntityKind::Tank || methamphetamines_ready)
        .collect()
}

pub(super) fn effective_unit_priorities_for_fast_tank_timing(
    profile: &AiProfile,
    facts: &AiFacts,
    unit_priorities: &[EntityKind],
) -> Vec<EntityKind> {
    let Some(timing) = profile.fast_tank_timing else {
        return unit_priorities.to_vec();
    };
    // Jeff's AI V1.1 (`ecafter`): its Factory comes before the Engineering Complex and so before
    // Tank Unlock, and its first unit is a Scout Car while no car is out and Tanks cannot be trained.
    let first_car = facts.v1_1_opening.is_some()
        && facts.unit_count(EntityKind::ScoutCar) == 0
        && !facts
            .completed_upgrades()
            .contains(&UpgradeKind::TankUnlock);
    let mut priorities: Vec<EntityKind> = unit_priorities
        .iter()
        .copied()
        .filter(|unit| {
            *unit != EntityKind::ScoutCar
                || first_car
                || facts.unit_count(EntityKind::Tank) >= timing.tanks_before_scout_car
        })
        .collect();
    if first_car
        || (facts.unit_count(EntityKind::Tank) >= timing.tanks_before_scout_car
            && facts.unit_count(EntityKind::ScoutCar) < timing.scout_car_target)
    {
        priorities.sort_by_key(|unit| (*unit != EntityKind::ScoutCar) as u8);
    }
    priorities
}

pub(super) fn effective_unit_priorities_for_turtle(
    profile: &AiProfile,
    memory: &AiDecisionMemory,
    facts: &AiFacts,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    unit_priorities: &[EntityKind],
) -> Vec<EntityKind> {
    let Some(policy) = profile.turtle_defense else {
        return unit_priorities.to_vec();
    };
    let opening_done = memory.turtle_opening_riflemen_ordered >= policy.opening_riflemen;
    let entrenchment_started_or_done = turtle_entrenchment_started_or_done(memory, facts);
    let machine_gunner_lines_staffed =
        turtle_machine_gunner_lines_staffed(observation, map_analysis, policy);
    unit_priorities
        .iter()
        .copied()
        .filter(|unit| match *unit {
            EntityKind::Rifleman => !opening_done,
            EntityKind::MachineGunner => {
                opening_done && entrenchment_started_or_done && !machine_gunner_lines_staffed
            }
            EntityKind::AntiTankGun => opening_done && entrenchment_started_or_done,
            _ => true,
        })
        .collect()
}

pub(super) fn turtle_entrenchment_started_or_done(
    memory: &AiDecisionMemory,
    facts: &AiFacts,
) -> bool {
    facts
        .completed_upgrades()
        .contains(&UpgradeKind::Entrenchment)
        || memory.pending_upgrades.contains(&UpgradeKind::Entrenchment)
}

pub(super) fn effective_unit_priorities_for_defensive_machine_gunners(
    profile: &AiProfile,
    facts: &AiFacts,
    unit_priorities: &[EntityKind],
) -> Vec<EntityKind> {
    let mut priorities = unit_priorities.to_vec();
    let Some(policy) = profile.defensive_machine_gunners else {
        return priorities;
    };
    if policy.target_count == 0 || facts.complete_building_count(EntityKind::TrainingCentre) == 0 {
        return priorities;
    }
    if priorities.contains(&EntityKind::MachineGunner) {
        return priorities;
    }
    let insert_at = priorities
        .iter()
        .position(|unit| *unit == EntityKind::Tank)
        .map(|index| index + 1)
        .unwrap_or(0);
    priorities.insert(insert_at, EntityKind::MachineGunner);
    priorities
}

pub(super) fn production_max_counts(
    profile: &AiProfile,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
) -> Vec<(EntityKind, usize)> {
    let mut counts = profile
        .defensive_machine_gunners
        .map(|policy| vec![(EntityKind::MachineGunner, policy.target_count)])
        .unwrap_or_default();
    if let Some(policy) = profile.turtle_defense {
        counts.push((EntityKind::Rifleman, policy.opening_riflemen));
        let target_chokes = map_analysis
            .map(|analysis| {
                analysis
                    .base_chokes_for_player(observation.player_id, policy.max_chokes)
                    .len()
                    .min(policy.machine_gunner_target_chokes)
            })
            .unwrap_or(policy.machine_gunner_target_chokes);
        counts.push((
            EntityKind::MachineGunner,
            target_chokes.saturating_mul(policy.machine_gunners_per_choke),
        ));
    }
    if let Some(timing) = profile.fast_tank_timing {
        counts.push((EntityKind::ScoutCar, timing.scout_car_target));
    }
    if let Some(policy) = profile.home_anti_tank {
        counts.push((EntityKind::AntiTankGun, policy.target_guns));
    }
    counts
}

pub(super) fn can_train_pre_tank_defensive_machine_gunner(
    profile: &AiProfile,
    facts: &AiFacts,
    building_kind: EntityKind,
) -> bool {
    if profile.defensive_machine_gunners.is_none() || building_kind != EntityKind::Barracks {
        return false;
    }
    let tank_production_available = !facts.production_buildings(EntityKind::Factory).is_empty()
        && facts
            .completed_upgrades()
            .contains(&UpgradeKind::TankUnlock)
        && facts
            .completed_upgrades()
            .contains(&UpgradeKind::Methamphetamines);
    !tank_production_available
}
