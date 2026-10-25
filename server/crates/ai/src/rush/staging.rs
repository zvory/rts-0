//! Opening Riflemen attack immediately; reinforcement cohorts assemble on a terrain-routed front.
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::sdk::{AiActions, AiEntityState, AiFrame, AiMap, AiTerrain, EntityKind, UnitGroup};

type Point = (f32, f32);
const STAGING_DISTANCE_TILES: f32 = 25.0;
const PANZERFAUST_REAR_OFFSET_TILES: f32 = 3.0;
const SLOT_SPACING_TILES: f32 = 1.5;
const ARRIVAL_TILES: f32 = 0.75;
const RETRY_TICKS: u32 = 90;

#[derive(Default)]
pub(super) struct RushWaves {
    plan: Option<(u32, StagingLine)>,
    slots: BTreeMap<u32, Point>,
    issued: BTreeMap<u32, u32>,
    held: BTreeSet<u32>,
    launched: BTreeSet<u32>,
    opening_dispatched: bool,
    flow: super::flow::RushFlow,
}

#[derive(Clone, Copy, Debug)]
struct StagingLine {
    center: Point,
    forward: Point,
}

impl RushWaves {
    pub(super) fn rally(&mut self, frame: &AiFrame, enemy_id: u32, target: Point) -> Option<Point> {
        if self.plan.as_ref().is_none_or(|(id, _)| *id != enemy_id) {
            let home = frame
                .players()
                .iter()
                .find(|player| player.id == frame.player_id())?;
            let size = frame.map().tile_size as f32;
            let home = (
                (home.start_tile.0 as f32 + 0.5) * size,
                (home.start_tile.1 as f32 + 0.5) * size,
            );
            self.plan = Some((enemy_id, StagingLine::on_route(frame.map(), home, target)?));
            self.slots.clear();
            self.issued.clear();
            self.held.clear();
        }
        self.plan.as_ref().map(|(_, line)| line.center)
    }

    pub(super) fn step(&mut self, frame: &AiFrame, target: Point, actions: &mut AiActions) {
        if !self.opening_dispatched {
            let opening = frame
                .owned()
                .iter()
                .filter(|entity| entity.kind == EntityKind::Rifleman)
                .take(4)
                .map(|entity| entity.id)
                .collect::<Vec<_>>();
            if opening.is_empty() {
                self.opening_dispatched = true;
            } else if let Ok(group) = UnitGroup::new(opening.iter().copied()) {
                if actions
                    .attack_move(&group, target.0, target.1, false)
                    .is_ok()
                {
                    self.launched.extend(opening);
                    self.opening_dispatched = true;
                }
            }
        }
        let Some((_, line)) = self.plan else {
            self.flow.step(frame, target, &self.launched, actions);
            return;
        };
        let units = frame
            .owned()
            .iter()
            .filter(|entity| {
                matches!(
                    entity.kind,
                    EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::Panzerfaust
                )
            })
            .collect::<Vec<_>>();
        let alive = units
            .iter()
            .map(|entity| entity.id)
            .collect::<BTreeSet<_>>();
        self.slots
            .retain(|id, _| alive.contains(id) && !self.launched.contains(id));
        self.launched.retain(|id| alive.contains(id));
        self.issued.retain(|id, _| alive.contains(id));
        self.held.retain(|id| alive.contains(id));
        let size = frame.map().tile_size as f32;
        for entity in units
            .iter()
            .filter(|entity| !self.launched.contains(&entity.id))
        {
            if self.slots.contains_key(&entity.id) {
                continue;
            }
            if let Some(point) = line.free_slot(frame, entity.kind, self.slots.values().copied()) {
                self.slots.insert(entity.id, point);
            }
        }
        let ready = units
            .iter()
            .filter_map(|entity| {
                let slot = self.slots.get(&entity.id)?;
                (distance(entity.position, *slot) <= ARRIVAL_TILES * size).then_some(entity.id)
            })
            .take(rts_contract::MAX_UNITS_PER_COMMAND)
            .collect::<Vec<_>>();
        if ready.len() >= 5 {
            if let Ok(group) = UnitGroup::new(ready.iter().copied()) {
                if actions
                    .attack_move(&group, target.0, target.1, false)
                    .is_ok()
                {
                    for id in &ready {
                        self.launched.insert(*id);
                        self.slots.remove(id);
                        self.held.remove(id);
                    }
                }
            }
        }
        self.flow.step(frame, target, &self.launched, actions);
        for entity in units {
            let Some(slot) = self.slots.get(&entity.id) else {
                continue;
            };
            let Ok(group) = UnitGroup::new([entity.id]) else {
                continue;
            };
            if distance(entity.position, *slot) <= ARRIVAL_TILES * size {
                if !self.held.contains(&entity.id) && actions.hold_position(&group, false).is_ok() {
                    self.held.insert(entity.id);
                }
            } else if (!self.issued.contains_key(&entity.id)
                || (entity.state == AiEntityState::Idle
                    && frame
                        .tick()
                        .saturating_sub(*self.issued.get(&entity.id).unwrap_or(&0))
                        >= RETRY_TICKS))
                && actions.move_group(&group, slot.0, slot.1, false).is_ok()
            {
                self.issued.insert(entity.id, frame.tick());
                self.held.remove(&entity.id);
            }
        }
    }
}

