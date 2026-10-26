use super::{tile_center_world, AiMapAnalysis};

impl AiMapAnalysis {
    pub(super) fn sdk_base_sites(&self) -> Vec<crate::sdk::AiBaseSite> {
        use crate::sdk::{AiBaseSite, AiBaseSiteId};
        let mut sites = Vec::new();
        let mut home_clusters = std::collections::BTreeSet::new();
        for start in &self.starts {
            let cluster = self
                .resource_clusters
                .iter()
                .find(|cluster| Some(cluster.id) == start.nearest_resource_cluster_id);
            if let Some(cluster) = cluster {
                home_clusters.insert(cluster.id);
            }
            sites.push(AiBaseSite {
                id: AiBaseSiteId::Start(start.player_id),
                position: tile_center_world(start.start_tile, self.tile_size),
                resource_ids: cluster.map(|c| c.resource_ids.clone()).unwrap_or_default(),
                ground_distances_from_starts: self.sdk_base_distances(start.start_tile),
            });
        }
        for cluster in &self.resource_clusters {
            if !home_clusters.contains(&cluster.id) {
                sites.push(AiBaseSite {
                    id: AiBaseSiteId::ResourceCluster(cluster.id),
                    position: tile_center_world(cluster.center_tile, self.tile_size),
                    resource_ids: cluster.resource_ids.clone(),
                    ground_distances_from_starts: self.sdk_base_distances(cluster.center_tile),
                });
            }
        }
        sites.sort_by_key(|site| site.id);
        sites
    }

    fn sdk_base_distances(&self, tile: super::AiTile) -> std::collections::BTreeMap<u32, f32> {
        self.starts
            .iter()
            .filter_map(|start| {
                // The original main is rank 1 even on a degenerate map with no passable terrain.
                let distance = if start.start_tile == tile {
                    Some(0.0)
                } else {
                    self.ground_distance_from_start(start.player_id, (tile.x, tile.y))
                }?;
                Some((start.player_id, distance * self.tile_size as f32))
            })
            .collect()
    }
}
