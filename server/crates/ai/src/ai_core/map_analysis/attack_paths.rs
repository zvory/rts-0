use std::cmp::Ordering;
use std::collections::BinaryHeap;

use serde::Serialize;

use super::{tile_distance2, tile_index, AiMapAnalysis, AiTile};

const ORTHOGONAL_COST: u32 = 1_000;
const DIAGONAL_COST: u32 = 1_414;
const PREVIOUS_ROUTE_PENALTY: u32 = 7_500;
const NEAR_ROUTE_PENALTY: u32 = 1_800;
const ROUTE_PENALTY_RADIUS: i32 = 4;
const DESIRED_ROUTE_CLEARANCE_TILES: u16 = 5;
const LOW_CLEARANCE_STEP_PENALTY: u32 = 450;
const SHARED_CORRIDOR_RADIUS_TILES: i64 = 6;
const EXPANSION_DISTANCE_MULTIPLIER: f32 = 1.75;
const EXPANSION_DISTANCE_MARGIN_TILES: f32 = 24.0;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiDefendedBase {
    pub(crate) id: u32,
    pub(crate) label: String,
    pub(crate) tile_x: u32,
    pub(crate) tile_y: u32,
    pub(crate) resource_cluster_id: Option<u32>,
    pub(crate) added_attack_vectors: u32,
    pub(crate) open_approaches: u32,
    pub(crate) shared_route_percent: u32,
    pub(crate) distance_from_home_tiles: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiAttackPath {
    pub(crate) id: u32,
    pub(crate) attacker_player_id: u32,
    pub(crate) defended_base_id: u32,
    pub(crate) alternative_rank: u32,
    pub(crate) length_tiles: f32,
    pub(crate) bottleneck_width_tiles: u16,
    pub(crate) bottleneck_tile_x: u32,
    pub(crate) bottleneck_tile_y: u32,
    pub(crate) intercept_tile_x: u32,
    pub(crate) intercept_tile_y: u32,
    pub(crate) crossed_choke_ids: Vec<u32>,
    pub(crate) tiles: Vec<[u32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiAttackPathReport {
    pub(crate) defender_player_id: u32,
    pub(crate) requested_base_count: usize,
    pub(crate) bases: Vec<AiDefendedBase>,
    pub(crate) paths: Vec<AiAttackPath>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueueState {
    cost: u32,
    index: usize,
}

impl Ord for QueueState {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| other.index.cmp(&self.index))
    }
}

impl PartialOrd for QueueState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) fn analyze_attack_paths(
    analysis: &AiMapAnalysis,
    defender_player_id: u32,
    base_count: usize,
    paths_per_base: usize,
) -> AiAttackPathReport {
    let mut warnings = Vec::new();
    let Some(defender) = analysis
        .starts
        .iter()
        .find(|start| start.player_id == defender_player_id)
    else {
        return AiAttackPathReport {
            defender_player_id,
            requested_base_count: base_count,
            bases: Vec::new(),
            paths: Vec::new(),
            warnings: vec![format!("player {defender_player_id} has no authored start")],
        };
    };

    let attackers = analysis
        .starts
        .iter()
        .filter(|start| {
            start.team_id != defender.team_id && start.component_id == defender.component_id
        })
        .collect::<Vec<_>>();
    let bases = select_defended_bases(analysis, defender, &attackers, base_count.max(1));
    if bases.len() < base_count {
        warnings.push(format!(
            "requested {base_count} bases but only {} reachable base sites were found",
            bases.len()
        ));
    }
    if attackers.is_empty() {
        warnings.push("no reachable enemy start was found".to_string());
    }

    let mut paths = Vec::new();
    let mut next_path_id = 0_u32;
    for base in &bases {
        let goal = AiTile::new(base.tile_x, base.tile_y);
        for attacker in &attackers {
            let start = attacker.start_tile;
            let mut penalties = vec![0_u32; analysis.passable.len()];
            for alternative_rank in 1..=paths_per_base.max(1) {
                let Some(route) = shortest_path(analysis, start, goal, &penalties) else {
                    if alternative_rank == 1 {
                        warnings.push(format!(
                            "no route from player {} to {}",
                            attacker.player_id, base.label
                        ));
                    }
                    break;
                };
                if alternative_rank > 1
                    && paths.iter().any(|existing: &AiAttackPath| {
                        existing.attacker_player_id == attacker.player_id
                            && existing.defended_base_id == base.id
                            && route_similarity(&route, &existing.tiles) > 0.82
                    })
                {
                    apply_route_penalty(analysis, &route, &mut penalties);
                    continue;
                }
                let (bottleneck, bottleneck_width) = route_bottleneck(analysis, &route);
                let crossed_choke_ids = crossed_chokes(analysis, &route);
                let intercept = intercept_tile(analysis, &route, &crossed_choke_ids);
                paths.push(AiAttackPath {
                    id: next_path_id,
                    attacker_player_id: attacker.player_id,
                    defended_base_id: base.id,
                    alternative_rank: alternative_rank as u32,
                    length_tiles: route_length(&route),
                    bottleneck_width_tiles: bottleneck_width,
                    bottleneck_tile_x: bottleneck.x,
                    bottleneck_tile_y: bottleneck.y,
                    intercept_tile_x: intercept.x,
                    intercept_tile_y: intercept.y,
                    crossed_choke_ids,
                    tiles: route.iter().map(|tile| [tile.x, tile.y]).collect(),
                });
                next_path_id = next_path_id.saturating_add(1);
                apply_route_penalty(analysis, &route, &mut penalties);
            }
        }
    }

    AiAttackPathReport {
        defender_player_id,
        requested_base_count: base_count,
        bases,
        paths,
        warnings,
    }
}

fn select_defended_bases(
    analysis: &AiMapAnalysis,
    defender: &super::AiStartMapping,
    attackers: &[&super::AiStartMapping],
    base_count: usize,
) -> Vec<AiDefendedBase> {
    let mut bases = vec![AiDefendedBase {
        id: 1,
        label: "home".to_string(),
        tile_x: defender.start_tile.x,
        tile_y: defender.start_tile.y,
        resource_cluster_id: defender.nearest_resource_cluster_id,
        added_attack_vectors: 0,
        open_approaches: 0,
        shared_route_percent: 100,
        distance_from_home_tiles: 0.0,
    }];
    if base_count == 1 {
        return bases;
    }

    let home_routes = attackers
        .iter()
        .filter_map(|attacker| {
            shortest_path(
                analysis,
                attacker.start_tile,
                defender.start_tile,
                &vec![0; analysis.passable.len()],
            )
        })
        .collect::<Vec<_>>();
    let mut expansions = analysis
        .resource_clusters
        .iter()
        .filter(|cluster| {
            cluster.component_id == defender.component_id
                && Some(cluster.id) != defender.nearest_resource_cluster_id
                && !analysis
                    .starts
                    .iter()
                    .any(|start| start.nearest_resource_cluster_id == Some(cluster.id))
        })
        .filter_map(|cluster| {
            let metrics = expansion_defense_metrics(
                analysis,
                defender.start_tile,
                cluster.center_tile,
                attackers,
                &home_routes,
            )?;
            Some((cluster, metrics))
        })
        .collect::<Vec<_>>();
    let nearest_distance = expansions
        .iter()
        .map(|(_, metrics)| metrics.distance_milli_tiles)
        .min()
        .unwrap_or(0);
    let max_distance = ((nearest_distance as f32 * EXPANSION_DISTANCE_MULTIPLIER)
        .max(nearest_distance as f32 + EXPANSION_DISTANCE_MARGIN_TILES * 1_000.0))
        as u32;
    expansions.retain(|(_, metrics)| metrics.distance_milli_tiles <= max_distance);
    expansions.sort_by_key(|(cluster, metrics)| expansion_rank_key(*metrics, cluster.id));
    for (cluster, metrics) in expansions.into_iter().take(base_count.saturating_sub(1)) {
        let id = bases.len() as u32 + 1;
        bases.push(AiDefendedBase {
            id,
            label: format!("expansion-{id}"),
            tile_x: cluster.center_tile.x,
            tile_y: cluster.center_tile.y,
            resource_cluster_id: Some(cluster.id),
            added_attack_vectors: metrics.added_attack_vectors,
            open_approaches: metrics.open_approaches,
            shared_route_percent: metrics.shared_route_percent,
            distance_from_home_tiles: metrics.distance_milli_tiles as f32 / 1_000.0,
        });
    }
    bases
}

#[derive(Clone, Copy, Debug)]
struct ExpansionDefenseMetrics {
    added_attack_vectors: u32,
    open_approaches: u32,
    shared_route_percent: u32,
    widest_bottleneck: u16,
    distance_milli_tiles: u32,
}

fn expansion_rank_key(
    metrics: ExpansionDefenseMetrics,
    cluster_id: u32,
) -> (u32, u32, std::cmp::Reverse<u32>, u16, u32, u32) {
    (
        metrics.added_attack_vectors,
        metrics.open_approaches,
        std::cmp::Reverse(metrics.shared_route_percent),
        metrics.widest_bottleneck,
        metrics.distance_milli_tiles,
        cluster_id,
    )
}

fn expansion_defense_metrics(
    analysis: &AiMapAnalysis,
    home: AiTile,
    candidate: AiTile,
    attackers: &[&super::AiStartMapping],
    home_routes: &[Vec<AiTile>],
) -> Option<ExpansionDefenseMetrics> {
    let mut candidate_routes = Vec::new();
    let mut open_approaches = 0_u32;
    let mut widest_bottleneck = 0_u16;
    for attacker in attackers {
        let mut penalties = vec![0_u32; analysis.passable.len()];
        for _ in 0..3 {
            let Some(route) = shortest_path(analysis, attacker.start_tile, candidate, &penalties)
            else {
                break;
            };
            let crossed = crossed_chokes(analysis, &route);
            if crossed.is_empty() {
                open_approaches = open_approaches.saturating_add(1);
            }
            widest_bottleneck = widest_bottleneck.max(route_bottleneck(analysis, &route).1);
            apply_route_penalty(analysis, &route, &mut penalties);
            candidate_routes.push(route);
        }
    }
    if candidate_routes.is_empty() {
        return None;
    }
    let similarities = candidate_routes
        .iter()
        .map(|route| {
            home_routes
                .iter()
                .map(|home| route_tile_similarity(route, home))
                .fold(0.0_f32, f32::max)
        })
        .collect::<Vec<_>>();
    let added_attack_vectors = similarities
        .iter()
        .filter(|similarity| **similarity < 0.30)
        .count() as u32;
    let shared_route_percent =
        (similarities.iter().sum::<f32>() / similarities.len() as f32 * 100.0).round() as u32;
    let distance_milli_tiles = (tile_distance2(candidate, home) as f32)
        .sqrt()
        .mul_add(1_000.0, 0.0) as u32;
    Some(ExpansionDefenseMetrics {
        added_attack_vectors,
        open_approaches,
        shared_route_percent,
        widest_bottleneck,
        distance_milli_tiles,
    })
}

fn route_tile_similarity(route: &[AiTile], other: &[AiTile]) -> f32 {
    if route.is_empty() || other.is_empty() {
        return 0.0;
    }
    let shared = route
        .iter()
        .filter(|tile| {
            other.iter().any(|other_tile| {
                let dx = i64::from(tile.x) - i64::from(other_tile.x);
                let dy = i64::from(tile.y) - i64::from(other_tile.y);
                dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
                    <= SHARED_CORRIDOR_RADIUS_TILES.saturating_pow(2)
            })
        })
        .count();
    shared as f32 / route.len().min(other.len()) as f32
}

fn shortest_path(
    analysis: &AiMapAnalysis,
    start: AiTile,
    goal: AiTile,
    penalties: &[u32],
) -> Option<Vec<AiTile>> {
    let start_index = tile_index(analysis.width, analysis.height, start.x, start.y)?;
    let goal_index = tile_index(analysis.width, analysis.height, goal.x, goal.y)?;
    if !analysis.passable.get(start_index).copied().unwrap_or(false)
        || !analysis.passable.get(goal_index).copied().unwrap_or(false)
    {
        return None;
    }
    let count = analysis.passable.len();
    let mut costs = vec![u32::MAX; count];
    let mut previous = vec![None; count];
    let mut queue = BinaryHeap::new();
    costs[start_index] = 0;
    queue.push(QueueState {
        cost: 0,
        index: start_index,
    });

    while let Some(QueueState { cost, index }) = queue.pop() {
        if index == goal_index {
            break;
        }
        if costs[index] != cost {
            continue;
        }
        let tile = AiTile::new(index as u32 % analysis.width, index as u32 / analysis.width);
        for neighbor in
            super::passable_neighbors(analysis.width, analysis.height, &analysis.passable, tile)
        {
            let Some(neighbor_index) =
                tile_index(analysis.width, analysis.height, neighbor.x, neighbor.y)
            else {
                continue;
            };
            let diagonal = neighbor.x != tile.x && neighbor.y != tile.y;
            let step = if diagonal {
                DIAGONAL_COST
            } else {
                ORTHOGONAL_COST
            };
            let clearance_penalty = u32::from(
                DESIRED_ROUTE_CLEARANCE_TILES
                    .saturating_sub(analysis.clearance.get(neighbor_index).copied().unwrap_or(0)),
            )
            .saturating_mul(LOW_CLEARANCE_STEP_PENALTY);
            let next_cost = cost
                .saturating_add(step)
                .saturating_add(clearance_penalty)
                .saturating_add(penalties.get(neighbor_index).copied().unwrap_or(0));
            if next_cost < costs[neighbor_index] {
                costs[neighbor_index] = next_cost;
                previous[neighbor_index] = Some(index);
                queue.push(QueueState {
                    cost: next_cost,
                    index: neighbor_index,
                });
            }
        }
    }
    if costs[goal_index] == u32::MAX {
        return None;
    }
    let mut indices = vec![goal_index];
    let mut cursor = goal_index;
    while cursor != start_index {
        cursor = previous.get(cursor).copied().flatten()?;
        indices.push(cursor);
    }
    indices.reverse();
    Some(
        indices
            .into_iter()
            .map(|index| AiTile::new(index as u32 % analysis.width, index as u32 / analysis.width))
            .collect(),
    )
}

fn apply_route_penalty(analysis: &AiMapAnalysis, route: &[AiTile], penalties: &mut [u32]) {
    for tile in route {
        for dy in -ROUTE_PENALTY_RADIUS..=ROUTE_PENALTY_RADIUS {
            for dx in -ROUTE_PENALTY_RADIUS..=ROUTE_PENALTY_RADIUS {
                let x = tile.x as i32 + dx;
                let y = tile.y as i32 + dy;
                if x < 0 || y < 0 || dx * dx + dy * dy > ROUTE_PENALTY_RADIUS.pow(2) {
                    continue;
                }
                let Some(index) = tile_index(analysis.width, analysis.height, x as u32, y as u32)
                else {
                    continue;
                };
                let penalty = if dx == 0 && dy == 0 {
                    PREVIOUS_ROUTE_PENALTY
                } else {
                    NEAR_ROUTE_PENALTY
                };
                penalties[index] = penalties[index].saturating_add(penalty);
            }
        }
    }
}

fn route_similarity(route: &[AiTile], other: &[[u32; 2]]) -> f32 {
    if route.is_empty() || other.is_empty() {
        return 0.0;
    }
    let shared = route
        .iter()
        .filter(|tile| other.contains(&[tile.x, tile.y]))
        .count();
    shared as f32 / route.len().min(other.len()) as f32
}

fn route_length(route: &[AiTile]) -> f32 {
    route
        .windows(2)
        .map(|pair| {
            if pair[0].x != pair[1].x && pair[0].y != pair[1].y {
                std::f32::consts::SQRT_2
            } else {
                1.0
            }
        })
        .sum()
}

fn route_bottleneck(analysis: &AiMapAnalysis, route: &[AiTile]) -> (AiTile, u16) {
    let margin = route.len().min(8);
    route
        .iter()
        .enumerate()
        .filter(|(index, _)| *index >= margin && *index + margin < route.len())
        .filter_map(|(_, tile)| {
            let index = tile_index(analysis.width, analysis.height, tile.x, tile.y)?;
            Some((*tile, analysis.clearance.get(index).copied().unwrap_or(0)))
        })
        .min_by_key(|(tile, clearance)| (*clearance, tile.x, tile.y))
        .or_else(|| route.first().copied().map(|tile| (tile, 0)))
        .unwrap_or((AiTile::new(0, 0), 0))
}

fn crossed_chokes(analysis: &AiMapAnalysis, route: &[AiTile]) -> Vec<u32> {
    let mut ids = analysis
        .chokes
        .iter()
        .filter(|choke| {
            route.iter().any(|tile| {
                tile_distance2(*tile, choke.center_tile)
                    <= u32::from(choke.width_tiles.max(2)).saturating_pow(2)
            })
        })
        .map(|choke| choke.id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn intercept_tile(analysis: &AiMapAnalysis, route: &[AiTile], crossed_choke_ids: &[u32]) -> AiTile {
    if let Some(choke) = crossed_choke_ids
        .iter()
        .filter_map(|id| analysis.chokes.iter().find(|choke| choke.id == *id))
        .max_by_key(|choke| {
            route
                .iter()
                .rposition(|tile| tile_distance2(*tile, choke.center_tile) <= 9)
                .unwrap_or(0)
        })
    {
        return choke.center_tile;
    }
    let index = route.len().saturating_mul(3) / 4;
    route
        .get(index.min(route.len().saturating_sub(1)))
        .copied()
        .unwrap_or(AiTile::new(0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, MapMetadata, PlayerInit};

    fn analysis(map_name: &str) -> AiMapAnalysis {
        let players = vec![
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "P1".to_string(),
                color: "#00f".to_string(),
                is_ai: true,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "P2".to_string(),
                color: "#f00".to_string(),
                is_ai: true,
            },
        ];
        let slots = vec![(1, 1), (2, 2)];
        let map = Map::load_for_players(map_name, &slots, 0x1234_5678).unwrap();
        let game = Game::new_with_random_ai_profiles_and_map_metadata(
            &players,
            0x1234_5678,
            map,
            MapMetadata {
                name: map_name.to_string(),
                schema_version: 0,
                content_hash: "test".to_string(),
            },
        );
        AiMapAnalysis::analyze(&game.start_payload())
    }

    #[test]
    fn bundled_maps_produce_home_and_expansion_attack_paths() {
        for map_name in ["Schone Tage", "1v1", "Chokes", "Crossroads"] {
            let report = analysis(map_name).likely_attack_paths(1, 2, 2);
            assert_eq!(report.bases.len(), 2, "{map_name}");
            assert!(report.paths.len() >= 2, "{map_name}: {:?}", report.warnings);
            assert!(report.paths.iter().all(|path| path.tiles.len() > 2));
        }
    }

    #[test]
    fn safer_expansion_outranks_closer_site_inside_distance_envelope() {
        let close_exposed = ExpansionDefenseMetrics {
            added_attack_vectors: 2,
            open_approaches: 2,
            shared_route_percent: 20,
            widest_bottleneck: 8,
            distance_milli_tiles: 20_000,
        };
        let farther_safe = ExpansionDefenseMetrics {
            added_attack_vectors: 0,
            open_approaches: 0,
            shared_route_percent: 80,
            widest_bottleneck: 3,
            distance_milli_tiles: 32_000,
        };
        assert!(expansion_rank_key(farther_safe, 2) < expansion_rank_key(close_exposed, 1));
    }
}
