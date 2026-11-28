//! Resource-centered Depot geometry shared with Pressure's expansion placement.
use super::{
    AiBaseSiteId, AiResourceAmount, AiTile, EntityKind, KnownBuildSite, KnownBuildSiteExclusions,
    WorldQueries,
};
use crate::config;
use rts_rules::math::GameplayMath as _;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ResourceCoverage {
    pub steel: usize,
    pub oil: usize,
    pub max_distance2: f32,
    pub sum_distance2: f32,
}

/// Pressure's mining-anchor coverage and resource distance objective.
pub(crate) fn resource_coverage(
    center: (f32, f32),
    tile_size: u32,
    resources: impl IntoIterator<Item = (EntityKind, (f32, f32))>,
) -> ResourceCoverage {
    let range2 = (config::MINING_ANCHOR_RANGE_TILES * tile_size as f32).det_powi(2);
    let mut result = ResourceCoverage::default();
    for (kind, point) in resources {
        let d = distance2(center, point);
        if !d.is_finite() || d > range2 {
            continue;
        }
        match kind {
            EntityKind::Steel => result.steel += 1,
            EntityKind::Oil => result.oil += 1,
            _ => continue,
        }
        result.max_distance2 = result.max_distance2.max(d);
        result.sum_distance2 += d;
    }
    result
}

/// Infer the base center behind a Steel patch, using the map-center direction used by Pressure.
pub(crate) fn resource_base_anchor(
    position: (f32, f32),
    tile_size: f32,
    width: u32,
    height: u32,
) -> Option<(u32, u32)> {
    if tile_size <= 0.0 {
        return None;
    }
    let resource = (position.0 / tile_size, position.1 / tile_size);
    let dir = (
        width as f32 * 0.5 - resource.0,
        height as f32 * 0.5 - resource.1,
    );
    let len = (dir.0 * dir.0 + dir.1 * dir.1).det_sqrt();
    if !len.is_finite() || len <= f32::EPSILON {
        return None;
    }
    let center = (
        resource.0 - dir.0 / len * config::STEEL_BLOCK_DIST_TILES,
        resource.1 - dir.1 / len * config::STEEL_BLOCK_DIST_TILES,
    );
    Some((
        center.0.round().clamp(0.0, width.saturating_sub(1) as f32) as u32,
        center.1.round().clamp(0.0, height.saturating_sub(1) as f32) as u32,
    ))
}

