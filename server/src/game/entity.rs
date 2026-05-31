//! Entities and their storage. See `DESIGN.md` §3 (`entity.rs`).
//!
//! An [`Entity`] is the single mutable record for any unit, building, or resource node
//! in the simulation. The simulation services (`services/`) read and mutate these records
//! every tick; the snapshot layer (`mod.rs`) projects them into `protocol::EntityView`.
//!
//! Storage is an [`EntityStore`]: a `HashMap<u32, Entity>` keyed by a stable, monotonically
//! increasing id. Ids are never reused, so a stale id (an entity that has died) simply
//! misses the map — every lookup is fallible and the tick loop tolerates misses (no panics).

use std::collections::HashMap;

use crate::config;
use crate::protocol::states;

/// Neutral owner id used for resource nodes (steel / oil nodes).
pub const NEUTRAL: u32 = 0;

// ---------------------------------------------------------------------------
// Typed entity kinds (internal simulation only; protocol strings live in
// `protocol::kinds` and conversion happens only at the wire boundary).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityKind {
    Worker,
    Rifleman,
    MachineGunner,
    AtTeam,
    Tank,
    IndustrialCenter,
    Depot,
    Barracks,
    TrainingCentre,
    TankFactory,
    Steel,
    Oil,
}

impl EntityKind {
    pub fn is_unit(self) -> bool {
        matches!(
            self,
            EntityKind::Worker
                | EntityKind::Rifleman
                | EntityKind::MachineGunner
                | EntityKind::AtTeam
                | EntityKind::Tank
        )
    }

    pub fn is_building(self) -> bool {
        matches!(
            self,
            EntityKind::IndustrialCenter
                | EntityKind::Depot
                | EntityKind::Barracks
                | EntityKind::TrainingCentre
                | EntityKind::TankFactory
        )
    }

    pub fn is_node(self) -> bool {
        matches!(self, EntityKind::Steel | EntityKind::Oil)
    }

    pub fn to_protocol_str(self) -> &'static str {
        use crate::protocol::kinds;
        match self {
            EntityKind::Worker => kinds::WORKER,
            EntityKind::Rifleman => kinds::RIFLEMAN,
            EntityKind::MachineGunner => kinds::MACHINE_GUNNER,
            EntityKind::AtTeam => kinds::AT_TEAM,
            EntityKind::Tank => kinds::TANK,
            EntityKind::IndustrialCenter => kinds::INDUSTRIAL_CENTER,
            EntityKind::Depot => kinds::DEPOT,
            EntityKind::Barracks => kinds::BARRACKS,
            EntityKind::TrainingCentre => kinds::TRAINING_CENTRE,
            EntityKind::TankFactory => kinds::TANK_FACTORY,
            EntityKind::Steel => kinds::STEEL,
            EntityKind::Oil => kinds::OIL,
        }
    }
}

impl std::str::FromStr for EntityKind {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use crate::protocol::kinds;
        match s {
            kinds::WORKER => Ok(EntityKind::Worker),
            kinds::RIFLEMAN => Ok(EntityKind::Rifleman),
            kinds::MACHINE_GUNNER => Ok(EntityKind::MachineGunner),
            kinds::AT_TEAM => Ok(EntityKind::AtTeam),
            kinds::TANK => Ok(EntityKind::Tank),
            kinds::INDUSTRIAL_CENTER => Ok(EntityKind::IndustrialCenter),
            kinds::DEPOT => Ok(EntityKind::Depot),
            kinds::BARRACKS => Ok(EntityKind::Barracks),
            kinds::TRAINING_CENTRE => Ok(EntityKind::TrainingCentre),
            kinds::TANK_FACTORY => Ok(EntityKind::TankFactory),
            kinds::STEEL => Ok(EntityKind::Steel),
            kinds::OIL => Ok(EntityKind::Oil),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_protocol_str())
    }
}

// ---------------------------------------------------------------------------
// Orders, production, carrying
// ---------------------------------------------------------------------------

/// The high-level order a unit/building is currently executing.
///
/// Orders drive the per-tick systems. Buildings only ever sit in [`Order::Idle`]; their
/// activity (production, construction) is tracked by their dedicated fields. Each active order
/// keeps immutable intent separate from execution phase, so systems transition explicit state
/// machines instead of smuggling progress through unrelated fields.
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    /// No order: units hold position and idle combat units auto-defend.
    Idle,
    /// Move to a world point; stop on arrival. No engaging en route.
    Move(MoveOrder),
    /// Move to a world point while engaging enemies encountered along the way.
    AttackMove(MoveOrder),
    /// Chase and attack a specific entity until it dies, then go idle.
    Attack(AttackOrder),
    /// Harvest from a resource node, ferrying loads back to the home Industrial Center. See [`CarryState`].
    Gather(GatherOrder),
    /// Walk to a target tile and construct a building of `kind` there. The building does
    /// not exist until the worker arrives, re-validates placement/affordability, and pays
    /// the cost; until then the order carries only the intent (kind + top-left tile).
    Build(BuildOrder),
}

