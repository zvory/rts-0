//! Shared terrain flow for launched infantry, using only recipient-visible trench contacts.
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::sdk::{AiActions, AiEntityState, AiFrame, EntityKind, UnitGroup};
use rts_rules::balance::{self, TICK_HZ};

type Point = (f32, f32);
type Cost = (u64, u32); // Integrated nominal DPS exposure, then terrain distance.
const UNREACHABLE: Cost = (u64::MAX, u32::MAX);
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
    grid: Option<FlowGrid>,
    orders: BTreeMap<u32, (u32, Point, bool)>,
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
            if enemy.occupied_trench_id.is_some()
                && balance::is_entrenchment_eligible_infantry(enemy.kind)
                && enemy.health.current > 0
            {
                if let Some(stats) = balance::unit_stats(enemy.kind) {
                    self.contacts.insert(
                        enemy.id,
                        (
                            Threat {
                                position: enemy.position,
                                radius: (stats.range_tiles
                                    + balance::ENTRENCHMENT_RANGE_BONUS_TILES as f32)
                                    * frame.map().tile_size as f32,
                                // Nominal anti-infantry DPS. Hidden enemy upgrades are deliberately unknown.
                                dps: (stats.dmg * TICK_HZ * 100 / stats.cooldown.max(1)).max(1),
                            },
                            frame.tick(),
                        ),
                    );
                }
            } else {
                // Observed departure/death cancels memory immediately.
                self.contacts.remove(&enemy.id);
            }
        }
        if self.contacts.is_empty() {
            // Return each previously steered unit to its original objective exactly once.
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
            self.grid = None;
            return;
        }
        let threats = self
            .contacts
            .values()
            .map(|(threat, _)| threat.clone())
            .collect::<Vec<_>>();
        let input = GridInput::from_frame(frame, target, threats);
        let changed = self.grid.as_ref().is_none_or(|grid| grid.input != input);
        if changed {
            self.grid = FlowGrid::new(input);
        }
        let Some(grid) = &self.grid else { return };
        for entity in frame.owned().iter().filter(|e| launched.contains(&e.id)) {
            // A newly observed building can invalidate an old waypoint or every arrival tile.
            // Let ordinary combat/pathing handle that objective instead of retaining a stale Move.
            let (point, breach) = grid.waypoint(entity.position).unwrap_or((target, true));
            let prior = self.orders.get(&entity.id);
            let due = prior.is_none_or(|(tick, previous, was_breach)| {
                changed
                    || *was_breach != breach
                    || (frame.tick().saturating_sub(*tick) >= ORDER_INTERVAL
                        && (distance(*previous, point) > grid.input.size * 0.5
                            || entity.state == AiEntityState::Idle))
            });
            if !due {
                continue;
            }
            let Ok(group) = UnitGroup::new([entity.id]) else {
                continue;
            };
            let result = if breach {
                actions.attack_move(&group, point.0, point.1, false)
            } else {
                // Move suppresses opportunistic chasing back into the avoided firing circles.
                actions.move_group(&group, point.0, point.1, false)
            };
            if result.is_ok() {
                self.orders.insert(entity.id, (frame.tick(), point, breach));
            }
        }
    }
}

#[derive(PartialEq)]
struct GridInput {
    width: usize,
    height: usize,
    size: f32,
    target: Point,
    blocked: Vec<bool>,
    threats: Vec<Threat>,
}

impl GridInput {
    fn from_frame(frame: &AiFrame, target: Point, threats: Vec<Threat>) -> Self {
        let map = frame.map();
        let mut blocked = (0..map.height)
            .flat_map(|y| {
                (0..map.width).map(move |x| !super::staging::passable(map, x as i32, y as i32))
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
            if kind == EntityKind::TankTrap {
                continue;
            } // Infantry can cross Tank Traps.
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
                }
            }
        }
        Self {
            width: map.width as usize,
            height: map.height as usize,
            size,
            target,
            blocked,
            threats,
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
        grid.safe = grid.field(true);
        grid.soft = grid.field(false);
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

    fn edge(&self, from: usize, to: usize, step: u32, safe: bool) -> Cost {
        (
            if safe {
                0
            } else {
                (self.danger[from] as u64 + self.danger[to] as u64) * step as u64
            },
            step,
        )
    }

    fn field(&self, safe: bool) -> Vec<Cost> {
        let mut costs = vec![UNREACHABLE; self.input.blocked.len()];
        let mut open = BinaryHeap::new();
        for (cell, cost) in costs.iter_mut().enumerate() {
            if self.passable(cell, safe)
                && distance(self.input.center(cell), self.input.target)
                    <= GOAL_RADIUS * self.input.size
            {
                *cost = (0, 0);
                open.push(Reverse(((0, 0), cell)));
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
                let candidate = (cost.0.saturating_add(edge.0), cost.1.saturating_add(edge.1));
                if candidate < costs[next] {
                    costs[next] = candidate;
                    open.push(Reverse((candidate, next)));
                }
            }
        }
        costs
    }

    fn next(&self, cell: usize, safe: bool) -> Option<usize> {
        let costs = if safe { &self.safe } else { &self.soft };
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
                    costs[next].1.saturating_add(edge.1),
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

    /// Safe detours use Move. An unavoidable firing zone uses AttackMove at its weak approach.
    fn waypoint(&self, position: Point) -> Option<(Point, bool)> {
        let mut cell = self.input.cell(position)?;
        let safe = self.safe.get(cell).is_some_and(|cost| *cost != UNREACHABLE);
        let costs = if safe { &self.safe } else { &self.soft };
        if costs.get(cell) == Some(&UNREACHABLE) {
            return None;
        }
        if costs.get(cell) == Some(&(0, 0)) {
            return Some((self.input.target, true));
        }
        let mut point = self.input.center(cell);
        let mut breach = !safe && self.danger[cell] > 0;
        for _ in 0..WAYPOINT_TILES {
            let Some(next) = self.next(cell, safe) else {
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
