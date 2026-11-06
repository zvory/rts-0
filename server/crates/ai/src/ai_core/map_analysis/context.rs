//! Immutable map contexts shared by controllers without retaining inactive room maps.

use std::cell::RefCell;
use std::sync::{Arc, Weak};

use rts_protocol::ObserverMapAnalysisDiagnostics;
use rts_sim::protocol::StartPayload;

use super::{fnv_bytes, fnv_u32, AiMapAnalysis, AiMapAnalysisKey};

#[derive(Clone, Debug)]
pub(crate) struct AiStaticMapContext {
    key: AiMapAnalysisKey,
    sharing_key: AiMapAnalysisKey,
    analysis: AiMapAnalysis,
    diagnostics: ObserverMapAnalysisDiagnostics,
    base_sites: Vec<crate::sdk::AiBaseSite>,
}

impl AiStaticMapContext {
    fn analyze_with_key(start: &StartPayload, key: AiMapAnalysisKey) -> Self {
        let analysis = AiMapAnalysis::analyze_with_key(start, key);
        let diagnostics = analysis.debug_overlay();
        let base_sites = analysis.sdk_base_sites();
        Self {
            key,
            sharing_key: static_map_sharing_key(start, key),
            analysis,
            diagnostics,
            base_sites,
        }
    }

    pub(crate) fn base_sites(&self) -> &[crate::sdk::AiBaseSite] {
        &self.base_sites
    }

    pub(crate) fn key(&self) -> AiMapAnalysisKey {
        self.key
    }

    pub(crate) fn analysis(&self) -> &AiMapAnalysis {
        &self.analysis
    }

    pub(crate) fn diagnostics(&self) -> &ObserverMapAnalysisDiagnostics {
        &self.diagnostics
    }
}

// Local cache invalidation keeps its established semantics. Sharing with a different controller
// additionally covers elevation and slow ground, which the analysis reads but the legacy key
// does not include. These extra hashes are only needed when the local cache misses.
fn static_map_sharing_key(start: &StartPayload, mut key: AiMapAnalysisKey) -> AiMapAnalysisKey {
    let mut hash = fnv_u32(key.overlays_hash, start.map.elevation.len() as u32);
    hash = fnv_bytes(hash, &start.map.elevation);
    hash = fnv_u32(hash, start.map.slow_movement_tiles.len() as u32);
    for tile in &start.map.slow_movement_tiles {
        hash = fnv_u32(hash, tile.x);
        hash = fnv_u32(hash, tile.y);
    }
    key.overlays_hash = hash;
    key
}

// Controllers invoked together use the same immutable start payload. Keep only a weak,
// single-entry hint: live controller caches own the data, and the hint retains no map buffers.
// Thread-local storage avoids a process-wide lock and keeps unrelated room work independent.
thread_local! {
    static SHARED_STATIC_MAP_CONTEXT: RefCell<Option<Weak<AiStaticMapContext>>> =
        const { RefCell::new(None) };
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AiStaticMapContextCache {
    context: Option<Arc<AiStaticMapContext>>,
}

impl AiStaticMapContextCache {
    pub(crate) fn get_or_analyze(&mut self, start: &StartPayload) -> &Arc<AiStaticMapContext> {
        let key = AiMapAnalysisKey::from_start(start);
        let refresh = self
            .context
            .as_ref()
            .map(|context| context.key() != key)
            .unwrap_or(true);
        if refresh {
            self.context = None;
        }
        self.context.get_or_insert_with(|| {
            let sharing_key = static_map_sharing_key(start, key);
            SHARED_STATIC_MAP_CONTEXT.with(|slot| {
                let mut slot = slot.borrow_mut();
                if let Some(context) = slot.as_ref().and_then(Weak::upgrade) {
                    if context.sharing_key == sharing_key {
                        return context;
                    }
                }
                let context = Arc::new(AiStaticMapContext::analyze_with_key(start, key));
                *slot = Some(Arc::downgrade(&context));
                context
            })
        })
    }

    pub(crate) fn current(&self) -> Option<&AiStaticMapContext> {
        self.context.as_deref()
    }
}
