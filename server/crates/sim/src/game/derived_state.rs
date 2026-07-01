//! Rebuildable simulation state owned under `Game`.
//!
//! This shell keeps cache/index state explicit without making it authoritative checkpoint state.

use crate::game::entity::EntityStore;
use crate::game::map::Map;
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;

#[derive(Clone)]
pub(in crate::game) struct DerivedState {
    final_spatial: SpatialIndex,
    pathing: PathingService,
}

impl DerivedState {
    pub(in crate::game) fn new(
        map: &Map,
        entities: &EntityStore,
        default_pathing_budget: usize,
        pathing_cache_capacity: usize,
    ) -> Self {
        DerivedState {
            final_spatial: SpatialIndex::build(entities, map.size),
            pathing: PathingService::new(default_pathing_budget, pathing_cache_capacity),
        }
    }

    pub(in crate::game) fn final_spatial(&self) -> &SpatialIndex {
        &self.final_spatial
    }

    pub(in crate::game) fn set_final_spatial(&mut self, spatial: SpatialIndex) {
        self.final_spatial = spatial;
    }

    pub(in crate::game) fn pathing_mut(&mut self) -> &mut PathingService {
        &mut self.pathing
    }

    pub(in crate::game) fn advance_pathing_tick(&mut self, tick: u32) {
        self.pathing.advance_tick(tick);
    }

    pub(in crate::game) fn pathing_config(&self) -> (usize, usize) {
        self.pathing.config()
    }

    pub(in crate::game) fn rebuild_final_spatial(&mut self, map: &Map, entities: &EntityStore) {
        self.final_spatial = SpatialIndex::build(entities, map.size);
    }

    #[allow(dead_code)]
    pub(in crate::game) fn clear_and_rebuild_from_authoritative(
        &mut self,
        map: &Map,
        entities: &EntityStore,
    ) {
        self.pathing.clear_rebuildable_state();
        self.rebuild_final_spatial(map, entities);
    }

    #[cfg(test)]
    pub(in crate::game) fn pathing_cache_len_for_test(&self) -> usize {
        self.pathing.cache_len()
    }

    #[cfg(test)]
    pub(in crate::game) fn pathing_config_for_test(&self) -> (usize, usize) {
        self.pathing_config()
    }
}