impl Order {
    pub fn move_to(x: f32, y: f32) -> Self {
        Order::Move(MoveOrder::new(x, y))
    }

    pub fn attack_move_to(x: f32, y: f32) -> Self {
        Order::AttackMove(MoveOrder::new(x, y))
    }

    pub fn attack(target: u32) -> Self {
        Order::Attack(AttackOrder::new(target))
    }

    pub fn gather(node: u32) -> Self {
        Order::Gather(GatherOrder::new(node))
    }

    pub fn build(kind: EntityKind, tile_x: u32, tile_y: u32) -> Self {
        Order::Build(BuildOrder::new(kind, tile_x, tile_y))
    }

    pub fn attack_target(&self) -> Option<u32> {
        match self {
            Order::Attack(order) => Some(order.intent.target),
            _ => None,
        }
    }

    pub fn gather_node(&self) -> Option<u32> {
        match self {
            Order::Gather(order) => Some(order.intent.node),
            _ => None,
        }
    }

    /// The id of the building being constructed, if construction has actually begun.
    /// Returns `None` while the worker is still walking to the site.
    pub fn build_site(&self) -> Option<u32> {
        match self {
            Order::Build(order) => match order.execution.phase {
                BuildPhase::Constructing { site } => Some(site),
                BuildPhase::ToSite => None,
            },
            _ => None,
        }
    }

    /// The pending placement intent for a build order, if any: (kind, tile_x, tile_y) of
    /// the footprint's top-left tile. Available in any build phase.
    pub fn build_intent_tile(&self) -> Option<(EntityKind, u32, u32)> {
        match self {
            Order::Build(order) => {
                Some((order.intent.kind, order.intent.tile_x, order.intent.tile_y))
            }
            _ => None,
        }
    }

