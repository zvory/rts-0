#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::actions::{
    self, AiActionContext, BuildPlacementRequest, ResourceAssignmentPolicy, SpendBudget,
    TrainUnitsRequest,
};
use crate::ai_core::facts::{AiFacts, EnemyBaseFact};
use crate::ai_core::map_analysis::AiMapAnalysis;
use crate::ai_core::observation::{
    AiEntityState, AiEntitySummary, AiMapSummary, AiObservation, AiResourceSummary,
};
use crate::ai_core::profiles::{
    AiProfile, AttackPolicy, BarracksCurve, ExpansionPolicy, ProductionPolicy, ProxyBarracksPolicy,
    RecoveryTransitionPolicy, ResourcePolicy, TechTransitionPolicy, WorkerPolicy,
};
use crate::ai_shared;
use crate::config;
use rts_protocol::ObserverMapAnalysisLayer;
use rts_rules;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::{self, UpgradeKind};

mod defense;
mod economy_manager;
mod expansion;
mod frontal;
mod geometry;
mod policies;
mod production;
mod proxy;
mod raids;
mod resources;
mod trace;
mod turtle;

use self::defense::{
    defensive_machine_gunner_units, defensive_panic_barracks_target, defensive_panic_plan,
    defensive_panic_response, local_defense_target, local_defense_units,
    stage_defensive_machine_gunner_perimeter, stage_main_steel_defensive_line,
    stages_expansion_defensive_line, DefensivePanic, DefensivePanicPlan, DefensivePanicResponse,
    ALL_COMBAT_UNITS, DEFENSIVE_PANIC_GRACE_TICKS, DEFENSIVE_PANIC_RIFLE_TECH_PATH,
    DEFENSIVE_PANIC_SUSTAINED_TICKS,
};
use self::economy_manager::{
    propose_economy, EconomyManagerInput, EconomyManagerOutput, EconomyManagerSignals,
    EconomyProposal, OilDemandSignal,
};
use self::expansion::{plan_expansion, try_build_expansion_city_centre, ExpansionBlocker};
use self::frontal::{issue_frontal_wave, plan_frontal_wave};
#[cfg(test)]
use self::geometry::tile_center;
use self::policies::{
    active_attack_policy, active_barracks_curve, active_production_policy,
    active_required_tech_path, active_tech_transition, recovery_delay_ticks,
};
use self::production::{
    production_building_order, production_uses_building, should_build_extra_factory,
    should_save_for_first_tech_unit, should_save_for_required_tech_building, try_build_kind,
    unit_counts_for_priorities, wants_depot,
};
use self::proxy::{should_use_proxy_barracks, try_proxy_barracks};
use self::raids::{
    group_center, is_rifle_raid_policy, rifle_raid_building_fallback_target,
    rifle_raid_move_target, rifle_raid_unit_target, rifle_raid_units_to_resume,
    select_rifle_raid_units,
};
use self::resources::plan_economy;
#[cfg(test)]
use self::resources::{desired_oil_workers, target_steel_workers_for_profile};
use self::trace::{build_manager_trace, ManagerOutputTrace, TraceInput};
use self::turtle::{
    stage_turtle_choke_defense, turtle_machine_gunner_lines_staffed, turtle_observer_debug_layers,
};

use super::profiles::{AI_2_1_ECONOMY_MANAGER_ID, AI_TURTLE_CHOKES_ID};

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AiDecisionMemory {
    profile_id: Option<&'static str>,
    attack_first_size: Option<usize>,
    next_attack_size: usize,
    last_attack_tick: Option<u32>,
    proxy_worker_id: Option<u32>,
    recovery_gate_completed_tick: Option<u32>,
    recovery_active: bool,
    defensive_panic_started_tick: Option<u32>,
    defensive_panic_last_tick: Option<u32>,
    defensive_panic_response: DefensivePanicResponse,
    pending_upgrades: BTreeSet<UpgradeKind>,
    launched_frontal_units: BTreeMap<u32, u32>,
    turtle_opening_riflemen_ordered: usize,
}

