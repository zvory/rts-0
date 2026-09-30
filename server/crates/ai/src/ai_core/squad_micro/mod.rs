//! Rifle squad micro policy.
//!
//! A pure planner from one fog-filtered [`AiObservation`] to per-rifleman orders. It is the seam the
//! `ai-skirmish` harness trains against the real AI 2.1 controller, and the one a profile can call
//! once a parameter set is proven: it reads only what the owning player may see, emits ordinary
//! attack/move/hold orders, and runs on the canonical decision cadence, so a win in the harness is a
//! win a live match can reproduce.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use rts_rules::combat::default_weapon_profile;
use rts_rules::defs::unit_def;
use rts_sim::game::entity::EntityKind;

use crate::ai_core::observation::{AiEntityState, AiEntitySummary, AiObservation};
use crate::sdk::{AiActions, AiFrame, AiStrategy, UnitGroup};

mod params;

pub(crate) use params::{FocusMode, RifleSquadParams};

/// The squad kind this planner controls.
pub(crate) const SQUAD_KIND: EntityKind = EntityKind::Rifleman;

/// One planned order. Units are sorted and deduplicated.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SquadOrder {
    Attack { units: Vec<u32>, target: u32 },
    AttackMove { units: Vec<u32>, x: f32, y: f32 },
    Move { units: Vec<u32>, x: f32, y: f32 },
    Hold { units: Vec<u32> },
}

/// Cross-decision state. Only the owning planner reads or writes it.
#[derive(Clone, Debug, Default)]
pub(crate) struct SquadMicroMemory {
    holding: BTreeSet<u32>,
    retreating: BTreeMap<u32, u32>,
    advance_point: Option<(f32, f32)>,
    advancing: BTreeSet<u32>,
}

impl SquadMicroMemory {
    fn prune(&mut self, alive: &BTreeSet<u32>, tick: u32) {
        self.holding.retain(|id| alive.contains(id));
        self.advancing.retain(|id| alive.contains(id));
        self.retreating
            .retain(|id, until| alive.contains(id) && *until > tick);
    }
}

/// Mirrors the simulation combat service's `RANGE_SLACK`: a unit fires at targets whose centre is
/// within `range_tiles * TILE_SIZE + own radius + RANGE_SLACK` pixels.
const SIM_RANGE_SLACK_PX: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct RifleWeapon {
    range_tiles: f32,
    radius_px: f32,
    damage: u32,
    max_hp: u32,
}

impl RifleWeapon {
    fn current() -> Self {
        let weapon = default_weapon_profile(SQUAD_KIND);
        let stats = unit_def(SQUAD_KIND).map(|d| d.stats);
        Self {
            range_tiles: weapon.map(|w| w.range_tiles).unwrap_or(5.0),
            radius_px: stats.map(|s| s.radius).unwrap_or(9.0),
            damage: weapon.map(|w| w.dmg).unwrap_or(5).max(1),
            max_hp: stats.map(|s| s.hp).unwrap_or(45),
        }
    }

    /// Centre-to-centre distance at which the simulation lets this weapon fire.
    fn reach_px(self, tile: f32) -> f32 {
        self.range_tiles * tile + self.radius_px + SIM_RANGE_SLACK_PX
    }
}

/// Maximum hit points of one squad member, from the rules catalog.
pub(crate) fn squad_member_max_hp() -> u32 {
    RifleWeapon::current().max_hp
}

/// Damage of one squad member's shot against an unarmored target, from the rules catalog.
pub(crate) fn squad_shot_damage() -> u32 {
    RifleWeapon::current().damage
}

