use super::*;

pub(super) type CacheKey = (
    EntityKind,
    (i32, i32),
    (i32, i32),
    u32,
    RouteShape,
    RoutePolicy,
    usize,
    u64,
    u64,
);

pub(super) type FinalizedCacheKey = (
    EntityKind,
    RouteShape,
    RoutePolicy,
    (u32, u32),
    (u32, u32),
    u64,
    Vec<(u32, u32)>,
);

#[derive(Clone)]
pub(super) struct CacheEntry {
    tile_path: Vec<(i32, i32)>,
    search_expanded_nodes: usize,
    last_used: u32,
}

#[derive(Clone)]
pub(super) struct FinalizedCacheEntry {
    waypoints: Vec<(f32, f32)>,
    last_used: u32,
}

impl PathingService {
    pub(super) fn finalized_cache_lookup(
        &mut self,
        key: &FinalizedCacheKey,
    ) -> Option<Vec<(f32, f32)>> {
        let entry = self.finalized_cache.get_mut(key)?;
        entry.last_used = self.tick;
        Some(entry.waypoints.clone())
    }

    pub(super) fn finalized_cache_insert(
        &mut self,
        key: FinalizedCacheKey,
        waypoints: Vec<(f32, f32)>,
    ) {
        if self.finalized_cache.len() >= self.cache_cap {
            if let Some(oldest_key) = self
                .finalized_cache
                .iter()
                .min_by_key(|(key, entry)| (entry.last_used, *key))
                .map(|(key, _)| key.clone())
            {
                self.finalized_cache.remove(&oldest_key);
            }
        }
        self.finalized_cache.insert(
            key,
            FinalizedCacheEntry {
                waypoints,
                last_used: self.tick,
            },
        );
    }

    pub(super) fn cache_lookup<P: Passability>(
        &mut self,
        req: &PathRequest,
        pass: &P,
        static_fingerprint: u64,
        cost_fingerprint: u64,
        search_budget: usize,
    ) -> Option<(Vec<(i32, i32)>, usize)> {
        let key: CacheKey = (
            req.kind,
            req.start,
            req.goal,
            req.radius_tiles,
            req.route_shape,
            req.policy,
            search_budget,
            static_fingerprint,
            cost_fingerprint,
        );
        let entry = self.cache.get_mut(&key)?;
        for &(tx, ty) in &entry.tile_path {
            if !pass.passable(tx, ty) {
                return None;
            }
        }
        entry.last_used = self.tick;
        Some((entry.tile_path.clone(), entry.search_expanded_nodes))
    }

    pub(super) fn cache_insert(
        &mut self,
        req: &PathRequest,
        static_fingerprint: u64,
        cost_fingerprint: u64,
        search_budget: usize,
        tile_path: Vec<(i32, i32)>,
        diagnostics: PathingRequestDiagnostics,
    ) {
        if self.cache.len() >= self.cache_cap {
            if let Some(oldest_key) = self
                .cache
                .iter()
                .min_by_key(|(key, entry)| (entry.last_used, *key))
                .map(|(key, _)| *key)
            {
                self.cache.remove(&oldest_key);
            }
        }
        self.cache.insert(
            (
                req.kind,
                req.start,
                req.goal,
                req.radius_tiles,
                req.route_shape,
                req.policy,
                search_budget,
                static_fingerprint,
                cost_fingerprint,
            ),
            CacheEntry {
                tile_path,
                search_expanded_nodes: diagnostics.scheduling_expanded_nodes,
                last_used: self.tick,
            },
        );
    }
}