impl AiDecisionMemory {
    pub(crate) fn for_profile(profile: &AiProfile) -> Self {
        Self {
            profile_id: Some(profile.id),
            attack_first_size: Some(profile.attack.first_attack_size),
            next_attack_size: profile.attack.first_attack_size,
            last_attack_tick: None,
            proxy_worker_id: None,
            recovery_gate_completed_tick: None,
            recovery_active: false,
            defensive_panic_started_tick: None,
            defensive_panic_last_tick: None,
            defensive_panic_response: DefensivePanicResponse::Riflemen,
            pending_upgrades: BTreeSet::new(),
            launched_frontal_units: BTreeMap::new(),
            turtle_opening_riflemen_ordered: 0,
        }
    }

    pub(crate) fn desired_attack_size(&mut self, profile: &AiProfile, tick: u32) -> usize {
        self.desired_attack_size_for(profile, profile.attack, tick)
    }

    fn desired_attack_size_for(
        &mut self,
        profile: &AiProfile,
        attack: AttackPolicy,
        tick: u32,
    ) -> usize {
        self.ensure_attack_policy(profile, attack);
        if self
            .last_attack_tick
            .map(|last| tick.saturating_sub(last) >= attack.regroup_reset_ticks)
            .unwrap_or(false)
        {
            self.next_attack_size = attack.first_attack_size;
        }
        self.next_attack_size
    }

    fn note_attack_for(
        &mut self,
        profile: &AiProfile,
        attack: AttackPolicy,
        tick: u32,
        units: &[u32],
    ) {
        self.ensure_attack_policy(profile, attack);
        self.last_attack_tick = Some(tick);
        self.next_attack_size = self.next_attack_size.saturating_add(attack.wave_growth);
        if profile.frontal_wave.exclude_launched_ticks.is_some() {
            for unit in units {
                self.launched_frontal_units.insert(*unit, tick);
            }
        }
    }

    fn attack_due_for(&mut self, profile: &AiProfile, attack: AttackPolicy, tick: u32) -> bool {
        self.ensure_attack_policy(profile, attack);
        self.last_attack_tick
            .map(|last| tick.saturating_sub(last) >= attack.reissue_cadence_ticks)
            .unwrap_or(true)
    }

    fn ensure_profile(&mut self, profile: &AiProfile) {
        if self.profile_id == Some(profile.id) && self.next_attack_size != 0 {
            return;
        }
        self.profile_id = Some(profile.id);
        self.attack_first_size = Some(profile.attack.first_attack_size);
        self.next_attack_size = profile.attack.first_attack_size;
        self.last_attack_tick = None;
        self.proxy_worker_id = None;
        self.recovery_gate_completed_tick = None;
        self.recovery_active = false;
        self.defensive_panic_started_tick = None;
        self.defensive_panic_last_tick = None;
        self.defensive_panic_response = DefensivePanicResponse::Riflemen;
        self.pending_upgrades.clear();
        self.launched_frontal_units.clear();
        self.turtle_opening_riflemen_ordered = 0;
    }

    fn ensure_attack_policy(&mut self, profile: &AiProfile, attack: AttackPolicy) {
        self.ensure_profile(profile);
        if self.attack_first_size == Some(attack.first_attack_size) && self.next_attack_size != 0 {
            return;
        }
        self.attack_first_size = Some(attack.first_attack_size);
        self.next_attack_size = attack.first_attack_size;
        self.last_attack_tick = None;
        self.launched_frontal_units.clear();
    }

    fn launched_frontal_unit_exclusions(
        &mut self,
        profile: &AiProfile,
        tick: u32,
        owned_units: &BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let Some(exclude_ticks) = profile.frontal_wave.exclude_launched_ticks else {
            self.launched_frontal_units.clear();
            return BTreeSet::new();
        };
        self.launched_frontal_units.retain(|unit, launched_tick| {
            owned_units.contains(unit) && tick.saturating_sub(*launched_tick) < exclude_ticks
        });
        self.launched_frontal_units.keys().copied().collect()
    }