/// Plan this decision's orders for `squad`, advancing on `objective` when no enemy is in sight.
pub(crate) fn plan_rifle_squad(
    observation: &AiObservation,
    squad: &[u32],
    objective: (f32, f32),
    params: &RifleSquadParams,
    memory: &mut SquadMicroMemory,
) -> Vec<SquadOrder> {
    let tick = observation.tick;
    let tile = observation.map.tile_size.max(1) as f32;
    let weapon = RifleWeapon::current();
    let wanted: BTreeSet<u32> = squad.iter().copied().collect();
    let units: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| wanted.contains(&unit.id) && is_live(unit))
        .collect();
    let alive: BTreeSet<u32> = units.iter().map(|unit| unit.id).collect();
    memory.prune(&alive, tick);
    if units.is_empty() {
        return Vec::new();
    }
    let enemies: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && is_live(enemy))
        .collect();
    let mut batch = OrderBatch::default();
    let center = centroid(&units);

    let contact_px = weapon.reach_px(tile) + params.contact_margin_tiles.max(0.0) * tile;
    let in_contact = units.iter().any(|unit| {
        enemies
            .iter()
            .any(|enemy| distance(unit, enemy) <= contact_px)
    });
    if enemies.is_empty() || (params.hold && !in_contact) {
        if params.hold {
            hold_units(&units, memory, &mut batch);
        } else {
            advance(&units, center, objective, params, tile, memory, &mut batch);
        }
        return batch.finish();
    }

    let retreating = if params.retreat_hp > 0 {
        plan_retreats(
            observation,
            &units,
            &enemies,
            params,
            tile,
            memory,
            &mut batch,
        )
    } else {
        BTreeSet::new()
    };
    let fighters: Vec<&AiEntitySummary> = units
        .iter()
        .copied()
        .filter(|unit| !retreating.contains(&unit.id))
        .collect();
    memory.holding.clear();
    match params.focus {
        FocusMode::Nearest => {
            let enemy_center = centroid(&enemies);
            advance(
                &fighters,
                center,
                enemy_center,
                &RifleSquadParams {
                    regroup_tiles: 0.0,
                    ..*params
                },
                tile,
                memory,
                &mut batch,
            );
        }
        FocusMode::SquadNearest | FocusMode::Weakest => {
            memory.advancing.clear();
            memory.advance_point = None;
            assign_focus(
                &fighters, &enemies, center, params, weapon, tile, &mut batch,
            );
        }
    }
    batch.finish()
}

fn is_live(entity: &AiEntitySummary) -> bool {
    entity.is_complete && entity.hp > 0 && entity.state != AiEntityState::Dead
}

fn hold_units(units: &[&AiEntitySummary], memory: &mut SquadMicroMemory, batch: &mut OrderBatch) {
    memory.advancing.clear();
    memory.advance_point = None;
    for unit in units {
        if memory.holding.insert(unit.id) {
            batch.hold(unit.id);
        }
    }
}

/// Attack-move toward `objective`, regrouping on the squad centre first when it is strung out.
/// Orders are reissued only when the destination moves or a rifleman has gone idle, so the
/// simulation's own pathing and target acquisition are not reset every decision.
fn advance(
    units: &[&AiEntitySummary],
    center: (f32, f32),
    objective: (f32, f32),
    params: &RifleSquadParams,
    tile: f32,
    memory: &mut SquadMicroMemory,
    batch: &mut OrderBatch,
) {
    memory.holding.clear();
    let spread = units
        .iter()
        .map(|unit| distance_to(unit, center))
        .fold(0.0_f32, f32::max);
    let destination = if params.regroup_tiles > 0.0 && spread > params.regroup_tiles * tile {
        center
    } else {
        objective
    };
    let moved = memory
        .advance_point
        .is_none_or(|point| distance_between(point, destination) > tile);
    if moved {
        memory.advancing.clear();
        memory.advance_point = Some(destination);
    }
    for unit in units {
        if !memory.advancing.contains(&unit.id) || unit.state == AiEntityState::Idle {
            memory.advancing.insert(unit.id);
            batch.attack_move(unit.id, destination);
        }
    }
}

/// Pull a wounded rifleman that a visible enemy is shooting at back out of the fight, leaving the
/// rest of the squad to absorb the next volley. The last healthy riflemen never retreat.
fn plan_retreats(
    observation: &AiObservation,
    units: &[&AiEntitySummary],
    enemies: &[&AiEntitySummary],
    params: &RifleSquadParams,
    tile: f32,
    memory: &mut SquadMicroMemory,
    batch: &mut OrderBatch,
) -> BTreeSet<u32> {
    let healthy = units
        .iter()
        .filter(|unit| unit.hp > params.retreat_hp)
        .count();
    if healthy == 0 {
        memory.retreating.clear();
        return BTreeSet::new();
    }
    let mut retreating: BTreeSet<u32> = memory.retreating.keys().copied().collect();
    let map_w = observation.map.width as f32 * tile;
    let map_h = observation.map.height as f32 * tile;
    for unit in units {
        if retreating.contains(&unit.id) || unit.hp > params.retreat_hp {
            continue;
        }
        let targeted = enemies.iter().any(|enemy| enemy.target_id == Some(unit.id));
        let Some(threat) = nearest(unit, enemies) else {
            continue;
        };
        if !targeted {
            continue;
        }
        let (mut dx, mut dy) = (unit.x - threat.x, unit.y - threat.y);
        let len = dx.hypot(dy);
        if len <= f32::EPSILON {
            (dx, dy) = (1.0, 0.0);
        } else {
            (dx, dy) = (dx / len, dy / len);
        }
        let step = params.retreat_tiles.max(0.0) * tile;
        let x = (unit.x + dx * step).clamp(tile * 0.5, (map_w - tile * 0.5).max(tile * 0.5));
        let y = (unit.y + dy * step).clamp(tile * 0.5, (map_h - tile * 0.5).max(tile * 0.5));
        batch.move_to(unit.id, (x, y));
        memory.advancing.remove(&unit.id);
        memory.retreating.insert(
            unit.id,
            observation.tick.saturating_add(params.retreat_ticks),
        );
        retreating.insert(unit.id);
    }
    retreating
}

