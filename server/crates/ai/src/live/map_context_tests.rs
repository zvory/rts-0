//! Map context sharing, invalidation, and lifetime equivalence checks.

use super::tests::cache_test_start_payload;
use super::*;
use rts_sim::protocol::terrain;

#[test]
fn live_controller_caches_static_map_analysis_by_start_identity() {
    let mut ai = AiController::new(1);
    let start = cache_test_start_payload();

    let first_sites = ai
        .static_map_context
        .get_or_analyze(&start)
        .base_sites()
        .to_vec();
    assert_eq!(first_sites.len(), start.players.len());
    let first_key = ai.static_map_context.get_or_analyze(&start).key();
    let second_key = ai.static_map_context.get_or_analyze(&start).key();
    assert_eq!(second_key, first_key);
    assert_eq!(
        ai.static_map_context.current().map(|context| context.key()),
        Some(first_key)
    );

    let mut no_resources = start.clone();
    no_resources.map.resources.clear();
    let empty_sites = ai
        .static_map_context
        .get_or_analyze(&no_resources)
        .base_sites();
    assert_eq!(empty_sites.len(), 1);
    assert!(empty_sites[0].resource_ids.is_empty());

    let mut moved_start = start.clone();
    moved_start.players[0].start_tile_x = 2;
    let moved_key = ai.static_map_context.get_or_analyze(&moved_start).key();

    assert_ne!(moved_key, first_key);
    assert_ne!(
        ai.static_map_context.current().unwrap().base_sites(),
        first_sites
    );
    assert_eq!(
        ai.static_map_context.current().map(|context| context.key()),
        Some(moved_key)
    );

    let mut edited_terrain = moved_start.clone();
    edited_terrain.map.terrain[0] = terrain::ROCK;
    let edited_key = ai.static_map_context.get_or_analyze(&edited_terrain).key();

    assert_ne!(edited_key, moved_key);
    assert_eq!(
        ai.static_map_context.current().map(|context| context.key()),
        Some(edited_key)
    );
}

#[test]
fn static_map_context_shares_only_matching_complete_keys() {
    use crate::ai_core::map_analysis::AiMapAnalysis;
    use std::sync::Arc;

    let start = cache_test_start_payload();
    let mut first = AiStaticMapContextCache::default();
    let mut second = AiStaticMapContextCache::default();
    let original = first.get_or_analyze(&start).clone();
    assert!(Arc::ptr_eq(&original, second.get_or_analyze(&start)));

    let mut changes = Vec::new();
    let mut changed = start.clone();
    changed.players[0].start_tile_x += 1;
    changes.push(changed);
    let mut changed = start.clone();
    changed.players[0].team_id += 1;
    changes.push(changed);
    let mut changed = start.clone();
    changed.map.resources[0].x += crate::config::TILE_SIZE as f32;
    changes.push(changed);
    let mut changed = start.clone();
    changed.map.resources.clear();
    changes.push(changed);
    let mut changed = start.clone();
    changed.map.terrain[0] = terrain::ROCK;
    changes.push(changed);
    let mut changed = start.clone();
    changed
        .map
        .concealment_tiles
        .push(rts_sim::protocol::MapTile { x: 0, y: 0 });
    changes.push(changed);

    for changed in changes {
        let updated = first.get_or_analyze(&changed).clone();
        assert!(!Arc::ptr_eq(&original, &updated));
        assert_ne!(original.key(), updated.key());
        assert_eq!(updated.analysis(), &AiMapAnalysis::analyze(&changed));
        assert!(Arc::ptr_eq(&updated, second.get_or_analyze(&changed)));
    }
    // A live controller's context remains valid after another key replaces the hint.
    assert_eq!(original.analysis(), &AiMapAnalysis::analyze(&start));
}

#[test]
fn shared_static_map_covers_elevation_and_slow_ground_without_changing_local_invalidation() {
    use crate::ai_core::map_analysis::AiMapAnalysis;
    use std::sync::Arc;

    let start = cache_test_start_payload();
    let mut first = AiStaticMapContextCache::default();
    let original = first.get_or_analyze(&start).clone();
    let mut changed = start.clone();
    changed.map.elevation[0] = 1;
    changed
        .map
        .slow_movement_tiles
        .push(rts_sim::protocol::MapTile { x: 0, y: 0 });
    // Existing controllers retain their established cache invalidation behavior.
    assert!(Arc::ptr_eq(&original, first.get_or_analyze(&changed)));
    // A different controller must never receive analysis from different map inputs.
    let mut second = AiStaticMapContextCache::default();
    let updated = second.get_or_analyze(&changed);
    assert!(!Arc::ptr_eq(&original, updated));
    assert_eq!(updated.key(), original.key());
    assert_eq!(updated.analysis(), &AiMapAnalysis::analyze(&changed));
}

#[test]
fn shared_static_map_hint_does_not_keep_analysis_alive() {
    use std::sync::Arc;

    let start = cache_test_start_payload();
    let weak = {
        let mut cache = AiStaticMapContextCache::default();
        Arc::downgrade(cache.get_or_analyze(&start))
    };
    assert!(weak.upgrade().is_none());
    let mut cache = AiStaticMapContextCache::default();
    assert_eq!(
        cache.get_or_analyze(&start).key(),
        crate::ai_core::map_analysis::AiMapAnalysisKey::from_start(&start)
    );
}
