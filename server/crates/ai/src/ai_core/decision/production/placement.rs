//! Where the live Jeff (`JEFFS_AI_ID` only) puts its production buildings on every map: behind its
//! HQ on the enemy's walk in, and never in reach of enemy Tanks.
//!
//! In the Wald des Todes and Doppelganger losses every production building Jeff lost before its
//! final collapse stood ahead of its HQ. The Factories stood 8-9 tiles out, within 2 tiles of the
//! line to the enemy HQ. AI Tanks shelled them from 7-11 tiles while the home Tanks, 8 tiles out and
//! seeing 9, could not see the shooters. Jeff then rebuilt on the same spot (one game lost 6
//! Factories in 39 s).

use super::super::geometry::{building_center, dist2, squared};
use super::super::jeff::site_depth;
use super::super::tank_micro::{stored, world, SeenTank, TankMicro};
use super::super::*;
use super::{
    build_search_for_kind, try_build_kind_with_search, CROSSROADS_SHELTERED_DEPTH_TILES,
    CROSSROADS_SHELTER_SEARCH_MAX_RADIUS,
};

/// Off Crossroads, a site at least this many tiles deeper than the HQ on the enemy's walk comes
/// first. A home Tank 8 tiles out sees to 17 tiles ahead of the HQ and a parked enemy Tank hits from
/// 14, so a building more than about 3 tiles ahead of the HQ can be shelled from out of sight.
const SHELTERED_DEPTH_TILES: f32 = 3.0;
/// No production building goes up with a remembered enemy Tank this close to its footprint: a
/// parked Tank hits from 14 tiles.
const TANK_CLEARANCE_TILES: f32 = 16.0;
/// A production building lost with a remembered enemy Tank this close to its footprint was shelled.
const SHELLED_TANK_TILES: f32 = 16.0;
/// No production building goes up with its centre this close to a shelled site...
const SHELLED_SITE_TILES: f32 = 8.0;
/// ...until no enemy Tank has been seen within `SHELLED_TANK_TILES` of the site for this long.
const SHELLED_MEMORY_TICKS: u32 = config::TICK_HZ * 60;

/// Jeff's production buildings, and the sites enemy Tanks destroyed them on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Shelled {
    /// Each production building last decision: its kind and centre.
    buildings: BTreeMap<u32, (EntityKind, (i32, i32))>,
    /// Each shelled site: the lost building's kind and centre, and when an enemy Tank was last
    /// seen near it.
    sites: Vec<(EntityKind, (i32, i32), u32)>,
}

/// The buildings Jeff keeps behind its HQ.
fn sheltered_kind(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Barracks
            | EntityKind::TrainingCentre
            | EntityKind::EngineeringComplex
            | EntityKind::Factory
            | EntityKind::Steelworks
    )
}

/// Notes the production buildings lost since the last decision with an enemy Tank near, and keeps
/// each shelled site while enemy Tanks are still seen around it. Runs after the Tank memory update.
pub(in crate::ai_core::decision) fn observe(
    observation: &AiObservation,
    enemy_tanks: &BTreeMap<u32, SeenTank>,
    shelled: &mut Shelled,
) {
    let tick = observation.tick;
    let tile_size = observation.map.tile_size as f32;
    let current: BTreeMap<u32, (EntityKind, (i32, i32))> = observation
        .owned
        .iter()
        .filter(|entity| sheltered_kind(entity.kind) && entity.hp > 0)
        .map(|entity| (entity.id, (entity.kind, stored((entity.x, entity.y)))))
        .collect();
    let remembered: Vec<(i32, i32)> = enemy_tanks.values().map(|seen| seen.pos).collect();
    let seen_now: Vec<(i32, i32)> = enemy_tanks
        .values()
        .filter(|seen| seen.last_seen == tick)
        .map(|seen| seen.pos)
        .collect();
    let near = |kind: EntityKind, center: (i32, i32), tanks: &[(i32, i32)]| {
        tanks.iter().any(|tank| {
            footprint_distance2(kind, world(center), world(*tank), tile_size)
                <= squared(SHELLED_TANK_TILES * tile_size)
        })
    };
    for (id, (kind, center)) in &shelled.buildings {
        if !current.contains_key(id) && near(*kind, *center, &remembered) {
            shelled.sites.push((*kind, *center, tick));
        }
    }
    for (kind, center, last_near) in &mut shelled.sites {
        if near(*kind, *center, &seen_now) {
            *last_near = tick;
        }
    }
    shelled
        .sites
        .retain(|(_, _, last_near)| tick.saturating_sub(*last_near) <= SHELLED_MEMORY_TICKS);
    shelled.buildings = current;
}