impl WorldQueries<'_> {
    /// Recommend a resource-centered Depot footprint at a public base landmark.
    /// Requires static ground reachability and rejects known Depot/hostile evidence. This is
    /// a fog-respecting recommendation, not authoritative legality or dynamic reachability.
    /// Prefer the exact landmark footprint; otherwise use Pressure's six-tile resource-anchor
    /// search, requiring the cluster's Steel and Oil coverage before ranking resource distances.
    pub fn base_depot_site(
        &self,
        site_id: AiBaseSiteId,
        exclusions: &KnownBuildSiteExclusions,
    ) -> Option<AiTile> {
        let site = self.frame.base_sites().iter().find(|s| s.id == site_id)?;
        let player = self.frame.player_id();
        if !site.ground_distances_from_starts.contains_key(&player)
            || matches!(site_id, AiBaseSiteId::Start(owner) if owner != player)
            || !self.known_base_depots(site_id).is_empty()
        {
            return None;
        }
        let map = self.frame.map();
        let ts = map.tile_size as f32;
        if ts <= 0.0 {
            return None;
        }
        let hostile_radius2 = (ts * 12.0).det_powi(2);
        if self
            .visible_enemies()
            .iter()
            .any(|e| distance2(e.position, site.position) <= hostile_radius2)
            || self
                .remembered_contacts()
                .iter()
                .any(|e| distance2(e.position, site.position) <= hostile_radius2)
        {
            return None;
        }
        let resources: Vec<_> = self
            .resources()
            .iter()
            .filter(|r| {
                site.resource_ids.contains(&r.id)
                    && r.remaining != AiResourceAmount::Known(0)
                    && matches!(r.kind, EntityKind::Steel | EntityKind::Oil)
            })
            .collect();
        let steel = resources
            .iter()
            .filter(|r| r.kind == EntityKind::Steel)
            .count()
            .min(config::STEEL_PATCHES_PER_BASE as usize);
        let oil = resources
            .iter()
            .filter(|r| r.kind == EntityKind::Oil)
            .count()
            .min(config::OIL_PATCHES_PER_BASE as usize);
        if steel == 0 {
            return None;
        }
        let stats = super::AiRulebook::for_frame(self.frame)?
            .entity(EntityKind::ResourceDepot)?
            .footprint?;
        let evaluate = |tile: AiTile| {
            if self.planned_build_site(EntityKind::ResourceDepot, tile, exclusions)
                != KnownBuildSite::NoKnownConflict
            {
                return None;
            }
            let center = (
                (tile.x() as f32 + stats.width_tiles as f32 * 0.5) * ts,
                (tile.y() as f32 + stats.height_tiles as f32 * 0.5) * ts,
            );
            // A recommendation for K must still associate with K under the SDK's
            // Depot evidence rule, rather than drifting toward an adjacent landmark.
            if distance2(center, site.position) > hostile_radius2
                || self
                    .frame
                    .base_sites()
                    .iter()
                    .min_by(|a, b| {
                        distance2(center, a.position)
                            .total_cmp(&distance2(center, b.position))
                            .then_with(|| a.id.cmp(&b.id))
                    })
                    .is_none_or(|nearest| nearest.id != site_id)
            {
                return None;
            }
            let coverage = resource_coverage(
                center,
                map.tile_size,
                resources.iter().map(|r| (r.kind, r.position)),
            );
            (coverage.steel >= steel && coverage.oil >= oil).then_some(coverage)
        };
        let footprint = |center: (u32, u32)| {
            self.tile(
                center.0.checked_sub(stats.width_tiles / 2)?,
                center.1.checked_sub(stats.height_tiles / 2)?,
            )
        };
        let landmark = self.world_to_tile(self.world_point(site.position.0, site.position.1)?)?;
        if let Some(tile) = footprint(landmark.as_tuple()) {
            if evaluate(tile).is_some() {
                return Some(tile);
            }
        }
        let mut candidates = std::collections::BTreeSet::new();
        for resource in resources.iter().filter(|r| r.kind == EntityKind::Steel) {
            let Some(anchor) = resource_base_anchor(resource.position, ts, map.width, map.height)
                .and_then(footprint)
            else {
                continue;
            };
            for dy in -6i64..=6 {
                for dx in -6i64..=6 {
                    let x = i64::from(anchor.x()) + dx;
                    let y = i64::from(anchor.y()) + dy;
                    if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) {
                        if let Some(tile) = self.tile(x, y) {
                            candidates.insert(tile);
                        }
                    }
                }
            }
        }
        candidates
            .into_iter()
            .filter_map(|tile| evaluate(tile).map(|score| (tile, score)))
            .min_by(|(a, sa), (b, sb)| {
                sa.max_distance2
                    .total_cmp(&sb.max_distance2)
                    .then_with(|| sa.sum_distance2.total_cmp(&sb.sum_distance2))
                    .then_with(|| a.cmp(b))
            })
            .map(|(tile, _)| tile)
    }
}
fn distance2(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::super::AiFrame;
    use super::*;
    use crate::ai_core::map_analysis::AiStaticMapContextCache;
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, PlayerInit};

    #[test]
    fn classic_both_sides_center_depots_on_steel_and_oil_and_fallback() {
        let players = [1, 2].map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        });
        let game = Game::new_with_random_ai_profiles_and_map(
            &players,
            7,
            Map::load("Classic", 2, 7).unwrap(),
        );
        let start = game.start_payload();
        let mut cache = AiStaticMapContextCache::default();
        let sites = cache.get_or_analyze(&start).base_sites().to_vec();
        for player in [1, 2] {
            let snapshot = game.snapshot_for(player);
            let frame = AiFrame::from_host(&start, &snapshot, player, [], Some(&[1, 2]))
                .unwrap()
                .with_base_sites(&sites);
            let queries = WorldQueries::new(&frame);
            assert!(queries
                .base_depot_site(
                    AiBaseSiteId::Start(player),
                    &KnownBuildSiteExclusions::default()
                )
                .is_none());
            assert!(queries
                .base_depot_site(
                    AiBaseSiteId::Start(3 - player),
                    &KnownBuildSiteExclusions::default()
                )
                .is_none());
            for base in queries.ranked_bases(player).iter().skip(1).take(3) {
                let mut exclusions = KnownBuildSiteExclusions::default();
                let tile = queries
                    .base_depot_site(base.site.id, &exclusions)
                    .unwrap_or_else(|| {
                        panic!("player {player} rank {} {:?}", base.rank, base.site)
                    });
                let ts = frame.map().tile_size as f32;
                let stats = config::building_stats(EntityKind::ResourceDepot).unwrap();
                let center = (
                    (tile.x() as f32 + stats.foot_w as f32 / 2.0) * ts,
                    (tile.y() as f32 + stats.foot_h as f32 / 2.0) * ts,
                );
                let coverage = resource_coverage(
                    center,
                    frame.map().tile_size,
                    frame
                        .resources()
                        .iter()
                        .filter(|r| base.site.resource_ids.contains(&r.id))
                        .map(|r| (r.kind, r.position)),
                );
                assert_eq!(coverage.steel, config::STEEL_PATCHES_PER_BASE as usize);
                assert_eq!(coverage.oil, config::OIL_PATCHES_PER_BASE as usize);
                let landmark = queries
                    .world_to_tile(
                        queries
                            .world_point(base.site.position.0, base.site.position.1)
                            .unwrap(),
                    )
                    .unwrap();
                let exact = queries
                    .tile(
                        landmark.x() - stats.foot_w / 2,
                        landmark.y() - stats.foot_h / 2,
                    )
                    .unwrap();
                let exact_center = (
                    (exact.x() as f32 + stats.foot_w as f32 / 2.0) * ts,
                    (exact.y() as f32 + stats.foot_h as f32 / 2.0) * ts,
                );
                let exact_coverage = resource_coverage(
                    exact_center,
                    frame.map().tile_size,
                    frame
                        .resources()
                        .iter()
                        .filter(|r| base.site.resource_ids.contains(&r.id))
                        .map(|r| (r.kind, r.position)),
                );
                if exact_coverage.steel >= config::STEEL_PATCHES_PER_BASE as usize
                    && exact_coverage.oil >= config::OIL_PATCHES_PER_BASE as usize
                    && queries.known_build_site(EntityKind::ResourceDepot, exact, &exclusions)
                        == KnownBuildSite::NoKnownConflict
                {
                    assert_eq!(tile, exact);
                }
                let mut crowded = snapshot.clone();
                let worker = crowded
                    .entities
                    .iter_mut()
                    .find(|e| e.owner == player && e.kind == "worker")
                    .unwrap();
                worker.x = center.0;
                worker.y = center.1;
                let crowded_frame = AiFrame::from_host(&start, &crowded, player, [], Some(&[1, 2]))
                    .unwrap()
                    .with_base_sites(&sites);
                let crowded_queries = WorldQueries::new(&crowded_frame);
                let depot_kind = EntityKind::ResourceDepot;
                assert_eq!(
                    crowded_queries.known_build_site(depot_kind, tile, &exclusions),
                    KnownBuildSite::KnownBlocked(super::super::KnownBuildSiteBlocker::CurrentUnit)
                );
                assert_eq!(
                    crowded_queries.base_depot_site(base.site.id, &exclusions),
                    Some(tile),
                    "movable own bodies must not erase the resource-centered recommendation"
                );
                exclusions.exclude(EntityKind::ResourceDepot, tile);
                let alternative = queries.base_depot_site(base.site.id, &exclusions).unwrap();
                assert_ne!(alternative, tile);
                assert_eq!(
                    queries.known_build_site(EntityKind::ResourceDepot, alternative, &exclusions),
                    KnownBuildSite::NoKnownConflict
                );
                let mut occupied = snapshot.clone();
                let mut depot = snapshot
                    .entities
                    .iter()
                    .find(|e| e.owner == player && e.kind == "resource_depot")
                    .unwrap()
                    .clone();
                depot.id = 999000;
                depot.x = center.0;
                depot.y = center.1;
                occupied.entities.push(depot.clone());
                let occupied_frame =
                    AiFrame::from_host(&start, &occupied, player, [], Some(&[1, 2]))
                        .unwrap()
                        .with_base_sites(&sites);
                assert!(WorldQueries::new(&occupied_frame)
                    .base_depot_site(base.site.id, &KnownBuildSiteExclusions::default())
                    .is_none());
                let mut remembered = snapshot.clone();
                remembered
                    .remembered_buildings
                    .push(rts_sim::protocol::RememberedBuildingView {
                        id: depot.id,
                        owner: 3 - player,
                        kind: "barracks".into(),
                        x: center.0,
                        y: center.1,
                        footprint: Vec::new(),
                        observed_tick: snapshot.tick,
                    });
                let hostile_frame =
                    AiFrame::from_host(&start, &remembered, player, [], Some(&[1, 2]))
                        .unwrap()
                        .with_base_sites(&sites);
                assert!(WorldQueries::new(&hostile_frame)
                    .base_depot_site(base.site.id, &KnownBuildSiteExclusions::default())
                    .is_none());
                // Explicitly rejected footprints eventually exhaust the bounded search.
                while let Some(tile) = queries.base_depot_site(base.site.id, &exclusions) {
                    assert!(exclusions.exclude(EntityKind::ResourceDepot, tile));
                }
            }
            let mut unreachable = sites.clone();
            for site in &mut unreachable {
                site.ground_distances_from_starts.remove(&player);
            }
            let frame = frame.with_base_sites(&unreachable);
            let queries = WorldQueries::new(&frame);
            assert!(queries.ranked_bases(player).iter().all(|base| queries
                .base_depot_site(base.site.id, &KnownBuildSiteExclusions::default())
                .is_none()));
        }
    }

    #[test]
    fn coverage_counts_only_patches_within_mining_range() {
        let range = config::MINING_ANCHOR_RANGE_TILES * 32.0;
        let score = resource_coverage(
            (0.0, 0.0),
            32,
            [
                (EntityKind::Steel, (range, 0.0)),
                (EntityKind::Oil, (0.0, range)),
                (EntityKind::Steel, (range + 1.0, 0.0)),
            ],
        );
        assert_eq!((score.steel, score.oil), (1, 1));
        assert_eq!(score.max_distance2, range * range);
        assert_eq!(score.sum_distance2, 2.0 * range * range);
    }
}
