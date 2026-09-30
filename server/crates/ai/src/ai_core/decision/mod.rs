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
    is_jeffs_ai_profile, uses_current_jeffs_ai_policy, AiProfile, AttackPolicy, BarracksCurve,
    ExpansionContainmentPolicy, ExpansionPolicy, ProductionPolicy, ResourcePolicy,
    TechTransitionPolicy, WorkerPolicy, JEFFS_AI_BETA_ID, JEFFS_AI_ID,
    JEFFS_AI_PRE_DEFENSE_ENVELOPE_ID, JEFFS_AI_PRE_OPENING_RUSH_ID, JEFFS_AI_PRE_RIFLE_COVERAGE_ID,
    JEFFS_AI_PRE_TANK_CATCHUP_ID,
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
mod expansion_security;
mod frontal;
mod geometry;
mod home_armor;
mod jeff;
mod later_bases;
mod memory;
mod obstacles;
mod opening_rush;
mod policies;
mod production;
mod resources;
mod trace;
mod turtle;
mod unit_mix;
mod upgrades;
use self::unit_mix::*;
use self::upgrades::*;

#[cfg(test)]
use self::defense::select_defensive_interceptors;
use self::defense::{
    defensive_machine_gunner_units, defensive_machine_gunner_units_for_build_clearance,
    defensive_panic_barracks_target, defensive_panic_plan, defensive_panic_response,
    home_defensive_tank_is_positioned, local_defense_target, local_defense_units,
    machine_gunner_meets_replacement_health, stage_defensive_machine_gunner_perimeter,
    stage_defensive_pocket_machine_gunners, stage_defensive_tank_at, stage_home_anti_tank_line,
    stage_home_defensive_pocket_riflemen, stage_home_defensive_tank,
    stage_home_machine_gunner_screen, stage_home_rifleman_screen, stage_main_steel_defensive_line,
    DefensivePanicPlan, DefensivePanicResponse, ALL_COMBAT_UNITS, DEFENSIVE_PANIC_RIFLE_TECH_PATH,
};
use self::economy_manager::{
    propose_economy, EconomyManagerInput, EconomyManagerOutput, EconomyManagerSignals,
    EconomyProposal, OilDemandSignal,
};
use self::expansion::{
    plan_expansion, resource_depot_to_resume, try_build_expansion_resource_depot, ExpansionBlocker,
};
use self::frontal::{issue_frontal_wave, plan_frontal_wave, sync_containment_recovery};
use self::geometry::{clamp_to_map, normalized_direction, tile_center};
use self::jeff::{
    production_rally as jeffs_production_rally, rifleman_home_rally as jeffs_rifleman_home_rally,
    uses_current_jeff_defense, uses_home_rifle_coverage,
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
    try_build_production, unit_counts_for_priorities,
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
    /// Cohort repositioning owns its cadence and must supersede cached staging orders.
    Assemble {
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
    memory.sync_defender_posture(observation);
    memory.sync_incomplete_resource_depots(observation);
    memory
        .pending_upgrades
        .retain(|upgrade| !observation.upgrades.contains(upgrade));

    let facts = AiFacts::from_observation(observation);
    memory.sync_home_defensive_tank(observation, profile);
    if uses_current_jeffs_ai_policy(profile.id) {
        memory.note_enemy_tanks(observation);
    }
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

    let local_threat_response =
        defensive_panic_response(observation, uses_current_jeff_defense(profile.id));
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
    if uses_current_jeffs_ai_policy(profile.id) {
        home_armor::update_home_post(
            observation,
            memory,
            map_analysis,
            facts.nearest_public_enemy_base,
            attack_policy.stage_distance_tiles,
        );
    }
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
    // Jeff's natural Depot must also clear resource node bodies, which the shared placement query
    // does not check. Other profiles keep the shared answer unchanged.
    let jeff_resource_bodies = uses_current_jeffs_ai_policy(profile.id);
    let mut expansion_placeable = |building: EntityKind, tile_x: u32, tile_y: u32| {
        placeable(building, tile_x, tile_y)
            && !(jeff_resource_bodies
                && jeff::resource_body_blocks_site(observation, building, tile_x, tile_y))
    };
    expansion_security::prepare(
        observation,
        &facts,
        profile,
        memory,
        map_analysis,
        &mut expansion_placeable,
    );
    let expansion_footprint_blockers = if uses_current_jeffs_ai_policy(profile.id)
        && expansion_security::predicts_natural_from_opening(observation)
    {
        expansion_security::clear_reserved_footprint(observation, memory, &mut actions)
    } else {
        Vec::new()
    };
    let expansion_secured =
        expansion_security::update_and_stage(observation, map_analysis, memory, &mut actions);
    let reserve_expansion =
        expansion_security::reserve_expansion(observation, &facts, profile, memory);
    let expansion_blocks_tech_path = expansion_plan.blocks_tech_path;
    let save_for_expansion = expansion_plan.should_save;
    if reserve_expansion && !expansion_secured {
        expansion_plan
            .blockers
            .push(ExpansionBlocker::SiteNotSecured);
    }
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

    let retry_builder = memory
        .expansion_security
        .retry_builder()
        .into_iter()
        .collect::<Vec<_>>();
    let expansion_builder_pools = [
        retry_builder.as_slice(),
        idle_builders.as_slice(),
        gathering_builders.as_slice(),
    ];

    // Jeff owns the natural's timing: once it is next and the site is secured, it is ordered as soon
    // as the builder can go safely with the full cost banked, and a dropped order is retried after
    // a pause for the rest of the match, rather than waiting on the economy manager, whose Tank
    // requirements kept a failed natural from ever being ordered again.
    let jeff_natural = uses_current_jeffs_ai_policy(profile.id);
    let natural_ready = jeff_natural
        && expansion_security::expansion_is_next(observation, &facts, profile)
        && expansion_security::natural_attempt_ready(
            observation,
            map_analysis,
            memory,
            expansion_secured,
        );
    let natural_pending =
        jeff_natural && expansion_security::natural_order_pending(observation, memory);
    if (jeff_natural && natural_ready)
        || (!jeff_natural
            && (should_build_expansion_from_economy_manager(&economy_manager_output)
                || !retry_builder.is_empty()))
    {
        if let Some(build_action) = try_build_expansion_resource_depot(
            observation,
            &facts,
            &mut actions,
            &expansion_builder_pools,
            profile,
            memory.expansion_security.site,
            !retry_builder.is_empty(),
            &mut expansion_placeable,
        ) {
            // Track every secured-site order, not only the predicted-natural maps: without an
            // attempt record a rejected order is never retried and the reserve holds forever.
            if uses_current_jeffs_ai_policy(profile.id) {
                memory
                    .expansion_security
                    .note_build_attempt(observation.tick, build_action.worker);
            }
            intents.push(AiIntent::Build {
                kind: EntityKind::ResourceDepot,
            });
        } else if expansion_plan.blockers.is_empty() {
            expansion_plan.blockers.push(ExpansionBlocker::NoValidSite);
        }
    }
    let save_for_unplanned_expansion = (save_for_expansion || reserve_expansion)
        && planned_in_intents(&intents, EntityKind::ResourceDepot) == 0;
    if (reserve_expansion || natural_pending)
        && planned_in_intents(&intents, EntityKind::ResourceDepot) == 0
    {
        let (steel, oil) = rts_rules::economy::cost(EntityKind::ResourceDepot);
        actions.holdback_resources(steel, oil);
    }
    // Jeff's bases beyond the natural. A second Factory that is due takes priority this decision.
    let factory_due = uses_current_jeffs_ai_policy(profile.id)
        && should_build_extra_factory(
            observation,
            &facts,
            profile,
            planned_in_intents(&intents, EntityKind::Factory),
        );
    let later_base = later_bases::plan(
        observation,
        &facts,
        profile,
        memory,
        map_analysis,
        &mut actions,
        &builder_pools,
        factory_due,
        &mut expansion_placeable,
    );
    intents.extend(later_base.intents.iter().cloned());

    // The live Jeff's starting Riflemen march on the enemy until a fallback sends them home.
    if opening_rush::uses_opening_rush(profile.id) {
        let rush = opening_rush::plan(&mut actions, observation, &facts, memory, map_analysis);
        if !rush.moved.is_empty() {
            intents.push(AiIntent::Move { units: rush.moved });
        }
        if !rush.attacked.is_empty() {
            intents.push(AiIntent::Attack {
                units: rush.attacked,
            });
        }
        if !rush.released.is_empty() {
            // Clears the live adapter's cached staging so the pocket can place them again.
            intents.push(AiIntent::Assemble {
                units: rush.released,
            });
        }
    }

    // Jeff's picket on the enemy's route and warned sealing of the home line. Its units are
    // reserved from every other system for this decision.
    if uses_current_jeffs_ai_policy(profile.id) {
        let route_line = defense::plan_route_line(&mut actions, observation, memory, map_analysis);
        if !route_line.ordered.is_empty() {
            intents.push(AiIntent::Move {
                units: route_line.ordered,
            });
        }
        if !route_line.released.is_empty() {
            // Released sealers still carry the live adapter's cached staging; assembling clears it
            // so their normal posts are sent again.
            intents.push(AiIntent::Assemble {
                units: route_line.released,
            });
        }
    }
    // Units the route line or the opening rush own this decision.
    let route_line_reserved: BTreeSet<u32> = memory
        .route_line
        .reserved()
        .chain(memory.opening_rush.reserved())
        .collect();

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
        if let Some(build_action) = try_build_production(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            *kind,
            build_search,
            map_analysis,
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
        && try_build_production(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Barracks,
            build_search,
            map_analysis,
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
        if let Some(build_action) = try_build_production(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Factory,
            build_search,
            map_analysis,
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
    if profile
        .home_anti_tank
        .is_some_and(|policy| policy.target_guns > 0)
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
        && try_build_production(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            EntityKind::Factory,
            build_search,
            map_analysis,
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
    if uses_current_jeffs_ai_policy(profile.id)
        && memory.expansion_security.site.is_some()
        && facts.unit_count(EntityKind::Rifleman) < 6
    {
        effective_unit_priorities.insert(0, EntityKind::Rifleman);
    }
    if let Some(policy) = profile.surplus_steel_production {
        let (unit_steel, _) = rts_rules::economy::cost(policy.unit);
        if actions.budget().steel() >= policy.reserve.saturating_add(unit_steel)
            && !effective_unit_priorities.contains(&policy.unit)
        {
            effective_unit_priorities.push(policy.unit);
        }
    }
    if profile
        .home_anti_tank
        .is_some_and(|policy| policy.target_guns > 0)
        && memory.containment.wave_launched
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
        let security_recruits = uses_current_jeffs_ai_policy(profile.id)
            && memory.expansion_security.site.is_some()
            && facts.unit_count(EntityKind::Rifleman) < 6
            && building_kind == EntityKind::Barracks;
        let save_for_tech = !security_recruits
            && (save_for_unplanned_expansion
                || (save_for_first_tech_unit
                    && !planned_train_in_intents(&intents, key_tech_unit))
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
            // On Crossroads Jeff is short of Oil, not Steel: past a home garrison, more Riflemen
            // only spend the Steel the third base and Factory rebuilds need.
            let surplus_cap = if uses_current_jeffs_ai_policy(profile.id)
                && policy.unit == EntityKind::Rifleman
                && defense::crossroads_wall_aware_approach_direction(observation).is_some()
            {
                CROSSROADS_MAX_SURPLUS_RIFLEMEN
            } else {
                usize::MAX
            };
            building_max_counts.retain(|(kind, _)| *kind != policy.unit);
            building_max_counts.push((
                policy.unit,
                current
                    .saturating_add(affordable_above_reserve)
                    .min(surplus_cap)
                    .max(if security_recruits { 6 } else { 0 }),
            ));
        }
        let home_holds_tank_reserve = !uses_current_jeffs_ai_policy(profile.id)
            || later_bases::main_tank_ids(observation, memory).len() >= memory.home_tank_reserve();
        let production_rally = is_jeffs_ai_profile(profile.id)
            .then(|| jeffs_production_rally(observation, &facts))
            .flatten();
        let rifleman_rally = uses_home_rifle_coverage(profile.id)
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
                // While a new base is being taken, fresh Tanks and Riflemen join its guards, but
                // Tanks only once the main holds its reserve.
                if let Some((x, y)) = later_base.rally.filter(|_| {
                    unit == EntityKind::Rifleman
                        || (unit == EntityKind::Tank && home_holds_tank_reserve)
                }) {
                    return Some((x, y, RallyKind::AttackMove));
                }
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
    // The current Jeff's launched push keeps its units: home defense answers raids with what stayed
    // home, and the push keeps its own orders meanwhile. It used to lose all but two Tanks to
    // home defense within moments of leaving.
    let push_units: BTreeSet<u32> = if uses_current_jeffs_ai_policy(profile.id)
        && memory.containment.wave_launched
        && !memory.containment.recovery_active
    {
        memory
            .containment
            .active_tanks
            .iter()
            .copied()
            .chain(memory.containment.active_scout)
            .chain(memory.containment.active_riflemen.iter().copied())
            .collect()
    } else {
        BTreeSet::new()
    };
    let forward_tank_position = uses_current_jeffs_ai_policy(profile.id)
        .then(|| expansion_security::tank_staging_center(observation, map_analysis))
        .flatten();
    let forward_defensive_tank = forward_tank_position
        .and_then(|_| expansion_security::surplus_tank_for_forward_base(observation, memory));
    if let Some(tank_id) = forward_defensive_tank {
        frontal_exclusions.insert(tank_id);
    }
    frontal_exclusions.extend(memory.expansion_security.riflemen.iter().copied());
    frontal_exclusions.extend(expansion_footprint_blockers.iter().copied());
    // New-base guards stay out of the push until the base is covered.
    frontal_exclusions.extend(memory.later_bases.guards.iter().copied());
    frontal_exclusions.extend(route_line_reserved.iter().copied());
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
    local_ready_units.retain(|id| !expansion_footprint_blockers.contains(id));
    local_ready_units.retain(|id| !push_units.contains(id));
    local_ready_units.retain(|id| !memory.opening_rush.is_reserved(*id));
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
        let new_jeff_defense = uses_current_jeff_defense(profile.id);
        let mut jeff_layered_home_defense = local_target.is_some()
            && is_jeffs_ai_profile(profile.id)
            && profile.home_anti_tank.is_some();
        let mut local_defenders = local_ready_units.clone();
        local_defenders.extend(defensive_machine_gunners.iter().copied());
        if new_jeff_defense {
            local_defenders.extend(
                observation
                    .owned
                    .iter()
                    .filter(|unit| unit.is_complete && unit.hp > 0)
                    .filter(|unit| {
                        matches!(
                            unit.kind,
                            EntityKind::Rifleman
                                | EntityKind::MachineGunner
                                | EntityKind::ScoutCar
                                | EntityKind::Panzerfaust
                                | EntityKind::Tank
                        )
                    })
                    .map(|unit| unit.id),
            );
        }
        // The picket holds its trench on the route; it never runs back to answer a raid.
        local_defenders.retain(|id| Some(*id) != memory.route_line.picket());
        local_defenders.retain(|id| !push_units.contains(id));
        local_defenders.retain(|id| !memory.opening_rush.is_reserved(*id));
        local_defenders.sort_unstable();
        local_defenders.dedup();
        if new_jeff_defense {
            if let Some(units) = defense::respond_to_local_incident(
                &mut actions,
                observation,
                memory,
                &local_defenders,
                map_analysis,
                uses_current_jeffs_ai_policy(profile.id),
            ) {
                local_defense_assigned.extend(units.iter().copied());
                intents.push(AiIntent::Attack { units });
                jeff_layered_home_defense = true;
            }
            // Enemy Tanks in reach of a base outnumber the home Tanks that could answer them: a
            // small push comes home rather than lose the base behind it, and none leaves meanwhile.
            if uses_current_jeffs_ai_policy(profile.id) {
                memory.home_outgunned = defense::tank_siege(observation, &local_defenders)
                    .is_some_and(|siege| !siege.matched());
                if memory.home_outgunned {
                    if let Some(units) =
                        frontal::recall_small_push_home(&mut actions, observation, memory)
                    {
                        local_defense_assigned.extend(units.iter().copied());
                        intents.push(AiIntent::Move { units });
                    }
                }
            }
        } else if jeff_layered_home_defense {
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
        // one crowded firing position.
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
            let riflemen: Vec<u32> = if uses_home_rifle_coverage(profile.id) {
                observation
                    .owned
                    .iter()
                    .filter(|unit| {
                        unit.kind == EntityKind::Rifleman
                            && unit.is_complete
                            && unit.hp > 0
                            && !local_defense_assigned.contains(&unit.id)
                            && !memory.containment.active_riflemen.contains(&unit.id)
                            && !memory.expansion_security.riflemen.contains(&unit.id)
                            && !route_line_reserved.contains(&unit.id)
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
                        && !memory.containment.active_tanks.contains(&entity.id)
                        && memory.containment.active_scout != Some(entity.id)
                })
                .min_by(|left, right| {
                    geometry::dist2(left.x, left.y, own_base.0, own_base.1)
                        .total_cmp(&geometry::dist2(right.x, right.y, own_base.0, own_base.1))
                })
                .map(|entity| entity.id);
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                let staged = if uses_current_jeff_defense(profile.id) {
                    stage_home_defensive_pocket_riflemen(
                        &mut actions,
                        observation,
                        map_analysis,
                        &riflemen,
                        enemy_base,
                    )
                } else if profile.id == JEFFS_AI_PRE_DEFENSE_ENVELOPE_ID {
                    defense::stage_home_rifleman_coverage(
                        &mut actions,
                        observation,
                        map_analysis,
                        &riflemen,
                        enemy_base,
                    )
                } else if profile.id == JEFFS_AI_PRE_RIFLE_COVERAGE_ID {
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
                } else {
                    None
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
                let staged = if uses_current_jeff_defense(profile.id) {
                    stage_defensive_pocket_machine_gunners(
                        &mut actions,
                        observation,
                        map_analysis,
                        &defensive_machine_gunners_available,
                        enemy_base,
                    )
                } else if memory.home_defensive_tank.is_some() {
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

        // The current Jeff's home Tank waits behind the home post once that has moved off the
        // main's line; it sat beside the HQ all game while the natural was shelled.
        let jeff_home_post = memory
            .home_post
            .filter(|post| uses_current_jeffs_ai_policy(profile.id) && !post.on_main_line);
        if let Some((tank_id, post)) = memory.home_defensive_tank.zip(jeff_home_post) {
            if !local_defense_assigned.contains(&tank_id) {
                let point = post.home_tank_point(observation);
                if let Some(units) =
                    stage_defensive_tank_at(&mut actions, observation, tank_id, point)
                {
                    intents.push(AiIntent::Stage { units });
                }
            }
        } else if let Some(enemy_base) = facts.nearest_public_enemy_base {
            if let Some(tank_id) = memory.home_defensive_tank {
                let distance = profile
                    .defensive_machine_gunners
                    .map(|policy| policy.perimeter_distance_tiles)
                    .unwrap_or(6.0);
                let staged = stage_home_defensive_tank(
                    &mut actions,
                    observation,
                    tank_id,
                    enemy_base,
                    distance,
                    map_analysis,
                );
                if let Some(units) = staged {
                    intents.push(AiIntent::Stage { units });
                }
            }
        }

        if let Some((tank_id, position)) = forward_defensive_tank.zip(forward_tank_position) {
            if let Some(units) =
                stage_defensive_tank_at(&mut actions, observation, tank_id, position)
            {
                intents.push(AiIntent::Stage { units });
            }
        }

        let guard_units = later_bases::issue_guard_orders(
            &mut actions,
            observation,
            memory,
            &later_base.guard_posts,
            &local_defense_assigned,
        );
        if !guard_units.is_empty() {
            intents.push(AiIntent::Move { units: guard_units });
        }
        if uses_current_jeffs_ai_policy(profile.id) {
            let returning = later_bases::recall_tanks_to_main(
                &mut actions,
                observation,
                memory,
                &local_defense_assigned,
                forward_defensive_tank,
            );
            if !returning.is_empty() {
                intents.push(AiIntent::Move { units: returning });
            }
            if let Some(clearing) = obstacles::clear_route_traps(
                &mut actions,
                observation,
                memory,
                map_analysis,
                &local_defense_assigned,
            ) {
                intents.push(AiIntent::Attack { units: clearing });
            }
        }

        let containment_needs_control = profile.id != JEFFS_AI_BETA_ID
            && profile.expansion_containment.is_some()
            && frontal::containment_wave_needs_control(memory);
        let containment_recall_target = if defensive_panic.active && containment_needs_control {
            local_target
        } else {
            None
        };
        // During a raid at home only a launched push is still commanded here.
        let push_only = handled_local_defense && containment_recall_target.is_none();
        if (!push_only || !push_units.is_empty())
            && !turtle_defense_active
            && (!frontal_wave.ready_units.is_empty() || containment_needs_control)
        {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                let containment_was_launched = memory.containment.wave_launched;
                let wave_plan = if push_only {
                    frontal_wave.push_only()
                } else {
                    frontal_wave.clone()
                };
                if let Some(intent) = issue_frontal_wave(
                    &mut actions,
                    observation,
                    profile,
                    attack_policy,
                    &wave_plan,
                    enemy_base,
                    map_analysis,
                    containment_recall_target,
                    memory,
                ) {
                    if let AiIntent::Attack { units } = &intent {
                        if profile.id == JEFFS_AI_BETA_ID
                            || profile.expansion_containment.is_none()
                            || !containment_was_launched
                        {
                            memory.note_attack_for(profile, attack_policy, observation.tick, units);
                        }
                    }
                    intents.push(intent);
                }
            }
        }

        // Last: the current Jeff's resting Tanks with no other orders gather on the home post, so
        // none sits idle in a corner of the main or out on the map.
        if uses_current_jeffs_ai_policy(profile.id) {
            let mut excluded: BTreeSet<u32> = local_defense_assigned.clone();
            excluded.extend(memory.home_defensive_tank);
            excluded.extend(forward_defensive_tank);
            excluded.extend(memory.later_bases.guards.iter().copied());
            excluded.extend(route_line_reserved.iter().copied());
            excluded.extend(memory.route_line.picket());
            excluded.extend(expansion_footprint_blockers.iter().copied());
            let ordered: BTreeSet<u32> = actions
                .unit_orders_since(0)
                .into_iter()
                .map(|(unit, _)| unit)
                .collect();
            let covered_sites: Vec<(f32, f32)> = [
                memory.later_bases.covered_site(),
                memory.expansion_security.site,
            ]
            .into_iter()
            .flatten()
            .filter_map(|site| {
                geometry::building_center(
                    site,
                    EntityKind::ResourceDepot,
                    observation.map.tile_size,
                )
            })
            .collect();
            let gathered = home_armor::gather_resting_tanks(
                &mut actions,
                observation,
                memory,
                &excluded,
                &ordered,
                &covered_sites,
            );
            if !gathered.is_empty() {
                intents.push(AiIntent::Move { units: gathered });
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

#[cfg(test)]
mod tests;
#[cfg(test)]
mod vehicle_worker_tests;

/// On Crossroads Jeff stops turning surplus Steel into Riflemen at this many. It fielded 30-40,
/// most of them idle, while Oil held it to 2-4 Tanks and it never took a third base.
const CROSSROADS_MAX_SURPLUS_RIFLEMEN: usize = 24;
