use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::protocol::TankTrailView;

const QUARTER_PIXELS: f32 = 4.0;
const HEADING_SCALE: f32 = i16::MAX as f32 / std::f32::consts::PI;
const SAMPLE_TRAVEL_PX: f32 = 24.0;
const SAMPLE_TURN_RAD: f32 = 0.1;
const TRACK_HALF_LENGTH_PX: f32 = 25.0;
const TRACK_HALF_WIDTH_PX: f32 = 16.0;
const TURN_CONTACT_RADIUS_PX: f32 = 29.0;
const MAX_ACTIVE_POSES: usize = 8;
const MAX_CENTER_SPAN_PX: f32 = 192.0;
const STOP_SETTLE_TICKS: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
struct TankTrailPose((u16, u16, i16));

impl TankTrailPose {
    fn from_world(x: f32, y: f32, facing: f32, map: &Map) -> Option<Self> {
        if !x.is_finite()
            || !y.is_finite()
            || !facing.is_finite()
            || !map.contains_world_point(x, y)
        {
            return None;
        }
        let x_quarter_px = (x * QUARTER_PIXELS).round();
        let y_quarter_px = (y * QUARTER_PIXELS).round();
        if x_quarter_px < 0.0
            || x_quarter_px > u16::MAX as f32
            || y_quarter_px < 0.0
            || y_quarter_px > u16::MAX as f32
        {
            return None;
        }
        let heading = normalize_angle(facing);
        Some(Self((
            x_quarter_px as u16,
            y_quarter_px as u16,
            (heading * HEADING_SCALE)
                .round()
                .clamp(i16::MIN as f32, i16::MAX as f32) as i16,
        )))
    }

    fn x(self) -> f32 {
        self.0 .0 as f32 / QUARTER_PIXELS
    }

    fn y(self) -> f32 {
        self.0 .1 as f32 / QUARTER_PIXELS
    }

    fn heading(self) -> f32 {
        self.0 .2 as f32 / HEADING_SCALE
    }

