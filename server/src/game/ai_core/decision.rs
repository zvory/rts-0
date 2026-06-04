#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::config;
use crate::game::ai_core::actions::{
    self, AiActionContext, BuildPlacementRequest, ResourceAssignmentPolicy, SpendBudget,
    TrainUnitsRequest,
};
use crate::game::ai_core::facts::{AiFacts, EnemyBaseFact};
use crate::game::ai_core::observation::{
    AiEntityState, AiEntitySummary, AiMapSummary, AiObservation, AiResourceSummary,
};
use crate::game::ai_core::profiles::{
    AiProfile, AttackPolicy, BarracksCurve, ExpansionPolicy, ProductionPolicy, ProxyBarracksPolicy,
    RecoveryTransitionPolicy, ResourcePolicy, TechTransitionPolicy, WorkerPolicy,
};
use crate::game::ai_shared;
use crate::game::command::SimCommand as Command;
use crate::game::entity::EntityKind;
use crate::rules;

const PRODUCTION_BUILDINGS: [EntityKind; 3] = [
    EntityKind::Factory,
    EntityKind::Barracks,
    EntityKind::CityCentre,
];
const LOCAL_DEFENSE_RADIUS_TILES: f32 = 12.0;
const RESOURCE_LINE_DEFENSE_RADIUS_TILES: f32 = 4.0;
const WORKER_DEFENSE_RADIUS_TILES: f32 = 5.0;
const PROXY_DISTANCE_BAND_TILES: f32 = 2.0;
const PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES: i32 = 4;
const EXPANSION_LOCAL_RESOURCE_ASSIGNMENT_RADIUS_TILES: f32 = config::MINING_CC_RANGE_TILES + 3.0;
const EXPANSION_DEFENSIVE_LINE_SPACING_TILES: f32 = 1.5;
const EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES: f32 = 0.75;
const RIFLE_RAID_DEEPEN_TILES: f32 = 7.0;
const RIFLE_RAID_STEEL_LINE_RADIUS_TILES: f32 = 4.0;
const DEFENSIVE_PANIC_GRACE_TICKS: u32 = 90;
const DEFENSIVE_PANIC_SUSTAINED_TICKS: u32 = 180;
const DEFENSIVE_PANIC_SUSTAINED_BARRACKS: usize = 2;
const DEFENSIVE_PANIC_DPS_DOMINANCE: f32 = 0.75;
const DEFENSIVE_PANIC_OIL_WORKERS: usize = 2;
const DEFENSIVE_PANIC_RIFLE_TECH_PATH: [EntityKind; 1] = [EntityKind::Barracks];
const DEFENSIVE_PANIC_RIFLE_UNITS: [EntityKind; 1] = [EntityKind::Rifleman];
const DEFENSIVE_PANIC_MG_UNITS: [EntityKind; 2] = [EntityKind::MachineGunner, EntityKind::Rifleman];
const DEFENSIVE_PANIC_AT_UNITS: [EntityKind; 2] = [EntityKind::AtTeam, EntityKind::Rifleman];
const DEFENSIVE_PANIC_SUPPORT_MIX_UNITS: [EntityKind; 3] = [
    EntityKind::AtTeam,
    EntityKind::MachineGunner,
    EntityKind::Rifleman,
];
const ALL_COMBAT_UNITS: [EntityKind; 5] = [
    EntityKind::Rifleman,
    EntityKind::MachineGunner,
    EntityKind::AtTeam,
    EntityKind::ScoutCar,
    EntityKind::Tank,
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AiDecision {
    pub(crate) profile_id: &'static str,
    pub(crate) intents: Vec<AiIntent>,
    pub(crate) commands: Vec<Command>,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DefensivePanicResponse {
    #[default]
    Riflemen,
    MachineGunners,
    AtTeams,
    SupportMix,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DefensivePanic {
    active: bool,
    sustained: bool,
    response: DefensivePanicResponse,
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

    let local_threat_response = local_defense_target(observation)
        .is_some()
        .then(|| defensive_panic_response(observation));
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

    for kind in required_tech_path {
        if proxy_barracks_active && *kind == EntityKind::Barracks {
            continue;
        }
        if expansion_blocks_tech_path {
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
        || save_for_expansion
        || save_for_first_tech_unit
        || (save_for_required_tech_building && facts.worker_count >= target_workers);
    for trained in actions::train_units(
        &mut actions,
        TrainUnitsRequest {
            buildings: facts.production_buildings(EntityKind::CityCentre),
            unit_priorities: &[EntityKind::Worker],
            completed_building_kinds: facts.complete_building_kinds(),
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
    for building_kind in production_building_order(production_policy.unit_priorities) {
        let buildings = facts.production_buildings(building_kind);
        if buildings.is_empty() {
            continue;
        }
        let key_tech_unit = production_policy
            .save_for_first_tech_unit
            .unwrap_or(EntityKind::Worker);
        let save_for_tech =
            (save_for_expansion || save_for_first_tech_unit || save_for_required_tech_building)
                && !rules::economy::trainable_units(building_kind).contains(&key_tech_unit);
        for trained in actions::train_units(
            &mut actions,
            TrainUnitsRequest {
                buildings,
                unit_priorities: production_policy.unit_priorities,
                completed_building_kinds: facts.complete_building_kinds(),
                max_queue_depth: production_policy.queue_depth,
                save_for_tech,
                current_counts: &production_unit_counts,
                max_counts: &[],
                balance_unit_priorities: production_policy.balance_unit_priorities,
            },
        ) {
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
                let geometry = LocalDefenseGeometry::from_observation(observation);
                local_defense_targets.extend(
                    observation
                        .visible_enemies
                        .iter()
                        .filter(|enemy| geometry.contains(enemy))
                        .map(|enemy| enemy.id),
                );
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
                    let continuing_units =
                        active_rifle_raid_units(observation, &raid_units_available);
                    if !continuing_units.is_empty() {
                        let (x, y) = rifle_raid_move_target(observation, enemy_base);
                        if let Some(units) =
                            actions::move_units(&mut actions, continuing_units, x, y)
                        {
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
                let attack_size =
                    memory.desired_attack_size_for(profile, attack_policy, observation.tick);
                if required_unit_ready
                    && ready_units.len() >= attack_size
                    && memory.attack_due_for(profile, attack_policy, observation.tick)
                {
                    let attack_units = if rifle_raid_policy {
                        let (x, y) = rifle_raid_move_target(observation, enemy_base);
                        actions::move_units(&mut actions, ready_units, x, y)
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

    AiDecision {
        profile_id: profile.id,
        intents,
        commands: actions.into_commands(),
    }
}

#[allow(clippy::too_many_arguments)]
fn try_proxy_barracks<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    actions: &mut AiActionContext<'_>,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    placeable: &mut F,
) -> Option<AiIntent>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let policy = profile.buildings.proxy_barracks?;
    let kind = EntityKind::Barracks;
    if !rules::economy::build_requirement_met(kind, facts.complete_building_kinds()) {
        return None;
    }
    let counts = facts.building_counts(kind);
    if counts.total_planned() > 0 {
        return None;
    }
    let enemy_base = facts.nearest_public_enemy_base?;
    let proxy_worker_was_committed = memory.proxy_worker_id.is_some();
    let worker = select_proxy_worker(observation, facts, memory)?;
    let worker_pool = [worker];
    let worker_entity = observation
        .owned
        .iter()
        .find(|entity| entity.id == worker && entity.kind == EntityKind::Worker)?;

    if proxy_worker_was_committed {
        if let Some((tile_x, tile_y)) =
            proxy_barracks_site_near_worker(observation, worker_entity, kind, placeable)
        {
            if actions::try_build_at(actions, &[&worker_pool], kind, tile_x, tile_y).is_some() {
                memory.proxy_worker_id = Some(worker);
                return Some(AiIntent::Build { kind });
            }
        }
    }

    let Some(transit_site) =
        proxy_barracks_transit_site(observation, enemy_base, kind, policy, placeable)
    else {
        if !proxy_worker_was_committed {
            memory.proxy_worker_id = None;
        }
        return None;
    };

    if actions::try_build_at(
        actions,
        &[&worker_pool],
        kind,
        transit_site.0,
        transit_site.1,
    )
    .is_some()
    {
        memory.proxy_worker_id = Some(worker);
        return Some(AiIntent::Build { kind });
    }

    if !actions.reserve_worker(worker) {
        return None;
    }
    let (x, y) = building_center(transit_site, kind, observation.map.tile_size)?;
    actions.emit_command(Command::Move {
        units: vec![worker],
        x,
        y,
    });
    Some(AiIntent::Move {
        units: vec![worker],
    })
}

fn should_use_proxy_barracks(facts: &AiFacts, profile: &AiProfile) -> bool {
    profile.buildings.proxy_barracks.is_some() && facts.building_count(EntityKind::Barracks) == 0
}

fn defensive_panic_barracks_target(panic: DefensivePanic) -> usize {
    if panic.sustained {
        DEFENSIVE_PANIC_SUSTAINED_BARRACKS
    } else {
        1
    }
}

#[derive(Clone, Copy, Debug)]
struct DefensivePanicPlan {
    required_tech_path: &'static [EntityKind],
    production: ProductionPolicy,
    oil_workers: usize,
}

fn defensive_panic_plan(response: DefensivePanicResponse, facts: &AiFacts) -> DefensivePanicPlan {
    let support_tech_ready = facts.complete_building_count(EntityKind::TrainingCentre) > 0;
    match response {
        DefensivePanicResponse::Riflemen => defensive_panic_rifle_plan(),
        DefensivePanicResponse::MachineGunners if support_tech_ready => DefensivePanicPlan {
            required_tech_path: &DEFENSIVE_PANIC_RIFLE_TECH_PATH,
            production: ProductionPolicy {
                queue_depth: 3,
                unit_priorities: &DEFENSIVE_PANIC_MG_UNITS,
                save_for_first_tech_unit: None,
                balance_unit_priorities: false,
            },
            oil_workers: DEFENSIVE_PANIC_OIL_WORKERS,
        },
        DefensivePanicResponse::AtTeams if support_tech_ready => DefensivePanicPlan {
            required_tech_path: &DEFENSIVE_PANIC_RIFLE_TECH_PATH,
            production: ProductionPolicy {
                queue_depth: 3,
                unit_priorities: &DEFENSIVE_PANIC_AT_UNITS,
                save_for_first_tech_unit: None,
                balance_unit_priorities: false,
            },
            oil_workers: DEFENSIVE_PANIC_OIL_WORKERS,
        },
        DefensivePanicResponse::SupportMix if support_tech_ready => DefensivePanicPlan {
            required_tech_path: &DEFENSIVE_PANIC_RIFLE_TECH_PATH,
            production: ProductionPolicy {
                queue_depth: 3,
                unit_priorities: &DEFENSIVE_PANIC_SUPPORT_MIX_UNITS,
                save_for_first_tech_unit: None,
                balance_unit_priorities: true,
            },
            oil_workers: DEFENSIVE_PANIC_OIL_WORKERS,
        },
        DefensivePanicResponse::MachineGunners
        | DefensivePanicResponse::AtTeams
        | DefensivePanicResponse::SupportMix => defensive_panic_rifle_plan(),
    }
}

fn defensive_panic_rifle_plan() -> DefensivePanicPlan {
    DefensivePanicPlan {
        required_tech_path: &DEFENSIVE_PANIC_RIFLE_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 3,
            unit_priorities: &DEFENSIVE_PANIC_RIFLE_UNITS,
            save_for_first_tech_unit: None,
            balance_unit_priorities: false,
        },
        oil_workers: 0,
    }
}

fn defensive_panic_response(observation: &AiObservation) -> DefensivePanicResponse {
    let geometry = LocalDefenseGeometry::from_observation(observation);
    let mut local_scores = DefensiveThreatScores::default();
    let mut visible_scores = DefensiveThreatScores::default();

    for enemy in &observation.visible_enemies {
        let score = defensive_threat_dps(enemy);
        if score <= 0.0 {
            continue;
        }
        visible_scores.add(enemy.kind, score);
        if geometry.contains(enemy) {
            local_scores.add(enemy.kind, score);
        }
    }

    if local_scores.non_empty() {
        local_scores
    } else {
        visible_scores
    }
    .response()
}

#[derive(Clone, Copy, Debug, Default)]
struct DefensiveThreatScores {
    armored_dps: f32,
    infantry_dps: f32,
}

impl DefensiveThreatScores {
    fn add(&mut self, kind: EntityKind, dps: f32) {
        if kind == EntityKind::Tank {
            self.armored_dps += dps;
        } else if kind.is_unit() {
            self.infantry_dps += dps;
        }
    }

    fn non_empty(self) -> bool {
        self.armored_dps + self.infantry_dps > f32::EPSILON
    }

    fn response(self) -> DefensivePanicResponse {
        let total = self.armored_dps + self.infantry_dps;
        if total <= f32::EPSILON {
            return DefensivePanicResponse::Riflemen;
        }
        if self.armored_dps / total >= DEFENSIVE_PANIC_DPS_DOMINANCE {
            DefensivePanicResponse::AtTeams
        } else if self.infantry_dps / total >= DEFENSIVE_PANIC_DPS_DOMINANCE {
            DefensivePanicResponse::MachineGunners
        } else {
            DefensivePanicResponse::SupportMix
        }
    }
}

fn defensive_threat_dps(enemy: &AiEntitySummary) -> f32 {
    if !enemy.kind.is_unit() {
        return 0.0;
    }
    let profile = rules::combat::attack_profile(enemy.kind);
    if profile.dmg == 0 || profile.cooldown == 0 {
        return 0.0;
    }
    profile.dmg as f32 / profile.cooldown as f32
}

fn active_expansion(
    observation: &AiObservation,
    profile: &AiProfile,
    recovery_active: bool,
) -> Option<ExpansionPolicy> {
    let expansion = active_expansion_policy(profile, recovery_active)?;
    if observation.economy.steel >= expansion.trigger_steel
        || observation.economy.supply_used >= expansion.trigger_supply_used
    {
        Some(expansion)
    } else {
        None
    }
}

fn expansion_blocks_tech_path(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
) -> bool {
    let Some(expansion) = active_expansion(observation, profile, recovery_active) else {
        return false;
    };
    expansion.blocks_tech_path
        && facts.building_count(EntityKind::CityCentre) < expansion.target_city_centres
}

fn should_save_for_expansion(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
) -> bool {
    let Some(expansion) = active_expansion(observation, profile, recovery_active) else {
        return false;
    };
    facts.building_count(EntityKind::CityCentre) < expansion.target_city_centres
        && expansion_prerequisites_met(facts, expansion)
}

fn expansion_prerequisites_met(facts: &AiFacts, expansion: ExpansionPolicy) -> bool {
    facts.complete_building_count(expansion.required_complete_building) > 0
        && facts.unit_count(expansion.defensive_unit) >= expansion.defensive_unit_count
}

fn active_tech_transition(
    observation: &AiObservation,
    profile: &AiProfile,
) -> Option<TechTransitionPolicy> {
    profile
        .tech_transition
        .filter(|transition| observation.economy.supply_used >= transition.supply_used_threshold)
}

fn active_recovery(profile: &AiProfile, recovery_active: bool) -> Option<RecoveryTransitionPolicy> {
    if recovery_active {
        profile.recovery_transition
    } else {
        None
    }
}

fn active_required_tech_path(
    observation: &AiObservation,
    profile: &AiProfile,
    recovery_active: bool,
) -> &'static [EntityKind] {
    active_tech_transition(observation, profile)
        .map(|transition| transition.required_tech_path)
        .or_else(|| {
            active_recovery(profile, recovery_active).map(|recovery| recovery.required_tech_path)
        })
        .unwrap_or(profile.buildings.required_tech_path)
}

fn active_production_policy(
    observation: &AiObservation,
    profile: &AiProfile,
    recovery_active: bool,
) -> ProductionPolicy {
    active_tech_transition(observation, profile)
        .map(|transition| transition.production)
        .or_else(|| active_recovery(profile, recovery_active).map(|recovery| recovery.production))
        .unwrap_or(profile.production)
}

fn active_attack_policy(
    observation: &AiObservation,
    profile: &AiProfile,
    recovery_active: bool,
) -> AttackPolicy {
    active_tech_transition(observation, profile)
        .map(|transition| transition.attack)
        .or_else(|| active_recovery(profile, recovery_active).map(|recovery| recovery.attack))
        .unwrap_or(profile.attack)
}

fn active_worker_policy(profile: &AiProfile, recovery_active: bool) -> WorkerPolicy {
    active_recovery(profile, recovery_active)
        .map(|recovery| recovery.workers)
        .unwrap_or(profile.workers)
}

fn active_resource_policy(profile: &AiProfile, recovery_active: bool) -> ResourcePolicy {
    active_recovery(profile, recovery_active)
        .map(|recovery| recovery.resources)
        .unwrap_or(profile.resources)
}

fn active_barracks_curve(profile: &AiProfile, recovery_active: bool) -> BarracksCurve {
    active_recovery(profile, recovery_active)
        .map(|recovery| recovery.barracks_curve)
        .unwrap_or(profile.buildings.barracks_curve)
}

fn active_expansion_policy(profile: &AiProfile, recovery_active: bool) -> Option<ExpansionPolicy> {
    active_recovery(profile, recovery_active)
        .and_then(|recovery| recovery.expansion)
        .or(profile.expansion)
}

fn recovery_delay_ticks(policy: RecoveryTransitionPolicy) -> Option<u32> {
    let build_ticks = config::unit_stats(policy.delay_unit)?.build_ticks;
    // The fast proxy should not stay all-in forever. Wait long enough for the proxy to have
    // produced a meaningful early rifle stream, then recover into economy and support tech.
    Some(build_ticks.saturating_mul(policy.delay_unit_build_count))
}

fn target_steel_workers_for_profile(
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

fn completed_cc_steel_saturation_target(observation: &AiObservation) -> usize {
    let completed_ccs: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::CityCentre
                && entity.is_complete
                && entity.state != AiEntityState::Dead
        })
        .collect();
    if completed_ccs.is_empty() {
        return 0;
    }
    let max_dist_px = (config::CC_RESOURCE_MAX_DIST_TILES + 0.5) * observation.map.tile_size as f32;
    let max_dist2 = squared(max_dist_px);
    observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
        .filter(|resource| {
            completed_ccs
                .iter()
                .any(|cc| dist2(resource.x, resource.y, cc.x, cc.y) <= max_dist2)
        })
        .count()
}

fn max_worker_resource_assignment_distance_px(
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

fn stages_expansion_defensive_line(profile: &AiProfile, attack: AttackPolicy) -> bool {
    profile.expansion.is_some() && attack.first_attack_size == usize::MAX
}

fn stage_main_steel_defensive_line(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
) -> Option<Vec<u32>> {
    let assignments = main_steel_defensive_line_assignments(
        observation,
        ready_units,
        enemy_base,
        distance_tiles,
    )?;
    let units_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let close_enough_px =
        EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * observation.map.tile_size as f32;
    let close_enough2 = squared(close_enough_px);
    let mut staged = Vec::new();

    for assignment in assignments {
        let Some(unit) = units_by_id.get(&assignment.unit_id).copied() else {
            continue;
        };
        if dist2(unit.x, unit.y, assignment.x, assignment.y) <= close_enough2 {
            continue;
        }
        if let Some(units) =
            actions::attack_move_units(actions, [assignment.unit_id], assignment.x, assignment.y)
        {
            staged.extend(units);
        }
    }

    (!staged.is_empty()).then_some(staged)
}

#[derive(Clone, Copy, Debug)]
struct DefensiveLineAssignment {
    unit_id: u32,
    x: f32,
    y: f32,
}

fn main_steel_defensive_line_assignments(
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
) -> Option<Vec<DefensiveLineAssignment>> {
    if ready_units.is_empty() {
        return None;
    }
    let steel_center = main_steel_cluster_center(observation)?;
    let enemy = (enemy_base.x, enemy_base.y);
    let (dir_x, dir_y) = normalized_direction(steel_center, enemy)?;
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return None;
    }
    let front_distance = distance_tiles.max(1.0) * tile_size;
    let line_center = clamp_to_map(
        (
            steel_center.0 + dir_x * front_distance,
            steel_center.1 + dir_y * front_distance,
        ),
        observation.map,
    );
    let perp = (-dir_y, dir_x);
    let spacing = EXPANSION_DEFENSIVE_LINE_SPACING_TILES * tile_size;
    let mut units = ready_units.to_vec();
    units.sort_unstable();
    units.dedup();
    let center_index = (units.len().saturating_sub(1)) as f32 * 0.5;

    let assignments = units
        .into_iter()
        .enumerate()
        .map(|(index, unit_id)| {
            let offset = (index as f32 - center_index) * spacing;
            let (x, y) = clamp_to_map(
                (
                    line_center.0 + perp.0 * offset,
                    line_center.1 + perp.1 * offset,
                ),
                observation.map,
            );
            DefensiveLineAssignment { unit_id, x, y }
        })
        .collect();
    Some(assignments)
}

fn main_steel_cluster_center(observation: &AiObservation) -> Option<(f32, f32)> {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let radius = (config::CC_RESOURCE_MAX_DIST_TILES + 1.5) * observation.map.tile_size as f32;
    let radius2 = squared(radius);
    steel_cluster_center(
        observation
            .resources
            .iter()
            .filter(|resource| dist2(resource.x, resource.y, own_base.0, own_base.1) <= radius2),
    )
}

fn steel_cluster_center<'a>(
    resources: impl IntoIterator<Item = &'a AiResourceSummary>,
) -> Option<(f32, f32)> {
    let steel: Vec<&AiResourceSummary> = resources
        .into_iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
        .collect();
    let count = steel.len().min(config::STEEL_PATCHES_PER_BASE as usize);
    if count == 0 {
        return None;
    }
    let (sum_x, sum_y) = steel
        .iter()
        .take(count)
        .fold((0.0, 0.0), |(sum_x, sum_y), resource| {
            (sum_x + resource.x, sum_y + resource.y)
        });
    Some((sum_x / count as f32, sum_y / count as f32))
}

fn normalized_direction(from: (f32, f32), to: (f32, f32)) -> Option<(f32, f32)> {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON || !len.is_finite() {
        return None;
    }
    Some((dx / len, dy / len))
}

fn clamp_to_map(point: (f32, f32), map: AiMapSummary) -> (f32, f32) {
    let tile_size = map.tile_size as f32;
    let min = tile_size * 0.5;
    let max_x = map.width as f32 * tile_size - min;
    let max_y = map.height as f32 * tile_size - min;
    (
        point.0.clamp(min, max_x.max(min)),
        point.1.clamp(min, max_y.max(min)),
    )
}

fn proxy_barracks_transit_site<F>(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
    kind: EntityKind,
    policy: ProxyBarracksPolicy,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let stats = config::building_stats(kind)?;
    if stats.foot_w > observation.map.width || stats.foot_h > observation.map.height {
        return None;
    }
    let search_radius_tiles = policy
        .search_radius_tiles
        .max(policy.min_enemy_base_distance_tiles)
        .max(0);
    let target_distance = policy.min_enemy_base_distance_tiles.max(0) as f32;
    let min_distance2 = squared(target_distance);
    let enemy_center = (
        enemy_base.start_tile.0 as f32 + 0.5,
        enemy_base.start_tile.1 as f32 + 0.5,
    );
    let own_center = (
        observation.own_start_tile.0 as f32 + 0.5,
        observation.own_start_tile.1 as f32 + 0.5,
    );
    let (sx, sy) = (
        enemy_base.start_tile.0 as i32,
        enemy_base.start_tile.1 as i32,
    );

    let mut best = None;
    for dy in -search_radius_tiles..=search_radius_tiles {
        for dx in -search_radius_tiles..=search_radius_tiles {
            if dx.abs().max(dy.abs()) > search_radius_tiles {
                continue;
            }
            let tx = sx + dx;
            let ty = sy + dy;
            if tx < 0 || ty < 0 {
                continue;
            }
            let (tx, ty) = (tx as u32, ty as u32);
            if tx > observation.map.width - stats.foot_w
                || ty > observation.map.height - stats.foot_h
            {
                continue;
            }

            let center_x = tx as f32 + stats.foot_w as f32 * 0.5;
            let center_y = ty as f32 + stats.foot_h as f32 * 0.5;
            let dx = center_x - enemy_center.0;
            let dy = center_y - enemy_center.1;
            let distance2 = dx * dx + dy * dy;
            if distance2 < min_distance2 || !placeable(kind, tx, ty) {
                continue;
            }
            let distance = distance2.sqrt();
            let distance_over_target = (distance - target_distance).max(0.0);
            let distance_band = (distance_over_target / PROXY_DISTANCE_BAND_TILES).floor() as i32;
            let candidate = ProxySiteCandidate {
                tile: (tx, ty),
                distance_band,
                distance_over_target,
                edge_distance_tiles: footprint_edge_distance_tiles(
                    (tx, ty),
                    &stats,
                    observation.map.width,
                    observation.map.height,
                ),
                scout_path_distance2: point_line_distance2(
                    (center_x, center_y),
                    own_center,
                    enemy_center,
                ),
            };
            if proxy_site_candidate_better(candidate, best) {
                best = Some(candidate);
            }
        }
    }

    best.map(|candidate| candidate.tile)
}

fn proxy_barracks_site_near_worker<F>(
    observation: &AiObservation,
    worker: &AiEntitySummary,
    kind: EntityKind,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let stats = config::building_stats(kind)?;
    if stats.foot_w > observation.map.width || stats.foot_h > observation.map.height {
        return None;
    }
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return None;
    }
    let worker_tile = (worker.x / tile_size, worker.y / tile_size);
    let sx = worker_tile.0.floor() as i32;
    let sy = worker_tile.1.floor() as i32;
    let mut best = None;

    for dy in -PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES..=PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES {
        for dx in -PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES..=PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES {
            if dx.abs().max(dy.abs()) > PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES {
                continue;
            }
            let tx = sx + dx;
            let ty = sy + dy;
            if tx < 0 || ty < 0 {
                continue;
            }
            let (tx, ty) = (tx as u32, ty as u32);
            if tx > observation.map.width - stats.foot_w
                || ty > observation.map.height - stats.foot_h
                || !placeable(kind, tx, ty)
            {
                continue;
            }
            let center_x = tx as f32 + stats.foot_w as f32 * 0.5;
            let center_y = ty as f32 + stats.foot_h as f32 * 0.5;
            let candidate = WorkerBuildSiteCandidate {
                tile: (tx, ty),
                worker_distance2: dist2(center_x, center_y, worker_tile.0, worker_tile.1),
            };
            if worker_build_site_candidate_better(candidate, best) {
                best = Some(candidate);
            }
        }
    }

    best.map(|candidate| candidate.tile)
}

#[derive(Clone, Copy, Debug)]
struct ProxySiteCandidate {
    tile: (u32, u32),
    distance_band: i32,
    distance_over_target: f32,
    edge_distance_tiles: u32,
    scout_path_distance2: f32,
}

fn proxy_site_candidate_better(
    candidate: ProxySiteCandidate,
    current: Option<ProxySiteCandidate>,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    if candidate.distance_band != current.distance_band {
        return candidate.distance_band < current.distance_band;
    }
    if candidate.edge_distance_tiles != current.edge_distance_tiles {
        return candidate.edge_distance_tiles < current.edge_distance_tiles;
    }
    match candidate
        .scout_path_distance2
        .total_cmp(&current.scout_path_distance2)
    {
        Ordering::Greater => return true,
        Ordering::Less => return false,
        Ordering::Equal => {}
    }
    match candidate
        .distance_over_target
        .total_cmp(&current.distance_over_target)
    {
        Ordering::Less => return true,
        Ordering::Greater => return false,
        Ordering::Equal => {}
    }
    candidate.tile < current.tile
}

#[derive(Clone, Copy, Debug)]
struct WorkerBuildSiteCandidate {
    tile: (u32, u32),
    worker_distance2: f32,
}

fn worker_build_site_candidate_better(
    candidate: WorkerBuildSiteCandidate,
    current: Option<WorkerBuildSiteCandidate>,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    match candidate
        .worker_distance2
        .total_cmp(&current.worker_distance2)
    {
        Ordering::Less => return true,
        Ordering::Greater => return false,
        Ordering::Equal => {}
    }
    candidate.tile < current.tile
}

fn footprint_edge_distance_tiles(
    tile: (u32, u32),
    stats: &config::BuildingStats,
    map_width: u32,
    map_height: u32,
) -> u32 {
    let left = tile.0;
    let top = tile.1;
    let right = map_width.saturating_sub(tile.0.saturating_add(stats.foot_w));
    let bottom = map_height.saturating_sub(tile.1.saturating_add(stats.foot_h));
    left.min(top).min(right).min(bottom)
}

fn point_line_distance2(point: (f32, f32), line_start: (f32, f32), line_end: (f32, f32)) -> f32 {
    let vx = line_end.0 - line_start.0;
    let vy = line_end.1 - line_start.1;
    let line_len2 = vx * vx + vy * vy;
    if line_len2 <= f32::EPSILON {
        return dist2(point.0, point.1, line_start.0, line_start.1);
    }
    let wx = point.0 - line_start.0;
    let wy = point.1 - line_start.1;
    let cross = wx * vy - wy * vx;
    cross * cross / line_len2
}

fn select_proxy_worker(
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
) -> Option<u32> {
    let workers_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Worker)
        .map(|entity| (entity.id, entity))
        .collect();
    if let Some(worker_id) = memory.proxy_worker_id {
        let worker = workers_by_id.get(&worker_id).copied()?;
        return (worker.state != AiEntityState::Build).then_some(worker.id);
    }

    let mut candidates = facts.idle_workers.clone();
    candidates.extend(facts.gathering_workers.iter().copied());
    candidates.extend(facts.build_capable_workers.iter().copied());
    candidates.sort_unstable();
    candidates.dedup();

    for worker_id in candidates {
        let Some(worker) = workers_by_id.get(&worker_id).copied() else {
            continue;
        };
        if worker.state == AiEntityState::Build {
            continue;
        }
        memory.proxy_worker_id = Some(worker.id);
        return Some(worker.id);
    }
    memory.proxy_worker_id = None;
    None
}

fn building_center(tile: (u32, u32), kind: EntityKind, tile_size: u32) -> Option<(f32, f32)> {
    let stats = config::building_stats(kind)?;
    let tile_size = tile_size as f32;
    Some((
        tile.0 as f32 * tile_size + stats.foot_w as f32 * tile_size * 0.5,
        tile.1 as f32 * tile_size + stats.foot_h as f32 * tile_size * 0.5,
    ))
}

fn wants_depot(facts: &AiFacts, profile: &AiProfile) -> bool {
    !facts.supply_capped
        && !facts.depot_in_progress
        && facts.free_supply <= profile.supply.free_supply_buffer
        && (facts.free_supply <= profile.supply.emergency_depot_threshold
            || !facts.production_buildings(EntityKind::Barracks).is_empty())
}

#[allow(clippy::too_many_arguments)]
fn try_build_expansion_city_centre<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    actions: &mut AiActionContext<'_>,
    builder_pools: &[&[u32]],
    profile: &AiProfile,
    recovery_active: bool,
    placeable: &mut F,
) -> Option<actions::BuildAction>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let expansion = active_expansion(observation, profile, recovery_active)?;
    let kind = EntityKind::CityCentre;
    config::building_stats(kind)?;
    if !rules::economy::build_requirement_met(kind, facts.complete_building_kinds()) {
        return None;
    }
    if facts.building_count(kind) >= expansion.target_city_centres {
        return None;
    }
    let counts = facts.building_counts(kind);
    if counts.incomplete + counts.intended >= profile.buildings.max_pending_per_kind {
        return None;
    }
    let (tile_x, tile_y) = expansion_city_centre_site(observation, expansion, kind, placeable)?;
    actions::try_build_at(actions, builder_pools, kind, tile_x, tile_y)
}

fn footprint_top_left_for_center(center_tile: (u32, u32), kind: EntityKind) -> Option<(u32, u32)> {
    let stats = config::building_stats(kind)?;
    Some((
        center_tile.0.saturating_sub(stats.foot_w / 2),
        center_tile.1.saturating_sub(stats.foot_h / 2),
    ))
}

fn expansion_city_centre_site<F>(
    observation: &AiObservation,
    expansion: ExpansionPolicy,
    kind: EntityKind,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let stats = config::building_stats(kind)?;
    let resources = expansion_candidate_resources(observation);
    if resources.is_empty() {
        return None;
    }
    let mut best = None;
    for anchor in expansion_anchor_tiles(observation, &resources) {
        let cluster_resources =
            expansion_cluster_resources_for_anchor(observation, anchor, &resources);
        if cluster_resources.is_empty() {
            continue;
        }
        let required_steel = cluster_resources
            .iter()
            .filter(|resource| resource.kind == EntityKind::Steel)
            .count()
            .min(config::STEEL_PATCHES_PER_BASE as usize);
        let required_oil = cluster_resources
            .iter()
            .filter(|resource| resource.kind == EntityKind::Oil)
            .count()
            .min(config::OIL_PATCHES_PER_BASE as usize);
        let mut seen = BTreeSet::new();
        let Some(start_tile) = footprint_top_left_for_center(anchor, kind) else {
            continue;
        };
        let (sx, sy) = (start_tile.0 as i32, start_tile.1 as i32);
        for dy in -expansion.search_radius_tiles..=expansion.search_radius_tiles {
            for dx in -expansion.search_radius_tiles..=expansion.search_radius_tiles {
                if dx.abs().max(dy.abs()) > expansion.search_radius_tiles {
                    continue;
                }
                let tx = sx + dx;
                let ty = sy + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let (tx, ty) = (tx as u32, ty as u32);
                if tx > observation.map.width.saturating_sub(stats.foot_w)
                    || ty > observation.map.height.saturating_sub(stats.foot_h)
                    || !seen.insert((tx, ty))
                    || !placeable(kind, tx, ty)
                {
                    continue;
                }
                let Some(candidate) =
                    expansion_site_candidate(observation, kind, tx, ty, &cluster_resources)
                else {
                    continue;
                };
                if candidate.steel_in_range < required_steel
                    || candidate.oil_in_range < required_oil
                {
                    continue;
                }
                if expansion_site_candidate_better(candidate, best) {
                    best = Some(candidate);
                }
            }
        }
    }

    best.map(|candidate: ExpansionSiteCandidate| candidate.tile)
}

fn expansion_candidate_resources(observation: &AiObservation) -> Vec<&AiResourceSummary> {
    let start_resource_radius =
        (config::CC_RESOURCE_MAX_DIST_TILES + 1.5) * observation.map.tile_size as f32;
    let start_resource_radius2 = squared(start_resource_radius);
    observation
        .resources
        .iter()
        .filter(|resource| matches!(resource.kind, EntityKind::Steel | EntityKind::Oil))
        .filter(|resource| resource.remaining > 0)
        .filter(|resource| {
            !resource_is_near_player_start(observation, resource, start_resource_radius2)
        })
        .collect()
}

fn expansion_anchor_tiles(
    observation: &AiObservation,
    resources: &[&AiResourceSummary],
) -> Vec<(u32, u32)> {
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return Vec::new();
    }
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let map_center_tiles = (
        observation.map.width as f32 * 0.5,
        observation.map.height as f32 * 0.5,
    );
    let mut anchors: Vec<((u32, u32), f32, u32)> = Vec::new();

    for resource in resources
        .iter()
        .copied()
        .filter(|resource| resource.kind == EntityKind::Steel)
    {
        let Some(tile) =
            estimated_expansion_center_tile(observation, resource, map_center_tiles, tile_size)
        else {
            continue;
        };
        let center = tile_center(tile, observation.map.tile_size);
        let distance2 = dist2(center.0, center.1, own.0, own.1);
        anchors.push((tile, distance2, resource.id));
    }

    anchors.sort_by(
        |(left_tile, left_distance, left_id), (right_tile, right_distance, right_id)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_id.cmp(right_id))
                .then_with(|| left_tile.cmp(right_tile))
        },
    );
    anchors.dedup_by_key(|(tile, _, _)| *tile);
    anchors.into_iter().map(|(tile, _, _)| tile).collect()
}

