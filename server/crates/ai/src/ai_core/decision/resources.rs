use super::expansion::{active_expansion, expansion_blocks_tech_path};
use super::geometry::normalized_direction;
use super::policies::{active_resource_policy, active_worker_policy};
use super::*;
use crate::ai_core::resource_availability::ResourceAvailability;

pub(super) const EXPANSION_LOCAL_RESOURCE_ASSIGNMENT_RADIUS_TILES: f32 =
    config::MINING_CC_RANGE_TILES + 3.0;

// Starting steel is split across both sides of a base; AI staging still treats the map-center side
// as the exposed resource line that existed before the split.
pub(super) fn forward_steel_cluster_center<'a>(
    resources: impl IntoIterator<Item = &'a AiResourceSummary>,
    base_center: (f32, f32),
    map: AiMapSummary,
) -> Option<(f32, f32)> {
    let steel: Vec<&AiResourceSummary> = resources
        .into_iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
        .collect();
    if steel.is_empty() {
        return None;
    }

    let tile_size = map.tile_size as f32;
    if tile_size <= 0.0 {
        return average_resource_center(&steel);
    }
    let map_center = (
        map.width as f32 * tile_size * 0.5,
        map.height as f32 * tile_size * 0.5,
    );
    let Some((dir_x, dir_y)) = normalized_direction(base_center, map_center) else {
        return average_resource_center(&steel);
    };

    let forward: Vec<&AiResourceSummary> = steel
        .iter()
        .copied()
        .filter(|resource| {
            (resource.x - base_center.0) * dir_x + (resource.y - base_center.1) * dir_y > 0.0
        })
        .collect();
    if forward.is_empty() {
        average_resource_center(&steel)
    } else {
        average_resource_center(&forward)
    }
}

