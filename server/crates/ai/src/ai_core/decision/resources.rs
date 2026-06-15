use super::expansion::{active_expansion, expansion_blocks_tech_path};
use super::policies::{active_resource_policy, active_worker_policy};
use super::*;
use crate::ai_core::resource_availability::ResourceAvailability;

pub(super) const EXPANSION_LOCAL_RESOURCE_ASSIGNMENT_RADIUS_TILES: f32 =
    config::MINING_CC_RANGE_TILES + 3.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct EconomyPlan {
    pub(super) target_steel_workers: usize,
    pub(super) desired_oil_workers: usize,
    pub(super) target_workers: usize,
    pub(super) current_steel_workers: usize,
    pub(super) current_oil_workers: usize,
    pub(super) resource_counts: BTreeMap<EntityKind, usize>,
    pub(super) occupied_nodes: BTreeSet<u32>,
    pub(super) max_worker_resource_distance_px: Option<f32>,
}

pub(super) fn plan_economy(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
    panic_oil_workers: Option<usize>,
) -> EconomyPlan {
    let worker_policy = active_worker_policy(profile, recovery_active);
    let availability = resource_availability(observation);
    let complete_gate_count = worker_policy
        .pressure_until_complete
        .map(|kind| facts.complete_building_count(kind))
        .unwrap_or(usize::MAX);
    let base_steel_target = worker_policy.target_steel_workers(
        availability.current_steel_saturation_target(),
        complete_gate_count,
    );
    let target_steel_workers = target_steel_workers_for_profile(
        observation,
        facts,
        profile,
        recovery_active,
        base_steel_target,
    );
    let desired_oil_workers = if availability.has_free_mineable_oil() {
        panic_oil_workers.unwrap_or_else(|| {
            desired_oil_workers(
                observation,
                facts,
                profile,
                recovery_active,
                target_steel_workers,
            )
        })
    } else {
        0
    };
    let resource_counts = resource_worker_counts(observation);
    let current_steel_workers = resource_counts
        .get(&EntityKind::Steel)
        .copied()
        .unwrap_or(0);
    let current_oil_workers = resource_counts.get(&EntityKind::Oil).copied().unwrap_or(0);
    EconomyPlan {
        target_steel_workers,
        desired_oil_workers,
        target_workers: target_steel_workers.saturating_add(desired_oil_workers),
        current_steel_workers,
        current_oil_workers,
        resource_counts,
        occupied_nodes: occupied_resource_nodes(observation),
        max_worker_resource_distance_px: max_worker_resource_assignment_distance_px(
            observation,
            facts,
            profile,
            recovery_active,
        ),
    }
}

pub(super) fn target_steel_workers_for_profile(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
    base_target: usize,
) -> usize {
    let Some(expansion) = active_expansion(observation, profile, recovery_active) else {
        return base_target;
    };
    if facts.complete_building_count(EntityKind::CityCentre) < expansion.target_city_centres {
        return base_target.min(expansion.pre_expansion_steel_worker_cap);
    }

    let expanded_target = base_target.max(completed_cc_steel_saturation_target(observation));
    expansion
        .post_expansion_steel_worker_cap
        .map(|cap| expanded_target.min(cap))
        .unwrap_or(expanded_target)
}

pub(super) fn completed_cc_steel_saturation_target(observation: &AiObservation) -> usize {
    resource_availability(observation).current_steel_saturation_target()
}

pub(super) fn max_worker_resource_assignment_distance_px(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
) -> Option<f32> {
    let expansion = active_expansion(observation, profile, recovery_active)?;
    if facts.complete_building_count(EntityKind::CityCentre) < expansion.target_city_centres {
        return None;
    }
    Some(EXPANSION_LOCAL_RESOURCE_ASSIGNMENT_RADIUS_TILES * observation.map.tile_size as f32)
}

