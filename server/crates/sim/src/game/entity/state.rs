use std::collections::BTreeMap;

use crate::config;
use crate::game::upgrade::UpgradeKind;
use crate::rules::combat::WeaponKind;
use serde::{Deserialize, Serialize};

use super::{EntityKind, Order, OrderIntent, RallyIntent};

/// A queued production order on a building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProdItem {
    /// Unit kind being produced.
    pub unit: EntityKind,
    /// Ticks of progress accumulated on this item so far.
    pub progress: u32,
    /// Total ticks required to finish this item.
    pub total: u32,
}

/// A queued research order on a tech building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchItem {
    pub upgrade: UpgradeKind,
    pub progress: u32,
    pub total: u32,
}

/// Authoritative runtime carried by an active Scout Plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(in crate::game) struct ScoutPlaneState {
    /// Current intended orbit center in world pixels.
    pub(in crate::game) orbit_center: (f32, f32),
    /// Deterministic phase around the orbit ring, in radians.
    pub(in crate::game) orbit_phase: f32,
    /// Whether the plane has reached the orbit area for `orbit_center`.
    pub(in crate::game) orbiting: bool,
    /// City Centre this sortie should return to after station time expires.
    #[serde(default)]
    pub(in crate::game) home_city_centre: Option<u32>,
    /// Ticks remaining on station after reaching the orbit area.
    #[serde(default = "default_scout_plane_station_ticks")]
    pub(in crate::game) station_ticks_remaining: u16,
    /// Whether the plane is flying back to its launch City Centre.
    #[serde(default)]
    pub(in crate::game) returning: bool,
}

impl ScoutPlaneState {
    pub(in crate::game) fn launched_at(x: f32, y: f32) -> Self {
        Self {
            orbit_center: (x, y),
            orbit_phase: 0.0,
            orbiting: false,
            home_city_centre: None,
            station_ticks_remaining: config::SCOUT_PLANE_ORBIT_DURATION_TICKS,
            returning: false,
        }
    }

    pub(in crate::game) fn launched_from(home_city_centre: u32, x: f32, y: f32) -> Self {
        Self {
            home_city_centre: Some(home_city_centre),
            ..Self::launched_at(x, y)
        }
    }

    pub(in crate::game) fn update_runtime(
        &mut self,
        orbit_center: (f32, f32),
        orbit_phase: f32,
        orbiting: bool,
    ) -> bool {
        if !orbit_center.0.is_finite() || !orbit_center.1.is_finite() || !orbit_phase.is_finite() {
            return false;
        }
        self.orbit_center = orbit_center;
        self.orbit_phase = orbit_phase;
        self.orbiting = orbiting;
        true
    }
}

fn default_scout_plane_station_ticks() -> u16 {
    config::SCOUT_PLANE_ORBIT_DURATION_TICKS
}

/// Reserved for future round-trip harvesting if attached mining is replaced.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CarryState {
    /// Amount of resource currently held.
    pub amount: u32,
    /// Resource kind being carried.
    pub kind: EntityKind,
}

/// Mobile unit state. Only units have this group.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Consecutive ticks this unit has held ground on untrenched terrain toward creating a trench.
    pub entrenchment_dig_ticks: u32,
    /// Active trench occupied by this unit. This is set only when the unit is stopped in a trench.
    pub occupied_trench_id: Option<u32>,
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
            entrenchment_dig_ticks: 0,
            occupied_trench_id: None,
        }
    }
}

