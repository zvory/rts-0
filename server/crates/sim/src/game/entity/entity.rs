use std::collections::BTreeMap;

use crate::config;
use crate::game::ability::AbilityKind;
use crate::protocol::states;
use crate::rules;
use serde::{Deserialize, Serialize};

use super::order::BUILD_UNIT_BLOCK_GRACE_TICKS;
#[cfg(test)]
use super::EntityStateGroups;
use super::{
    supports_manual_emplacement, AttackPhase, BuildPhase, CombatState, ConstructionState,
    DeconstructPhase, EntityKind, GatherPhase, MovePhase, MovementState, Order, OrderIntent,
    PanzerfaustState, ProductionState, ResourceExtractorState, ResourceNodeState, ScoutPlaneState,
    WeaponSetup, WorkerState, MAX_QUEUED_ORDERS, NEUTRAL,
};

mod production;
mod production_repeat;
mod rally;
mod research;

const BUILDING_START_HP_NUMERATOR: u32 = 1;
const BUILDING_START_HP_DENOMINATOR: u32 = 10;

/// A single simulation entity: unit, building, or resource node.
///
/// All positional state is in world pixels (`pos_x`/`pos_y` are the entity center).
/// State that only applies to a subset of kinds lives in typed optional groups, keeping
/// the store homogeneous while making kind-specific state explicit.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Remaining health ceiling. Damage taken during construction permanently lowers this value;
    /// finished entities otherwise retain their configured maximum.
    pub max_hp: u32,
    invulnerable: bool,
    /// Player id that most recently damaged this target. Used for score attribution when the
    /// death system removes the entity.
    last_damage_owner: Option<u32>,
    /// Tick on which this entity was most recently damaged by a direct hit, plus the attacker's
    /// position. Set together by combat for diagnostics and future public observation surfaces.
    last_damage_tick: Option<u32>,
    last_damage_pos: Option<(f32, f32)>,

    pub movement: Option<MovementState>,
    pub combat: Option<CombatState>,
    pub production: Option<ProductionState>,
    pub construction: Option<ConstructionState>,
    pub worker: Option<WorkerState>,
    pub resource_node: Option<ResourceNodeState>,
    pub resource_extractor: Option<ResourceExtractorState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::game) scout_plane: Option<ScoutPlaneState>,
    pub ability_cooldowns: BTreeMap<AbilityKind, u16>,
    pub ability_lockouts_until_tick: BTreeMap<AbilityKind, u32>,
    pub ability_uses_remaining: BTreeMap<AbilityKind, u16>,
    #[serde(default)]
    pub ability_charge_recharge_ticks: BTreeMap<AbilityKind, u16>,
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
            invulnerable: false,
            last_damage_owner: None,
            last_damage_tick: None,
            last_damage_pos: None,
            movement: Some(MovementState::default()),
            combat: if s.dmg > 0 || kind == EntityKind::Artillery {
                Some(initial_combat_state(kind))
            } else {
                None
            },
            production: None,
            construction: None,
            worker: matches!(kind, EntityKind::Worker | EntityKind::Golem)
                .then(WorkerState::default),
            resource_node: None,
            resource_extractor: None,
            scout_plane: (kind == EntityKind::ScoutPlane)
                .then_some(ScoutPlaneState::launched_at(x, y)),
            ability_cooldowns: BTreeMap::new(),
            ability_lockouts_until_tick: BTreeMap::new(),
            ability_uses_remaining: initial_ability_uses(kind),
            ability_charge_recharge_ticks: BTreeMap::new(),
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
            hp: if finished {
                s.hp
            } else {
                construction_hp_for_progress(s.hp, 0, s.build_ticks)
            },
            max_hp: s.hp,
            invulnerable: false,
            last_damage_owner: None,
            last_damage_tick: None,
            last_damage_pos: None,
            movement: None,
            combat: if s.dmg > 0 {
                Some(CombatState::default())
            } else {
                None
            },
            production: if rules::economy::trainable_units(kind).is_empty()
                && crate::game::upgrade::researchable_upgrades(kind).is_empty()
            {
                None
            } else {
                Some(ProductionState::default())
            },
            construction: (!finished).then_some(ConstructionState {
                progress: 0,
                total: s.build_ticks,
                cost_paid: false,
            }),
            worker: None,
            resource_node: None,
            resource_extractor: (kind == EntityKind::PumpJack)
                .then(ResourceExtractorState::default),
            scout_plane: None,
            ability_cooldowns: BTreeMap::new(),
            ability_lockouts_until_tick: BTreeMap::new(),
            ability_uses_remaining: BTreeMap::new(),
            ability_charge_recharge_ticks: BTreeMap::new(),
        })
    }

    pub fn new_node(kind: EntityKind, x: f32, y: f32) -> Option<Self> {
        let amount = rules::economy::node_amount(kind);
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
            invulnerable: false,
            last_damage_owner: None,
            last_damage_tick: None,
            last_damage_pos: None,
            movement: None,
            combat: None,
            production: None,
            construction: None,
            worker: None,
            resource_node: Some(ResourceNodeState {
                remaining: amount,
                miner: None,
            }),
            resource_extractor: None,
            scout_plane: None,
            ability_cooldowns: BTreeMap::new(),
            ability_lockouts_until_tick: BTreeMap::new(),
            ability_uses_remaining: BTreeMap::new(),
            ability_charge_recharge_ticks: BTreeMap::new(),
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
            resource_extractor: self.resource_extractor.is_some(),
            scout_plane: self.scout_plane.is_some(),
        }
    }

    pub fn order(&self) -> Order {
        self.movement
            .as_ref()
            .map(|m| m.order.clone())
            .unwrap_or(Order::Idle)
    }

    pub fn set_order(&mut self, order: Order) {
        self.cancel_panzerfaust_windup();
        if let Some(m) = self.movement.as_mut() {
            m.order = order;
        }
        self.reset_attack_move_no_target_ticks();
    }

    /// Replace only the active order. Future queued intents remain intact.
    ///
    /// This is the common command/promotion boundary for starting a fresh active order: the
    /// previous path, target latch, and path goal no longer belong to the new order.
    pub(crate) fn replace_active_order(&mut self, order: Order) {
        self.cancel_panzerfaust_windup();
        if let Some(m) = self.movement.as_mut() {
            m.order = order;
            m.path.clear();
            m.path_goal = None;
            m.last_move_delta = (0.0, 0.0);
            m.scout_car_reverse_waypoint = None;
            m.entrenchment_dig_ticks = 0;
            m.occupied_trench_id = None;
        }
        self.set_target_id(None);
        self.reset_attack_move_no_target_ticks();
    }

    #[allow(dead_code)]
    pub fn queued_orders(&self) -> &[OrderIntent] {
        self.movement
            .as_ref()
            .map(|m| m.queued_orders.as_slice())
            .unwrap_or(&[])
    }

    pub fn clear_queued_orders(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.queued_orders.clear();
        }
    }

    pub fn append_queued_order(&mut self, intent: OrderIntent) -> bool {
        let Some(m) = self.movement.as_mut() else {
            return false;
        };
        if m.queued_orders.len() >= MAX_QUEUED_ORDERS {
            return false;
        }
        m.queued_orders.push(intent);
        true
    }

    #[allow(dead_code)]
    pub fn pop_queued_order(&mut self) -> Option<OrderIntent> {
        let m = self.movement.as_mut()?;
        if m.queued_orders.is_empty() {
            None
        } else {
            Some(m.queued_orders.remove(0))
        }
    }

    pub(crate) fn pop_promoted_intent(&mut self) -> Option<OrderIntent> {
        self.pop_queued_order()
    }

    pub fn mark_move_phase(&mut self, phase: MovePhase) {
        if let Some(m) = self.movement.as_mut() {
            match &mut m.order {
                Order::Move(order) | Order::AttackMove(order) => {
                    order.execution.phase = phase;
                }
                Order::Ability(order) => {
                    order.execution.phase = phase;
                }
                _ => {}
            }
        }
    }

    pub fn move_phase(&self) -> Option<MovePhase> {
        self.movement.as_ref().and_then(|m| match &m.order {
            Order::Move(order) | Order::AttackMove(order) => Some(order.execution.phase),
            Order::Ability(order) => Some(order.execution.phase),
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
            m.scout_car_recovery_cooldown = 0;
            m.scout_car_reverse_waypoint = None;
            m.static_blocked_ticks = 0;
        }
    }

    pub(crate) fn set_movement_delta(&mut self, dx: f32, dy: f32) {
        if let Some(m) = self.movement.as_mut() {
            m.last_move_delta = if dx.is_finite() && dy.is_finite() {
                (dx, dy)
            } else {
                (0.0, 0.0)
            };
        }
    }

    pub(crate) fn movement_delta(&self) -> (f32, f32) {
        self.movement
            .as_ref()
            .map(|m| m.last_move_delta)
            .unwrap_or((0.0, 0.0))
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

    /// The intended destination for a move/attack-move order, if any.
    pub fn move_intent(&self) -> Option<(f32, f32)> {
        match self.movement.as_ref().map(|m| &m.order) {
            Some(Order::Move(order)) | Some(Order::AttackMove(order)) => {
                Some((order.intent.x, order.intent.y))
            }
            _ => None,
        }
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
                if order.execution.phase != phase {
                    order.execution.phase = phase;
                    order.execution.unit_blocked_ticks = 0;
                }
            }
        }
    }

    pub fn update_build_unit_blocked(&mut self, blocked: bool) -> Option<bool> {
        let m = self.movement.as_mut()?;
        let Order::Build(order) = &mut m.order else {
            return None;
        };
        if order.execution.phase != BuildPhase::WaitingAtSite {
            order.execution.unit_blocked_ticks = 0;
            return None;
        }
        if !blocked {
            order.execution.unit_blocked_ticks = 0;
            return Some(false);
        }
        order.execution.unit_blocked_ticks = order.execution.unit_blocked_ticks.saturating_add(1);
        Some(order.execution.unit_blocked_ticks >= BUILD_UNIT_BLOCK_GRACE_TICKS)
    }

    pub fn deconstruct_phase(&self) -> Option<DeconstructPhase> {
        self.movement.as_ref().and_then(|m| match &m.order {
            Order::Deconstruct(order) => Some(order.execution.phase),
            _ => None,
        })
    }

    pub fn mark_deconstruct_phase(&mut self, phase: DeconstructPhase) {
        if let Some(m) = self.movement.as_mut() {
            if let Order::Deconstruct(order) = &mut m.order {
                order.execution.phase = phase;
                if phase != DeconstructPhase::Deconstructing {
                    order.execution.progress = 0;
                }
            }
        }
    }

    pub fn tick_deconstruction(&mut self) -> Option<u32> {
        let m = self.movement.as_mut()?;
        let Order::Deconstruct(order) = &mut m.order else {
            return None;
        };
        if order.execution.phase != DeconstructPhase::Deconstructing {
            return None;
        }
        order.execution.progress = order.execution.progress.saturating_add(1);
        Some(order.execution.progress)
    }

    pub fn deconstruction_progress(&self) -> Option<u32> {
        let m = self.movement.as_ref()?;
        let Order::Deconstruct(order) = &m.order else {
            return None;
        };
        (order.execution.phase == DeconstructPhase::Deconstructing)
            .then_some(order.execution.progress)
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
            m.scout_car_reverse_waypoint = None;
            if m.path.is_empty() {
                m.last_move_delta = (0.0, 0.0);
            }
        }
    }

    pub fn clear_path(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.path.clear();
            m.scout_car_reverse_waypoint = None;
            m.last_move_delta = (0.0, 0.0);
        }
    }

    pub fn next_waypoint(&self) -> Option<(f32, f32)> {
        self.movement.as_ref().and_then(|m| m.path.last().copied())
    }

    pub fn pop_waypoint(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.path.pop();
            m.scout_car_reverse_waypoint = None;
        }
    }

    /// Push a waypoint to the front of the visit queue (path is stored reversed, so this
    /// makes `wp` the *next* waypoint consumed by the movement system).
    pub fn push_waypoint(&mut self, wp: (f32, f32)) {
        if let Some(m) = self.movement.as_mut() {
            m.path.push(wp);
            m.scout_car_reverse_waypoint = None;
        }
    }

    pub fn facing(&self) -> f32 {
        self.movement.as_ref().map(|m| m.facing).unwrap_or(0.0)
    }

    pub fn lifetime_oil_used(&self) -> Option<f32> {
        self.movement
            .as_ref()
            .and_then(|m| (self.kind == EntityKind::Tank).then_some(m.lifetime_oil_used))
    }

    pub(crate) fn breakthrough_ticks(&self) -> u16 {
        self.movement
            .as_ref()
            .map(|m| m.breakthrough_ticks)
            .unwrap_or(0)
    }

    pub(crate) fn breakthrough_aura_ticks(&self) -> u16 {
        self.movement
            .as_ref()
            .map(|m| m.breakthrough_aura_ticks)
            .unwrap_or(0)
    }

    pub(crate) fn recent_smoke_ticks(&self) -> u16 {
        self.movement
            .as_ref()
            .map(|m| m.recent_smoke_ticks)
            .unwrap_or(0)
    }

    pub(crate) fn start_breakthrough(&mut self, ticks: u16) {
        if self.is_unit() {
            if let Some(m) = self.movement.as_mut() {
                m.breakthrough_ticks = m.breakthrough_ticks.max(ticks);
            }
        }
    }

    pub(crate) fn start_breakthrough_aura(&mut self, ticks: u16) {
        if self.kind == EntityKind::CommandCar {
            if let Some(m) = self.movement.as_mut() {
                m.breakthrough_aura_ticks = ticks;
            }
        }
    }

    pub(crate) fn mark_in_smoke_for_breakthrough(&mut self, grace_ticks: u16) {
        if let Some(m) = self.movement.as_mut() {
            m.recent_smoke_ticks = grace_ticks;
        }
    }

    pub(crate) fn tick_breakthrough_status(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.breakthrough_ticks = m.breakthrough_ticks.saturating_sub(1);
            m.breakthrough_aura_ticks = m.breakthrough_aura_ticks.saturating_sub(1);
            m.recent_smoke_ticks = m.recent_smoke_ticks.saturating_sub(1);
        }
    }

    pub fn ability_cooldown_ticks(&self, ability: AbilityKind) -> u16 {
        self.ability_cooldowns.get(&ability).copied().unwrap_or(0)
    }

    pub fn ability_lockout_until_tick(&self, ability: AbilityKind, tick: u32) -> Option<u32> {
        self.ability_lockouts_until_tick
            .get(&ability)
            .copied()
            .filter(|until| *until > tick)
    }

    pub fn autocast_enabled(&self, ability: AbilityKind) -> Option<bool> {
        match (self.kind, ability) {
            (EntityKind::MortarTeam, AbilityKind::MortarFire) => Some(
                self.combat
                    .as_ref()
                    .map(|c| c.autocast_enabled)
                    .unwrap_or(false),
            ),
            _ => None,
        }
    }

    pub fn set_autocast_enabled(&mut self, ability: AbilityKind, enabled: bool) {
        if matches!(
            (self.kind, ability),
            (EntityKind::MortarTeam, AbilityKind::MortarFire)
        ) {
            if let Some(c) = self.combat.as_mut() {
                c.autocast_enabled = enabled;
            }
        }
    }

    pub fn ability_uses_remaining(&self, ability: AbilityKind) -> Option<u16> {
        let max_charges = crate::game::ability::definition(ability).charges?;
        Some(
            self.ability_uses_remaining
                .get(&ability)
                .copied()
                .unwrap_or(max_charges),
        )
    }

    pub fn consume_ability_use(&mut self, ability: AbilityKind) -> bool {
        let definition = crate::game::ability::definition(ability);
        match self.ability_uses_remaining(ability) {
            Some(0) => false,
            Some(_) => {
                let uses = self
                    .ability_uses_remaining
                    .entry(ability)
                    .or_insert(definition.charges.unwrap_or(0));
                *uses = uses.saturating_sub(1);
                if let Some(recharge_ticks) = definition.charge_recharge_ticks {
                    self.ability_charge_recharge_ticks
                        .entry(ability)
                        .or_insert_with(|| recharge_ticks.saturating_add(1));
                }
                true
            }
            None => true,
        }
    }

    pub fn start_ability_cooldown(&mut self, ability: AbilityKind, ticks: u16) {
        if ticks == 0 {
            self.ability_cooldowns.remove(&ability);
        } else {
            self.ability_cooldowns.insert(ability, ticks);
        }
    }

    pub fn start_ability_lockout_until(&mut self, ability: AbilityKind, until_tick: u32) {
        self.ability_lockouts_until_tick.insert(ability, until_tick);
    }

    pub fn tick_ability_cooldowns(&mut self) {
        self.ability_cooldowns.retain(|_, ticks| {
            *ticks = ticks.saturating_sub(1);
            *ticks > 0
        });
    }

    pub(crate) fn tick_ability_charge_recharges(&mut self) {
        for (&ability, &remaining) in &self.ability_uses_remaining {
            let definition = crate::game::ability::definition(ability);
            let (Some(max_charges), Some(recharge_ticks)) =
                (definition.charges, definition.charge_recharge_ticks)
            else {
                continue;
            };
            if remaining < max_charges {
                self.ability_charge_recharge_ticks
                    .entry(ability)
                    .or_insert_with(|| recharge_ticks.saturating_add(1));
            }
        }
        let uses = &mut self.ability_uses_remaining;
        self.ability_charge_recharge_ticks.retain(|ability, ticks| {
            *ticks = ticks.saturating_sub(1);
            if *ticks > 0 {
                return true;
            }
            let definition = crate::game::ability::definition(*ability);
            let (Some(max_charges), Some(recharge_ticks), Some(remaining)) = (
                definition.charges,
                definition.charge_recharge_ticks,
                uses.get_mut(ability),
            ) else {
                return false;
            };
            *remaining = remaining.saturating_add(1).min(max_charges);
            if *remaining < max_charges {
                *ticks = recharge_ticks;
                true
            } else {
                false
            }
        });
    }

    pub(crate) fn ability_charge_recharge_ticks(&self, ability: AbilityKind) -> Option<u16> {
        self.ability_charge_recharge_ticks.get(&ability).copied()
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.pos_x = x;
        self.pos_y = y;
    }

    pub(in crate::game) fn scout_plane_state(&self) -> Option<&ScoutPlaneState> {
        self.scout_plane.as_ref()
    }

    pub(in crate::game) fn scout_plane_state_mut(&mut self) -> Option<&mut ScoutPlaneState> {
        self.scout_plane.as_mut()
    }

    pub(crate) fn scout_plane_private_details(&self) -> Option<((f32, f32), Option<u32>)> {
        if self.kind != EntityKind::ScoutPlane {
            return None;
        }
        self.scout_plane
            .as_ref()
            .map(|state| (state.orbit_center, state.source_command_car))
    }

    pub(in crate::game) fn ensure_scout_plane_state(&mut self) {
        if self.kind == EntityKind::ScoutPlane && self.scout_plane.is_none() {
            self.scout_plane = Some(ScoutPlaneState::launched_at(self.pos_x, self.pos_y));
        }
    }

    pub(in crate::game) fn update_scout_plane_runtime(
        &mut self,
        orbit_center: (f32, f32),
        orbit_phase: f32,
        orbiting: bool,
    ) -> bool {
        if self.kind != EntityKind::ScoutPlane {
            return false;
        }
        let Some(state) = self.scout_plane.as_mut() else {
            return false;
        };
        state.update_runtime(orbit_center, orbit_phase, orbiting)
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
            if target_id.is_some() {
                c.attack_move_no_target_ticks = 0;
            }
        }
    }

    pub(in crate::game) fn weapon_firing_reveal_reaction_ready(
        &mut self,
        weapon: rules::combat::WeaponKind,
        target_id: u32,
        episode: super::FiringRevealEpisode,
        tick: u32,
        ticks: u32,
    ) -> bool {
        let Some(c) = self.combat.as_mut() else {
            return false;
        };
        c.firing_reveal_reaction_ready(weapon, target_id, episode, tick, ticks)
    }

    pub(in crate::game) fn retain_firing_reveal_reaction_gates(
        &mut self,
        gate_is_active: impl FnMut(u32, u32, u32, u32) -> bool,
    ) {
        if let Some(combat) = self.combat.as_mut() {
            combat.retain_firing_reveal_reaction_gates(gate_is_active);
        }
    }

    pub fn reset_attack_move_no_target_ticks(&mut self) {
        if let Some(c) = self.combat.as_mut() {
            c.attack_move_no_target_ticks = 0;
        }
    }

    pub fn increment_attack_move_no_target_ticks(&mut self) -> u16 {
        let Some(c) = self.combat.as_mut() else {
            return 0;
        };
        c.attack_move_no_target_ticks = c.attack_move_no_target_ticks.saturating_add(1);
        c.attack_move_no_target_ticks
    }

    pub fn weapon_facing(&self) -> Option<f32> {
        self.combat.as_ref().map(|c| c.weapon_facing)
    }

    pub fn set_weapon_facing(&mut self, facing: f32) {
        if let Some(c) = self.combat.as_mut() {
            c.weapon_facing = facing;
        }
    }

    pub fn set_desired_weapon_facing(&mut self, facing: f32) {
        if let Some(c) = self.combat.as_mut() {
            c.desired_weapon_facing = facing;
        }
    }

    pub fn emplacement_facing(&self) -> Option<f32> {
        self.combat.as_ref().and_then(|c| c.emplacement_facing)
    }

    pub fn set_emplacement_facing(&mut self, facing: Option<f32>) {
        if let Some(c) = self.combat.as_mut() {
            c.emplacement_facing = facing.filter(|f| f.is_finite()).map(normalize_angle);
        }
    }

    pub fn set_pending_redeploy_facing(&mut self, facing: Option<f32>) {
        if let Some(c) = self.combat.as_mut() {
            c.pending_redeploy_facing = facing.filter(|f| f.is_finite()).map(normalize_angle);
        }
    }

    pub fn pending_redeploy_facing(&self) -> Option<f32> {
        self.combat.as_ref().and_then(|c| c.pending_redeploy_facing)
    }

    pub fn attack_cd(&self) -> u32 {
        self.default_weapon_kind()
            .map(|weapon| self.weapon_cooldown(weapon))
            .unwrap_or(0)
    }

    pub fn set_attack_cd(&mut self, attack_cd: u32) {
        let Some(weapon) = self.default_weapon_kind() else {
            return;
        };
        self.set_weapon_cooldown(weapon, attack_cd);
    }

    pub(in crate::game) fn weapon_cooldown(&self, weapon: rules::combat::WeaponKind) -> u32 {
        self.combat
            .as_ref()
            .map(|c| c.weapon_cooldown(weapon))
            .unwrap_or(0)
    }

    pub(in crate::game) fn set_weapon_cooldown(
        &mut self,
        weapon: rules::combat::WeaponKind,
        ticks: u32,
    ) {
        if let Some(c) = self.combat.as_mut() {
            c.set_weapon_cooldown(weapon, ticks);
        }
    }

    pub(in crate::game) fn tick_weapon_cooldowns(&mut self) {
        if let Some(c) = self.combat.as_mut() {
            c.tick_weapon_cooldowns();
        }
    }

    pub fn artillery_shots_fired(&self) -> u16 {
        self.combat
            .as_ref()
            .map(|c| c.artillery_shots_fired)
            .unwrap_or(0)
    }

    pub fn increment_artillery_shots_fired(&mut self) -> u16 {
        let Some(c) = self.combat.as_mut() else {
            return 0;
        };
        c.artillery_shots_fired = c.artillery_shots_fired.saturating_add(1);
        c.artillery_shots_fired
    }

    pub fn reset_artillery_accuracy(&mut self) {
        if let Some(c) = self.combat.as_mut() {
            c.artillery_shots_fired = 0;
        }
    }

    pub(in crate::game) fn increment_artillery_blanket_shots_fired(&mut self) -> u16 {
        let Some(c) = self.combat.as_mut() else {
            return 0;
        };
        c.artillery_blanket_shots_fired = c.artillery_blanket_shots_fired.saturating_add(1);
        c.artillery_blanket_shots_fired
    }

    pub(in crate::game) fn reset_artillery_blanket_sequence(&mut self) {
        if let Some(c) = self.combat.as_mut() {
            c.artillery_blanket_shots_fired = 0;
        }
    }

    pub fn tick_attack_cd(&mut self) {
        let Some(weapon) = self.default_weapon_kind() else {
            return;
        };
        if let Some(c) = self.combat.as_mut() {
            c.tick_weapon_cooldown(weapon);
        }
    }

    fn default_weapon_kind(&self) -> Option<rules::combat::WeaponKind> {
        rules::combat::default_weapon_kind(self.kind)
    }

    pub fn last_damage_owner(&self) -> Option<u32> {
        self.last_damage_owner
    }

    pub fn set_last_damage_owner(&mut self, owner: Option<u32>) {
        self.last_damage_owner = owner;
    }

    pub fn invulnerable(&self) -> bool {
        self.invulnerable
    }

    pub fn set_invulnerable(&mut self, invulnerable: bool) {
        self.invulnerable = invulnerable;
    }

    pub fn apply_damage(
        &mut self,
        amount: u32,
        attribution: Option<(u32, (f32, f32), u32)>,
    ) -> bool {
        if self.hp == 0 || amount == 0 || self.invulnerable {
            return false;
        }
        if let Some(construction) = self.construction.as_ref() {
            // Construction progress is not healing. Damage destroys part of the scaffold's
            // eventual health budget, and subsequent progress only fills the reduced budget.
            self.max_hp = self.max_hp.saturating_sub(amount);
            self.hp = construction_hp_for_progress(
                self.max_hp,
                construction.progress,
                construction.total,
            );
        } else {
            self.hp = self.hp.saturating_sub(amount);
        }
        if let Some((owner, pos, tick)) = attribution {
            self.last_damage_owner = Some(owner);
            self.last_damage_tick = Some(tick);
            self.last_damage_pos = Some(pos);
        } else if self.hp == 0 {
            self.last_damage_owner = None;
            self.last_damage_tick = None;
            self.last_damage_pos = None;
        }
        true
    }

    pub fn restore_hp(&mut self, amount: u32) -> bool {
        if self.hp == 0 || amount == 0 || self.hp >= self.max_hp {
            return false;
        }
        self.hp = self.hp.saturating_add(amount).min(self.max_hp);
        true
    }

    pub fn last_damage_tick(&self) -> Option<u32> {
        self.last_damage_tick
    }

    pub fn last_damage_pos(&self) -> Option<(f32, f32)> {
        self.last_damage_pos
    }

    pub fn record_damage_from(&mut self, attacker_pos: (f32, f32), tick: u32) {
        self.last_damage_tick = Some(tick);
        self.last_damage_pos = Some(attacker_pos);
    }

    pub fn weapon_setup(&self) -> WeaponSetup {
        self.combat
            .as_ref()
            .map(|c| c.setup)
            .unwrap_or(WeaponSetup::Packed)
    }

    pub fn set_weapon_setup(&mut self, setup: WeaponSetup) {
        if let Some(c) = self.combat.as_mut() {
            if matches!(setup, WeaponSetup::Packed) {
                c.emplacement_facing = None;
                c.artillery_shots_fired = 0;
                c.artillery_blanket_shots_fired = 0;
            }
            c.setup = setup;
        }
    }

    pub fn tick_weapon_setup(&mut self) {
        if let Some(c) = self.combat.as_mut() {
            c.setup = match c.setup {
                WeaponSetup::SettingUp { ticks } => {
                    let ticks = ticks.saturating_sub(1);
                    if ticks == 0 {
                        WeaponSetup::Deployed
                    } else {
                        WeaponSetup::SettingUp { ticks }
                    }
                }
                WeaponSetup::TearingDown { ticks } => {
                    let ticks = ticks.saturating_sub(1);
                    if ticks == 0 {
                        c.emplacement_facing = None;
                        WeaponSetup::Packed
                    } else {
                        WeaponSetup::TearingDown { ticks }
                    }
                }
                WeaponSetup::TearingDownToRedeploy { ticks } => {
                    let ticks = ticks.saturating_sub(1);
                    if ticks == 0 {
                        if let Some(facing) = c.pending_redeploy_facing.take() {
                            c.emplacement_facing = Some(facing);
                            c.desired_weapon_facing = facing;
                        }
                        WeaponSetup::Packed
                    } else {
                        WeaponSetup::TearingDownToRedeploy { ticks }
                    }
                }
                setup => setup,
            };
        }
    }

    /// Start or continue the weapon transition required before this entity can move.
    /// Returns whether the entity is already packed and may move immediately.
    pub(in crate::game) fn begin_weapon_teardown_for_movement(&mut self) -> bool {
        let teardown_ticks = match self.kind {
            EntityKind::MachineGunner => config::MACHINE_GUNNER_SETUP_TICKS,
            EntityKind::AntiTankGun => config::ANTI_TANK_GUN_SETUP_TICKS,
            EntityKind::MortarTeam => config::MORTAR_TEAM_TEARDOWN_TICKS,
            EntityKind::Artillery => {
                self.reset_artillery_accuracy();
                self.reset_artillery_blanket_sequence();
                config::ARTILLERY_SETUP_TICKS
            }
            _ => return true,
        };
        if supports_manual_emplacement(self.kind) {
            self.set_emplacement_facing(None);
            self.set_pending_redeploy_facing(None);
        }
        match self.weapon_setup() {
            WeaponSetup::Packed => true,
            WeaponSetup::TearingDown { .. } => false,
            WeaponSetup::TearingDownToRedeploy { ticks } => {
                self.set_weapon_setup(WeaponSetup::TearingDown { ticks });
                false
            }
            WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed => {
                self.set_weapon_setup(WeaponSetup::TearingDown {
                    ticks: teardown_ticks,
                });
                false
            }
        }
    }

    pub fn under_construction(&self) -> bool {
        self.construction.is_some()
    }

    pub(crate) fn construction_cost_paid(&self) -> bool {
        self.construction
            .as_ref()
            .is_some_and(|construction| construction.cost_paid)
    }

    pub(crate) fn mark_construction_cost_paid(&mut self) -> bool {
        let Some(construction) = self.construction.as_mut() else {
            return false;
        };
        construction.cost_paid = true;
        true
    }

    pub fn build_progress_fraction(&self) -> Option<f32> {
        let c = self.construction.as_ref()?;
        Some(if c.total == 0 {
            1.0
        } else {
            (c.progress as f32 / c.total as f32).min(1.0)
        })
    }

    pub fn advance_construction(&mut self) -> Option<bool> {
        let c = self.construction.as_mut()?;
        c.progress = c.progress.saturating_add(1);
        if c.progress < c.total {
            self.hp = construction_hp_for_progress(self.max_hp, c.progress, c.total);
            return Some(false);
        }
        c.progress = c.total;
        self.hp = self.max_hp;
        self.construction = None;
        Some(true)
    }

    pub fn set_construction_progress(&mut self, progress: u32) -> bool {
        let Some(c) = self.construction.as_mut() else {
            return false;
        };
        c.progress = progress.min(c.total);
        self.hp = construction_hp_for_progress(self.max_hp, c.progress, c.total);
        true
    }

    pub fn remaining(&self) -> Option<u32> {
        self.resource_node.as_ref().map(|n| n.remaining)
    }

    pub fn harvest_resources(&mut self, amount: u32) -> u32 {
        let Some(node) = self.resource_node.as_mut() else {
            return 0;
        };
        let taken = amount.min(node.remaining);
        node.remaining = node.remaining.saturating_sub(taken);
        taken
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

    pub fn clear_worker_carry(&mut self) {
        if let Some(w) = self.worker.as_mut() {
            w.carry = None;
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
        !self.is_node() && self.kind != EntityKind::ScoutPlane
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
            if !self.prod_queue().is_empty() || !self.research_queue().is_empty() {
                return states::TRAIN;
            }
            return states::IDLE;
        }
        if self.kind == EntityKind::ScoutPlane {
            return states::MOVE;
        }
        match self.order() {
            Order::Idle | Order::HoldPosition => states::IDLE,
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
            Order::Deconstruct(_) => states::BUILD,
            Order::Ability(_) => states::MOVE,
            Order::ArtilleryPointFire(_) | Order::ArtilleryBlanketFire(_) => states::ATTACK,
        }
    }

    /// Clear all movement/combat orders and reset to idle (the `stop` command, deaths, etc.).
    /// Does not touch production queues (those belong to buildings).
    pub fn clear_orders(&mut self) {
        self.cancel_panzerfaust_windup();
        if let Some(m) = self.movement.as_mut() {
            m.order = Order::Idle;
            m.queued_orders.clear();
            m.path.clear();
            m.last_move_delta = (0.0, 0.0);
        }
        self.set_target_id(None);
        self.reset_artillery_accuracy();
        self.reset_artillery_blanket_sequence();
        self.reset_attack_move_no_target_ticks();
    }

    /// Clear active/queued movement and enter a hold-position stance.
    pub fn hold_position(&mut self) {
        self.cancel_panzerfaust_windup();
        if let Some(m) = self.movement.as_mut() {
            m.order = Order::HoldPosition;
            m.queued_orders.clear();
            m.path.clear();
            m.path_goal = None;
            m.last_move_delta = (0.0, 0.0);
            m.scout_car_reverse_waypoint = None;
        }
        self.set_target_id(None);
        self.reset_artillery_accuracy();
        self.reset_artillery_blanket_sequence();
        self.reset_attack_move_no_target_ticks();
    }

    /// Reset only the active order (idle + clear path + drop target latch). Leaves any
    /// queued order intents intact so the order_queue promotion pass can advance to the
    /// next one. Used by build/gather completion and failure paths that hand the worker
    /// off to its next queued order.
    pub fn clear_active_order(&mut self) {
        self.cancel_panzerfaust_windup();
        if let Some(m) = self.movement.as_mut() {
            m.order = Order::Idle;
            m.path.clear();
            m.last_move_delta = (0.0, 0.0);
        }
        self.set_target_id(None);
        self.reset_attack_move_no_target_ticks();
    }

    fn cancel_panzerfaust_windup(&mut self) {
        if matches!(
            self.combat.as_ref().and_then(|combat| combat.panzerfaust),
            Some(PanzerfaustState::Windup { .. })
        ) {
            if let Some(combat) = self.combat.as_mut() {
                combat.panzerfaust = Some(PanzerfaustState::Loaded);
            }
        }
    }

    pub(in crate::game) fn spend_panzerfaust(&mut self) {
        if self.kind == EntityKind::Panzerfaust {
            if let Some(combat) = self.combat.as_mut() {
                combat.panzerfaust = Some(PanzerfaustState::Spent);
            }
        }
    }
}

