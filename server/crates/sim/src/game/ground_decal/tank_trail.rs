use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::game::entity::{EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::protocol::TankTrailView;

mod checkpoint;
mod geometry;

use geometry::{TankTrailSpatialIndex, TrailBounds};

const POSITION_QUANTUM_PX: f32 = 4.0;
const HEADING_SCALE: f32 = i8::MAX as f32 / std::f32::consts::PI;
const HEADING_WIRE_SCALE: f32 = i16::MAX as f32 / std::f32::consts::PI;
const SAMPLE_TRAVEL_PX: f32 = 64.0;
const SAMPLE_TURN_RAD: f32 = 0.25;
const TRACK_HALF_LENGTH_PX: f32 = 25.0;
const TRACK_HALF_WIDTH_PX: f32 = 16.0;
const TURN_CONTACT_RADIUS_PX: f32 = 29.0;
const MAX_ACTIVE_POSES: usize = 8;
const MAX_CENTER_SPAN_PX: f32 = 512.0;
const MAX_FINALIZED_TRAILS: usize = 4_096;
const STOP_SETTLE_TICKS: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
struct TankTrailPose((u16, u16, i8));

impl TankTrailPose {
    fn from_world(x: f32, y: f32, facing: f32, map: &Map) -> Option<Self> {
        if !x.is_finite()
            || !y.is_finite()
            || !facing.is_finite()
            || !map.contains_world_point(x, y)
        {
            return None;
        }
        let x_quantized = (x / POSITION_QUANTUM_PX).round();
        let y_quantized = (y / POSITION_QUANTUM_PX).round();
        if x_quantized < 0.0
            || x_quantized > u16::MAX as f32
            || y_quantized < 0.0
            || y_quantized > u16::MAX as f32
        {
            return None;
        }
        let heading = normalize_angle(facing);
        Some(Self((
            x_quantized as u16,
            y_quantized as u16,
            (heading * HEADING_SCALE)
                .round()
                .clamp(i8::MIN as f32, i8::MAX as f32) as i8,
        )))
    }

    fn x(self) -> f32 {
        self.0 .0 as f32 * POSITION_QUANTUM_PX
    }

    fn y(self) -> f32 {
        self.0 .1 as f32 * POSITION_QUANTUM_PX
    }

    fn heading(self) -> f32 {
        self.0 .2 as f32 / HEADING_SCALE
    }

    fn wire(self) -> [i32; 3] {
        [
            i32::from(self.0 .0) * 16,
            i32::from(self.0 .1) * 16,
            (self.heading() * HEADING_WIRE_SCALE).round() as i32,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalizedTankTrail {
    id: u32,
    owner: u32,
    poses: Vec<TankTrailPose>,
    bounds: TrailBounds,
    created_revision: u32,
}

impl FinalizedTankTrail {
    fn to_view(&self) -> TankTrailView {
        TankTrailView {
            id: self.id,
            poses: self
                .poses
                .iter()
                .copied()
                .map(TankTrailPose::wire)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActiveTankTrail {
    owner: u32,
    poses: Vec<TankTrailPose>,
    last_observed: TankTrailPose,
    last_motion_tick: u32,
}

#[derive(Debug, Clone)]
pub(super) struct PendingTankTrail {
    owner: u32,
    poses: Vec<TankTrailPose>,
    bounds: TrailBounds,
}

#[derive(Debug, Clone)]
pub(super) struct TankTrailStore {
    next_id: u32,
    finalized: Vec<FinalizedTankTrail>,
    active_by_tank: BTreeMap<u32, ActiveTankTrail>,
    spatial_index: TankTrailSpatialIndex,
}

impl Default for TankTrailStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TankTrailStore {
    pub(super) fn new() -> Self {
        Self {
            next_id: 1,
            finalized: Vec::new(),
            active_by_tank: BTreeMap::new(),
            spatial_index: TankTrailSpatialIndex::default(),
        }
    }

    pub(super) fn update(
        &mut self,
        entities: &EntityStore,
        map: &Map,
        tick: u32,
    ) -> Vec<PendingTankTrail> {
        if self.finalized.len() >= MAX_FINALIZED_TRAILS {
            self.active_by_tank.clear();
            return Vec::new();
        }
        let observations = entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Tank && entity.hp > 0)
            .filter_map(|entity| {
                TankTrailPose::from_world(entity.pos_x, entity.pos_y, entity.facing(), map)
                    .map(|pose| (entity.id, entity.owner, pose))
            })
            .collect::<Vec<_>>();
        let alive = observations
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<BTreeSet<_>>();
        let mut pending = Vec::new();
        for (tank_id, owner, pose) in observations {
            let Some(mut active) = self.active_by_tank.remove(&tank_id) else {
                self.active_by_tank.insert(
                    tank_id,
                    ActiveTankTrail {
                        owner,
                        poses: vec![pose],
                        last_observed: pose,
                        last_motion_tick: tick,
                    },
                );
                continue;
            };
            if contact_motion(active.last_observed, pose) > 0.05 {
                active.last_motion_tick = tick;
            }
            active.last_observed = pose;
            if active
                .poses
                .last()
                .is_some_and(|last| sample_needed(*last, pose))
            {
                if active.poses.len() >= MAX_ACTIVE_POSES
                    || center_span_with(&active.poses, pose) > MAX_CENTER_SPAN_PX
                {
                    let continuity = active.poses.last().copied();
                    if let Some(chunk) = Self::pending(active.owner, active.poses) {
                        pending.push(chunk);
                    }
                    active.poses = continuity
                        .filter(|last| center_span(&[*last, pose]) <= MAX_CENTER_SPAN_PX)
                        .map_or_else(|| vec![pose], |last| vec![last, pose]);
                } else {
                    active.poses.push(pose);
                }
            }
            if tick.saturating_sub(active.last_motion_tick) >= STOP_SETTLE_TICKS {
                pending.extend(Self::finish(active));
            } else {
                self.active_by_tank.insert(tank_id, active);
            }
        }
        let vanished = self
            .active_by_tank
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect::<Vec<_>>();
        for id in vanished {
            if let Some(active) = self.active_by_tank.remove(&id) {
                pending.extend(Self::finish(active));
            }
        }
        pending
    }

    fn finish(mut active: ActiveTankTrail) -> Vec<PendingTankTrail> {
        let mut pending = Vec::new();
        if active.poses.last().copied() != Some(active.last_observed) {
            if active.poses.len() < MAX_ACTIVE_POSES
                && center_span_with(&active.poses, active.last_observed) <= MAX_CENTER_SPAN_PX
            {
                active.poses.push(active.last_observed);
            } else {
                let continuity = active.poses.last().copied();
                if let Some(chunk) = Self::pending(active.owner, active.poses) {
                    pending.push(chunk);
                }
                active.poses = continuity
                    .filter(|last| {
                        center_span(&[ *last, active.last_observed]) <= MAX_CENTER_SPAN_PX
                    })
                    .map_or_else(
                        || vec![active.last_observed],
                        |last| vec![last, active.last_observed],
                    );
            }
        }
        if let Some(chunk) = Self::pending(active.owner, active.poses) {
            pending.push(chunk);
        }
        pending
    }

    fn pending(owner: u32, poses: Vec<TankTrailPose>) -> Option<PendingTankTrail> {
        if poses.len() < 2 {
            return None;
        }
        Some(PendingTankTrail {
            owner,
            bounds: TrailBounds::from_poses(&poses)?,
            poses,
        })
    }

    pub(super) fn commit(
        &mut self,
        pending: PendingTankTrail,
        id: u32,
        created_revision: u32,
        map: &Map,
    ) -> bool {
        if id == 0 || id != self.next_id || self.finalized.len() >= MAX_FINALIZED_TRAILS {
            return false;
        }
        let Some(next_id) = self.next_id.checked_add(1) else {
            return false;
        };
        let trail = FinalizedTankTrail {
            id,
            owner: pending.owner,
            poses: pending.poses,
            bounds: pending.bounds,
            created_revision,
        };
        self.spatial_index.ensure(&self.finalized, map);
        self.spatial_index.add(&trail, map);
        self.finalized.push(trail);
        self.spatial_index.indexed_len = self.finalized.len();
        self.next_id = next_id;
        true
    }

    pub(super) fn next_id(&self) -> Option<u32> {
        if self.finalized.len() >= MAX_FINALIZED_TRAILS {
            return None;
        }
        self.next_id.checked_add(1).map(|_| self.next_id)
    }

    fn trail(&self, id: u32) -> Option<&FinalizedTankTrail> {
        let index = usize::try_from(id.checked_sub(1)?).ok()?;
        self.finalized.get(index).filter(|trail| trail.id == id)
    }

    pub(super) fn contains(&self, id: u32) -> bool {
        self.trail(id).is_some()
    }

    pub(super) fn view(&self, id: u32) -> Option<TankTrailView> {
        self.trail(id).map(FinalizedTankTrail::to_view)
    }

    pub(super) fn created_revision(&self, id: u32) -> Option<u32> {
        self.trail(id).map(|trail| trail.created_revision)
    }

    pub(super) fn owner(&self, id: u32) -> Option<u32> {
        self.trail(id).map(|trail| trail.owner)
    }

    pub(super) fn owned_created_revisions<'a>(
        &'a self,
        players: &'a BTreeSet<u32>,
    ) -> impl Iterator<Item = u32> + 'a {
        self.finalized
            .iter()
            .filter(|trail| players.contains(&trail.owner))
            .map(|trail| trail.created_revision)
    }

    pub(super) fn owned_views_after(
        &self,
        players: &BTreeSet<u32>,
        after_revision: u32,
    ) -> Vec<TankTrailView> {
        self.finalized
            .iter()
            .filter(|trail| {
                players.contains(&trail.owner) && trail.created_revision > after_revision
            })
            .map(FinalizedTankTrail::to_view)
            .collect()
    }

    pub(super) fn created_revisions(&self) -> impl Iterator<Item = u32> + '_ {
        self.finalized.iter().map(|trail| trail.created_revision)
    }

    pub(super) fn full_world_views_after(&self, after_revision: u32) -> Vec<TankTrailView> {
        self.finalized
            .iter()
            .filter(|trail| trail.created_revision > after_revision)
            .map(FinalizedTankTrail::to_view)
            .collect()
    }

    pub(super) fn newly_fully_visible(
        &mut self,
        player: u32,
        fog: &Fog,
        map: &Map,
        known: &BTreeMap<u32, u32>,
    ) -> Vec<u32> {
        self.spatial_index.ensure(&self.finalized, map);
        let mut candidates = BTreeSet::new();
        if let Some((width, visible)) = fog.visible_grid_for(player) {
            for (index, is_visible) in visible.iter().copied().enumerate() {
                if !is_visible {
                    continue;
                }
                let Ok(index) = u32::try_from(index) else {
                    continue;
                };
                let key = (index % width, index / width);
                if let Some(ids) = self.spatial_index.by_tile.get(&key) {
                    candidates.extend(ids.iter().copied());
                }
            }
        }
        candidates
            .into_iter()
            .filter(|id| !known.contains_key(id))
            .filter(|id| self.owner(*id) != Some(player))
            .filter(|id| {
                self.trail(*id)
                    .is_some_and(|trail| trail_fully_visible(trail, player, fog, map))
            })
            .collect()
    }

    pub(super) fn valid_checkpoint_state(
        &self,
        map: &Map,
        player_ids: &BTreeSet<u32>,
        tick: u32,
    ) -> bool {
        if self.next_id == 0 || self.finalized.len() > MAX_FINALIZED_TRAILS {
            return false;
        }
        for (index, trail) in self.finalized.iter().enumerate() {
            let expected_id = u32::try_from(index).ok().and_then(|i| i.checked_add(1));
            if Some(trail.id) != expected_id
                || trail.created_revision == 0
                || trail.owner == 0
                || !player_ids.contains(&trail.owner)
                || trail.poses.len() < 2
                || trail.poses.len() > MAX_ACTIVE_POSES
                || TrailBounds::from_poses(&trail.poses) != Some(trail.bounds)
                || center_span(&trail.poses) > MAX_CENTER_SPAN_PX + SAMPLE_TRAVEL_PX
                || !trail.poses.iter().all(|pose| pose_valid(*pose, map))
            {
                return false;
            }
        }
        let expected_next = self
            .finalized
            .last()
            .map_or(Some(1), |trail| trail.id.checked_add(1));
        if expected_next != Some(self.next_id) {
            return false;
        }
        self.active_by_tank.iter().all(|(tank_id, active)| {
            *tank_id != 0
                && active.owner != 0
                && player_ids.contains(&active.owner)
                && !active.poses.is_empty()
                && active.poses.len() <= MAX_ACTIVE_POSES
                && active.last_motion_tick <= tick
                && pose_valid(active.last_observed, map)
                && active.poses.iter().all(|pose| pose_valid(*pose, map))
                && center_span(&active.poses) <= MAX_CENTER_SPAN_PX
        })
    }

    #[cfg(test)]
    pub(super) fn finalized_len(&self) -> usize {
        self.finalized.len()
    }
}

fn pose_valid(pose: TankTrailPose, map: &Map) -> bool {
    pose.0 .2 != i8::MIN && map.contains_world_point(pose.x(), pose.y())
}

fn trail_fully_visible(trail: &FinalizedTankTrail, player: u32, fog: &Fog, map: &Map) -> bool {
    let Some((min_tx, min_ty, max_tx, max_ty)) = trail.bounds.tile_range(map) else {
        return false;
    };
    (min_ty..=max_ty).all(|ty| (min_tx..=max_tx).all(|tx| fog.is_visible(player, tx, ty)))
}

fn contact_motion(a: TankTrailPose, b: TankTrailPose) -> f32 {
    let travel = (b.x() - a.x()).hypot(b.y() - a.y());
    travel + shortest_angle_delta(a.heading(), b.heading()).abs() * TURN_CONTACT_RADIUS_PX
}

fn sample_needed(a: TankTrailPose, b: TankTrailPose) -> bool {
    (b.x() - a.x()).hypot(b.y() - a.y()) >= SAMPLE_TRAVEL_PX
        || shortest_angle_delta(a.heading(), b.heading()).abs() >= SAMPLE_TURN_RAD
}

fn center_span_with(poses: &[TankTrailPose], candidate: TankTrailPose) -> f32 {
    let mut points = poses.to_vec();
    points.push(candidate);
    center_span(&points)
}

fn center_span(poses: &[TankTrailPose]) -> f32 {
    let Some(first) = poses.first().copied() else {
        return 0.0;
    };
    let mut min_x = first.x();
    let mut min_y = first.y();
    let mut max_x = min_x;
    let mut max_y = min_y;
    for pose in poses {
        min_x = min_x.min(pose.x());
        min_y = min_y.min(pose.y());
        max_x = max_x.max(pose.x());
        max_y = max_y.max(pose.y());
    }
    (max_x - min_x).max(max_y - min_y)
}

fn world_pose_bounds(x: f32, y: f32, heading: f32) -> (f32, f32, f32, f32) {
    let cos = heading.cos().abs();
    let sin = heading.sin().abs();
    let extent_x = TRACK_HALF_LENGTH_PX * cos + TRACK_HALF_WIDTH_PX * sin;
    let extent_y = TRACK_HALF_LENGTH_PX * sin + TRACK_HALF_WIDTH_PX * cos;
    (x - extent_x, y - extent_y, x + extent_x, y + extent_y)
}

fn normalize_angle(angle: f32) -> f32 {
    angle.sin().atan2(angle.cos())
}

fn shortest_angle_delta(from: f32, to: f32) -> f32 {
    normalize_angle(to - from)
}

#[cfg(test)]
mod tests;
