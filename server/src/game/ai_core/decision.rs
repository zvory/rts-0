#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use crate::config;
use crate::game::ai_core::actions::{
    self, AiActionContext, BuildPlacementRequest, ResourceAssignmentPolicy, SpendBudget,
    TrainUnitsRequest,
};
use crate::game::ai_core::facts::AiFacts;
use crate::game::ai_core::observation::AiObservation;
use crate::game::ai_core::profiles::AiProfile;
use crate::game::ai_shared;
use crate::game::entity::EntityKind;
use crate::protocol::Command;
use crate::rules;

const PRODUCTION_BUILDINGS: [EntityKind; 3] = [
    EntityKind::TankFactory,
    EntityKind::Barracks,
    EntityKind::IndustrialCenter,
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AiDecision {
    pub(crate) profile_id: &'static str,
    pub(crate) intents: Vec<AiIntent>,
    pub(crate) commands: Vec<Command>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AiIntent {
    Build {
        kind: EntityKind,
    },
    Train {
        kind: EntityKind,
    },
    Gather {
        resource: EntityKind,
        assignments: usize,
    },
    Stage {
        units: Vec<u32>,
    },
    Attack {
        units: Vec<u32>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AiDecisionMemory {
    profile_id: Option<&'static str>,
    next_attack_size: usize,
    last_attack_tick: Option<u32>,
}

impl AiDecisionMemory {
    pub(crate) fn for_profile(profile: &AiProfile) -> Self {
        Self {
            profile_id: Some(profile.id),
            next_attack_size: profile.attack.first_attack_size,
            last_attack_tick: None,
        }
    }

    pub(crate) fn desired_attack_size(&mut self, profile: &AiProfile, tick: u32) -> usize {
        self.ensure_profile(profile);
        if self
            .last_attack_tick
            .map(|last| tick.saturating_sub(last) >= profile.attack.regroup_reset_ticks)
            .unwrap_or(false)
        {
            self.next_attack_size = profile.attack.first_attack_size;
        }
        self.next_attack_size
    }

    fn note_attack(&mut self, profile: &AiProfile, tick: u32) {
        self.ensure_profile(profile);
        self.last_attack_tick = Some(tick);
        self.next_attack_size = self
            .next_attack_size
            .saturating_add(profile.attack.wave_growth);
    }

    fn attack_due(&self, profile: &AiProfile, tick: u32) -> bool {
        self.last_attack_tick
            .map(|last| tick.saturating_sub(last) >= profile.attack.reissue_cadence_ticks)
            .unwrap_or(true)
    }

    fn ensure_profile(&mut self, profile: &AiProfile) {
        if self.profile_id == Some(profile.id) && self.next_attack_size != 0 {
            return;
        }
        self.profile_id = Some(profile.id);
        self.next_attack_size = profile.attack.first_attack_size;
        self.last_attack_tick = None;
    }
}

pub(crate) fn decide_profile<F>(
    observation: &AiObservation,
    profile: &'static AiProfile,
    memory: &mut AiDecisionMemory,
    build_search: ai_shared::BuildSearch,
    mut placeable: F,
) -> AiDecision
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    memory.ensure_profile(profile);

    let facts = AiFacts::from_observation(observation);
    let budget = SpendBudget::with_committed_steel(
        observation.economy.steel,
        observation.economy.oil,
        observation.economy.supply_used,
        observation.economy.supply_cap,
        facts.committed_steel,
    );
    let mut actions = AiActionContext::new(&facts, budget);
    let mut intents = Vec::new();

    let mut idle_builders = facts.idle_workers.clone();
    let mut gathering_builders = facts.gathering_workers.clone();
    idle_builders.sort_unstable();
    gathering_builders.sort_unstable();
    let builder_pools = [idle_builders.as_slice(), gathering_builders.as_slice()];
    let save_for_required_tech_building = should_save_for_required_tech_building(&facts, profile);

    if wants_depot(&facts, profile)
        && (!save_for_required_tech_building
            || facts.free_supply <= profile.supply.emergency_depot_threshold)
        && try_build_kind(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Depot,
            build_search,
            &mut placeable,
        )
        .is_some()
    {
        intents.push(AiIntent::Build {
            kind: EntityKind::Depot,
        });
    }

    for kind in profile.buildings.required_tech_path {
        if facts.building_count(*kind) + planned_in_intents(&intents, *kind) > 0 {
            continue;
        }
        if try_build_kind(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            *kind,
            build_search,
            &mut placeable,
        )
        .is_some()
        {
            intents.push(AiIntent::Build { kind: *kind });
        }
    }

    let complete_gate_count = profile
        .workers
        .pressure_until_complete
        .map(|kind| facts.complete_building_count(kind))
        .unwrap_or(usize::MAX);
    let target_steel_workers = profile
        .workers
        .target_steel_workers(facts.target_steel_workers, complete_gate_count);
    let target_barracks = profile.buildings.barracks_curve.target(
        observation.economy.steel,
        facts.worker_count,
        target_steel_workers,
    );
    if facts.building_count(EntityKind::Barracks)
        + planned_in_intents(&intents, EntityKind::Barracks)
        < target_barracks
        && planned_in_intents(&intents, EntityKind::Barracks) == 0
        && try_build_kind(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Barracks,
            build_search,
            &mut placeable,
        )
        .is_some()
    {
        intents.push(AiIntent::Build {
            kind: EntityKind::Barracks,
        });
    }

    let desired_oil_workers =
        desired_oil_workers(observation, &facts, profile, target_steel_workers);
    let target_workers = target_steel_workers.saturating_add(desired_oil_workers);
    let save_for_first_tech_unit = should_save_for_first_tech_unit(&facts, profile);
    let save_for_tech = save_for_first_tech_unit || save_for_required_tech_building;
    for trained in actions::train_units(
        &mut actions,
        TrainUnitsRequest {
            buildings: facts.production_buildings(EntityKind::IndustrialCenter),
            unit_priorities: &[EntityKind::Worker],
            max_queue_depth: 1,
            save_for_tech,
            current_counts: &[(EntityKind::Worker, facts.worker_count)],
            max_counts: &[(EntityKind::Worker, target_workers)],
        },
    ) {
        intents.push(AiIntent::Train { kind: trained.unit });
    }

    for building_kind in production_building_order(profile.production.unit_priorities) {
        let buildings = facts.production_buildings(building_kind);
        if buildings.is_empty() {
            continue;
        }
        let key_tech_unit = profile
            .production
            .save_for_first_tech_unit
            .unwrap_or(EntityKind::Worker);
        let save_for_tech = (save_for_first_tech_unit || save_for_required_tech_building)
            && !rules::economy::trainable_units(building_kind).contains(&key_tech_unit);
        for trained in actions::train_units(
            &mut actions,
            TrainUnitsRequest {
                buildings,
                unit_priorities: profile.production.unit_priorities,
                max_queue_depth: profile.production.queue_depth,
                save_for_tech,
                current_counts: &[],
                max_counts: &[],
            },
        ) {
            intents.push(AiIntent::Train { kind: trained.unit });
        }
    }

    let occupied_nodes = occupied_resource_nodes(observation);
    let skipped_workers = BTreeSet::new();
    let resource_counts = resource_worker_counts(observation);
    let current_oil_workers = resource_counts.get(&EntityKind::Oil).copied().unwrap_or(0);
    if desired_oil_workers > current_oil_workers {
        let assigned = actions::assign_workers_to_resource(
            &mut actions,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Oil,
                candidate_worker_ids: Some(&facts.idle_workers),
                skip_workers: &skipped_workers,
                pre_reserved_nodes: &occupied_nodes,
                idle_only: true,
                max_assignments: Some(desired_oil_workers - current_oil_workers),
            },
        );
        if !assigned.is_empty() {
            intents.push(AiIntent::Gather {
                resource: EntityKind::Oil,
                assignments: assigned.len(),
            });
        }
    }

    let current_steel_workers = resource_counts
        .get(&EntityKind::Steel)
        .copied()
        .unwrap_or(0);
    if target_steel_workers > current_steel_workers {
        let assigned = actions::assign_workers_to_resource(
            &mut actions,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Steel,
                candidate_worker_ids: Some(&facts.idle_workers),
                skip_workers: &skipped_workers,
                pre_reserved_nodes: &occupied_nodes,
                idle_only: true,
                max_assignments: Some(target_steel_workers - current_steel_workers),
            },
        );
        if !assigned.is_empty() {
            intents.push(AiIntent::Gather {
                resource: EntityKind::Steel,
                assignments: assigned.len(),
            });
        }
    }

    if let Some(enemy_base) = facts.nearest_public_enemy_base {
        let ready_units =
            actions::select_ready_combat_units(&observation.owned, profile.attack.unit_kinds);
        let required_unit_ready = profile
            .attack
            .required_unit
            .map(|kind| {
                observation
                    .owned
                    .iter()
                    .any(|entity| entity.kind == kind && ready_units.contains(&entity.id))
            })
            .unwrap_or(true);
        let attack_size = memory.desired_attack_size(profile, observation.tick);
        if required_unit_ready
            && ready_units.len() >= attack_size
            && memory.attack_due(profile, observation.tick)
        {
            if let Some(units) =
                actions::attack_move_units(&mut actions, ready_units, enemy_base.x, enemy_base.y)
            {
                memory.note_attack(profile, observation.tick);
                intents.push(AiIntent::Attack { units });
            }
        } else if !ready_units.is_empty() {
            let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
            if let Some(units) = actions::stage_units_toward(
                &mut actions,
                ready_units,
                own_base,
                (enemy_base.x, enemy_base.y),
                observation.map.tile_size,
                profile.attack.stage_distance_tiles,
            ) {
                intents.push(AiIntent::Stage { units });
            }
        }
    }

    AiDecision {
        profile_id: profile.id,
        intents,
        commands: actions.into_commands(),
    }
}

fn wants_depot(facts: &AiFacts, profile: &AiProfile) -> bool {
    !facts.supply_capped
        && !facts.depot_in_progress
        && facts.free_supply <= profile.supply.free_supply_buffer
        && (facts.free_supply <= profile.supply.emergency_depot_threshold
            || !facts.production_buildings(EntityKind::Barracks).is_empty())
}

#[allow(clippy::too_many_arguments)]
fn try_build_kind<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    actions: &mut AiActionContext<'_>,
    builder_pools: &[&[u32]],
    profile: &AiProfile,
    kind: EntityKind,
    build_search: ai_shared::BuildSearch,
    placeable: &mut F,
) -> Option<actions::BuildAction>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    config::building_stats(kind)?;
    if !rules::economy::build_requirement_met(kind, facts.complete_building_kinds()) {
        return None;
    }
    let counts = facts.building_counts(kind);
    if counts.incomplete + counts.intended >= profile.buildings.max_pending_per_kind {
        return None;
    }
    let empty = BTreeSet::new();
    actions::try_build(
        actions,
        builder_pools,
        BuildPlacementRequest {
            building: kind,
            map_width: observation.map.width,
            map_height: observation.map.height,
            start_tile: observation.own_start_tile,
            search: build_search,
            skip_tiles: &empty,
            placeable: |tx, ty| placeable(kind, tx, ty),
        },
    )
}

