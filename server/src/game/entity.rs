//! Entities and their storage. See `DESIGN.md` §3 (`entity.rs`).
//!
//! An [`Entity`] is the single mutable record for any unit, building, or resource node
//! in the simulation. The simulation systems (`systems.rs`) read and mutate these records
//! every tick; the snapshot layer (`mod.rs`) projects them into `protocol::EntityView`.
//!
//! Storage is an [`EntityStore`]: a `HashMap<u32, Entity>` keyed by a stable, monotonically
//! increasing id. Ids are never reused, so a stale id (an entity that has died) simply
//! misses the map — every lookup is fallible and the tick loop tolerates misses (no panics).

use std::collections::HashMap;

use crate::config;
use crate::protocol::{kinds, states};

/// Neutral owner id used for resource nodes (minerals / gas geysers).
pub const NEUTRAL: u32 = 0;

/// The high-level order a unit/building is currently executing.
///
/// Orders drive the per-tick systems. Buildings only ever sit in [`Order::Idle`]; their
/// activity (production, construction) is tracked by their dedicated fields. Movement
/// targets are stored as world-pixel goals plus a tile waypoint path consumed by the
/// movement system.
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    /// No order: units hold position, idle combat units auto-defend, bunkers auto-fire.
    Idle,
    /// Move to a world point; stop on arrival. No engaging en route.
    Move { x: f32, y: f32 },
    /// Move to a world point while engaging enemies encountered along the way.
    AttackMove { x: f32, y: f32 },
    /// Chase and attack a specific entity until it dies, then go idle.
    Attack { target: u32 },
    /// Harvest from a resource node, ferrying loads back to the home Industrial Center. See [`CarryState`].
    Gather { node: u32 },
    /// Walk to a building site and construct it. `site` is the building entity id (the
    /// building already exists in CONSTRUCT state). Worker is occupied until completion.
    Build { site: u32 },
}

/// A queued production order on a building.
#[derive(Debug, Clone)]
pub struct ProdItem {
    /// Unit kind being produced (e.g. `"worker"`, `"rifleman"`).
    pub unit: String,
    /// Ticks of progress accumulated on this item so far.
    pub progress: u32,
    /// Total ticks required to finish this item.
    pub total: u32,
}

/// What a worker is carrying back to base, if anything.
#[derive(Debug, Clone, Copy)]
pub struct CarryState {
    /// Amount of resource currently held.
    pub amount: u32,
    /// `true` if the load is gas, `false` for minerals.
    pub is_gas: bool,
}

/// The phase a gathering worker is in. Kept separate from [`Order::Gather`] so the order
/// (which node) stays stable while the worker cycles through fetch/harvest/return.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GatherPhase {
    /// Walking out to the resource node.
    ToNode,
    /// Standing on the node, accumulating harvest ticks.
    Harvesting,
    /// Walking back to the home Industrial Center with a load.
    ToHome,
}

/// A single simulation entity: unit, building, or resource node.
///
/// All positional state is in world pixels (`pos_x`/`pos_y` are the entity center).
/// Fields that only apply to some kinds are present on every entity but left at their
/// neutral defaults otherwise — this keeps the store homogeneous and lookup-free.
#[derive(Debug, Clone)]
pub struct Entity {
    /// Stable unique id (never reused).
    pub id: u32,
    /// Owning player id, or [`NEUTRAL`] (0) for resource nodes.
    pub owner: u32,
    /// Entity kind string from [`crate::protocol::kinds`].
    pub kind: String,

    /// Center position in world pixels.
    pub pos_x: f32,
    pub pos_y: f32,

    pub hp: u32,
    pub max_hp: u32,

    /// Facing angle in radians (for unit orientation / render). Updated when moving/attacking.
    pub facing: f32,

    /// Current high-level order / AI state.
    pub order: Order,

    /// Tile-center waypoints remaining to walk through (world pixels), in reverse order so
    /// the next waypoint is the last element (cheap `pop`). Empty when not moving.
    pub path: Vec<(f32, f32)>,

