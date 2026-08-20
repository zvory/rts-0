#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::actions::{
    self, AiActionContext, BuildPlacementRequest, SpendBudget, TrainUnitsRequest,
};
use crate::ai_core::facts::{AiFacts, EnemyBaseFact};
use crate::ai_core::map_analysis::AiMapAnalysis;
use crate::ai_core::observation::{
    AiEntityState, AiEntitySummary, AiMapSummary, AiObservation, AiResourceSummary,
};
use crate::ai_core::profiles::{
    is_jeffs_ai_profile, AiProfile, AttackPolicy, BarracksCurve, ExpansionContainmentPolicy,
    ExpansionPolicy, ProductionPolicy, ResourcePolicy, TechTransitionPolicy, WorkerPolicy,
    JEFFS_AI_ID,
};
use crate::ai_shared;
use crate::config;
use rts_protocol::ObserverMapAnalysisLayer;
use rts_rules;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::{EntityKind, RallyKind};
use rts_sim::game::upgrade::{self, UpgradeKind};

mod defense;
mod economy_manager;
mod expansion;
mod frontal;
mod geometry;
mod memory;
mod policies;
mod production;
mod resources;
mod trace;
mod turtle;

use self::defense::{
    defensive_machine_gunner_units, defensive_machine_gunner_units_for_build_clearance,
    defensive_panic_barracks_target, defensive_panic_plan, defensive_panic_response,
    home_defensive_tank_is_positioned, local_defense_target, local_defense_units,
    machine_gunner_meets_replacement_health, stage_defensive_machine_gunner_perimeter,
    stage_home_anti_tank_line, stage_home_defensive_tank, stage_home_machine_gunner_screen,
    stage_home_rifleman_screen, stage_main_steel_defensive_line, DefensivePanicPlan,
    DefensivePanicResponse, ALL_COMBAT_UNITS, DEFENSIVE_PANIC_RIFLE_TECH_PATH,
};
use self::economy_manager::{
    propose_economy, EconomyManagerInput, EconomyManagerOutput, EconomyManagerSignals,
    EconomyProposal, OilDemandSignal,
};
use self::expansion::{plan_expansion, try_build_expansion_resource_depot, ExpansionBlocker};
use self::frontal::{issue_frontal_wave, plan_frontal_wave, sync_containment_recovery};
use self::geometry::{
    clamp_to_map, footprint_top_left_for_center, normalized_direction, tile_center,
};
pub(crate) use self::memory::AiDecisionMemory;
use self::policies::{
    active_attack_policy, active_barracks_curve, active_production_policy,
    active_required_tech_path, active_tech_transition,
};
use self::production::{
    producer_for_unit, production_building_order, production_uses_building,
    relocate_machine_gunners_blocking_factory, relocate_machine_gunners_from_factory_site,
    should_build_extra_factory, should_build_extra_turtle_gun_works,
    should_save_for_first_tech_unit, should_save_for_required_tech_building, try_build_kind,
    unit_counts_for_priorities,
};
use self::trace::{build_manager_trace, ManagerOutputTrace, TraceInput};
use self::turtle::{
    stage_turtle_choke_defense, turtle_machine_gunner_lines_staffed, turtle_observer_debug_layers,
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AiDecision {
    pub(crate) profile_id: &'static str,
    pub(crate) intents: Vec<AiIntent>,
    pub(crate) commands: Vec<Command>,
    pub(crate) trace: ManagerOutputTrace,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AiIntent {
    Move {
        units: Vec<u32>,
    },
    Build {
        kind: EntityKind,
    },
    ResumeConstruction {
        kind: EntityKind,
    },
    Train {
        kind: EntityKind,
    },
    Research {
        upgrade: UpgradeKind,
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

pub(crate) fn observer_debug_map_layers_for_profile(
    observation: &AiObservation,
    map_analysis: &AiMapAnalysis,
    profile: &'static AiProfile,
) -> Vec<ObserverMapAnalysisLayer> {
    let Some(policy) = profile.turtle_defense else {
        return Vec::new();
    };
    turtle_observer_debug_layers(observation, map_analysis, policy)
}

#[cfg(test)]
pub(crate) fn decide_profile_without_static_map_for_tests<F>(
    observation: &AiObservation,
    profile: &'static AiProfile,
    memory: &mut AiDecisionMemory,
    build_search: ai_shared::BuildSearch,
    mut placeable: F,
) -> AiDecision
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    decide_profile_inner(
        observation,
        profile,
        memory,
        None,
        build_search,
        &mut placeable,
    )
}

pub(crate) fn decide_profile_with_analysis<F>(
    observation: &AiObservation,
    profile: &'static AiProfile,
    memory: &mut AiDecisionMemory,
    map_analysis: &AiMapAnalysis,
    build_search: ai_shared::BuildSearch,
    mut placeable: F,
) -> AiDecision
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    decide_profile_inner(
        observation,
        profile,
        memory,
        Some(map_analysis),
        build_search,
        &mut placeable,
    )
}

fn decide_profile_inner<F>(
    observation: &AiObservation,
    profile: &'static AiProfile,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    build_search: ai_shared::BuildSearch,
    mut placeable: F,
) -> AiDecision
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    memory.ensure_profile(profile);
    memory.sync_incomplete_resource_depots(observation);
    memory
        .pending_upgrades
        .retain(|upgrade| !observation.upgrades.contains(upgrade));

    let facts = AiFacts::from_observation(observation);
    memory.sync_home_defensive_tank(observation, profile);
    memory.sync_turtle_opening(profile, observation);
    let budget = SpendBudget::with_committed_steel(
        observation.economy.steel,
        observation.economy.oil,
        observation.economy.supply_used,
        observation.economy.supply_cap,
        facts.committed_steel,
    );
    let start_budget = budget;
    let mut actions = AiActionContext::new(&facts, budget);
    let mut intents = Vec::new();

    let local_threat_response = defensive_panic_response(observation);
    let defensive_panic = memory.defensive_panic(local_threat_response, observation.tick);
    let panic_plan = defensive_panic
        .active
        .then(|| defensive_panic_plan(defensive_panic.response, &facts));
    let active_tech_transition = active_tech_transition(observation, profile);
    let required_tech_path = if defensive_panic.active && active_tech_transition.is_none() {
        panic_plan
            .map(|plan| plan.required_tech_path)
            .unwrap_or(&DEFENSIVE_PANIC_RIFLE_TECH_PATH)
    } else {
        active_required_tech_path(observation, profile)
    };
    let preserve_fast_tank_timing = profile
        .fast_tank_timing
        .is_some_and(|timing| timing.preserve_during_defensive_panic);
    let production_policy = if defensive_panic.active && !preserve_fast_tank_timing {
        panic_plan.map(|plan| plan.production).unwrap_or_else(|| {
            defensive_panic_plan(DefensivePanicResponse::Riflemen, &facts).production
        })
    } else {
        active_production_policy(observation, profile)
    };
    let attack_policy = active_attack_policy(observation, profile);
    let mut idle_builders = facts.idle_workers.clone();
    let mut gathering_builders = facts.gathering_workers.clone();
    idle_builders.sort_unstable();
    gathering_builders.sort_unstable();
    let builder_pools = [idle_builders.as_slice(), gathering_builders.as_slice()];
    if let Some((tile_x, tile_y)) = resource_depot_to_resume(observation, memory) {
        if actions::try_resume_construction_at(
            &mut actions,
            &builder_pools,
            EntityKind::ResourceDepot,
            tile_x,
            tile_y,
        )
        .is_some()
        {
            intents.push(AiIntent::ResumeConstruction {
                kind: EntityKind::ResourceDepot,
            });
        }
    }
    let save_for_required_tech_building =
        should_save_for_required_tech_building(&facts, required_tech_path, production_policy);
    let delay_opening_barracks = profile.fast_tank_timing.is_some_and(|timing| {
        facts.complete_building_count(EntityKind::Barracks) == 0
            && (facts.worker_count < timing.workers_before_barracks
                || facts.building_count(EntityKind::PumpJack) < timing.pump_jacks_before_barracks)
    });
    let preserve_fast_tank_economy = profile
        .fast_tank_timing
        .map(|timing| timing.preserve_during_defensive_panic)
        .unwrap_or(false);
    let defer_economy_for_panic = defensive_panic.active && !preserve_fast_tank_economy;
    let mut expansion_plan = plan_expansion(observation, &facts, profile, defer_economy_for_panic);
    let expansion_blocks_tech_path = expansion_plan.blocks_tech_path;
    let save_for_expansion = expansion_plan.should_save;
    let economy_manager_output = propose_economy(EconomyManagerInput {
        observation,
        facts: &facts,
        profile,
        expansion_plan: &expansion_plan,
        signals: EconomyManagerSignals {
            oil_demand: oil_demand_signal(profile, memory, panic_plan),
            defer_worker_training_for_tech: defer_economy_for_panic,
        },
    });

    if should_build_expansion_from_economy_manager(&economy_manager_output) {
        if try_build_expansion_resource_depot(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            &mut placeable,
        )
        .is_some()
        {
            intents.push(AiIntent::Build {
                kind: EntityKind::ResourceDepot,
            });
        } else if expansion_plan.blockers.is_empty() {
            expansion_plan.blockers.push(ExpansionBlocker::NoValidSite);
        }
    }
    let save_for_unplanned_expansion =
        save_for_expansion && planned_in_intents(&intents, EntityKind::ResourceDepot) == 0;

    let economy_plan = economy_manager_output.plan.clone();
    let save_worker_training_for_tech = defer_economy_for_panic;
    let should_train_workers = economy_manager_output.proposes(EconomyProposal::TrainWorker);
    if should_train_workers {
        for trained in actions::train_units(
            &mut actions,
            TrainUnitsRequest {
                buildings: facts.production_buildings(EntityKind::ResourceDepot),
                unit_priorities: &[EntityKind::Worker],
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: 1,
                save_for_tech: save_worker_training_for_tech,
                current_counts: &[(EntityKind::Worker, facts.worker_count)],
                max_counts: &[(EntityKind::Worker, economy_plan.target_workers)],
                balance_unit_priorities: false,
            },
        ) {
            intents.push(AiIntent::Train { kind: trained.unit });
        }
    }

    if profile.turtle_defense.is_some() {
        queue_profile_upgrades(&mut actions, &facts, memory, &mut intents, profile);
    }

    for kind in required_tech_path {
        if *kind == EntityKind::Barracks && delay_opening_barracks {
            continue;
        }
        if turtle_should_delay_tech_for_entrenchment(profile, memory, &facts, *kind) {
            continue;
        }
        if expansion_blocks_tech_path || save_for_unplanned_expansion {
            continue;
        }
        if facts.building_count(*kind) + planned_in_intents(&intents, *kind) > 0 {
            continue;
        }
        if let Some(build_action) = try_build_kind(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            *kind,
            build_search,
            &mut placeable,
        ) {
            if *kind == EntityKind::Factory {
                if let Some(enemy_base) = facts.nearest_public_enemy_base {
                    let defensive_machine_gunners =
                        defensive_machine_gunner_units_for_build_clearance(observation, profile);
                    if let Some(units) = relocate_machine_gunners_from_factory_site(
                        observation,
                        &mut actions,
                        (build_action.tile_x, build_action.tile_y),
                        &defensive_machine_gunners,
                        enemy_base,
                    ) {
                        // Clearing a construction footprint is a tactical move, not a new
                        // staging assignment. The live adapter suppresses repeated staging
                        // commands for units that are already in position.
                        intents.push(AiIntent::Move { units });
                    }
                }
            }
            intents.push(AiIntent::Build { kind: *kind });
        }
    }

    let target_barracks = if defensive_panic.active {
        defensive_panic_barracks_target(defensive_panic)
    } else {
        active_barracks_curve(profile).target(
            observation.economy.steel,
            facts.worker_count,
            economy_plan.target_steel_workers,
        )
    };
    let target_barracks = turtle_barracks_target(profile, &facts, target_barracks);
    let surplus_barracks_enabled = profile.surplus_steel_production.is_some_and(|policy| {
        let (barracks_steel, _) = rts_rules::economy::cost(EntityKind::Barracks);
        actions.budget().steel() >= policy.reserve.saturating_add(barracks_steel)
    });
    if (production_uses_building(production_policy, EntityKind::Barracks)
        || surplus_barracks_enabled)
        && !delay_opening_barracks
        && facts.building_count(EntityKind::Barracks)
            + planned_in_intents(&intents, EntityKind::Barracks)
            < target_barracks
        && !expansion_blocks_tech_path
        && !save_for_unplanned_expansion
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

    let first_factory_needed = production_uses_building(production_policy, EntityKind::Factory)
        && facts.building_count(EntityKind::Factory)
            + planned_in_intents(&intents, EntityKind::Factory)
            < profile.buildings.factory_target
        && !expansion_blocks_tech_path
        && !save_for_unplanned_expansion
        && planned_in_intents(&intents, EntityKind::Factory) == 0;
    if first_factory_needed {
        if let Some(build_action) = try_build_kind(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Factory,
            build_search,
            &mut placeable,
        ) {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                let defensive_machine_gunners =
                    defensive_machine_gunner_units_for_build_clearance(observation, profile);
                if let Some(units) = relocate_machine_gunners_from_factory_site(
                    observation,
                    &mut actions,
                    (build_action.tile_x, build_action.tile_y),
                    &defensive_machine_gunners,
                    enemy_base,
                ) {
                    intents.push(AiIntent::Move { units });
                }
            }
            intents.push(AiIntent::Build {
                kind: EntityKind::Factory,
            });
        } else if let Some(enemy_base) = facts.nearest_public_enemy_base {
            let defensive_machine_gunners =
                defensive_machine_gunner_units_for_build_clearance(observation, profile);
            if let Some(units) = relocate_machine_gunners_blocking_factory(
                observation,
                &mut actions,
                profile,
                build_search,
                &defensive_machine_gunners,
                enemy_base,
                &mut placeable,
            ) {
                intents.push(AiIntent::Move { units });
            }
        }
    }

    if !expansion_blocks_tech_path
        && !save_for_unplanned_expansion
        && planned_in_intents(&intents, EntityKind::Steelworks) == 0
        && should_build_extra_turtle_gun_works(
            observation,
            &facts,
            profile,
            planned_in_intents(&intents, EntityKind::Steelworks),
        )
        && try_build_kind(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Steelworks,
            build_search,
            &mut placeable,
        )
        .is_some()
    {
        intents.push(AiIntent::Build {
            kind: EntityKind::Steelworks,
        });
    }

    let home_defensive_tank_ready = memory
        .home_defensive_tank
        .zip(facts.nearest_public_enemy_base)
        .is_some_and(|(tank_id, enemy_base)| {
            let distance = profile
                .defensive_machine_gunners
                .map(|policy| policy.perimeter_distance_tiles)
                .unwrap_or(6.0);
            home_defensive_tank_is_positioned(
                observation,
                tank_id,
                enemy_base,
                distance,
                map_analysis,
            )
        });
    if profile.home_anti_tank.is_some()
        && home_defensive_tank_ready
        && facts.building_count(EntityKind::Steelworks)
            + planned_in_intents(&intents, EntityKind::Steelworks)
            == 0
        && !save_for_unplanned_expansion
        && try_build_kind(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Steelworks,
            build_search,
            &mut placeable,
        )
        .is_some()
    {
        intents.push(AiIntent::Build {
            kind: EntityKind::Steelworks,
        });
    }

    if !defensive_panic.active
        && !expansion_blocks_tech_path
        && !save_for_unplanned_expansion
        && planned_in_intents(&intents, EntityKind::Factory) == 0
        && should_build_extra_factory(
            observation,
            &facts,
            profile,
            planned_in_intents(&intents, EntityKind::Factory),
        )
        && try_build_kind(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Factory,
            build_search,
            &mut placeable,
        )
        .is_some()
    {
        intents.push(AiIntent::Build {
            kind: EntityKind::Factory,
        });
    }

    let save_for_first_tech_unit = should_save_for_first_tech_unit(&facts, production_policy);
    let tank_methamphetamines_pending = profile.fast_tank_timing.is_none()
        && production_policy
            .unit_priorities
            .contains(&EntityKind::Tank)
        && !facts
            .completed_upgrades()
            .contains(&UpgradeKind::Methamphetamines);
    if tank_methamphetamines_pending {
        queue_upgrade_if_available(
            &mut actions,
            &facts,
            memory,
            &mut intents,
            UpgradeKind::Methamphetamines,
        );
    }
    queue_jeff_infantry_mass_methamphetamines(&mut actions, &facts, memory, &mut intents, profile);
    if profile.turtle_defense.is_none() {
        queue_profile_upgrades(&mut actions, &facts, memory, &mut intents, profile);
    }
    queue_fast_tank_optional_upgrades(&mut actions, &facts, memory, &mut intents, profile);
    let defensive_tank_started = memory.home_defensive_tank.is_some()
        || (memory.containment_wave_launched
            && observation.owned.iter().any(|entity| {
                entity.kind == EntityKind::Factory
                    && entity.production_kind == Some(EntityKind::Tank)
            }));
    if profile.home_anti_tank.is_some() && defensive_tank_started {
        queue_upgrade_if_available(
            &mut actions,
            &facts,
            memory,
            &mut intents,
            UpgradeKind::AntiTankGunUnlock,
        );
    }
    let effective_unit_priorities = effective_unit_priorities_for_upgrades(
        profile,
        production_policy.unit_priorities,
        facts.completed_upgrades(),
    );
    let effective_unit_priorities =
        effective_unit_priorities_for_fast_tank_timing(profile, &facts, &effective_unit_priorities);
    let effective_unit_priorities = effective_unit_priorities_for_turtle(
        profile,
        memory,
        &facts,
        observation,
        map_analysis,
        &effective_unit_priorities,
    );
    let effective_unit_priorities = effective_unit_priorities_for_defensive_machine_gunners(
        profile,
        &facts,
        &effective_unit_priorities,
    );
    let mut effective_unit_priorities = effective_unit_priorities;
    if let Some(policy) = profile.surplus_steel_production {
        let (unit_steel, _) = rts_rules::economy::cost(policy.unit);
        if actions.budget().steel() >= policy.reserve.saturating_add(unit_steel)
            && !effective_unit_priorities.contains(&policy.unit)
        {
            effective_unit_priorities.push(policy.unit);
        }
    }
    if profile.home_anti_tank.is_some()
        && memory.containment_wave_launched
        && !effective_unit_priorities.contains(&EntityKind::AntiTankGun)
    {
        effective_unit_priorities.push(EntityKind::AntiTankGun);
    }
    queue_required_unit_unlocks(
        &mut actions,
        &facts,
        production_policy.unit_priorities,
        memory,
        &mut intents,
        profile,
    );
    let production_unit_counts =
        unit_counts_for_priorities(observation, &facts, profile, &effective_unit_priorities);
    let production_max_counts = production_max_counts(profile, observation, map_analysis);
    for building_kind in production_building_order(&effective_unit_priorities) {
        let buildings = facts.production_buildings(building_kind);
        if buildings.is_empty() {
            continue;
        }
        let key_tech_unit = production_policy
            .save_for_first_tech_unit
            .unwrap_or(EntityKind::Worker);
        let save_for_tech = (save_for_unplanned_expansion
            || (save_for_first_tech_unit && !planned_train_in_intents(&intents, key_tech_unit))
            || save_for_required_tech_building)
            && !rts_rules::economy::trainable_units(building_kind).contains(&key_tech_unit)
            && !can_train_pre_tank_defensive_machine_gunner(profile, &facts, building_kind);
        let mut building_max_counts = production_max_counts.clone();
        if let Some(policy) = profile
            .surplus_steel_production
            .filter(|policy| producer_for_unit(policy.unit) == Some(building_kind))
        {
            let current = production_unit_counts
                .iter()
                .find_map(|(kind, count)| (*kind == policy.unit).then_some(*count))
                .unwrap_or(0);
            let (unit_steel, _) = rts_rules::economy::cost(policy.unit);
            let affordable_above_reserve = if unit_steel == 0 {
                0
            } else {
                actions.budget().steel().saturating_sub(policy.reserve) as usize
                    / unit_steel as usize
            };
            building_max_counts.retain(|(kind, _)| *kind != policy.unit);
            building_max_counts.push((
                policy.unit,
                current.saturating_add(affordable_above_reserve),
            ));
        }
        let production_rally = is_jeffs_ai_profile(profile.id)
            .then(|| jeffs_production_rally(observation, &facts))
            .flatten();
        let rifleman_rally = (profile.id == JEFFS_AI_ID)
            .then(|| jeffs_rifleman_home_rally(observation, &facts))
            .flatten();
        let trained_units = actions::train_units_with_rally_for_unit(
            &mut actions,
            TrainUnitsRequest {
                buildings,
                unit_priorities: &effective_unit_priorities,
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: production_policy.queue_depth,
                save_for_tech,
                current_counts: &production_unit_counts,
                max_counts: &building_max_counts,
                balance_unit_priorities: production_policy.balance_unit_priorities,
            },
            |unit| {
                if unit == EntityKind::Rifleman {
                    rifleman_rally
                        .map(|(x, y)| (x, y, RallyKind::Move))
                        .or_else(|| production_rally.map(|(x, y)| (x, y, RallyKind::AttackMove)))
                } else {
                    production_rally.map(|(x, y)| (x, y, RallyKind::AttackMove))
                }
            },
        );
        for trained in trained_units {
            memory.note_turtle_train(profile, trained.unit);
            intents.push(AiIntent::Train { kind: trained.unit });
        }
    }

    let defensive_machine_gunners = defensive_machine_gunner_units(observation, profile);
    let defensive_machine_gunner_units: BTreeSet<u32> =
        defensive_machine_gunners.iter().copied().collect();
    let mut frontal_exclusions = defensive_machine_gunner_units.clone();
    if let Some(tank_id) = memory.home_defensive_tank {
        frontal_exclusions.insert(tank_id);
    }
    sync_containment_recovery(observation, profile, memory);
    let frontal_wave = plan_frontal_wave(
        observation,
        attack_policy,
        memory,
        profile,
        &frontal_exclusions,
    );
    let ready_units_count = frontal_wave.ready_units.len();
    let attack_size = frontal_wave.desired_size;
    let attack_due = frontal_wave.attack_due;
    let mut local_ready_units =
        actions::select_ready_combat_units(&observation.owned, &ALL_COMBAT_UNITS);
    if profile.home_anti_tank.is_some() {
        local_ready_units.retain(|id| {
            Some(*id) != memory.home_defensive_tank
                && observation.owned.iter().any(|entity| {
                    entity.id == *id
                        && entity.kind != EntityKind::AntiTankGun
                        && (memory.home_defensive_tank.is_none()
                            || entity.kind != EntityKind::MachineGunner)
                })
        });
    }
    if !frontal_wave.ready_units.is_empty()
        || !local_ready_units.is_empty()
        || !defensive_machine_gunners.is_empty()
    {
        let mut handled_local_defense = false;
        let mut local_defense_assigned = BTreeSet::new();
        if profile.home_anti_tank.is_some() {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                if let Some(units) = stage_home_anti_tank_line(
                    &mut actions,
                    observation,
                    profile,
                    enemy_base,
                    map_analysis,
                ) {
                    intents.push(AiIntent::Stage { units });
                }
            }
        }
        let local_target = local_defense_target(observation);
        let jeff_layered_home_defense = local_target.is_some()
            && is_jeffs_ai_profile(profile.id)
            && profile.home_anti_tank.is_some();
        let mut local_defenders = local_ready_units.clone();
        local_defenders.extend(defensive_machine_gunners.iter().copied());
        local_defenders.sort_unstable();
        local_defenders.dedup();
        if jeff_layered_home_defense {
            let local_targets: Vec<u32> = defense::local_defense_targets(observation)
                .into_iter()
                .collect();
            let interceptors: Vec<u32> = local_defense_units(observation, &local_defenders)
                .into_iter()
                .filter(|id| {
                    observation.owned.iter().any(|unit| {
                        unit.id == *id
                            && matches!(
                                unit.kind,
                                EntityKind::Rifleman
                                    | EntityKind::MachineGunner
                                    | EntityKind::ScoutCar
                            )
                    })
                })
                .collect();
            for (index, unit_id) in interceptors.into_iter().enumerate() {
                let Some(target) = local_targets.get(index % local_targets.len().max(1)) else {
                    break;
                };
                if let Some(units) = actions::attack_units(&mut actions, [unit_id], *target) {
                    local_defense_assigned.extend(units.iter().copied());
                    intents.push(AiIntent::Attack { units });
                }
            }
        } else if let Some(target) = local_target {
            if let Some(units) = actions::attack_units(
                &mut actions,
                local_defense_units(observation, &local_defenders),
                target,
            ) {
                local_defense_assigned.extend(units.iter().copied());
                intents.push(AiIntent::Attack { units });
                handled_local_defense = true;
            }
        }
        // Preserve Jeff's layered firing line on contact. Automatic target
        // acquisition meets the raid without dog-piling every defender into
        // one Tank overpenetration lane.
        handled_local_defense |= jeff_layered_home_defense;

        let defensive_machine_gunners_available: Vec<u32> = defensive_machine_gunners
            .iter()
            .copied()
            .filter(|id| !local_defense_assigned.contains(id))
            .collect();
        let turtle_defense_active = profile.turtle_defense.is_some();

        if !handled_local_defense
            && is_jeffs_ai_profile(profile.id)
            && profile.home_anti_tank.is_some()
        {
            let riflemen: Vec<u32> = if profile.id == JEFFS_AI_ID {
                observation
                    .owned
                    .iter()
                    .filter(|unit| {
                        unit.kind == EntityKind::Rifleman
                            && unit.is_complete
                            && unit.hp > 0
                            && !local_defense_assigned.contains(&unit.id)
                    })
                    .map(|unit| unit.id)
                    .collect()
            } else {
                actions::select_ready_combat_units(&observation.owned, &[EntityKind::Rifleman])
                    .into_iter()
                    .filter(|id| !local_defense_assigned.contains(id))
                    .collect()
            };
            let own_base =
                geometry::tile_center(observation.own_start_tile, observation.map.tile_size);
            let fallback_armor = observation
                .owned
                .iter()
                .filter(|entity| {
                    entity.is_complete
                        && matches!(entity.kind, EntityKind::Tank | EntityKind::ScoutCar)
                        && !memory.containment_active_tanks.contains(&entity.id)
                        && memory.containment_active_scout != Some(entity.id)
                })
                .min_by(|left, right| {
                    geometry::dist2(left.x, left.y, own_base.0, own_base.1)
                        .total_cmp(&geometry::dist2(right.x, right.y, own_base.0, own_base.1))
                })
                .map(|entity| entity.id);
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                let staged = if profile.id == JEFFS_AI_ID {
                    defense::stage_home_rifleman_coverage(
                        &mut actions,
                        observation,
                        map_analysis,
                        &riflemen,
                        enemy_base,
                    )
                } else {
                    memory
                        .home_defensive_tank
                        .or(fallback_armor)
                        .and_then(|armor_id| {
                            stage_home_rifleman_screen(
                                &mut actions,
                                observation,
                                &riflemen,
                                armor_id,
                                enemy_base,
                                3.0,
                                1.75,
                            )
                        })
                };
                if let Some(units) = staged {
                    intents.push(AiIntent::Stage { units });
                }
            }
        }

        if !handled_local_defense && turtle_defense_active {
            if let Some(policy) = profile.turtle_defense {
                if let Some(units) = stage_turtle_choke_defense(
                    &mut actions,
                    observation,
                    map_analysis,
                    policy,
                    &local_defense_assigned,
                ) {
                    intents.push(AiIntent::Stage { units });
                }
            }
        }

        if !handled_local_defense
            && !turtle_defense_active
            && !defensive_machine_gunners_available.is_empty()
        {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                let staged = if memory.home_defensive_tank.is_some() {
                    let distance = profile
                        .defensive_machine_gunners
                        .map(|policy| policy.perimeter_distance_tiles)
                        .unwrap_or(6.0)
                        + profile
                            .home_anti_tank
                            .map(|policy| policy.machine_gunner_screen_tiles)
                            .unwrap_or(0.0);
                    stage_home_machine_gunner_screen(
                        &mut actions,
                        observation,
                        map_analysis,
                        &defensive_machine_gunners_available,
                        enemy_base,
                        distance,
                        profile
                            .home_anti_tank
                            .map(|policy| policy.lateral_spacing_tiles)
                            .unwrap_or(4.5),
                    )
                } else {
                    stage_defensive_machine_gunner_perimeter(
                        &mut actions,
                        observation,
                        map_analysis,
                        profile,
                        &defensive_machine_gunners_available,
                        enemy_base,
                    )
                };
                if let Some(units) = staged {
                    intents.push(AiIntent::Stage { units });
                }
            }
        }

        if let Some(enemy_base) = facts.nearest_public_enemy_base {
            if let Some(tank_id) = memory.home_defensive_tank {
                let distance = profile
                    .defensive_machine_gunners
                    .map(|policy| policy.perimeter_distance_tiles)
                    .unwrap_or(6.0);
                if let Some(units) = stage_home_defensive_tank(
                    &mut actions,
                    observation,
                    tank_id,
                    enemy_base,
                    distance,
                    map_analysis,
                ) {
                    intents.push(AiIntent::Stage { units });
                }
            }
        }

        if !handled_local_defense && !turtle_defense_active && !frontal_wave.ready_units.is_empty()
        {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                if let Some(intent) = issue_frontal_wave(
                    &mut actions,
                    observation,
                    profile,
                    attack_policy,
                    &frontal_wave,
                    enemy_base,
                    memory,
                ) {
                    if let AiIntent::Attack { units } = &intent {
                        memory.note_attack_for(profile, attack_policy, observation.tick, units);
                    }
                    intents.push(intent);
                }
            }
        }
    }

    let trace = build_manager_trace(TraceInput {
        observation,
        profile,
        facts: &facts,
        intents: &intents,
        command_trace: actions.command_trace(),
        start_budget,
        end_budget: *actions.budget(),
        reservations: actions.reservations().counts(),
        save_for_expansion,
        expansion_blockers: &expansion_plan.blockers,
        expansion_blocks_tech_path,
        save_for_unplanned_expansion,
        save_for_required_tech_building,
        save_worker_training_for_tech,
        defensive_panic_active: defensive_panic.active,
        local_threat_active: local_threat_response.is_some(),
        ready_units: ready_units_count,
        attack_size,
        attack_due,
        frontal_wave_blockers: &frontal_wave.blockers,
        required_tech_path,
    });

    AiDecision {
        profile_id: profile.id,
        intents,
        commands: actions.into_commands(),
        trace,
    }
}

