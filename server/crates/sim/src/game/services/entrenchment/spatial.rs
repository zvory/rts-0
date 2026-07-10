//! Local indexes used while entrenchment evaluates trench occupation and slot legality.

use super::{
    building_rect_for_entity, config, trench_radius_px, unit_body_for_entity, Entity, EntityStore,
    Map, Trench, TrenchStore, UnitBody, SLOT_EXTRA_RADIUS_PX,
};

/// A per-tick grid of trench centers. Its exact distance checks preserve the authoritative
/// candidate set while avoiding a complete trench scan for every stationary infantry unit.
pub(super) struct TrenchSpatialIndex {
    size: u32,
    cells: Vec<Vec<Trench>>,
    max_radius_px: f32,
    max_search_radius_px: f32,
}

impl TrenchSpatialIndex {
    pub(super) fn build(map: &Map, trenches: &TrenchStore) -> Self {
        let mut index = Self {
            size: map.size,
            cells: vec![Vec::new(); (map.size * map.size) as usize],
            max_radius_px: 0.0,
            max_search_radius_px: 0.0,
        };
        for trench in trenches.all().iter().copied() {
            index.insert(trench);
        }
        index
    }

    pub(super) fn insert(&mut self, trench: Trench) {
        let radius_px = trench_radius_px(trench);
        if !radius_px.is_finite() || radius_px < 0.0 {
            return;
        }
        self.max_radius_px = self.max_radius_px.max(radius_px);
        self.max_search_radius_px = self
            .max_search_radius_px
            .max(radius_px + SLOT_EXTRA_RADIUS_PX);
        let Some(cell) = self.cell_for(trench.x, trench.y) else {
            return;
        };
        self.cells[cell].push(trench);
    }

    pub(super) fn occupation_candidates(
        &self,
        entity: &Entity,
    ) -> impl Iterator<Item = Trench> + '_ {
        self.trenches_near(entity.pos_x, entity.pos_y, self.max_search_radius_px)
    }

    pub(super) fn containing_candidates(
        &self,
        x: f32,
        y: f32,
    ) -> impl Iterator<Item = Trench> + '_ {
        self.trenches_near(x, y, self.max_radius_px)
    }

    fn trenches_near(&self, x: f32, y: f32, radius_px: f32) -> impl Iterator<Item = Trench> + '_ {
        let ts = config::TILE_SIZE as f32;
        let min_tx = ((x - radius_px) / ts).floor() as i32;
        let min_ty = ((y - radius_px) / ts).floor() as i32;
        let max_tx = ((x + radius_px) / ts).floor() as i32;
        let max_ty = ((y + radius_px) / ts).floor() as i32;
        self.cells_in_rect(min_tx, min_ty, max_tx, max_ty)
            .flat_map(|cell| cell.iter().copied())
    }

    fn cells_in_rect(
        &self,
        min_tx: i32,
        min_ty: i32,
        max_tx: i32,
        max_ty: i32,
    ) -> impl Iterator<Item = &Vec<Trench>> {
        let max_tile = self.size.saturating_sub(1) as i32;
        let min_tx = min_tx.clamp(0, max_tile);
        let min_ty = min_ty.clamp(0, max_tile);
        let max_tx = max_tx.clamp(0, max_tile);
        let max_ty = max_ty.clamp(0, max_tile);
        (min_ty..=max_ty).flat_map(move |ty| {
            (min_tx..=max_tx)
                .map(move |tx| &self.cells[(ty as u32 * self.size + tx as u32) as usize])
        })
    }

    fn cell_for(&self, x: f32, y: f32) -> Option<usize> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let ts = config::TILE_SIZE as f32;
        let tx = (x / ts).floor() as i32;
        let ty = (y / ts).floor() as i32;
        if tx < 0 || ty < 0 || tx >= self.size as i32 || ty >= self.size as i32 {
            return None;
        }
        Some((ty as u32 * self.size + tx as u32) as usize)
    }
}

/// Dynamic local entity lookup used only while this service evaluates slot legality. The index
/// is updated immediately after each slot correction, matching the existing id-ordered mutation
/// semantics without repeating a global entity scan for every candidate slot.
pub(super) struct EntrenchmentEntityIndex {
    size: u32,
    cells: Vec<Vec<u32>>,
    max_unit_radius_px: f32,
    max_building_reach_px: f32,
}