impl StagingLine {
    fn on_route(map: &AiMap, home: Point, target: Point) -> Option<Self> {
        let path = shortest_route(map, home, target)?;
        let radius = STAGING_DISTANCE_TILES * map.tile_size as f32;
        // Walk outward from the enemy along the actual terrain route until it crosses the staging radius.
        for i in (0..path.len().saturating_sub(1)).rev() {
            let outer = path[i];
            let inner = path[i + 1];
            if distance(outer, target) < radius || distance(inner, target) > radius {
                continue;
            }
            let mut low = 0.0;
            let mut high = 1.0;
            for _ in 0..24 {
                let fraction = (low + high) * 0.5;
                let point = (
                    outer.0 + (inner.0 - outer.0) * fraction,
                    outer.1 + (inner.1 - outer.1) * fraction,
                );
                if distance(point, target) >= radius {
                    low = fraction;
                } else {
                    high = fraction;
                }
            }
            let center = (
                outer.0 + (inner.0 - outer.0) * low,
                outer.1 + (inner.1 - outer.1) * low,
            );
            // Smooth grid stair-steps so the line faces the local approach, not one arbitrary tile.
            let behind = path[i.saturating_sub(4)];
            let ahead = path[(i + 5).min(path.len() - 1)];
            let length = distance(behind, ahead);
            if length <= 0.0 {
                continue;
            }
            return Some(Self {
                center,
                forward: ((ahead.0 - behind.0) / length, (ahead.1 - behind.1) / length),
            });
        }
        None
    }

    fn free_slot(
        &self,
        frame: &AiFrame,
        kind: EntityKind,
        occupied: impl Iterator<Item = Point>,
    ) -> Option<Point> {
        let occupied = occupied.collect::<Vec<_>>();
        let size = frame.map().tile_size as f32;
        let lateral = (-self.forward.1, self.forward.0);
        // Two fixed parallel lines: front infantry at zero depth, Panzerfausts three tiles back.
        // Skip blocked slots rather than putting front infantry into the rear formation.
        let back = if kind == EntityKind::Panzerfaust {
            PANZERFAUST_REAR_OFFSET_TILES * size
        } else {
            0.0
        };
        for column in 0..129 {
            let signed = if column % 2 == 0 {
                column / 2
            } else {
                -(column + 1) / 2
            };
            let across = signed as f32 * SLOT_SPACING_TILES * size;
            let point = (
                self.center.0 + lateral.0 * across - self.forward.0 * back,
                self.center.1 + lateral.1 * across - self.forward.1 * back,
            );
            if occupied
                .iter()
                .any(|other| distance(*other, point) < SLOT_SPACING_TILES * size * 0.9)
            {
                continue;
            }
            let steps = (distance(self.center, point) / (size * 0.5))
                .ceil()
                .max(1.0) as usize;
            let connected = (0..=steps).all(|step| {
                let t = step as f32 / steps as f32;
                passable_world(
                    frame.map(),
                    (
                        self.center.0 + (point.0 - self.center.0) * t,
                        self.center.1 + (point.1 - self.center.1) * t,
                    ),
                )
            });
            if connected && clear_slot(frame, point) {
                return Some(point);
            }
        }
        None
    }
}