    fn defensive_panic(
        &mut self,
        threat_response: Option<DefensivePanicResponse>,
        tick: u32,
    ) -> DefensivePanic {
        if let Some(response) = threat_response {
            let should_restart = self
                .defensive_panic_last_tick
                .map(|last| tick.saturating_sub(last) > DEFENSIVE_PANIC_GRACE_TICKS)
                .unwrap_or(true);
            if should_restart {
                self.defensive_panic_started_tick = Some(tick);
            }
            self.defensive_panic_last_tick = Some(tick);
            self.defensive_panic_response = response;
        }

        let active = self
            .defensive_panic_last_tick
            .map(|last| tick.saturating_sub(last) <= DEFENSIVE_PANIC_GRACE_TICKS)
            .unwrap_or(false);
        if !active {
            self.defensive_panic_started_tick = None;
            self.defensive_panic_response = DefensivePanicResponse::Riflemen;
        }
        let sustained = active
            && self
                .defensive_panic_started_tick
                .map(|started| tick.saturating_sub(started) >= DEFENSIVE_PANIC_SUSTAINED_TICKS)
                .unwrap_or(false);
        DefensivePanic {
            active,
            sustained,
            response: self.defensive_panic_response,
        }
    }

    fn recovery_active(&mut self, profile: &AiProfile, facts: &AiFacts, tick: u32) -> bool {
        let Some(policy) = profile.recovery_transition else {
            self.recovery_gate_completed_tick = None;
            self.recovery_active = false;
            return false;
        };
        if self.recovery_active {
            return true;
        }
        if facts.complete_building_count(policy.completed_building) == 0 {
            return false;
        }
        let completed_tick = *self.recovery_gate_completed_tick.get_or_insert(tick);
        let Some(delay_ticks) = recovery_delay_ticks(policy) else {
            return false;
        };
        if tick.saturating_sub(completed_tick) >= delay_ticks {
            self.recovery_active = true;
        }
        self.recovery_active
    }

    fn sync_turtle_opening(&mut self, profile: &AiProfile, observation: &AiObservation) {
        let Some(policy) = profile.turtle_defense else {
            self.turtle_opening_riflemen_ordered = 0;
            return;
        };
        self.turtle_opening_riflemen_ordered = self
            .turtle_opening_riflemen_ordered
            .max(unit_and_queue_count(observation, EntityKind::Rifleman))
            .min(policy.opening_riflemen);
    }

    fn note_turtle_train(&mut self, profile: &AiProfile, unit: EntityKind) {
        let Some(policy) = profile.turtle_defense else {
            return;
        };
        if unit == EntityKind::Rifleman {
            self.turtle_opening_riflemen_ordered = self
                .turtle_opening_riflemen_ordered
                .saturating_add(1)
                .min(policy.opening_riflemen);
        }
    }
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
    memory
        .pending_upgrades
        .retain(|upgrade| !observation.upgrades.contains(upgrade));

    let facts = AiFacts::from_observation(observation);
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
    let recovery_active =
        !defensive_panic.active && memory.recovery_active(profile, &facts, observation.tick);
    let active_tech_transition = active_tech_transition(observation, profile);
    let required_tech_path = if defensive_panic.active && active_tech_transition.is_none() {
        panic_plan
            .map(|plan| plan.required_tech_path)
            .unwrap_or(&DEFENSIVE_PANIC_RIFLE_TECH_PATH)
    } else {
        active_required_tech_path(observation, profile, recovery_active)
    };
    let production_policy = if defensive_panic.active {
        panic_plan.map(|plan| plan.production).unwrap_or_else(|| {
            defensive_panic_plan(DefensivePanicResponse::Riflemen, &facts).production
        })
    } else {
        active_production_policy(observation, profile, recovery_active)
    };
    let attack_policy = active_attack_policy(observation, profile, recovery_active);
    let mut idle_builders = facts.idle_workers.clone();
    let mut gathering_builders = facts.gathering_workers.clone();
    idle_builders.sort_unstable();
    gathering_builders.sort_unstable();
    let builder_pools = [idle_builders.as_slice(), gathering_builders.as_slice()];
    let save_for_required_tech_building =
        should_save_for_required_tech_building(&facts, required_tech_path, production_policy);
    let mut expansion_plan = plan_expansion(
        observation,
        &facts,
        profile,
        recovery_active,
        defensive_panic.active,
    );
    let expansion_blocks_tech_path = expansion_plan.blocks_tech_path;
    let save_for_expansion = expansion_plan.should_save;
    let proxy_barracks_active =
        !defensive_panic.active && should_use_proxy_barracks(&facts, profile);
    let economy_manager_output = if uses_economy_manager(profile) {
        Some(propose_economy(EconomyManagerInput {
            observation,
            facts: &facts,
            profile,
            expansion_plan: &expansion_plan,
            signals: EconomyManagerSignals {
                recovery_active,
                oil_demand: oil_demand_signal(profile, memory, panic_plan),
                defer_supply_for_tech: save_for_required_tech_building,
                emergency_supply: facts.free_supply <= profile.supply.emergency_depot_threshold,
                defer_worker_training_for_tech: defensive_panic.active,
            },
        }))
    } else {
        None
    };