/// Jeff's producers send fresh combat units to a safe forward staging point immediately.
/// The normal frontal and defense planners remain authoritative and can redirect them on
/// the next think; this route only removes the idle interval at the production building.
fn jeffs_production_rally(observation: &AiObservation, facts: &AiFacts) -> Option<(f32, f32)> {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy_base = facts.nearest_public_enemy_base?;
    let direction = normalized_direction(own_base, (enemy_base.x, enemy_base.y))?;
    let forward_distance = observation.map.tile_size as f32 * 8.0;
    Some(clamp_to_map(
        (
            own_base.0 + direction.0 * forward_distance,
            own_base.1 + direction.1 * forward_distance,
        ),
        observation.map,
    ))
}

/// Riflemen are permanent home-screen units. Rally them to the base-centric defensive anchor so
/// they do not walk through the forward army staging lane before receiving a stable slot.
fn jeffs_rifleman_home_rally(observation: &AiObservation, facts: &AiFacts) -> Option<(f32, f32)> {
    let anchor = defense::main_steel_cluster_center(observation)
        .unwrap_or_else(|| tile_center(observation.own_start_tile, observation.map.tile_size));
    let enemy_base = facts.nearest_public_enemy_base?;
    let direction = normalized_direction(anchor, (enemy_base.x, enemy_base.y))?;
    let distance = observation.map.tile_size as f32 * 3.5;
    Some(clamp_to_map(
        (
            anchor.0 + direction.0 * distance,
            anchor.1 + direction.1 * distance,
        ),
        observation.map,
    ))
}