fn desired_oil_workers(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    target_steel_workers: usize,
) -> usize {
    if profile.workers.extra_oil_workers == 0 {
        return 0;
    }
    let current_steel_workers = resource_worker_counts(observation)
        .get(&EntityKind::Steel)
        .copied()
        .unwrap_or(0);
    if facts.worker_count >= profile.resources.oil_after_steel_workers
        || current_steel_workers
            >= target_steel_workers.min(profile.resources.oil_after_steel_workers)
    {
        profile.workers.extra_oil_workers
    } else {
        0
    }
}

fn should_save_for_first_tech_unit(facts: &AiFacts, profile: &AiProfile) -> bool {
    let Some(unit) = profile.production.save_for_first_tech_unit else {
        return false;
    };
    if facts.unit_count(unit) > 0 {
        return false;
    }
    let Some(producer) = producer_for_unit(unit) else {
        return false;
    };
    facts.building_count(producer) > 0
}

fn should_save_for_required_tech_building(facts: &AiFacts, profile: &AiProfile) -> bool {
    let Some(unit) = profile.production.save_for_first_tech_unit else {
        return false;
    };
    let Some(producer) = producer_for_unit(unit) else {
        return false;
    };
    facts.building_count(producer) == 0
        && profile.buildings.required_tech_path.contains(&producer)
        && rules::economy::build_requirement_met(producer, facts.complete_building_kinds())
}