/// Weapon and active target state. Present on combat-capable entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatState {
    /// Ticks until each weapon may attack again (missing or 0 = ready).
    pub weapon_cooldowns: BTreeMap<WeaponKind, u32>,
    /// Artillery consecutive shots since its current deployment/move reset.
    pub artillery_shots_fired: u16,
    /// Blanket Fire shots since the current blanket order began.
    pub artillery_blanket_shots_fired: u16,
    /// Current attack/interaction target id. Combat uses enemy ids; gather/build commands use
    /// this for client feedback while the order executes.
    pub target_id: Option<u32>,
    /// Per-weapon target id this combatant has already spent its firing-reveal reaction delay on.
    pub firing_reveal_response_targets: BTreeMap<WeaponKind, u32>,
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
    /// Ticks this tank has spent still enough to extend its weapon range.
    pub tank_stationary_range_ticks: u16,
    /// Set when tank movement reset the range this tick, so combat does not immediately re-add one
    /// stationary tick after the movement phase.
    pub tank_stationary_range_reset_this_tick: bool,
    /// Panzerfaust loaded-shot runtime. Only Panzerfaust entities carry this; the projectile is
    /// hidden while in flight or reloading, then restored when the state returns to Loaded.
    pub panzerfaust: Option<PanzerfaustState>,
}

impl Default for CombatState {
    fn default() -> Self {
        CombatState {
            weapon_cooldowns: BTreeMap::new(),
            artillery_shots_fired: 0,
            artillery_blanket_shots_fired: 0,
            target_id: None,
            firing_reveal_response_targets: BTreeMap::new(),
            attack_move_no_target_ticks: 0,
            setup: WeaponSetup::Packed,
            weapon_facing: 0.0,
            desired_weapon_facing: 0.0,
            emplacement_facing: None,
            pending_redeploy_facing: None,
            autocast_enabled: true,
            tank_stationary_range_ticks: 0,
            tank_stationary_range_reset_this_tick: false,
            panzerfaust: None,
        }
    }
}

impl CombatState {
    pub(in crate::game) fn weapon_cooldown(&self, weapon: WeaponKind) -> u32 {
        self.weapon_cooldowns.get(&weapon).copied().unwrap_or(0)
    }
    pub(in crate::game) fn set_weapon_cooldown(&mut self, weapon: WeaponKind, ticks: u32) {
        if ticks == 0 {
            self.weapon_cooldowns.remove(&weapon);
        } else {
            self.weapon_cooldowns.insert(weapon, ticks);
        }
    }

    pub(in crate::game) fn tick_weapon_cooldown(&mut self, weapon: WeaponKind) {
        let ticks = self.weapon_cooldown(weapon).saturating_sub(1);
        self.set_weapon_cooldown(weapon, ticks);
    }

    pub(in crate::game) fn tick_weapon_cooldowns(&mut self) {
        self.weapon_cooldowns.retain(|_, ticks| {
            *ticks = ticks.saturating_sub(1);
            *ticks > 0
        });
    }

    pub(in crate::game) fn start_firing_reveal_response_delay(
        &mut self,
        weapon: WeaponKind,
        target_id: u32,
        ticks: u32,
    ) -> bool {
        if ticks == 0
            || self
                .firing_reveal_response_targets
                .get(&weapon)
                .is_some_and(|previous_target| *previous_target == target_id)
        {
            return false;
        }
        self.firing_reveal_response_targets
            .insert(weapon, target_id);
        self.set_weapon_cooldown(weapon, self.weapon_cooldown(weapon).saturating_add(ticks));
        true
    }

    pub(in crate::game) fn clear_firing_reveal_response_targets(&mut self) {
        self.firing_reveal_response_targets.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PanzerfaustState {
    Loaded,
    Windup {
        target: u32,
        ticks_remaining: u16,
    },
    InFlight {
        target: u32,
        impact_x: f32,
        impact_y: f32,
        ticks_remaining: u32,
    },
    Recovery {
        ticks_remaining: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructionState {
    /// Ticks of construction accumulated so far.
    pub progress: u32,
    /// Total ticks of construction required (`building_stats.build_ticks`).
    pub total: u32,
}

/// Worker-only economy state.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerState {
    /// Present only if round-trip harvesting is reintroduced.
    pub carry: Option<CarryState>,
    /// Reserved drop-off target for future round-trip harvesting.
    pub home_city_centre: Option<u32>,
}

/// Resource-node state. Present only on steel/oil nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    pub scout_plane: bool,
}
