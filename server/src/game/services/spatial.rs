//! Spatial query layer: uniform grid index for fast nearest-neighbor and range queries.
//!
//! Rebuilt each tick from the live [`EntityStore`] so it always reflects current positions.
//! Used by combat target acquisition, resource search, building overlap checks, unit
//! collision resolution, and snapshot interest filtering.

use crate::config;
use crate::game::entity::{Entity, EntityStore};

/// A uniform grid index with 1-tile cells.
///
/// Map sizes are at most ~96×96, so a dense Vec-of-Vecs (~9k cells) is cheap to clear and
/// refill each tick. Each cell stores the ids of entities whose center falls on that tile.
#[derive(Debug, Default)]
pub struct SpatialIndex {
    size: u32,
    cells: Vec<Vec<u32>>,
}

impl SpatialIndex {
    /// Build a fresh index from all live entities.
    pub fn build(entities: &EntityStore, map_size: u32) -> Self {
        let cell_count = (map_size * map_size) as usize;
        let mut cells = vec![Vec::new(); cell_count];
        for e in entities.iter() {
            let tx = (e.pos_x / config::TILE_SIZE as f32).floor() as i32;
            let ty = (e.pos_y / config::TILE_SIZE as f32).floor() as i32;
            if tx < 0 || ty < 0 || tx >= map_size as i32 || ty >= map_size as i32 {
                continue;
            }
            let idx = (ty as u32 * map_size + tx as u32) as usize;
            cells[idx].push(e.id);
        }
        SpatialIndex {
            size: map_size,
            cells,
        }
    }

    /// Iterate all entity ids whose tile center lies within the inclusive tile rectangle.
    pub fn ids_in_rect(&self, min_tx: i32, min_ty: i32, max_tx: i32, max_ty: i32) -> RectIter<'_> {
        let min_tx = min_tx.clamp(0, self.size as i32 - 1);
        let min_ty = min_ty.clamp(0, self.size as i32 - 1);
        let max_tx = max_tx.clamp(0, self.size as i32 - 1);
        let max_ty = max_ty.clamp(0, self.size as i32 - 1);
        RectIter {
            index: self,
            x: min_tx,
            y: min_ty,
            min_tx,
            max_tx,
            max_ty,
            cell_idx: 0,
        }
    }

    /// Iterate all entity ids whose tile center lies within the bounding box of the circle
    /// centered at `(cx, cy)` with radius `radius_px`.
    pub fn ids_in_circle_bbox(
        &self,
        cx: f32,
        cy: f32,
        radius_px: f32,
    ) -> impl Iterator<Item = u32> + '_ {
        let ts = config::TILE_SIZE as f32;
        let min_tx = ((cx - radius_px) / ts).floor() as i32;
        let min_ty = ((cy - radius_px) / ts).floor() as i32;
        let max_tx = ((cx + radius_px) / ts).floor() as i32;
        let max_ty = ((cy + radius_px) / ts).floor() as i32;
        self.ids_in_rect(min_tx, min_ty, max_tx, max_ty)
    }

    /// Find the nearest entity to `(cx, cy)` within `max_radius_px` that satisfies `pred`.
    /// Returns `(id, squared_distance)`.
    pub fn nearest(
        &self,
        cx: f32,
        cy: f32,
        max_radius_px: f32,
        entities: &EntityStore,
        pred: impl Fn(&Entity) -> bool,
    ) -> Option<(u32, f32)> {
        let mut best: Option<(u32, f32)> = None;
        for id in self.ids_in_circle_bbox(cx, cy, max_radius_px) {
            let e = match entities.get(id) {
                Some(e) => e,
                None => continue,
            };
            if !pred(e) {
                continue;
            }
            let d2 = (e.pos_x - cx) * (e.pos_x - cx) + (e.pos_y - cy) * (e.pos_y - cy);
            let r2 = max_radius_px * max_radius_px;
            if d2 <= r2 && best.map(|(_, bd2)| d2 < bd2).unwrap_or(true) {
                best = Some((id, d2));
            }
        }
        best
    }

    /// Iterate all entity ids in every cell (row-major order). Equivalent to a full scan but
    /// avoids borrowing the [`EntityStore`].
    pub fn all_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.cells.iter().flat_map(|c| c.iter().copied())
    }
}

/// Iterator over entity ids inside a tile rectangle.
pub struct RectIter<'a> {
    index: &'a SpatialIndex,
    x: i32,
    y: i32,
    min_tx: i32,
    max_tx: i32,
    max_ty: i32,
    cell_idx: usize,
}

impl Iterator for RectIter<'_> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        loop {
            if self.y > self.max_ty {
                return None;
            }
            let idx = (self.y as u32 * self.index.size + self.x as u32) as usize;
            if self.cell_idx < self.index.cells[idx].len() {
                let id = self.index.cells[idx][self.cell_idx];
                self.cell_idx += 1;
                return Some(id);
            }
            self.cell_idx = 0;
            self.x += 1;
            if self.x > self.max_tx {
                self.x = self.min_tx;
                self.y += 1;
            }
        }
    }
}