    fn wire(self) -> [i32; 3] {
        [
            i32::from(self.0 .0),
            i32::from(self.0 .1),
            i32::from(self.0 .2),
        ]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TrailBounds {
    min_x_quarter_px: i32,
    min_y_quarter_px: i32,
    max_x_quarter_px: i32,
    max_y_quarter_px: i32,
}

impl TrailBounds {
    fn from_poses(poses: &[TankTrailPose]) -> Option<Self> {
        let first = *poses.first()?;
        let mut min_x = first.x();
        let mut min_y = first.y();
        let mut max_x = min_x;
        let mut max_y = min_y;
        for pair in poses.windows(2) {
            let from = pair[0];
            let to = pair[1];
            let turn = shortest_angle_delta(from.heading(), to.heading());
            let steps = ((contact_motion(from, to) / 4.0).ceil() as usize).clamp(1, 64);
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let x = from.x() + (to.x() - from.x()) * t;
                let y = from.y() + (to.y() - from.y()) * t;
                let heading = from.heading() + turn * t;
                let (pose_min_x, pose_min_y, pose_max_x, pose_max_y) =
                    world_pose_bounds(x, y, heading);
                min_x = min_x.min(pose_min_x);
                min_y = min_y.min(pose_min_y);
                max_x = max_x.max(pose_max_x);
                max_y = max_y.max(pose_max_y);
            }
        }
        Some(Self {
            min_x_quarter_px: (min_x * QUARTER_PIXELS).floor() as i32,
            min_y_quarter_px: (min_y * QUARTER_PIXELS).floor() as i32,
            max_x_quarter_px: (max_x * QUARTER_PIXELS).ceil() as i32,
            max_y_quarter_px: (max_y * QUARTER_PIXELS).ceil() as i32,
        })
    }

    fn tile_range(self, map: &Map) -> Option<(u32, u32, u32, u32)> {
        let tile_size_quarter = i64::from(config::TILE_SIZE) * 4;
        let map_width = i64::from(map.width);
        let map_height = i64::from(map.height);
        if tile_size_quarter <= 0 || map_width <= 0 || map_height <= 0 {
            return None;
        }
        let min_tx = i64::from(self.min_x_quarter_px)
            .div_euclid(tile_size_quarter)
            .clamp(0, map_width - 1);
        let min_ty = i64::from(self.min_y_quarter_px)
            .div_euclid(tile_size_quarter)
            .clamp(0, map_height - 1);
        let max_tx = i64::from(self.max_x_quarter_px.saturating_sub(1))
            .div_euclid(tile_size_quarter)
            .clamp(0, map_width - 1);
        let max_ty = i64::from(self.max_y_quarter_px.saturating_sub(1))
            .div_euclid(tile_size_quarter)
            .clamp(0, map_height - 1);
        Some((
            u32::try_from(min_tx).ok()?,
            u32::try_from(min_ty).ok()?,
            u32::try_from(max_tx).ok()?,
            u32::try_from(max_ty).ok()?,
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalizedTankTrail {
    id: u32,
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
    poses: Vec<TankTrailPose>,
    bounds: TrailBounds,
}

#[derive(Debug, Clone, Default)]
struct TankTrailSpatialIndex {
    by_tile: BTreeMap<(u32, u32), Vec<u32>>,
    indexed_len: usize,
}

impl TankTrailSpatialIndex {
    fn ensure(&mut self, trails: &[FinalizedTankTrail], map: &Map) {
        if self.indexed_len == trails.len() {
            return;
        }
        self.by_tile.clear();
        for trail in trails {
            self.add(trail, map);
        }
        self.indexed_len = trails.len();
    }

    fn add(&mut self, trail: &FinalizedTankTrail, map: &Map) {
        let Some((min_tx, min_ty, max_tx, max_ty)) = trail.bounds.tile_range(map) else {
            return;
        };
        for ty in min_ty..=max_ty {
            for tx in min_tx..=max_tx {
                self.by_tile.entry((tx, ty)).or_default().push(trail.id);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct TankTrailStore {
    next_id: u32,
    finalized: Vec<FinalizedTankTrail>,
    active_by_tank: BTreeMap<u32, ActiveTankTrail>,
    #[serde(skip)]
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
                    if let Some(chunk) = Self::pending(active.poses) {
                        pending.push(chunk);
                    }
                    active.poses = continuity.map_or_else(|| vec![pose], |last| vec![last, pose]);
                } else {
                    active.poses.push(pose);
                }
            }
            if tick.saturating_sub(active.last_motion_tick) >= STOP_SETTLE_TICKS {
                if let Some(chunk) = Self::pending(active.poses) {
                    pending.push(chunk);
                }
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
                if let Some(chunk) = Self::pending(active.poses) {
                    pending.push(chunk);
                }
            }
        }
        pending
    }

    fn pending(poses: Vec<TankTrailPose>) -> Option<PendingTankTrail> {
        if poses.len() < 2 {
            return None;
        }
        Some(PendingTankTrail {
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
        if id == 0 || id != self.next_id {
            return false;
        }
        let Some(next_id) = self.next_id.checked_add(1) else {
            return false;
        };
        let trail = FinalizedTankTrail {
            id,
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
        if self.next_id == 0 || self.finalized.len() >= u32::MAX as usize {
            return false;
        }
        for (index, trail) in self.finalized.iter().enumerate() {
            let expected_id = u32::try_from(index).ok().and_then(|i| i.checked_add(1));
            if Some(trail.id) != expected_id
                || trail.created_revision == 0
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
    map.contains_world_point(pose.x(), pose.y())
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
mod tests {
    use super::*;

    #[test]
    fn pose_is_six_bytes_and_serializes_as_one_compact_tuple() {
        let pose = TankTrailPose((400, 404, -12));
        assert_eq!(std::mem::size_of::<TankTrailPose>(), 6);
        assert_eq!(serde_json::to_string(&pose).unwrap(), "[400,404,-12]");
    }

    #[test]
    fn sampling_is_sparse_but_preserves_pivots() {
        let origin = TankTrailPose((400, 400, 0));
        let short_travel = TankTrailPose((495, 400, 0));
        let full_travel = TankTrailPose((496, 400, 0));
        let small_turn = TankTrailPose((400, 400, (0.09 * HEADING_SCALE) as i16));
        let sampled_turn = TankTrailPose((400, 400, (0.11 * HEADING_SCALE) as i16));

        assert!(!sample_needed(origin, short_travel));
        assert!(sample_needed(origin, full_travel));
        assert!(!sample_needed(origin, small_turn));
        assert!(sample_needed(origin, sampled_turn));
    }

    #[test]
    fn packed_heading_preserves_an_in_place_pivot() {
        let a = TankTrailPose((400, 400, 0));
        let b = TankTrailPose((
            400,
            400,
            (std::f32::consts::FRAC_PI_2 * HEADING_SCALE).round() as i16,
        ));
        assert!(contact_motion(a, b) > 40.0);
        assert_eq!(a.wire()[..2], b.wire()[..2]);
        assert_ne!(a.wire()[2], b.wire()[2]);
    }

    #[test]
    fn finalized_chunk_bounds_include_the_oriented_track_footprint() {
        let poses = vec![TankTrailPose((400, 400, 0)), TankTrailPose((440, 400, 0))];
        let bounds = TrailBounds::from_poses(&poses).unwrap();
        assert!(bounds.min_x_quarter_px <= 300);
        assert!(bounds.max_x_quarter_px >= 540);
        assert!(bounds.min_y_quarter_px <= 336);
        assert!(bounds.max_y_quarter_px >= 464);
    }

    #[test]
    fn checkpoint_accepts_an_active_chunk_at_the_runtime_pose_limit() {
        let map = Map::generate(1, 7);
        let pose = TankTrailPose((400, 400, 0));
        let mut store = TankTrailStore::new();
        store.active_by_tank.insert(
            7,
            ActiveTankTrail {
                owner: 1,
                poses: vec![pose; MAX_ACTIVE_POSES],
                last_observed: pose,
                last_motion_tick: 3,
            },
        );
        assert!(store.valid_checkpoint_state(&map, &BTreeSet::from([1]), 3));
    }
}