/// Assign each rifleman one target. Riflemen with fewer targets in range choose first; with the
/// overkill guard on, a target stops attracting shooters once their next shots cover its HP.
fn assign_focus(
    fighters: &[&AiEntitySummary],
    enemies: &[&AiEntitySummary],
    center: (f32, f32),
    params: &RifleSquadParams,
    weapon: RifleWeapon,
    tile: f32,
    batch: &mut OrderBatch,
) {
    let mut ranked: Vec<&AiEntitySummary> = enemies.to_vec();
    ranked.sort_by(|a, b| {
        let worker = |e: &AiEntitySummary| u8::from(e.kind == EntityKind::Worker);
        worker(a)
            .cmp(&worker(b))
            .then_with(|| match params.focus {
                FocusMode::Weakest => {
                    a.hp.cmp(&b.hp)
                        .then_with(|| distance_to(a, center).total_cmp(&distance_to(b, center)))
                }
                FocusMode::Nearest | FocusMode::SquadNearest => {
                    distance_to(a, center).total_cmp(&distance_to(b, center))
                }
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    let Some(first) = ranked.first() else {
        return;
    };
    if params.focus == FocusMode::SquadNearest && !params.overkill_guard {
        for fighter in fighters {
            attack_unless_engaged(fighter, first.id, batch);
        }
        return;
    }

    let range_px = weapon.reach_px(tile) + params.range_margin_tiles * tile;
    let mut shooters: Vec<(&AiEntitySummary, Vec<usize>)> = fighters
        .iter()
        .map(|fighter| {
            let options = ranked
                .iter()
                .enumerate()
                .filter(|(_, enemy)| distance(fighter, enemy) <= range_px)
                .map(|(index, _)| index)
                .collect();
            (*fighter, options)
        })
        .collect();
    shooters.sort_by(|(a, a_opts), (b, b_opts)| {
        a_opts
            .len()
            .cmp(&b_opts.len())
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut committed: BTreeMap<u32, u32> = BTreeMap::new();
    for (fighter, options) in shooters {
        let lethal = |index: usize, committed: &BTreeMap<u32, u32>| {
            let enemy = ranked[index];
            params.overkill_guard && committed.get(&enemy.id).copied().unwrap_or(0) >= enemy.hp
        };
        let in_range_pick = options
            .iter()
            .copied()
            .find(|index| !lethal(*index, &committed))
            .or_else(|| options.first().copied());
        let pick = in_range_pick
            .or_else(|| (0..ranked.len()).find(|index| !lethal(*index, &committed)))
            .unwrap_or(0);
        let target = ranked[pick];
        if in_range_pick.is_some() {
            *committed.entry(target.id).or_insert(0) += weapon.damage;
        }
        attack_unless_engaged(fighter, target.id, batch);
    }
}

fn attack_unless_engaged(fighter: &AiEntitySummary, target: u32, batch: &mut OrderBatch) {
    if fighter.target_id == Some(target) && fighter.state == AiEntityState::Attack {
        return;
    }
    batch.attack(fighter.id, target);
}

fn centroid(units: &[&AiEntitySummary]) -> (f32, f32) {
    let n = units.len().max(1) as f32;
    let (sx, sy) = units
        .iter()
        .fold((0.0, 0.0), |(sx, sy), unit| (sx + unit.x, sy + unit.y));
    (sx / n, sy / n)
}

fn nearest<'a>(
    unit: &AiEntitySummary,
    enemies: &[&'a AiEntitySummary],
) -> Option<&'a AiEntitySummary> {
    enemies.iter().copied().min_by(|a, b| {
        distance(unit, a)
            .total_cmp(&distance(unit, b))
            .then(a.id.cmp(&b.id))
    })
}

fn distance(a: &AiEntitySummary, b: &AiEntitySummary) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn distance_to(a: &AiEntitySummary, point: (f32, f32)) -> f32 {
    (a.x - point.0).hypot(a.y - point.1)
}

fn distance_between(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Collects per-unit orders and merges identical ones into group orders.
#[derive(Default)]
struct OrderBatch {
    attacks: BTreeMap<u32, Vec<u32>>,
    attack_moves: Vec<(u32, (f32, f32))>,
    moves: Vec<(u32, (f32, f32))>,
    holds: Vec<u32>,
}

impl OrderBatch {
    fn attack(&mut self, unit: u32, target: u32) {
        self.attacks.entry(target).or_default().push(unit);
    }

    fn attack_move(&mut self, unit: u32, point: (f32, f32)) {
        self.attack_moves.push((unit, point));
    }

    fn move_to(&mut self, unit: u32, point: (f32, f32)) {
        self.moves.push((unit, point));
    }

    fn hold(&mut self, unit: u32) {
        self.holds.push(unit);
    }

    fn finish(self) -> Vec<SquadOrder> {
        let mut orders = Vec::new();
        for (target, mut units) in self.attacks {
            units.sort_unstable();
            units.dedup();
            orders.push(SquadOrder::Attack { units, target });
        }
        for (units, (x, y)) in group_by_point(self.attack_moves) {
            orders.push(SquadOrder::AttackMove { units, x, y });
        }
        for (units, (x, y)) in group_by_point(self.moves) {
            orders.push(SquadOrder::Move { units, x, y });
        }
        if !self.holds.is_empty() {
            let mut units = self.holds;
            units.sort_unstable();
            units.dedup();
            orders.push(SquadOrder::Hold { units });
        }
        orders
    }
}

fn group_by_point(entries: Vec<(u32, (f32, f32))>) -> Vec<(Vec<u32>, (f32, f32))> {
    let mut groups: Vec<(Vec<u32>, (f32, f32))> = Vec::new();
    for (unit, point) in entries {
        match groups.iter_mut().find(|(_, existing)| {
            existing.0.to_bits() == point.0.to_bits() && existing.1.to_bits() == point.1.to_bits()
        }) {
            Some((units, _)) => units.push(unit),
            None => groups.push((vec![unit], point)),
        }
    }
    for (units, _) in &mut groups {
        units.sort_unstable();
        units.dedup();
    }
    groups
}

/// Custom-strategy adapter: controls every owned rifleman with [`plan_rifle_squad`], advancing on
/// the first enemy start when nothing is in sight. It runs inside the canonical controller, so it
/// has the same nine-tick cadence and fog-filtered frame as the built-in profiles.
pub(crate) struct SquadMicroStrategy {
    params: RifleSquadParams,
    memory: SquadMicroMemory,
}

impl SquadMicroStrategy {
    pub(crate) fn new(params: RifleSquadParams) -> Self {
        Self {
            params,
            memory: SquadMicroMemory::default(),
        }
    }
}

impl fmt::Debug for SquadMicroStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SquadMicroStrategy({})", self.params)
    }
}

impl AiStrategy for SquadMicroStrategy {
    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let Some(observation) = AiObservation::from_frame(frame) else {
            return;
        };
        let squad: Vec<u32> = observation
            .owned
            .iter()
            .filter(|unit| unit.kind == SQUAD_KIND)
            .map(|unit| unit.id)
            .collect();
        let Some(objective) = enemy_start_center(&observation) else {
            return;
        };
        let orders = plan_rifle_squad(
            &observation,
            &squad,
            objective,
            &self.params,
            &mut self.memory,
        );
        for order in orders {
            emit_sdk_order(actions, order);
        }
    }
}

fn enemy_start_center(observation: &AiObservation) -> Option<(f32, f32)> {
    let own_team = observation
        .players
        .iter()
        .find(|player| player.id == observation.player_id)
        .map(|player| player.team_id)?;
    let tile = observation.map.tile_size as f32;
    observation
        .players
        .iter()
        .filter(|player| player.team_id != own_team)
        .min_by_key(|player| (!player.is_alive, player.id))
        .map(|player| {
            (
                (player.start_tile.0 as f32 + 0.5) * tile,
                (player.start_tile.1 as f32 + 0.5) * tile,
            )
        })
}

fn emit_sdk_order(actions: &mut AiActions, order: SquadOrder) {
    // Units are pre-grouped and each rifleman appears in at most one order, so a rejected request
    // (for example a stale id) only drops that order.
    let _ = match order {
        SquadOrder::Attack { units, target } => {
            UnitGroup::new(units).and_then(|group| actions.attack(&group, target, false))
        }
        SquadOrder::AttackMove { units, x, y } => {
            UnitGroup::new(units).and_then(|group| actions.attack_move(&group, x, y, false))
        }
        SquadOrder::Move { units, x, y } => {
            UnitGroup::new(units).and_then(|group| actions.move_group(&group, x, y, false))
        }
        SquadOrder::Hold { units } => {
            UnitGroup::new(units).and_then(|group| actions.hold_position(&group, false))
        }
    };
}

#[cfg(test)]
mod tests;