    if proxy_barracks_active {
        if let Some(intent) = try_proxy_barracks(
            observation,
            &facts,
            &mut actions,
            memory,
            profile,
            &mut placeable,
        ) {
            intents.push(intent);
        }
    }

    if should_build_depot_from_economy_manager(&economy_manager_output)
        .unwrap_or_else(|| {
            wants_depot(&facts, profile)
                && (!save_for_required_tech_building
                    || facts.free_supply <= profile.supply.emergency_depot_threshold)
        })
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

    if should_build_expansion_from_economy_manager(&economy_manager_output)
        .unwrap_or(save_for_expansion)
    {
        if try_build_expansion_city_centre(
            observation,
            &facts,
            &mut actions,
            &builder_pools,
            profile,
            recovery_active,
            &mut placeable,
        )
        .is_some()
        {
            intents.push(AiIntent::Build {
                kind: EntityKind::CityCentre,
            });
        } else if expansion_plan.blockers.is_empty() {
            expansion_plan.blockers.push(ExpansionBlocker::NoValidSite);
        }
    }
    let save_for_unplanned_expansion =
        save_for_expansion && planned_in_intents(&intents, EntityKind::CityCentre) == 0;

    let economy_plan = economy_manager_output
        .as_ref()
        .map(|output| output.plan.clone())
        .unwrap_or_else(|| {
            let mut plan = plan_economy(
                observation,
                &facts,
                profile,
                recovery_active,
                panic_plan.map(|plan| plan.oil_workers),
            );
            if turtle_opening_pending(profile, memory) {
                plan.desired_oil_workers = plan.current_oil_workers;
            }
            plan
        });
    let save_worker_training_for_tech = defensive_panic.active;
    let should_train_workers = economy_manager_output
        .as_ref()
        .map(|output| output.proposes(EconomyProposal::TrainWorker))
        .unwrap_or(true);
    if should_train_workers {
        for trained in actions::train_units(
            &mut actions,
            TrainUnitsRequest {
                buildings: facts.production_buildings(EntityKind::CityCentre),
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
        if proxy_barracks_active && *kind == EntityKind::Barracks {
            continue;
        }
        if turtle_should_delay_tech_for_opening(profile, memory, *kind) {
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

    let target_barracks = if defensive_panic.active {
        defensive_panic_barracks_target(defensive_panic)
    } else {
        active_barracks_curve(profile, recovery_active).target(
            observation.economy.steel,
            facts.worker_count,
            economy_plan.target_steel_workers,
        )
    };
    let target_barracks = turtle_barracks_target(profile, &facts, target_barracks);
    if production_uses_building(production_policy, EntityKind::Barracks)
        && facts.building_count(EntityKind::Barracks)
            + planned_in_intents(&intents, EntityKind::Barracks)
            < target_barracks
        && !(proxy_barracks_active && facts.building_count(EntityKind::Barracks) == 0)
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

    if production_uses_building(production_policy, EntityKind::Factory)
        && facts.building_count(EntityKind::Factory)
            + planned_in_intents(&intents, EntityKind::Factory)
            < profile.buildings.factory_target
        && !expansion_blocks_tech_path
        && !save_for_unplanned_expansion
        && planned_in_intents(&intents, EntityKind::Factory) == 0
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
    let tank_methamphetamines_pending = production_policy
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
    if profile.turtle_defense.is_none() {
        queue_profile_upgrades(&mut actions, &facts, memory, &mut intents, profile);
    }
    let effective_unit_priorities = effective_unit_priorities_for_upgrades(
        production_policy.unit_priorities,
        facts.completed_upgrades(),
    );
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
    queue_required_unit_unlocks(
        &mut actions,
        &facts,
        production_policy.unit_priorities,
        memory,
        &mut intents,
    );
    let production_unit_counts =
        unit_counts_for_priorities(observation, &facts, &effective_unit_priorities);
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
        let trained_units = actions::train_units(
            &mut actions,
            TrainUnitsRequest {
                buildings,
                unit_priorities: &effective_unit_priorities,
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: production_policy.queue_depth,
                save_for_tech,
                current_counts: &production_unit_counts,
                max_counts: &production_max_counts,
                balance_unit_priorities: production_policy.balance_unit_priorities,
            },
        );
        for trained in trained_units {
            memory.note_turtle_train(profile, trained.unit);
            intents.push(AiIntent::Train { kind: trained.unit });
        }
    }

    let skipped_workers = BTreeSet::new();
    let panic_support_oil = panic_plan.map(|plan| plan.oil_workers > 0).unwrap_or(false);
    let mut panic_oil_candidates = Vec::new();
    if panic_support_oil {
        panic_oil_candidates.extend(facts.idle_workers.iter().copied());
        panic_oil_candidates.extend(facts.gathering_workers.iter().copied());
        panic_oil_candidates.sort_unstable();
        panic_oil_candidates.dedup();
    }
    let oil_candidate_workers = if panic_support_oil {
        panic_oil_candidates.as_slice()
    } else {
        facts.idle_workers.as_slice()
    };
    let should_assign_oil_workers = economy_manager_output
        .as_ref()
        .map(|output| output.proposes(EconomyProposal::AssignOilWorkers))
        .unwrap_or_else(|| economy_plan.desired_oil_workers > economy_plan.current_oil_workers);
    if should_assign_oil_workers
        && economy_plan.desired_oil_workers > economy_plan.current_oil_workers
    {
        let assigned = actions::assign_workers_to_resource(
            &mut actions,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Oil,
                assignable_node_ids: &economy_plan.mineable_oil_nodes,
                candidate_worker_ids: Some(oil_candidate_workers),
                skip_workers: &skipped_workers,
                pre_reserved_nodes: &economy_plan.occupied_nodes,
                idle_only: !panic_support_oil,
                allow_latched_reassignment: panic_support_oil,
                max_assignments: Some(
                    economy_plan.desired_oil_workers - economy_plan.current_oil_workers,
                ),
                max_worker_resource_distance_px: economy_plan.max_worker_resource_distance_px,
                remote_worker_assignment_fallback: economy_plan.remote_worker_assignment_fallback,
            },
        );
        if !assigned.is_empty() {
            intents.push(AiIntent::Gather {
                resource: EntityKind::Oil,
                assignments: assigned.len(),
            });
        }
    }

    let should_assign_steel_workers = economy_manager_output
        .as_ref()
        .map(|output| output.proposes(EconomyProposal::AssignSteelWorkers))
        .unwrap_or_else(|| economy_plan.target_steel_workers > economy_plan.current_steel_workers);
    if should_assign_steel_workers
        && economy_plan.target_steel_workers > economy_plan.current_steel_workers
    {
        let assigned = actions::assign_workers_to_resource(
            &mut actions,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Steel,
                assignable_node_ids: &economy_plan.mineable_steel_nodes,
                candidate_worker_ids: Some(&facts.idle_workers),
                skip_workers: &skipped_workers,
                pre_reserved_nodes: &economy_plan.occupied_nodes,
                idle_only: true,
                allow_latched_reassignment: false,
                max_assignments: Some(
                    economy_plan.target_steel_workers - economy_plan.current_steel_workers,
                ),
                max_worker_resource_distance_px: economy_plan.max_worker_resource_distance_px,
                remote_worker_assignment_fallback: economy_plan.remote_worker_assignment_fallback,
            },
        );
        if !assigned.is_empty() {
            intents.push(AiIntent::Gather {
                resource: EntityKind::Steel,
                assignments: assigned.len(),
            });
        }
    }

    let defensive_machine_gunners = defensive_machine_gunner_units(observation, profile);
    let defensive_machine_gunner_units: BTreeSet<u32> =
        defensive_machine_gunners.iter().copied().collect();
    let frontal_wave = plan_frontal_wave(
        observation,
        attack_policy,
        memory,
        profile,
        &defensive_machine_gunner_units,
    );
    let ready_units_count = frontal_wave.ready_units.len();
    let attack_size = frontal_wave.desired_size;
    let attack_due = frontal_wave.attack_due;
    let local_ready_units =
        actions::select_ready_combat_units(&observation.owned, &ALL_COMBAT_UNITS);
    let rifle_raid_policy = is_rifle_raid_policy(attack_policy);
    let rifle_raid_units = if rifle_raid_policy {
        select_rifle_raid_units(observation)
    } else {
        Vec::new()
    };
    if !frontal_wave.ready_units.is_empty()
        || !local_ready_units.is_empty()
        || !rifle_raid_units.is_empty()
        || !defensive_machine_gunners.is_empty()
    {
        let mut handled_local_defense = false;
        let mut local_defense_assigned = BTreeSet::new();
        let mut local_defense_targets = BTreeSet::new();
        if let Some(target) = local_defense_target(observation) {
            if let Some(units) = actions::attack_units(
                &mut actions,
                local_defense_units(observation, &local_ready_units),
                target,
            ) {
                local_defense_assigned.extend(units.iter().copied());
                local_defense_targets.extend(defense::local_defense_targets(observation));
                intents.push(AiIntent::Attack { units });
                handled_local_defense = true;
            }
        }

        let mut handled_raid_target = false;
        let raid_units_available: Vec<u32> = rifle_raid_units
            .iter()
            .copied()
            .filter(|id| !local_defense_assigned.contains(id))
            .collect();
        if rifle_raid_policy && !raid_units_available.is_empty() {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                if let Some(target) = rifle_raid_unit_target(
                    observation,
                    &raid_units_available,
                    &local_defense_targets,
                )
                .or_else(|| {
                    rifle_raid_building_fallback_target(
                        observation,
                        &raid_units_available,
                        &local_defense_targets,
                        enemy_base,
                    )
                }) {
                    if let Some(units) =
                        actions::attack_units(&mut actions, raid_units_available.clone(), target)
                    {
                        intents.push(AiIntent::Attack { units });
                        handled_raid_target = true;
                    }
                } else {
                    let (x, y) = rifle_raid_move_target(observation, enemy_base);
                    let resume_units =
                        rifle_raid_units_to_resume(observation, &raid_units_available, (x, y));
                    if !resume_units.is_empty() {
                        if let Some(units) = actions::move_units(&mut actions, resume_units, x, y) {
                            intents.push(AiIntent::Move { units });
                            handled_raid_target = true;
                        }
                    }
                }
            }
        }

        let defensive_machine_gunners_available: Vec<u32> = defensive_machine_gunners
            .iter()
            .copied()
            .filter(|id| !local_defense_assigned.contains(id))
            .collect();
        let turtle_defense_active = profile.turtle_defense.is_some();

        if !handled_local_defense && !handled_raid_target && turtle_defense_active {
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
            && !handled_raid_target
            && !turtle_defense_active
            && !defensive_machine_gunners_available.is_empty()
        {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                if let Some(units) = stage_defensive_machine_gunner_perimeter(
                    &mut actions,
                    observation,
                    &defensive_machine_gunners_available,
                    enemy_base,
                ) {
                    intents.push(AiIntent::Stage { units });
                }
            }
        }

        if !handled_local_defense
            && !handled_raid_target
            && !turtle_defense_active
            && !frontal_wave.ready_units.is_empty()
        {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                if rifle_raid_policy && frontal_wave.should_attack() {
                    let attack_units = {
                        let (x, y) = rifle_raid_move_target(observation, enemy_base);
                        actions::move_units(&mut actions, frontal_wave.ready_units.clone(), x, y)
                    };
                    if let Some(units) = attack_units {
                        memory.note_attack_for(profile, attack_policy, observation.tick, &units);
                        intents.push(AiIntent::Attack { units });
                    }
                } else if !rifle_raid_policy {
                    if let Some(intent) = issue_frontal_wave(
                        &mut actions,
                        observation,
                        profile,
                        attack_policy,
                        &frontal_wave,
                        enemy_base,
                    ) {
                        if let AiIntent::Attack { units } = &intent {
                            memory.note_attack_for(profile, attack_policy, observation.tick, units);
                        }
                        intents.push(intent);
                    }
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
        wants_depot: wants_depot(&facts, profile),
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
        rifle_raid_policy,
        rifle_raid_units: rifle_raid_units.len(),
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

fn uses_economy_manager(profile: &AiProfile) -> bool {
    matches!(
        profile.id,
        AI_2_1_ECONOMY_MANAGER_ID | AI_TURTLE_CHOKES_ID
    )
}

fn oil_demand_signal(
    profile: &AiProfile,
    memory: &AiDecisionMemory,
    panic_plan: Option<DefensivePanicPlan>,
) -> OilDemandSignal {
    if turtle_opening_pending(profile, memory) {
        return OilDemandSignal::HoldCurrent;
    }
    panic_plan
        .map(|plan| OilDemandSignal::ExactWorkers(plan.oil_workers))
        .unwrap_or(OilDemandSignal::ProfileDefault)
}

fn should_build_depot_from_economy_manager(
    output: &Option<EconomyManagerOutput>,
) -> Option<bool> {
    output
        .as_ref()
        .map(|output| output.proposes(EconomyProposal::BuildSupplyDepot))
}

fn should_build_expansion_from_economy_manager(
    output: &Option<EconomyManagerOutput>,
) -> Option<bool> {
    output
        .as_ref()
        .map(|output| output.proposes(EconomyProposal::BuildExpansionCityCentre))
}

fn turtle_should_delay_tech_for_opening(
    profile: &AiProfile,
    memory: &AiDecisionMemory,
    kind: EntityKind,
) -> bool {
    kind != EntityKind::Barracks && turtle_opening_pending(profile, memory)
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

fn unit_and_queue_count(observation: &AiObservation, kind: EntityKind) -> usize {
    let units = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == kind)
        .count();
    let queued = observation
        .owned
        .iter()
        .filter(|entity| entity.is_complete)
        .filter(|entity| entity.production_kind == Some(kind))
        .map(|entity| entity.production_queue_len.unwrap_or(0))
        .sum::<usize>();
    units.saturating_add(queued)
}

fn effective_unit_priorities_for_upgrades(
    unit_priorities: &[EntityKind],
    completed_upgrades: &[UpgradeKind],
) -> Vec<EntityKind> {
    let methamphetamines_ready = completed_upgrades.contains(&UpgradeKind::Methamphetamines);
    unit_priorities
        .iter()
        .copied()
        .filter(|unit| *unit != EntityKind::Tank || methamphetamines_ready)
        .collect()
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
        queue_upgrade_if_available(actions, facts, memory, intents, *upgrade);
    }
}

fn queue_required_unit_unlocks(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    unit_priorities: &[EntityKind],
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    for unit in unit_priorities {
        let Some(upgrade) = upgrade::required_for_unit(*unit) else {
            continue;
        };
        queue_upgrade_if_available(actions, facts, memory, intents, upgrade);
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod vehicle_worker_tests;