fn planned_in_intents(intents: &[AiIntent], kind: EntityKind) -> usize {
    intents
        .iter()
        .filter(|intent| matches!(intent, AiIntent::Build { kind: built } if *built == kind))
        .count()
}

fn planned_train_in_intents(intents: &[AiIntent], kind: EntityKind) -> bool {
    intents
        .iter()
        .any(|intent| matches!(intent, AiIntent::Train { kind: trained } if *trained == kind))
}

fn resource_depot_to_resume(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
) -> Option<(u32, u32)> {
    observation
        .owned
        .iter()
        .filter(|site| site.kind == EntityKind::ResourceDepot && !site.is_complete)
        .find_map(|site| {
            if !memory.resource_depot_is_safe_to_resume(site.id, observation.tick) {
                return None;
            }
            let (tile_x, tile_y) = resource_depot_site_tile(observation, site)?;
            if resource_depot_has_assigned_builder(observation, site.id, tile_x, tile_y) {
                None
            } else {
                Some((tile_x, tile_y))
            }
        })
}

fn resource_depot_has_assigned_builder(
    observation: &AiObservation,
    site_id: u32,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    observation.owned.iter().any(|entity| {
        entity.kind == EntityKind::Worker
            && entity.state == AiEntityState::Build
            && entity.target_id == Some(site_id)
    }) || observation.pending_builds.iter().any(|intent| {
        intent.kind == EntityKind::ResourceDepot
            && intent.tile_x == tile_x
            && intent.tile_y == tile_y
    })
}

