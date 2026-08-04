use std::collections::{BTreeMap, BTreeSet};

use rts_rules::faction::UpgradeKind;
use rts_rules::EntityKind;
use rts_sim::game::upgrade;

use super::AiFrame;

/// Maximum actions retained from one strategy step.
pub const MAX_ACTIONS_PER_STEP: usize = 256;

/// The independent namespace in which an id is reserved for this think.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservationNamespace {
    Actor,
    ResourceNode,
    Producer,
}

/// A fact known by the local action builder that prevented command emission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionBlocker {
    EmptyGroup,
    EmptyCandidates(ReservationNamespace),
    NoKnownCandidate(ReservationNamespace),
    UnsupportedKind(EntityKind),
    NoCompatibleProducer,
    InsufficientBudget {
        steel: u32,
        oil: u32,
        supply: u32,
    },
    AlreadyReserved {
        namespace: ReservationNamespace,
        id: u32,
    },
    ActionCapacity,
}

/// A local-preflight error. It says nothing about later simulation validation or completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionError {
    blocker: ActionBlocker,
}

impl ActionError {
    pub fn blocker(&self) -> &ActionBlocker {
        &self.blocker
    }
}

impl From<ActionBlocker> for ActionError {
    fn from(blocker: ActionBlocker) -> Self {
        Self { blocker }
    }
}

/// A canonical non-empty tactical group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitGroup(Vec<u32>);

impl UnitGroup {
    pub fn new(units: impl IntoIterator<Item = u32>) -> Result<Self, ActionError> {
        let mut units = units.into_iter().collect::<Vec<_>>();
        units.sort_unstable();
        units.dedup();
        if units.is_empty() {
            return Err(ActionBlocker::EmptyGroup.into());
        }
        Ok(Self(units))
    }

