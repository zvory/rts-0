use crate::config;
use crate::game::map::{Map, Tile};

use super::{MovementClass, MovementLayerAtlas};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AtlasAnchor {
    id: String,
    kind: AtlasAnchorKind,
    tile: Tile,
    movement_class: MovementClass,
    pub(super) component_id: Option<usize>,
    pub(super) region_id: Option<usize>,
}

impl AtlasAnchor {
    pub(super) fn validate(&self) {
        debug_assert!(!self.id.is_empty());
        debug_assert!(self.tile.0 < u32::MAX);
        debug_assert!(self.tile.1 < u32::MAX);
        debug_assert!(matches!(
            self.kind,
            AtlasAnchorKind::Main
                | AtlasAnchorKind::Natural
                | AtlasAnchorKind::ResourceCluster
                | AtlasAnchorKind::ResourceLineApproach
        ));
        debug_assert!(MovementClass::ALL.contains(&self.movement_class));
        debug_assert_eq!(self.component_id.is_some(), self.region_id.is_some());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum AtlasAnchorKind {
    Main,
    Natural,
    ResourceCluster,
    ResourceLineApproach,
}

pub(super) fn build_anchors(map: &Map, layers: &[MovementLayerAtlas]) -> Vec<AtlasAnchor> {
    let mut anchors = Vec::new();
    for (index, tile) in map.starts.iter().copied().enumerate() {
        push_anchor_for_all_layers(
            &mut anchors,
            map,
            layers,
            format!("main:{index}"),
            AtlasAnchorKind::Main,
            tile,
        );
        push_anchor_for_all_layers(
            &mut anchors,
            map,
            layers,
            format!("resource-cluster:main:{index}"),
            AtlasAnchorKind::ResourceCluster,
            resource_cluster_tile(map, tile),
        );
        push_anchor_for_all_layers(
            &mut anchors,
            map,
            layers,
            format!("resource-line-approach:main:{index}"),
            AtlasAnchorKind::ResourceLineApproach,
            resource_line_approach_tile(map, tile),
        );
    }

    for (index, tile) in map.expansion_sites.iter().copied().enumerate() {
        push_anchor_for_all_layers(
            &mut anchors,
            map,
            layers,
            format!("natural:{index}"),
            AtlasAnchorKind::Natural,
            tile,
        );
        push_anchor_for_all_layers(
            &mut anchors,
            map,
            layers,
            format!("resource-cluster:natural:{index}"),
            AtlasAnchorKind::ResourceCluster,
            resource_cluster_tile(map, tile),
        );
        push_anchor_for_all_layers(
            &mut anchors,
            map,
            layers,
            format!("resource-line-approach:natural:{index}"),
            AtlasAnchorKind::ResourceLineApproach,
            resource_line_approach_tile(map, tile),
        );
    }

    anchors.sort_by(|a, b| {
        (a.id.as_str(), a.kind, a.tile.1, a.tile.0, a.movement_class).cmp(&(
            b.id.as_str(),
            b.kind,
            b.tile.1,
            b.tile.0,
            b.movement_class,
        ))
    });
    anchors
}

fn push_anchor_for_all_layers(
    anchors: &mut Vec<AtlasAnchor>,
    map: &Map,
    layers: &[MovementLayerAtlas],
    id: String,
    kind: AtlasAnchorKind,
    tile: Tile,
) {
    for layer in layers {
        anchors.push(AtlasAnchor {
            id: id.clone(),
            kind,
            tile,
            movement_class: layer.movement_class,
            component_id: layer.component_at(map, tile),
            region_id: layer.region_at(map, tile),
        });
    }
}

fn resource_cluster_tile(map: &Map, tile: Tile) -> Tile {
    directional_tile(map, tile, config::STEEL_BLOCK_DIST_TILES)
}

fn resource_line_approach_tile(map: &Map, tile: Tile) -> Tile {
    directional_tile(map, tile, config::STEEL_BLOCK_DIST_TILES + 2.0)
}

fn directional_tile(map: &Map, tile: Tile, distance_tiles: f32) -> Tile {
    let (hx, hy) = map.tile_center(tile.0, tile.1);
    let center = map.world_size_px() * 0.5;
    let angle = (center - hy).atan2(center - hx);
    let distance_px = distance_tiles * config::TILE_SIZE as f32;
    map.tile_of(
        hx + distance_px * angle.cos(),
        hy + distance_px * angle.sin(),
    )
}