/// Builds `kind` for the live Jeff:
/// - a sheltered kind (all but its first Barracks off Crossroads, which trains the opening's
///   Machine Gunners) goes first to the nearest site deeper than the HQ on the enemy's walk by
///   `SHELTERED_DEPTH_TILES` (Crossroads: `CROSSROADS_SHELTERED_DEPTH_TILES`), else to the nearest
///   one no nearer the enemy than the HQ, else to the usual site;
/// - no site is in reach of a remembered enemy Tank or on a shelled site, so with Tanks waiting
///   around the base nothing is built until they leave.
#[allow(clippy::too_many_arguments)]
pub(in crate::ai_core::decision) fn try_build_live<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    actions: &mut AiActionContext<'_>,
    builder_pools: &[&[u32]],
    profile: &AiProfile,
    kind: EntityKind,
    build_search: ai_shared::BuildSearch,
    (map_analysis, micro): (Option<&AiMapAnalysis>, &TankMicro),
    placeable: &mut F,
) -> Option<actions::BuildAction>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let mut safe = |building: EntityKind, tile_x: u32, tile_y: u32| {
        placeable(building, tile_x, tile_y)
            && clear_of_tanks(observation, micro, building, tile_x, tile_y)
    };
    let crossroads = defense::crossroads_wall_aware_approach_direction(observation).is_some();
    let first_barracks = kind == EntityKind::Barracks && facts.building_count(kind) == 0;
    let analysis = map_analysis.filter(|_| sheltered_kind(kind) && (crossroads || !first_barracks));
    if let Some(analysis) = analysis {
        let usual = build_search_for_kind(build_search, profile, kind);
        // Nearest first: the rings grow outward from the HQ with no pull toward the map centre.
        let sheltered_search = ai_shared::BuildSearch {
            max_radius: usual.max_radius.max(CROSSROADS_SHELTER_SEARCH_MAX_RADIUS),
            prefer_away_from_center: false,
            prefer_toward_center: false,
            ..usual
        };
        let deepest = if crossroads {
            CROSSROADS_SHELTERED_DEPTH_TILES
        } else {
            SHELTERED_DEPTH_TILES
        };
        for min_depth in [deepest, 0.0] {
            let built = try_build_kind_with_search(
                observation,
                facts,
                actions,
                builder_pools,
                profile,
                kind,
                sheltered_search,
                &mut |building, tile_x, tile_y| {
                    safe(building, tile_x, tile_y)
                        && site_depth(observation, analysis, building, tile_x, tile_y)
                            .is_some_and(|depth| depth >= min_depth)
                },
            );
            if built.is_some() {
                return built;
            }
        }
    }
    try_build_kind(
        observation,
        facts,
        actions,
        builder_pools,
        profile,
        kind,
        build_search,
        &mut safe,
    )
}

/// Whether a `kind` building at this site is out of reach of every remembered enemy Tank and off
/// every shelled site.
fn clear_of_tanks(
    observation: &AiObservation,
    micro: &TankMicro,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    let tile_size = observation.map.tile_size;
    let Some(center) = building_center((tile_x, tile_y), kind, tile_size) else {
        return false;
    };
    let tile_size = tile_size as f32;
    micro.enemy_tanks.values().all(|seen| {
        footprint_distance2(kind, center, world(seen.pos), tile_size)
            > squared(TANK_CLEARANCE_TILES * tile_size)
    }) && micro.shelled.sites.iter().all(|(_, site, _)| {
        dist2(center.0, center.1, world(*site).0, world(*site).1)
            > squared(SHELLED_SITE_TILES * tile_size)
    })
}

/// Squared distance from `point` to the footprint of a `kind` building centred on `center`.
fn footprint_distance2(
    kind: EntityKind,
    center: (f32, f32),
    point: (f32, f32),
    tile_size: f32,
) -> f32 {
    let (half_w, half_h) = config::building_stats(kind).map_or((0.0, 0.0), |stats| {
        (
            stats.foot_w as f32 * tile_size * 0.5,
            stats.foot_h as f32 * tile_size * 0.5,
        )
    });
    let dx = ((point.0 - center.0).abs() - half_w).max(0.0);
    let dy = ((point.1 - center.1).abs() - half_h).max(0.0);
    dx * dx + dy * dy
}

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
