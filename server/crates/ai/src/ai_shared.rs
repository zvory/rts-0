use std::collections::BTreeSet;

use crate::ai_core::facts;
use crate::ai_core::observation::AiResourceSummary;
use crate::config;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::entity::EntityStore;
use rts_sim::protocol::{MapInfo, Snapshot};

pub(crate) const DEFAULT_BUILD_SEARCH_MIN_RADIUS: i32 = 3;
pub(crate) const DEFAULT_BUILD_SEARCH_MAX_RADIUS: i32 = 16;
pub(crate) const FORWARD_PRODUCTION_BUILD_SEARCH_MAX_RADIUS: i32 = 18;
/// Turtle Gun Works only need a compact forward site: their mobile support units do not need the
/// same far-forward construction band as vehicle production.
pub(crate) const TURTLE_GUN_WORKS_BUILD_SEARCH_MAX_RADIUS: i32 =
    FORWARD_PRODUCTION_BUILD_SEARCH_MAX_RADIUS / 2;
pub(crate) const AI_DEFAULT_BUILDING_CLEARANCE_TILES: i32 = 1;
pub(crate) const AI_PRODUCTION_BUILDING_CLEARANCE_TILES: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BuildSearch {
    pub(crate) min_radius: i32,
    pub(crate) max_radius: i32,
    pub(crate) prefer_away_from_center: bool,
    pub(crate) prefer_toward_center: bool,
}

impl Default for BuildSearch {
    fn default() -> Self {
        Self {
            min_radius: DEFAULT_BUILD_SEARCH_MIN_RADIUS,
            max_radius: DEFAULT_BUILD_SEARCH_MAX_RADIUS,
            prefer_away_from_center: true,
            prefer_toward_center: false,
        }
    }
}

pub(crate) fn building_clearance_tiles(kind: EntityKind) -> i32 {
    match kind {
        EntityKind::Factory | EntityKind::Steelworks => AI_PRODUCTION_BUILDING_CLEARANCE_TILES,
        _ => AI_DEFAULT_BUILDING_CLEARANCE_TILES,
    }
}

pub(crate) fn footprints_respect_clearance(
    candidate_kind: EntityKind,
    candidate_tile_x: u32,
    candidate_tile_y: u32,
    existing_kind: EntityKind,
    existing_tile_x: u32,
    existing_tile_y: u32,
) -> bool {
    let Some(candidate_stats) = config::building_stats(candidate_kind) else {
        return false;
    };
    let Some(existing_stats) = config::building_stats(existing_kind) else {
        return false;
    };
    let clearance =
        building_clearance_tiles(candidate_kind).max(building_clearance_tiles(existing_kind));
    let candidate_left = candidate_tile_x as i32;
    let candidate_top = candidate_tile_y as i32;
    let candidate_right = candidate_left + candidate_stats.foot_w as i32 - 1;
    let candidate_bottom = candidate_top + candidate_stats.foot_h as i32 - 1;
    let existing_left = existing_tile_x as i32;
    let existing_top = existing_tile_y as i32;
    let existing_right = existing_left + existing_stats.foot_w as i32 - 1;
    let existing_bottom = existing_top + existing_stats.foot_h as i32 - 1;

    candidate_left > existing_right + clearance
        || existing_left > candidate_right + clearance
        || candidate_top > existing_bottom + clearance
        || existing_top > candidate_bottom + clearance
}

#[allow(dead_code)]
pub(crate) fn main_base_steel_saturation_target_from_entities(
    entities: &EntityStore,
    start_tile: (u32, u32),
) -> usize {
    facts::main_base_steel_saturation_target(
        start_tile,
        config::TILE_SIZE,
        entities
            .iter()
            .filter(|e| e.kind.is_node())
            .map(|e| AiResourceSummary {
                id: e.id,
                kind: e.kind,
                x: e.pos_x,
                y: e.pos_y,
                remaining: e.remaining().unwrap_or(0),
            }),
    )
}

#[allow(dead_code)]
pub(crate) fn main_base_steel_saturation_target_from_snapshot(
    map: &MapInfo,
    snapshot: &Snapshot,
    start_tile: (u32, u32),
) -> usize {
    facts::main_base_steel_saturation_target(
        start_tile,
        map.tile_size,
        snapshot.entities.iter().filter_map(|e| {
            let kind: EntityKind = e.kind.parse().ok()?;
            kind.is_node().then_some(AiResourceSummary {
                id: e.id,
                kind,
                x: e.x,
                y: e.y,
                remaining: e.remaining.unwrap_or(0),
            })
        }),
    )
}

/// Deterministically scan outward from `start`, preferring build sites that extend away from the
/// map center so the local base grows outward instead of clogging its interior.
#[allow(dead_code)]
pub(crate) fn find_build_spot_near_start(
    map_width: u32,
    map_height: u32,
    start: (u32, u32),
    building: EntityKind,
    skip: &BTreeSet<(u32, u32)>,
    placeable: impl FnMut(u32, u32) -> bool,
) -> Option<(u32, u32)> {
    find_build_spot_near_start_with(
        map_width,
        map_height,
        start,
        building,
        BuildSearch::default(),
        skip,
        placeable,
    )
}