fn expansion_cluster_resources_for_anchor<'a>(
    observation: &AiObservation,
    anchor: (u32, u32),
    resources: &[&'a AiResourceSummary],
) -> Vec<&'a AiResourceSummary> {
    let center = tile_center(anchor, observation.map.tile_size);
    let radius = (config::MINING_CC_RANGE_TILES + 2.0) * observation.map.tile_size as f32;
    let radius2 = squared(radius);
    resources
        .iter()
        .copied()
        .filter(|resource| dist2(resource.x, resource.y, center.0, center.1) <= radius2)
        .collect()
}

fn estimated_expansion_center_tile(
    observation: &AiObservation,
    resource: &AiResourceSummary,
    map_center_tiles: (f32, f32),
    tile_size: f32,
) -> Option<(u32, u32)> {
    let resource_tile = (resource.x / tile_size, resource.y / tile_size);
    let dir = (
        map_center_tiles.0 - resource_tile.0,
        map_center_tiles.1 - resource_tile.1,
    );
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
    if len <= f32::EPSILON {
        return None;
    }
    let estimated_center = (
        resource_tile.0 - dir.0 / len * config::STEEL_BLOCK_DIST_TILES,
        resource_tile.1 - dir.1 / len * config::STEEL_BLOCK_DIST_TILES,
    );
    if !estimated_center.0.is_finite() || !estimated_center.1.is_finite() {
        return None;
    }
    Some((
        estimated_center
            .0
            .round()
            .clamp(0.0, observation.map.width.saturating_sub(1) as f32) as u32,
        estimated_center
            .1
            .round()
            .clamp(0.0, observation.map.height.saturating_sub(1) as f32) as u32,
    ))
}