pub(super) fn desired_oil_workers(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
    target_steel_workers: usize,
) -> usize {
    let worker_policy = active_worker_policy(profile, recovery_active);
    let resource_policy = active_resource_policy(profile, recovery_active);
    let availability = resource_availability(observation);

    if worker_policy.extra_oil_workers == 0 {
        return 0;
    }
    if !availability.has_free_mineable_oil() {
        return 0;
    }
    if expansion_blocks_tech_path(observation, facts, profile, recovery_active) {
        return 0;
    }
    let current_steel_workers = resource_worker_counts(observation)
        .get(&EntityKind::Steel)
        .copied()
        .unwrap_or(0);
    let expansion_oil_first = active_expansion(observation, profile, recovery_active)
        .filter(|e| e.oil_before_steel_in_expansion)
        .map(|e| facts.complete_building_count(EntityKind::CityCentre) >= e.target_city_centres)
        .unwrap_or(false);
    let oil_steel_floor = if expansion_oil_first {
        0
    } else if resource_policy.oil_after_full_steel_saturation {
        target_steel_workers
    } else {
        target_steel_workers.min(resource_policy.oil_after_steel_workers)
    };
    if current_steel_workers < oil_steel_floor {
        return 0;
    }

    let Some(policy) = resource_policy.tank_adaptive else {
        return worker_policy.extra_oil_workers;
    };

    let max_oil_workers = worker_policy.extra_oil_workers.min(policy.max_oil_workers);
    if max_oil_workers == 0 {
        return 0;
    }
    let tank_factories = facts.complete_building_count(EntityKind::Factory).max(1);
    let mut target = tank_factories
        .saturating_mul(policy.oil_workers_per_factory)
        .clamp(1, max_oil_workers);

    if let Some(goal) = next_tank_resource_goal(facts, profile) {
        let steel_deficit = goal.steel.saturating_sub(observation.economy.steel);
        let oil_deficit = goal.oil.saturating_sub(observation.economy.oil);
        if oil_deficit > steel_deficit / 2 {
            target = target
                .saturating_add(policy.deficit_response_workers)
                .min(max_oil_workers);
        } else if steel_deficit > oil_deficit.saturating_mul(2) {
            target = target.saturating_sub(policy.deficit_response_workers);
        } else if oil_deficit == 0 && observation.economy.oil >= goal.oil.saturating_mul(2) {
            target = target.saturating_sub(1);
        }
    }

    target.min(max_oil_workers)
}

fn resource_availability(observation: &AiObservation) -> ResourceAvailability {
    ResourceAvailability::from_observation(observation, &BTreeSet::new())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ResourceGoal {
    steel: u32,
    oil: u32,
}

pub(super) fn next_tank_resource_goal(
    facts: &AiFacts,
    profile: &AiProfile,
) -> Option<ResourceGoal> {
    if profile.production.save_for_first_tech_unit != Some(EntityKind::Tank) {
        return None;
    }
    let kind = if facts.complete_building_count(EntityKind::TrainingCentre) == 0 {
        EntityKind::TrainingCentre
    } else if facts.complete_building_count(EntityKind::ResearchComplex) == 0 {
        EntityKind::ResearchComplex
    } else if facts.complete_building_count(EntityKind::Factory) == 0 {
        EntityKind::Factory
    } else if facts.complete_building_count(EntityKind::Steelworks) == 0 {
        EntityKind::Steelworks
    } else {
        EntityKind::Tank
    };
    let (steel, oil) = rts_rules::economy::cost(kind);
    let scale = if kind == EntityKind::Tank {
        facts.complete_building_count(EntityKind::Factory).max(1) as u32
    } else {
        1
    };
    Some(ResourceGoal {
        steel: steel.saturating_mul(scale),
        oil: oil.saturating_mul(scale),
    })
}

pub(super) fn resource_worker_counts(observation: &AiObservation) -> BTreeMap<EntityKind, usize> {
    let resources_by_id: BTreeMap<u32, EntityKind> = observation
        .resources
        .iter()
        .map(|resource| (resource.id, resource.kind))
        .collect();
    let mut counts = BTreeMap::new();
    for worker in observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Worker)
    {
        let Some(node) = worker.latched_node else {
            continue;
        };
        let Some(kind) = resources_by_id.get(&node).copied() else {
            continue;
        };
        *counts.entry(kind).or_default() += 1;
    }
    counts
}

pub(super) fn occupied_resource_nodes(observation: &AiObservation) -> BTreeSet<u32> {
    observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Worker)
        .filter_map(|worker| worker.latched_node)
        .collect()
}
