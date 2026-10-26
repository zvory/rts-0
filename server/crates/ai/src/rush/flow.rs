//! Extractor raids and least-exposure combat routes from fog-filtered observations.
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::sdk::{AiActions, AiEntity, AiEntityState, AiFrame, EntityKind, UnitGroup};
use rts_rules::balance::{self, TICK_HZ};

type Point = (f32, f32);
type Cost = (u64, u8, u32, usize); // Fire exposure, extractor priority, distance, goal tie.
const UNREACHABLE: Cost = (u64::MAX, u8::MAX, u32::MAX, usize::MAX);
const CONTACT_MEMORY: u32 = TICK_HZ * 5;
const ORDER_INTERVAL: u32 = TICK_HZ;
const GOAL_RADIUS: f32 = 3.0;
const WAYPOINT_TILES: usize = 3;
const NEIGHBORS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

#[derive(Clone, Debug, PartialEq)]
struct Threat {
    position: Point,
    radius: f32,
    dps: u32,
}

#[derive(Default)]
pub(super) struct RushFlow {
    contacts: BTreeMap<u32, (Threat, u32)>,
    grids: BTreeMap<u32, FlowGrid>,
    orders: BTreeMap<u32, (u32, Intent)>,
}

impl RushFlow {
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        target: Point,
        launched: &BTreeSet<u32>,
        actions: &mut AiActions,
    ) {
        self.orders.retain(|id, _| launched.contains(id));
        self.contacts
            .retain(|_, (_, seen)| frame.tick().saturating_sub(*seen) <= CONTACT_MEMORY);
        for enemy in frame.visible_enemies() {
            if enemy.kind.is_unit()
                && !rts_rules::target::is_economy_unit(enemy.kind)
                && enemy.health.current > 0
            {
                if let Some(stats) = balance::unit_stats(enemy.kind).filter(|stats| stats.dmg > 0) {
                    self.contacts.insert(
                        enemy.id,
                        (
                            Threat {
                                position: enemy.position,
                                radius: (stats.range_tiles
                                    + if enemy.occupied_trench_id.is_some() {
                                        balance::ENTRENCHMENT_RANGE_BONUS_TILES as f32
                                    } else {
                                        0.0
                                    })
                                    * frame.map().tile_size as f32,
                                // Nominal anti-infantry DPS. Hidden enemy upgrades are deliberately unknown.
                                dps: (stats.dmg * TICK_HZ * 100 / stats.cooldown.max(1)).max(1),
                            },
                            frame.tick(),
                        ),
                    );
                }
            } else {
                // A visible economy or dead contact does not protect an extractor.
                self.contacts.remove(&enemy.id);
            }
        }
        let objectives = objectives(frame);
        if self.contacts.is_empty() && objectives.is_empty() {
            for entity in frame.owned() {
                if !self.orders.contains_key(&entity.id) {
                    continue;
                }
                if let Ok(group) = UnitGroup::new([entity.id]) {
                    if actions
                        .attack_move(&group, target.0, target.1, false)
                        .is_ok()
                    {
                        self.orders.remove(&entity.id);
                    }
                }
            }
            self.grids.clear();
            return;
        }
        let threats = self
            .contacts
            .values()
            .map(|(threat, _)| threat.clone())
            .collect::<Vec<_>>();
        let mut active_ranges = BTreeSet::new();
        let mut changed_ranges = BTreeSet::new();
        for entity in frame.owned().iter().filter(|e| launched.contains(&e.id)) {
            let Some(stats) = balance::unit_stats(entity.kind) else {
                continue;
            };
            let key = (stats.range_tiles * 1000.0).round() as u32;
            let first = active_ranges.insert(key);
            if first {
                let mut input = GridInput::from_frame(frame, target, threats.clone());
                input.range = stats.range_tiles;
                input.objectives = objectives.clone();
                let changed = self.grids.get(&key).is_none_or(|grid| grid.input != input);
                if changed {
                    changed_ranges.insert(key);
                    if let Some(grid) = FlowGrid::new(input) {
                        self.grids.insert(key, grid);
                    }
                }
            }
            let Some(grid) = self.grids.get(&key) else {
                continue;
            };
            let mut intent = grid.intent(entity).unwrap_or(Intent::AttackMove(target));
            if let Intent::AttackMove(point) = intent {
                if !grid.input.objectives.is_empty()
                    && grid
                        .input
                        .cell(entity.position)
                        .is_some_and(|cell| grid.safe[cell] == UNREACHABLE)
                {
                    intent = grid.fight_intent(frame, entity, point);
                }
            }
            let prior = self.orders.get(&entity.id);
            let due = prior.is_none_or(|(tick, previous)| {
                let committed = match intent {
                    Intent::Attack(id) => entity.target == Some(id),
                    _ => entity.state != AiEntityState::Idle,
                };
                if *previous == intent && committed {
                    return false;
                }
                changed_ranges.contains(&key)
                    || previous.mode() != intent.mode()
                    || frame.tick().saturating_sub(*tick) >= ORDER_INTERVAL
            });
            if !due {
                continue;
            }
            let Ok(group) = UnitGroup::new([entity.id]) else {
                continue;
            };
            let result = match intent {
                Intent::Move(point) => actions.move_group(&group, point.0, point.1, false),
                Intent::AttackMove(point) => actions.attack_move(&group, point.0, point.1, false),
                Intent::Attack(id) => actions.attack(&group, id, false),
            };
            if result.is_ok() {
                self.orders.insert(entity.id, (frame.tick(), intent));
            }
        }
        self.grids.retain(|range, _| active_ranges.contains(range));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Intent {
    Move(Point),
    AttackMove(Point),
    Attack(u32),
}
impl Intent {
    fn mode(self) -> u8 {
        match self {
            Self::Move(_) => 0,
            Self::AttackMove(_) => 1,
            Self::Attack(_) => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Objective {
    id: u32,
    kind: EntityKind,
    position: Point,
    visible: bool,
}

fn objectives(frame: &AiFrame) -> Vec<Objective> {
    let mut known = BTreeMap::new();
    for contact in frame.remembered_contacts() {
        if matches!(contact.kind, EntityKind::PumpJack | EntityKind::SteelMine) {
            known.insert(
                contact.id,
                Objective {
                    id: contact.id,
                    kind: contact.kind,
                    position: contact.position,
                    visible: false,
                },
            );
        }
    }
    for entity in frame.visible_enemies() {
        if matches!(entity.kind, EntityKind::PumpJack | EntityKind::SteelMine)
            && entity.health.current > 0
        {
            known.insert(
                entity.id,
                Objective {
                    id: entity.id,
                    kind: entity.kind,
                    position: entity.position,
                    visible: true,
                },
            );
        }
    }
    known.into_values().collect()
}

#[derive(PartialEq)]
struct GridInput {
    width: usize,
    height: usize,
    size: f32,
    target: Point,
    blocked: Vec<bool>,
    shot_blocked: Vec<bool>,
    threats: Vec<Threat>,
    objectives: Vec<Objective>,
    range: f32,
}

impl GridInput {
    fn from_frame(frame: &AiFrame, target: Point, threats: Vec<Threat>) -> Self {
        let map = frame.map();
        let mut blocked = (0..map.height)
            .flat_map(|y| {
                (0..map.width).map(move |x| !super::staging::passable(map, x as i32, y as i32))
            })
            .collect::<Vec<_>>();
        let mut shot_blocked = map
            .terrain
            .iter()
            .map(|terrain| {
                matches!(
                    terrain,
                    crate::sdk::AiTerrain::Rock | crate::sdk::AiTerrain::Unknown(_)
                )
            })
            .collect::<Vec<_>>();
        let size = map.tile_size as f32;
        // Buildings are observed obstacles. Never inspect hidden simulation occupancy.
        let buildings = frame
            .owned()
            .iter()
            .chain(frame.visible_allies())
            .chain(frame.visible_enemies())
            .map(|e| (e.kind, e.position))
            .chain(
                frame
                    .remembered_contacts()
                    .iter()
                    .map(|e| (e.kind, e.position)),
            );
        for (kind, position) in buildings {
            let Some(stats) = balance::building_stats(kind) else {
                continue;
            };
            if rts_rules::static_blocker_class(kind) != rts_rules::StaticBlockerClass::AllGround {
                continue;
            } // Extractors and Tank Traps do not block infantry.
            let left = position.0 / size - stats.foot_w as f32 / 2.0;
            let top = position.1 / size - stats.foot_h as f32 / 2.0;
            for y in top.floor().max(0.0) as usize
                ..(top + stats.foot_h as f32).ceil().min(map.height as f32) as usize
            {
                for x in left.floor().max(0.0) as usize
                    ..(left + stats.foot_w as f32).ceil().min(map.width as f32) as usize
                {
                    if let Some(cell) = blocked.get_mut(y * map.width as usize + x) {
                        *cell = true;
                    }
                    if rts_rules::blocks_line_of_sight(kind) {
                        if let Some(cell) = shot_blocked.get_mut(y * map.width as usize + x) {
                            *cell = true;
                        }
                    }
                }
            }
        }
        Self {
            width: map.width as usize,
            height: map.height as usize,
            size,
            target,
            blocked,
            shot_blocked,
            threats,
            objectives: Vec::new(),
            range: 5.0,
        }
    }

    fn center(&self, cell: usize) -> Point {
        (
            (cell % self.width) as f32 * self.size + self.size * 0.5,
            (cell / self.width) as f32 * self.size + self.size * 0.5,
        )
    }

    fn cell(&self, point: Point) -> Option<usize> {
        if !point.0.is_finite() || !point.1.is_finite() || point.0 < 0.0 || point.1 < 0.0 {
            return None;
        }
        let (x, y) = (
            (point.0 / self.size) as usize,
            (point.1 / self.size) as usize,
        );
        (x < self.width && y < self.height).then_some(y * self.width + x)
    }
}

struct FlowGrid {
    input: GridInput,
    danger: Vec<u32>,
    safe: Vec<Cost>,
    soft: Vec<Cost>,
    escape: Vec<Cost>,
}

impl FlowGrid {
    fn new(input: GridInput) -> Option<Self> {
        if input.width == 0 || input.height == 0 || input.size <= 0.0 {
            return None;
        }
        let mut grid = Self {
            danger: vec![0; input.blocked.len()],
            input,
            safe: Vec::new(),
            soft: Vec::new(),
            escape: Vec::new(),
        };
        for cell in 0..grid.danger.len() {
            let center = grid.input.center(cell);
            for threat in &grid.input.threats {
                // Half diagonal plus infantry clearance: whole cell/segment stays outside range.
                if distance(center, threat.position) <= threat.radius + grid.input.size {
                    grid.danger[cell] = grid.danger[cell].saturating_add(threat.dps);
                }
            }
        }
        let seeds = grid.goal_seeds();
        grid.safe = grid.field(true, &seeds, true);
        // Combat considers every reachable goal, even if a safe goal exists on a disconnected island.
        grid.soft = grid.field(false, &seeds, false);
        // Escape considers only safe firing positions, so a covered nearby extractor cannot pin us.
        grid.escape = grid.field(false, &seeds, true);
        Some(grid)
    }

    fn passable(&self, cell: usize, safe: bool) -> bool {
        !self.input.blocked.get(cell).copied().unwrap_or(true)
            && (!safe || self.danger.get(cell) == Some(&0))
    }

    fn neighbor(&self, cell: usize, dx: i32, dy: i32, safe: bool) -> Option<usize> {
        let (x, y) = (cell % self.input.width, cell / self.input.width);
        let (nx, ny) = (
            x.checked_add_signed(dx as isize)?,
            y.checked_add_signed(dy as isize)?,
        );
        if nx >= self.input.width || ny >= self.input.height {
            return None;
        }
        let next = ny * self.input.width + nx;
        if !self.passable(next, safe)
            || (dx != 0
                && dy != 0
                && (!self.passable(y * self.input.width + nx, safe)
                    || !self.passable(ny * self.input.width + x, safe)))
        {
            return None;
        }
        Some(next)
    }

    fn edge(&self, from: usize, to: usize, step: u32, safe: bool) -> (u64, u32) {
        (
            if safe {
                0
            } else {
                (self.danger[from] as u64 + self.danger[to] as u64) * step as u64
            },
            step,
        )
    }

    fn goal_seeds(&self) -> Vec<(usize, u8, usize)> {
        let mut seeds = Vec::new();
        for cell in 0..self.input.blocked.len() {
            if !self.passable(cell, false) {
                continue;
            }
            let point = self.input.center(cell);
            if self.input.objectives.is_empty() {
                if distance(point, self.input.target) <= GOAL_RADIUS * self.input.size {
                    seeds.push((cell, 2, 0));
                }
                continue;
            }
            for (index, objective) in self.input.objectives.iter().enumerate() {
                if self.within_range(point, objective, self.input.range - 0.4)
                    && self.firing_lane(point, objective.position)
                {
                    seeds.push((
                        cell,
                        if objective.kind == EntityKind::PumpJack {
                            0
                        } else {
                            1
                        },
                        index,
                    ));
                }
            }
        }
        seeds
    }

    fn within_range(&self, point: Point, objective: &Objective, range: f32) -> bool {
        let Some(stats) = balance::building_stats(objective.kind) else {
            return false;
        };
        let half = (
            stats.foot_w as f32 * self.input.size * 0.5,
            stats.foot_h as f32 * self.input.size * 0.5,
        );
        let nearest = (
            point
                .0
                .clamp(objective.position.0 - half.0, objective.position.0 + half.0),
            point
                .1
                .clamp(objective.position.1 - half.1, objective.position.1 + half.1),
        );
        distance(point, nearest) <= range * self.input.size
    }

    fn firing_lane(&self, from: Point, to: Point) -> bool {
        let steps = (distance(from, to) / (self.input.size * 0.25))
            .ceil()
            .max(1.0) as usize;
        (0..=steps).all(|i| {
            let t = i as f32 / steps as f32;
            self.input
                .cell((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t))
                .is_some_and(|cell| !self.input.shot_blocked[cell])
        })
    }

    fn field(&self, safe: bool, seeds: &[(usize, u8, usize)], has_safe_goal: bool) -> Vec<Cost> {
        let mut costs = vec![UNREACHABLE; self.input.blocked.len()];
        let mut open = BinaryHeap::new();
        for &(cell, priority, goal) in seeds {
            if !self.passable(cell, safe) || (has_safe_goal && self.danger[cell] > 0) {
                continue;
            }
            let cost = (
                if safe {
                    0
                } else {
                    self.danger[cell] as u64 * 30
                },
                priority,
                0,
                goal,
            );
            if cost < costs[cell] {
                costs[cell] = cost;
                open.push(Reverse((cost, cell)));
            }
        }
        while let Some(Reverse((cost, cell))) = open.pop() {
            if costs[cell] != cost {
                continue;
            }
            for (dx, dy) in NEIGHBORS {
                let Some(next) = self.neighbor(cell, dx, dy, safe) else {
                    continue;
                };
                let edge = self.edge(cell, next, if dx == 0 || dy == 0 { 10 } else { 14 }, safe);
                let candidate = (
                    cost.0.saturating_add(edge.0),
                    cost.1,
                    cost.2.saturating_add(edge.1),
                    cost.3,
                );
                if candidate < costs[next] {
                    costs[next] = candidate;
                    open.push(Reverse((candidate, next)));
                }
            }
        }
        costs
    }

    #[cfg(test)]
    fn next(&self, cell: usize, safe: bool) -> Option<usize> {
        let costs = if safe { &self.safe } else { &self.soft };
        self.next_on(cell, safe, costs)
    }

    fn next_on(&self, cell: usize, safe: bool, costs: &[Cost]) -> Option<usize> {
        NEIGHBORS
            .into_iter()
            .filter_map(|(dx, dy)| {
                let next = self.neighbor(cell, dx, dy, safe)?;
                if costs[next] == UNREACHABLE {
                    return None;
                }
                let edge = self.edge(cell, next, if dx == 0 || dy == 0 { 10 } else { 14 }, safe);
                let total = (
                    costs[next].0.saturating_add(edge.0),
                    costs[next].1,
                    costs[next].2.saturating_add(edge.1),
                    costs[next].3,
                );
                (total == costs[cell] && costs[next] < costs[cell]).then_some((total, next))
            })
            .min()
            .map(|(_, next)| next)
    }

    fn segment_clear(&self, from: Point, to: Point, safe: bool) -> bool {
        let steps = (distance(from, to) / (self.input.size * 0.2))
            .ceil()
            .max(1.0) as usize;
        (0..=steps).all(|i| {
            let t = i as f32 / steps as f32;
            let point = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
            // Check body clearance as well as center; don't skim terrain/building corners.
            [(0.0, 0.0), (-0.3, 0.0), (0.3, 0.0), (0.0, -0.3), (0.0, 0.3)]
                .into_iter()
                .all(|(dx, dy)| {
                    self.input
                        .cell((
                            point.0 + dx * self.input.size,
                            point.1 + dy * self.input.size,
                        ))
                        .is_some_and(|cell| self.passable(cell, safe))
                })
        })
    }

    fn fight_intent(&self, frame: &AiFrame, entity: &AiEntity, point: Point) -> Intent {
        let guards = frame
            .visible_enemies()
            .iter()
            .filter(|enemy| {
                enemy.kind.is_unit()
                    && !rts_rules::target::is_economy_unit(enemy.kind)
                    && enemy.health.current > 0
                    && balance::unit_stats(enemy.kind).is_some_and(|stats| stats.dmg > 0)
            })
            .collect::<Vec<_>>();
        let target = guards
            .iter()
            .copied()
            .filter(|enemy| {
                distance(entity.position, enemy.position)
                    <= (self.input.range - 0.1) * self.input.size
                    && self.firing_lane(entity.position, enemy.position)
            })
            .min_by(|a, b| {
                distance(a.position, point)
                    .total_cmp(&distance(b.position, point))
                    .then_with(|| a.id.cmp(&b.id))
            });
        if let Some(enemy) = target {
            Intent::Attack(enemy.id)
        } else if !guards.is_empty() {
            Intent::Move(point)
        } else {
            Intent::AttackMove(point)
        }
    }

    fn intent(&self, entity: &AiEntity) -> Option<Intent> {
        let cell = self.input.cell(entity.position)?;
        if !self.input.objectives.is_empty() && self.safe[cell] != UNREACHABLE {
            let goal = self.input.objectives.get(self.safe[cell].3)?;
            if self.safe[cell].2 == 0 {
                if !self.within_range(entity.position, goal, self.input.range - 0.1)
                    || !self.firing_lane(entity.position, goal.position)
                {
                    return Some(Intent::Move(self.input.center(cell)));
                }
                return Some(if goal.visible {
                    Intent::Attack(goal.id)
                } else {
                    Intent::AttackMove(goal.position)
                });
            }
            let (point, _) = self.waypoint(entity.position)?;
            return Some(Intent::Move(point));
        }
        // A unit already inside fire may escape into a component with a safe extractor route.
        // An outside unit separated from every objective by a wall fights through it instead.
        let escape = !self.input.objectives.is_empty()
            && self.danger[cell] > 0
            && self.escape_reaches_safe_goal(cell);
        if escape {
            let (point, _) = self.waypoint_on(entity.position, false, &self.escape)?;
            return Some(Intent::Move(point));
        }
        let (point, breach) = self.waypoint(entity.position)?;
        Some(if !breach {
            Intent::Move(point)
        } else {
            Intent::AttackMove(point)
        })
    }

    fn escape_reaches_safe_goal(&self, mut cell: usize) -> bool {
        for _ in 0..self.danger.len() {
            if self.danger[cell] == 0 {
                return self.safe[cell] != UNREACHABLE;
            }
            let Some(next) = self.next_on(cell, false, &self.escape) else {
                return false;
            };
            cell = next;
        }
        false
    }

    /// Safe detours use Move. An unavoidable firing zone uses AttackMove at its weak approach.
    fn waypoint(&self, position: Point) -> Option<(Point, bool)> {
        let cell = self.input.cell(position)?;
        let safe = self.safe.get(cell).is_some_and(|cost| *cost != UNREACHABLE);
        self.waypoint_on(position, safe, if safe { &self.safe } else { &self.soft })
    }

    fn waypoint_on(&self, position: Point, safe: bool, costs: &[Cost]) -> Option<(Point, bool)> {
        let mut cell = self.input.cell(position)?;
        if costs.get(cell) == Some(&UNREACHABLE) {
            return None;
        }
        if costs.get(cell).is_some_and(|cost| cost.2 == 0) {
            let point = self
                .input
                .objectives
                .get(costs[cell].3)
                .map_or(self.input.target, |goal| goal.position);
            return Some((point, true));
        }
        let mut point = self.input.center(cell);
        let mut breach = !safe && self.danger[cell] > 0;
        for _ in 0..WAYPOINT_TILES {
            let Some(next) = self.next_on(cell, safe, costs) else {
                break;
            };
            let candidate = self.input.center(next);
            if !self.segment_clear(position, candidate, safe) {
                break;
            }
            cell = next;
            point = candidate;
            breach |= !safe && self.danger[cell] > 0;
        }
        Some((point, breach))
    }
}

fn distance(a: Point, b: Point) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[cfg(test)]
mod tests;
