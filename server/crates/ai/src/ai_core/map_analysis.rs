//! AI-owned static map analysis built only from public start-payload data.
//!
//! This is intentionally a scaffold: it records stable terrain, component, start, and resource
//! facts for later route analysis without feeding them into command decisions yet.

use std::collections::VecDeque;

use crate::config;
use rts_protocol::{
    ObserverMapAnalysisDiagnostics, ObserverMapAnalysisLayer, ObserverMapAnalysisPrimitive,
};
use rts_sim::protocol::{kinds, terrain, MapInfo, PlayerStart, ResourceNode, StartPayload};

const MAX_CLEARANCE_TILES: u16 = 16;
const RESOURCE_CLUSTER_RADIUS_MARGIN_TILES: f32 = 0.75;
const MAP_ANALYSIS_COMPONENT_COLORS: [&str; 8] = [
    "#3da5d9", "#f2a541", "#7ac74f", "#c77dff", "#ef476f", "#ffd166", "#06d6a0", "#8fb8d0",
];
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const NEIGHBORS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AiMapAnalysisKey {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) tile_size: u32,
    pub(crate) terrain_hash: u64,
    pub(crate) starts_hash: u64,
    pub(crate) resources_hash: u64,
}

impl AiMapAnalysisKey {
    pub(crate) fn from_start(start: &StartPayload) -> Self {
        Self {
            width: start.map.width,
            height: start.map.height,
            tile_size: start.map.tile_size,
            terrain_hash: fnv_bytes(FNV_OFFSET_BASIS, &start.map.terrain),
            starts_hash: hash_player_starts(&start.players),
            resources_hash: hash_resources(&start.map.resources),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AiTile {
    pub(crate) x: u32,
    pub(crate) y: u32,
}

impl AiTile {
    fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AiTileBounds {
    pub(crate) min: AiTile,
    pub(crate) max: AiTile,
}

impl AiTileBounds {
    fn new(tile: AiTile) -> Self {
        Self {
            min: tile,
            max: tile,
        }
    }

    fn include(&mut self, tile: AiTile) {
        self.min.x = self.min.x.min(tile.x);
        self.min.y = self.min.y.min(tile.y);
        self.max.x = self.max.x.max(tile.x);
        self.max.y = self.max.y.max(tile.y);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiMapComponent {
    pub(crate) id: u32,
    pub(crate) tile_count: u32,
    pub(crate) bounds: AiTileBounds,
    pub(crate) representative: AiTile,
    pub(crate) max_clearance_tiles: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiStartMapping {
    pub(crate) player_id: u32,
    pub(crate) team_id: u32,
    pub(crate) start_tile: AiTile,
    pub(crate) component_id: Option<u32>,
    pub(crate) clearance_tiles: u16,
    pub(crate) nearest_resource_cluster_id: Option<u32>,
    pub(crate) nearest_resource_cluster_distance2: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiResourceCluster {
    pub(crate) id: u32,
    pub(crate) center_tile: AiTile,
    pub(crate) component_id: Option<u32>,
    pub(crate) resource_ids: Vec<u32>,
    pub(crate) steel_nodes: u16,
    pub(crate) oil_nodes: u16,
    pub(crate) nearest_start_player_id: Option<u32>,
    pub(crate) nearest_start_distance2: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiMapAnalysis {
    key: AiMapAnalysisKey,
    width: u32,
    height: u32,
    tile_size: u32,
    passable: Vec<bool>,
    clearance: Vec<u16>,
    component_by_tile: Vec<Option<u32>>,
    components: Vec<AiMapComponent>,
    starts: Vec<AiStartMapping>,
    resource_clusters: Vec<AiResourceCluster>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct AiMapAnalysisDebugSnapshot {
    pub(crate) key: AiMapAnalysisKey,
    pub(crate) map_width: u32,
    pub(crate) map_height: u32,
    pub(crate) tile_size: u32,
    pub(crate) passable_tiles: u32,
    pub(crate) blocked_tiles: u32,
    pub(crate) max_clearance_tiles: u16,
    pub(crate) component_count: usize,
    pub(crate) largest_component_tiles: u32,
    pub(crate) components: Vec<AiMapComponent>,
    pub(crate) starts: Vec<AiStartMapping>,
    pub(crate) resource_clusters: Vec<AiResourceCluster>,
}

impl AiMapAnalysis {
    #[allow(dead_code)]
    pub(crate) fn analyze(start: &StartPayload) -> Self {
        Self::analyze_with_key(start, AiMapAnalysisKey::from_start(start))
    }

    pub(crate) fn analyze_with_key(start: &StartPayload, key: AiMapAnalysisKey) -> Self {
        let width = start.map.width;
        let height = start.map.height;
        let tile_size = start.map.tile_size;
        let passable = build_passability(&start.map);
        let clearance = build_clearance(width, height, &passable);
        let (component_by_tile, components) =
            build_components(width, height, &passable, &clearance);
        let resource_clusters = build_resource_clusters(
            &start.map,
            &start.players,
            &clearance,
            &component_by_tile,
        );
        let starts = build_start_mappings(
            &start.players,
            width,
            height,
            &clearance,
            &component_by_tile,
            &resource_clusters,
        );

        Self {
            key,
            width,
            height,
            tile_size,
            passable,
            clearance,
            component_by_tile,
            components,
            starts,
            resource_clusters,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn key(&self) -> AiMapAnalysisKey {
        self.key
    }

    #[allow(dead_code)]
    pub(crate) fn component_id_at(&self, tile: AiTile) -> Option<u32> {
        tile_index(self.width, self.height, tile.x, tile.y)
            .and_then(|idx| self.component_by_tile.get(idx).copied().flatten())
    }

    #[allow(dead_code)]
    pub(crate) fn debug_snapshot(&self) -> AiMapAnalysisDebugSnapshot {
        let passable_tiles = self.passable.iter().filter(|passable| **passable).count() as u32;
        let total_tiles = self.passable.len() as u32;
        AiMapAnalysisDebugSnapshot {
            key: self.key,
            map_width: self.width,
            map_height: self.height,
            tile_size: self.tile_size,
            passable_tiles,
            blocked_tiles: total_tiles.saturating_sub(passable_tiles),
            max_clearance_tiles: self.clearance.iter().copied().max().unwrap_or(0),
            component_count: self.components.len(),
            largest_component_tiles: self
                .components
                .iter()
                .map(|component| component.tile_count)
                .max()
                .unwrap_or(0),
            components: self.components.clone(),
            starts: self.starts.clone(),
            resource_clusters: self.resource_clusters.clone(),
        }
    }

    pub(crate) fn debug_overlay(&self) -> ObserverMapAnalysisDiagnostics {
        ObserverMapAnalysisDiagnostics {
            map_width: self.width,
            map_height: self.height,
            tile_size: self.tile_size,
            layers: vec![
                ObserverMapAnalysisLayer {
                    id: "components".to_string(),
                    label: "Components".to_string(),
                    default_visible: true,
                    primitives: self.component_overlay_primitives(),
                },
                ObserverMapAnalysisLayer {
                    id: "bases".to_string(),
                    label: "Bases".to_string(),
                    default_visible: true,
                    primitives: self.base_overlay_primitives(),
                },
                ObserverMapAnalysisLayer {
                    id: "resources".to_string(),
                    label: "Resources".to_string(),
                    default_visible: true,
                    primitives: self.resource_overlay_primitives(),
                },
            ],
        }
    }

    fn component_overlay_primitives(&self) -> Vec<ObserverMapAnalysisPrimitive> {
        self.components
            .iter()
            .map(|component| {
                let fill = component_color(component.id).to_string();
                ObserverMapAnalysisPrimitive::TileRect {
                    id: format!("component:{}", component.id),
                    tile_x: component.bounds.min.x,
                    tile_y: component.bounds.min.y,
                    tile_w: component
                        .bounds
                        .max
                        .x
                        .saturating_sub(component.bounds.min.x)
                        .saturating_add(1),
                    tile_h: component
                        .bounds
                        .max
                        .y
                        .saturating_sub(component.bounds.min.y)
                        .saturating_add(1),
                    stroke: fill.clone(),
                    fill,
                    alpha: component_fill_alpha(component.tile_count),
                    label: Some(format!(
                        "C{} {}t clr{}",
                        component.id, component.tile_count, component.max_clearance_tiles
                    )),
                }
            })
            .collect()
    }

    fn base_overlay_primitives(&self) -> Vec<ObserverMapAnalysisPrimitive> {
        self.starts
            .iter()
            .map(|start| {
                let (x, y) = tile_center_world(start.start_tile, self.tile_size);
                let color = start
                    .component_id
                    .map(component_color)
                    .unwrap_or("#e7dfc5")
                    .to_string();
                ObserverMapAnalysisPrimitive::Marker {
                    id: format!("base:{}", start.player_id),
                    x,
                    y,
                    radius: (self.tile_size as f32 * 0.62).max(8.0),
                    shape: "diamond".to_string(),
                    color,
                    label: Some(format!(
                        "P{} T{} {}",
                        start.player_id,
                        start.team_id,
                        component_label(start.component_id)
                    )),
                }
            })
            .collect()
    }

    fn resource_overlay_primitives(&self) -> Vec<ObserverMapAnalysisPrimitive> {
        self.resource_clusters
            .iter()
            .map(|cluster| {
                let (x, y) = tile_center_world(cluster.center_tile, self.tile_size);
                let color = cluster
                    .component_id
                    .map(component_color)
                    .unwrap_or("#e7dfc5")
                    .to_string();
                ObserverMapAnalysisPrimitive::Marker {
                    id: format!("resourceCluster:{}", cluster.id),
                    x,
                    y,
                    radius: (self.tile_size as f32 * 0.5).max(7.0),
                    shape: "circle".to_string(),
                    color,
                    label: Some(format!(
                        "R{} {}S/{}O {}",
                        cluster.id,
                        cluster.steel_nodes,
                        cluster.oil_nodes,
                        component_label(cluster.component_id)
                    )),
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
struct ResourcePoint {
    id: u32,
    kind: ResourcePointKind,
    x: f32,
    y: f32,
    component_id: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResourcePointKind {
    Steel,
    Oil,
}

#[derive(Clone, Debug)]
struct ClusterCandidate {
    center: AiTile,
    clearance_tiles: u16,
    resource_indices: Vec<usize>,
}

fn build_passability(map: &MapInfo) -> Vec<bool> {
    let Some(tile_count) = tile_count(map.width, map.height) else {
        return Vec::new();
    };
    (0..tile_count)
        .map(|idx| map.terrain.get(idx).copied() == Some(terrain::GRASS))
        .collect()
}

fn build_clearance(width: u32, height: u32, passable: &[bool]) -> Vec<u16> {
    let mut clearance = vec![0; passable.len()];
    for y in 0..height {
        for x in 0..width {
            let Some(idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if passable.get(idx).copied() != Some(true) {
                continue;
            }

            let mut tile_clearance = 1;
            for radius in 1..MAX_CLEARANCE_TILES {
                if passable_ring(width, height, passable, x, y, i32::from(radius)) {
                    tile_clearance = radius + 1;
                } else {
                    break;
                }
            }
            clearance[idx] = tile_clearance;
        }
    }
    clearance
}

fn build_components(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
) -> (Vec<Option<u32>>, Vec<AiMapComponent>) {
    let mut component_by_tile = vec![None; passable.len()];
    let mut components = Vec::new();
    let mut queue = VecDeque::new();

    for y in 0..height {
        for x in 0..width {
            let Some(start_idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if passable.get(start_idx).copied() != Some(true)
                || component_by_tile
                    .get(start_idx)
                    .copied()
                    .flatten()
                    .is_some()
            {
                continue;
            }

            let id = components.len() as u32;
            let start_tile = AiTile::new(x, y);
            let mut component = AiMapComponent {
                id,
                tile_count: 0,
                bounds: AiTileBounds::new(start_tile),
                representative: start_tile,
                max_clearance_tiles: 0,
            };
            component_by_tile[start_idx] = Some(id);
            queue.push_back(start_tile);

            while let Some(tile) = queue.pop_front() {
                let Some(idx) = tile_index(width, height, tile.x, tile.y) else {
                    continue;
                };
                component.tile_count = component.tile_count.saturating_add(1);
                component.bounds.include(tile);
                component.max_clearance_tiles = component
                    .max_clearance_tiles
                    .max(clearance.get(idx).copied().unwrap_or(0));

                for neighbor in passable_neighbors(width, height, passable, tile) {
                    let Some(neighbor_idx) = tile_index(width, height, neighbor.x, neighbor.y)
                    else {
                        continue;
                    };
                    if component_by_tile
                        .get(neighbor_idx)
                        .copied()
                        .flatten()
                        .is_some()
                    {
                        continue;
                    }
                    component_by_tile[neighbor_idx] = Some(id);
                    queue.push_back(neighbor);
                }
            }

            components.push(component);
        }
    }

    (component_by_tile, components)
}

fn build_start_mappings(
    players: &[PlayerStart],
    width: u32,
    height: u32,
    clearance: &[u16],
    component_by_tile: &[Option<u32>],
    resource_clusters: &[AiResourceCluster],
) -> Vec<AiStartMapping> {
    let mut starts: Vec<_> = players.iter().collect();
    starts.sort_by_key(|player| player.id);
    starts
        .into_iter()
        .map(|player| {
            let start_tile = AiTile::new(player.start_tile_x, player.start_tile_y);
            let component_id = component_id_for_tile(width, height, component_by_tile, start_tile);
            let idx = tile_index(width, height, start_tile.x, start_tile.y);
            let (nearest_resource_cluster_id, nearest_resource_cluster_distance2) =
                nearest_cluster(start_tile, component_id, resource_clusters)
                    .map(|(id, distance2)| (Some(id), Some(distance2)))
                    .unwrap_or((None, None));
            AiStartMapping {
                player_id: player.id,
                team_id: player.team_id,
                start_tile,
                component_id,
                clearance_tiles: idx.and_then(|idx| clearance.get(idx).copied()).unwrap_or(0),
                nearest_resource_cluster_id,
                nearest_resource_cluster_distance2,
            }
        })
        .collect()
}

fn build_resource_clusters(
    map: &MapInfo,
    players: &[PlayerStart],
    clearance: &[u16],
    component_by_tile: &[Option<u32>],
) -> Vec<AiResourceCluster> {
    let mut resources: Vec<_> = map
        .resources
        .iter()
        .filter_map(|resource| resource_point(map, component_by_tile, resource))
        .collect();
    resources.sort_by_key(|resource| resource.id);
    if resources.is_empty() {
        return Vec::new();
    }

    let expected_cluster_size =
        (config::STEEL_PATCHES_PER_BASE + config::OIL_PATCHES_PER_BASE) as usize;
    let radius_px = (config::CC_RESOURCE_MAX_DIST_TILES + RESOURCE_CLUSTER_RADIUS_MARGIN_TILES)
        * map.tile_size as f32;
    let radius2 = radius_px * radius_px;
    let mut unassigned = vec![true; resources.len()];
    let mut remaining = resources.len();
    let mut clusters = Vec::new();

    while remaining > 0 {
        let Some(candidate) = best_resource_cluster_candidate(
            map,
            clearance,
            &resources,
            &unassigned,
            radius2,
            expected_cluster_size,
            component_by_tile,
        ) else {
            break;
        };
        let mut resource_indices = candidate.resource_indices;
        if resource_indices.len() > expected_cluster_size {
            resource_indices.sort_by(|a, b| {
                let da = distance2_to_tile_center(map, candidate.center, &resources[*a]);
                let db = distance2_to_tile_center(map, candidate.center, &resources[*b]);
                da.total_cmp(&db)
                    .then_with(|| resources[*a].id.cmp(&resources[*b].id))
            });
            resource_indices.truncate(expected_cluster_size);
            resource_indices.sort_by_key(|idx| resources[*idx].id);
        }

        let id = clusters.len() as u32;
        let resource_ids = resource_indices
            .iter()
            .map(|idx| resources[*idx].id)
            .collect();
        let steel_nodes = resource_indices
            .iter()
            .filter(|idx| resources[**idx].kind == ResourcePointKind::Steel)
            .count() as u16;
        let oil_nodes = resource_indices
            .iter()
            .filter(|idx| resources[**idx].kind == ResourcePointKind::Oil)
            .count() as u16;
        let component_id =
            component_id_for_tile(map.width, map.height, component_by_tile, candidate.center);
        let (nearest_start_player_id, nearest_start_distance2) =
            nearest_start(
                candidate.center,
                component_id,
                players,
                map.width,
                map.height,
                component_by_tile,
            )
                .map(|(id, distance2)| (Some(id), Some(distance2)))
                .unwrap_or((None, None));

        for idx in resource_indices {
            if unassigned.get(idx).copied() == Some(true) {
                unassigned[idx] = false;
                remaining = remaining.saturating_sub(1);
            }
        }

        clusters.push(AiResourceCluster {
            id,
            center_tile: candidate.center,
            component_id,
            resource_ids,
            steel_nodes,
            oil_nodes,
            nearest_start_player_id,
            nearest_start_distance2,
        });
    }

    clusters
}

fn best_resource_cluster_candidate(
    map: &MapInfo,
    clearance: &[u16],
    resources: &[ResourcePoint],
    unassigned: &[bool],
    radius2: f32,
    expected_cluster_size: usize,
    component_by_tile: &[Option<u32>],
) -> Option<ClusterCandidate> {
    let mut best = None;
    for y in 0..map.height {
        for x in 0..map.width {
            let Some(idx) = tile_index(map.width, map.height, x, y) else {
                continue;
            };
            let center = AiTile::new(x, y);
            let center_component_id = component_id_for_tile(
                map.width,
                map.height,
                component_by_tile,
                center,
            );
            if center_component_id.is_none() {
                continue;
            }
            let mut resource_indices = Vec::new();
            for (resource_idx, resource) in resources.iter().enumerate() {
                if unassigned.get(resource_idx).copied() != Some(true) {
                    continue;
                }
                if !same_component_or_unknown(center_component_id, resource.component_id) {
                    continue;
                }
                if distance2_to_tile_center(map, center, resource) <= radius2 {
                    resource_indices.push(resource_idx);
                }
            }
            if resource_indices.is_empty() {
                continue;
            }

            let candidate = ClusterCandidate {
                center,
                clearance_tiles: clearance.get(idx).copied().unwrap_or(0),
                resource_indices,
            };
            if cluster_candidate_better(&candidate, best.as_ref(), expected_cluster_size) {
                best = Some(candidate);
            }
        }
    }
    best
}

fn cluster_candidate_better(
    candidate: &ClusterCandidate,
    incumbent: Option<&ClusterCandidate>,
    expected_cluster_size: usize,
) -> bool {
    let Some(incumbent) = incumbent else {
        return true;
    };
    let candidate_count = candidate.resource_indices.len();
    let incumbent_count = incumbent.resource_indices.len();
    let candidate_useful = candidate_count.min(expected_cluster_size);
    let incumbent_useful = incumbent_count.min(expected_cluster_size);
    let candidate_overage = candidate_count.saturating_sub(expected_cluster_size);
    let incumbent_overage = incumbent_count.saturating_sub(expected_cluster_size);

    candidate_useful > incumbent_useful
        || (candidate_useful == incumbent_useful && candidate_overage < incumbent_overage)
        || (candidate_useful == incumbent_useful
            && candidate_overage == incumbent_overage
            && candidate.clearance_tiles > incumbent.clearance_tiles)
        || (candidate_useful == incumbent_useful
            && candidate_overage == incumbent_overage
            && candidate.clearance_tiles == incumbent.clearance_tiles
            && (candidate.center.y, candidate.center.x) < (incumbent.center.y, incumbent.center.x))
}

fn resource_point(
    map: &MapInfo,
    component_by_tile: &[Option<u32>],
    resource: &ResourceNode,
) -> Option<ResourcePoint> {
    let kind = match resource.kind.as_str() {
        kinds::STEEL => ResourcePointKind::Steel,
        kinds::OIL => ResourcePointKind::Oil,
        _ => return None,
    };
    let component_id = resource_tile(map, resource)
        .and_then(|tile| component_id_for_tile(map.width, map.height, component_by_tile, tile));
    Some(ResourcePoint {
        id: resource.id,
        kind,
        x: resource.x,
        y: resource.y,
        component_id,
    })
}

fn nearest_cluster(
    tile: AiTile,
    component_id: Option<u32>,
    clusters: &[AiResourceCluster],
) -> Option<(u32, u32)> {
    if let Some(component_id) = component_id {
        if let Some(nearest) = nearest_cluster_matching(tile, clusters, |cluster| {
            cluster.component_id == Some(component_id)
        }) {
            return Some(nearest);
        }
    }
    nearest_cluster_matching(tile, clusters, |_| true)
}

fn nearest_cluster_matching<F>(
    tile: AiTile,
    clusters: &[AiResourceCluster],
    mut accepts: F,
) -> Option<(u32, u32)>
where
    F: FnMut(&AiResourceCluster) -> bool,
{
    clusters
        .iter()
        .filter(|cluster| accepts(cluster))
        .map(|cluster| {
            (
                cluster.id,
                tile_distance2(tile, cluster.center_tile),
                cluster.center_tile,
            )
        })
        .min_by_key(|(id, distance2, center)| (*distance2, center.y, center.x, *id))
        .map(|(id, distance2, _)| (id, distance2))
}

fn nearest_start(
    tile: AiTile,
    component_id: Option<u32>,
    players: &[PlayerStart],
    width: u32,
    height: u32,
    component_by_tile: &[Option<u32>],
) -> Option<(u32, u32)> {
    if let Some(component_id) = component_id {
        if let Some(nearest) = nearest_start_matching(tile, players, |player| {
            let start_tile = AiTile::new(player.start_tile_x, player.start_tile_y);
            component_id_for_tile(width, height, component_by_tile, start_tile)
                == Some(component_id)
        }) {
            return Some(nearest);
        }
    }
    nearest_start_matching(tile, players, |_| true)
}

fn nearest_start_matching<F>(
    tile: AiTile,
    players: &[PlayerStart],
    mut accepts: F,
) -> Option<(u32, u32)>
where
    F: FnMut(&PlayerStart) -> bool,
{
    players
        .iter()
        .filter(|player| accepts(player))
        .map(|player| {
            let start_tile = AiTile::new(player.start_tile_x, player.start_tile_y);
            (player.id, tile_distance2(tile, start_tile), start_tile)
        })
        .min_by_key(|(id, distance2, start_tile)| (*distance2, start_tile.y, start_tile.x, *id))
        .map(|(id, distance2, _)| (id, distance2))
}

fn same_component_or_unknown(a: Option<u32>, b: Option<u32>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

fn component_id_for_tile(
    width: u32,
    height: u32,
    component_by_tile: &[Option<u32>],
    tile: AiTile,
) -> Option<u32> {
    tile_index(width, height, tile.x, tile.y)
        .and_then(|idx| component_by_tile.get(idx).copied().flatten())
}

fn resource_tile(map: &MapInfo, resource: &ResourceNode) -> Option<AiTile> {
    if map.tile_size == 0 || !resource.x.is_finite() || !resource.y.is_finite() {
        return None;
    }
    let tile_size = map.tile_size as f32;
    let x = (resource.x / tile_size).floor();
    let y = (resource.y / tile_size).floor();
    if x < 0.0 || y < 0.0 || x >= map.width as f32 || y >= map.height as f32 {
        return None;
    }
    Some(AiTile::new(x as u32, y as u32))
}

fn passable_neighbors(width: u32, height: u32, passable: &[bool], tile: AiTile) -> Vec<AiTile> {
    let mut out = Vec::with_capacity(8);
    let x = tile.x as i32;
    let y = tile.y as i32;
    for (dx, dy) in NEIGHBORS {
        let nx = x + dx;
        let ny = y + dy;
        if !passable_at_i32(width, height, passable, nx, ny) {
            continue;
        }
        if dx != 0
            && dy != 0
            && (!passable_at_i32(width, height, passable, x + dx, y)
                || !passable_at_i32(width, height, passable, x, y + dy))
        {
            continue;
        }
        out.push(AiTile::new(nx as u32, ny as u32));
    }
    out
}

fn passable_ring(
    width: u32,
    height: u32,
    passable: &[bool],
    center_x: u32,
    center_y: u32,
    radius: i32,
) -> bool {
    let cx = center_x as i32;
    let cy = center_y as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx.abs() != radius && dy.abs() != radius {
                continue;
            }
            if !passable_at_i32(width, height, passable, cx + dx, cy + dy) {
                return false;
            }
        }
    }
    true
}

fn passable_at_i32(width: u32, height: u32, passable: &[bool], x: i32, y: i32) -> bool {
    if x < 0 || y < 0 {
        return false;
    }
    tile_index(width, height, x as u32, y as u32)
        .and_then(|idx| passable.get(idx).copied())
        .unwrap_or(false)
}

fn distance2_to_tile_center(map: &MapInfo, tile: AiTile, resource: &ResourcePoint) -> f32 {
    let tile_size = map.tile_size as f32;
    let x = tile.x as f32 * tile_size + tile_size * 0.5;
    let y = tile.y as f32 * tile_size + tile_size * 0.5;
    (x - resource.x).powi(2) + (y - resource.y).powi(2)
}

fn tile_distance2(a: AiTile, b: AiTile) -> u32 {
    let dx = i64::from(a.x) - i64::from(b.x);
    let dy = i64::from(a.y) - i64::from(b.y);
    let distance2 = dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy));
    u32::try_from(distance2).unwrap_or(u32::MAX)
}

fn tile_index(width: u32, height: u32, x: u32, y: u32) -> Option<usize> {
    if x >= width || y >= height {
        return None;
    }
    y.checked_mul(width)
        .and_then(|row| row.checked_add(x))
        .and_then(|idx| usize::try_from(idx).ok())
}

fn tile_count(width: u32, height: u32) -> Option<usize> {
    width
        .checked_mul(height)
        .and_then(|count| usize::try_from(count).ok())
}

fn component_color(component_id: u32) -> &'static str {
    MAP_ANALYSIS_COMPONENT_COLORS
        .get(component_id as usize % MAP_ANALYSIS_COMPONENT_COLORS.len())
        .copied()
        .unwrap_or("#8fb8d0")
}

fn component_fill_alpha(tile_count: u32) -> f32 {
    if tile_count >= 1_000 {
        0.12
    } else if tile_count >= 100 {
        0.16
    } else {
        0.22
    }
}

fn component_label(component_id: Option<u32>) -> String {
    component_id
        .map(|id| format!("C{id}"))
        .unwrap_or_else(|| "C?".to_string())
}

fn tile_center_world(tile: AiTile, tile_size: u32) -> (f32, f32) {
    let tile_size = tile_size as f32;
    (
        tile.x as f32 * tile_size + tile_size * 0.5,
        tile.y as f32 * tile_size + tile_size * 0.5,
    )
}

fn hash_player_starts(players: &[PlayerStart]) -> u64 {
    let mut sorted: Vec<_> = players.iter().collect();
    sorted.sort_by_key(|player| player.id);
    let mut hash = FNV_OFFSET_BASIS;
    for player in sorted {
        hash = fnv_u32(hash, player.id);
        hash = fnv_u32(hash, player.team_id);
        hash = fnv_u32(hash, player.start_tile_x);
        hash = fnv_u32(hash, player.start_tile_y);
    }
    hash
}

fn hash_resources(resources: &[ResourceNode]) -> u64 {
    let mut sorted: Vec<_> = resources.iter().collect();
    sorted.sort_by_key(|resource| resource.id);
    let mut hash = FNV_OFFSET_BASIS;
    for resource in sorted {
        hash = fnv_u32(hash, resource.id);
        hash = fnv_bytes(hash, resource.kind.as_bytes());
        hash = fnv_u32(hash, resource.x.to_bits());
        hash = fnv_u32(hash, resource.y.to_bits());
    }
    hash
}

fn fnv_u32(hash: u64, value: u32) -> u64 {
    fnv_bytes(hash, &value.to_le_bytes())
}

fn fnv_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, MapMetadata, PlayerInit};

    const FIXTURE_SEED: u32 = 0x1234_5678;

    #[derive(Clone, Copy)]
    struct ExpectedFixture {
        name: &'static str,
        component_count: usize,
        passable_tiles: u32,
        blocked_tiles: u32,
        largest_component_tiles: u32,
        resource_clusters: usize,
    }

    fn player_inits(count: u32) -> Vec<PlayerInit> {
        (1..=count)
            .map(|id| PlayerInit {
                id,
                team_id: id,
                faction_id: "kriegsia".to_string(),
                name: format!("P{id}"),
                color: format!("#{id}{id}{id}"),
                is_ai: true,
            })
            .collect()
    }

    fn fixture_analysis(map_name: &str) -> AiMapAnalysisDebugSnapshot {
        let players = player_inits(2);
        let player_slots: Vec<_> = players
            .iter()
            .map(|player| (player.id, player.team_id))
            .collect();
        let map = Map::load_for_players(map_name, &player_slots, FIXTURE_SEED)
            .expect("fixture map should load");
        let metadata = Map::metadata_for_name(map_name).unwrap_or_else(|_| MapMetadata {
            name: map_name.to_string(),
            schema_version: rts_sim::game::map::CURRENT_MAP_VERSION,
            content_hash: "test".to_string(),
        });
        let game = Game::new_with_random_ai_profiles_and_map_metadata(
            &players,
            FIXTURE_SEED,
            map,
            metadata,
        );
        AiMapAnalysis::analyze(&game.start_payload()).debug_snapshot()
    }

    fn resource_at(id: u32, kind: &str, tile_x: u32, tile_y: u32) -> ResourceNode {
        let tile_size = config::TILE_SIZE as f32;
        ResourceNode {
            id,
            kind: kind.to_string(),
            x: (tile_x as f32 + 0.5) * tile_size,
            y: (tile_y as f32 + 0.5) * tile_size,
        }
    }

    #[test]
    fn no_terrain_fixture_is_one_clear_component() {
        let debug = fixture_analysis("No Terrain");

        assert_eq!(debug.map_width, 126);
        assert_eq!(debug.map_height, 126);
        assert_eq!(debug.passable_tiles, 126 * 126);
        assert_eq!(debug.blocked_tiles, 0);
        assert_eq!(debug.component_count, 1);
        assert_eq!(debug.largest_component_tiles, 126 * 126);
        assert_eq!(debug.max_clearance_tiles, MAX_CLEARANCE_TILES);
        assert!(debug.starts.iter().all(|start| {
            start.component_id == Some(0) && start.clearance_tiles == MAX_CLEARANCE_TILES
        }));
    }

    #[test]
    fn bundled_fixture_counts_are_deterministic() {
        let expected = [
            ExpectedFixture {
                name: "Default",
                component_count: 43,
                passable_tiles: 14_634,
                blocked_tiles: 1_242,
                largest_component_tiles: 14_476,
                resource_clusters: 6,
            },
            ExpectedFixture {
                name: "Low Econ",
                component_count: 45,
                passable_tiles: 14_615,
                blocked_tiles: 1_261,
                largest_component_tiles: 14_451,
                resource_clusters: 4,
            },
            ExpectedFixture {
                name: "No Terrain",
                component_count: 1,
                passable_tiles: 126 * 126,
                blocked_tiles: 0,
                largest_component_tiles: 126 * 126,
                resource_clusters: 4,
            },
        ];

        for fixture in expected {
            let debug = fixture_analysis(fixture.name);

            assert_eq!(
                debug.component_count, fixture.component_count,
                "{} component count changed",
                fixture.name
            );
            assert_eq!(
                debug.passable_tiles, fixture.passable_tiles,
                "{} passable tile count changed",
                fixture.name
            );
            assert_eq!(
                debug.blocked_tiles, fixture.blocked_tiles,
                "{} blocked tile count changed",
                fixture.name
            );
            assert_eq!(
                debug.largest_component_tiles, fixture.largest_component_tiles,
                "{} largest component size changed",
                fixture.name
            );
            assert_eq!(
                debug.resource_clusters.len(),
                fixture.resource_clusters,
                "{} resource cluster count changed",
                fixture.name
            );
            assert_eq!(debug.passable_tiles + debug.blocked_tiles, 126 * 126);
        }
    }

    #[test]
    fn resource_clusters_cover_all_static_nodes_with_expected_base_shape() {
        let expected_nodes_per_cluster =
            (config::STEEL_PATCHES_PER_BASE + config::OIL_PATCHES_PER_BASE) as usize;

        for map_name in ["Default", "Low Econ", "No Terrain"] {
            let debug = fixture_analysis(map_name);
            let total_clustered_nodes: usize = debug
                .resource_clusters
                .iter()
                .map(|cluster| cluster.resource_ids.len())
                .sum();

            assert_eq!(
                total_clustered_nodes,
                debug.resource_clusters.len() * expected_nodes_per_cluster,
                "{map_name} should assign every static resource to full base clusters"
            );
            for cluster in &debug.resource_clusters {
                assert_eq!(
                    cluster.resource_ids.len(),
                    expected_nodes_per_cluster,
                    "{map_name} cluster {:?} should keep one base resource group",
                    cluster.id
                );
                assert_eq!(cluster.steel_nodes, config::STEEL_PATCHES_PER_BASE as u16);
                assert_eq!(cluster.oil_nodes, config::OIL_PATCHES_PER_BASE as u16);
                assert!(
                    cluster.component_id.is_some(),
                    "{map_name} cluster {:?} should map to passable terrain",
                    cluster.id
                );
            }
        }
    }

    #[test]
    fn player_starts_map_to_components_and_nearby_resource_clusters() {
        for map_name in ["Default", "Low Econ", "No Terrain"] {
            let debug = fixture_analysis(map_name);

            assert_eq!(debug.starts.len(), 2);
            for start in &debug.starts {
                assert!(
                    start.component_id.is_some(),
                    "{map_name} player {} start should map to a passable component",
                    start.player_id
                );
                assert!(
                    start.clearance_tiles >= 8,
                    "{map_name} player {} start clearance was {}",
                    start.player_id,
                    start.clearance_tiles
                );
                assert!(
                    start.nearest_resource_cluster_id.is_some(),
                    "{map_name} player {} should have a nearest resource cluster",
                    start.player_id
                );
            }
        }
    }

    #[test]
    fn resource_mappings_prefer_reachable_components_over_cross_wall_distance() {
        let width = 40;
        let height = 10;
        let mut terrain = vec![terrain::GRASS; (width * height) as usize];
        for y in 0..height {
            terrain[(y * width + 20) as usize] = terrain::ROCK;
        }
        let start = StartPayload {
            player_id: 1,
            spectator: false,
            prediction_build_id: None,
            prediction_version: 0,
            match_run_id: None,
            capabilities: Default::default(),
            diagnostics: Default::default(),
            replay: None,
            lab: None,
            tick: 0,
            map: MapInfo {
                width,
                height,
                tile_size: config::TILE_SIZE,
                terrain,
                resources: vec![
                    resource_at(1, kinds::STEEL, 2, 5),
                    resource_at(2, kinds::STEEL, 21, 5),
                ],
            },
            players: vec![
                PlayerStart {
                    id: 1,
                    team_id: 1,
                    faction_id: "kriegsia".to_string(),
                    name: "P1".to_string(),
                    color: "#111".to_string(),
                    start_tile_x: 19,
                    start_tile_y: 5,
                },
                PlayerStart {
                    id: 2,
                    team_id: 2,
                    faction_id: "kriegsia".to_string(),
                    name: "P2".to_string(),
                    color: "#222".to_string(),
                    start_tile_x: 39,
                    start_tile_y: 5,
                },
            ],
        };

        let debug = AiMapAnalysis::analyze(&start).debug_snapshot();
        let p1 = debug
            .starts
            .iter()
            .find(|start| start.player_id == 1)
            .expect("player 1 start should be present");
        let p2 = debug
            .starts
            .iter()
            .find(|start| start.player_id == 2)
            .expect("player 2 start should be present");
        assert_ne!(p1.component_id, p2.component_id);

        let p1_cluster = debug
            .resource_clusters
            .iter()
            .find(|cluster| Some(cluster.id) == p1.nearest_resource_cluster_id)
            .expect("player 1 should have a nearest cluster");
        assert_eq!(p1_cluster.component_id, p1.component_id);
        assert!(
            p1_cluster.resource_ids.contains(&1),
            "player 1 should map to the same-component resource, not the closer cross-wall one"
        );

        let right_cluster = debug
            .resource_clusters
            .iter()
            .find(|cluster| cluster.resource_ids.contains(&2))
            .expect("right-side resource should be clustered");
        assert_eq!(right_cluster.component_id, p2.component_id);
        assert_eq!(right_cluster.nearest_start_player_id, Some(2));
    }

    #[test]
    fn analysis_key_tracks_static_map_start_and_resource_identity() {
        let mut start = StartPayload {
            player_id: 1,
            spectator: false,
            prediction_build_id: None,
            prediction_version: 0,
            match_run_id: None,
            capabilities: Default::default(),
            diagnostics: Default::default(),
            replay: None,
            lab: None,
            tick: 0,
            map: MapInfo {
                width: 4,
                height: 4,
                tile_size: config::TILE_SIZE,
                terrain: vec![terrain::GRASS; 16],
                resources: Vec::new(),
            },
            players: vec![PlayerStart {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "P1".to_string(),
                color: "#111".to_string(),
                start_tile_x: 1,
                start_tile_y: 1,
            }],
        };

        let original = AiMapAnalysisKey::from_start(&start);
        start.players[0].start_tile_x = 2;
        let moved_start = AiMapAnalysisKey::from_start(&start);
        start.map.terrain[0] = terrain::ROCK;
        let changed_terrain = AiMapAnalysisKey::from_start(&start);

        assert_ne!(original, moved_start);
        assert_ne!(moved_start, changed_terrain);
    }
}
