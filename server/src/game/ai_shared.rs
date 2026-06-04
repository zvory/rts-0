use std::collections::BTreeSet;

use crate::config;
use crate::game::ai_core::facts;
use crate::game::ai_core::observation::AiResourceSummary;
use crate::game::entity::EntityKind;
use crate::game::entity::EntityStore;
use crate::protocol::{MapInfo, Snapshot};

pub(crate) const DEFAULT_BUILD_SEARCH_MIN_RADIUS: i32 = 3;
pub(crate) const DEFAULT_BUILD_SEARCH_MAX_RADIUS: i32 = 16;
pub(crate) const AI_DEFAULT_BUILDING_CLEARANCE_TILES: i32 = 1;
pub(crate) const AI_FACTORY_CLEARANCE_TILES: i32 = 2;

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
        EntityKind::Factory => AI_FACTORY_CLEARANCE_TILES,
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

    for radius in search.min_radius..=search.max_radius {
        if search.prefer_toward_center {
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
                    let map_dx = center_x - map_center.0;
                    let map_dy = center_y - map_center.1;
                    let start_dx = center_x - start.0 as f32;
                    let start_dy = center_y - start.1 as f32;
                    let map_distance = map_dx * map_dx + map_dy * map_dy;
                    let start_distance = start_dx * start_dx + start_dy * start_dy;
                    if fallback.is_none() {
                        fallback = Some((tx, ty));
                    }
                    let better = best_in_ring
                        .map(|(_, _, best_map_distance, best_start_distance)| {
                            map_distance < best_map_distance
                                || (map_distance == best_map_distance
                                    && start_distance < best_start_distance)
                        })
                        .unwrap_or(true);
                    if better {
                        best_in_ring = Some((tx, ty, map_distance, start_distance));
                    }
                }
            }
            if let Some((tx, ty, _, _)) = best_in_ring {
                return Some((tx, ty));
            }
            continue;
        }

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
    use crate::game::ai_core::actions::{ready_attack_wave, SpendBudget};
    use crate::protocol::{terrain, EntityView, Snapshot};
    use crate::rules;

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
    fn factory_spacing_requires_two_clear_tiles() {
        assert!(!footprints_respect_clearance(
            EntityKind::Factory,
            10,
            10,
            EntityKind::Depot,
            14,
            10,
        ));
        assert!(footprints_respect_clearance(
            EntityKind::Factory,
            10,
            10,
            EntityKind::Depot,
            15,
            10,
        ));
    }

    #[test]
    fn spend_budget_reserves_unit_and_building_costs() {
        let (tank_steel, tank_oil) = rules::economy::cost(EntityKind::Tank);
        let tank_supply = rules::economy::supply_cost(EntityKind::Tank);
        let (depot_steel, _) = rules::economy::cost(EntityKind::Depot);
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
        let in_range = (config::IC_RESOURCE_MAX_DIST_TILES - 0.25) * config::TILE_SIZE as f32;
        let out_of_range = (config::IC_RESOURCE_MAX_DIST_TILES + 2.0) * config::TILE_SIZE as f32;
        let snapshot = Snapshot {
            tick: 0,
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            entities: vec![
                EntityView {
                    id: 1,
                    owner: 0,
                    kind: EntityKind::Steel.to_protocol_str().to_string(),
                    x: hx + in_range,
                    y: hy,
                    hp: 1,
                    max_hp: 1,
                    state: "idle".into(),
                    facing: None,
                    weapon_facing: None,
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    latched_node: None,
                    remaining: Some(100),
                    target_id: None,
                    setup_state: None,
                    rally: None,
                    oil_used: None,
                },
                EntityView {
                    id: 2,
                    owner: 0,
                    kind: EntityKind::Steel.to_protocol_str().to_string(),
                    x: hx - in_range,
                    y: hy,
                    hp: 1,
                    max_hp: 1,
                    state: "idle".into(),
                    facing: None,
                    weapon_facing: None,
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    latched_node: None,
                    remaining: Some(100),
                    target_id: None,
                    setup_state: None,
                    rally: None,
                    oil_used: None,
                },
                EntityView {
                    id: 3,
                    owner: 0,
                    kind: EntityKind::Oil.to_protocol_str().to_string(),
                    x: hx,
                    y: hy + in_range,
                    hp: 1,
                    max_hp: 1,
                    state: "idle".into(),
                    facing: None,
                    weapon_facing: None,
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    latched_node: None,
                    remaining: Some(100),
                    target_id: None,
                    setup_state: None,
                    rally: None,
                    oil_used: None,
                },
                EntityView {
                    id: 4,
                    owner: 0,
                    kind: EntityKind::Steel.to_protocol_str().to_string(),
                    x: hx,
                    y: hy + out_of_range,
                    hp: 1,
                    max_hp: 1,
                    state: "idle".into(),
                    facing: None,
                    weapon_facing: None,
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    latched_node: None,
                    remaining: Some(100),
                    target_id: None,
                    setup_state: None,
                    rally: None,
                    oil_used: None,
                },
                EntityView {
                    id: 5,
                    owner: 0,
                    kind: EntityKind::Steel.to_protocol_str().to_string(),
                    x: hx,
                    y: hy - in_range,
                    hp: 1,
                    max_hp: 1,
                    state: "idle".into(),
                    facing: None,
                    weapon_facing: None,
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    latched_node: None,
                    remaining: Some(0),
                    target_id: None,
                    setup_state: None,
                    rally: None,
                    oil_used: None,
                },
            ],
            resource_deltas: Vec::new(),
            events: Vec::new(),
            player_resources: Vec::new(),
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