    /// Ticks until this entity may attack again (0 = ready).
    pub attack_cd: u32,

    /// Current attack/interaction target id (enemy for combat, node/industrial_center for gather). Used for
    /// render tracers and to remember focus across ticks. `None` when not engaged.
    pub target_id: Option<u32>,

    // --- Buildings: production -------------------------------------------------
    /// FIFO production queue (front = item being produced). Empty for non-producers.
    pub prod_queue: Vec<ProdItem>,

    // --- Buildings: construction ----------------------------------------------
    /// `true` while a building is still being constructed (CONSTRUCT state).
    pub under_construction: bool,
    /// Ticks of construction accumulated so far.
    pub build_progress: u32,
    /// Total ticks of construction required (`building_stats.build_ticks`).
    pub build_total: u32,

    // --- Workers: carrying -----------------------------------------------------
    /// Present when the worker is laden with a resource load.
    pub carry: Option<CarryState>,
    /// Gathering sub-phase (only meaningful under [`Order::Gather`]).
    pub gather_phase: GatherPhase,
    /// Ticks accumulated while [`GatherPhase::Harvesting`].
    pub harvest_progress: u32,
    /// The Industrial Center this worker deposits into. Resolved lazily to the nearest own Industrial Center.
    pub home_industrial_center: Option<u32>,

    // --- Resource nodes --------------------------------------------------------
    /// Remaining resource amount (resource nodes only).
    pub remaining: u32,
    /// The single worker currently occupying this node's harvest slot (resource nodes only).
    /// At most one worker may be in [`GatherPhase::Harvesting`] on a node at a time; others
    /// queue in [`GatherPhase::ToNode`] until the slot frees. Advisory: validated each tick
    /// against the recorded worker's live state, so it self-heals on death/retarget/deposit.
    pub miner: Option<u32>,
}

impl Entity {
    /// Whether this entity is a unit (mobile, combat-capable).
    pub fn is_unit(&self) -> bool {
        kinds::is_unit(&self.kind)
    }

    /// Whether this entity is a building.
    pub fn is_building(&self) -> bool {
        kinds::is_building(&self.kind)
    }

    /// Whether this entity is a resource node (minerals or gas).
    pub fn is_node(&self) -> bool {
        self.kind == kinds::MINERALS || self.kind == kinds::GAS
    }

    /// Whether this building can be attacked / can take damage and die. Resource nodes are
    /// indestructible (they only deplete).
    pub fn is_targetable(&self) -> bool {
        !self.is_node()
    }

    /// Whether this entity can deal damage (units with dmg, or bunkers).
    pub fn can_attack(&self) -> bool {
        if let Some(s) = config::unit_stats(&self.kind) {
            s.dmg > 0
        } else if let Some(s) = config::building_stats(&self.kind) {
            s.dmg > 0 && !self.under_construction
        } else {
            false
        }
    }

    /// Sight radius in tiles for fog computation.
    pub fn sight_tiles(&self) -> u32 {
        if let Some(s) = config::unit_stats(&self.kind) {
            s.sight_tiles
        } else if let Some(s) = config::building_stats(&self.kind) {
            s.sight_tiles
        } else {
            // Resource nodes contribute no sight.
            0
        }
    }

