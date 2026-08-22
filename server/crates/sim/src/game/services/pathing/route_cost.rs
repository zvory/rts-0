use crate::config;
use crate::game::map::Map;
use crate::rules::terrain::{self, TerrainKind};

/// Shared deterministic terrain-time metric for graph edges and continuous authored segments.
pub(super) struct RouteCostModel<'a> {
    map: &'a Map,
}

impl<'a> RouteCostModel<'a> {
    pub(super) fn new(map: &'a Map) -> Self {
        Self { map }
    }

    pub(super) fn edge_cost(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        base_step_cost: u32,
    ) -> Option<u32> {
        if !self.map.in_bounds(from.0, from.1) || !self.map.in_bounds(to.0, to.1) {
            return None;
        }
        let terrain =
            TerrainKind::from_map_code(self.map.terrain_at(from.0 as u32, from.1 as u32))?;
        let source_elevation = self.elevation(from);
        let destination_elevation = self.elevation(to);
        Some(terrain::terrain_route_edge_cost(
            base_step_cost,
            terrain,
            self.map.is_slow_movement_tile(from.0 as u32, from.1 as u32),
            source_elevation,
            destination_elevation,
        ))
    }

    /// Re-cost a world-space segment by exact crossings of half-open authored tile boundaries.
    /// Segment length uses 1/1024-pixel fixed point before the same rational speed composition as
    /// graph edges; this avoids fixed-distance sampling and is deterministic across repeated runs.
    pub(super) fn segment_cost(&self, from: (f32, f32), to: (f32, f32)) -> Option<u64> {
        if !self.map.contains_world_point(from.0, from.1)
            || !self.map.contains_world_point(to.0, to.1)
        {
            return None;
        }
        let dx = f64::from(to.0 - from.0);
        let dy = f64::from(to.1 - from.1);
        let total_distance = dx.hypot(dy);
        if !total_distance.is_finite() {
            return None;
        }
        if total_distance == 0.0 {
            return Some(0);
        }

        let tile_size = f64::from(config::TILE_SIZE);
        let from_tile = self.map.tile_of(from.0, from.1);
        let to_tile = self.map.tile_of(to.0, to.1);
        if from == self.map.tile_center(from_tile.0, from_tile.1)
            && to == self.map.tile_center(to_tile.0, to_tile.1)
            && (from_tile.0 as i64 - to_tile.0 as i64).unsigned_abs() <= 1
            && (from_tile.1 as i64 - to_tile.1 as i64).unsigned_abs() <= 1
        {
            let source_terrain =
                TerrainKind::from_map_code(self.map.terrain_at(from_tile.0, from_tile.1))?;
            let destination_terrain =
                TerrainKind::from_map_code(self.map.terrain_at(to_tile.0, to_tile.1))?;
            let source_elevation = self.elevation((from_tile.0 as i32, from_tile.1 as i32));
            let destination_elevation = self.elevation((to_tile.0 as i32, to_tile.1 as i32));
            let source_ratio = terrain::terrain_route_speed_ratio(
                source_terrain,
                self.map.is_slow_movement_tile(from_tile.0, from_tile.1),
                source_elevation,
                destination_elevation,
            );
            let destination_ratio = terrain::terrain_route_speed_ratio(
                destination_terrain,
                self.map.is_slow_movement_tile(to_tile.0, to_tile.1),
                destination_elevation,
                destination_elevation,
            );
            let scaled_distance =
                total_distance * 1024.0 * 10.0 * f64::from(terrain::ROUTE_TIME_SCALE) / tile_size;
            return Some(if source_ratio == destination_ratio {
                apply_ratio(scaled_distance, source_ratio)
            } else {
                apply_ratio(scaled_distance * 0.5, source_ratio)
                    .saturating_add(apply_ratio(scaled_distance * 0.5, destination_ratio))
            });
        }

        let mut crossings = vec![0.0_f64, 1.0_f64];
        add_axis_crossings(f64::from(from.0), dx, tile_size, &mut crossings);
        add_axis_crossings(f64::from(from.1), dy, tile_size, &mut crossings);
        crossings.sort_by(f64::total_cmp);
        crossings.dedup_by(|a, b| (*a - *b).abs() <= f64::EPSILON * 8.0);

        let mut total = 0_u64;
        let mut pending_scaled_distance = 0.0_f64;
        let mut pending_ratio = None;
        for interval in crossings.windows(2) {
            let t0 = interval[0];
            let t1 = interval[1];
            if t1 <= t0 {
                continue;
            }
            let midpoint = (t0 + t1) * 0.5;
            let x = f64::from(from.0) + dx * midpoint;
            let y = f64::from(from.1) + dy * midpoint;
            let (tx, ty) = self.map.tile_of(x as f32, y as f32);
            let terrain = TerrainKind::from_map_code(self.map.terrain_at(tx, ty))?;
            let ahead_t = (midpoint + tile_size / total_distance).min(1.0);
            let ahead = self.map.tile_of(
                (f64::from(from.0) + dx * ahead_t) as f32,
                (f64::from(from.1) + dy * ahead_t) as f32,
            );
            let interval_distance = total_distance * (t1 - t0);
            let scaled_distance =
                interval_distance * 1024.0 * 10.0 * f64::from(terrain::ROUTE_TIME_SCALE)
                    / tile_size;
            let ratio = terrain::terrain_route_speed_ratio(
                terrain,
                self.map.is_slow_movement_tile(tx, ty),
                self.elevation((tx as i32, ty as i32)),
                self.elevation((ahead.0 as i32, ahead.1 as i32)),
            );
            if pending_ratio.is_some_and(|pending| pending != ratio) {
                total = total.saturating_add(apply_ratio(pending_scaled_distance, pending_ratio?));
                pending_scaled_distance = 0.0;
            }
            pending_ratio = Some(ratio);
            pending_scaled_distance += scaled_distance;
        }
        if let Some(ratio) = pending_ratio {
            total = total.saturating_add(apply_ratio(pending_scaled_distance, ratio));
        }
        Some(total)
    }

    fn elevation(&self, tile: (i32, i32)) -> u8 {
        if !self.map.in_bounds(tile.0, tile.1) {
            return 0;
        }
        self.map
            .elevation
            .get(self.map.index(tile.0 as u32, tile.1 as u32))
            .copied()
            .unwrap_or(0)
    }
}

fn apply_ratio(scaled_distance: f64, ratio: (u64, u64)) -> u64 {
    let exact = scaled_distance * ratio.1 as f64 / ratio.0 as f64;
    (exact - exact.abs() * 1.0e-12).ceil() as u64
}

fn add_axis_crossings(origin: f64, delta: f64, tile_size: f64, crossings: &mut Vec<f64>) {
    if delta.abs() <= f64::EPSILON {
        return;
    }
    let end = origin + delta;
    let low = origin.min(end);
    let high = origin.max(end);
    let first = (low / tile_size).floor() as i64 + 1;
    let last = (high / tile_size).ceil() as i64 - 1;
    for boundary_index in first..=last {
        let boundary = boundary_index as f64 * tile_size;
        let t = (boundary - origin) / delta;
        if t > 0.0 && t < 1.0 {
            crossings.push(t);
        }
    }
}
