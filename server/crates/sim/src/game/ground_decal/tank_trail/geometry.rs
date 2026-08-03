use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::game::map::Map;

use super::{FinalizedTankTrail, TankTrailPose};

const TRACK_HALF_LENGTH_PX: f32 = 25.0;
const TRACK_HALF_WIDTH_PX: f32 = 16.0;
const TURN_CONTACT_RADIUS_PX: f32 = 29.0;

pub(super) fn contact_motion(a: TankTrailPose, b: TankTrailPose) -> f32 {
    let travel = (b.x() - a.x()).hypot(b.y() - a.y());
    travel + shortest_angle_delta(a.heading(), b.heading()).abs() * TURN_CONTACT_RADIUS_PX
}

pub(super) fn shortest_angle_delta(from: f32, to: f32) -> f32 {
    let delta = to - from;
    delta.sin().atan2(delta.cos())
}

fn world_pose_bounds(x: f32, y: f32, heading: f32) -> (f32, f32, f32, f32) {
    let cos = heading.cos().abs();
    let sin = heading.sin().abs();
    let extent_x = TRACK_HALF_LENGTH_PX * cos + TRACK_HALF_WIDTH_PX * sin;
    let extent_y = TRACK_HALF_LENGTH_PX * sin + TRACK_HALF_WIDTH_PX * cos;
    (x - extent_x, y - extent_y, x + extent_x, y + extent_y)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct TrailBounds {
    pub(super) min_x_quarter_px: i32,
    pub(super) min_y_quarter_px: i32,
    pub(super) max_x_quarter_px: i32,
    pub(super) max_y_quarter_px: i32,
}

impl TrailBounds {
    pub(super) fn from_poses(poses: &[TankTrailPose]) -> Option<Self> {
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
            min_x_quarter_px: (min_x * 4.0).floor() as i32,
            min_y_quarter_px: (min_y * 4.0).floor() as i32,
            max_x_quarter_px: (max_x * 4.0).ceil() as i32,
            max_y_quarter_px: (max_y * 4.0).ceil() as i32,
        })
    }

    pub(super) fn tile_range(self, map: &Map) -> Option<(u32, u32, u32, u32)> {
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

#[derive(Debug, Clone, Default)]
pub(super) struct TankTrailSpatialIndex {
    pub(super) by_tile: BTreeMap<(u32, u32), Vec<u32>>,
    pub(super) indexed_len: usize,
}

impl TankTrailSpatialIndex {
    pub(super) fn ensure(&mut self, trails: &[FinalizedTankTrail], map: &Map) {
        if self.indexed_len == trails.len() {
            return;
        }
        self.by_tile.clear();
        for trail in trails {
            self.add(trail, map);
        }
        self.indexed_len = trails.len();
    }

    pub(super) fn add(&mut self, trail: &FinalizedTankTrail, map: &Map) {
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