#[derive(Clone, Copy, Debug)]
struct ExpansionSiteCandidate {
    tile: (u32, u32),
    steel_in_range: usize,
    oil_in_range: usize,
    max_resource_distance2: f32,
    sum_resource_distance2: f32,
    own_distance2: f32,
    approach_exposure: Option<f32>,
}

fn expansion_site_candidate(
    observation: &AiObservation,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
    resources: &[&AiResourceSummary],
) -> Option<ExpansionSiteCandidate> {
    let (cx, cy) = building_center((tile_x, tile_y), kind, observation.map.tile_size)?;
    let max_dist = config::MINING_CC_RANGE_TILES * observation.map.tile_size as f32;
    let max_dist2 = squared(max_dist);
    let mut steel_in_range = 0usize;
    let mut oil_in_range = 0usize;
    let mut max_resource_distance2 = 0.0f32;
    let mut sum_resource_distance2 = 0.0f32;

    for resource in resources {
        let distance2 = dist2(cx, cy, resource.x, resource.y);
        if distance2 > max_dist2 {
            continue;
        }
        match resource.kind {
            EntityKind::Steel => steel_in_range += 1,
            EntityKind::Oil => oil_in_range += 1,
            _ => {}
        }
        max_resource_distance2 = max_resource_distance2.max(distance2);
        sum_resource_distance2 += distance2;
    }
    if steel_in_range == 0 && oil_in_range == 0 {
        return None;
    }
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let own_distance2 = dist2(cx, cy, own.0, own.1);
    let enemy_distance2 = nearest_enemy_start_distance2(observation, cx, cy);
    Some(ExpansionSiteCandidate {
        tile: (tile_x, tile_y),
        steel_in_range,
        oil_in_range,
        max_resource_distance2,
        sum_resource_distance2,
        own_distance2,
        approach_exposure: expansion_approach_exposure(own_distance2, enemy_distance2),
    })
}

fn expansion_site_candidate_better(
    candidate: ExpansionSiteCandidate,
    current: Option<ExpansionSiteCandidate>,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    candidate
        .oil_in_range
        .cmp(&current.oil_in_range)
        .then_with(|| candidate.steel_in_range.cmp(&current.steel_in_range))
        .then_with(|| {
            expansion_approach_exposure_order(
                candidate.approach_exposure,
                current.approach_exposure,
            )
        })
        .then_with(|| {
            current
                .max_resource_distance2
                .total_cmp(&candidate.max_resource_distance2)
        })
        .then_with(|| {
            current
                .sum_resource_distance2
                .total_cmp(&candidate.sum_resource_distance2)
        })
        .then_with(|| current.own_distance2.total_cmp(&candidate.own_distance2))
        .then_with(|| current.tile.cmp(&candidate.tile))
        == Ordering::Greater
}

