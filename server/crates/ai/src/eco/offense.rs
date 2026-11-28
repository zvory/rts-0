//! Capacity-triggered Pressure-style cohorts, clearing public base sites from outside inward.
use super::tactics::TacticalOwnership;
use crate::pressure::{ground_reachable, StagingLine};
use crate::sdk::{
    AiActions, AiBaseSiteId, AiEntity, AiEntityState, AiFrame, AiRulebook, EntityKind, UnitGroup,
    WorldQueries,
};
use rts_rules::math::GameplayMath as _;
use std::collections::{BTreeMap, BTreeSet};
type Point = (f32, f32);
#[derive(Default)]
pub(super) struct EcoOffense {
    activated: bool,
    has_launched: bool,
    targets: Vec<AiBaseSiteId>,
    index: usize,
    line: Option<StagingLine>,
    reachable: Vec<bool>,
    slots: BTreeMap<u32, Point>,
    cohort: BTreeSet<u32>,
    launched: BTreeSet<u32>,
    issued: BTreeMap<u32, u32>,
    held: BTreeSet<u32>,
    failed: BTreeMap<u32, u32>,
}
pub(super) fn combat(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman
            | EntityKind::MachineGunner
            | EntityKind::Panzerfaust
            | EntityKind::Tank
    )
}
fn distance(a: Point, b: Point) -> f32 {
    ((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)).det_sqrt()
}
fn value(book: AiRulebook, e: &AiEntity) -> f64 {
    book.cost(e.kind)
        .map_or(0., |c| f64::from(c.steel.saturating_add(c.oil)))
        * f64::from(e.health.current)
        / f64::from(e.health.maximum.max(1))
}
impl EcoOffense {
    pub(super) fn observe(&mut self, frame: &AiFrame) {
        if self.activated || frame.economy().supply_used < 300 {
            return;
        }
        let Some(enemy) = frame.players().iter().find(|p| {
            p.is_alive
                && p.id != frame.player_id()
                && (p.team_id == 0 || p.team_id != frame.team_id())
        }) else {
            return;
        };
        let q = WorldQueries::new(frame);
        self.targets = (1..=frame.base_sites().len().saturating_div(2).max(1))
            .rev()
            .filter_map(|rank| {
                q.base(enemy.id, rank)
                    .filter(|b| b.distance_from_main.is_some())
                    .map(|b| b.site.id)
            })
            .collect();
        self.activated = !self.targets.is_empty();
    }
    pub(super) fn controlled(&self, frame: &AiFrame, screen: &BTreeSet<u32>) -> BTreeSet<u32> {
        if !self.activated || self.index >= self.targets.len() {
            return BTreeSet::new();
        }
        frame
            .owned()
            .iter()
            .filter(|e| combat(e.kind) && e.health.current > 0 && !screen.contains(&e.id))
            .map(|e| e.id)
            .collect()
    }
    pub(super) fn attacking_ids(&self) -> &BTreeSet<u32> {
        &self.launched
    }
    fn target(&self, frame: &AiFrame) -> Option<Point> {
        let id = self.targets.get(self.index)?;
        frame
            .base_sites()
            .iter()
            .find(|b| b.id == *id)
            .map(|b| b.position)
    }
    fn reset_stage(&mut self) {
        self.has_launched = false;
        self.line = None;
        self.slots.clear();
        self.cohort.clear();
        self.launched.clear();
        self.issued.clear();
        self.held.clear();
        self.failed.clear();
    }
    fn clear(&self, frame: &AiFrame, target: Point) -> bool {
        let Some(id) = self.targets.get(self.index) else {
            return false;
        };
        let q = WorldQueries::new(frame);
        let size = frame.map().tile_size as f32;
        if q.known_base_depots(*id)
            .iter()
            .any(|d| d.owner != frame.player_id())
        {
            return false;
        }
        if frame.visible_enemies().iter().any(|e| {
            e.health.current > 0
                && (distance(e.position, target) <= 12. * size
                    || e.target.is_some_and(|id| {
                        frame.owned().iter().any(|u| {
                            u.id == id
                                && combat(u.kind)
                                && distance(u.position, target) <= 12. * size
                        })
                    }))
        }) {
            return false;
        }
        let Some(site) = frame.base_sites().iter().find(|b| b.id == *id) else {
            return false;
        };
        std::iter::once(target)
            .chain(
                frame
                    .resources()
                    .iter()
                    .filter(|r| site.resource_ids.contains(&r.id))
                    .map(|r| r.position),
            )
            .all(|p| {
                q.world_point(p.0, p.1)
                    .is_some_and(|p| q.ground_position_is_observable(p))
            })
            && frame
                .owned()
                .iter()
                .any(|e| self.launched.contains(&e.id) && distance(e.position, target) <= 6. * size)
    }
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        controlled: &BTreeSet<u32>,
        tactical: &TacticalOwnership,
    ) {
        if !self.activated {
            return;
        }
        let q = WorldQueries::new(frame);
        // Friendly occupation is public evidence, not an invented enemy base to attack.
        while self.targets.get(self.index).is_some_and(|site| {
            q.known_base_depots(*site)
                .iter()
                .any(|d| d.owner == frame.player_id())
        }) {
            self.index += 1;
            self.reset_stage();
        }
        let Some(mut target) = self.target(frame) else {
            return;
        };
        if !self.launched.is_empty() && self.clear(frame, target) {
            self.index += 1;
            self.reset_stage();
            let Some(next) = self.target(frame) else {
                return;
            };
            target = next;
        }
        let units = frame
            .owned()
            .iter()
            .filter(|e| controlled.contains(&e.id) && e.health.current > 0)
            .collect::<Vec<_>>();
        let alive = units.iter().map(|e| e.id).collect::<BTreeSet<_>>();
        self.launched.retain(|id| alive.contains(id));
        self.cohort.retain(|id| alive.contains(id));
        self.slots
            .retain(|id, _| alive.contains(id) && !self.launched.contains(id));
        self.issued.retain(|id, _| alive.contains(id));
        self.held.retain(|id| alive.contains(id));
        self.failed.retain(|id, _| alive.contains(id));
        for id in &tactical.released {
            self.issued.remove(id);
            self.held.remove(id);
        }
        if self.line.is_none() {
            let Some(home) = frame.players().iter().find(|p| p.id == frame.player_id()) else {
                return;
            };
            let size = frame.map().tile_size as f32;
            let home = (
                (home.start_tile.0 as f32 + 0.5) * size,
                (home.start_tile.1 as f32 + 0.5) * size,
            );
            self.line = StagingLine::on_axis(frame.map(), home, target);
            self.reachable = ground_reachable(frame.map(), home);
        }
        let Some(line) = self.line else {
            return;
        };
        let mut waiting = units
            .iter()
            .filter(|e| !self.launched.contains(&e.id))
            .copied()
            .collect::<Vec<_>>();
        waiting.sort_by_key(|e| (e.kind != EntityKind::Tank, e.id));
        // Snapshot each wave: ongoing auto-production cannot postpone its departure forever.
        if self.cohort.is_empty() && (!self.has_launched || waiting.len() >= 20) {
            self.cohort.extend(waiting.iter().map(|e| e.id));
        }
        let size = frame.map().tile_size as f32;
        // Spread the full cohort over exactly three ranks, each two tiles behind the last.
        let armor_columns = waiting
            .iter()
            .filter(|e| e.kind == EntityKind::Tank)
            .count()
            .div_ceil(3)
            .max(1);
        let columns = waiting.len().div_ceil(3) + armor_columns * 2 + 4;
        for (rank_index, e) in waiting.iter().enumerate() {
            if tactical.active.contains(&e.id) {
                continue;
            }
            if !self.slots.contains_key(&e.id)
                && self
                    .failed
                    .get(&e.id)
                    .is_none_or(|t| frame.tick().saturating_sub(*t) >= 90)
            {
                let mut occupied = self.slots.values().copied().collect::<Vec<_>>();
                let mut selected = None;
                'rows: for offset in 0..3 {
                    let row = (rank_index + offset) % 3;
                    let row = line.shifted_back(row as f32 * 2. * size);
                    for _ in 0..8 {
                        let Some(p) = row.free_slot_rank(
                            frame,
                            e.kind,
                            occupied.iter().copied(),
                            &self.reachable,
                            // Existing infantry posts can occupy the original Tank columns.
                            // Search the whole formation frontage for new armor as well.
                            columns,
                        ) else {
                            break;
                        };
                        let reachable = q
                            .world_point(e.position.0, e.position.1)
                            .zip(q.world_point(p.0, p.1))
                            .and_then(|(a, b)| q.ground_route(a, b, e.kind))
                            .is_some();
                        if reachable {
                            selected = Some(p);
                            break 'rows;
                        }
                        occupied.push(p);
                    }
                }
                if let Some(p) = selected {
                    self.slots.insert(e.id, p);
                } else {
                    self.failed.insert(e.id, frame.tick());
                }
            }
        }
        let Some(book) = AiRulebook::for_frame(frame) else {
            return;
        };
        let ready = waiting
            .iter()
            .filter(|e| !tactical.active.contains(&e.id))
            .filter(|e| {
                let arrival_tiles = if e.kind == EntityKind::Tank { 2.0 } else { 0.9 };
                self.slots
                    .get(&e.id)
                    .is_some_and(|p| distance(e.position, *p) <= size * arrival_tiles)
            })
            .copied()
            .collect::<Vec<_>>();
        let required = units
            .iter()
            .filter(|e| {
                self.cohort.contains(&e.id)
                    && !tactical.active.contains(&e.id)
                    && self.slots.contains_key(&e.id)
            })
            .map(|e| value(book, e))
            .sum::<f64>();
        let assembled = ready
            .iter()
            .filter(|e| {
                self.cohort.contains(&e.id)
                    && !tactical.active.contains(&e.id)
                    && self.slots.contains_key(&e.id)
            })
            .map(|e| value(book, e))
            .sum::<f64>();
        let cohort_tanks = units
            .iter()
            .filter(|e| self.cohort.contains(&e.id) && e.kind == EntityKind::Tank)
            .count();
        let ready_tanks = ready
            .iter()
            .filter(|e| self.cohort.contains(&e.id) && e.kind == EntityKind::Tank)
            .count();
        // Armor leads the offensive: late infantry cannot hold an assembled Tank force.
        // Count the frozen cohort, including unslotted/borrowed Tanks, rather than only
        // units that were successfully assigned a post or fresh production at home.
        let wave_ready = if cohort_tanks > 0 {
            ready_tanks * 10 >= cohort_tanks * 9
        } else {
            required > 0. && assembled >= required * 0.9 && ready.len() >= 20.min(self.cohort.len())
        };
        if wave_ready {
            let ids = ready.iter().map(|e| e.id).collect::<Vec<_>>();
            let mut sent = false;
            for chunk in ids.chunks(rts_contract::MAX_UNITS_PER_COMMAND) {
                if UnitGroup::new(chunk.iter().copied())
                    .and_then(|g| actions.attack_move(&g, target.0, target.1, false))
                    .is_ok()
                {
                    sent = true;
                    self.has_launched = true;
                    for id in chunk {
                        self.launched.insert(*id);
                        self.slots.remove(id);
                        self.issued.insert(*id, frame.tick());
                        self.held.remove(id);
                    }
                }
            }
            if sent {
                self.cohort.clear();
            }
        }
        for e in units {
            if tactical.active.contains(&e.id) {
                continue;
            }
            let Ok(group) = UnitGroup::new([e.id]) else {
                continue;
            };
            if self.launched.contains(&e.id) {
                let resume = !self.issued.contains_key(&e.id)
                    || (e.state == AiEntityState::Idle
                        && self
                            .issued
                            .get(&e.id)
                            .is_none_or(|t| frame.tick().saturating_sub(*t) >= 90));
                if resume
                    && actions
                        .attack_move(&group, target.0, target.1, false)
                        .is_ok()
                {
                    self.issued.insert(e.id, frame.tick());
                }
                continue;
            }
            let Some(p) = self.slots.get(&e.id).copied() else {
                continue;
            };
            if distance(e.position, p) <= size * 0.9 {
                if !self.held.contains(&e.id) && actions.hold_position(&group, false).is_ok() {
                    self.held.insert(e.id);
                }
                continue;
            }
            if self
                .issued
                .get(&e.id)
                .is_some_and(|t| frame.tick().saturating_sub(*t) < 90)
                || (self.issued.contains_key(&e.id) && e.state != AiEntityState::Idle)
            {
                continue;
            }
            let route = q
                .world_point(e.position.0, e.position.1)
                .zip(q.world_point(p.0, p.1))
                .and_then(|(a, b)| q.ground_route(a, b, e.kind));
            let Some(route) = route else {
                self.slots.remove(&e.id);
                self.issued.remove(&e.id);
                continue;
            };
            let points = crate::sdk::ground_route_corners(
                &route.iter().map(|p| p.as_tuple()).collect::<Vec<_>>(),
            );
            let points = points.into_iter().skip(1).take(24).collect::<Vec<_>>();
            if let Some((first, rest)) = points.split_first() {
                if actions.move_route(&group, *first, rest).is_ok() {
                    self.issued.insert(e.id, frame.tick());
                    self.held.remove(&e.id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