fn construction_hp_for_progress(max_hp: u32, progress: u32, total: u32) -> u32 {
    if max_hp == 0 {
        return 0;
    }
    if total == 0 || progress >= total {
        return max_hp;
    }
    let start_hp = max_hp
        .saturating_mul(BUILDING_START_HP_NUMERATOR)
        .div_ceil(BUILDING_START_HP_DENOMINATOR)
        .clamp(1, max_hp);
    let remaining_hp = max_hp.saturating_sub(start_hp);
    let gained_hp = (remaining_hp as u64)
        .saturating_mul(progress as u64)
        .checked_div(total as u64)
        .unwrap_or(remaining_hp as u64) as u32;
    start_hp.saturating_add(gained_hp).min(max_hp)
}

fn initial_ability_uses(kind: EntityKind) -> BTreeMap<AbilityKind, u16> {
    let mut uses = BTreeMap::new();
    for entry in rules::faction::CATALOGS
        .iter()
        .flat_map(|catalog| catalog.abilities.iter())
        .filter(|entry| entry.carriers.contains(&kind))
    {
        let Some(charges) = entry.charges else {
            continue;
        };
        let ability = entry.kind;
        uses.insert(ability, charges);
    }
    uses
}

fn initial_combat_state(kind: EntityKind) -> CombatState {
    let mut combat = CombatState::default();
    if kind == EntityKind::MortarTeam {
        combat.autocast_enabled = false;
    }
    if kind == EntityKind::Panzerfaust {
        combat.panzerfaust = Some(PanzerfaustState::Loaded);
    }
    combat
}

fn normalize_angle(angle: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    (angle + std::f32::consts::PI).rem_euclid(two_pi) - std::f32::consts::PI
}