fn average_resource_center(resources: &[&AiResourceSummary]) -> Option<(f32, f32)> {
    let count = resources.len().min(config::STEEL_PATCHES_PER_BASE as usize);
    if count == 0 {
        return None;
    }
    let (sum_x, sum_y) = resources
        .iter()
        .take(count)
        .fold((0.0, 0.0), |(sum_x, sum_y), resource| {
            (sum_x + resource.x, sum_y + resource.y)
        });
    Some((sum_x / count as f32, sum_y / count as f32))
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct EconomyPlan {
    pub(super) target_steel_workers: usize,
    pub(super) desired_oil_workers: usize,
    pub(super) target_workers: usize,
    pub(super) current_steel_workers: usize,
    pub(super) current_oil_workers: usize,
    pub(super) resource_counts: BTreeMap<EntityKind, usize>,
    pub(super) occupied_nodes: BTreeSet<u32>,
    pub(super) mineable_steel_nodes: BTreeSet<u32>,
    pub(super) mineable_oil_nodes: BTreeSet<u32>,
    pub(super) max_worker_resource_distance_px: Option<f32>,
    pub(super) remote_worker_assignment_fallback: bool,
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
    let mut resource_counts = resource_worker_counts(observation);
    let oil_extractors = availability.live_completed_extractor_count(EntityKind::Oil);
    if oil_extractors > 0 {
        *resource_counts.entry(EntityKind::Oil).or_default() += oil_extractors;
    }
    let current_steel_workers = resource_counts
        .get(&EntityKind::Steel)
        .copied()
        .unwrap_or(0);
    let current_oil_workers = resource_counts.get(&EntityKind::Oil).copied().unwrap_or(0);
    let max_worker_resource_distance_px =
        max_worker_resource_assignment_distance_px(observation, facts, profile, recovery_active);
    EconomyPlan {
        target_steel_workers,
        desired_oil_workers,
        target_workers: target_steel_workers.saturating_add(desired_oil_workers),
        current_steel_workers,
        current_oil_workers,
        resource_counts,
        occupied_nodes: availability.occupied_node_ids(),
        mineable_steel_nodes: availability.free_mineable_node_ids(EntityKind::Steel),
        mineable_oil_nodes: availability.free_mineable_node_ids(EntityKind::Oil),
        max_worker_resource_distance_px,
        remote_worker_assignment_fallback: max_worker_resource_distance_px.is_some()
            && active_expansion(observation, profile, recovery_active)
                .map(|expansion| expansion.remote_worker_assignment_fallback)
                .unwrap_or(false),
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
    ResourceAvailability::from_observation(observation, &BTreeSet::new()).occupied_node_ids()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::facts::AiFacts;
    use crate::ai_core::observation::{
        AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiObservation, AiPlayerSummary,
        AiResourceSummary,
    };
    use crate::ai_core::profiles::TECH_TO_TANKS;

    fn entity(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x,
            y,
            hp: 100,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn pump_jack_at(id: u32, x: f32, y: f32, complete: bool) -> AiEntitySummary {
        let mut entity = entity(id, EntityKind::PumpJack, x, y);
        entity.is_complete = complete;
        entity.state = if complete {
            AiEntityState::Idle
        } else {
            AiEntityState::Construct
        };
        entity
    }

    fn oil(id: u32, x: f32, y: f32) -> AiResourceSummary {
        AiResourceSummary {
            id,
            kind: EntityKind::Oil,
            x,
            y,
            remaining: 1_000,
        }
    }

    fn observation(
        owned: Vec<AiEntitySummary>,
        resources: Vec<AiResourceSummary>,
    ) -> AiObservation {
        let tile_size = config::TILE_SIZE;
        AiObservation {
            player_id: 1,
            tick: 90,
            map: AiMapSummary {
                width: 64,
                height: 64,
                tile_size,
            },
            economy: AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 11,
                supply_cap: 20,
            },
            own_start_tile: (8, 8),
            players: vec![
                AiPlayerSummary {
                    id: 1,
                    team_id: 1,
                    start_tile: (8, 8),
                    is_ai: true,
                    is_alive: true,
                },
                AiPlayerSummary {
                    id: 2,
                    team_id: 2,
                    start_tile: (48, 48),
                    is_ai: false,
                    is_alive: true,
                },
            ],
            owned,
            resources,
            visible_allies: Vec::new(),
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        }
    }

    #[test]
    fn plan_counts_only_live_completed_pump_jacks_as_current_oil_workers() {
        let ts = config::TILE_SIZE as f32;
        let mut observation = observation(
            vec![
                entity(10, EntityKind::CityCentre, 8.5 * ts, 8.5 * ts),
                pump_jack_at(60, 10.5 * ts, 12.5 * ts, false),
            ],
            vec![
                oil(200, 10.5 * ts, 12.5 * ts),
                oil(201, 11.5 * ts, 12.5 * ts),
            ],
        );
        let facts = AiFacts::from_observation(&observation);

        let plan = plan_economy(&observation, &facts, &TECH_TO_TANKS, false, Some(3));

        assert_eq!(plan.desired_oil_workers, 3);
        assert_eq!(
            plan.current_oil_workers, 0,
            "in-progress Pump Jacks reserve oil patches but are not current oil income"
        );
        assert_eq!(plan.mineable_oil_nodes, BTreeSet::from([201]));

        let pump_jack = observation
            .owned
            .iter_mut()
            .find(|entity| entity.id == 60)
            .expect("pump jack should exist");
        pump_jack.is_complete = true;
        pump_jack.state = AiEntityState::Idle;
        let facts = AiFacts::from_observation(&observation);
        let plan = plan_economy(&observation, &facts, &TECH_TO_TANKS, false, Some(3));
        assert_eq!(plan.current_oil_workers, 1);

        observation
            .resources
            .iter_mut()
            .find(|resource| resource.id == 200)
            .expect("first oil should exist")
            .remaining = 0;
        let facts = AiFacts::from_observation(&observation);
        let plan = plan_economy(&observation, &facts, &TECH_TO_TANKS, false, Some(3));
        assert_eq!(
            plan.current_oil_workers, 0,
            "completed Pump Jacks on depleted oil must not satisfy current oil demand"
        );
        assert_eq!(plan.mineable_oil_nodes, BTreeSet::from([201]));
    }
}
