#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::actions::{
    self, AiActionContext, BuildPlacementRequest, ResourceAssignmentPolicy, SpendBudget,
    TrainUnitsRequest,
};
use crate::ai_core::facts::{AiFacts, EnemyBaseFact};
use crate::ai_core::observation::{
    AiEntityState, AiEntitySummary, AiMapSummary, AiObservation, AiResourceSummary,
};
use crate::ai_core::profiles::{
    AiProfile, AttackPolicy, BarracksCurve, ExpansionPolicy, ProductionPolicy, ProxyBarracksPolicy,
    RecoveryTransitionPolicy, ResourcePolicy, TechTransitionPolicy, WorkerPolicy,
    AI_1_2_TANK_MG_MICRO_ID,
};
use crate::ai_shared;
use crate::config;
use rts_rules;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::{self, UpgradeKind};

mod defense;
mod expansion;
mod frontal;
mod geometry;
mod policies;
mod production;
mod proxy;
mod raids;
mod resources;
mod trace;

use self::defense::{
    defensive_machine_gunner_units, defensive_panic_barracks_target, defensive_panic_plan,
    defensive_panic_response, local_defense_target, local_defense_units,
    stage_defensive_machine_gunner_perimeter, stage_main_steel_defensive_line,
    stages_expansion_defensive_line, DefensivePanic, DefensivePanicResponse, ALL_COMBAT_UNITS,
    DEFENSIVE_PANIC_GRACE_TICKS, DEFENSIVE_PANIC_RIFLE_TECH_PATH, DEFENSIVE_PANIC_SUSTAINED_TICKS,
};
use self::expansion::{plan_expansion, try_build_expansion_city_centre, ExpansionBlocker};
use self::frontal::{issue_frontal_wave, plan_frontal_wave};
#[cfg(test)]
use self::geometry::tile_center;
use self::policies::{
    active_attack_policy, active_barracks_curve, active_production_policy,
    active_required_tech_path, recovery_delay_ticks,
};
use self::production::{
    production_building_order, production_uses_building, should_save_for_first_tech_unit,
    should_save_for_required_tech_building, try_build_kind, unit_counts_for_priorities,
    wants_depot,
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

    fn note_attack_for(&mut self, profile: &AiProfile, attack: AttackPolicy, tick: u32) {
        self.ensure_attack_policy(profile, attack);
        self.last_attack_tick = Some(tick);
        self.next_attack_size = self.next_attack_size.saturating_add(attack.wave_growth);
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
    }

    fn ensure_attack_policy(&mut self, profile: &AiProfile, attack: AttackPolicy) {
        self.ensure_profile(profile);
        if self.attack_first_size == Some(attack.first_attack_size) && self.next_attack_size != 0 {
            return;
        }
        self.attack_first_size = Some(attack.first_attack_size);
        self.next_attack_size = attack.first_attack_size;
        self.last_attack_tick = None;
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
    memory
        .pending_upgrades
        .retain(|upgrade| !observation.upgrades.contains(upgrade));

    let facts = AiFacts::from_observation(observation);
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
    let required_tech_path = if defensive_panic.active {
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

    if save_for_expansion {
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

    for kind in required_tech_path {
        if proxy_barracks_active && *kind == EntityKind::Barracks {
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

    let economy_plan = plan_economy(
        observation,
        &facts,
        profile,
        recovery_active,
        panic_plan.map(|plan| plan.oil_workers),
    );
    let target_barracks = if defensive_panic.active {
        defensive_panic_barracks_target(defensive_panic)
    } else {
        active_barracks_curve(profile, recovery_active).target(
            observation.economy.steel,
            facts.worker_count,
            economy_plan.target_steel_workers,
        )
    };
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

    let save_for_first_tech_unit = should_save_for_first_tech_unit(&facts, production_policy);
    let save_worker_training_for_tech = defensive_panic.active
        || save_for_unplanned_expansion
        || save_for_first_tech_unit
        || (save_for_required_tech_building && facts.worker_count >= economy_plan.target_workers);
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
    let effective_unit_priorities = effective_unit_priorities_for_upgrades(
        production_policy.unit_priorities,
        facts.completed_upgrades(),
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
    let defensive_machine_gunner_max_counts = defensive_machine_gunner_max_counts(profile);
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
                max_counts: &defensive_machine_gunner_max_counts,
                balance_unit_priorities: production_policy.balance_unit_priorities,
            },
        );
        for trained in trained_units {
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
    if economy_plan.desired_oil_workers > economy_plan.current_oil_workers {
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
            },
        );
        if !assigned.is_empty() {
            intents.push(AiIntent::Gather {
                resource: EntityKind::Oil,
                assignments: assigned.len(),
            });
        }
    }

    if economy_plan.target_steel_workers > economy_plan.current_steel_workers {
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
            },
        );
        if !assigned.is_empty() {
            intents.push(AiIntent::Gather {
                resource: EntityKind::Steel,
                assignments: assigned.len(),
            });
        }
    }

    let wounded_retreat_units = issue_wounded_combat_retreats(
        observation,
        profile,
        &mut actions,
        &mut intents,
    );
    let defensive_machine_gunners: Vec<u32> = defensive_machine_gunner_units(observation, profile)
        .into_iter()
        .filter(|id| !wounded_retreat_units.contains(id))
        .collect();
    let defensive_machine_gunner_units: BTreeSet<u32> =
        defensive_machine_gunners.iter().copied().collect();
    let frontal_excluded_units: BTreeSet<u32> = defensive_machine_gunner_units
        .union(&wounded_retreat_units)
        .copied()
        .collect();
    let frontal_wave = plan_frontal_wave(
        observation,
        attack_policy,
        memory,
        profile,
        &frontal_excluded_units,
    );
    let ready_units_count = frontal_wave.ready_units.len();
    let attack_size = frontal_wave.desired_size;
    let attack_due = frontal_wave.attack_due;
    let local_ready_units = actions::select_ready_combat_units_excluding(
        &observation.owned,
        &ALL_COMBAT_UNITS,
        &wounded_retreat_units,
    );
    let rifle_raid_policy = is_rifle_raid_policy(attack_policy);
    let rifle_raid_units = if rifle_raid_policy {
        select_rifle_raid_units(observation)
            .into_iter()
            .filter(|id| !wounded_retreat_units.contains(id))
            .collect()
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

        if !handled_local_defense
            && !handled_raid_target
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

        if !handled_local_defense && !handled_raid_target && !frontal_wave.ready_units.is_empty() {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                if rifle_raid_policy && frontal_wave.should_attack() {
                    let attack_units = {
                        let (x, y) = rifle_raid_move_target(observation, enemy_base);
                        actions::move_units(&mut actions, frontal_wave.ready_units.clone(), x, y)
                    };
                    if let Some(units) = attack_units {
                        memory.note_attack_for(profile, attack_policy, observation.tick);
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
                        if matches!(intent, AiIntent::Attack { .. }) {
                            memory.note_attack_for(profile, attack_policy, observation.tick);
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

fn defensive_machine_gunner_max_counts(profile: &AiProfile) -> Vec<(EntityKind, usize)> {
    profile
        .defensive_machine_gunners
        .map(|policy| vec![(EntityKind::MachineGunner, policy.target_count)])
        .unwrap_or_default()
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

const WOUNDED_COMBAT_RETREAT_HP_PERCENT: u32 = 35;

fn issue_wounded_combat_retreats(
    observation: &AiObservation,
    profile: &AiProfile,
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
) -> BTreeSet<u32> {
    if profile.id != AI_1_2_TANK_MG_MICRO_ID {
        return BTreeSet::new();
    }
    let own_base = geometry::tile_center(observation.own_start_tile, observation.map.tile_size);
    let retreat_kinds = profile
        .tech_transition
        .map(|transition| transition.attack.unit_kinds)
        .unwrap_or(profile.attack.unit_kinds);
    let retreat_units: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| retreat_kinds.contains(&unit.kind))
        .filter(|unit| unit.max_hp > 0)
        .filter(|unit| {
            unit.hp.saturating_mul(100)
                <= unit
                    .max_hp
                    .saturating_mul(WOUNDED_COMBAT_RETREAT_HP_PERCENT)
        })
        .map(|unit| unit.id)
        .collect();
    let Some(units) = actions::move_units(actions, retreat_units, own_base.0, own_base.1) else {
        return BTreeSet::new();
    };
    intents.push(AiIntent::Move {
        units: units.clone(),
    });
    units.into_iter().collect()
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
