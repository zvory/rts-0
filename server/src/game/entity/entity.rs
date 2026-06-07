use std::collections::BTreeMap;

use crate::config;
use crate::game::ability::AbilityKind;
use crate::protocol::states;
use crate::rules;

#[cfg(test)]
use super::EntityStateGroups;
use super::{
    AttackPhase, BuildPhase, CombatState, ConstructionState, EntityKind, GatherPhase, MovePhase,
    MovementState, Order, OrderIntent, PointIntent, ProdItem, ProductionState, ResourceNodeState,
    WeaponSetup, WorkerState, MAX_QUEUED_ORDERS, NEUTRAL,
};

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
    /// Player id that most recently damaged this target. Used for score attribution when the
    /// death system removes the entity.
    last_damage_owner: Option<u32>,
    /// Tick on which this entity was most recently damaged by a direct hit, plus the attacker's
    /// position. Set together by combat; used by the AI controller to issue retreat commands.
    last_damage_tick: Option<u32>,
    last_damage_pos: Option<(f32, f32)>,

    pub movement: Option<MovementState>,
    pub combat: Option<CombatState>,
    pub production: Option<ProductionState>,
    pub construction: Option<ConstructionState>,
    pub worker: Option<WorkerState>,
    pub resource_node: Option<ResourceNodeState>,
    pub ability_cooldowns: BTreeMap<AbilityKind, u16>,
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
            last_damage_owner: None,
            last_damage_tick: None,
            last_damage_pos: None,
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
            ability_cooldowns: BTreeMap::new(),
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
            last_damage_owner: None,
            last_damage_tick: None,
            last_damage_pos: None,
            movement: None,
            combat: if s.dmg > 0 {
                Some(CombatState::default())
            } else {
                None
            },
            production: if rules::economy::trainable_units(kind).is_empty() {
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
            ability_cooldowns: BTreeMap::new(),
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
            ability_cooldowns: BTreeMap::new(),
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
                if phase == AttackPhase::Firing {
                    order.execution.unreachable_checks = 0;
                }
            }
        }
    }

    pub fn reset_attack_unreachable_checks(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            if let Order::Attack(order) = &mut m.order {
                order.execution.unreachable_checks = 0;
            }
        }
    }

    pub fn increment_attack_unreachable_checks(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            if let Order::Attack(order) = &mut m.order {
                order.execution.unreachable_checks =
                    order.execution.unreachable_checks.saturating_add(1);
            }
        }
    }

    pub fn attack_unreachable_checks(&self) -> u16 {
        self.movement
            .as_ref()
            .and_then(|m| match &m.order {
                Order::Attack(order) => Some(order.execution.unreachable_checks),
                _ => None,
            })
            .unwrap_or(0)
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
            m.scout_car_reverse_waypoint = None;
        }
    }

    pub fn clear_path(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.path.clear();
            m.scout_car_reverse_waypoint = None;
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

    pub fn charge_ticks(&self) -> u16 {
        self.movement.as_ref().map(|m| m.charge_ticks).unwrap_or(0)
    }

    pub fn charge_cooldown_ticks(&self) -> u16 {
        self.ability_cooldown_ticks(AbilityKind::Charge)
    }

    pub fn start_charge(&mut self, ticks: u16) {
        if self.kind == EntityKind::Rifleman {
            if let Some(m) = self.movement.as_mut() {
                m.charge_ticks = ticks;
            }
        }
    }

    pub fn tick_charge(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.charge_ticks = m.charge_ticks.saturating_sub(1);
        }
    }

    pub fn ability_cooldown_ticks(&self, ability: AbilityKind) -> u16 {
        self.ability_cooldowns.get(&ability).copied().unwrap_or(0)
    }

    pub fn start_ability_cooldown(&mut self, ability: AbilityKind, ticks: u16) {
        if ticks == 0 {
            self.ability_cooldowns.remove(&ability);
        } else {
            self.ability_cooldowns.insert(ability, ticks);
        }
    }

    pub fn tick_ability_cooldowns(&mut self) {
        self.ability_cooldowns.retain(|_, ticks| {
            *ticks = ticks.saturating_sub(1);
            *ticks > 0
        });
    }

    pub fn movement_speed_multiplier(&self) -> f32 {
        if self.kind == EntityKind::Rifleman && self.charge_ticks() > 0 {
            config::RIFLEMAN_CHARGE_SPEED_MULTIPLIER
        } else {
            1.0
        }
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

    pub fn last_damage_owner(&self) -> Option<u32> {
        self.last_damage_owner
    }

    pub fn set_last_damage_owner(&mut self, owner: Option<u32>) {
        self.last_damage_owner = owner;
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

    pub fn prod_queue(&self) -> &[ProdItem] {
        self.production
            .as_ref()
            .map(|p| p.queue.as_slice())
            .unwrap_or(&[])
    }

    pub fn prod_queue_mut(&mut self) -> Option<&mut Vec<ProdItem>> {
        self.production.as_mut().map(|p| &mut p.queue)
    }

    /// Rally point for a unit-producing building, if one has been set.
    pub fn rally_point(&self) -> Option<(f32, f32)> {
        self.production.as_ref().and_then(|p| p.rally_point)
    }

    /// Set (or clear with `None`) this building's rally point. No-op on entities without a
    /// production component.
    pub fn set_rally_point(&mut self, rally: Option<(f32, f32)>) {
        if let Some(p) = self.production.as_mut() {
            p.rally_point = rally;
        }
    }

    #[allow(dead_code)]
    pub fn rally_stages(&self) -> &[PointIntent] {
        self.production
            .as_ref()
            .map(|p| p.rally_queue.as_slice())
            .unwrap_or(&[])
    }

    pub fn clear_rally_stages(&mut self) {
        if let Some(p) = self.production.as_mut() {
            p.rally_queue.clear();
        }
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
            Order::Ability(_) => states::MOVE,
        }
    }

    /// Clear all movement/combat orders and reset to idle (the `stop` command, deaths, etc.).
    /// Does not touch production queues (those belong to buildings).
    pub fn clear_orders(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.order = Order::Idle;
            m.queued_orders.clear();
            m.path.clear();
        }
        self.set_target_id(None);
    }

    /// Reset only the active order (idle + clear path + drop target latch). Leaves any
    /// queued order intents intact so the order_queue promotion pass can advance to the
    /// next one. Used by build/gather completion and failure paths that hand the worker
    /// off to its next queued order.
    pub fn clear_active_order(&mut self) {
        if let Some(m) = self.movement.as_mut() {
            m.order = Order::Idle;
            m.path.clear();
        }
        self.set_target_id(None);
    }
}

fn normalize_angle(angle: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    (angle + std::f32::consts::PI).rem_euclid(two_pi) - std::f32::consts::PI
}