    /// The collision/interaction radius in world pixels.
    pub fn radius(&self) -> f32 {
        if let Some(s) = config::unit_stats(&self.kind) {
            s.radius
        } else if self.is_building() {
            // Footprint half-extent (approx) for range/interaction checks.
            let s = config::building_stats(&self.kind).expect("building stats");
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
        if self.under_construction {
            return states::CONSTRUCT;
        }
        if self.is_building() {
            if !self.prod_queue.is_empty() {
                return states::TRAIN;
            }
            return states::IDLE;
        }
        match self.order {
            Order::Idle => states::IDLE,
            Order::Move { .. } => states::MOVE,
            Order::AttackMove { .. } => {
                if self.target_id.is_some() {
                    states::ATTACK
                } else {
                    states::MOVE
                }
            }
            Order::Attack { .. } => states::ATTACK,
            Order::Gather { .. } => states::GATHER,
            Order::Build { .. } => states::BUILD,
        }
    }

    /// Clear all movement/combat orders and reset to idle (the `stop` command, deaths, etc.).
    /// Does not touch production queues (those belong to buildings).
    pub fn clear_orders(&mut self) {
        self.order = Order::Idle;
        self.path.clear();
        self.target_id = None;
        // A laden worker keeps its load but stops ferrying.
        self.gather_phase = GatherPhase::ToNode;
        self.harvest_progress = 0;
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
    pub fn spawn_unit(&mut self, owner: u32, kind: &str, x: f32, y: f32) -> Option<u32> {
        let s = config::unit_stats(kind)?;
        let e = Entity {
            id: 0,
            owner,
            kind: kind.to_string(),
            pos_x: x,
            pos_y: y,
            hp: s.hp,
            max_hp: s.hp,
            facing: 0.0,
            order: Order::Idle,
            path: Vec::new(),
            attack_cd: 0,
            target_id: None,
            prod_queue: Vec::new(),
            under_construction: false,
            build_progress: 0,
            build_total: 0,
            carry: None,
            gather_phase: GatherPhase::ToNode,
            harvest_progress: 0,
            home_industrial_center: None,
            remaining: 0,
            miner: None,
        };
        Some(self.insert(e))
    }

    /// Spawn a building of `kind` for `owner`. The position is the building center in world
    /// pixels. If `finished` is true the building starts fully built; otherwise it begins in
    /// CONSTRUCT state with zero progress. Returns `None` if `kind` is not a known building.
    pub fn spawn_building(
        &mut self,
        owner: u32,
        kind: &str,
        x: f32,
        y: f32,
        finished: bool,
    ) -> Option<u32> {
        let s = config::building_stats(kind)?;
        let e = Entity {
            id: 0,
            owner,
            kind: kind.to_string(),
            pos_x: x,
            pos_y: y,
            // Under-construction buildings still occupy their footprint and have full HP so
            // they are not trivially destroyed; CONSTRUCT is purely a production gate here.
            hp: s.hp,
            max_hp: s.hp,
            facing: 0.0,
            order: Order::Idle,
            path: Vec::new(),
            attack_cd: 0,
            target_id: None,
            prod_queue: Vec::new(),
            under_construction: !finished,
            build_progress: 0,
            build_total: s.build_ticks,
            carry: None,
            gather_phase: GatherPhase::ToNode,
            harvest_progress: 0,
            home_industrial_center: None,
            remaining: 0,
            miner: None,
        };
        Some(self.insert(e))
    }

    /// Spawn a neutral resource node of `kind` (`minerals` | `gas`) at a world position.
    pub fn spawn_node(&mut self, kind: &str, x: f32, y: f32) -> Option<u32> {
        let amount = config::node_amount(kind);
        if amount == 0 {
            return None;
        }
        let e = Entity {
            id: 0,
            owner: NEUTRAL,
            kind: kind.to_string(),
            pos_x: x,
            pos_y: y,
            hp: 1,
            max_hp: 1,
            facing: 0.0,
            order: Order::Idle,
            path: Vec::new(),
            attack_cd: 0,
            target_id: None,
            prod_queue: Vec::new(),
            under_construction: false,
            build_progress: 0,
            build_total: 0,
            carry: None,
            gather_phase: GatherPhase::ToNode,
            harvest_progress: 0,
            home_industrial_center: None,
            remaining: amount,
            miner: None,
        };
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

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate over all entities (shared).
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.map.values()
    }

    /// Iterate over all entities (mutable).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entity> {
        self.map.values_mut()
    }

    /// All currently-live entity ids (order unspecified). Useful for index-free iteration
    /// when the body needs `&mut self` on the store.
    pub fn ids(&self) -> Vec<u32> {
        self.map.keys().copied().collect()
    }

    /// Whether `player` owns at least one entity (unit or building).
    pub fn player_alive(&self, player: u32) -> bool {
        self.map.values().any(|e| e.owner == player)
    }
}
