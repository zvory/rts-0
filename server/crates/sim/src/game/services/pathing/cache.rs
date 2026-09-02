use super::*;

pub(super) type CacheKey = (
    EntityKind,
    (i32, i32),
    (i32, i32),
    u32,
    RouteShape,
    usize,
    u64,
);

#[derive(Clone)]
pub(super) struct CacheEntry {
    tile_path: Vec<(i32, i32)>,
    search_expanded_nodes: usize,
    last_used: u32,
}

impl PathingService {
    pub(super) fn cache_lookup<P: Passability>(
        &mut self,
        req: &PathRequest,
        pass: &P,
        static_fingerprint: u64,
        search_budget: usize,
    ) -> Option<(Vec<(i32, i32)>, usize)> {
        let key: CacheKey = (
            req.kind,
            req.start,
            req.goal,
            req.radius_tiles,
            req.route_shape,
            search_budget,
            static_fingerprint,
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
                search_budget,
                static_fingerprint,
            ),
            CacheEntry {
                tile_path,
                search_expanded_nodes: diagnostics.scheduling_expanded_nodes,
                last_used: self.tick,
            },
        );
    }
}
