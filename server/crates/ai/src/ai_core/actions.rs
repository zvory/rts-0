use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::facts::{AiFacts, ProductionBuildingFact};
use crate::ai_core::observation::{AiEntityState, AiEntitySummary, AiResourceSummary};
use crate::ai_shared;
use crate::config;
use rts_rules;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::{self, UpgradeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpendBudget {
    steel: u32,
    oil: u32,
    free_supply: u32,
}

impl SpendBudget {
    pub(crate) fn new(steel: u32, oil: u32, supply_used: u32, supply_cap: u32) -> Self {
        Self::with_committed_steel(steel, oil, supply_used, supply_cap, 0)
    }

    pub(crate) fn with_committed_steel(
        steel: u32,
        oil: u32,
        supply_used: u32,
        supply_cap: u32,
        committed_steel: u32,
    ) -> Self {
        Self {
            steel: steel.saturating_sub(committed_steel),
            oil,
            free_supply: supply_cap.saturating_sub(supply_used),
        }
    }

    pub(crate) fn free_supply(&self) -> u32 {
        self.free_supply
    }

    pub(crate) fn steel(&self) -> u32 {
        self.steel
    }

    pub(crate) fn oil(&self) -> u32 {
        self.oil
    }

    pub(crate) fn can_afford_unit(&self, kind: EntityKind) -> bool {
        if config::unit_stats(kind).is_none() {
            return false;
        }
        let (steel, oil) = rts_rules::economy::cost(kind);
        let supply = rts_rules::economy::supply_cost(kind);
        self.steel >= steel && self.oil >= oil && self.free_supply >= supply
    }

    pub(crate) fn reserve_unit(&mut self, kind: EntityKind) -> bool {
        if config::unit_stats(kind).is_none() {
            return false;
        }
        let (steel, oil) = rts_rules::economy::cost(kind);
        let supply = rts_rules::economy::supply_cost(kind);
        self.reserve_cost(steel, oil, supply)
    }

    pub(crate) fn can_afford_building(&self, kind: EntityKind) -> bool {
        if config::building_stats(kind).is_none() {
            return false;
        }
        let (steel, oil) = rts_rules::economy::cost(kind);
        self.steel >= steel && self.oil >= oil
    }

    pub(crate) fn reserve_building(&mut self, kind: EntityKind) -> bool {
        if config::building_stats(kind).is_none() {
            return false;
        }
        let (steel, oil) = rts_rules::economy::cost(kind);
        self.reserve_cost(steel, oil, 0)
    }

    pub(crate) fn reserve_upgrade(&mut self, kind: UpgradeKind) -> bool {
        let definition = upgrade::definition(kind);
        self.reserve_cost(definition.cost_steel, definition.cost_oil, 0)
    }

    fn reserve_cost(&mut self, steel: u32, oil: u32, supply: u32) -> bool {
        if self.steel < steel || self.oil < oil || self.free_supply < supply {
            return false;
        }
        self.steel -= steel;
        self.oil -= oil;
        self.free_supply -= supply;
        true
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AiReservations {
    workers: BTreeSet<u32>,
    resource_nodes: BTreeSet<u32>,
    production_buildings: BTreeSet<u32>,
}

impl AiReservations {
    pub(crate) fn reserve_worker(&mut self, worker: u32) -> bool {
        self.workers.insert(worker)
    }

    pub(crate) fn worker_reserved(&self, worker: u32) -> bool {
        self.workers.contains(&worker)
    }

    pub(crate) fn reserve_resource_node(&mut self, node: u32) -> bool {
        self.resource_nodes.insert(node)
    }

    pub(crate) fn resource_node_reserved(&self, node: u32) -> bool {
        self.resource_nodes.contains(&node)
    }

    fn reserve_production_building(&mut self, building: u32) -> bool {
        self.production_buildings.insert(building)
    }

    fn production_building_reserved(&self, building: u32) -> bool {
        self.production_buildings.contains(&building)
    }
}

pub(crate) struct AiActionContext<'a> {
    _facts: &'a AiFacts,
    budget: SpendBudget,
    reservations: AiReservations,
    emitted: Vec<Command>,
    command_trace: Vec<String>,
}

impl<'a> AiActionContext<'a> {
    pub(crate) fn new(facts: &'a AiFacts, budget: SpendBudget) -> Self {
        Self {
            _facts: facts,
            budget,
            reservations: AiReservations::default(),
            emitted: Vec::new(),
            command_trace: Vec::new(),
        }
    }

    pub(crate) fn budget(&self) -> &SpendBudget {
        &self.budget
    }

    pub(crate) fn reservations(&self) -> &AiReservations {
        &self.reservations
    }

    pub(crate) fn command_trace(&self) -> &[String] {
        &self.command_trace
    }

    pub(crate) fn emit_command(&mut self, command: Command) {
        self.command_trace.push(command_trace_label(&command));
        self.emitted.push(command);
    }

    pub(crate) fn into_commands(self) -> Vec<Command> {
        self.emitted
    }

    pub(crate) fn reserve_worker_from_pools(&mut self, worker_pools: &[&[u32]]) -> Option<u32> {
        for pool in worker_pools {
            for worker in *pool {
                if self.reservations.reserve_worker(*worker) {
                    return Some(*worker);
                }
            }
        }
        None
    }

    pub(crate) fn reserve_worker(&mut self, worker: u32) -> bool {
        self.reservations.reserve_worker(worker)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReservationCounts {
    pub(crate) workers: usize,
    pub(crate) resource_nodes: usize,
    pub(crate) production_buildings: usize,
}

impl AiReservations {
    pub(crate) fn counts(&self) -> ReservationCounts {
        ReservationCounts {
            workers: self.workers.len(),
            resource_nodes: self.resource_nodes.len(),
            production_buildings: self.production_buildings.len(),
        }
    }
}

fn command_trace_label(command: &Command) -> String {
    match command {
        Command::Move {
            units,
            x,
            y,
            queued,
        } => format!(
            "move units={} x={:.1} y={:.1} queued={}",
            id_list(units),
            x,
            y,
            queued
        ),
        Command::Attack {
            units,
            target,
            queued,
        } => format!(
            "attack units={} target={} queued={}",
            id_list(units),
            target,
            queued
        ),
        Command::AttackMove {
            units,
            x,
            y,
            queued,
        } => format!(
            "attack_move units={} x={:.1} y={:.1} queued={}",
            id_list(units),
            x,
            y,
            queued
        ),
        Command::Gather {
            units,
            node,
            queued,
        } => format!(
            "gather units={} node={} queued={}",
            id_list(units),
            node,
            queued
        ),
        Command::Build {
            units,
            building,
            tile_x,
            tile_y,
            queued,
        } => format!(
            "build units={} building={:?} tile={},{} queued={}",
            id_list(units),
            building,
            tile_x,
            tile_y,
            queued
        ),
        Command::Deconstruct {
            units,
            target,
            queued,
        } => format!(
            "deconstruct units={} target={} queued={}",
            id_list(units),
            target,
            queued
        ),
        Command::Train { building, unit } => {
            format!("train building={} unit={:?}", building, unit)
        }
        Command::Research { building, upgrade } => {
            format!("research building={} upgrade={:?}", building, upgrade)
        }
        Command::Cancel { building } => format!("cancel building={}", building),
        Command::Stop { units } => format!("stop units={}", id_list(units)),
        Command::HoldPosition { units } => format!("hold_position units={}", id_list(units)),
        Command::SetRally {
            building,
            x,
            y,
            kind,
            queued,
        } => {
            format!(
                "set_rally building={} x={:.1} y={:.1} kind={:?} queued={}",
                building, x, y, kind, queued
            )
        }
        Command::SetupAntiTankGuns {
            units,
            x,
            y,
            queued,
        } => {
            format!(
                "setup_anti_tank_guns units={} x={:.1} y={:.1} queued={}",
                id_list(units),
                x,
                y,
                queued
            )
        }
        Command::TearDownAntiTankGuns { units } => {
            format!("tear_down_anti_tank_guns units={}", id_list(units))
        }
        Command::UseAbility {
            units,
            ability,
            x,
            y,
            queued,
        } => format!(
            "use_ability units={} ability={:?} x={:?} y={:?} queued={}",
            id_list(units),
            ability,
            x,
            y,
            queued
        ),
        Command::RecastAbility {
            units,
            ability,
            target_object_id,
            queued,
        } => format!(
            "recast_ability units={} ability={:?} target_object_id={:?} queued={}",
            id_list(units),
            ability,
            target_object_id,
            queued
        ),
        Command::SetAutocast {
            units,
            ability,
            enabled,
        } => format!(
            "set_autocast units={} ability={:?} enabled={}",
            id_list(units),
            ability,
            enabled
        ),
        Command::Rejected { reason } => format!("rejected reason={:?}", reason),
    }
}

fn id_list(ids: &[u32]) -> String {
    ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

pub(crate) struct BuildPlacementRequest<'a, F>
where
    F: FnMut(u32, u32) -> bool,
{
    pub(crate) building: EntityKind,
    pub(crate) map_width: u32,
    pub(crate) map_height: u32,
    pub(crate) start_tile: (u32, u32),
    pub(crate) search: ai_shared::BuildSearch,
    pub(crate) skip_tiles: &'a BTreeSet<(u32, u32)>,
    pub(crate) placeable: F,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BuildAction {
    pub(crate) worker: u32,
    pub(crate) building: EntityKind,
    pub(crate) tile_x: u32,
    pub(crate) tile_y: u32,
}

pub(crate) fn try_build<F>(
    ctx: &mut AiActionContext<'_>,
    worker_pools: &[&[u32]],
    mut request: BuildPlacementRequest<'_, F>,
) -> Option<BuildAction>
where
    F: FnMut(u32, u32) -> bool,
{
    if !ctx.budget.can_afford_building(request.building) {
        return None;
    }
    let (tile_x, tile_y) = ai_shared::find_build_spot_near_start_with(
        request.map_width,
        request.map_height,
        request.start_tile,
        request.building,
        request.search,
        request.skip_tiles,
        &mut request.placeable,
    )?;
    try_build_at(ctx, worker_pools, request.building, tile_x, tile_y)
}

pub(crate) fn try_build_at(
    ctx: &mut AiActionContext<'_>,
    worker_pools: &[&[u32]],
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<BuildAction> {
    if !ctx.budget.can_afford_building(building) {
        return None;
    }
    let worker = ctx.reserve_worker_from_pools(worker_pools)?;
    if !ctx.budget.reserve_building(building) {
        return None;
    }
    ctx.emit_command(Command::Build {
        units: vec![worker],
        building,
        tile_x,
        tile_y,
        queued: false,
    });
    Some(BuildAction {
        worker,
        building,
        tile_x,
        tile_y,
    })
}

pub(crate) struct TrainUnitsRequest<'a> {
    pub(crate) buildings: &'a [ProductionBuildingFact],
    pub(crate) unit_priorities: &'a [EntityKind],
    pub(crate) completed_building_kinds: &'a [EntityKind],
    pub(crate) completed_upgrades: &'a [UpgradeKind],
    pub(crate) max_queue_depth: usize,
    pub(crate) save_for_tech: bool,
    pub(crate) current_counts: &'a [(EntityKind, usize)],
    pub(crate) max_counts: &'a [(EntityKind, usize)],
    pub(crate) balance_unit_priorities: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TrainAction {
    pub(crate) building: u32,
    pub(crate) unit: EntityKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResearchAction {
    pub(crate) building: u32,
    pub(crate) upgrade: UpgradeKind,
}

pub(crate) fn try_research_upgrade(
    ctx: &mut AiActionContext<'_>,
    buildings: &[ProductionBuildingFact],
    upgrade: UpgradeKind,
) -> Option<ResearchAction> {
    let definition = upgrade::definition(upgrade);
    let building = buildings
        .iter()
        .find(|building| building.kind == definition.researched_at && building.queue_len == 0)?;
    if ctx.reservations.production_building_reserved(building.id) {
        return None;
    }
    if !ctx.budget.reserve_upgrade(upgrade) {
        return None;
    }
    ctx.reservations.reserve_production_building(building.id);
    ctx.emit_command(Command::Research {
        building: building.id,
        upgrade,
    });
    Some(ResearchAction {
        building: building.id,
        upgrade,
    })
}

pub(crate) fn train_units(
    ctx: &mut AiActionContext<'_>,
    request: TrainUnitsRequest<'_>,
) -> Vec<TrainAction> {
    if request.save_for_tech {
        return Vec::new();
    }

    let mut current_counts: BTreeMap<EntityKind, usize> =
        request.current_counts.iter().copied().collect();
    let max_counts: BTreeMap<EntityKind, usize> = request.max_counts.iter().copied().collect();
    let mut trained = Vec::new();

    for building in request.buildings {
        if building.queue_len >= request.max_queue_depth {
            continue;
        }
        if ctx.reservations.production_building_reserved(building.id) {
            continue;
        }

        let Some(unit) = train_unit_choice(TrainUnitChoiceRequest {
            unit_priorities: request.unit_priorities,
            trainable: rts_rules::economy::trainable_units(building.kind),
            completed_building_kinds: request.completed_building_kinds,
            completed_upgrades: request.completed_upgrades,
            current_counts: &current_counts,
            max_counts: &max_counts,
            balance_unit_priorities: request.balance_unit_priorities,
            budget: &ctx.budget,
        }) else {
            continue;
        };

        if !ctx.budget.reserve_unit(unit) {
            continue;
        }
        ctx.reservations.reserve_production_building(building.id);
        *current_counts.entry(unit).or_default() += 1;
        ctx.emit_command(Command::Train {
            building: building.id,
            unit,
        });
        trained.push(TrainAction {
            building: building.id,
            unit,
        });
    }

    trained
}

struct TrainUnitChoiceRequest<'a> {
    unit_priorities: &'a [EntityKind],
    trainable: &'a [EntityKind],
    completed_building_kinds: &'a [EntityKind],
    completed_upgrades: &'a [UpgradeKind],
    current_counts: &'a BTreeMap<EntityKind, usize>,
    max_counts: &'a BTreeMap<EntityKind, usize>,
    balance_unit_priorities: bool,
    budget: &'a SpendBudget,
}

fn train_unit_choice(request: TrainUnitChoiceRequest<'_>) -> Option<EntityKind> {
    let mut eligible = Vec::new();
    for (index, unit) in request.unit_priorities.iter().copied().enumerate() {
        if !request.trainable.contains(&unit) {
            continue;
        }
        if !rts_rules::economy::train_requirement_met(unit, request.completed_building_kinds) {
            continue;
        }
        if upgrade::required_for_unit(unit)
            .is_some_and(|upgrade| !request.completed_upgrades.contains(&upgrade))
        {
            continue;
        }
        let current = request.current_counts.get(&unit).copied().unwrap_or(0);
        if request
            .max_counts
            .get(&unit)
            .map(|max| current >= *max)
            .unwrap_or(false)
        {
            continue;
        }
        if !request.budget.can_afford_unit(unit) {
            continue;
        }
        eligible.push((index, unit));
    }

    if request.balance_unit_priorities {
        eligible
            .into_iter()
            .min_by_key(|(index, unit)| {
                (
                    request.current_counts.get(unit).copied().unwrap_or(0),
                    *index,
                )
            })
            .map(|(_, unit)| unit)
    } else {
        eligible.into_iter().map(|(_, unit)| unit).next()
    }
}

pub(crate) struct ResourceAssignmentPolicy<'a> {
    pub(crate) workers: &'a [AiEntitySummary],
    pub(crate) resources: &'a [AiResourceSummary],
    pub(crate) resource_kind: EntityKind,
    pub(crate) assignable_node_ids: &'a BTreeSet<u32>,
    pub(crate) candidate_worker_ids: Option<&'a [u32]>,
    pub(crate) skip_workers: &'a BTreeSet<u32>,
    pub(crate) pre_reserved_nodes: &'a BTreeSet<u32>,
    pub(crate) idle_only: bool,
    pub(crate) allow_latched_reassignment: bool,
    pub(crate) max_assignments: Option<usize>,
    pub(crate) max_worker_resource_distance_px: Option<f32>,
    pub(crate) remote_worker_assignment_fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResourceAssignment {
    pub(crate) worker: u32,
    pub(crate) node: u32,
}

pub(crate) fn assign_workers_to_resource(
    ctx: &mut AiActionContext<'_>,
    policy: ResourceAssignmentPolicy<'_>,
) -> Vec<ResourceAssignment> {
    if !policy.resource_kind.is_node() {
        return Vec::new();
    }

    let workers_by_id: BTreeMap<u32, &AiEntitySummary> = policy
        .workers
        .iter()
        .map(|worker| (worker.id, worker))
        .collect();
    let candidate_ids = candidate_worker_ids(policy.workers, policy.candidate_worker_ids);
    let mut assignments = Vec::new();

    for worker_id in candidate_ids {
        if policy
            .max_assignments
            .map(|max| assignments.len() >= max)
            .unwrap_or(false)
        {
            break;
        }
        if ctx.reservations.worker_reserved(worker_id) || policy.skip_workers.contains(&worker_id) {
            continue;
        }
        let Some(worker) = workers_by_id.get(&worker_id).copied() else {
            continue;
        };
        if worker.state == AiEntityState::Build {
            continue;
        }
        if policy.idle_only && worker.state != AiEntityState::Idle {
            continue;
        }
        if worker.latched_node.is_some() && !policy.allow_latched_reassignment {
            continue;
        }

        let Some(node) = nearest_unreserved_node(worker, &policy, ctx.reservations()) else {
            continue;
        };
        let pump_jack_site = if policy.resource_kind == EntityKind::Oil {
            let Some(resource) = resource_by_id(policy.resources, node) else {
                continue;
            };
            Some(pump_jack_tile_for_resource(resource))
        } else {
            None
        };
        ctx.reservations.reserve_worker(worker.id);
        ctx.reservations.reserve_resource_node(node);
        if let Some((tile_x, tile_y)) = pump_jack_site {
            ctx.emit_command(Command::Build {
                units: vec![worker.id],
                building: EntityKind::PumpJack,
                tile_x,
                tile_y,
                queued: false,
            });
        } else {
            ctx.emit_command(Command::Gather {
                units: vec![worker.id],
                node,
                queued: false,
            });
        }
        assignments.push(ResourceAssignment {
            worker: worker.id,
            node,
        });
    }

    assignments
}

#[cfg(test)]
pub(crate) fn ready_attack_wave<T>(
    units: impl IntoIterator<Item = T>,
    min_size: usize,
    mut select: impl FnMut(T) -> Option<u32>,
) -> Option<Vec<u32>> {
    let mut ids: Vec<u32> = units.into_iter().filter_map(&mut select).collect();
    ids.sort_unstable();
    ids.dedup();
    (ids.len() >= min_size).then_some(ids)
}

pub(crate) fn attack_move_units(
    ctx: &mut AiActionContext<'_>,
    units: impl IntoIterator<Item = u32>,
    x: f32,
    y: f32,
) -> Option<Vec<u32>> {
    let mut units: Vec<u32> = units.into_iter().collect();
    units.sort_unstable();
    units.dedup();
    if units.is_empty() {
        return None;
    }
    ctx.emit_command(Command::AttackMove {
        units: units.clone(),
        x,
        y,
        queued: false,
    });
    Some(units)
}

pub(crate) fn move_units(
    ctx: &mut AiActionContext<'_>,
    units: impl IntoIterator<Item = u32>,
    x: f32,
    y: f32,
) -> Option<Vec<u32>> {
    move_units_with_queue(ctx, units, x, y, false)
}

pub(crate) fn move_units_with_queue(
    ctx: &mut AiActionContext<'_>,
    units: impl IntoIterator<Item = u32>,
    x: f32,
    y: f32,
    queued: bool,
) -> Option<Vec<u32>> {
    let mut units: Vec<u32> = units.into_iter().collect();
    units.sort_unstable();
    units.dedup();
    if units.is_empty() {
        return None;
    }
    ctx.emit_command(Command::Move {
        units: units.clone(),
        x,
        y,
        queued,
    });
    Some(units)
}

pub(crate) fn attack_units(
    ctx: &mut AiActionContext<'_>,
    units: impl IntoIterator<Item = u32>,
    target: u32,
) -> Option<Vec<u32>> {
    let mut units: Vec<u32> = units.into_iter().collect();
    units.sort_unstable();
    units.dedup();
    if units.is_empty() {
        return None;
    }
    ctx.emit_command(Command::Attack {
        units: units.clone(),
        target,
        queued: false,
    });
    Some(units)
}

pub(crate) fn select_ready_combat_units(
    units: &[AiEntitySummary],
    kinds: &[EntityKind],
) -> Vec<u32> {
    select_ready_combat_units_excluding(units, kinds, &BTreeSet::new())
}

pub(crate) fn select_ready_combat_units_excluding(
    units: &[AiEntitySummary],
    kinds: &[EntityKind],
    excluded_units: &BTreeSet<u32>,
) -> Vec<u32> {
    let mut selected: Vec<u32> = units
        .iter()
        .filter(|unit| {
            unit.free_for_combat && kinds.contains(&unit.kind) && !excluded_units.contains(&unit.id)
        })
        .map(|unit| unit.id)
        .collect();
    selected.sort_unstable();
    selected
}

#[allow(dead_code)]
pub(crate) fn stage_units_toward(
    ctx: &mut AiActionContext<'_>,
    units: impl IntoIterator<Item = u32>,
    from: (f32, f32),
    to: (f32, f32),
    tile_size: u32,
    distance_tiles: f32,
) -> Option<Vec<u32>> {
    let (x, y) = point_toward(from, to, distance_tiles * tile_size as f32);
    attack_move_units(ctx, units, x, y)
}

fn candidate_worker_ids(workers: &[AiEntitySummary], explicit_ids: Option<&[u32]>) -> Vec<u32> {
    let ids: Vec<u32> = explicit_ids
        .map(|ids| ids.to_vec())
        .unwrap_or_else(|| workers.iter().map(|worker| worker.id).collect());
    let mut seen = BTreeSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

fn resource_by_id(resources: &[AiResourceSummary], id: u32) -> Option<&AiResourceSummary> {
    resources.iter().find(|resource| resource.id == id)
}

fn pump_jack_tile_for_resource(resource: &AiResourceSummary) -> (u32, u32) {
    let tile_size = config::TILE_SIZE as f32;
    let tile_x = ((resource.x / tile_size) - 0.5).round().max(0.0) as u32;
    let tile_y = ((resource.y / tile_size) - 0.5).round().max(0.0) as u32;
    (tile_x, tile_y)
}

fn nearest_unreserved_node(
    worker: &AiEntitySummary,
    policy: &ResourceAssignmentPolicy<'_>,
    reservations: &AiReservations,
) -> Option<u32> {
    let mut best_local: Option<(u32, f32)> = None;
    let mut best_remote: Option<(u32, f32)> = None;
    let max_distance2 = policy
        .max_worker_resource_distance_px
        .filter(|distance| distance.is_finite() && *distance >= 0.0)
        .map(|distance| distance * distance);
    for node in policy.resources {
        if node.kind != policy.resource_kind || node.remaining == 0 {
            continue;
        }
        if !policy.assignable_node_ids.contains(&node.id) {
            continue;
        }
        if policy.pre_reserved_nodes.contains(&node.id)
            || reservations.resource_node_reserved(node.id)
        {
            continue;
        }
        let d = dist2(worker.x, worker.y, node.x, node.y);
        let beyond_preferred_distance = max_distance2.map(|max| d > max).unwrap_or(false);
        if beyond_preferred_distance && !policy.remote_worker_assignment_fallback {
            continue;
        }
        let best = if beyond_preferred_distance {
            &mut best_remote
        } else {
            &mut best_local
        };
        let better = best
            .map(|(best_id, best_dist)| d < best_dist || (d == best_dist && node.id < best_id))
            .unwrap_or(true);
        if better {
            *best = Some((node.id, d));
        }
    }
    best_local.or(best_remote).map(|(id, _)| id)
}

fn dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

#[allow(dead_code)]
fn point_toward(from: (f32, f32), to: (f32, f32), distance: f32) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON {
        return from;
    }
    let clamped = distance.min(len);
    (from.0 + dx / len * clamped, from.1 + dy / len * clamped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::observation::{AiEconomy, AiMapSummary, AiObservation, AiPlayerSummary};
    use crate::ai_core::resource_availability::ResourceAvailability;

    fn worker(id: u32, x: f32, y: f32, state: AiEntityState) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind: EntityKind::Worker,
            x,
            y,
            state,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn production_building(id: u32, kind: EntityKind, queue_len: usize) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x: 0.0,
            y: 0.0,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: Some(queue_len),
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn complete_building(id: u32, kind: EntityKind) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x: 0.0,
            y: 0.0,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
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
            remaining: 100,
        }
    }

    fn observation(
        economy: AiEconomy,
        owned: Vec<AiEntitySummary>,
        resources: Vec<AiResourceSummary>,
    ) -> AiObservation {
        AiObservation {
            player_id: 1,
            tick: 0,
            map: AiMapSummary {
                width: 32,
                height: 32,
                tile_size: config::TILE_SIZE,
            },
            economy,
            own_start_tile: (8, 8),
            players: vec![AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (8, 8),
                is_ai: false,
                is_alive: true,
            }],
            owned,
            resources,
            visible_allies: Vec::new(),
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        }
    }

    fn budget_from_observation(observation: &AiObservation) -> SpendBudget {
        SpendBudget::new(
            observation.economy.steel,
            observation.economy.oil,
            observation.economy.supply_used,
            observation.economy.supply_cap,
        )
    }

    fn context_from_facts<'a>(
        facts: &'a AiFacts,
        observation: &AiObservation,
    ) -> AiActionContext<'a> {
        AiActionContext::new(facts, budget_from_observation(observation))
    }

    fn facts_from_observation(observation: &AiObservation) -> AiFacts {
        AiFacts::from_observation(observation)
    }

    #[test]
    fn committed_steel_is_reserved_from_budget() {
        let budget = SpendBudget::with_committed_steel(150, 0, 0, 10, 100);

        assert_eq!(budget.steel, 50);
        assert!(!budget.can_afford_building(EntityKind::Depot));
    }

    #[test]
    fn build_action_reserves_worker_and_cost() {
        let observation = observation(
            AiEconomy {
                steel: 100,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
            Vec::new(),
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);
        let empty = BTreeSet::new();
        let workers = [10];

        let action = try_build(
            &mut ctx,
            &[&workers],
            BuildPlacementRequest {
                building: EntityKind::Depot,
                map_width: 32,
                map_height: 32,
                start_tile: (8, 8),
                search: ai_shared::BuildSearch {
                    min_radius: 0,
                    max_radius: 0,
                    prefer_away_from_center: false,
                    prefer_toward_center: false,
                },
                skip_tiles: &empty,
                placeable: |tx, ty| (tx, ty) == (8, 8),
            },
        );

        assert_eq!(
            action,
            Some(BuildAction {
                worker: 10,
                building: EntityKind::Depot,
                tile_x: 8,
                tile_y: 8,
            })
        );
        assert!(ctx.reservations().worker_reserved(10));
        assert_eq!(ctx.budget().steel, 0);
        assert!(matches!(
            ctx.into_commands().as_slice(),
            [Command::Build { units, building, tile_x: 8, tile_y: 8, .. }]
                if units.as_slice() == [10] && *building == EntityKind::Depot
        ));
    }

    #[test]
    fn second_build_action_cannot_reuse_same_worker() {
        let observation = observation(
            AiEconomy {
                steel: 300,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
            Vec::new(),
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);
        let empty = BTreeSet::new();
        let workers = [10];
        let request = || BuildPlacementRequest {
            building: EntityKind::Depot,
            map_width: 32,
            map_height: 32,
            start_tile: (8, 8),
            search: ai_shared::BuildSearch {
                min_radius: 0,
                max_radius: 0,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            skip_tiles: &empty,
            placeable: |_, _| true,
        };

        assert!(try_build(&mut ctx, &[&workers], request()).is_some());
        assert!(try_build(&mut ctx, &[&workers], request()).is_none());
        assert_eq!(ctx.into_commands().len(), 1);
    }

    #[test]
    fn unit_training_respects_local_budget_and_supply() {
        let observation = observation(
            AiEconomy {
                steel: 100,
                oil: 0,
                supply_used: 9,
                supply_cap: 10,
            },
            vec![
                production_building(20, EntityKind::CityCentre, 0),
                production_building(21, EntityKind::CityCentre, 0),
            ],
            Vec::new(),
        );
        let facts = AiFacts::from_observation(&observation);
        let mut ctx = AiActionContext::new(
            &facts,
            SpendBudget::new(
                observation.economy.steel,
                observation.economy.oil,
                observation.economy.supply_used,
                observation.economy.supply_cap,
            ),
        );

        let trained = train_units(
            &mut ctx,
            TrainUnitsRequest {
                buildings: facts.production_buildings(EntityKind::CityCentre),
                unit_priorities: &[EntityKind::Worker],
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: 1,
                save_for_tech: false,
                current_counts: &[(EntityKind::Worker, 0)],
                max_counts: &[(EntityKind::Worker, 2)],
                balance_unit_priorities: false,
            },
        );

        assert_eq!(trained.len(), 1);
        assert_eq!(ctx.budget().free_supply(), 0);
        assert_eq!(ctx.into_commands().len(), 1);
    }

    #[test]
    fn support_training_requires_tech_and_can_balance_priorities() {
        let without_tech = observation(
            AiEconomy {
                steel: 500,
                oil: 200,
                supply_used: 0,
                supply_cap: 20,
            },
            vec![
                production_building(20, EntityKind::Barracks, 0),
                production_building(21, EntityKind::Barracks, 0),
                production_building(22, EntityKind::Steelworks, 0),
            ],
            Vec::new(),
        );
        let facts = AiFacts::from_observation(&without_tech);
        let mut ctx = AiActionContext::new(&facts, budget_from_observation(&without_tech));

        let trained = train_units(
            &mut ctx,
            TrainUnitsRequest {
                buildings: facts.production_buildings(EntityKind::Barracks),
                unit_priorities: &[EntityKind::MachineGunner, EntityKind::AntiTankGun],
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: 1,
                save_for_tech: false,
                current_counts: &[],
                max_counts: &[],
                balance_unit_priorities: true,
            },
        );

        assert!(trained.is_empty());
        assert!(ctx.into_commands().is_empty());

        let with_training_centre = observation(
            AiEconomy {
                steel: 500,
                oil: 200,
                supply_used: 0,
                supply_cap: 20,
            },
            vec![
                production_building(20, EntityKind::Barracks, 0),
                production_building(21, EntityKind::Barracks, 0),
                production_building(22, EntityKind::Steelworks, 0),
                complete_building(30, EntityKind::TrainingCentre),
            ],
            Vec::new(),
        );
        let facts = AiFacts::from_observation(&with_training_centre);
        let mut ctx = AiActionContext::new(&facts, budget_from_observation(&with_training_centre));

        let trained = train_units(
            &mut ctx,
            TrainUnitsRequest {
                buildings: facts.production_buildings(EntityKind::Barracks),
                unit_priorities: &[EntityKind::MachineGunner, EntityKind::AntiTankGun],
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: 1,
                save_for_tech: false,
                current_counts: &[],
                max_counts: &[],
                balance_unit_priorities: true,
            },
        );

        assert_eq!(
            trained.iter().map(|action| action.unit).collect::<Vec<_>>(),
            vec![EntityKind::MachineGunner, EntityKind::MachineGunner]
        );

        let mut with_steelworks = observation(
            AiEconomy {
                steel: 500,
                oil: 200,
                supply_used: 0,
                supply_cap: 20,
            },
            vec![
                production_building(20, EntityKind::Barracks, 0),
                production_building(21, EntityKind::Barracks, 0),
                production_building(22, EntityKind::Steelworks, 0),
                complete_building(30, EntityKind::TrainingCentre),
                complete_building(31, EntityKind::Steelworks),
            ],
            Vec::new(),
        );
        with_steelworks
            .upgrades
            .push(UpgradeKind::AntiTankGunUnlock);
        let facts = AiFacts::from_observation(&with_steelworks);
        let mut ctx = AiActionContext::new(&facts, budget_from_observation(&with_steelworks));

        let trained = train_units(
            &mut ctx,
            TrainUnitsRequest {
                buildings: facts.production_buildings(EntityKind::Barracks),
                unit_priorities: &[EntityKind::MachineGunner, EntityKind::AntiTankGun],
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: 1,
                save_for_tech: false,
                current_counts: &[],
                max_counts: &[],
                balance_unit_priorities: true,
            },
        );

        assert_eq!(
            trained.iter().map(|action| action.unit).collect::<Vec<_>>(),
            vec![EntityKind::MachineGunner, EntityKind::MachineGunner]
        );

        let trained = train_units(
            &mut ctx,
            TrainUnitsRequest {
                buildings: facts.production_buildings(EntityKind::Steelworks),
                unit_priorities: &[EntityKind::MachineGunner, EntityKind::AntiTankGun],
                completed_building_kinds: facts.complete_building_kinds(),
                completed_upgrades: facts.completed_upgrades(),
                max_queue_depth: 1,
                save_for_tech: false,
                current_counts: &[(EntityKind::MachineGunner, 2)],
                max_counts: &[],
                balance_unit_priorities: true,
            },
        );

        assert_eq!(
            trained.iter().map(|action| action.unit).collect::<Vec<_>>(),
            vec![EntityKind::AntiTankGun]
        );
    }

    #[test]
    fn resource_assignment_picks_distinct_nodes() {
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            vec![
                worker(10, 0.0, 0.0, AiEntityState::Idle),
                worker(11, 8.0, 0.0, AiEntityState::Idle),
            ],
            vec![
                resource(30, EntityKind::Steel, 64.0, 0.0),
                resource(31, EntityKind::Steel, 96.0, 0.0),
            ],
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);
        let empty = BTreeSet::new();
        let assignable_node_ids = BTreeSet::from([30, 31]);

        let assigned = assign_workers_to_resource(
            &mut ctx,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Steel,
                assignable_node_ids: &assignable_node_ids,
                candidate_worker_ids: None,
                skip_workers: &empty,
                pre_reserved_nodes: &empty,
                idle_only: true,
                allow_latched_reassignment: false,
                max_assignments: None,
                max_worker_resource_distance_px: None,
                remote_worker_assignment_fallback: false,
            },
        );

        assert_eq!(assigned.len(), 2);
        assert_eq!(assigned[0].node, 30);
        assert_eq!(assigned[1].node, 31);
    }

    #[test]
    fn resource_assignment_can_reassign_latched_workers_when_allowed() {
        let mut latched = worker(10, 0.0, 0.0, AiEntityState::Gather);
        latched.latched_node = Some(30);
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            vec![latched],
            vec![
                resource(30, EntityKind::Steel, 64.0, 0.0),
                resource(31, EntityKind::Oil, 96.0, 0.0),
            ],
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);
        let empty = BTreeSet::new();
        let assignable_node_ids = BTreeSet::from([31]);

        let assigned = assign_workers_to_resource(
            &mut ctx,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Oil,
                assignable_node_ids: &assignable_node_ids,
                candidate_worker_ids: Some(&[10]),
                skip_workers: &empty,
                pre_reserved_nodes: &empty,
                idle_only: false,
                allow_latched_reassignment: true,
                max_assignments: None,
                max_worker_resource_distance_px: None,
                remote_worker_assignment_fallback: false,
            },
        );

        assert_eq!(
            assigned,
            vec![ResourceAssignment {
                worker: 10,
                node: 31
            }]
        );
        assert!(matches!(
            ctx.into_commands().as_slice(),
            [Command::Build {
                units,
                building: EntityKind::PumpJack,
                tile_x: 0,
                tile_y: 0,
                queued: false
            }] if units == &vec![10]
        ));
    }

    #[test]
    fn resource_assignment_ignores_nodes_beyond_worker_distance_limit() {
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
            vec![
                resource(30, EntityKind::Steel, 64.0, 0.0),
                resource(31, EntityKind::Steel, 640.0, 0.0),
            ],
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);
        let mut reserved = BTreeSet::new();
        reserved.insert(30);
        let empty = BTreeSet::new();
        let assignable_node_ids = BTreeSet::from([30, 31]);

        let assigned = assign_workers_to_resource(
            &mut ctx,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Steel,
                assignable_node_ids: &assignable_node_ids,
                candidate_worker_ids: None,
                skip_workers: &empty,
                pre_reserved_nodes: &reserved,
                idle_only: true,
                allow_latched_reassignment: false,
                max_assignments: None,
                max_worker_resource_distance_px: Some(128.0),
                remote_worker_assignment_fallback: false,
            },
        );

        assert!(assigned.is_empty());
        assert!(ctx.into_commands().is_empty());
    }

    #[test]
    fn resource_assignment_can_fall_back_to_remote_node_after_local_nodes_fill() {
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
            vec![
                resource(30, EntityKind::Steel, 64.0, 0.0),
                resource(31, EntityKind::Steel, 640.0, 0.0),
            ],
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);
        let reserved = BTreeSet::from([30]);
        let empty = BTreeSet::new();
        let assignable_node_ids = BTreeSet::from([30, 31]);

        let assigned = assign_workers_to_resource(
            &mut ctx,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Steel,
                assignable_node_ids: &assignable_node_ids,
                candidate_worker_ids: None,
                skip_workers: &empty,
                pre_reserved_nodes: &reserved,
                idle_only: true,
                allow_latched_reassignment: false,
                max_assignments: None,
                max_worker_resource_distance_px: Some(128.0),
                remote_worker_assignment_fallback: true,
            },
        );

        assert_eq!(
            assigned,
            vec![ResourceAssignment {
                worker: 10,
                node: 31
            }]
        );
    }

    #[test]
    fn assign_workers_to_resource_ignores_non_mineable_oil_without_reserving_worker() {
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            vec![worker(10, 0.0, 0.0, AiEntityState::Idle)],
            vec![
                resource(30, EntityKind::Steel, 32.0, 0.0),
                resource(31, EntityKind::Oil, 64.0, 0.0),
            ],
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);
        let empty = BTreeSet::new();
        let no_mineable_oil = BTreeSet::new();

        let assigned_oil = assign_workers_to_resource(
            &mut ctx,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Oil,
                assignable_node_ids: &no_mineable_oil,
                candidate_worker_ids: Some(&[10]),
                skip_workers: &empty,
                pre_reserved_nodes: &empty,
                idle_only: true,
                allow_latched_reassignment: false,
                max_assignments: Some(1),
                max_worker_resource_distance_px: None,
                remote_worker_assignment_fallback: false,
            },
        );

        assert!(assigned_oil.is_empty());
        assert_eq!(
            ctx.reservations().counts(),
            ReservationCounts {
                workers: 0,
                resource_nodes: 0,
                production_buildings: 0
            }
        );

        let mineable_steel = BTreeSet::from([30]);
        let assigned_steel = assign_workers_to_resource(
            &mut ctx,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Steel,
                assignable_node_ids: &mineable_steel,
                candidate_worker_ids: Some(&[10]),
                skip_workers: &empty,
                pre_reserved_nodes: &empty,
                idle_only: true,
                allow_latched_reassignment: false,
                max_assignments: Some(1),
                max_worker_resource_distance_px: None,
                remote_worker_assignment_fallback: false,
            },
        );

        assert_eq!(
            assigned_steel,
            vec![ResourceAssignment {
                worker: 10,
                node: 30
            }]
        );
        assert!(matches!(
            ctx.into_commands().as_slice(),
            [Command::Gather { units, node: 30, queued: false }] if units == &vec![10]
        ));
    }

    #[test]
    fn assign_workers_to_resource_accepts_completed_expansion_oil_candidate() {
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            vec![
                complete_building(20, EntityKind::CityCentre),
                worker(10, 0.0, 0.0, AiEntityState::Idle),
            ],
            vec![resource(31, EntityKind::Oil, 0.0, 0.0)],
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);
        let empty = BTreeSet::new();
        let availability = ResourceAvailability::from_observation(&observation, &empty);
        let mineable_oil = availability.free_mineable_node_ids(EntityKind::Oil);

        let assigned = assign_workers_to_resource(
            &mut ctx,
            ResourceAssignmentPolicy {
                workers: &observation.owned,
                resources: &observation.resources,
                resource_kind: EntityKind::Oil,
                assignable_node_ids: &mineable_oil,
                candidate_worker_ids: Some(&[10]),
                skip_workers: &empty,
                pre_reserved_nodes: &empty,
                idle_only: true,
                allow_latched_reassignment: false,
                max_assignments: Some(1),
                max_worker_resource_distance_px: None,
                remote_worker_assignment_fallback: false,
            },
        );

        assert_eq!(
            assigned,
            vec![ResourceAssignment {
                worker: 10,
                node: 31
            }]
        );
    }

    #[test]
    fn attack_command_unit_order_is_deterministic() {
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            Vec::new(),
            Vec::new(),
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);

        let units = attack_move_units(&mut ctx, [5, 3, 4, 3], 10.0, 20.0);

        assert_eq!(units, Some(vec![3, 4, 5]));
        assert!(matches!(
            ctx.into_commands().as_slice(),
            [Command::AttackMove { units, x: 10.0, y: 20.0, .. }]
                if units == &vec![3, 4, 5]
        ));
    }

    #[test]
    fn move_command_unit_order_is_deterministic() {
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            Vec::new(),
            Vec::new(),
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);

        let units = move_units(&mut ctx, [5, 3, 4, 3], 10.0, 20.0);

        assert_eq!(units, Some(vec![3, 4, 5]));
        assert!(matches!(
            ctx.into_commands().as_slice(),
            [Command::Move { units, x: 10.0, y: 20.0, .. }]
                if units == &vec![3, 4, 5]
        ));
    }

    #[test]
    fn staging_uses_point_toward_target_and_deterministic_units() {
        let observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 10,
            },
            Vec::new(),
            Vec::new(),
        );
        let facts = facts_from_observation(&observation);
        let mut ctx = context_from_facts(&facts, &observation);

        let units = stage_units_toward(&mut ctx, [8, 6, 8], (0.0, 0.0), (96.0, 0.0), 32, 2.0);

        assert_eq!(units, Some(vec![6, 8]));
        assert!(matches!(
            ctx.into_commands().as_slice(),
            [Command::AttackMove { units, x: 64.0, y: 0.0, .. }]
                if units == &vec![6, 8]
        ));
    }
}