    pub fn units(&self) -> &[u32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionBudgetSnapshot {
    pub steel: u32,
    pub oil: u32,
    pub free_supply: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservationCounts {
    pub actors: usize,
    pub resource_nodes: usize,
    pub producers: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActionBudget {
    steel: u32,
    oil: u32,
    free_supply: u32,
}

impl ActionBudget {
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
        kind.is_unit() && self.can_afford(cost_for_unit(kind))
    }

    pub(crate) fn can_afford_building(&self, kind: EntityKind) -> bool {
        kind.is_building() && self.can_afford(cost_for_building(kind))
    }

    pub(crate) fn reserve_unit(&mut self, kind: EntityKind) -> bool {
        let cost = cost_for_unit(kind);
        if !kind.is_unit() || !self.can_afford(cost) {
            return false;
        }
        self.reserve(cost);
        true
    }

    pub(crate) fn reserve_building(&mut self, kind: EntityKind) -> bool {
        let cost = cost_for_building(kind);
        if !kind.is_building() || !self.can_afford(cost) {
            return false;
        }
        self.reserve(cost);
        true
    }

    pub(crate) fn reserve_upgrade(&mut self, kind: UpgradeKind) -> bool {
        let definition = upgrade::definition(kind);
        let cost = ActionCost {
            steel: definition.cost_steel,
            oil: definition.cost_oil,
            supply: 0,
        };
        if !self.can_afford(cost) {
            return false;
        }
        self.reserve(cost);
        true
    }

    fn can_afford(&self, cost: ActionCost) -> bool {
        self.steel >= cost.steel && self.oil >= cost.oil && self.free_supply >= cost.supply
    }

    fn reserve(&mut self, cost: ActionCost) {
        self.steel -= cost.steel;
        self.oil -= cost.oil;
        self.free_supply -= cost.supply;
    }

    fn snapshot(&self) -> ActionBudgetSnapshot {
        ActionBudgetSnapshot {
            steel: self.steel,
            oil: self.oil,
            free_supply: self.free_supply,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ActionReservations {
    actors: BTreeSet<u32>,
    resource_nodes: BTreeSet<u32>,
    producers: BTreeSet<u32>,
}

impl ActionReservations {
    pub(crate) fn reserve_worker(&mut self, worker: u32) -> bool {
        self.actors.insert(worker)
    }

    pub(crate) fn worker_reserved(&self, worker: u32) -> bool {
        self.actors.contains(&worker)
    }

    pub(crate) fn reserve_resource_node(&mut self, node: u32) -> bool {
        self.resource_nodes.insert(node)
    }

    pub(crate) fn resource_node_reserved(&self, node: u32) -> bool {
        self.resource_nodes.contains(&node)
    }

    pub(crate) fn reserve_production_building(&mut self, producer: u32) -> bool {
        self.producers.insert(producer)
    }

    pub(crate) fn production_building_reserved(&self, producer: u32) -> bool {
        self.producers.contains(&producer)
    }

    pub(crate) fn snapshot_counts(&self) -> ReservationCounts {
        ReservationCounts {
            actors: self.actors.len(),
            resource_nodes: self.resource_nodes.len(),
            producers: self.producers.len(),
        }
    }
}

#[derive(Clone, Copy)]
struct ActionCost {
    steel: u32,
    oil: u32,
    supply: u32,
}

/// A typed, bounded, call-ordered action batch for one strategy think.
#[derive(Debug)]
pub struct AiActions {
    budget: ActionBudget,
    reservations: ActionReservations,
    requests: Vec<AiActionRequest>,
    owned_kinds: BTreeMap<u32, EntityKind>,
    resource_ids: BTreeSet<u32>,
}

impl AiActions {
    pub(crate) fn for_frame(frame: &AiFrame) -> Self {
        let economy = frame.economy();
        Self {
            budget: ActionBudget::new(
                economy.steel,
                economy.oil,
                economy.supply_used,
                economy.supply_cap,
            ),
            reservations: ActionReservations::default(),
            requests: Vec::new(),
            owned_kinds: frame
                .owned()
                .iter()
                .map(|entity| (entity.id, entity.kind))
                .collect(),
            resource_ids: frame
                .resources()
                .iter()
                .map(|resource| resource.id)
                .collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_budget(budget: ActionBudget) -> Self {
        Self {
            budget,
            reservations: ActionReservations::default(),
            requests: Vec::new(),
            owned_kinds: BTreeMap::new(),
            resource_ids: BTreeSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.requests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn remaining_budget(&self) -> ActionBudgetSnapshot {
        self.budget.snapshot()
    }

    pub fn reservation_counts(&self) -> ReservationCounts {
        self.reservations.snapshot_counts()
    }

    pub fn paid_build(
        &mut self,
        workers: &[u32],
        building: EntityKind,
        tile_x: u32,
        tile_y: u32,
    ) -> Result<u32, ActionError> {
        if !building.is_building() {
            return Err(ActionBlocker::UnsupportedKind(building).into());
        }
        let worker = self.preflight_candidate(workers, ReservationNamespace::Actor, None)?;
        let cost = cost_for_building(building);
        self.preflight_cost(cost)?;
        self.preflight_capacity()?;
        self.budget.reserve(cost);
        self.reservations.reserve_worker(worker);
        self.requests.push(AiActionRequest::Build {
            units: vec![worker],
            building,
            tile_x,
            tile_y,
            queued: false,
        });
        Ok(worker)
    }

    pub fn resume_build(
        &mut self,
        workers: &[u32],
        building: EntityKind,
        tile_x: u32,
        tile_y: u32,
    ) -> Result<u32, ActionError> {
        if !building.is_building() {
            return Err(ActionBlocker::UnsupportedKind(building).into());
        }
        let worker = self.preflight_candidate(workers, ReservationNamespace::Actor, None)?;
        self.preflight_capacity()?;
        self.reservations.reserve_worker(worker);
        self.requests.push(AiActionRequest::Build {
            units: vec![worker],
            building,
            tile_x,
            tile_y,
            queued: false,
        });
        Ok(worker)
    }

    pub fn train(&mut self, producers: &[u32], unit: EntityKind) -> Result<u32, ActionError> {
        if !unit.is_unit() {
            return Err(ActionBlocker::UnsupportedKind(unit).into());
        }
        let producer = self.preflight_producer(producers, |kind| {
            rts_rules::economy::trainable_units(kind).contains(&unit)
        })?;
        let cost = cost_for_unit(unit);
        self.preflight_cost(cost)?;
        self.preflight_capacity()?;
        self.budget.reserve(cost);
        self.reservations.reserve_production_building(producer);
        self.requests.push(AiActionRequest::Train {
            building: producer,
            unit,
        });
        Ok(producer)
    }

    /// Enable or disable one standing production allocation without spending its eventual cost.
    /// The simulation remains authoritative over producer compatibility, repeat state, resources,
    /// and queue insertion. Multiple kinds may be enabled on the same producer in one step.
    pub fn set_production_repeat(
        &mut self,
        producers: &[u32],
        kind: EntityKind,
        enabled: bool,
    ) -> Result<u32, ActionError> {
        if !kind.is_unit() && !kind.is_resource_extractor() {
            return Err(ActionBlocker::UnsupportedKind(kind).into());
        }
        if producers.is_empty() {
            return Err(ActionBlocker::EmptyCandidates(ReservationNamespace::Producer).into());
        }
        let producer = producers
            .iter()
            .find(|id| {
                self.owned_kinds.get(id).is_some_and(|producer_kind| {
                    rts_rules::economy::trainable_units(*producer_kind).contains(&kind)
                })
            })
            .copied()
            .ok_or(ActionBlocker::NoCompatibleProducer)?;
        self.preflight_capacity()?;
        self.requests.push(AiActionRequest::AdjustProductionRepeat {
            buildings: vec![producer],
            unit: kind,
            delta: if enabled { 1 } else { -1 },
        });
        Ok(producer)
    }

    pub fn research(
        &mut self,
        producers: &[u32],
        research: UpgradeKind,
    ) -> Result<u32, ActionError> {
        let definition = upgrade::definition(research);
        let producer =
            self.preflight_producer(producers, |kind| kind == definition.researched_at)?;
        let cost = ActionCost {
            steel: definition.cost_steel,
            oil: definition.cost_oil,
            supply: 0,
        };
        self.preflight_cost(cost)?;
        self.preflight_capacity()?;
        self.budget.reserve(cost);
        self.reservations.reserve_production_building(producer);
        self.requests.push(AiActionRequest::Research {
            building: producer,
            upgrade: research,
        });
        Ok(producer)
    }

    pub fn gather(
        &mut self,
        workers: &[u32],
        nodes: &[u32],
        queued: bool,
    ) -> Result<(u32, u32), ActionError> {
        let worker = self.preflight_candidate(workers, ReservationNamespace::Actor, None)?;
        let node = self.preflight_candidate(
            nodes,
            ReservationNamespace::ResourceNode,
            Some(&self.resource_ids),
        )?;
        self.preflight_capacity()?;
        self.reservations.reserve_worker(worker);
        self.reservations.reserve_resource_node(node);
        self.requests.push(AiActionRequest::Gather {
            units: vec![worker],
            node,
            queued,
        });
        Ok((worker, node))
    }

    pub fn move_group(
        &mut self,
        group: &UnitGroup,
        x: f32,
        y: f32,
        queued: bool,
    ) -> Result<(), ActionError> {
        self.emit_reserved_group(
            group,
            AiActionRequest::Move {
                units: group.0.clone(),
                x,
                y,
                queued,
            },
        )
    }

    pub fn attack_move(
        &mut self,
        group: &UnitGroup,
        x: f32,
        y: f32,
        queued: bool,
    ) -> Result<(), ActionError> {
        self.emit_reserved_group(
            group,
            AiActionRequest::AttackMove {
                units: group.0.clone(),
                x,
                y,
                queued,
            },
        )
    }

    pub fn attack(
        &mut self,
        group: &UnitGroup,
        target: u32,
        queued: bool,
    ) -> Result<(), ActionError> {
        self.emit_reserved_group(
            group,
            AiActionRequest::Attack {
                units: group.0.clone(),
                target,
                queued,
            },
        )
    }

    pub fn hold_position(&mut self, group: &UnitGroup, queued: bool) -> Result<(), ActionError> {
        self.emit_reserved_group(
            group,
            AiActionRequest::HoldPosition {
                units: group.0.clone(),
                queued,
            },
        )
    }

    pub fn setup_anti_tank_guns(
        &mut self,
        group: &UnitGroup,
        x: f32,
        y: f32,
        queued: bool,
    ) -> Result<(), ActionError> {
        self.emit_reserved_group(
            group,
            AiActionRequest::SetupAntiTankGuns {
                units: group.0.clone(),
                x,
                y,
                queued,
            },
        )
    }

    fn emit_reserved_group(
        &mut self,
        group: &UnitGroup,
        request: AiActionRequest,
    ) -> Result<(), ActionError> {
        if let Some(id) = group
            .0
            .iter()
            .find(|id| self.reservations.worker_reserved(**id))
        {
            return Err(ActionBlocker::AlreadyReserved {
                namespace: ReservationNamespace::Actor,
                id: *id,
            }
            .into());
        }
        self.preflight_capacity()?;
        for id in &group.0 {
            self.reservations.reserve_worker(*id);
        }
        self.requests.push(request);
        Ok(())
    }

    fn preflight_candidate(
        &self,
        candidates: &[u32],
        namespace: ReservationNamespace,
        known: Option<&BTreeSet<u32>>,
    ) -> Result<u32, ActionError> {
        if candidates.is_empty() {
            return Err(ActionBlocker::EmptyCandidates(namespace).into());
        }
        let mut first_known = None;
        for id in candidates {
            if known.is_some_and(|known| !known.contains(id)) {
                continue;
            }
            first_known.get_or_insert(*id);
            let reserved = match namespace {
                ReservationNamespace::Actor => self.reservations.worker_reserved(*id),
                ReservationNamespace::ResourceNode => self.reservations.resource_node_reserved(*id),
                ReservationNamespace::Producer => {
                    self.reservations.production_building_reserved(*id)
                }
            };
            if !reserved {
                return Ok(*id);
            }
        }
        match first_known {
            Some(id) => Err(ActionBlocker::AlreadyReserved { namespace, id }.into()),
            None => Err(ActionBlocker::NoKnownCandidate(namespace).into()),
        }
    }

    fn preflight_producer(
        &self,
        candidates: &[u32],
        compatible: impl Fn(EntityKind) -> bool,
    ) -> Result<u32, ActionError> {
        if candidates.is_empty() {
            return Err(ActionBlocker::EmptyCandidates(ReservationNamespace::Producer).into());
        }
        let mut first_compatible = None;
        for id in candidates {
            let Some(kind) = self.owned_kinds.get(id) else {
                continue;
            };
            if !compatible(*kind) {
                continue;
            }
            first_compatible.get_or_insert(*id);
            if !self.reservations.production_building_reserved(*id) {
                return Ok(*id);
            }
        }
        if let Some(id) = first_compatible {
            Err(ActionBlocker::AlreadyReserved {
                namespace: ReservationNamespace::Producer,
                id,
            }
            .into())
        } else {
            Err(ActionBlocker::NoCompatibleProducer.into())
        }
    }

    fn preflight_cost(&self, cost: ActionCost) -> Result<(), ActionError> {
        if self.budget.can_afford(cost) {
            Ok(())
        } else {
            Err(ActionBlocker::InsufficientBudget {
                steel: cost.steel,
                oil: cost.oil,
                supply: cost.supply,
            }
            .into())
        }
    }

    fn preflight_capacity(&self) -> Result<(), ActionError> {
        if self.requests.len() < MAX_ACTIONS_PER_STEP {
            Ok(())
        } else {
            Err(ActionBlocker::ActionCapacity.into())
        }
    }

    #[cfg(test)]
    pub(crate) fn emit_compat(&mut self, request: AiActionRequest) {
        if self.requests.len() < MAX_ACTIONS_PER_STEP {
            self.requests.push(request);
        }
    }

    pub(crate) fn into_requests(self) -> Vec<AiActionRequest> {
        self.requests
    }
}

fn cost_for_unit(kind: EntityKind) -> ActionCost {
    let (steel, oil) = rts_rules::economy::cost(kind);
    ActionCost {
        steel,
        oil,
        supply: rts_rules::economy::supply_cost(kind),
    }
}

fn cost_for_building(kind: EntityKind) -> ActionCost {
    let (steel, oil) = rts_rules::economy::cost(kind);
    ActionCost {
        steel,
        oil,
        supply: 0,
    }
}

/// SDK-owned action vocabulary translated only by the runtime emitter.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AiActionRequest {
    Move {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    AttackMove {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    Attack {
        units: Vec<u32>,
        target: u32,
        queued: bool,
    },
    Gather {
        units: Vec<u32>,
        node: u32,
        queued: bool,
    },
    Build {
        units: Vec<u32>,
        building: EntityKind,
        tile_x: u32,
        tile_y: u32,
        queued: bool,
    },
    Train {
        building: u32,
        unit: EntityKind,
    },
    AdjustProductionRepeat {
        buildings: Vec<u32>,
        unit: EntityKind,
        delta: i8,
    },
    Research {
        building: u32,
        upgrade: UpgradeKind,
    },
    HoldPosition {
        units: Vec<u32>,
        queued: bool,
    },
    SetupAntiTankGuns {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
}

#[cfg(test)]
mod tests;
