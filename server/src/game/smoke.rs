use crate::config;
use crate::game::fog::Fog;
use crate::game::map::Map;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SmokeCloud {
    pub(crate) id: u32,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) radius_tiles: f32,
    pub(crate) spawned_tick: u32,
    pub(crate) expires_tick: u32,
}

impl SmokeCloud {
    pub(crate) fn radius_px(self) -> f32 {
        self.radius_tiles * config::TILE_SIZE as f32
    }

    pub(crate) fn contains_point(self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let dx = x - self.x;
        let dy = y - self.y;
        let radius = self.radius_px();
        dx * dx + dy * dy <= radius * radius
    }

    pub(crate) fn expires_in(self, tick: u32) -> u16 {
        self.expires_tick.saturating_sub(tick).min(u16::MAX as u32) as u16
    }

    fn active_at(self, tick: u32) -> bool {
        self.expires_tick > tick
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SmokeCloudStore {
    #[allow(dead_code)]
    next_id: u32,
    clouds: Vec<SmokeCloud>,
}

impl Default for SmokeCloudStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SmokeCloudStore {
    pub(crate) fn new() -> Self {
        SmokeCloudStore {
            next_id: 1,
            clouds: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn spawn(
        &mut self,
        x: f32,
        y: f32,
        radius_tiles: f32,
        duration_ticks: u32,
        tick: u32,
    ) -> Option<u32> {
        if !x.is_finite() || !y.is_finite() || !radius_tiles.is_finite() || radius_tiles <= 0.0 {
            return None;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.clouds.push(SmokeCloud {
            id,
            x,
            y,
            radius_tiles,
            spawned_tick: tick,
            expires_tick: tick.saturating_add(duration_ticks),
        });
        Some(id)
    }

    pub(crate) fn retain_active(&mut self, tick: u32) {
        self.clouds.retain(|cloud| cloud.active_at(tick));
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &SmokeCloud> {
        self.clouds.iter()
    }

    pub(crate) fn point_inside(&self, x: f32, y: f32) -> bool {
        self.clouds.iter().any(|cloud| cloud.contains_point(x, y))
    }

    pub(crate) fn segment_blocked(&self, start: (f32, f32), end: (f32, f32)) -> bool {
        if !start.0.is_finite() || !start.1.is_finite() || !end.0.is_finite() || !end.1.is_finite()
        {
            return true;
        }
        self.clouds
            .iter()
            .any(|cloud| segment_intersects_cloud(start, end, *cloud))
    }

    pub(crate) fn segment_blocked_allowing_target_cloud(
        &self,
        start: (f32, f32),
        end: (f32, f32),
    ) -> bool {
        if !start.0.is_finite() || !start.1.is_finite() || !end.0.is_finite() || !end.1.is_finite()
        {
            return true;
        }
        self.clouds.iter().any(|cloud| {
            segment_intersects_cloud(start, end, *cloud) && !cloud.contains_point(end.0, end.1)
        })
    }

    pub(crate) fn visible_to_player(&self, cloud: &SmokeCloud, player: u32, fog: &Fog) -> bool {
        let radius_tiles = cloud.radius_tiles.ceil().max(0.0) as i32;
        let ts = config::TILE_SIZE as f32;
        let cx = (cloud.x / ts).floor() as i32;
        let cy = (cloud.y / ts).floor() as i32;
        for dy in -radius_tiles..=radius_tiles {
            for dx in -radius_tiles..=radius_tiles {
                let tx = cx + dx;
                let ty = cy + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let center_x = (tx as f32 + 0.5) * ts;
                let center_y = (ty as f32 + 0.5) * ts;
                if !cloud.contains_point(center_x, center_y) {
                    continue;
                }
                if fog.is_visible(player, tx as u32, ty as u32) {
                    return true;
                }
            }
        }
        false
    }

    #[allow(dead_code)]
    pub(crate) fn clamp_point_to_map(map: &Map, x: f32, y: f32) -> Option<(f32, f32)> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let max = (map.world_size_px() - 1.0).max(0.0);
        Some((x.clamp(0.0, max), y.clamp(0.0, max)))
    }
}

fn segment_intersects_cloud(start: (f32, f32), end: (f32, f32), cloud: SmokeCloud) -> bool {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return cloud.contains_point(start.0, start.1);
    }
    let t = (((cloud.x - start.0) * dx + (cloud.y - start.1) * dy) / len2).clamp(0.0, 1.0);
    let closest = (start.0 + dx * t, start.1 + dy * t);
    cloud.contains_point(closest.0, closest.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_intersection_detects_smoke_disc() {
        let cloud = SmokeCloud {
            id: 1,
            x: 64.0,
            y: 64.0,
            radius_tiles: 1.0,
            spawned_tick: 1,
            expires_tick: 10,
        };

        assert!(segment_intersects_cloud((0.0, 64.0), (128.0, 64.0), cloud));
        assert!(!segment_intersects_cloud((0.0, 0.0), (128.0, 0.0), cloud));
    }
}
