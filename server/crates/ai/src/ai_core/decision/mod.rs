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
};
use crate::ai_shared;
use crate::config;
use rts_rules;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::{self, UpgradeKind};

mod defense;
mod expansion;
mod geometry;
mod policies;
mod production;
mod proxy;
mod raids;
mod resources;
mod trace;

use self::defense::{
    defensive_panic_barracks_target, defensive_panic_plan, defensive_panic_response,
    local_defense_target, local_defense_units, stage_main_steel_defensive_line,
    stages_expansion_defensive_line, DefensivePanic, DefensivePanicResponse, ALL_COMBAT_UNITS,
    DEFENSIVE_PANIC_GRACE_TICKS, DEFENSIVE_PANIC_RIFLE_TECH_PATH, DEFENSIVE_PANIC_SUSTAINED_TICKS,
};
use self::expansion::{
    expansion_blocks_tech_path, should_save_for_expansion, try_build_expansion_city_centre,
};
use self::geometry::tile_center;
use self::policies::{
    active_attack_policy, active_barracks_curve, active_production_policy,
    active_required_tech_path, active_worker_policy, recovery_delay_ticks,
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
use self::resources::{
    desired_oil_workers, max_worker_resource_assignment_distance_px, occupied_resource_nodes,
    resource_worker_counts, target_steel_workers_for_profile,
};
use self::trace::{build_manager_trace, ManagerOutputTrace, TraceInput};

const OUTBOUND_WAVE_VISIBLE_TARGET_RADIUS_TILES: f32 = 14.0;

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
    let expansion_blocks_tech_path = !defensive_panic.active
        && expansion_blocks_tech_path(observation, &facts, profile, recovery_active);
    let save_for_expansion = !defensive_panic.active
        && should_save_for_expansion(observation, &facts, profile, recovery_active);
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

    if save_for_expansion
        && try_build_expansion_city_centre(
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

    let worker_policy = active_worker_policy(profile, recovery_active);
    let complete_gate_count = worker_policy
        .pressure_until_complete
        .map(|kind| facts.complete_building_count(kind))
        .unwrap_or(usize::MAX);
    let target_steel_workers =
        worker_policy.target_steel_workers(facts.target_steel_workers, complete_gate_count);
    let target_steel_workers = target_steel_workers_for_profile(
        observation,
        &facts,
        profile,
        recovery_active,
        target_steel_workers,
    );
    let target_barracks = if defensive_panic.active {
        defensive_panic_barracks_target(defensive_panic)
    } else {
        active_barracks_curve(profile, recovery_active).target(
            observation.economy.steel,
            facts.worker_count,
            target_steel_workers,
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

    let desired_oil_workers = if let Some(plan) = panic_plan {
        plan.oil_workers
    } else {
        desired_oil_workers(
            observation,
            &facts,
            profile,
            recovery_active,
            target_steel_workers,
        )
    };
    let target_workers = target_steel_workers.saturating_add(desired_oil_workers);
    let save_for_first_tech_unit = should_save_for_first_tech_unit(&facts, production_policy);
    let save_worker_training_for_tech = defensive_panic.active
        || save_for_unplanned_expansion
        || save_for_first_tech_unit
        || (save_for_required_tech_building && facts.worker_count >= target_workers);
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
            max_counts: &[(EntityKind::Worker, target_workers)],
            balance_unit_priorities: false,
        },
    ) {
        intents.push(AiIntent::Train { kind: trained.unit });
    }

    let production_unit_counts =
        unit_counts_for_priorities(observation, &facts, production_policy.unit_priorities);
    queue_required_unit_unlocks(
        &mut actions,
        &facts,
        production_policy.unit_priorities,
        memory,
        &mut intents,
    );
    for building_kind in production_building_order(production_policy.unit_priorities) {
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
            && !rts_rules::economy::trainable_units(building_kind).contains(&key_tech_unit);
        let trained_units = actions::train_units(
            &mut actions,
            TrainUnitsRequest {
                buildings,
                unit_priorities: production_policy.unit_priorities,
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: production_policy.queue_depth,
                save_for_tech,
                current_counts: &production_unit_counts,
                max_counts: &[],
                balance_unit_priorities: production_policy.balance_unit_priorities,
            },
        );
        for trained in trained_units {
            intents.push(AiIntent::Train { kind: trained.unit });
        }
    }

    let occupied_nodes = occupied_resource_nodes(observation);
    let skipped_workers = BTreeSet::new();
    let resource_counts = resource_worker_counts(observation);
    let max_worker_resource_distance_px =
        max_worker_resource_assignment_distance_px(observation, &facts, profile, recovery_active);
    let current_oil_workers = resource_counts.get(&EntityKind::Oil).copied().unwrap_or(0);
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
    if desired_oil_workers > current_oil_workers {
        let assigned = actions::assign_workers_to_resource(
            &mut actions,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Oil,
                candidate_worker_ids: Some(oil_candidate_workers),
                skip_workers: &skipped_workers,
                pre_reserved_nodes: &occupied_nodes,
                idle_only: !panic_support_oil,
                allow_latched_reassignment: panic_support_oil,
                max_assignments: Some(desired_oil_workers - current_oil_workers),
                max_worker_resource_distance_px,
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
                allow_latched_reassignment: false,
                max_assignments: Some(target_steel_workers - current_steel_workers),
                max_worker_resource_distance_px,
            },
        );
        if !assigned.is_empty() {
            intents.push(AiIntent::Gather {
                resource: EntityKind::Steel,
                assignments: assigned.len(),
            });
        }
    }

    let ready_units =
        actions::select_ready_combat_units(&observation.owned, attack_policy.unit_kinds);
    let ready_units_count = ready_units.len();
    let attack_size = memory.desired_attack_size_for(profile, attack_policy, observation.tick);
    let attack_due = memory.attack_due_for(profile, attack_policy, observation.tick);
    let local_ready_units =
        actions::select_ready_combat_units(&observation.owned, &ALL_COMBAT_UNITS);
    let rifle_raid_policy = is_rifle_raid_policy(attack_policy);
    let rifle_raid_units = if rifle_raid_policy {
        select_rifle_raid_units(observation)
    } else {
        Vec::new()
    };
    if !ready_units.is_empty() || !local_ready_units.is_empty() || !rifle_raid_units.is_empty() {
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

        if !handled_local_defense && !handled_raid_target && !ready_units.is_empty() {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
                let required_unit_ready = attack_policy
                    .required_unit
                    .map(|kind| {
                        observation
                            .owned
                            .iter()
                            .any(|entity| entity.kind == kind && ready_units.contains(&entity.id))
                    })
                    .unwrap_or(true);
                if required_unit_ready
                    && ready_units.len() >= attack_size
                    && attack_due
                {
                    let attack_units = if rifle_raid_policy {
                        let (x, y) = rifle_raid_move_target(observation, enemy_base);
                        actions::move_units(&mut actions, ready_units, x, y)
                    } else if let Some(target) =
                        visible_combat_target_for_wave(observation, &ready_units)
                    {
                        actions::attack_units(&mut actions, ready_units, target)
                    } else {
                        actions::attack_move_units(
                            &mut actions,
                            ready_units,
                            enemy_base.x,
                            enemy_base.y,
                        )
                    };
                    if let Some(units) = attack_units {
                        memory.note_attack_for(profile, attack_policy, observation.tick);
                        intents.push(AiIntent::Attack { units });
                    }
                } else if !ready_units.is_empty() {
                    let staged = if stages_expansion_defensive_line(profile, attack_policy) {
                        stage_main_steel_defensive_line(
                            &mut actions,
                            observation,
                            &ready_units,
                            enemy_base,
                            attack_policy.stage_distance_tiles,
                        )
                    } else {
                        let own_base =
                            tile_center(observation.own_start_tile, observation.map.tile_size);
                        actions::stage_units_toward(
                            &mut actions,
                            ready_units,
                            own_base,
                            (enemy_base.x, enemy_base.y),
                            observation.map.tile_size,
                            attack_policy.stage_distance_tiles,
                        )
                    };
                    if let Some(units) = staged {
                        intents.push(AiIntent::Stage { units });
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
        expansion_blocks_tech_path,
        save_for_unplanned_expansion,
        save_for_required_tech_building,
        save_worker_training_for_tech,
        defensive_panic_active: defensive_panic.active,
        local_threat_active: local_threat_response.is_some(),
        ready_units: ready_units_count,
        attack_size,
        attack_due,
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

fn visible_combat_target_for_wave(observation: &AiObservation, unit_ids: &[u32]) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = OUTBOUND_WAVE_VISIBLE_TARGET_RADIUS_TILES * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .map(|enemy| {
            let distance2 = geometry::dist2(center.0, center.1, enemy.x, enemy.y);
            (
                enemy.id,
                outbound_wave_target_priority(enemy.kind),
                distance2,
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn outbound_wave_target_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Tank => 0,
        EntityKind::MachineGunner | EntityKind::AtTeam => 1,
        EntityKind::Rifleman | EntityKind::ScoutCar => 2,
        _ => 3,
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
        if facts.completed_upgrades().contains(&upgrade)
            || memory.pending_upgrades.contains(&upgrade)
        {
            continue;
        }
        let definition = upgrade::definition(upgrade);
        if facts.complete_building_count(definition.researched_at) == 0 {
            continue;
        }
        let Some(researched) = actions::try_research_upgrade(
            actions,
            facts.production_buildings(definition.researched_at),
            upgrade,
        ) else {
            continue;
        };
        memory.pending_upgrades.insert(researched.upgrade);
        intents.push(AiIntent::Research {
            upgrade: researched.upgrade,
        });
    }
}

#[cfg(test)]
mod tests;