fn expansion_approach_exposure(own_distance2: f32, enemy_distance2: Option<f32>) -> Option<f32> {
    enemy_distance2
        .filter(|distance2| distance2.is_finite() && *distance2 > f32::EPSILON)
        .map(|distance2| own_distance2 / distance2)
}

fn expansion_approach_exposure_order(candidate: Option<f32>, current: Option<f32>) -> Ordering {
    match (candidate, current) {
        (Some(candidate), Some(current)) => current.total_cmp(&candidate),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn nearest_enemy_start_distance2(observation: &AiObservation, x: f32, y: f32) -> Option<f32> {
    observation
        .players
        .iter()
        .filter(|player| player.id != observation.player_id && player.is_alive)
        .map(|player| {
            let center = tile_center(player.start_tile, observation.map.tile_size);
            dist2(x, y, center.0, center.1)
        })
        .min_by(|left, right| left.total_cmp(right))
}

fn resource_is_near_player_start(
    observation: &AiObservation,
    resource: &AiResourceSummary,
    radius2: f32,
) -> bool {
    observation.players.iter().any(|player| {
        let center = tile_center(player.start_tile, observation.map.tile_size);
        dist2(resource.x, resource.y, center.0, center.1) <= radius2
    })
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
    let build_search = build_search_for_kind(build_search, kind);
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

fn build_search_for_kind(
    mut build_search: ai_shared::BuildSearch,
    kind: EntityKind,
) -> ai_shared::BuildSearch {
    if kind == EntityKind::Factory {
        build_search.prefer_away_from_center = false;
        build_search.prefer_toward_center = true;
    }
    build_search
}

fn desired_oil_workers(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
    target_steel_workers: usize,
) -> usize {
    let worker_policy = active_worker_policy(profile, recovery_active);
    let resource_policy = active_resource_policy(profile, recovery_active);

    if worker_policy.extra_oil_workers == 0 {
        return 0;
    }
    if expansion_blocks_tech_path(observation, facts, profile, recovery_active) {
        return 0;
    }
    let current_steel_workers = resource_worker_counts(observation)
        .get(&EntityKind::Steel)
        .copied()
        .unwrap_or(0);
    let oil_steel_floor = if resource_policy.oil_after_full_steel_saturation {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResourceGoal {
    steel: u32,
    oil: u32,
}

fn next_tank_resource_goal(facts: &AiFacts, profile: &AiProfile) -> Option<ResourceGoal> {
    if profile.production.save_for_first_tech_unit != Some(EntityKind::Tank) {
        return None;
    }
    let kind = if facts.complete_building_count(EntityKind::TrainingCentre) == 0 {
        EntityKind::TrainingCentre
    } else if facts.complete_building_count(EntityKind::Factory) == 0 {
        EntityKind::Factory
    } else if facts.complete_building_count(EntityKind::Steelworks) == 0 {
        EntityKind::Steelworks
    } else {
        EntityKind::Tank
    };
    let (steel, oil) = rules::economy::cost(kind);
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

fn should_save_for_first_tech_unit(facts: &AiFacts, production: ProductionPolicy) -> bool {
    let Some(unit) = production.save_for_first_tech_unit else {
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

fn should_save_for_required_tech_building(
    facts: &AiFacts,
    required_tech_path: &[EntityKind],
    production: ProductionPolicy,
) -> bool {
    let Some(unit) = production.save_for_first_tech_unit else {
        return false;
    };
    if facts.unit_count(unit) > 0 {
        return false;
    }
    let Some(producer) = producer_for_unit(unit) else {
        return false;
    };
    if facts.building_count(producer) == 0 {
        return required_tech_path.contains(&producer)
            && rules::economy::build_requirement_met(producer, facts.complete_building_kinds());
    }
    if rules::economy::train_requirement_met(unit, facts.complete_building_kinds()) {
        return false;
    }
    required_tech_path.iter().copied().any(|kind| {
        facts.building_count(kind) == 0
            && rules::economy::build_requirement_met(kind, facts.complete_building_kinds())
    })
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
    order.retain(|kind| *kind != EntityKind::CityCentre);
    order
}

fn production_uses_building(production: ProductionPolicy, building: EntityKind) -> bool {
    production
        .unit_priorities
        .iter()
        .copied()
        .any(|unit| producer_for_unit(unit) == Some(building))
}

fn unit_counts_for_priorities(
    observation: &AiObservation,
    facts: &AiFacts,
    unit_priorities: &[EntityKind],
) -> Vec<(EntityKind, usize)> {
    let mut counts: BTreeMap<EntityKind, usize> = unit_priorities
        .iter()
        .copied()
        .map(|unit| (unit, facts.unit_count(unit)))
        .collect();
    for building in observation.owned.iter().filter(|entity| entity.is_complete) {
        let Some(kind) = building.production_kind else {
            continue;
        };
        if !unit_priorities.contains(&kind) {
            continue;
        }
        let queued = building.production_queue_len.unwrap_or(0);
        *counts.entry(kind).or_default() += queued;
    }
    unit_priorities
        .iter()
        .copied()
        .map(|unit| (unit, counts.get(&unit).copied().unwrap_or(0)))
        .collect()
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

fn is_rifle_raid_policy(attack: AttackPolicy) -> bool {
    matches!(attack.unit_kinds, [EntityKind::Rifleman]) && attack.required_unit.is_none()
}

fn select_rifle_raid_units(observation: &AiObservation) -> Vec<u32> {
    let mut units: Vec<u32> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Rifleman && entity.is_complete)
        .filter(|entity| {
            entity.free_for_combat
                || matches!(entity.state, AiEntityState::Move | AiEntityState::Attack)
        })
        .map(|entity| entity.id)
        .collect();
    units.sort_unstable();
    units
}

fn active_rifle_raid_units(observation: &AiObservation, raid_units: &[u32]) -> Vec<u32> {
    let raid_ids: BTreeSet<u32> = raid_units.iter().copied().collect();
    let mut units: Vec<u32> = observation
        .owned
        .iter()
        .filter(|entity| raid_ids.contains(&entity.id))
        .filter(|entity| matches!(entity.state, AiEntityState::Move | AiEntityState::Attack))
        .map(|entity| entity.id)
        .collect();
    units.sort_unstable();
    units
}

fn rifle_raid_unit_target(
    observation: &AiObservation,
    raid_units: &[u32],
    excluded_targets: &BTreeSet<u32>,
) -> Option<u32> {
    let center = group_center(observation, raid_units)?;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| !excluded_targets.contains(&enemy.id))
        .filter(|enemy| enemy.kind.is_unit())
        .map(|enemy| {
            (
                enemy.id,
                rifle_raid_unit_priority(enemy.kind),
                dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn rifle_raid_unit_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Worker => 0,
        EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::AtTeam => 1,
        EntityKind::Tank => 2,
        _ => 3,
    }
}

fn rifle_raid_building_fallback_target(
    observation: &AiObservation,
    raid_units: &[u32],
    excluded_targets: &BTreeSet<u32>,
    enemy_base: EnemyBaseFact,
) -> Option<u32> {
    let raid_ids: BTreeSet<u32> = raid_units.iter().copied().collect();
    let fallback_center =
        enemy_main_steel_center(observation, enemy_base).unwrap_or((enemy_base.x, enemy_base.y));
    let radius_px = RIFLE_RAID_STEEL_LINE_RADIUS_TILES * observation.map.tile_size as f32;
    let radius2 = squared(radius_px);
    let raider_ready_to_burn_buildings = observation.owned.iter().any(|entity| {
        raid_ids.contains(&entity.id)
            && !matches!(entity.state, AiEntityState::Move)
            && dist2(entity.x, entity.y, fallback_center.0, fallback_center.1) <= radius2
    });
    if !raider_ready_to_burn_buildings {
        return None;
    }

    let center = group_center(observation, raid_units)?;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| !excluded_targets.contains(&enemy.id))
        .filter(|enemy| enemy.kind.is_building())
        .map(|enemy| (enemy.id, dist2(center.0, center.1, enemy.x, enemy.y)))
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _)| id)
}

fn enemy_main_steel_center(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
) -> Option<(f32, f32)> {
    let tile_size = observation.map.tile_size as f32;
    let search_radius_px = (config::CC_RESOURCE_MAX_DIST_TILES + 0.5) * tile_size;
    let search_radius2 = squared(search_radius_px);
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut count = 0usize;
    for resource in observation
        .resources
        .iter()
        .filter(|r| r.kind == EntityKind::Steel)
    {
        if dist2(resource.x, resource.y, enemy_base.x, enemy_base.y) > search_radius2 {
            continue;
        }
        sum_x += resource.x;
        sum_y += resource.y;
        count += 1;
    }
    (count > 0).then(|| (sum_x / count as f32, sum_y / count as f32))
}

fn rifle_raid_move_target(observation: &AiObservation, enemy_base: EnemyBaseFact) -> (f32, f32) {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let dx = enemy_base.x - own_base.0;
    let dy = enemy_base.y - own_base.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON || !len.is_finite() {
        return (enemy_base.x, enemy_base.y);
    }
    let deepen_px = RIFLE_RAID_DEEPEN_TILES * observation.map.tile_size as f32;
    let max = observation.map.width as f32 * observation.map.tile_size as f32 - 0.01;
    (
        (enemy_base.x + dx / len * deepen_px).clamp(0.0, max),
        (enemy_base.y + dy / len * deepen_px).clamp(0.0, max),
    )
}

fn group_center(observation: &AiObservation, unit_ids: &[u32]) -> Option<(f32, f32)> {
    let ids: BTreeSet<u32> = unit_ids.iter().copied().collect();
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut count = 0usize;
    for entity in observation
        .owned
        .iter()
        .filter(|entity| ids.contains(&entity.id))
    {
        sum_x += entity.x;
        sum_y += entity.y;
        count += 1;
    }
    (count > 0).then(|| (sum_x / count as f32, sum_y / count as f32))
}

fn local_defense_target(observation: &AiObservation) -> Option<u32> {
    let geometry = LocalDefenseGeometry::from_observation(observation);
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() || enemy.kind.is_building())
        .filter_map(|enemy| {
            geometry
                .contains(enemy)
                .then_some((enemy.id, geometry.base_dist2(enemy)))
        })
        .min_by(|(left_id, left_dist), (right_id, right_dist)| {
            left_dist
                .total_cmp(right_dist)
                .then_with(|| left_id.cmp(right_id))
        })
        .map(|(id, _)| id)
}

fn local_defense_units(observation: &AiObservation, ready_units: &[u32]) -> Vec<u32> {
    let geometry = LocalDefenseGeometry::from_observation(observation);
    let ready: BTreeSet<u32> = ready_units.iter().copied().collect();
    observation
        .owned
        .iter()
        .filter(|entity| ready.contains(&entity.id))
        .filter(|entity| geometry.contains(entity))
        .map(|entity| entity.id)
        .collect()
}

struct LocalDefenseGeometry {
    own_base: (f32, f32),
    base_radius2: f32,
    resource_radius2: f32,
    worker_radius2: f32,
    home_resources: Vec<(f32, f32)>,
    workers: Vec<(f32, f32)>,
}

impl LocalDefenseGeometry {
    fn from_observation(observation: &AiObservation) -> Self {
        let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
        let tile_size = observation.map.tile_size as f32;
        let base_radius2 = squared(LOCAL_DEFENSE_RADIUS_TILES * tile_size);
        let resource_radius2 = squared(RESOURCE_LINE_DEFENSE_RADIUS_TILES * tile_size);
        let worker_radius2 = squared(WORKER_DEFENSE_RADIUS_TILES * tile_size);
        let home_resource_radius2 = squared((config::CC_RESOURCE_MAX_DIST_TILES + 1.5) * tile_size);
        let home_resources = observation
            .resources
            .iter()
            .filter(|resource| {
                matches!(resource.kind, EntityKind::Steel | EntityKind::Oil)
                    && dist2(resource.x, resource.y, own_base.0, own_base.1)
                        <= home_resource_radius2
            })
            .map(|resource| (resource.x, resource.y))
            .collect();
        let workers = observation
            .owned
            .iter()
            .filter(|entity| entity.kind == EntityKind::Worker)
            .map(|worker| (worker.x, worker.y))
            .collect();

        Self {
            own_base,
            base_radius2,
            resource_radius2,
            worker_radius2,
            home_resources,
            workers,
        }
    }

    fn contains(&self, entity: &AiEntitySummary) -> bool {
        self.base_dist2(entity) <= self.base_radius2
            || self
                .home_resources
                .iter()
                .any(|(x, y)| dist2(entity.x, entity.y, *x, *y) <= self.resource_radius2)
            || self
                .workers
                .iter()
                .any(|(x, y)| dist2(entity.x, entity.y, *x, *y) <= self.worker_radius2)
    }

    fn base_dist2(&self, entity: &AiEntitySummary) -> f32 {
        dist2(entity.x, entity.y, self.own_base.0, self.own_base.1)
    }
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

fn dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

fn squared(value: f32) -> f32 {
    value * value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ai_core::observation::{
        AiBuildIntent, AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiPlayerSummary,
        AiResourceSummary,
    };
    use crate::game::ai_core::profiles::{
        RIFLE_FLOOD_FAST, RIFLE_FLOOD_FULL_SATURATION, STEEL_EXPANSION_TANKS, TECH_TO_TANKS,
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
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn worker_at(id: u32, state: AiEntityState, x: f32, y: f32) -> AiEntitySummary {
        let mut worker = worker(id, state);
        worker.x = x;
        worker.y = y;
        worker
    }

    fn steel_worker(id: u32, node: u32) -> AiEntitySummary {
        gathering_worker(id, node)
    }

    fn gathering_worker(id: u32, node: u32) -> AiEntitySummary {
        let mut worker = worker(id, AiEntityState::Gather);
        worker.latched_node = Some(node);
        worker
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
        building_at(id, kind, queue_len, 0.0, 0.0)
    }

    fn building_training(id: u32, kind: EntityKind, unit: EntityKind) -> AiEntitySummary {
        let mut building = building(id, kind, Some(3));
        building.production_kind = Some(unit);
        building
    }

    fn building_at(
        id: u32,
        kind: EntityKind,
        queue_len: Option<usize>,
        x: f32,
        y: f32,
    ) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x,
            y,
            state: queue_len
                .filter(|queue| *queue > 0)
                .map(|_| AiEntityState::Train)
                .unwrap_or(AiEntityState::Idle),
            is_complete: true,
            production_queue_len: queue_len,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn combat(id: u32, kind: EntityKind) -> AiEntitySummary {
        combat_at(id, kind, 0.0, 0.0)
    }

    fn combat_at(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x,
            y,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: true,
        }
    }

    fn enemy(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 2,
            kind,
            x,
            y,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
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

    fn with_expansion_resources(mut observation: AiObservation) -> AiObservation {
        let ts = observation.map.tile_size as f32;
        for i in 0..18 {
            observation.resources.push(resource(
                300 + i,
                EntityKind::Steel,
                (21.5 + (i % 6) as f32) * ts,
                (31.5 + (i / 6) as f32) * ts,
            ));
        }
        for i in 0..3 {
            observation.resources.push(resource(
                400 + i,
                EntityKind::Oil,
                (16.5 + i as f32) * ts,
                38.5 * ts,
            ));
        }
        observation.resources.sort_by_key(|resource| resource.id);
        observation
    }

    fn with_enemy_main_resources(mut observation: AiObservation) -> AiObservation {
        observation.resources.extend(base_site_resources(
            300,
            enemy_start_tile(&observation),
            observation.map.width,
        ));
        observation.resources.sort_by_key(|resource| resource.id);
        observation
    }

    fn enemy_base_fact(observation: &AiObservation) -> EnemyBaseFact {
        let start_tile = enemy_start_tile(observation);
        let (x, y) = tile_center(start_tile, observation.map.tile_size);
        EnemyBaseFact {
            player_id: 2,
            start_tile,
            x,
            y,
        }
    }

    fn base_site_resources(
        first_id: u32,
        site: (u32, u32),
        map_size: u32,
    ) -> Vec<AiResourceSummary> {
        let ts = config::TILE_SIZE as f32;
        let hx = site.0 as f32 + 0.5;
        let hy = site.1 as f32 + 0.5;
        let map_center = map_size as f32 * 0.5;
        let base_angle = (map_center - hy).atan2(map_center - hx);

        let block_cx = hx + config::STEEL_BLOCK_DIST_TILES * base_angle.cos();
        let block_cy = hy + config::STEEL_BLOCK_DIST_TILES * base_angle.sin();
        let perp_x = -base_angle.sin();
        let perp_y = base_angle.cos();
        let rows = config::STEEL_PATCHES_PER_BASE.div_ceil(6);
        let row_center = (rows - 1) as f32 / 2.0;
        let mut resources = Vec::new();
        for i in 0..config::STEEL_PATCHES_PER_BASE {
            let col = (i % 6) as f32;
            let row = (i / 6) as f32;
            let off_x = col - 2.5;
            let off_y = row - row_center;
            resources.push(resource(
                first_id + i,
                EntityKind::Steel,
                (block_cx + off_x * perp_x + off_y * base_angle.cos()) * ts,
                (block_cy + off_x * perp_y + off_y * base_angle.sin()) * ts,
            ));
        }

        let oil_angle = base_angle + std::f32::consts::FRAC_PI_2;
        let oil_perp_x = -oil_angle.sin();
        let oil_perp_y = oil_angle.cos();
        let oil_cx = hx + config::OIL_DIST_TILES * oil_angle.cos();
        let oil_cy = hy + config::OIL_DIST_TILES * oil_angle.sin();
        for (i, (off_x, off_y)) in [(-0.5, -0.5), (0.5, -0.5), (0.0, 0.5)]
            .into_iter()
            .enumerate()
        {
            resources.push(resource(
                first_id + config::STEEL_PATCHES_PER_BASE + i as u32,
                EntityKind::Oil,
                (oil_cx + off_x * oil_perp_x + off_y * oil_angle.cos()) * ts,
                (oil_cy + off_x * oil_perp_y + off_y * oil_angle.sin()) * ts,
            ));
        }
        resources
    }

    fn expansion_resource_counts_for_site(
        observation: &AiObservation,
        site: (u32, u32),
    ) -> (usize, usize) {
        let (cx, cy) = building_center(site, EntityKind::CityCentre, observation.map.tile_size)
            .expect("city centre should have a center");
        let max_dist = config::MINING_CC_RANGE_TILES * observation.map.tile_size as f32;
        let max_dist2 = squared(max_dist);
        let mut steel = 0usize;
        let mut oil = 0usize;
        for resource in expansion_candidate_resources(observation) {
            if dist2(cx, cy, resource.x, resource.y) > max_dist2 {
                continue;
            }
            match resource.kind {
                EntityKind::Steel => steel += 1,
                EntityKind::Oil => oil += 1,
                _ => {}
            }
        }
        (steel, oil)
    }

    fn decide(
        observation: &AiObservation,
        profile: &'static AiProfile,
        memory: &mut AiDecisionMemory,
    ) -> AiDecision {
        let width = observation.map.width;
        let height = observation.map.height;
        decide_profile(
            observation,
            profile,
            memory,
            ai_shared::BuildSearch {
                min_radius: 0,
                max_radius: 0,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            |_, tx, ty| tx < width && ty < height,
        )
    }

    fn enemy_start_tile(observation: &AiObservation) -> (u32, u32) {
        observation
            .players
            .iter()
            .find(|player| player.id != observation.player_id)
            .expect("test observation should have an enemy")
            .start_tile
    }

    fn footprint_center_tiles(tile: (u32, u32), kind: EntityKind) -> (f32, f32) {
        let stats = config::building_stats(kind).expect("test kind should be a building");
        (
            tile.0 as f32 + stats.foot_w as f32 * 0.5,
            tile.1 as f32 + stats.foot_h as f32 * 0.5,
        )
    }

    fn proxy_distance_to_enemy_tiles(observation: &AiObservation, tile: (u32, u32)) -> f32 {
        let enemy = enemy_start_tile(observation);
        let enemy_center = (enemy.0 as f32 + 0.5, enemy.1 as f32 + 0.5);
        let barracks_center = footprint_center_tiles(tile, EntityKind::Barracks);
        let dx = barracks_center.0 - enemy_center.0;
        let dy = barracks_center.1 - enemy_center.1;
        (dx * dx + dy * dy).sqrt()
    }

    fn point_distance_to_enemy_tiles(observation: &AiObservation, point: (f32, f32)) -> f32 {
        let enemy = enemy_start_tile(observation);
        let enemy_center = (enemy.0 as f32 + 0.5, enemy.1 as f32 + 0.5);
        let dx = point.0 - enemy_center.0;
        let dy = point.1 - enemy_center.1;
        (dx * dx + dy * dy).sqrt()
    }

    fn point_edge_distance_tiles(observation: &AiObservation, point: (f32, f32)) -> f32 {
        point
            .0
            .min(point.1)
            .min(observation.map.width as f32 - point.0)
            .min(observation.map.height as f32 - point.1)
    }

    fn point_scout_path_distance_tiles(observation: &AiObservation, point: (f32, f32)) -> f32 {
        let own_center = (
            observation.own_start_tile.0 as f32 + 0.5,
            observation.own_start_tile.1 as f32 + 0.5,
        );
        let enemy = enemy_start_tile(observation);
        let enemy_center = (enemy.0 as f32 + 0.5, enemy.1 as f32 + 0.5);
        point_line_distance2(point, own_center, enemy_center).sqrt()
    }

    fn assert_hidden_proxy_point(observation: &AiObservation, point: (f32, f32)) {
        let distance = point_distance_to_enemy_tiles(observation, point);
        assert!(
            distance >= 18.0,
            "proxy transit target should not be within 18 tiles of the enemy base, got {distance}"
        );
        assert!(
            distance < 20.0,
            "proxy transit target should stay close to the requested 18-tile ring, got {distance}"
        );
        let edge_distance = point_edge_distance_tiles(observation, point);
        assert!(
            edge_distance <= 2.0,
            "proxy transit target should hug a map edge, got {edge_distance} tiles from the edge"
        );
        let scout_path_distance = point_scout_path_distance_tiles(observation, point);
        assert!(
            scout_path_distance >= 8.0,
            "proxy transit target should be off the direct scouting line, got {scout_path_distance}"
        );
    }

    fn assert_hidden_proxy_site(observation: &AiObservation, tile: (u32, u32)) {
        let distance = proxy_distance_to_enemy_tiles(observation, tile);
        assert!(
            distance >= 18.0,
            "proxy barracks target should not be within 18 tiles of the enemy base, got {distance}"
        );
        assert!(
            distance < 20.0,
            "proxy barracks target should stay close to the requested 18-tile ring, got {distance}"
        );
        let stats = config::building_stats(EntityKind::Barracks).expect("barracks stats");
        let edge_distance = footprint_edge_distance_tiles(
            tile,
            &stats,
            observation.map.width,
            observation.map.height,
        );
        assert!(
            edge_distance <= 1,
            "proxy barracks target should be near a map edge, got {edge_distance} tiles"
        );
        let center = footprint_center_tiles(tile, EntityKind::Barracks);
        let scout_path_distance = point_scout_path_distance_tiles(observation, center);
        assert!(
            scout_path_distance >= 8.0,
            "proxy barracks target should be off the direct scouting line, got {scout_path_distance}"
        );
        assert_ne!(
            tile,
            (observation.map.width / 2, observation.map.height / 2),
            "proxy barracks should no longer use the map center"
        );
    }

    #[test]
    fn fast_flood_sends_proxy_worker_before_barracks_is_affordable() {
        let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
        owned.extend((0..4).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: config::STARTING_STEEL,
                oil: 0,
                supply_used: config::STARTING_WORKERS,
                supply_cap: config::CITY_CENTRE_SUPPLY,
            },
            owned,
        );

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        assert!(decision.intents.iter().any(|intent| {
            matches!(
                intent,
                AiIntent::Move { units } if units.as_slice() == [20]
            )
        }));
        let move_target = decision
            .commands
            .iter()
            .find_map(|command| match command {
                Command::Move { units, x, y } if units.as_slice() == [20] => Some((*x, *y)),
                _ => None,
            })
            .expect("proxy worker should receive a move command");
        let tile_size = observation.map.tile_size as f32;
        let move_target_tiles = (move_target.0 / tile_size, move_target.1 / tile_size);
        assert_hidden_proxy_point(&observation, move_target_tiles);
        assert!(
            point_distance_to_enemy_tiles(&observation, move_target_tiles)
                < point_distance_to_enemy_tiles(
                    &observation,
                    (
                        observation.own_start_tile.0 as f32 + 0.5,
                        observation.own_start_tile.1 as f32 + 0.5,
                    ),
                )
        );
        assert!(
            !decision.commands.iter().any(|command| {
                matches!(
                    command,
                    Command::Build { building, .. }
                        if *building == EntityKind::Barracks
                )
            }),
            "the proxy worker should move out before the barracks is affordable"
        );
        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }));
    }

    #[test]
    fn fast_flood_stops_worker_training_after_one_extra_worker() {
        let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
        owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: 75,
                oil: 0,
                supply_used: 5,
                supply_cap: config::CITY_CENTRE_SUPPLY,
            },
            owned,
        );

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }));
    }

    #[test]
    fn fast_flood_initial_affordable_proxy_uses_hidden_edge_target() {
        let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
        owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: 150,
                oil: 0,
                supply_used: 5,
                supply_cap: config::CITY_CENTRE_SUPPLY,
            },
            owned,
        );

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        let proxy_builds: Vec<_> = decision
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::Build {
                    worker,
                    building,
                    tile_x,
                    tile_y,
                } if *building == EntityKind::Barracks => Some((*worker, (*tile_x, *tile_y))),
                _ => None,
            })
            .collect();

        assert_eq!(
            proxy_builds.len(),
            1,
            "fast rush should send exactly one worker to build the proxy barracks"
        );
        assert_eq!(proxy_builds[0].0, 20);
        assert_hidden_proxy_site(&observation, proxy_builds[0].1);
    }

    #[test]
    fn fast_flood_builds_proxy_barracks_with_reserved_worker_once_affordable() {
        let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
        let tile_size = config::TILE_SIZE as f32;
        let worker_tile = (30.5, 20.5);
        owned.push(worker_at(
            20,
            AiEntityState::Move,
            worker_tile.0 * tile_size,
            worker_tile.1 * tile_size,
        ));
        owned.extend((0..4).map(|i| worker(21 + i, AiEntityState::Gather)));
        let observation = observation(
            AiEconomy {
                steel: 150,
                oil: 0,
                supply_used: 5,
                supply_cap: config::CITY_CENTRE_SUPPLY,
            },
            owned,
        );
        let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
        memory.proxy_worker_id = Some(20);

        let decision = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Barracks
        }));
        let proxy_builds: Vec<_> = decision
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::Build {
                    worker,
                    building,
                    tile_x,
                    tile_y,
                } if *building == EntityKind::Barracks => Some((*worker, (*tile_x, *tile_y))),
                _ => None,
            })
            .collect();

        assert_eq!(
            proxy_builds.len(),
            1,
            "fast rush should send exactly one worker to build the proxy barracks"
        );
        assert_eq!(proxy_builds[0].0, 20);
        let build_center = footprint_center_tiles(proxy_builds[0].1, EntityKind::Barracks);
        assert!(
            dist2(build_center.0, build_center.1, worker_tile.0, worker_tile.1) <= squared(1.0),
            "committed proxy worker should build near its current position"
        );
        assert!(decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Build { worker, building, .. }
                    if *worker == 20
                        && *building == EntityKind::Barracks
            )
        }));
        assert!(
            !decision
                .commands
                .iter()
                .any(|command| matches!(command, Command::Move { units, .. } if units.as_slice() == [20])),
            "the reserved proxy worker should build instead of receiving another move once affordable"
        );
    }

    #[test]
    fn fast_flood_does_not_replace_missing_proxy_worker() {
        let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
        owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: 150,
                oil: 0,
                supply_used: 5,
                supply_cap: config::CITY_CENTRE_SUPPLY,
            },
            owned,
        );
        let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
        memory.proxy_worker_id = Some(999);

        let decision = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

        assert!(!decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Build { building, .. }
                    if *building == EntityKind::Barracks
            )
        }));
        assert!(
            !decision
                .commands
                .iter()
                .any(|command| matches!(command, Command::Move { units, .. } if units.len() == 1)),
            "fast rush should not send a replacement proxy worker after committing one"
        );
    }

    #[test]
    fn fast_flood_spends_first_fifty_steel_on_rifle_where_full_saturation_trains_worker() {
        let mut owned = vec![
            building(10, EntityKind::CityCentre, Some(0)),
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
    fn fast_flood_recovers_after_barracks_rifle_window() {
        let mut owned = vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
        ];
        owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
        let mut observation = observation(
            AiEconomy {
                steel: 200,
                oil: 0,
                supply_used: 5,
                supply_cap: 20,
            },
            owned,
        );
        let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
        let before = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

        assert!(
            !before.intents.contains(&AiIntent::Train {
                kind: EntityKind::Worker
            }),
            "fast flood should keep its five-worker cap before the recovery window"
        );

        let rifle_build_ticks = config::unit_stats(EntityKind::Rifleman)
            .expect("rifleman stats should exist")
            .build_ticks;
        observation.tick = observation
            .tick
            .saturating_add(rifle_build_ticks.saturating_mul(7));
        let after = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

        assert!(
            after.intents.contains(&AiIntent::Train {
                kind: EntityKind::Worker
            }),
            "fast flood should resume worker production once the proxy window has passed"
        );
        assert!(
            after.intents.contains(&AiIntent::Build {
                kind: EntityKind::Barracks
            }),
            "fast flood should add a home barracks during recovery instead of relying only on the proxy"
        );
    }

    #[test]
    fn fast_flood_recovery_builds_support_tech_and_takes_oil() {
        let mut owned = vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
        ];
        owned.extend((0..8).map(|i| steel_worker(20 + i, 100 + i)));
        owned.extend((0..3).map(|i| worker(40 + i, AiEntityState::Idle)));
        let mut observation = observation(
            AiEconomy {
                steel: 300,
                oil: 50,
                supply_used: 11,
                supply_cap: 28,
            },
            owned,
        );
        let mut memory = AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST);
        let _ = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);
        let rifle_build_ticks = config::unit_stats(EntityKind::Rifleman)
            .expect("rifleman stats should exist")
            .build_ticks;
        observation.tick = observation
            .tick
            .saturating_add(rifle_build_ticks.saturating_mul(7));

        let decision = decide(&observation, &RIFLE_FLOOD_FAST, &mut memory);

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TrainingCentre
        }));
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
    fn tech_to_tanks_delays_oil_until_steel_floor_and_builds_tank_tech() {
        let mut owned = vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
        ];
        owned.extend((0..4).map(|i| worker(20 + i, AiEntityState::Idle)));
        let initial_observation = observation(
            AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 4,
                supply_cap: 20,
            },
            owned,
        );

        let decision = decide(
            &initial_observation,
            &TECH_TO_TANKS,
            &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Factory
        }));
        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Steelworks
        }));
        assert!(
            decision.intents.contains(&AiIntent::Train {
                kind: EntityKind::Worker
            }),
            "tech_to_tanks should keep worker production alive while saving for tank tech"
        );
        assert!(
            !decision.intents.contains(&AiIntent::Train {
                kind: EntityKind::Rifleman
            }),
            "tech_to_tanks should save barracks steel once the factory is buildable"
        );
        assert!(
            !decision.intents.iter().any(|intent| matches!(
                intent,
                AiIntent::Gather {
                    resource: EntityKind::Oil,
                    ..
                }
            )),
            "tech_to_tanks should not send workers to oil before the steel floor is saturated"
        );

        let mut steel_floor_owned = vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
        ];
        steel_floor_owned.extend((0..8).map(|i| steel_worker(20 + i, 100 + i)));
        steel_floor_owned.extend((0..3).map(|i| worker(40 + i, AiEntityState::Idle)));
        let steel_floor_observation = observation(
            AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 11,
                supply_cap: 20,
            },
            steel_floor_owned,
        );

        let steel_floor_decision = decide(
            &steel_floor_observation,
            &TECH_TO_TANKS,
            &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
        );

        assert!(steel_floor_decision.intents.iter().any(|intent| {
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
    fn full_saturation_pivots_to_tank_tech_but_waits_for_full_steel_before_oil() {
        let mut owned = vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
        ];
        owned.extend((0..17).map(|i| steel_worker(20 + i, 100 + i)));
        owned.extend((0..40).map(|i| combat(200 + i, EntityKind::Rifleman)));
        owned.extend((0..2).map(|i| worker(300 + i, AiEntityState::Idle)));
        let mut observation = observation(
            AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 50,
                supply_cap: 70,
            },
            owned,
        );
        let facts = AiFacts::from_observation(&observation);
        let target_steel_workers = target_steel_workers_for_profile(
            &observation,
            &facts,
            &RIFLE_FLOOD_FULL_SATURATION,
            false,
            RIFLE_FLOOD_FULL_SATURATION
                .workers
                .target_steel_workers(facts.target_steel_workers, usize::MAX),
        );
        let desired_oil = desired_oil_workers(
            &observation,
            &facts,
            &RIFLE_FLOOD_FULL_SATURATION,
            false,
            target_steel_workers,
        );

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FULL_SATURATION,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
        );

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TrainingCentre
        }));
        assert_eq!(target_steel_workers, 18);
        assert_eq!(desired_oil, 0);

        observation.owned.push(steel_worker(37, 117));
        let facts = AiFacts::from_observation(&observation);
        let target_steel_workers = target_steel_workers_for_profile(
            &observation,
            &facts,
            &RIFLE_FLOOD_FULL_SATURATION,
            false,
            RIFLE_FLOOD_FULL_SATURATION
                .workers
                .target_steel_workers(facts.target_steel_workers, usize::MAX),
        );
        let desired_oil = desired_oil_workers(
            &observation,
            &facts,
            &RIFLE_FLOOD_FULL_SATURATION,
            false,
            target_steel_workers,
        );
        assert_eq!(desired_oil, 3);
    }

    #[test]
    fn full_saturation_oil_timing_tracks_observed_steel_patch_count() {
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
        ];
        owned.extend((0..18).map(|i| steel_worker(20 + i, 100 + i)));
        owned.push(worker(300, AiEntityState::Idle));
        let mut observation = observation(
            AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 50,
                supply_cap: 70,
            },
            owned,
        );
        observation
            .resources
            .push(resource(118, EntityKind::Steel, 13.5 * ts, 11.5 * ts));

        let facts = AiFacts::from_observation(&observation);
        let target_steel_workers = target_steel_workers_for_profile(
            &observation,
            &facts,
            &RIFLE_FLOOD_FULL_SATURATION,
            false,
            RIFLE_FLOOD_FULL_SATURATION
                .workers
                .target_steel_workers(facts.target_steel_workers, usize::MAX),
        );
        let desired_oil = desired_oil_workers(
            &observation,
            &facts,
            &RIFLE_FLOOD_FULL_SATURATION,
            false,
            target_steel_workers,
        );

        assert_eq!(target_steel_workers, 19);
        assert_eq!(desired_oil, 0);
    }

    #[test]
    fn full_saturation_can_expand_while_teching_to_tanks() {
        let mut owned = vec![
            building(10, EntityKind::CityCentre, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, Some(0)),
        ];
        owned.extend((0..18).map(|i| steel_worker(20 + i, 100 + i)));
        owned.extend((0..29).map(|i| combat(200 + i, EntityKind::Rifleman)));
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 2_000,
                oil: 2_000,
                supply_used: 50,
                supply_cap: 70,
            },
            owned,
        ));

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FULL_SATURATION,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
        );

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Factory
        }));
        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::CityCentre
        }));
    }

    #[test]
    fn steel_expansion_tanks_builds_expansion_cc_before_any_barracks() {
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![building_at(
            10,
            EntityKind::CityCentre,
            Some(0),
            8.5 * ts,
            8.5 * ts,
        )];
        owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
        owned.push(worker(60, AiEntityState::Idle));
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 12,
                supply_cap: 30,
            },
            owned,
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::CityCentre
        }));
        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Barracks
        }));
        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TrainingCentre
        }));
        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Factory
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::MachineGunner
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::AtTeam
        }));
        let non_depot_builds: Vec<_> = decision
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::Build { building, .. } if *building != EntityKind::Depot => {
                    Some(*building)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            non_depot_builds,
            vec![EntityKind::CityCentre],
            "the first non-depot build should be the expansion City Centre"
        );
        assert!(
            !decision.intents.iter().any(|intent| matches!(
                intent,
                AiIntent::Gather {
                    resource: EntityKind::Oil,
                    ..
                }
            )),
            "expansion profile should not move into oil before the second City Centre is planned"
        );
    }

    #[test]
    fn steel_expansion_tanks_places_expansion_cc_in_range_of_whole_resource_line() {
        let map_size = 96;
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![building_at(
            10,
            EntityKind::CityCentre,
            Some(0),
            10.5 * ts,
            85.5 * ts,
        )];
        owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
        owned.push(worker(60, AiEntityState::Idle));
        let mut resources = base_site_resources(100, (10, 85), map_size);
        resources.extend(base_site_resources(200, (48, 73), map_size));
        resources.sort_by_key(|resource| resource.id);
        let observation = AiObservation {
            player_id: 1,
            tick: 90,
            map: AiMapSummary {
                width: map_size,
                height: map_size,
                tile_size: config::TILE_SIZE,
            },
            economy: AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 12,
                supply_cap: 30,
            },
            own_start_tile: (10, 85),
            players: vec![
                AiPlayerSummary {
                    id: 1,
                    start_tile: (10, 85),
                    is_ai: true,
                    is_alive: true,
                },
                AiPlayerSummary {
                    id: 2,
                    start_tile: (85, 10),
                    is_ai: false,
                    is_alive: true,
                },
            ],
            owned,
            resources,
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
        };

        let mut placeable = |_: EntityKind, tx: u32, ty: u32| tx < map_size && ty < map_size;
        let site = expansion_city_centre_site(
            &observation,
            STEEL_EXPANSION_TANKS.expansion.unwrap(),
            EntityKind::CityCentre,
            &mut placeable,
        )
        .expect("expansion City Centre site should be found");

        assert_eq!(
            expansion_resource_counts_for_site(&observation, site),
            (
                config::STEEL_PATCHES_PER_BASE as usize,
                config::OIL_PATCHES_PER_BASE as usize
            ),
            "expansion City Centre at {site:?} should cover the whole natural resource line"
        );

        let mut memory = AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS);
        let decision = decide_profile(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut memory,
            ai_shared::BuildSearch {
                min_radius: 0,
                max_radius: 0,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            |_, tx, ty| tx < map_size && ty < map_size,
        );
        let command_site = decision
            .commands
            .iter()
            .find_map(|command| match command {
                Command::Build {
                    building,
                    tile_x,
                    tile_y,
                    ..
                } if *building == EntityKind::CityCentre => Some((*tile_x, *tile_y)),
                _ => None,
            })
            .expect("decision should issue an expansion City Centre build");

        assert_eq!(
            expansion_resource_counts_for_site(&observation, command_site),
            (
                config::STEEL_PATCHES_PER_BASE as usize,
                config::OIL_PATCHES_PER_BASE as usize
            ),
            "emitted expansion City Centre build at {command_site:?} should cover all expansion resources"
        );
    }

    #[test]
    fn steel_expansion_tanks_prefers_closer_natural_resource_cluster() {
        let map_size = 96;
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![building_at(
            10,
            EntityKind::CityCentre,
            Some(0),
            10.5 * ts,
            85.5 * ts,
        )];
        owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
        owned.push(worker(60, AiEntityState::Idle));
        let mut resources = base_site_resources(100, (10, 85), map_size);
        resources.extend(base_site_resources(200, (23, 47), map_size));
        resources.extend(base_site_resources(300, (48, 73), map_size));
        resources.sort_by_key(|resource| resource.id);
        let observation = AiObservation {
            player_id: 1,
            tick: 90,
            map: AiMapSummary {
                width: map_size,
                height: map_size,
                tile_size: config::TILE_SIZE,
            },
            economy: AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 12,
                supply_cap: 30,
            },
            own_start_tile: (10, 85),
            players: vec![
                AiPlayerSummary {
                    id: 1,
                    start_tile: (10, 85),
                    is_ai: true,
                    is_alive: true,
                },
                AiPlayerSummary {
                    id: 2,
                    start_tile: (85, 10),
                    is_ai: false,
                    is_alive: true,
                },
            ],
            owned,
            resources,
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
        };

        let mut placeable = |_: EntityKind, tx: u32, ty: u32| tx < map_size && ty < map_size;
        let site = expansion_city_centre_site(
            &observation,
            STEEL_EXPANSION_TANKS.expansion.unwrap(),
            EntityKind::CityCentre,
            &mut placeable,
        )
        .expect("expansion City Centre site should be found");
        let center = building_center(site, EntityKind::CityCentre, observation.map.tile_size)
            .expect("city centre should have a center");
        let closer_natural = tile_center((23, 47), observation.map.tile_size);
        let farther_natural = tile_center((48, 73), observation.map.tile_size);

        assert!(
            dist2(center.0, center.1, closer_natural.0, closer_natural.1)
                < dist2(center.0, center.1, farther_natural.0, farther_natural.1),
            "expansion City Centre at {site:?} should choose the closer natural cluster"
        );
        assert_eq!(
            expansion_resource_counts_for_site(&observation, site),
            (
                config::STEEL_PATCHES_PER_BASE as usize,
                config::OIL_PATCHES_PER_BASE as usize
            ),
            "chosen closer natural should still cover its whole resource line"
        );
    }

    #[test]
    fn expansion_site_selection_prefers_oil_over_steel_only_output() {
        let map_size = 96;
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![building_at(
            10,
            EntityKind::CityCentre,
            Some(0),
            10.5 * ts,
            85.5 * ts,
        )];
        owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
        let mut resources = base_site_resources(100, (10, 85), map_size);
        resources.extend(
            base_site_resources(200, (22, 75), map_size)
                .into_iter()
                .filter(|resource| resource.kind == EntityKind::Steel),
        );
        resources.extend(base_site_resources(300, (55, 55), map_size));
        resources.sort_by_key(|resource| resource.id);
        let observation = AiObservation {
            player_id: 1,
            tick: 90,
            map: AiMapSummary {
                width: map_size,
                height: map_size,
                tile_size: config::TILE_SIZE,
            },
            economy: AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 70,
                supply_cap: 80,
            },
            own_start_tile: (10, 85),
            players: vec![
                AiPlayerSummary {
                    id: 1,
                    start_tile: (10, 85),
                    is_ai: true,
                    is_alive: true,
                },
                AiPlayerSummary {
                    id: 2,
                    start_tile: (86, 10),
                    is_ai: false,
                    is_alive: true,
                },
            ],
            owned,
            resources,
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
        };
        let expansion = STEEL_EXPANSION_TANKS.expansion.unwrap();

        let site = expansion_city_centre_site(
            &observation,
            expansion,
            EntityKind::CityCentre,
            &mut |_, _, _| true,
        )
        .expect("oil-bearing expansion site should be found");

        let (_, oil) = expansion_resource_counts_for_site(&observation, site);
        assert_eq!(oil, config::OIL_PATCHES_PER_BASE as usize);
    }

    #[test]
    fn steel_expansion_tanks_prefers_safer_natural_when_distances_are_similar() {
        let map_size = 96;
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![building_at(
            10,
            EntityKind::CityCentre,
            Some(0),
            10.5 * ts,
            85.5 * ts,
        )];
        owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
        owned.push(worker(60, AiEntityState::Idle));
        let mut resources = base_site_resources(100, (10, 85), map_size);
        resources.extend(base_site_resources(200, (23, 47), map_size));
        resources.extend(base_site_resources(300, (48, 73), map_size));
        resources.sort_by_key(|resource| resource.id);
        let observation = AiObservation {
            player_id: 1,
            tick: 90,
            map: AiMapSummary {
                width: map_size,
                height: map_size,
                tile_size: config::TILE_SIZE,
            },
            economy: AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 12,
                supply_cap: 30,
            },
            own_start_tile: (10, 85),
            players: vec![
                AiPlayerSummary {
                    id: 1,
                    start_tile: (10, 85),
                    is_ai: true,
                    is_alive: true,
                },
                AiPlayerSummary {
                    id: 2,
                    start_tile: (85, 85),
                    is_ai: false,
                    is_alive: true,
                },
            ],
            owned,
            resources,
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
        };

        let mut placeable = |_: EntityKind, tx: u32, ty: u32| tx < map_size && ty < map_size;
        let site = expansion_city_centre_site(
            &observation,
            STEEL_EXPANSION_TANKS.expansion.unwrap(),
            EntityKind::CityCentre,
            &mut placeable,
        )
        .expect("expansion City Centre site should be found");
        let center = building_center(site, EntityKind::CityCentre, observation.map.tile_size)
            .expect("city centre should have a center");
        let safer_natural = tile_center((23, 47), observation.map.tile_size);
        let exposed_natural = tile_center((48, 73), observation.map.tile_size);

        assert!(
            dist2(center.0, center.1, safer_natural.0, safer_natural.1)
                < dist2(center.0, center.1, exposed_natural.0, exposed_natural.1),
            "expansion City Centre at {site:?} should choose the natural farther from the enemy start"
        );
        assert_eq!(
            expansion_resource_counts_for_site(&observation, site),
            (
                config::STEEL_PATCHES_PER_BASE as usize,
                config::OIL_PATCHES_PER_BASE as usize
            ),
            "chosen safer natural should still cover its whole resource line"
        );
    }

    #[test]
    fn steel_expansion_tanks_builds_barracks_after_expansion_cc_is_planned() {
        let mut observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 10,
                supply_cap: 30,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                worker(60, AiEntityState::Idle),
            ],
        ));
        observation
            .pending_builds
            .push(AiBuildIntent::to_site(60, EntityKind::CityCentre, 20, 30));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::CityCentre
        }));
        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Barracks
        }));
        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TrainingCentre
        }));
    }

    #[test]
    fn steel_expansion_tanks_builds_training_centre_before_training_support_units() {
        let mut observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 10,
                supply_cap: 30,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                worker(60, AiEntityState::Idle),
            ],
        ));
        observation
            .pending_builds
            .push(AiBuildIntent::to_site(60, EntityKind::CityCentre, 20, 30));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TrainingCentre
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::MachineGunner
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::AtTeam
        }));
    }

    #[test]
    fn steel_expansion_tanks_balances_machine_gunner_and_at_team_training() {
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 200,
                supply_used: 10,
                supply_cap: 40,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::CityCentre, Some(0)),
                building(12, EntityKind::Barracks, Some(0)),
                building(13, EntityKind::Barracks, Some(0)),
                building(14, EntityKind::TrainingCentre, None),
                worker(60, AiEntityState::Idle),
            ],
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::MachineGunner
        }));
        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::AtTeam
        }));
    }

    #[test]
    fn steel_expansion_tanks_counts_queued_machine_gunners_when_balancing_support() {
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 200,
                supply_used: 14,
                supply_cap: 50,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::CityCentre, Some(0)),
                building_training(12, EntityKind::Barracks, EntityKind::MachineGunner),
                building(13, EntityKind::Barracks, Some(0)),
                building(15, EntityKind::TrainingCentre, None),
            ],
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::AtTeam
        }));
        assert!(
            !decision.intents.contains(&AiIntent::Train {
                kind: EntityKind::MachineGunner
            }),
            "pending machine gunners should count toward the support mix"
        );
    }

    #[test]
    fn steel_expansion_tanks_sends_workers_to_oil_after_expansion_is_planned() {
        let ts = config::TILE_SIZE as f32;
        let mut observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 0,
                supply_used: 17,
                supply_cap: 40,
            },
            {
                let mut owned = vec![building_at(
                    10,
                    EntityKind::CityCentre,
                    Some(0),
                    8.5 * ts,
                    8.5 * ts,
                )];
                owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
                owned.extend((0..6).map(|i| worker(60 + i, AiEntityState::Idle)));
                owned
            },
        ));
        observation
            .pending_builds
            .push(AiBuildIntent::to_site(60, EntityKind::CityCentre, 20, 30));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        let oil_assignments = decision
            .intents
            .iter()
            .filter_map(|intent| match intent {
                AiIntent::Gather {
                    resource: EntityKind::Oil,
                    assignments,
                } => Some(*assignments),
                _ => None,
            })
            .sum::<usize>();
        assert!(
            oil_assignments >= 5,
            "support tech should send most idle workers to oil once expanding, got {oil_assignments}"
        );
    }

    #[test]
    fn steel_expansion_tanks_keeps_main_workers_off_distant_expansion_steel() {
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
        ];
        owned.extend((0..18u32).map(|i| gathering_worker(40 + i, 100 + i)));
        owned.extend((0..6u32).map(|i| {
            let node = if i < 3 { 200 + i } else { 400 + (i - 3) };
            gathering_worker(70 + i, node)
        }));
        owned.push(worker_at(90, AiEntityState::Idle, 8.5 * ts, 8.5 * ts));
        owned.push(worker_at(91, AiEntityState::Idle, 9.5 * ts, 8.5 * ts));
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 26,
                supply_cap: 80,
            },
            owned,
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(
            !decision.commands.iter().any(|command| {
                matches!(command, Command::Gather { node, .. } if (300..318).contains(node))
            }),
            "main-base idle workers should not be sent to expansion steel patches"
        );
    }

    #[test]
    fn steel_expansion_tanks_sends_expansion_workers_to_expansion_steel() {
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![
            building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
            building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
        ];
        owned.extend((0..18u32).map(|i| gathering_worker(40 + i, 100 + i)));
        owned.extend((0..6u32).map(|i| {
            let node = if i < 3 { 200 + i } else { 400 + (i - 3) };
            gathering_worker(70 + i, node)
        }));
        owned.push(worker_at(90, AiEntityState::Idle, 23.5 * ts, 36.5 * ts));
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 25,
                supply_cap: 80,
            },
            owned,
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(
            decision.commands.iter().any(|command| {
                matches!(
                    command,
                    Command::Gather { units, node }
                        if units.as_slice() == [90] && (300..318).contains(node)
                )
            }),
            "an idle expansion worker should take a local expansion steel patch"
        );
    }

    #[test]
    fn steel_expansion_tanks_stages_support_weapons_on_enemy_facing_main_steel_line() {
        let ts = config::TILE_SIZE as f32;
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 200,
                supply_used: 24,
                supply_cap: 80,
            },
            vec![
                building_at(10, EntityKind::CityCentre, Some(0), 8.5 * ts, 8.5 * ts),
                building_at(11, EntityKind::CityCentre, Some(0), 23.5 * ts, 36.5 * ts),
                building(12, EntityKind::Barracks, Some(0)),
                building(13, EntityKind::TrainingCentre, None),
                combat_at(30, EntityKind::MachineGunner, 8.5 * ts, 8.5 * ts),
                combat_at(31, EntityKind::AtTeam, 9.5 * ts, 8.5 * ts),
                combat_at(32, EntityKind::MachineGunner, 10.5 * ts, 8.5 * ts),
            ],
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        let stage_targets: Vec<(u32, f32, f32)> = decision
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::AttackMove { units, x, y } if units.len() == 1 => Some((units[0], *x, *y)),
                _ => None,
            })
            .collect();
        assert_eq!(
            stage_targets
                .iter()
                .map(|(id, _, _)| *id)
                .collect::<Vec<_>>(),
            vec![30, 31, 32],
            "support weapons should receive deterministic individual stage slots"
        );

        let steel_center =
            main_steel_cluster_center(&observation).expect("main steel cluster should be found");
        let enemy = AiFacts::from_observation(&observation)
            .nearest_public_enemy_base
            .expect("enemy base should be public");
        let dir = normalized_direction(steel_center, (enemy.x, enemy.y))
            .expect("enemy should not overlap the main steel");
        let perp = (-dir.1, dir.0);
        let mut lateral_offsets = Vec::new();
        for (_, x, y) in &stage_targets {
            let dx = *x - steel_center.0;
            let dy = *y - steel_center.1;
            let front_tiles = (dx * dir.0 + dy * dir.1) / ts;
            assert!(
                (2.0..=4.0).contains(&front_tiles),
                "stage point should be in front of the steel patch, got {front_tiles} tiles"
            );
            lateral_offsets.push((dx * perp.0 + dy * perp.1) / ts);
        }
        lateral_offsets.sort_by(|left, right| left.total_cmp(right));
        let spread = lateral_offsets.last().unwrap() - lateral_offsets.first().unwrap();
        assert!(
            spread >= 2.5,
            "support weapons should spread across a line, got {spread} tiles"
        );
    }

    #[test]
    fn steel_expansion_tanks_switches_to_factory_at_fifty_supply() {
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 50,
                supply_cap: 130,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::CityCentre, Some(0)),
                building(12, EntityKind::Barracks, Some(0)),
                building(13, EntityKind::Barracks, Some(0)),
                building(14, EntityKind::TrainingCentre, None),
                worker(60, AiEntityState::Idle),
            ],
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Factory
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::MachineGunner
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::AtTeam
        }));
    }

    #[test]
    fn steel_expansion_tanks_trains_only_tanks_after_fifty_supply() {
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 300,
                supply_used: 50,
                supply_cap: 130,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::CityCentre, Some(0)),
                building(12, EntityKind::Barracks, Some(0)),
                building(13, EntityKind::TrainingCentre, None),
                building(14, EntityKind::Factory, Some(0)),
                building(15, EntityKind::Steelworks, None),
            ],
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Tank
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::MachineGunner
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::AtTeam
        }));
    }

    #[test]
    fn steel_expansion_tanks_attacks_with_three_or_more_tanks_after_transition() {
        let two_tanks = with_expansion_resources(observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 50,
                supply_cap: 130,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::CityCentre, Some(0)),
                building(12, EntityKind::Barracks, Some(0)),
                building(13, EntityKind::TrainingCentre, None),
                building(14, EntityKind::Factory, Some(0)),
                combat(30, EntityKind::Tank),
                combat(31, EntityKind::Tank),
            ],
        ));
        let two_tank_decision = decide(
            &two_tanks,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(
            !two_tank_decision
                .intents
                .iter()
                .any(|intent| matches!(intent, AiIntent::Attack { .. })),
            "two tanks should not launch an outbound attack"
        );

        let three_tanks = with_expansion_resources(observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 50,
                supply_cap: 130,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::CityCentre, Some(0)),
                building(12, EntityKind::Barracks, Some(0)),
                building(13, EntityKind::TrainingCentre, None),
                building(14, EntityKind::Factory, Some(0)),
                combat(30, EntityKind::Tank),
                combat(31, EntityKind::Tank),
                combat(32, EntityKind::Tank),
                combat(40, EntityKind::MachineGunner),
                combat(41, EntityKind::AtTeam),
            ],
        ));
        let three_tank_decision = decide(
            &three_tanks,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(three_tank_decision.intents.iter().any(|intent| {
            matches!(
                intent,
                AiIntent::Attack { units } if units.as_slice() == [30, 31, 32]
            )
        }));
        assert!(
            three_tank_decision.commands.iter().any(|command| {
                matches!(
                    command,
                    Command::AttackMove { units, .. } if units.as_slice() == [30, 31, 32]
                )
            }),
            "three ready tanks should attack as a group"
        );
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
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building(13, EntityKind::Factory, Some(0)),
                building(14, EntityKind::Steelworks, None),
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
    fn infantry_heavy_home_threat_prefers_machine_gunners_before_tanks() {
        let ts = config::TILE_SIZE as f32;
        let mut observation = observation(
            AiEconomy {
                steel: 200,
                oil: 150,
                supply_used: 4,
                supply_cap: 20,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building(13, EntityKind::Factory, Some(0)),
                worker(20, AiEntityState::Gather),
                worker(21, AiEntityState::Gather),
                worker(22, AiEntityState::Gather),
                worker(23, AiEntityState::Gather),
            ],
        );
        observation
            .visible_enemies
            .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));

        let decision = decide(
            &observation,
            &TECH_TO_TANKS,
            &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::MachineGunner
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Tank
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }));
    }

    #[test]
    fn armor_heavy_home_threat_prefers_at_teams_before_tanks() {
        let ts = config::TILE_SIZE as f32;
        let mut observation = observation(
            AiEconomy {
                steel: 200,
                oil: 150,
                supply_used: 4,
                supply_cap: 20,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building(13, EntityKind::Factory, Some(0)),
                worker(20, AiEntityState::Gather),
                worker(21, AiEntityState::Gather),
                worker(22, AiEntityState::Gather),
                worker(23, AiEntityState::Gather),
            ],
        );
        observation
            .visible_enemies
            .push(enemy(90, EntityKind::Tank, 10.5 * ts, 10.5 * ts));

        let decision = decide(
            &observation,
            &TECH_TO_TANKS,
            &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
        );

        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::AtTeam
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Tank
        }));
        assert!(!decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Worker
        }));
    }

    #[test]
    fn sustained_support_panic_falls_back_to_riflemen_without_training_centre() {
        let ts = config::TILE_SIZE as f32;
        let mut observation = observation(
            AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 8,
                supply_cap: 30,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                worker(20, AiEntityState::Gather),
                worker(21, AiEntityState::Gather),
            ],
        );
        observation
            .visible_enemies
            .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));
        let mut memory = AiDecisionMemory::for_profile(&TECH_TO_TANKS);

        let first_decision = decide(&observation, &TECH_TO_TANKS, &mut memory);
        assert!(
            !first_decision.intents.contains(&AiIntent::Build {
                kind: EntityKind::Barracks
            }),
            "fresh panic should use the existing barracks before adding another one"
        );
        assert!(
            !first_decision.intents.contains(&AiIntent::Build {
                kind: EntityKind::TrainingCentre
            }),
            "panic mode must not create support tech"
        );
        assert!(first_decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }));
        assert!(
            !first_decision.intents.iter().any(|intent| matches!(
                intent,
                AiIntent::Gather {
                    resource: EntityKind::Oil,
                    ..
                }
            )),
            "support fallback should not pull workers onto oil"
        );

        let started_tick = observation.tick;
        observation.tick = started_tick.saturating_add(DEFENSIVE_PANIC_GRACE_TICKS);
        let _ = decide(&observation, &TECH_TO_TANKS, &mut memory);
        observation.tick = started_tick.saturating_add(DEFENSIVE_PANIC_SUSTAINED_TICKS);
        let sustained_decision = decide(&observation, &TECH_TO_TANKS, &mut memory);

        assert!(sustained_decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::Barracks
        }));
        assert!(
            !sustained_decision.intents.iter().any(|intent| matches!(
                intent,
                AiIntent::Build {
                    kind: EntityKind::TrainingCentre | EntityKind::Factory
                }
            )),
            "panic mode should block all tech spending"
        );
        assert!(sustained_decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }));
    }

    #[test]
    fn visible_home_threat_preempts_outbound_tank_attack() {
        let ts = config::TILE_SIZE as f32;
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 10,
                supply_cap: 20,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building(13, EntityKind::Factory, Some(0)),
                combat_at(30, EntityKind::Tank, 8.5 * ts, 8.5 * ts),
            ],
        );
        observation
            .visible_enemies
            .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));

        let decision = decide(
            &observation,
            &TECH_TO_TANKS,
            &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
        );

        assert!(decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target } if *target == 90 && units == &[30]
            )
        }));
        assert!(
            !decision
                .commands
                .iter()
                .any(|command| matches!(command, Command::AttackMove { .. })),
            "local defense should preempt the outbound tank wave"
        );
    }

    #[test]
    fn far_tank_is_not_recalled_for_home_threat() {
        let ts = config::TILE_SIZE as f32;
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 10,
                supply_cap: 20,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building(13, EntityKind::Factory, Some(0)),
                combat_at(30, EntityKind::Tank, 48.5 * ts, 48.5 * ts),
            ],
        );
        observation
            .visible_enemies
            .push(enemy(90, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts));

        let decision = decide(
            &observation,
            &TECH_TO_TANKS,
            &mut AiDecisionMemory::for_profile(&TECH_TO_TANKS),
        );

        assert!(
            !decision.commands.iter().any(|command| {
                matches!(
                    command,
                    Command::Attack { units, target } if *target == 90 && units == &[30]
                )
            }),
            "far outbound tanks should not be pulled back by local defense"
        );
        assert!(
            decision.commands.iter().any(|command| {
                matches!(
                    command,
                    Command::AttackMove { units, .. } if units == &[30]
                )
            }),
            "far tanks should keep their outbound attack behavior"
        );
    }

    #[test]
    fn rifle_attack_wave_uses_plain_move_deeper_than_enemy_base() {
        let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
        owned.extend((0..6).map(|i| combat(30 + i, EntityKind::Rifleman)));
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 6,
                supply_cap: 20,
            },
            owned,
        );

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FULL_SATURATION,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FULL_SATURATION),
        );

        assert!(
            !decision
                .commands
                .iter()
                .any(|command| matches!(command, Command::AttackMove { .. })),
            "pure rifle raids should not use generic attack-move"
        );
        let enemy_base = tile_center(enemy_start_tile(&observation), observation.map.tile_size);
        assert!(decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Move { units, x, y }
                    if units.as_slice() == [30, 31, 32, 33, 34, 35]
                        && *x > enemy_base.0
                        && *y > enemy_base.1
            )
        }));
    }

    #[test]
    fn moving_rifle_raid_targets_visible_workers_before_buildings() {
        let ts = config::TILE_SIZE as f32;
        let mut raider = combat_at(30, EntityKind::Rifleman, 46.0 * ts, 46.0 * ts);
        raider.state = AiEntityState::Move;
        raider.free_for_combat = false;
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
            },
            vec![building(10, EntityKind::CityCentre, Some(0)), raider],
        );
        observation
            .visible_enemies
            .push(enemy(80, EntityKind::Depot, 45.5 * ts, 45.5 * ts));
        observation
            .visible_enemies
            .push(enemy(90, EntityKind::Worker, 48.5 * ts, 48.5 * ts));

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        assert!(decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target } if units.as_slice() == [30] && *target == 90
            )
        }));
    }

    #[test]
    fn moving_rifle_raid_targets_visible_scout_car() {
        let ts = config::TILE_SIZE as f32;
        let mut raider = combat_at(30, EntityKind::Rifleman, 46.0 * ts, 46.0 * ts);
        raider.state = AiEntityState::Move;
        raider.free_for_combat = false;
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
            },
            vec![building(10, EntityKind::CityCentre, Some(0)), raider],
        );
        observation
            .visible_enemies
            .push(enemy(90, EntityKind::ScoutCar, 48.5 * ts, 48.5 * ts));

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        assert!(decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target } if units.as_slice() == [30] && *target == 90
            )
        }));
    }

    #[test]
    fn local_defense_does_not_block_moving_raid_from_targeting_scout_car() {
        let ts = config::TILE_SIZE as f32;
        let mut raider = combat_at(30, EntityKind::Rifleman, 46.0 * ts, 46.0 * ts);
        raider.state = AiEntityState::Move;
        raider.free_for_combat = false;
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 2,
                supply_cap: 10,
            },
            vec![
                building(10, EntityKind::CityCentre, Some(0)),
                combat_at(20, EntityKind::Rifleman, 8.5 * ts, 8.5 * ts),
                raider,
            ],
        );
        observation
            .visible_enemies
            .push(enemy(80, EntityKind::Worker, 9.5 * ts, 9.5 * ts));
        observation
            .visible_enemies
            .push(enemy(90, EntityKind::ScoutCar, 48.5 * ts, 48.5 * ts));

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        assert!(decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target } if units.as_slice() == [20] && *target == 80
            )
        }));
        assert!(decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target } if units.as_slice() == [30] && *target == 90
            )
        }));
    }

    #[test]
    fn rifle_raid_attacks_buildings_after_reaching_enemy_main_steel_line_without_units() {
        let ts = config::TILE_SIZE as f32;
        let observation = {
            let mut observation = observation(
                AiEconomy {
                    steel: 0,
                    oil: 0,
                    supply_used: 1,
                    supply_cap: 10,
                },
                vec![building(10, EntityKind::CityCentre, Some(0))],
            );
            observation = with_enemy_main_resources(observation);
            let enemy_base = enemy_base_fact(&observation);
            let steel_center = enemy_main_steel_center(&observation, enemy_base)
                .expect("enemy main steel should be present");
            observation.owned.push(combat_at(
                30,
                EntityKind::Rifleman,
                steel_center.0 + ts,
                steel_center.1,
            ));
            observation
                .visible_enemies
                .push(enemy(80, EntityKind::Depot, 48.5 * ts, 48.5 * ts));
            observation
        };

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        assert!(decision.commands.iter().any(|command| {
            matches!(
                command,
                Command::Attack { units, target } if units.as_slice() == [30] && *target == 80
            )
        }));
    }

    #[test]
    fn rifle_raid_ignores_buildings_near_enemy_start_before_reaching_main_steel_line() {
        let ts = config::TILE_SIZE as f32;
        let observation = {
            let mut observation = observation(
                AiEconomy {
                    steel: 0,
                    oil: 0,
                    supply_used: 1,
                    supply_cap: 10,
                },
                vec![
                    building(10, EntityKind::CityCentre, Some(0)),
                    combat_at(30, EntityKind::Rifleman, 49.0 * ts, 49.0 * ts),
                ],
            );
            observation = with_enemy_main_resources(observation);
            observation
                .visible_enemies
                .push(enemy(80, EntityKind::Depot, 48.5 * ts, 48.5 * ts));
            observation
        };

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        assert!(
            !decision.commands.iter().any(|command| {
                matches!(
                    command,
                    Command::Attack { units, target } if units.as_slice() == [30] && *target == 80
                )
            }),
            "rifle raids should not switch to buildings until they reach the enemy main steel line"
        );
    }

    #[test]
    fn moving_rifle_raid_ignores_visible_buildings_until_arrival() {
        let ts = config::TILE_SIZE as f32;
        let mut raider = combat_at(30, EntityKind::Rifleman, 46.0 * ts, 46.0 * ts);
        raider.state = AiEntityState::Move;
        raider.free_for_combat = false;
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
            },
            vec![building(10, EntityKind::CityCentre, Some(0)), raider],
        );
        observation
            .visible_enemies
            .push(enemy(80, EntityKind::Depot, 48.5 * ts, 48.5 * ts));

        let decision = decide(
            &observation,
            &RIFLE_FLOOD_FAST,
            &mut AiDecisionMemory::for_profile(&RIFLE_FLOOD_FAST),
        );

        assert!(
            !decision.commands.iter().any(|command| {
                matches!(
                    command,
                    Command::Attack { units, target } if units.as_slice() == [30] && *target == 80
                )
            }),
            "moving rifle raids should keep moving past buildings"
        );
    }

    #[test]
    fn attack_memory_uses_profile_thresholds_and_growth() {
        let mut owned = Vec::new();
        owned.push(combat(30, EntityKind::Rifleman));
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 1,
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
            AiIntent::Attack { units } if units.as_slice() == [30]
        )));
        assert!(full.intents.iter().any(|intent| matches!(
            intent,
            AiIntent::Stage { units } if units.as_slice() == [30]
        )));
        assert_eq!(fast_memory.desired_attack_size(&RIFLE_FLOOD_FAST, 91), 1);
    }

    #[test]
    fn each_required_profile_emits_a_starting_state_command() {
        let mut owned = vec![building(10, EntityKind::CityCentre, Some(0))];
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
