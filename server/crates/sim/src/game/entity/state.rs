use crate::game::upgrade::UpgradeKind;

use super::{EntityKind, Order, OrderIntent, RallyIntent};

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

/// A queued research order on a tech building.
#[derive(Debug, Clone)]
pub struct ResearchItem {
    pub upgrade: UpgradeKind,
    pub progress: u32,
    pub total: u32,
}

/// Reserved for future round-trip harvesting if attached mining is replaced.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct CarryState {
    /// Amount of resource currently held.
    pub amount: u32,
    /// Resource kind being carried.
    pub kind: EntityKind,
}

/// Mobile unit state. Only units have this group.
#[derive(Debug, Clone)]
pub struct MovementState {
    /// Facing angle in radians (for unit orientation / render). Updated when moving/attacking.
    pub facing: f32,
    /// Current high-level order / AI state.
    pub order: Order,
    /// Future order intents appended by queued commands. These are inert until promoted.
    pub queued_orders: Vec<OrderIntent>,
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
    /// Position delta from this tick's path-following movement phase. This is transient:
    /// reset before movement, set after waypoint advancement, and intentionally excludes
    /// later collision shoves.
    pub last_move_delta: (f32, f32),
    /// Ticks remaining before this unit may sidestep again. Decremented each tick; reset to 0
    /// on new order.
    pub sidestep_cooldown: u16,
    /// Ticks remaining before this scout car may inject another reverse recovery waypoint.
    /// Used only by scout cars; reset to 0 on new order.
    pub scout_car_recovery_cooldown: u16,
    /// Immediate waypoint this scout car is currently reversing toward. This latches a short
    /// reverse maneuver so drive direction cannot flip every tick as the hull rotates.
    pub scout_car_reverse_waypoint: Option<(f32, f32)>,
    /// Consecutive ticks where the next path step was blocked by terrain/building occupancy.
    /// Once this reaches the debounce threshold, movement queues a fresh path to `path_goal`.
    pub static_blocked_ticks: u16,
    /// Experimental: total movement oil this vehicle has burnt over its lifetime (fractional units).
    /// Only tanks expose this through the selected-entity fuel readout today.
    pub lifetime_oil_used: f32,
    /// Experimental: sub-1 oil consumed since the last whole-oil deduction from the player's
    /// stockpile. Used by vehicle-fuel charging to round fractional cost up into integer oil.
    pub oil_debt: f32,
    /// Ticks remaining before an oil-starved vehicle may try to advance again.
    pub oil_starved_pause_ticks: u16,
    /// Ticks remaining for Command Car Breakthrough movement boost.
    pub breakthrough_ticks: u16,
    /// Ticks remaining for this Command Car's active Breakthrough aura origin.
    pub breakthrough_aura_ticks: u16,
    /// Ticks remaining after this unit last stood in smoke. Breakthrough uses this for synergy.
    pub recent_smoke_ticks: u16,
}

impl Default for MovementState {
    fn default() -> Self {
        MovementState {
            facing: 0.0,
            order: Order::Idle,
            queued_orders: Vec::new(),
            path: Vec::new(),
            last_repath_tick: 0,
            path_goal: None,
            stuck_ticks: 0,
            last_progress_pos: (0.0, 0.0),
            last_move_delta: (0.0, 0.0),
            sidestep_cooldown: 0,
            scout_car_recovery_cooldown: 0,
            scout_car_reverse_waypoint: None,
            static_blocked_ticks: 0,
            lifetime_oil_used: 0.0,
            oil_debt: 0.0,
            oil_starved_pause_ticks: 0,
            breakthrough_ticks: 0,
            breakthrough_aura_ticks: 0,
            recent_smoke_ticks: 0,
        }
    }
}

/// Weapon and active target state. Present on combat-capable entities.
#[derive(Debug, Clone)]
pub struct CombatState {
    /// Ticks until this entity may attack again (0 = ready).
    pub attack_cd: u32,
    /// Artillery consecutive shots since its current deployment/move reset.
    pub artillery_shots_fired: u16,
    /// Current attack/interaction target id. Combat uses enemy ids; gather/build commands use
    /// this for client feedback while the order executes.
    pub target_id: Option<u32>,
    /// Consecutive no-target ticks while a deployed/setup support weapon is trying to resume an
    /// unfinished attack-move order.
    pub attack_move_no_target_ticks: u16,
    /// Setup state for support weapons that must deploy before firing. Other combatants leave
    /// this packed and ignore it.
    pub setup: WeaponSetup,
    /// Current weapon/barrel facing in radians. For tanks this is independent turret state.
    pub weapon_facing: f32,
    /// Target weapon/barrel facing in radians. Useful for projection/debugging and future arcs.
    pub desired_weapon_facing: f32,
    /// Fixed center of a manually emplaced anti-tank gun's field of fire.
    pub emplacement_facing: Option<f32>,
    /// Pending facing to apply when a TearingDownToRedeploy phase completes.
    pub pending_redeploy_facing: Option<f32>,
    /// Whether this support weapon may acquire and fire at targets without a point-fire command.
    pub autocast_enabled: bool,
}

impl Default for CombatState {
    fn default() -> Self {
        CombatState {
            attack_cd: 0,
            artillery_shots_fired: 0,
            target_id: None,
            attack_move_no_target_ticks: 0,
            setup: WeaponSetup::Packed,
            weapon_facing: 0.0,
            desired_weapon_facing: 0.0,
            emplacement_facing: None,
            pending_redeploy_facing: None,
            autocast_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSetup {
    Packed,
    SettingUp {
        ticks: u16,
    },
    Deployed,
    TearingDown {
        ticks: u16,
    },
    /// Tearing down in order to re-deploy at a new facing. Sent as "tearing_down" on the wire.
    TearingDownToRedeploy {
        ticks: u16,
    },
}

impl WeaponSetup {
    pub fn to_protocol_str(self) -> &'static str {
        match self {
            WeaponSetup::Packed => "packed",
            WeaponSetup::SettingUp { .. } => "setting_up",
            WeaponSetup::Deployed => "deployed",
            WeaponSetup::TearingDown { .. } | WeaponSetup::TearingDownToRedeploy { .. } => {
                "tearing_down"
            }
        }
    }
}

/// Production queue state. Present only on buildings that can train units.
#[derive(Debug, Clone, Default)]
pub struct ProductionState {
    /// FIFO production queue (front = item being produced).
    pub queue: Vec<ProdItem>,
    /// FIFO research queue (front = item being researched).
    pub research_queue: Vec<ResearchItem>,
    /// Optional first rally stage (world pixels). When set, freshly produced units receive this
    /// order and the producer prefers the spawn exit closest to it.
    pub rally_point: Option<RallyIntent>,
    /// Additional rally stages applied as queued orders to freshly produced units.
    pub rally_queue: Vec<RallyIntent>,
}

/// Construction progress state. Present only while a building is under construction.
#[derive(Debug, Clone)]
pub struct ConstructionState {
    /// Ticks of construction accumulated so far.
    pub progress: u32,
    /// Total ticks of construction required (`building_stats.build_ticks`).
    pub total: u32,
}

/// Worker-only economy state.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct WorkerState {
    /// Present only if round-trip harvesting is reintroduced.
    pub carry: Option<CarryState>,
    /// Reserved drop-off target for future round-trip harvesting.
    pub home_city_centre: Option<u32>,
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

/// Static resource extractor state. Present only on completed-capable extractor buildings.
#[derive(Debug, Clone, Default)]
pub struct ResourceExtractorState {
    /// Ticks accumulated toward the next attached harvest payout.
    pub progress: u32,
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
    pub resource_extractor: bool,
}