    pub fn gather_phase(&self) -> Option<GatherPhase> {
        match self {
            Order::Gather(order) => Some(order.execution.phase),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointIntent {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveOrder {
    pub intent: PointIntent,
    pub execution: MoveExecution,
}

impl MoveOrder {
    fn new(x: f32, y: f32) -> Self {
        MoveOrder {
            intent: PointIntent { x, y },
            execution: MoveExecution {
                phase: MovePhase::AwaitingPath,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveExecution {
    pub phase: MovePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovePhase {
    AwaitingPath,
    Moving,
    Arrived,
    PathFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetIntent {
    pub target: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttackOrder {
    pub intent: TargetIntent,
    pub execution: AttackExecution,
}

impl AttackOrder {
    fn new(target: u32) -> Self {
        AttackOrder {
            intent: TargetIntent { target },
            execution: AttackExecution {
                phase: AttackPhase::Chasing,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackExecution {
    pub phase: AttackPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackPhase {
    Chasing,
    Firing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatherIntent {
    pub node: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatherOrder {
    pub intent: GatherIntent,
    pub execution: GatherExecution,
}

impl GatherOrder {
    fn new(node: u32) -> Self {
        GatherOrder {
            intent: GatherIntent { node },
            execution: GatherExecution {
                phase: GatherPhase::ToNode,
                harvest_progress: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatherExecution {
    pub phase: GatherPhase,
    pub harvest_progress: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildIntent {
    pub kind: EntityKind,
    pub tile_x: u32,
    pub tile_y: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildOrder {
    pub intent: BuildIntent,
    pub execution: BuildExecution,
}

impl BuildOrder {
    fn new(kind: EntityKind, tile_x: u32, tile_y: u32) -> Self {
        BuildOrder {
            intent: BuildIntent {
                kind,
                tile_x,
                tile_y,
            },
            execution: BuildExecution {
                phase: BuildPhase::ToSite,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildExecution {
    pub phase: BuildPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildPhase {
    /// Worker is walking toward the target tile. No building has been spawned and no
    /// resources have been deducted yet.
    ToSite,
    /// Worker has arrived, the building has been spawned in CONSTRUCT state, and
    /// construction is progressing. `site` is the building entity id.
    Constructing { site: u32 },
}

/// A queued production order on a building.
#[derive(Debug, Clone)]
pub struct ProdItem {
    /// Unit kind being produced.
    pub unit: EntityKind,
    /// Ticks of progress accumulated on this item so far.
    pub progress: u32,
    /// Total ticks required to finish this item.
    pub total: u32,
}

/// What a worker is carrying back to base, if anything.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct CarryState {
    /// Amount of resource currently held.
    pub amount: u32,
    /// Resource kind being carried.
    pub kind: EntityKind,
}

/// The phase a gathering worker is in. Kept inside [`GatherOrder`] so the order's intent
/// (which node) stays stable while the worker's execution cycles through phases.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatherPhase {
    /// Walking out to the resource node.
    ToNode,
    /// Standing on the node, accumulating harvest ticks.
    Harvesting,
    /// Walking back to the home Industrial Center with a load.
    ToHome,
}

// ---------------------------------------------------------------------------
// Component-shaped state groups
// ---------------------------------------------------------------------------

/// Mobile unit state. Only units have this group.
#[derive(Debug, Clone)]
pub struct MovementState {
    /// Facing angle in radians (for unit orientation / render). Updated when moving/attacking.
    pub facing: f32,
    /// Current high-level order / AI state.
    pub order: Order,
    /// Tile-center waypoints remaining to walk through (world pixels), in reverse order so
    /// the next waypoint is the last element (cheap `pop`). Empty when not moving.
    pub path: Vec<(f32, f32)>,
    /// Tick when this unit was last assigned a path. Used for repath throttling.
    pub last_repath_tick: u32,
    /// The goal world point of the most recently assigned path, for throttle-bypass checks.
    pub path_goal: Option<(f32, f32)>,
    /// Consecutive ticks in which the unit moved less than `STUCK_EPS_PX`. Reset on progress
    /// or when a new order is assigned. Used by tolerant arrival.
    pub stuck_ticks: u16,
    /// Position snapshot taken when `stuck_ticks` was last reset to 0. Used to measure
    /// progress each tick for tolerant arrival.
    pub last_progress_pos: (f32, f32),
    /// Ticks remaining before this unit may sidestep again. Decremented each tick; reset to 0
    /// on new order.
    pub sidestep_cooldown: u16,
}

impl Default for MovementState {
    fn default() -> Self {
        MovementState {
            facing: 0.0,
            order: Order::Idle,
            path: Vec::new(),
            last_repath_tick: 0,
            path_goal: None,
            stuck_ticks: 0,
            last_progress_pos: (0.0, 0.0),
            sidestep_cooldown: 0,
        }
    }
}

/// Weapon and active target state. Present on combat-capable entities.
#[derive(Debug, Clone, Default)]
pub struct CombatState {
    /// Ticks until this entity may attack again (0 = ready).
    pub attack_cd: u32,
    /// Current attack/interaction target id. Combat uses enemy ids; gather/build commands use
    /// this for client feedback while the order executes.
    pub target_id: Option<u32>,
}

/// Production queue state. Present only on buildings that can train units.
#[derive(Debug, Clone, Default)]
pub struct ProductionState {
    /// FIFO production queue (front = item being produced).
    pub queue: Vec<ProdItem>,
}

/// Construction progress state. Present only while a building is under construction.
#[derive(Debug, Clone)]
pub struct ConstructionState {
    /// Ticks of construction accumulated so far.
    pub progress: u32,
    /// Total ticks of construction required (`building_stats.build_ticks`).
    pub total: u32,
}

/// Worker-only gathering and carrying state.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct WorkerState {
    /// Present when the worker is laden with a resource load.
    pub carry: Option<CarryState>,
    /// The Industrial Center this worker deposits into. Resolved lazily to the nearest own Industrial Center.
    pub home_industrial_center: Option<u32>,
}

/// Resource-node state. Present only on steel/oil nodes.
#[derive(Debug, Clone)]
pub struct ResourceNodeState {
    /// Remaining resource amount.
    pub remaining: u32,
    /// The single worker currently occupying this node's harvest slot.
    ///
    /// At most one worker may be in [`GatherPhase::Harvesting`] on a node at a time; others
    /// queue in [`GatherPhase::ToNode`] until the slot frees. Advisory: validated each tick
    /// against the recorded worker's live state, so it self-heals on death/retarget/deposit.
    pub miner: Option<u32>,
}

/// Compact classification of which optional state groups an entity kind owns.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityStateGroups {
    pub movement: bool,
    pub combat: bool,
    pub production: bool,
    pub construction: bool,
    pub worker: bool,
    pub resource_node: bool,
}

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// A single simulation entity: unit, building, or resource node.
///
/// All positional state is in world pixels (`pos_x`/`pos_y` are the entity center).
/// State that only applies to a subset of kinds lives in typed optional groups, keeping
/// the store homogeneous while making kind-specific state explicit.
#[derive(Debug, Clone)]
pub struct Entity {
    /// Stable unique id (never reused).
    pub id: u32,
    /// Owning player id, or [`NEUTRAL`] (0) for resource nodes.
    pub owner: u32,
    /// Entity kind.
    pub kind: EntityKind,

    /// Center position in world pixels.
    pub pos_x: f32,
    pub pos_y: f32,

    pub hp: u32,
    pub max_hp: u32,

    pub movement: Option<MovementState>,
    pub combat: Option<CombatState>,
    pub production: Option<ProductionState>,
    pub construction: Option<ConstructionState>,
    pub worker: Option<WorkerState>,
    pub resource_node: Option<ResourceNodeState>,
}

impl Entity {
    pub fn new_unit(owner: u32, kind: EntityKind, x: f32, y: f32) -> Option<Self> {
        let s = config::unit_stats(kind)?;
        Some(Entity {
            id: 0,
            owner,
            kind,
            pos_x: x,
            pos_y: y,
            hp: s.hp,
            max_hp: s.hp,
            movement: Some(MovementState::default()),
            combat: if s.dmg > 0 {
                Some(CombatState::default())
            } else {
                None
            },
            production: None,
            construction: None,
            worker: (kind == EntityKind::Worker).then(WorkerState::default),
            resource_node: None,
        })
    }

    pub fn new_building(
        owner: u32,
        kind: EntityKind,
        x: f32,
        y: f32,
        finished: bool,
    ) -> Option<Self> {
        let s = config::building_stats(kind)?;
        Some(Entity {
            id: 0,
            owner,
            kind,
            pos_x: x,
            pos_y: y,
            hp: s.hp,
            max_hp: s.hp,
            movement: None,
            combat: if s.dmg > 0 {
                Some(CombatState::default())
            } else {
                None
            },
            production: if config::trainable_units(kind).is_empty() {
                None
            } else {
                Some(ProductionState::default())
            },
            construction: (!finished).then_some(ConstructionState {
                progress: 0,
                total: s.build_ticks,
            }),
            worker: None,
            resource_node: None,
        })
    }

    pub fn new_node(kind: EntityKind, x: f32, y: f32) -> Option<Self> {
        let amount = config::node_amount(kind);
        if amount == 0 {
            return None;
        }
        Some(Entity {
            id: 0,
            owner: NEUTRAL,
            kind,
            pos_x: x,
            pos_y: y,
            hp: 1,
            max_hp: 1,
            movement: None,
            combat: None,
            production: None,
            construction: None,
            worker: None,
            resource_node: Some(ResourceNodeState {
                remaining: amount,
                miner: None,
            }),
        })
    }

    #[cfg(test)]
    pub fn state_groups(&self) -> EntityStateGroups {
        EntityStateGroups {
            movement: self.movement.is_some(),
            combat: self.combat.is_some(),
            production: self.production.is_some(),
            construction: self.construction.is_some(),
            worker: self.worker.is_some(),
            resource_node: self.resource_node.is_some(),
        }
    }

    pub fn order(&self) -> Order {
        self.movement
            .as_ref()
            .map(|m| m.order.clone())
            .unwrap_or(Order::Idle)
    }

    pub fn set_order(&mut self, order: Order) {
        if let Some(m) = self.movement.as_mut() {
            m.order = order;
        }
    }

    pub fn mark_move_phase(&mut self, phase: MovePhase) {
        if let Some(m) = self.movement.as_mut() {
            match &mut m.order {
                Order::Move(order) | Order::AttackMove(order) => {
                    order.execution.phase = phase;
                }
                _ => {}
            }
        }
    }

    pub fn move_phase(&self) -> Option<MovePhase> {
        self.movement.as_ref().and_then(|m| match &m.order {
            Order::Move(order) | Order::AttackMove(order) => Some(order.execution.phase),
            _ => None,
        })
    }

    /// Reset the stuck counter and reference position when a new order begins or progress
    /// is made. Call this whenever a fresh move order is assigned.
    pub fn reset_stuck(&mut self, pos_x: f32, pos_y: f32) {
        if let Some(m) = self.movement.as_mut() {
            m.stuck_ticks = 0;
            m.last_progress_pos = (pos_x, pos_y);
            m.sidestep_cooldown = 0;
        }
    }

    pub fn set_last_repath_tick(&mut self, tick: u32) {
        if let Some(m) = self.movement.as_mut() {
            m.last_repath_tick = tick;
        }
    }

    pub fn last_repath_tick(&self) -> u32 {
        self.movement
            .as_ref()
            .map(|m| m.last_repath_tick)
            .unwrap_or(0)
    }

    pub fn set_path_goal(&mut self, goal: Option<(f32, f32)>) {
        if let Some(m) = self.movement.as_mut() {
            m.path_goal = goal;
        }
    }

    pub fn path_goal(&self) -> Option<(f32, f32)> {
        self.movement.as_ref().and_then(|m| m.path_goal)
    }

    pub fn mark_attack_phase(&mut self, phase: AttackPhase) {
        if let Some(m) = self.movement.as_mut() {
            if let Order::Attack(order) = &mut m.order {
                order.execution.phase = phase;
            }
        }
    }

    pub fn mark_gather_phase(&mut self, phase: GatherPhase) {
        if let Some(m) = self.movement.as_mut() {
            if let Order::Gather(order) = &mut m.order {
                order.execution.phase = phase;
                if phase != GatherPhase::Harvesting {
                    order.execution.harvest_progress = 0;
                }
            }
        }
    }

    pub fn tick_gather_harvest(&mut self) -> Option<u32> {
        let m = self.movement.as_mut()?;
        let Order::Gather(order) = &mut m.order else {
            return None;
        };
        if order.execution.phase != GatherPhase::Harvesting {
            return None;
        }
        order.execution.harvest_progress = order.execution.harvest_progress.saturating_add(1);
        Some(order.execution.harvest_progress)
    }

    pub fn reset_gather_harvest(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            if let Order::Gather(order) = &mut m.order {
                order.execution.harvest_progress = 0;
            }
        }
    }

    pub fn build_phase(&self) -> Option<BuildPhase> {
        self.movement.as_ref().and_then(|m| match &m.order {
            Order::Build(order) => Some(order.execution.phase),
            _ => None,
        })
    }

    pub fn mark_build_phase(&mut self, phase: BuildPhase) {
        if let Some(m) = self.movement.as_mut() {
            if let Order::Build(order) = &mut m.order {
                order.execution.phase = phase;
            }
        }
    }

    pub fn path_is_empty(&self) -> bool {
        self.movement
            .as_ref()
            .map(|m| m.path.is_empty())
            .unwrap_or(true)
    }

    pub fn set_path(&mut self, path: Vec<(f32, f32)>) {
        if let Some(m) = self.movement.as_mut() {
            m.path = path;
        }
    }

    pub fn clear_path(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.path.clear();
        }
    }

    pub fn next_waypoint(&self) -> Option<(f32, f32)> {
        self.movement.as_ref().and_then(|m| m.path.last().copied())
    }

    pub fn pop_waypoint(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.path.pop();
        }
    }

    /// Push a waypoint to the front of the visit queue (path is stored reversed, so this
    /// makes `wp` the *next* waypoint consumed by the movement system).
    pub fn push_waypoint(&mut self, wp: (f32, f32)) {
        if let Some(m) = self.movement.as_mut() {
            m.path.push(wp);
        }
    }

    pub fn facing(&self) -> f32 {
        self.movement.as_ref().map(|m| m.facing).unwrap_or(0.0)
    }

    pub fn set_facing(&mut self, facing: f32) {
        if let Some(m) = self.movement.as_mut() {
            m.facing = facing;
        }
    }

    pub fn target_id(&self) -> Option<u32> {
        self.combat.as_ref().and_then(|c| c.target_id)
    }

    pub fn set_target_id(&mut self, target_id: Option<u32>) {
        if let Some(c) = self.combat.as_mut() {
            c.target_id = target_id;
        }
    }

    pub fn attack_cd(&self) -> u32 {
        self.combat.as_ref().map(|c| c.attack_cd).unwrap_or(0)
    }

    pub fn set_attack_cd(&mut self, attack_cd: u32) {
        if let Some(c) = self.combat.as_mut() {
            c.attack_cd = attack_cd;
        }
    }

    pub fn tick_attack_cd(&mut self) {
        if let Some(c) = self.combat.as_mut() {
            c.attack_cd = c.attack_cd.saturating_sub(1);
        }
    }

    pub fn prod_queue(&self) -> &[ProdItem] {
        self.production
            .as_ref()
            .map(|p| p.queue.as_slice())
            .unwrap_or(&[])
    }

    pub fn prod_queue_mut(&mut self) -> Option<&mut Vec<ProdItem>> {
        self.production.as_mut().map(|p| &mut p.queue)
    }

    pub fn under_construction(&self) -> bool {
        self.construction.is_some()
    }

    pub fn build_progress_fraction(&self) -> Option<f32> {
        let c = self.construction.as_ref()?;
        Some(if c.total == 0 {
            1.0
        } else {
            (c.progress as f32 / c.total as f32).min(1.0)
        })
    }

    pub fn remaining(&self) -> Option<u32> {
        self.resource_node.as_ref().map(|n| n.remaining)
    }

    pub fn miner(&self) -> Option<u32> {
        self.resource_node.as_ref().and_then(|n| n.miner)
    }

    pub fn gather_phase(&self) -> Option<GatherPhase> {
        self.movement.as_ref().and_then(|m| m.order.gather_phase())
    }

    pub fn reset_gather_state(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            if let Order::Gather(order) = &mut m.order {
                order.execution.phase = GatherPhase::ToNode;
                order.execution.harvest_progress = 0;
            }
        }
    }

    /// Whether this entity is a unit (mobile, combat-capable).
    pub fn is_unit(&self) -> bool {
        self.kind.is_unit()
    }

    /// Whether this entity is a building.
    pub fn is_building(&self) -> bool {
        self.kind.is_building()
    }

    /// Whether this entity is a resource node (steel or oil).
    pub fn is_node(&self) -> bool {
        self.kind.is_node()
    }

    /// Whether this building can be attacked / can take damage and die. Resource nodes are
    /// indestructible (they only deplete).
    pub fn is_targetable(&self) -> bool {
        !self.is_node()
    }

    /// Whether this entity can deal damage.
    pub fn can_attack(&self) -> bool {
        if let Some(s) = config::unit_stats(self.kind) {
            s.dmg > 0
        } else if let Some(s) = config::building_stats(self.kind) {
            s.dmg > 0 && !self.under_construction()
        } else {
            false
        }
    }

    /// Sight radius in tiles for fog computation.
    pub fn sight_tiles(&self) -> u32 {
        if let Some(s) = config::unit_stats(self.kind) {
            s.sight_tiles
        } else if let Some(s) = config::building_stats(self.kind) {
            s.sight_tiles
        } else {
            // Resource nodes contribute no sight.
            0
        }
    }

    /// The collision/interaction radius in world pixels.
    pub fn radius(&self) -> f32 {
        if let Some(s) = config::unit_stats(self.kind) {
            s.radius
        } else if self.is_building() {
            // Footprint half-extent (approx) for range/interaction checks.
            let Some(s) = config::building_stats(self.kind) else {
                return config::TILE_SIZE as f32 * 0.5;
            };
            (s.foot_w.max(s.foot_h) as f32) * config::TILE_SIZE as f32 * 0.5
        } else {
            // Resource node footprint ~1 tile.
            config::TILE_SIZE as f32 * 0.5
        }
    }

    /// The protocol `state` string reflecting this entity's current activity.
    pub fn state_str(&self) -> &'static str {
        if self.hp == 0 {
            return states::DEAD;
        }
        if self.under_construction() {
            return states::CONSTRUCT;
        }
        if self.is_building() {
            if !self.prod_queue().is_empty() {
                return states::TRAIN;
            }
            return states::IDLE;
        }
        match self.order() {
            Order::Idle => states::IDLE,
            Order::Move(_) => states::MOVE,
            Order::AttackMove(_) => {
                if self.target_id().is_some() {
                    states::ATTACK
                } else {
                    states::MOVE
                }
            }
            Order::Attack(_) => states::ATTACK,
            Order::Gather(_) => states::GATHER,
            Order::Build(_) => states::BUILD,
        }
    }

    /// Clear all movement/combat orders and reset to idle (the `stop` command, deaths, etc.).
    /// Does not touch production queues (those belong to buildings).
    pub fn clear_orders(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.order = Order::Idle;
            m.path.clear();
        }
        self.set_target_id(None);
    }
}

/// The authoritative collection of all entities, keyed by stable id.
///
/// Ids increase monotonically and are never reused. All access is fallible so the tick loop
/// can freely reference ids that may have been removed (dead units, depleted state) without
/// risking a panic.
#[derive(Debug, Default)]
pub struct EntityStore {
    next_id: u32,
    map: HashMap<u32, Entity>,
}

impl EntityStore {
    pub fn new() -> Self {
        EntityStore {
            // Start ids at 1 so 0 can never collide with the neutral-owner sentinel in
            // any accidental id/owner confusion, and so `0` reads as "no entity".
            next_id: 1,
            map: HashMap::new(),
        }
    }

    /// Allocate the next stable id.
    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    /// Insert a fully-formed entity, assigning it a fresh id. Returns the new id.
    pub fn insert(&mut self, mut e: Entity) -> u32 {
        let id = self.alloc_id();
        e.id = id;
        self.map.insert(id, e);
        id
    }

    /// Spawn a unit of `kind` for `owner` at a world position, fully built and idle.
    /// Returns `None` if `kind` is not a known unit.
    pub fn spawn_unit(&mut self, owner: u32, kind: EntityKind, x: f32, y: f32) -> Option<u32> {
        let e = Entity::new_unit(owner, kind, x, y)?;
        Some(self.insert(e))
    }

    /// Spawn a building of `kind` for `owner`. The position is the building center in world
    /// pixels. If `finished` is true the building starts fully built; otherwise it begins in
    /// CONSTRUCT state with zero progress. Returns `None` if `kind` is not a known building.
    pub fn spawn_building(
        &mut self,
        owner: u32,
        kind: EntityKind,
        x: f32,
        y: f32,
        finished: bool,
    ) -> Option<u32> {
        let e = Entity::new_building(owner, kind, x, y, finished)?;
        Some(self.insert(e))
    }

    /// Spawn a neutral resource node of `kind` (`steel` | `oil`) at a world position.
    pub fn spawn_node(&mut self, kind: EntityKind, x: f32, y: f32) -> Option<u32> {
        let e = Entity::new_node(kind, x, y)?;
        Some(self.insert(e))
    }

    pub fn get(&self, id: u32) -> Option<&Entity> {
        self.map.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Entity> {
        self.map.get_mut(&id)
    }

    /// Whether an entity with this id still exists.
    pub fn contains(&self, id: u32) -> bool {
        self.map.contains_key(&id)
    }

    /// Remove an entity, returning it if present.
    pub fn remove(&mut self, id: u32) -> Option<Entity> {
        self.map.remove(&id)
    }

    /// Iterate over all entities (shared).
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        let mut ids: Vec<u32> = self.map.keys().copied().collect();
        ids.sort_unstable();
        ids.into_iter().filter_map(|id| self.map.get(&id))
    }

    /// Iterate over all entities (mutable).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entity> {
        self.map.values_mut()
    }

    /// All currently-live entity ids in stable ascending order. Useful for index-free iteration
    /// when the body needs `&mut self` on the store.
    pub fn ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.map.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Whether `player` owns at least one entity (unit or building).
    pub fn player_alive(&self, player: u32) -> bool {
        self.map.values().any(|e| e.owner == player)
    }

    /// If `worker_id` currently has a `Gather` order, clear that node's `miner` reservation
    /// if it points to this worker.
    pub fn release_miner(&mut self, worker_id: u32) {
        let old_node = match self.get(worker_id) {
            Some(e) => match e.order().gather_node() {
                Some(node) => node,
                None => return,
            },
            None => return,
        };
        if let Some(n) = self.get_mut(old_node) {
            if let Some(node) = n.resource_node.as_mut() {
                if node.miner == Some(worker_id) {
                    node.miner = None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups(
        movement: bool,
        combat: bool,
        production: bool,
        construction: bool,
        worker: bool,
        resource_node: bool,
    ) -> EntityStateGroups {
        EntityStateGroups {
            movement,
            combat,
            production,
            construction,
            worker,
            resource_node,
        }
    }

    #[test]
    fn unit_kinds_have_exact_state_groups() {
        let cases = [
            (
                EntityKind::Worker,
                groups(true, true, false, false, true, false),
            ),
            (
                EntityKind::Rifleman,
                groups(true, true, false, false, false, false),
            ),
            (
                EntityKind::MachineGunner,
                groups(true, true, false, false, false, false),
            ),
            (
                EntityKind::AtTeam,
                groups(true, true, false, false, false, false),
            ),
            (
                EntityKind::Tank,
                groups(true, true, false, false, false, false),
            ),
        ];

        for (kind, expected) in cases {
            let entity = Entity::new_unit(1, kind, 10.0, 20.0).expect("unit kind should spawn");
            assert_eq!(entity.state_groups(), expected, "{kind:?}");
        }
    }

    #[test]
    fn finished_building_kinds_have_exact_state_groups() {
        let cases = [
            (
                EntityKind::IndustrialCenter,
                groups(false, false, true, false, false, false),
            ),
            (
                EntityKind::Depot,
                groups(false, false, false, false, false, false),
            ),
            (
                EntityKind::Barracks,
                groups(false, false, true, false, false, false),
            ),
            (
                EntityKind::TrainingCentre,
                groups(false, false, false, false, false, false),
            ),
            (
                EntityKind::TankFactory,
                groups(false, false, true, false, false, false),
            ),
        ];

        for (kind, expected) in cases {
            let entity = Entity::new_building(1, kind, 10.0, 20.0, true)
                .expect("building kind should spawn");
            assert_eq!(entity.state_groups(), expected, "{kind:?}");
        }
    }

    #[test]
    fn unfinished_buildings_add_construction_state_only() {
        let kinds = [
            EntityKind::IndustrialCenter,
            EntityKind::Depot,
            EntityKind::Barracks,
            EntityKind::TrainingCentre,
            EntityKind::TankFactory,
        ];

        for kind in kinds {
            let finished = Entity::new_building(1, kind, 10.0, 20.0, true)
                .expect("building kind should spawn");
            let unfinished = Entity::new_building(1, kind, 10.0, 20.0, false)
                .expect("building kind should spawn");
            let mut expected = finished.state_groups();
            expected.construction = true;
            assert_eq!(unfinished.state_groups(), expected, "{kind:?}");
        }
    }

    #[test]
    fn resource_node_kinds_have_exact_state_groups() {
        for kind in [EntityKind::Steel, EntityKind::Oil] {
            let entity = Entity::new_node(kind, 10.0, 20.0).expect("node kind should spawn");
            assert_eq!(
                entity.state_groups(),
                groups(false, false, false, false, false, true),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn order_state_machines_keep_intent_separate_from_execution() {
        let mut worker =
            Entity::new_unit(1, EntityKind::Worker, 10.0, 20.0).expect("worker should spawn");

        worker.set_order(Order::gather(42));
        assert_eq!(worker.order().gather_node(), Some(42));
        assert_eq!(worker.gather_phase(), Some(GatherPhase::ToNode));

        worker.mark_gather_phase(GatherPhase::Harvesting);
        assert_eq!(worker.order().gather_node(), Some(42));
        assert_eq!(worker.gather_phase(), Some(GatherPhase::Harvesting));
        assert_eq!(worker.tick_gather_harvest(), Some(1));
        assert_eq!(worker.tick_gather_harvest(), Some(2));

        worker.mark_gather_phase(GatherPhase::ToNode);
        assert_eq!(worker.order().gather_node(), Some(42));
        assert_eq!(worker.gather_phase(), Some(GatherPhase::ToNode));
        assert_eq!(worker.tick_gather_harvest(), None);

        worker.clear_orders();
        assert_eq!(worker.order(), Order::Idle);
        assert_eq!(worker.gather_phase(), None);
    }

    #[test]
    fn attack_and_build_orders_have_explicit_execution_phases() {
        let mut unit =
            Entity::new_unit(1, EntityKind::Rifleman, 10.0, 20.0).expect("unit should spawn");

        unit.set_order(Order::attack(99));
        assert_eq!(unit.order().attack_target(), Some(99));
        assert!(matches!(
            unit.order(),
            Order::Attack(AttackOrder {
                execution: AttackExecution {
                    phase: AttackPhase::Chasing
                },
                ..
            })
        ));
        unit.mark_attack_phase(AttackPhase::Firing);
        assert_eq!(unit.order().attack_target(), Some(99));
        assert!(matches!(
            unit.order(),
            Order::Attack(AttackOrder {
                execution: AttackExecution {
                    phase: AttackPhase::Firing
                },
                ..
            })
        ));

        let mut worker =
            Entity::new_unit(1, EntityKind::Worker, 10.0, 20.0).expect("worker should spawn");
        worker.set_order(Order::build(EntityKind::Depot, 4, 5));
        assert_eq!(worker.order().build_site(), None);
        assert_eq!(
            worker.order().build_intent_tile(),
            Some((EntityKind::Depot, 4, 5))
        );
        assert!(matches!(
            worker.order(),
            Order::Build(BuildOrder {
                execution: BuildExecution {
                    phase: BuildPhase::ToSite
                },
                ..
            })
        ));
        worker.mark_build_phase(BuildPhase::Constructing { site: 7 });
        assert_eq!(worker.order().build_site(), Some(7));
        assert!(matches!(
            worker.order(),
            Order::Build(BuildOrder {
                execution: BuildExecution {
                    phase: BuildPhase::Constructing { site: 7 }
                },
                ..
            })
        ));
    }
}