impl EntrenchmentEntityIndex {
    pub(super) fn build(map: &Map, entities: &EntityStore) -> Self {
        let mut index = Self {
            size: map.size,
            cells: vec![Vec::new(); (map.size * map.size) as usize],
            max_unit_radius_px: 0.0,
            max_building_reach_px: 0.0,
        };
        for entity in entities.iter() {
            index.insert(entity.id, entity.pos_x, entity.pos_y);
            if entity.hp > 0 && entity.is_unit() {
                index.max_unit_radius_px = index
                    .max_unit_radius_px
                    .max(unit_body_for_entity(entity).map_or(0.0, UnitBody::bounding_radius));
            }
            if entity.hp > 0 && entity.is_building() {
                index.max_building_reach_px =
                    index
                        .max_building_reach_px
                        .max(building_rect_for_entity(map, entity).map_or(0.0, |rect| {
                            (rect.min_x - entity.pos_x)
                                .abs()
                                .max((rect.max_x - entity.pos_x).abs())
                                .max((rect.min_y - entity.pos_y).abs())
                                .max((rect.max_y - entity.pos_y).abs())
                        }));
            }
        }
        index
    }

    pub(super) fn relocate(&mut self, id: u32, old_position: (f32, f32), new_position: (f32, f32)) {
        let old_cell = self.cell_for(old_position.0, old_position.1);
        let new_cell = self.cell_for(new_position.0, new_position.1);
        if old_cell == new_cell {
            return;
        }
        if let Some(old_cell) = old_cell {
            if let Some(position) = self.cells[old_cell].iter().position(|other| *other == id) {
                self.cells[old_cell].swap_remove(position);
            }
        }
        if let Some(new_cell) = new_cell {
            self.cells[new_cell].push(id);
        }
    }

    pub(super) fn unit_query_radius(&self, candidate_body: UnitBody) -> f32 {
        candidate_body.bounding_radius() + self.max_unit_radius_px
    }

    pub(super) fn building_query_radius(&self, candidate_body: UnitBody) -> f32 {
        candidate_body.bounding_radius() + self.max_building_reach_px
    }

    pub(super) fn ids_near(
        &self,
        x: f32,
        y: f32,
        radius_px: f32,
    ) -> impl Iterator<Item = u32> + '_ {
        let ts = config::TILE_SIZE as f32;
        let min_tx = ((x - radius_px) / ts).floor() as i32;
        let min_ty = ((y - radius_px) / ts).floor() as i32;
        let max_tx = ((x + radius_px) / ts).floor() as i32;
        let max_ty = ((y + radius_px) / ts).floor() as i32;
        self.cells_in_rect(min_tx, min_ty, max_tx, max_ty)
            .flat_map(|cell| cell.iter().copied())
    }

    fn insert(&mut self, id: u32, x: f32, y: f32) {
        if let Some(cell) = self.cell_for(x, y) {
            self.cells[cell].push(id);
        }
    }

    fn cells_in_rect(
        &self,
        min_tx: i32,
        min_ty: i32,
        max_tx: i32,
        max_ty: i32,
    ) -> impl Iterator<Item = &Vec<u32>> {
        let max_tile = self.size.saturating_sub(1) as i32;
        let min_tx = min_tx.clamp(0, max_tile);
        let min_ty = min_ty.clamp(0, max_tile);
        let max_tx = max_tx.clamp(0, max_tile);
        let max_ty = max_ty.clamp(0, max_tile);
        (min_ty..=max_ty).flat_map(move |ty| {
            (min_tx..=max_tx)
                .map(move |tx| &self.cells[(ty as u32 * self.size + tx as u32) as usize])
        })
    }

    fn cell_for(&self, x: f32, y: f32) -> Option<usize> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let ts = config::TILE_SIZE as f32;
        let tx = (x / ts).floor() as i32;
        let ty = (y / ts).floor() as i32;
        if tx < 0 || ty < 0 || tx >= self.size as i32 || ty >= self.size as i32 {
            return None;
        }
        Some((ty as u32 * self.size + tx as u32) as usize)
    }
}

pub(super) struct EntrenchmentIndexes {
    pub(super) trenches: TrenchSpatialIndex,
    pub(super) entities: EntrenchmentEntityIndex,
}

impl EntrenchmentIndexes {
    pub(super) fn build(map: &Map, entities: &EntityStore, trenches: &TrenchStore) -> Self {
        Self {
            trenches: TrenchSpatialIndex::build(map, trenches),
            entities: EntrenchmentEntityIndex::build(map, entities),
        }
    }
}
