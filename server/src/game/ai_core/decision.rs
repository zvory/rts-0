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
    AiEntityState, AiEntitySummary, AiObservation, AiResourceSummary,
};
use crate::game::ai_core::profiles::{AiProfile, ExpansionPolicy, ProxyBarracksPolicy};
use crate::game::ai_shared;
use crate::game::entity::EntityKind;
use crate::protocol::Command;
use crate::rules;

const PRODUCTION_BUILDINGS: [EntityKind; 3] = [
    EntityKind::TankFactory,
    EntityKind::Barracks,
    EntityKind::IndustrialCenter,
];
const LOCAL_DEFENSE_RADIUS_TILES: f32 = 12.0;
const RESOURCE_LINE_DEFENSE_RADIUS_TILES: f32 = 4.0;
const WORKER_DEFENSE_RADIUS_TILES: f32 = 5.0;
const PROXY_DISTANCE_BAND_TILES: f32 = 2.0;
const PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES: i32 = 4;

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
    next_attack_size: usize,
    last_attack_tick: Option<u32>,
    proxy_worker_id: Option<u32>,
}

impl AiDecisionMemory {
    pub(crate) fn for_profile(profile: &AiProfile) -> Self {
        Self {
            profile_id: Some(profile.id),
            next_attack_size: profile.attack.first_attack_size,
            last_attack_tick: None,
            proxy_worker_id: None,
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
        self.proxy_worker_id = None;
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
    let expansion_blocks_tank_tech = expansion_blocks_tank_tech(&facts, profile);
    let save_for_expansion = should_save_for_expansion(&facts, profile);
    let proxy_barracks_active = should_use_proxy_barracks(&facts, profile);

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

    for kind in profile.buildings.required_tech_path {
        if proxy_barracks_active && *kind == EntityKind::Barracks {
            continue;
        }
        if expansion_blocks_tank_tech && *kind != EntityKind::Barracks {
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

    let complete_gate_count = profile
        .workers
        .pressure_until_complete
        .map(|kind| facts.complete_building_count(kind))
        .unwrap_or(usize::MAX);
    let target_steel_workers = profile
        .workers
        .target_steel_workers(facts.target_steel_workers, complete_gate_count);
    let target_steel_workers =
        target_steel_workers_for_profile(observation, &facts, profile, target_steel_workers);
    let target_barracks = profile.buildings.barracks_curve.target(
        observation.economy.steel,
        facts.worker_count,
        target_steel_workers,
    );
    if facts.building_count(EntityKind::Barracks)
        + planned_in_intents(&intents, EntityKind::Barracks)
        < target_barracks
        && !(proxy_barracks_active && facts.building_count(EntityKind::Barracks) == 0)
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
        && try_build_expansion_industrial_center(
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
            kind: EntityKind::IndustrialCenter,
        });
    }

    let desired_oil_workers =
        desired_oil_workers(observation, &facts, profile, target_steel_workers);
    let target_workers = target_steel_workers.saturating_add(desired_oil_workers);
    let save_for_first_tech_unit = should_save_for_first_tech_unit(&facts, profile);
    let save_worker_training_for_tech = save_for_expansion
        || save_for_first_tech_unit
        || (save_for_required_tech_building && facts.worker_count >= target_workers);
    for trained in actions::train_units(
        &mut actions,
        TrainUnitsRequest {
            buildings: facts.production_buildings(EntityKind::IndustrialCenter),
            unit_priorities: &[EntityKind::Worker],
            max_queue_depth: 1,
            save_for_tech: save_worker_training_for_tech,
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
        let save_for_tech =
            (save_for_expansion || save_for_first_tech_unit || save_for_required_tech_building)
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

    let ready_units =
        actions::select_ready_combat_units(&observation.owned, profile.attack.unit_kinds);
    if !ready_units.is_empty() {
        let mut handled_local_defense = false;
        if let Some(target) = local_defense_target(observation) {
            if let Some(units) = actions::attack_units(
                &mut actions,
                local_defense_units(observation, &ready_units),
                target,
            ) {
                intents.push(AiIntent::Attack { units });
                handled_local_defense = true;
            }
        }

        if !handled_local_defense {
            if let Some(enemy_base) = facts.nearest_public_enemy_base {
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
                    if let Some(units) = actions::attack_move_units(
                        &mut actions,
                        ready_units,
                        enemy_base.x,
                        enemy_base.y,
                    ) {
                        memory.note_attack(profile, observation.tick);
                        intents.push(AiIntent::Attack { units });
                    }
                } else if !ready_units.is_empty() {
                    let own_base =
                        tile_center(observation.own_start_tile, observation.map.tile_size);
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

fn expansion_blocks_tank_tech(facts: &AiFacts, profile: &AiProfile) -> bool {
    let Some(expansion) = profile.expansion else {
        return false;
    };
    facts.complete_building_count(EntityKind::IndustrialCenter)
        < expansion.target_industrial_centers
}

fn should_save_for_expansion(facts: &AiFacts, profile: &AiProfile) -> bool {
    let Some(expansion) = profile.expansion else {
        return false;
    };
    facts.building_count(EntityKind::IndustrialCenter) < expansion.target_industrial_centers
        && expansion_prerequisites_met(facts, expansion)
}

fn expansion_prerequisites_met(facts: &AiFacts, expansion: ExpansionPolicy) -> bool {
    facts.complete_building_count(expansion.required_complete_building) > 0
        && facts.unit_count(expansion.defensive_unit) >= expansion.defensive_unit_count
}

fn target_steel_workers_for_profile(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    base_target: usize,
) -> usize {
    let Some(expansion) = profile.expansion else {
        return base_target;
    };
    if facts.complete_building_count(EntityKind::IndustrialCenter)
        < expansion.target_industrial_centers
    {
        return base_target.min(expansion.pre_expansion_steel_worker_cap);
    }

    let expanded_target = base_target.max(completed_ic_steel_saturation_target(observation));
    expansion
        .post_expansion_steel_worker_cap
        .map(|cap| expanded_target.min(cap))
        .unwrap_or(expanded_target)
}

fn completed_ic_steel_saturation_target(observation: &AiObservation) -> usize {
    let completed_ics: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::IndustrialCenter
                && entity.is_complete
                && entity.state != AiEntityState::Dead
        })
        .collect();
    if completed_ics.is_empty() {
        return 0;
    }
    let max_dist_px = (config::IC_RESOURCE_MAX_DIST_TILES + 0.5) * observation.map.tile_size as f32;
    let max_dist2 = squared(max_dist_px);
    observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
        .filter(|resource| {
            completed_ics
                .iter()
                .any(|ic| dist2(resource.x, resource.y, ic.x, ic.y) <= max_dist2)
        })
        .count()
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
fn try_build_expansion_industrial_center<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    actions: &mut AiActionContext<'_>,
    builder_pools: &[&[u32]],
    profile: &AiProfile,
    placeable: &mut F,
) -> Option<actions::BuildAction>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let expansion = profile.expansion?;
    let kind = EntityKind::IndustrialCenter;
    config::building_stats(kind)?;
    if !rules::economy::build_requirement_met(kind, facts.complete_building_kinds()) {
        return None;
    }
    if facts.building_count(kind) >= expansion.target_industrial_centers {
        return None;
    }
    let counts = facts.building_counts(kind);
    if counts.incomplete + counts.intended >= profile.buildings.max_pending_per_kind {
        return None;
    }
    let anchor = expansion_anchor_tile(observation)?;
    let start_tile = footprint_top_left_for_center(anchor, kind)?;
    let empty = BTreeSet::new();
    actions::try_build(
        actions,
        builder_pools,
        BuildPlacementRequest {
            building: kind,
            map_width: observation.map.width,
            map_height: observation.map.height,
            start_tile,
            search: ai_shared::BuildSearch {
                min_radius: 0,
                max_radius: expansion.search_radius_tiles,
                prefer_away_from_center: false,
            },
            skip_tiles: &empty,
            placeable: |tx, ty| placeable(kind, tx, ty),
        },
    )
}

fn footprint_top_left_for_center(center_tile: (u32, u32), kind: EntityKind) -> Option<(u32, u32)> {
    let stats = config::building_stats(kind)?;
    Some((
        center_tile.0.saturating_sub(stats.foot_w / 2),
        center_tile.1.saturating_sub(stats.foot_h / 2),
    ))
}

fn expansion_anchor_tile(observation: &AiObservation) -> Option<(u32, u32)> {
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return None;
    }
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let map_center_tiles = (
        observation.map.width as f32 * 0.5,
        observation.map.height as f32 * 0.5,
    );
    let start_resource_radius =
        (config::IC_RESOURCE_MAX_DIST_TILES + 1.5) * observation.map.tile_size as f32;
    let start_resource_radius2 = squared(start_resource_radius);
    let mut best: Option<(u32, (u32, u32), f32)> = None;

    for resource in observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
    {
        if resource_is_near_player_start(observation, resource, start_resource_radius2) {
            continue;
        }
        let resource_tile = (resource.x / tile_size, resource.y / tile_size);
        let dir = (
            map_center_tiles.0 - resource_tile.0,
            map_center_tiles.1 - resource_tile.1,
        );
        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
        if len <= f32::EPSILON {
            continue;
        }
        let estimated_center = (
            resource_tile.0 - dir.0 / len * config::STEEL_BLOCK_DIST_TILES,
            resource_tile.1 - dir.1 / len * config::STEEL_BLOCK_DIST_TILES,
        );
        if !estimated_center.0.is_finite() || !estimated_center.1.is_finite() {
            continue;
        }
        let tile = (
            estimated_center
                .0
                .round()
                .clamp(0.0, observation.map.width.saturating_sub(1) as f32) as u32,
            estimated_center
                .1
                .round()
                .clamp(0.0, observation.map.height.saturating_sub(1) as f32) as u32,
        );
        let center = tile_center(tile, observation.map.tile_size);
        let distance2 = dist2(center.0, center.1, own.0, own.1);
        let better = best
            .map(|(best_id, _, best_distance2)| {
                distance2 < best_distance2 || (distance2 == best_distance2 && resource.id < best_id)
            })
            .unwrap_or(true);
        if better {
            best = Some((resource.id, tile, distance2));
        }
    }

    best.map(|(_, tile, _)| tile)
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
    if expansion_blocks_tank_tech(facts, profile) {
        return 0;
    }
    let current_steel_workers = resource_worker_counts(observation)
        .get(&EntityKind::Steel)
        .copied()
        .unwrap_or(0);
    if current_steel_workers < target_steel_workers.min(profile.resources.oil_after_steel_workers) {
        return 0;
    }

    let Some(policy) = profile.resources.tank_adaptive else {
        return profile.workers.extra_oil_workers;
    };

    let max_oil_workers = profile
        .workers
        .extra_oil_workers
        .min(policy.max_oil_workers);
    if max_oil_workers == 0 {
        return 0;
    }
    let tank_factories = facts
        .complete_building_count(EntityKind::TankFactory)
        .max(1);
    let mut target = tank_factories
        .saturating_mul(policy.oil_workers_per_tank_factory)
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
    } else if facts.complete_building_count(EntityKind::TankFactory) == 0 {
        EntityKind::TankFactory
    } else {
        EntityKind::Tank
    };
    let (steel, oil) = rules::economy::cost(kind);
    let scale = if kind == EntityKind::Tank {
        facts
            .complete_building_count(EntityKind::TankFactory)
            .max(1) as u32
    } else {
        1
    };
    Some(ResourceGoal {
        steel: steel.saturating_mul(scale),
        oil: oil.saturating_mul(scale),
    })
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
        let home_resource_radius2 = squared((config::IC_RESOURCE_MAX_DIST_TILES + 1.5) * tile_size);
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
        AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiPlayerSummary, AiResourceSummary,
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
        let mut owned = vec![building(10, EntityKind::IndustrialCenter, Some(0))];
        owned.extend((0..4).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: config::STARTING_STEEL,
                oil: 0,
                supply_used: config::STARTING_WORKERS,
                supply_cap: config::INDUSTRIAL_CENTER_SUPPLY,
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
                        if building == EntityKind::Barracks.to_protocol_str()
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
        let mut owned = vec![building(10, EntityKind::IndustrialCenter, Some(0))];
        owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: 75,
                oil: 0,
                supply_used: 5,
                supply_cap: config::INDUSTRIAL_CENTER_SUPPLY,
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
        let mut owned = vec![building(10, EntityKind::IndustrialCenter, Some(0))];
        owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: 150,
                oil: 0,
                supply_used: 5,
                supply_cap: config::INDUSTRIAL_CENTER_SUPPLY,
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
                } if building == EntityKind::Barracks.to_protocol_str() => {
                    Some((*worker, (*tile_x, *tile_y)))
                }
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
        let mut owned = vec![building(10, EntityKind::IndustrialCenter, Some(0))];
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
                supply_cap: config::INDUSTRIAL_CENTER_SUPPLY,
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
                } if building == EntityKind::Barracks.to_protocol_str() => {
                    Some((*worker, (*tile_x, *tile_y)))
                }
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
                        && building == EntityKind::Barracks.to_protocol_str()
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
        let mut owned = vec![building(10, EntityKind::IndustrialCenter, Some(0))];
        owned.extend((0..5).map(|i| worker(20 + i, AiEntityState::Idle)));
        let observation = observation(
            AiEconomy {
                steel: 150,
                oil: 0,
                supply_used: 5,
                supply_cap: config::INDUSTRIAL_CENTER_SUPPLY,
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
                    if building == EntityKind::Barracks.to_protocol_str()
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
    fn tech_to_tanks_delays_oil_until_steel_floor_and_builds_tank_factory() {
        let mut owned = vec![
            building(10, EntityKind::IndustrialCenter, Some(0)),
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
            kind: EntityKind::TankFactory
        }));
        assert!(
            decision.intents.contains(&AiIntent::Train {
                kind: EntityKind::Worker
            }),
            "tech_to_tanks should keep worker production alive while saving for the factory"
        );
        assert!(
            !decision.intents.contains(&AiIntent::Train {
                kind: EntityKind::Rifleman
            }),
            "tech_to_tanks should save barracks steel once the tank factory is buildable"
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
            building(10, EntityKind::IndustrialCenter, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
            building(12, EntityKind::TrainingCentre, None),
        ];
        steel_floor_owned.extend((0..8).map(|i| steel_worker(20 + i, 100 + i)));
        steel_floor_owned.extend((0..2).map(|i| worker(40 + i, AiEntityState::Idle)));
        let steel_floor_observation = observation(
            AiEconomy {
                steel: 1_000,
                oil: 1_000,
                supply_used: 10,
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
    fn steel_expansion_tanks_expands_before_tank_tech_after_defensive_rifles() {
        let ts = config::TILE_SIZE as f32;
        let mut owned = vec![
            building_at(
                10,
                EntityKind::IndustrialCenter,
                Some(0),
                8.5 * ts,
                8.5 * ts,
            ),
            building(11, EntityKind::Barracks, Some(0)),
        ];
        owned.extend((0..3).map(|i| combat(30 + i, EntityKind::Rifleman)));
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
            kind: EntityKind::IndustrialCenter
        }));
        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TrainingCentre
        }));
        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TankFactory
        }));
        assert!(
            !decision.intents.iter().any(|intent| matches!(
                intent,
                AiIntent::Gather {
                    resource: EntityKind::Oil,
                    ..
                }
            )),
            "expansion profile should not move into oil before the second IC is complete"
        );
    }

    #[test]
    fn steel_expansion_tanks_trains_defensive_rifles_before_expanding() {
        let mut owned = vec![
            building(10, EntityKind::IndustrialCenter, Some(0)),
            building(11, EntityKind::Barracks, Some(0)),
        ];
        owned.push(combat(30, EntityKind::Rifleman));
        owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
        owned.push(worker(60, AiEntityState::Idle));
        let observation = with_expansion_resources(observation(
            AiEconomy {
                steel: 500,
                oil: 500,
                supply_used: 10,
                supply_cap: 30,
            },
            owned,
        ));

        let decision = decide(
            &observation,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::IndustrialCenter
        }));
        assert!(!decision.intents.contains(&AiIntent::Build {
            kind: EntityKind::TrainingCentre
        }));
        assert!(decision.intents.contains(&AiIntent::Train {
            kind: EntityKind::Rifleman
        }));
    }

    #[test]
    fn steel_expansion_tanks_biases_idle_workers_toward_tank_bottleneck() {
        let ts = config::TILE_SIZE as f32;
        let base_owned = || {
            let mut owned = vec![
                building_at(
                    10,
                    EntityKind::IndustrialCenter,
                    Some(0),
                    8.5 * ts,
                    8.5 * ts,
                ),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building_at(
                    13,
                    EntityKind::IndustrialCenter,
                    Some(0),
                    16.5 * ts,
                    32.5 * ts,
                ),
            ];
            owned.extend((0..3).map(|i| combat(30 + i, EntityKind::Rifleman)));
            owned.extend((0..8).map(|i| steel_worker(40 + i, 100 + i)));
            owned.extend((0..6).map(|i| worker(60 + i, AiEntityState::Idle)));
            owned
        };

        let oil_starved = with_expansion_resources(observation(
            AiEconomy {
                steel: 1_000,
                oil: 0,
                supply_used: 17,
                supply_cap: 40,
            },
            base_owned(),
        ));
        let oil_starved_decision = decide(
            &oil_starved,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        let oil_assignments = oil_starved_decision
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
            "oil-starved tank tech should send most idle workers to oil, got {oil_assignments}"
        );

        let steel_starved = with_expansion_resources(observation(
            AiEconomy {
                steel: 0,
                oil: 1_000,
                supply_used: 17,
                supply_cap: 40,
            },
            base_owned(),
        ));
        let steel_starved_decision = decide(
            &steel_starved,
            &STEEL_EXPANSION_TANKS,
            &mut AiDecisionMemory::for_profile(&STEEL_EXPANSION_TANKS),
        );

        let mut oil_assignments = 0usize;
        let mut steel_assignments = 0usize;
        for intent in &steel_starved_decision.intents {
            match intent {
                AiIntent::Gather {
                    resource: EntityKind::Oil,
                    assignments,
                } => oil_assignments += *assignments,
                AiIntent::Gather {
                    resource: EntityKind::Steel,
                    assignments,
                } => steel_assignments += *assignments,
                _ => {}
            }
        }
        assert!(
            steel_assignments > oil_assignments,
            "steel-starved tank tech should favor steel workers, got steel={steel_assignments}, oil={oil_assignments}"
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
                building(10, EntityKind::IndustrialCenter, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building(13, EntityKind::TankFactory, Some(0)),
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
                building(10, EntityKind::IndustrialCenter, Some(0)),
                building(11, EntityKind::Barracks, Some(0)),
                building(12, EntityKind::TrainingCentre, None),
                building(13, EntityKind::TankFactory, Some(0)),
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