fn clear_slot(frame: &AiFrame, point: Point) -> bool {
    let map = frame.map();
    let radius = map.tile_size as f32 * 0.45;
    for (dx, dy) in [
        (0.0, 0.0),
        (-radius, 0.0),
        (radius, 0.0),
        (0.0, -radius),
        (0.0, radius),
    ] {
        if !passable_world(map, (point.0 + dx, point.1 + dy)) {
            return false;
        }
    }
    !frame
        .owned()
        .iter()
        .chain(frame.visible_allies())
        .chain(frame.visible_enemies())
        .any(|entity| {
            let Some(stats) = rts_rules::balance::building_stats(entity.kind) else {
                return false;
            };
            (point.0 - entity.position.0).abs()
                < stats.foot_w as f32 * map.tile_size as f32 / 2.0 + radius
                && (point.1 - entity.position.1).abs()
                    < stats.foot_h as f32 * map.tile_size as f32 / 2.0 + radius
        })
}

fn passable_world(map: &AiMap, point: Point) -> bool {
    if point.0 < 0.0 || point.1 < 0.0 {
        return false;
    }
    let size = map.tile_size as f32;
    passable(map, (point.0 / size) as i32, (point.1 / size) as i32)
}

pub(super) fn passable(map: &AiMap, x: i32, y: i32) -> bool {
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return false;
    };
    if x >= map.width || y >= map.height {
        return false;
    }
    let index = y as usize * map.width as usize + x as usize;
    map.terrain.get(index).is_some_and(|terrain| {
        !matches!(
            terrain,
            AiTerrain::Rock | AiTerrain::Water | AiTerrain::Unknown(_)
        )
    })
}

/// Shortest octile terrain route, with deterministic ties and no diagonal corner cutting.
/// Uses only the public terrain map, never hidden occupancy or private simulation pathing.
fn shortest_route(map: &AiMap, home: Point, target: Point) -> Option<Vec<Point>> {
    let size = map.tile_size as f32;
    if size <= 0.0 {
        return None;
    }
    let start = ((home.0 / size) as i32, (home.1 / size) as i32);
    let goal = ((target.0 / size) as i32, (target.1 / size) as i32);
    if !passable(map, start.0, start.1) || !passable(map, goal.0, goal.1) {
        return None;
    }
    let count = usize::try_from(map.width.checked_mul(map.height)?).ok()?;
    let index = |x: i32, y: i32| y as usize * map.width as usize + x as usize;
    let mut costs = vec![u32::MAX; count];
    let mut parents = vec![None; count];
    let mut open = BinaryHeap::new();
    costs[index(start.0, start.1)] = 0;
    open.push(Reverse((0_u32, start.0, start.1)));
    while let Some(Reverse((cost, x, y))) = open.pop() {
        if cost != costs[index(x, y)] {
            continue;
        }
        if (x, y) == goal {
            break;
        }
        for (dx, dy) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ] {
            let (nx, ny) = (x + dx, y + dy);
            if !passable(map, nx, ny)
                || (dx != 0 && dy != 0 && (!passable(map, x + dx, y) || !passable(map, x, y + dy)))
            {
                continue;
            }
            let next_cost = cost.saturating_add(if dx == 0 || dy == 0 { 10 } else { 14 });
            let next = index(nx, ny);
            if next_cost < costs[next] {
                costs[next] = next_cost;
                parents[next] = Some((x, y));
                open.push(Reverse((next_cost, nx, ny)));
            }
        }
    }
    if costs[index(goal.0, goal.1)] == u32::MAX {
        return None;
    }
    let mut cursor = goal;
    let mut path = vec![(
        (cursor.0 as f32 + 0.5) * size,
        (cursor.1 as f32 + 0.5) * size,
    )];
    while cursor != start {
        cursor = parents[index(cursor.0, cursor.1)]?;
        path.push((
            (cursor.0 as f32 + 0.5) * size,
            (cursor.1 as f32 + 0.5) * size,
        ));
    }
    path.reverse();
    Some(path)
}

fn distance(a: Point, b: Point) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[cfg(test)]
mod tests;
