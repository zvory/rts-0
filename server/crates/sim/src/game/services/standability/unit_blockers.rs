//! Tick-local, incrementally updated body index. Enemy contact blocks motion, never transfers it.
use super::*;
use crate::game::teams::TeamRelations;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct Entry {
    owner: u32,
    body: UnitBody,
}

pub(crate) struct UnitBlockers {
    entries: BTreeMap<u32, Entry>,
    cells: BTreeMap<(i32, i32), BTreeSet<u32>>,
}

impl UnitBlockers {
    pub(crate) fn new(entities: &EntityStore) -> Self {
        let mut index = Self {
            entries: BTreeMap::new(),
            cells: BTreeMap::new(),
        };
        for e in entities.iter() {
            index.update(e);
        }
        index
    }

    pub(crate) fn update(&mut self, entity: &Entity) {
        if let Some(old) = self.entries.remove(&entity.id) {
            for cell in cells(old.body) {
                if let Some(ids) = self.cells.get_mut(&cell) {
                    ids.remove(&entity.id);
                }
            }
        }
        if entity.hp == 0 || !entity.is_unit() {
            return;
        }
        let Some(body) = unit_body_for_entity(entity) else {
            return;
        };
        for cell in cells(body) {
            self.cells.entry(cell).or_default().insert(entity.id);
        }
        self.entries.insert(
            entity.id,
            Entry {
                owner: entity.owner,
                body,
            },
        );
    }

    /// Placement rejects all occupied bodies, including friends and working workers.
    pub(crate) fn placement_clear(&self, entity: &Entity, pos: (f32, f32)) -> bool {
        let Some(body) = body_at(entity, pos, entity.facing(), entity.radius()) else {
            return false;
        };
        self.neighbors(body).into_iter().all(|id| {
            id == entity.id
                || self
                    .entries
                    .get(&id)
                    .is_none_or(|other| !unit_bodies_intersect(body, other.body))
        })
    }

    pub(crate) fn pose_clear(
        &self,
        entity: &Entity,
        pos: (f32, f32),
        facing: f32,
        radius: f32,
        teams: &TeamRelations,
    ) -> bool {
        let Some(body) = body_at(entity, pos, facing, radius) else {
            return false;
        };
        self.neighbors(body).into_iter().all(|id| {
            id == entity.id
                || self.entries.get(&id).is_none_or(|other| {
                    teams.same_team_or_same_owner(entity.owner, other.owner)
                        || !unit_bodies_intersect(body, other.body)
                })
        })
    }

    /// Swept checks include rotation and stored-coordinate rounding. Conservative expansion
    /// covers the gaps between samples, so even a fast push cannot tunnel through a body.
    /// Pre-existing authored overlaps may escape, but may never deepen.
    pub(crate) fn motion_clear(
        &self,
        entity: &Entity,
        to: (f32, f32),
        facing: f32,
        teams: &TeamRelations,
    ) -> bool {
        let from = (entity.pos_x(), entity.pos_y());
        let Some(start) = unit_body_for_entity(entity) else {
            return true;
        };
        let mut delta = (facing - entity.facing() + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        if matches!(start, UnitBody::Circle(_)) {
            delta = 0.0;
        }
        let travel =
            (to.0 - from.0).det_hypot(to.1 - from.1) + delta.abs() * start.bounding_radius();
        if !travel.is_finite() {
            return false;
        }
        let steps = (travel / 0.5).ceil().max(1.0) as u32;
        let margin = if travel > 0.0 {
            travel / steps as f32 * 0.5 + 0.001
        } else {
            0.0
        };
        let bounds = start.aabb();
        let end_body = body_at(entity, to, facing, entity.radius());
        let Some(end_body) = end_body else {
            return false;
        };
        let end = end_body.aabb();
        let mut ids = BTreeSet::new();
        // A bounding circle includes the complete rotational sweep.
        let r = start.bounding_radius() + margin;
        for tx in tile((from.0.min(to.0) - r).min(bounds.min_x))
            ..=tile((from.0.max(to.0) + r).max(end.max_x))
        {
            for ty in tile(from.1.min(to.1) - r)..=tile(from.1.max(to.1) + r) {
                if let Some(cell) = self.cells.get(&(tx, ty)) {
                    ids.extend(cell);
                }
            }
        }
        for id in ids {
            let Some(other) = self.entries.get(&id) else {
                continue;
            };
            if id == entity.id || teams.same_team_or_same_owner(entity.owner, other.owner) {
                continue;
            }
            let initial = unit_body_overlap(start, other.body).map_or(0.0, |o| o.depth);
            // Already overlapping scenes must be able to separate. For legal scenes, a
            // midpoint's expanded body bounds every pose in its sample interval.
            for step in 0..steps {
                let t = (step as f32 + 0.5) / steps as f32;
                let pos = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                let Some(body) = body_at(entity, pos, entity.facing() + delta * t, entity.radius())
                else {
                    return false;
                };
                let body = if initial <= 0.001 {
                    expanded(body, margin)
                } else {
                    body
                };
                if unit_body_overlap(body, other.body).is_some_and(|o| o.depth > initial + 0.001) {
                    return false;
                }
            }
            if unit_body_overlap(end_body, other.body).is_some_and(|o| o.depth > initial + 0.001) {
                return false;
            }
        }
        true
    }

    fn neighbors(&self, body: UnitBody) -> BTreeSet<u32> {
        let mut ids = BTreeSet::new();
        for cell in cells(body) {
            if let Some(found) = self.cells.get(&cell) {
                ids.extend(found);
            }
        }
        ids
    }
}

fn tile(x: f32) -> i32 {
    (x / config::TILE_SIZE as f32).floor() as i32
}
fn cells(body: UnitBody) -> impl Iterator<Item = (i32, i32)> {
    let b = body.aabb();
    (tile(b.min_x)..=tile(b.max_x))
        .flat_map(move |x| (tile(b.min_y)..=tile(b.max_y)).map(move |y| (x, y)))
}

pub(crate) fn body_at(
    entity: &Entity,
    pos: (f32, f32),
    facing: f32,
    radius: f32,
) -> Option<UnitBody> {
    let pos = stored_position(pos.0, pos.1)?;
    let mut body = unit_body_with_facing(entity.kind, pos.0, pos.1, facing)?;
    if entity.kind.is_disciple() {
        if let UnitBody::Circle(circle) = &mut body {
            circle.radius = radius;
        }
    }
    Some(body)
}

fn expanded(mut body: UnitBody, margin: f32) -> UnitBody {
    match &mut body {
        UnitBody::Circle(b) => b.radius += margin,
        UnitBody::OrientedCapsule(b) => b.radius += margin,
        UnitBody::OrientedBox(b) => {
            b.half_len += margin;
            b.half_width += margin;
        }
    }
    body
}