fn resource_depot_site_tile(
    observation: &AiObservation,
    site: &AiEntitySummary,
) -> Option<(u32, u32)> {
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0
        || !site.x.is_finite()
        || !site.y.is_finite()
        || site.x < 0.0
        || site.y < 0.0
    {
        return None;
    }
    let center_tile = (
        (site.x / tile_size).floor() as u32,
        (site.y / tile_size).floor() as u32,
    );
    let (tile_x, tile_y) = footprint_top_left_for_center(center_tile, EntityKind::ResourceDepot)?;
    let stats = config::building_stats(EntityKind::ResourceDepot)?;
    (tile_x <= observation.map.width.saturating_sub(stats.foot_w)
        && tile_y <= observation.map.height.saturating_sub(stats.foot_h))
    .then_some((tile_x, tile_y))
}

fn turtle_opening_pending(profile: &AiProfile, memory: &AiDecisionMemory) -> bool {
    profile
        .turtle_defense
        .map(|policy| memory.turtle_opening_riflemen_ordered < policy.opening_riflemen)
        .unwrap_or(false)
}

fn oil_demand_signal(
    profile: &AiProfile,
    memory: &AiDecisionMemory,
    panic_plan: Option<DefensivePanicPlan>,
) -> OilDemandSignal {
    // Start one Pump Jack while Turtle is still assembling its compact Rifleman
    // screen. This preserves the screen without diverting the whole early worker
    // economy into oil before its Training Centre can use the income.
    if turtle_opening_pending(profile, memory) {
        return OilDemandSignal::ExactWorkers(1);
    }
    panic_plan
        .map(|plan| OilDemandSignal::ExactWorkers(plan.oil_workers))
        .unwrap_or(OilDemandSignal::ProfileDefault)
}

