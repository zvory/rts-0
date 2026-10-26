//! Which enemy base the raid goes for: one that lies open.
//!
//! AI 2.1's army walks to Jeff along the ground route between the mains and stands on it, and a
//! base answers only with the units inside its zone. So a base well off that route has nobody
//! near it, and one on it has the army beside it. Its natural sits on the route on every map
//! tested, so the raid goes for the third; a natural off the route would come first, as it is taken
//! first and surely has a Depot.

use super::*;
use crate::ai_core::map_analysis::AiTile;

/// A base this close to the enemy main sits inside the main's own defense zone and garrison.
const MIN_TARGET_FROM_ENEMY_MAIN_TILES: f32 = 35.0;
/// A base found defended is left alone this long.
const DEFENDED_SKIP_TICKS: u32 = config::TICK_HZ * 180;
/// Patches this close to another of a cluster belong to the same base. A base's patches come in
/// groups up to 9.4 tiles apart; different bases' patches stand at least 17.7 apart.
const BASE_MERGE_TILES: f32 = 12.0;
/// A base at least this far from the enemy army's route to Jeff lies open. AI 2.1's naturals sit
/// 9-21 tiles off it, with its fighters within 15 tiles of them 88-100% of 6:00-10:00 (41% on
/// Schone Tage); its thirds 27-59 tiles off, with fighters near them under half that time, and
/// never on Crossroads and Doppelganger.
pub(super) const OPEN_OFF_ROUTE_TILES: f32 = 24.0;

/// The enemy base to press: an open enemy base (`OPEN_OFF_ROUTE_TILES` off the enemy army's
/// route) on the enemy's side of the map, at least `MIN_TARGET_FROM_ENEMY_MAIN_TILES` from the
/// enemy main, not found empty (unless a Depot has been seen there since) and not found defended
/// lately. A base where a Depot has been seen comes first, then the one the enemy takes first.
/// Without the route, every base but the natural (the base nearest the enemy main) counts as open.
pub(super) fn target_site(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    state: &ThirdPressure,
) -> Option<EnemyBase> {
    let ts = tile(observation);
    let route = map_analysis.and_then(|analysis| analysis.base_route_tiles(observation.player_id));
    let tick = observation.tick;
    let enemy = enemy_start(observation)?;
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let bases = enemy_bases(observation, state);
    let natural = bases
        .iter()
        .map(|base| base.center)
        .min_by(|left, right| distance(*left, enemy).total_cmp(&distance(*right, enemy)));
    let same = |site: (i32, i32), base: &EnemyBase| {
        distance(unrounded(site), base.center) < SITE_SAME_TILES * ts
    };
    let mut candidates: Vec<EnemyBase> = bases
        .into_iter()
        .filter(|base| match route {
            Some(route) => {
                off_route_tiles(observation, route, base.depot.unwrap_or(base.center))
                    >= OPEN_OFF_ROUTE_TILES
            }
            None => Some(base.center) != natural,
        })
        .filter(|base| distance(base.center, enemy) < distance(base.center, own))
        .filter(|base| distance(base.center, enemy) >= MIN_TARGET_FROM_ENEMY_MAIN_TILES * ts)
        .filter(|base| base.depot.is_some() || !state.checked.iter().any(|site| same(*site, base)))
        .filter(|base| {
            !state.defended.iter().any(|(site, since)| {
                tick.saturating_sub(*since) < DEFENDED_SKIP_TICKS && same(*site, base)
            })
        })
        .collect();
    let first = candidates
        .iter()
        .position(|base| base.depot.is_some())
        .unwrap_or(0);
    (first < candidates.len()).then(|| candidates.swap_remove(first))
}

/// How far `point` lies from the nearest tile of `route`, in tiles.
pub(super) fn off_route_tiles(
    observation: &AiObservation,
    route: &[AiTile],
    point: (f32, f32),
) -> f32 {
    let ts = tile(observation);
    route
        .iter()
        .map(|step| {
            distance(
                point,
                ((step.x as f32 + 0.5) * ts, (step.y as f32 + 0.5) * ts),
            )
        })
        .fold(f32::MAX, f32::min)
        / ts
}

/// The enemy's bases beyond its main, in the order AI 2.1 takes them: it ranks sites by how much
/// nearer they are to itself than to its enemy, so its natural comes first and its third next.
/// Each is a cluster of Steel patches away from both starts, with the Oil patches beside them, and
/// the Depot seen within mining reach of them, if any.
pub(super) fn enemy_bases(observation: &AiObservation, state: &ThirdPressure) -> Vec<EnemyBase> {
    let ts = tile(observation);
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let Some(enemy) = enemy_start(observation) else {
        return Vec::new();
    };
    let start_radius = (config::START_RESOURCE_MAX_DIST_TILES + 1.5) * ts;
    let starts: Vec<(f32, f32)> = observation
        .players
        .iter()
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))
        .collect();
    let away_from_starts = |resource: &&AiResourceSummary| {
        resource.remaining > 0
            && starts
                .iter()
                .all(|start| distance((resource.x, resource.y), *start) > start_radius)
    };
    let mut clusters: Vec<Vec<(f32, f32)>> = Vec::new();
    for resource in observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel)
        .filter(away_from_starts)
    {
        let point = (resource.x, resource.y);
        let (joined, rest): (Vec<_>, Vec<_>) = clusters.into_iter().partition(|cluster| {
            cluster
                .iter()
                .any(|other| distance(*other, point) <= BASE_MERGE_TILES * ts)
        });
        let mut merged: Vec<(f32, f32)> = joined.into_iter().flatten().collect();
        merged.push(point);
        clusters = rest;
        clusters.push(merged);
    }
    let mut bases: Vec<EnemyBase> = clusters
        .into_iter()
        .map(|patches| {
            let n = patches.len() as f32;
            let center = patches
                .iter()
                .fold((0.0, 0.0), |sum, p| (sum.0 + p.0 / n, sum.1 + p.1 / n));
            EnemyBase {
                center,
                patches,
                depot: None,
            }
        })
        .collect();
    for oil in observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Oil)
        .filter(away_from_starts)
    {
        let point = (oil.x, oil.y);
        if let Some(base) = bases.iter_mut().find(|base| {
            base.patches
                .iter()
                .any(|patch| distance(*patch, point) <= BASE_MERGE_TILES * ts)
        }) {
            base.patches.push(point);
        }
    }
    for depot in &state.seen_depots {
        let depot = unrounded(*depot);
        if let Some(base) = bases
            .iter_mut()
            .find(|base| mines_from(observation, depot, &base.patches))
        {
            base.depot = Some(depot);
        }
    }
    let ratio = |point: (f32, f32)| distance(point, enemy) / distance(point, own).max(1.0);
    bases.sort_by(|left, right| ratio(left.center).total_cmp(&ratio(right.center)));
    bases
}

/// Whether a Depot at `at` mines one of `patches`: AI 2.1's Depots anchor every patch within
/// `MINING_ANCHOR_RANGE_TILES` of their centre.
pub(super) fn mines_from(
    observation: &AiObservation,
    at: (f32, f32),
    patches: &[(f32, f32)],
) -> bool {
    let reach = (config::MINING_ANCHOR_RANGE_TILES + 0.5) * tile(observation);
    patches.iter().any(|patch| distance(*patch, at) <= reach)
}
