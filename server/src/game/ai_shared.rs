use std::collections::BTreeSet;

use crate::config;
use crate::game::entity::EntityKind;
use crate::game::entity::EntityStore;
use crate::protocol::{MapInfo, Snapshot};

pub(crate) const DEFAULT_BUILD_SEARCH_MIN_RADIUS: i32 = 3;
pub(crate) const DEFAULT_BUILD_SEARCH_MAX_RADIUS: i32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BuildSearch {
    pub(crate) min_radius: i32,
    pub(crate) max_radius: i32,
    pub(crate) prefer_away_from_center: bool,
}

impl Default for BuildSearch {
    fn default() -> Self {
        Self {
            min_radius: DEFAULT_BUILD_SEARCH_MIN_RADIUS,
            max_radius: DEFAULT_BUILD_SEARCH_MAX_RADIUS,
            prefer_away_from_center: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpendBudget {
    steel: u32,
    oil: u32,
    free_supply: u32,
}

impl SpendBudget {
    pub(crate) fn new(steel: u32, oil: u32, supply_used: u32, supply_cap: u32) -> Self {
        Self {
            steel,
            oil,
            free_supply: supply_cap.saturating_sub(supply_used),
        }
    }

    pub(crate) fn steel(&self) -> u32 {
        self.steel
    }

    pub(crate) fn free_supply(&self) -> u32 {
        self.free_supply
    }

    pub(crate) fn can_afford_unit(&self, kind: EntityKind) -> bool {
        let Some(stats) = config::unit_stats(kind) else {
            return false;
        };
        self.steel >= stats.cost_steel
            && self.oil >= stats.cost_oil
            && self.free_supply >= stats.supply
    }

    pub(crate) fn reserve_unit(&mut self, kind: EntityKind) -> bool {
        let Some(stats) = config::unit_stats(kind) else {
            return false;
        };
        self.reserve_cost(stats.cost_steel, stats.cost_oil, stats.supply)
    }

    pub(crate) fn can_afford_building(&self, kind: EntityKind) -> bool {
        let Some(stats) = config::building_stats(kind) else {
            return false;
        };
        self.steel >= stats.cost_steel && self.oil >= stats.cost_oil
    }

    pub(crate) fn reserve_building(&mut self, kind: EntityKind) -> bool {
        let Some(stats) = config::building_stats(kind) else {
            return false;
        };
        self.reserve_cost(stats.cost_steel, stats.cost_oil, 0)
    }

    fn reserve_cost(&mut self, steel: u32, oil: u32, supply: u32) -> bool {
        if self.steel < steel || self.oil < oil || self.free_supply < supply {
            return false;
        }
        self.steel -= steel;
        self.oil -= oil;
        self.free_supply -= supply;
        true
    }
}

pub(crate) fn main_base_steel_saturation_target_from_entities(
    entities: &EntityStore,
    start_tile: (u32, u32),
) -> usize {
    steel_saturation_target(
        start_tile,
        config::TILE_SIZE,
        entities
            .iter()
            .filter_map(|e| Some((e.kind, e.pos_x, e.pos_y, e.remaining()?))),
    )
}

pub(crate) fn main_base_steel_saturation_target_from_snapshot(
    map: &MapInfo,
    snapshot: &Snapshot,
    start_tile: (u32, u32),
) -> usize {
    steel_saturation_target(
        start_tile,
        map.tile_size,
        snapshot
            .entities
            .iter()
            .filter_map(|e| Some((e.kind.parse().ok()?, e.x, e.y, e.remaining?))),
    )
}

pub(crate) fn ready_attack_wave<T>(
    units: impl IntoIterator<Item = T>,
    min_size: usize,
    mut select: impl FnMut(T) -> Option<u32>,
) -> Option<Vec<u32>> {
    let ids: Vec<u32> = units.into_iter().filter_map(&mut select).collect();
    (ids.len() >= min_size).then_some(ids)
}

/// Deterministically scan outward from `start`, preferring build sites that extend away from the
/// map center so the local base grows outward instead of clogging its interior.
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

fn steel_saturation_target(
    start_tile: (u32, u32),
    tile_size: u32,
    steel_nodes: impl IntoIterator<Item = (EntityKind, f32, f32, u32)>,
) -> usize {
    let (hx, hy) = (
        start_tile.0 as f32 * tile_size as f32 + tile_size as f32 * 0.5,
        start_tile.1 as f32 * tile_size as f32 + tile_size as f32 * 0.5,
    );
    let max_dist_px = (config::IC_RESOURCE_MAX_DIST_TILES + 0.5) * tile_size as f32;
    let max_dist2 = max_dist_px * max_dist_px;
    steel_nodes
        .into_iter()
        .filter(|(kind, _, _, remaining)| *kind == EntityKind::Steel && *remaining > 0)
        .filter(|(_, x, y, _)| {
            let dx = *x - hx;
            let dy = *y - hy;
            dx * dx + dy * dy <= max_dist2
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{terrain, EntityView, Snapshot};

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
            },
            &BTreeSet::new(),
            |tx, ty| matches!((tx, ty), (6, 4) | (2, 4)),
        );

        assert_eq!(spot, Some((2, 4)));
    }

    #[test]
    fn spend_budget_reserves_unit_and_building_costs() {
        let mut budget = SpendBudget::new(150, 50, 2, 5);

        assert!(budget.can_afford_unit(EntityKind::Tank));
        assert!(budget.reserve_unit(EntityKind::Tank));
        assert_eq!(budget.steel(), 50);
        assert_eq!(budget.free_supply(), 1);
        assert!(!budget.can_afford_building(EntityKind::TankFactory));
        assert!(!budget.reserve_building(EntityKind::TankFactory));
        assert!(budget.reserve_building(EntityKind::Depot));
        assert_eq!(budget.steel(), 0);
    }

    #[test]
    fn snapshot_saturation_target_counts_only_nearby_nonempty_steel() {
        let map = MapInfo {
            width: 64,
            height: 64,
            tile_size: config::TILE_SIZE,
            terrain: vec![terrain::GRASS; 64 * 64],
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
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    carrying: None,
                    carrying_kind: None,
                    latched_node: None,
                    remaining: Some(100),
                    target_id: None,
                    setup_state: None,
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
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    carrying: None,
                    carrying_kind: None,
                    latched_node: None,
                    remaining: Some(100),
                    target_id: None,
                    setup_state: None,
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
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    carrying: None,
                    carrying_kind: None,
                    latched_node: None,
                    remaining: Some(100),
                    target_id: None,
                    setup_state: None,
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
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    carrying: None,
                    carrying_kind: None,
                    latched_node: None,
                    remaining: Some(100),
                    target_id: None,
                    setup_state: None,
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
                    prod_kind: None,
                    prod_progress: None,
                    prod_queue: None,
                    build_progress: None,
                    carrying: None,
                    carrying_kind: None,
                    latched_node: None,
                    remaining: Some(0),
                    target_id: None,
                    setup_state: None,
                },
            ],
            events: Vec::new(),
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