fn producer_for_unit(unit: EntityKind) -> Option<EntityKind> {
    PRODUCTION_BUILDINGS
        .into_iter()
        .find(|building| rules::economy::trainable_units(*building).contains(&unit))
}

fn production_building_order(unit_priorities: &[EntityKind]) -> Vec<EntityKind> {
    let mut order = Vec::new();
    for unit in unit_priorities {
        if let Some(building) = producer_for_unit(*unit) {
            if !order.contains(&building) {
                order.push(building);
            }
        }
    }
    order.retain(|kind| *kind != EntityKind::IndustrialCenter);
    order
}

fn resource_worker_counts(observation: &AiObservation) -> BTreeMap<EntityKind, usize> {
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

fn occupied_resource_nodes(observation: &AiObservation) -> BTreeSet<u32> {
    observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Worker)
        .filter_map(|worker| worker.latched_node)
        .collect()
}

fn planned_in_intents(intents: &[AiIntent], kind: EntityKind) -> usize {
    intents
        .iter()
        .filter(|intent| matches!(intent, AiIntent::Build { kind: built } if *built == kind))
        .count()
}

fn tile_center(tile: (u32, u32), tile_size: u32) -> (f32, f32) {
    (
        tile.0 as f32 * tile_size as f32 + tile_size as f32 * 0.5,
        tile.1 as f32 * tile_size as f32 + tile_size as f32 * 0.5,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ai_core::observation::{
        AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiPlayerSummary, AiResourceSummary,
    };
    use crate::game::ai_core::profiles::{
        RIFLE_FLOOD_FAST, RIFLE_FLOOD_FULL_SATURATION, TECH_TO_TANKS,
    };

    fn worker(id: u32, state: AiEntityState) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind: EntityKind::Worker,
            x: id as f32,
            y: 0.0,
            state,
            is_complete: true,
            production_queue_len: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn resource(id: u32, kind: EntityKind, x: f32, y: f32) -> AiResourceSummary {
        AiResourceSummary {
            id,
            kind,
            x,
            y,
            remaining: 1_000,
        }
    }

    fn building(id: u32, kind: EntityKind, queue_len: Option<usize>) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x: 0.0,
            y: 0.0,
            state: queue_len
                .filter(|queue| *queue > 0)
                .map(|_| AiEntityState::Train)
                .unwrap_or(AiEntityState::Idle),
            is_complete: true,
            production_queue_len: queue_len,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn combat(id: u32, kind: EntityKind) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x: 0.0,
            y: 0.0,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            latched_node: None,
            target_id: None,
            free_for_combat: true,
        }
    }

    fn observation(economy: AiEconomy, owned: Vec<AiEntitySummary>) -> AiObservation {
        let tile_size = config::TILE_SIZE;
        let mut resources = Vec::new();
        for i in 0..18 {
            resources.push(resource(
                100 + i,
                EntityKind::Steel,
                (8.5 + (i % 6) as f32) * tile_size as f32,
                (8.5 + (i / 6) as f32) * tile_size as f32,
            ));
        }
        for i in 0..3 {
            resources.push(resource(
                200 + i,
                EntityKind::Oil,
                (10.5 + i as f32) * tile_size as f32,
                12.5 * tile_size as f32,
            ));
        }
        AiObservation {
            player_id: 1,
            tick: 90,
            map: AiMapSummary {
                width: 64,
                height: 64,
                tile_size,
            },
            economy,
            own_start_tile: (8, 8),
            players: vec![
                AiPlayerSummary {
                    id: 1,
                    start_tile: (8, 8),
                    is_ai: true,
                    is_alive: true,
                },
                AiPlayerSummary {
                    id: 2,
                    start_tile: (48, 48),
                    is_ai: false,
                    is_alive: true,
                },
            ],
            owned,
            resources,
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
        }
    }

    fn decide(
        observation: &AiObservation,
        profile: &'static AiProfile,
        memory: &mut AiDecisionMemory,
    ) -> AiDecision {
        decide_profile(
            observation,
            profile,
            memory,
            ai_shared::BuildSearch {
                min_radius: 0,
                max_radius: 0,
                prefer_away_from_center: false,
            },
            |_, tx, ty| (tx, ty) == observation.own_start_tile,
        )
    }

    #[test]
    fn fast_flood_spends_first_fifty_steel_on_rifle_where_full_saturation_trains_worker() {
        let mut owned = vec![
            building(10, EntityKind::IndustrialCenter, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
        ];
        owned.extend((0..8).map(|i| worker(20 + i, AiEntityState::Gather)));
        let observation = observation(
            AiEconomy {
                steel: 50,
                oil: 0,
                supply_used: 8,
                supply_cap: 10,
            },
            owned,
        );

        let fast = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );
        let full = decide(
            &observation,
            &RIFLE_FLOOD_FULL_SATURATION,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
        );

        assert!(fast.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }));
        assert!(full.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }));
        assert!(!full.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }));
    }

    #[test]
    fn tech_to_tanks_requests_oil_workers_and_tank_factory_path() {
        let mut owned = vec![
            building(10, EntityKind::IndustrialCenter, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
        ];
        owned.extend((0..4).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 4,
                supply_cap: 20,
            },
            owned,
        );

        let decision = decide(
            &observation,
            &TECH_TO_TANKS,
            &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TankFactory
        }));
        assert!(
            !decision.intents.contains(&AiIntent::Train {
                kind: EntityKind::Worker
            }),
            "tech_to_tanks should save worker-training steel once the tank factory is buildable"
        );
        assert!(
            !decision.intents.contains(&AiIntent::Train {
                kind: EntityKind::Rifleman
            }),
            "tech_to_tanks should save barracks steel once the tank factory is buildable"
        );
        assert!(decision.intents.iter().any(|intent| {
            matches!(
                intent,
                AiIntent::Gather {
                    resource: EntityKind::Oil,
                    assignments
                } if *assignments > 0
            )
        }));
    }

    #[test]
    fn tech_to_tanks_trains_tank_before_spending_barracks_budget() {
        let observation = observation(
            AiEconomy {
                steel: 200,
                oil: 150,
                supply_used: 4,
                supply_cap: 20,
            },
            vec![
                building(10, EntityKind::IndustrialCenter, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building(13, EntityKind::TankFactory, Some(0)),
                worker(20, AiEntityState::Gather),
                worker(21, AiEntityState::Gather),
                worker(22, AiEntityState::Gather),
                worker(23, AiEntityState::Gather),
            ],
        );

        let decision = decide(
            &observation,
            &TECH_TO_TANKS,
            &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Tank
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }));
    }

    #[test]
    fn attack_memory_uses_profile_thresholds_and_growth() {
        let mut owned = Vec::new();
        owned.extend((0..3).map(|i| combat(30 + i, EntityKind::Rifleman)));
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 3,
                supply_cap: 10,
            },
            owned,
        );
        let mut fast_memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
        let mut full_memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION);

        let fast = decide(&observation, &RIFLE_FLOOD_FAST, &mut fast_memory);
        let full = decide(&observation, &RIFLE_FLOOD_FULL_SATURATION, &mut full_memory);

        assert!(fast.intents.iter().any(|intent| matches!(
            intent,
            AiIntent::Attack { units } if units.len() == 3
        )));
        assert!(full.intents.iter().any(|intent| matches!(
            intent,
            AiIntent::Stage { units } if units.len() == 3
        )));
        assert_eq!(fast_memory.desired_attack_size(&RIFLE_FLOOD_FAST, 91), 4);
    }

    #[test]
    fn each_required_profile_emits_a_starting_state_command() {
        let mut owned = vec![building(10, EntityKind::IndustrialCenter, Some(0))];
        owned.extend((0..4).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 4,
                supply_cap: 20,
            },
            owned,
        );

        for profile in crate::game::ai_core::profiles::required_profiles() {
            let decision = decide(
                &observation,
                profile,
                &mut AiDecisionMemory::for_profile(profile),
            );

            assert!(
                !decision.commands.is_empty(),
                "{} should emit at least one plausible opening command",
                profile.id
            );
        }
    }
}