pub(crate) fn find_build_spot_near_start_with(
    map_width: u32,
    map_height: u32,
    start: (u32, u32),
    building: EntityKind,
    search: BuildSearch,
    skip: &BTreeSet<(u32, u32)>,
    mut placeable: impl FnMut(u32, u32) -> bool,
) -> Option<(u32, u32)> {
    let stats = config::building_stats(building)?;
    let map_center = (map_width as f32 * 0.5, map_height as f32 * 0.5);
    let away = (start.0 as f32 - map_center.0, start.1 as f32 - map_center.1);
    let (sx, sy) = (start.0 as i32, start.1 as i32);
    let mut fallback = None;

    if search.prefer_toward_center {
        let mut best: Option<(u32, u32, f32, f32)> = None;
        for radius in search.min_radius..=search.max_radius {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let tx = sx + dx;
                    let ty = sy + dy;
                    if tx < 0 || ty < 0 {
                        continue;
                    }
                    let (tx, ty) = (tx as u32, ty as u32);
                    if skip.contains(&(tx, ty)) || !placeable(tx, ty) {
                        continue;
                    }
                    let center_x = tx as f32 + stats.foot_w as f32 * 0.5;
                    let center_y = ty as f32 + stats.foot_h as f32 * 0.5;
                    let map_dx = center_x - map_center.0;
                    let map_dy = center_y - map_center.1;
                    let start_dx = center_x - start.0 as f32;
                    let start_dy = center_y - start.1 as f32;
                    let map_distance = map_dx * map_dx + map_dy * map_dy;
                    let start_distance = start_dx * start_dx + start_dy * start_dy;
                    if fallback.is_none() {
                        fallback = Some((tx, ty));
                    }
                    let better = best
                        .map(|(_, _, best_map_distance, best_start_distance)| {
                            map_distance < best_map_distance
                                || (map_distance == best_map_distance
                                    && start_distance < best_start_distance)
                        })
                        .unwrap_or(true);
                    if better {
                        best = Some((tx, ty, map_distance, start_distance));
                    }
                }
            }
        }
        return best.map(|(tx, ty, _, _)| (tx, ty)).or(fallback);
    }

    for radius in search.min_radius..=search.max_radius {
        if !search.prefer_away_from_center {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let tx = sx + dx;
                    let ty = sy + dy;
                    if tx < 0 || ty < 0 {
                        continue;
                    }
                    let (tx, ty) = (tx as u32, ty as u32);
                    if !skip.contains(&(tx, ty)) && placeable(tx, ty) {
                        return Some((tx, ty));
                    }
                }
            }
            continue;
        }

        let mut best_in_ring: Option<(u32, u32, f32, f32)> = None;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let tx = sx + dx;
                let ty = sy + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let (tx, ty) = (tx as u32, ty as u32);
                if skip.contains(&(tx, ty)) || !placeable(tx, ty) {
                    continue;
                }
                let center_x = tx as f32 + stats.foot_w as f32 * 0.5;
                let center_y = ty as f32 + stats.foot_h as f32 * 0.5;
                let from_start = (center_x - start.0 as f32, center_y - start.1 as f32);
                let away_score = from_start.0 * away.0 + from_start.1 * away.1;
                let dist = from_start.0 * from_start.0 + from_start.1 * from_start.1;
                if fallback.is_none() {
                    fallback = Some((tx, ty));
                }
                let better = best_in_ring
                    .map(|(_, _, best_score, best_dist)| {
                        away_score > best_score || (away_score == best_score && dist < best_dist)
                    })
                    .unwrap_or(true);
                if better {
                    best_in_ring = Some((tx, ty, away_score, dist));
                }
            }
        }
        if let Some((tx, ty, away_score, _)) = best_in_ring {
            if away_score >= 0.0 {
                return Some((tx, ty));
            }
        }
    }

    fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::actions::{ready_attack_wave, SpendBudget};
    use rts_sim::protocol::{terrain, EntityView, Snapshot};

    fn resource_view(id: u32, kind: EntityKind, x: f32, y: f32, remaining: u32) -> EntityView {
        let mut view = EntityView::new(
            id,
            0,
            rts_sim::protocol::kind_to_wire(kind),
            x,
            y,
            1,
            1,
            "idle",
        );
        view.remaining = Some(remaining);
        view
    }

    #[test]
    fn prefers_tiles_away_from_map_center() {
        let spot = find_build_spot_near_start(
            20,
            20,
            (4, 4),
            EntityKind::Depot,
            &BTreeSet::new(),
            |tx, ty| matches!((tx, ty), (1, 4) | (7, 4)),
        );

        assert_eq!(spot, Some((1, 4)));
    }

    #[test]
    fn falls_back_to_best_available_tile_even_if_not_away_from_center() {
        let spot = find_build_spot_near_start(
            20,
            20,
            (4, 4),
            EntityKind::Depot,
            &BTreeSet::new(),
            |tx, ty| (tx, ty) == (7, 4),
        );

        assert_eq!(spot, Some((7, 4)));
    }

    #[test]
    fn nearest_first_mode_preserves_ring_scan_order() {
        let spot = find_build_spot_near_start_with(
            20,
            20,
            (4, 4),
            EntityKind::Depot,
            BuildSearch {
                min_radius: 2,
                max_radius: 16,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            &BTreeSet::new(),
            |tx, ty| matches!((tx, ty), (6, 4) | (2, 4)),
        );

        assert_eq!(spot, Some((2, 4)));
    }

    #[test]
    fn prefers_tiles_toward_map_center_when_requested() {
        let spot = find_build_spot_near_start_with(
            30,
            30,
            (4, 15),
            EntityKind::Factory,
            BuildSearch {
                min_radius: 2,
                max_radius: 16,
                prefer_away_from_center: false,
                prefer_toward_center: true,
            },
            &BTreeSet::new(),
            |tx, ty| matches!((tx, ty), (2, 15) | (6, 15)),
        );

        assert_eq!(spot, Some((6, 15)));
    }

    #[test]
    fn toward_center_mode_prefers_farther_front_tile_over_nearer_back_tile() {
        let spot = find_build_spot_near_start_with(
            30,
            30,
            (4, 15),
            EntityKind::Factory,
            BuildSearch {
                min_radius: 2,
                max_radius: 6,
                prefer_away_from_center: false,
                prefer_toward_center: true,
            },
            &BTreeSet::new(),
            |tx, ty| matches!((tx, ty), (2, 15) | (10, 15)),
        );

        assert_eq!(spot, Some((10, 15)));
    }

    #[test]
    fn production_buildings_require_two_clear_tiles() {
        for kind in [EntityKind::Factory, EntityKind::Steelworks] {
            assert!(!footprints_respect_clearance(
                kind,
                10,
                10,
                EntityKind::Depot,
                14,
                10,
            ));
            assert!(footprints_respect_clearance(
                kind,
                10,
                10,
                EntityKind::Depot,
                15,
                10,
            ));
        }
    }

    #[test]
    fn spend_budget_reserves_unit_and_building_costs() {
        let (tank_steel, tank_oil) = rts_rules::economy::cost(EntityKind::Tank);
        let tank_supply = rts_rules::economy::supply_cost(EntityKind::Tank);
        let (depot_steel, _) = rts_rules::economy::cost(EntityKind::Depot);
        let mut budget = SpendBudget::new(tank_steel + depot_steel, tank_oil, 0, tank_supply + 1);

        assert!(budget.can_afford_unit(EntityKind::Tank));
        assert!(budget.reserve_unit(EntityKind::Tank));
        assert_eq!(budget.free_supply(), 1);
        assert!(!budget.can_afford_building(EntityKind::Factory));
        assert!(!budget.reserve_building(EntityKind::Factory));
        assert!(budget.reserve_building(EntityKind::Depot));
        assert!(!budget.can_afford_building(EntityKind::Depot));
    }

    #[test]
    fn snapshot_saturation_target_counts_only_nearby_nonempty_steel() {
        let map = MapInfo {
            width: 64,
            height: 64,
            tile_size: config::TILE_SIZE,
            terrain: vec![terrain::GRASS; 64 * 64],
            resources: vec![],
        };
        let (hx, hy) = (
            10.5 * config::TILE_SIZE as f32,
            20.5 * config::TILE_SIZE as f32,
        );
        let in_range = (config::CC_RESOURCE_MAX_DIST_TILES - 0.25) * config::TILE_SIZE as f32;
        let out_of_range = (config::CC_RESOURCE_MAX_DIST_TILES + 2.0) * config::TILE_SIZE as f32;
        let snapshot = Snapshot {
            tick: 0,
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            entities: vec![
                resource_view(1, EntityKind::Steel, hx + in_range, hy, 100),
                resource_view(2, EntityKind::Steel, hx - in_range, hy, 100),
                resource_view(3, EntityKind::Oil, hx, hy + in_range, 100),
                resource_view(4, EntityKind::Steel, hx, hy + out_of_range, 100),
                resource_view(5, EntityKind::Steel, hx, hy - in_range, 0),
            ],
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            ability_objects: Vec::new(),
            trenches: Vec::new(),
            visible_tiles: Vec::new(),
            remembered_buildings: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: Vec::new(),
            net_status: rts_sim::protocol::SnapshotNetStatus::default(),
        };

        assert_eq!(
            main_base_steel_saturation_target_from_snapshot(&map, &snapshot, (10, 20)),
            2
        );
    }

    #[test]
    fn ready_attack_wave_requires_threshold() {
        assert_eq!(ready_attack_wave([7_u32, 9], 3, Some), None);
        assert_eq!(ready_attack_wave([7_u32, 9], 2, Some), Some(vec![7, 9]));
    }
}