fn should_build_expansion_from_economy_manager(output: &EconomyManagerOutput) -> bool {
    output.proposes(EconomyProposal::BuildExpansionResourceDepot)
}

fn turtle_should_delay_tech_for_entrenchment(
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

fn turtle_barracks_target(profile: &AiProfile, facts: &AiFacts, base_target: usize) -> usize {
    let Some(policy) = profile.turtle_defense else {
        return base_target;
    };
    if facts.complete_building_count(EntityKind::TrainingCentre) == 0 {
        return base_target.min(1);
    }
    base_target.max(policy.support_barracks_target)
}

fn effective_unit_priorities_for_upgrades(
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

fn effective_unit_priorities_for_fast_tank_timing(
    profile: &AiProfile,
    facts: &AiFacts,
    unit_priorities: &[EntityKind],
) -> Vec<EntityKind> {
    let Some(timing) = profile.fast_tank_timing else {
        return unit_priorities.to_vec();
    };
    let mut priorities: Vec<EntityKind> = unit_priorities
        .iter()
        .copied()
        .filter(|unit| {
            *unit != EntityKind::ScoutCar
                || facts.unit_count(EntityKind::Tank) >= timing.tanks_before_scout_car
        })
        .collect();
    if facts.unit_count(EntityKind::Tank) >= timing.tanks_before_scout_car
        && facts.unit_count(EntityKind::ScoutCar) < timing.scout_car_target
    {
        priorities.sort_by_key(|unit| (*unit != EntityKind::ScoutCar) as u8);
    }
    priorities
}

fn effective_unit_priorities_for_turtle(
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

fn turtle_entrenchment_started_or_done(memory: &AiDecisionMemory, facts: &AiFacts) -> bool {
    facts
        .completed_upgrades()
        .contains(&UpgradeKind::Entrenchment)
        || memory.pending_upgrades.contains(&UpgradeKind::Entrenchment)
}

fn effective_unit_priorities_for_defensive_machine_gunners(
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

fn production_max_counts(
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

fn can_train_pre_tank_defensive_machine_gunner(
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

fn queue_upgrade_if_available(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    upgrade: UpgradeKind,
) {
    if facts.completed_upgrades().contains(&upgrade) || memory.pending_upgrades.contains(&upgrade) {
        return;
    }
    let definition = upgrade::definition(upgrade);
    if facts.complete_building_count(definition.researched_at) == 0 {
        return;
    }
    if let Some(researched) = actions::try_research_upgrade(
        actions,
        facts.production_buildings(definition.researched_at),
        upgrade,
    ) {
        memory.pending_upgrades.insert(researched.upgrade);
        intents.push(AiIntent::Research {
            upgrade: researched.upgrade,
        });
    }
}

fn queue_profile_upgrades(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile: &AiProfile,
) {
    for upgrade in profile.upgrade_priorities {
        if profile.fast_tank_timing.is_some()
            && *upgrade == UpgradeKind::TankUnlock
            && if is_jeffs_ai_profile(profile.id) {
                facts.building_counts(EntityKind::Factory).existing == 0
            } else {
                facts.building_count(EntityKind::Factory) == 0
            }
        {
            continue;
        }
        queue_upgrade_if_available(actions, facts, memory, intents, *upgrade);
    }
}

fn queue_fast_tank_optional_upgrades(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile: &AiProfile,
) {
    let Some(timing) = profile.fast_tank_timing else {
        return;
    };
    if facts.unit_count(EntityKind::Tank) < timing.tanks_before_optional_upgrades {
        return;
    }
    for upgrade in timing.optional_upgrades {
        queue_upgrade_if_available(actions, facts, memory, intents, *upgrade);
    }
}

fn queue_jeff_infantry_mass_methamphetamines(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile: &AiProfile,
) {
    if !is_jeffs_ai_profile(profile.id)
        || facts
            .unit_count(EntityKind::Rifleman)
            .saturating_add(facts.unit_count(EntityKind::MachineGunner))
            <= 15
    {
        return;
    }
    queue_upgrade_if_available(
        actions,
        facts,
        memory,
        intents,
        UpgradeKind::Methamphetamines,
    );
}

fn queue_required_unit_unlocks(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    unit_priorities: &[EntityKind],
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile: &AiProfile,
) {
    for unit in unit_priorities {
        let Some(upgrade) = upgrade::required_for_unit(*unit) else {
            continue;
        };
        if profile.fast_tank_timing.is_some()
            && upgrade == UpgradeKind::TankUnlock
            && if is_jeffs_ai_profile(profile.id) {
                facts.building_counts(EntityKind::Factory).existing == 0
            } else {
                facts.building_count(EntityKind::Factory) == 0
            }
        {
            continue;
        }
        queue_upgrade_if_available(actions, facts, memory, intents, upgrade);
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod vehicle_worker_tests;
