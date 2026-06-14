use std::collections::{BTreeMap, VecDeque};

use crate::game::entity::EntityKind;
use crate::game::map::{Map, Tile};
use crate::rules::terrain::{self, TerrainKind};

mod anchors;
mod diagnostics;

use anchors::{build_anchors, AtlasAnchor};
pub(super) use diagnostics::atlas_diagnostics_json;

const REGION_SIZE_TILES: u32 = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MapAtlas {
    movement_layers: Vec<MovementLayerAtlas>,
    anchors: Vec<AtlasAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MovementLayerAtlas {
    movement_class: MovementClass,
    passable_tiles: Vec<bool>,
    clearance_tiles: Vec<u16>,
    component_by_tile: Vec<Option<usize>>,
    components: Vec<AtlasComponent>,
    region_by_tile: Vec<Option<usize>>,
    regions: Vec<AtlasRegion>,
    portals: Vec<AtlasPortal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum MovementClass {
    Infantry,
    Vehicle,
}

impl MovementClass {
    pub const ALL: [MovementClass; 2] = [MovementClass::Infantry, MovementClass::Vehicle];

    fn as_str(self) -> &'static str {
        match self {
            MovementClass::Infantry => "infantry",
            MovementClass::Vehicle => "vehicle",
        }
    }

    fn representative_kind(self) -> EntityKind {
        match self {
            MovementClass::Infantry => EntityKind::Rifleman,
            MovementClass::Vehicle => EntityKind::ScoutCar,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AtlasComponent {
    id: usize,
    tile_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AtlasRegion {
    id: usize,
    component_id: usize,
    min_tile: Tile,
    max_tile: Tile,
    tile_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AtlasPortal {
    id: usize,
    movement_class: MovementClass,
    component_id: usize,
    from_region: usize,
    to_region: usize,
    center_tile: Tile,
    width_tiles: u16,
}

impl MapAtlas {
    pub(super) fn generate(map: &Map) -> Self {
        let movement_layers: Vec<_> = MovementClass::ALL
            .into_iter()
            .map(|movement_class| MovementLayerAtlas::generate(map, movement_class))
            .collect();
        let anchors = build_anchors(map, &movement_layers);

        MapAtlas {
            movement_layers,
            anchors,
        }
    }

    #[cfg(test)]
    fn layer(&self, movement_class: MovementClass) -> Option<&MovementLayerAtlas> {
        self.movement_layers
            .iter()
            .find(|layer| layer.movement_class == movement_class)
    }

    pub(super) fn validate(&self) {
        debug_assert_eq!(self.movement_layers.len(), MovementClass::ALL.len());
        debug_assert!(!self.anchors.is_empty());
        for layer in &self.movement_layers {
            layer.validate();
        }
        for anchor in &self.anchors {
            anchor.validate();
        }
    }
}

impl MovementLayerAtlas {
    fn generate(map: &Map, movement_class: MovementClass) -> Self {
        let passable_tiles = build_playable_passable_tiles(map, movement_class);
        let clearance_tiles = build_clearance_field(map, &passable_tiles);
        let (component_by_tile, components) = build_components(map, &passable_tiles);
        let (region_by_tile, regions) = build_regions(map, &passable_tiles, &component_by_tile);
        let portals = build_portals(map, movement_class, &region_by_tile, &regions);

        MovementLayerAtlas {
            movement_class,
            passable_tiles,
            clearance_tiles,
            component_by_tile,
            components,
            region_by_tile,
            regions,
            portals,
        }
    }

    fn tile_index(&self, map: &Map, tile: Tile) -> Option<usize> {
        if tile.0 < map.size && tile.1 < map.size {
            Some(map.index(tile.0, tile.1))
        } else {
            None
        }
    }

    fn component_at(&self, map: &Map, tile: Tile) -> Option<usize> {
        self.tile_index(map, tile)
            .and_then(|idx| self.component_by_tile[idx])
    }

    fn region_at(&self, map: &Map, tile: Tile) -> Option<usize> {
        self.tile_index(map, tile)
            .and_then(|idx| self.region_by_tile[idx])
    }

    fn validate(&self) {
        debug_assert!(MovementClass::ALL.contains(&self.movement_class));
        debug_assert_eq!(self.passable_tiles.len(), self.clearance_tiles.len());
        debug_assert_eq!(self.passable_tiles.len(), self.component_by_tile.len());
        debug_assert_eq!(self.passable_tiles.len(), self.region_by_tile.len());
        for (index, component) in self.components.iter().enumerate() {
            debug_assert_eq!(component.id, index);
            debug_assert!(component.tile_count > 0);
        }
        for (index, region) in self.regions.iter().enumerate() {
            debug_assert_eq!(region.id, index);
            debug_assert!(self.components.get(region.component_id).is_some());
            debug_assert!(region.tile_count > 0);
        }
        for (index, portal) in self.portals.iter().enumerate() {
            debug_assert_eq!(portal.id, index);
            debug_assert_eq!(portal.movement_class, self.movement_class);
            debug_assert!(self.components.get(portal.component_id).is_some());
            debug_assert!(self.regions.get(portal.from_region).is_some());
            debug_assert!(self.regions.get(portal.to_region).is_some());
            debug_assert!(portal.width_tiles > 0);
        }
    }
}

fn build_playable_passable_tiles(map: &Map, movement_class: MovementClass) -> Vec<bool> {
    let terrain_passable_tiles = build_terrain_passable_tiles(map, movement_class);
    let (component_by_tile, _) = build_components(map, &terrain_passable_tiles);
    let playable_component = playable_component_id(map, &component_by_tile);

    match playable_component {
        Some(component_id) => terrain_passable_tiles
            .into_iter()
            .enumerate()
            .map(|(idx, passable)| passable && component_by_tile[idx] == Some(component_id))
            .collect(),
        None => terrain_passable_tiles,
    }
}

fn playable_component_id(map: &Map, component_by_tile: &[Option<usize>]) -> Option<usize> {
    let playable_component = map.starts.iter().find_map(|&(x, y)| {
        if x < map.size && y < map.size {
            component_by_tile[map.index(x, y)]
        } else {
            None
        }
    });

    debug_assert!(
        map.starts.iter().all(|&(x, y)| {
            x >= map.size
                || y >= map.size
                || component_by_tile[map.index(x, y)].is_none()
                || component_by_tile[map.index(x, y)] == playable_component
        }),
        "atlas generation assumes selected starts share one passable component"
    );

    playable_component
}

fn build_terrain_passable_tiles(map: &Map, movement_class: MovementClass) -> Vec<bool> {
    (0..map.size)
        .flat_map(|y| {
            (0..map.size).map(move |x| {
                let Some(terrain_kind) = TerrainKind::from_map_code(map.terrain_at(x, y)) else {
                    return false;
                };
                terrain::movement_allowed(movement_class.representative_kind(), terrain_kind)
            })
        })
        .collect()
}

fn build_clearance_field(map: &Map, passable_tiles: &[bool]) -> Vec<u16> {
    let size = map.size as i32;
    let len = (map.size * map.size) as usize;
    let mut clearance = vec![u16::MAX; len];
    let mut queue = VecDeque::new();

    for ty in 0..size {
        for tx in 0..size {
            let idx = (ty as u32 * map.size + tx as u32) as usize;
            if !passable_tiles[idx] {
                clearance[idx] = 0;
                queue.push_back((tx, ty));
            }
        }
    }

    while let Some((tx, ty)) = queue.pop_front() {
        let idx = (ty as u32 * map.size + tx as u32) as usize;
        let next_clearance = clearance[idx].saturating_add(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = tx + dx;
                let ny = ty + dy;
                if !map.in_bounds(nx, ny) {
                    continue;
                }
                let nidx = (ny as u32 * map.size + nx as u32) as usize;
                if next_clearance < clearance[nidx] {
                    clearance[nidx] = next_clearance;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    for ty in 0..size {
        for tx in 0..size {
            let idx = (ty as u32 * map.size + tx as u32) as usize;
            let edge_clearance = (tx + 1).min(ty + 1).min(size - tx).min(size - ty) as u16;
            clearance[idx] = if passable_tiles[idx] {
                clearance[idx].min(edge_clearance)
            } else {
                0
            };
        }
    }

    clearance
}

fn build_components(
    map: &Map,
    passable_tiles: &[bool],
) -> (Vec<Option<usize>>, Vec<AtlasComponent>) {
    let len = (map.size * map.size) as usize;
    let mut component_by_tile = vec![None; len];
    let mut components = Vec::new();

    for y in 0..map.size {
        for x in 0..map.size {
            let idx = map.index(x, y);
            if !passable_tiles[idx] || component_by_tile[idx].is_some() {
                continue;
            }

            let id = components.len();
            let mut tile_count = 0u32;
            let mut queue = VecDeque::from([(x, y)]);
            component_by_tile[idx] = Some(id);

            while let Some((tx, ty)) = queue.pop_front() {
                tile_count += 1;
                for (nx, ny) in cardinal_neighbors(map, tx, ty) {
                    let nidx = map.index(nx, ny);
                    if passable_tiles[nidx] && component_by_tile[nidx].is_none() {
                        component_by_tile[nidx] = Some(id);
                        queue.push_back((nx, ny));
                    }
                }
            }

            components.push(AtlasComponent { id, tile_count });
        }
    }

    (component_by_tile, components)
}

fn build_regions(
    map: &Map,
    passable_tiles: &[bool],
    component_by_tile: &[Option<usize>],
) -> (Vec<Option<usize>>, Vec<AtlasRegion>) {
    let len = (map.size * map.size) as usize;
    let mut region_by_tile = vec![None; len];
    let mut regions = Vec::new();

    for y in 0..map.size {
        for x in 0..map.size {
            let idx = map.index(x, y);
            if !passable_tiles[idx] || region_by_tile[idx].is_some() {
                continue;
            }
            let Some(component_id) = component_by_tile[idx] else {
                continue;
            };
            let bucket = (x / REGION_SIZE_TILES, y / REGION_SIZE_TILES);
            let id = regions.len();
            let mut region = AtlasRegion {
                id,
                component_id,
                min_tile: (x, y),
                max_tile: (x, y),
                tile_count: 0,
            };
            let mut queue = VecDeque::from([(x, y)]);
            region_by_tile[idx] = Some(id);

            while let Some((tx, ty)) = queue.pop_front() {
                region.min_tile.0 = region.min_tile.0.min(tx);
                region.min_tile.1 = region.min_tile.1.min(ty);
                region.max_tile.0 = region.max_tile.0.max(tx);
                region.max_tile.1 = region.max_tile.1.max(ty);
                region.tile_count += 1;

                for (nx, ny) in cardinal_neighbors(map, tx, ty) {
                    if (nx / REGION_SIZE_TILES, ny / REGION_SIZE_TILES) != bucket {
                        continue;
                    }
                    let nidx = map.index(nx, ny);
                    if passable_tiles[nidx]
                        && component_by_tile[nidx] == Some(component_id)
                        && region_by_tile[nidx].is_none()
                    {
                        region_by_tile[nidx] = Some(id);
                        queue.push_back((nx, ny));
                    }
                }
            }

            regions.push(region);
        }
    }

    (region_by_tile, regions)
}

fn build_portals(
    map: &Map,
    movement_class: MovementClass,
    region_by_tile: &[Option<usize>],
    regions: &[AtlasRegion],
) -> Vec<AtlasPortal> {
    let mut runs: BTreeMap<(usize, usize), PortalRun> = BTreeMap::new();

    for y in 0..map.size {
        for x in 0..map.size {
            let Some(region) = region_by_tile[map.index(x, y)] else {
                continue;
            };
            for (nx, ny) in [(x.saturating_add(1), y), (x, y.saturating_add(1))] {
                if nx >= map.size || ny >= map.size {
                    continue;
                }
                let Some(other) = region_by_tile[map.index(nx, ny)] else {
                    continue;
                };
                if region == other {
                    continue;
                }
                let (from_region, to_region) = if region < other {
                    (region, other)
                } else {
                    (other, region)
                };
                runs.entry((from_region, to_region))
                    .and_modify(|run| run.add((x + nx, y + ny)))
                    .or_insert_with(|| PortalRun::new((x + nx, y + ny)));
            }
        }
    }

    runs.into_iter()
        .enumerate()
        .filter_map(|(id, ((from_region, to_region), run))| {
            let from = regions.get(from_region)?;
            let to = regions.get(to_region)?;
            if from.component_id != to.component_id {
                return None;
            }
            Some(AtlasPortal {
                id,
                movement_class,
                component_id: from.component_id,
                from_region,
                to_region,
                center_tile: run.center_tile(),
                width_tiles: run.width_tiles(),
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct PortalRun {
    count: u32,
    sum_x2: u64,
    sum_y2: u64,
}

impl PortalRun {
    fn new(center_twice: Tile) -> Self {
        PortalRun {
            count: 1,
            sum_x2: center_twice.0 as u64,
            sum_y2: center_twice.1 as u64,
        }
    }

    fn add(&mut self, center_twice: Tile) {
        self.count += 1;
        self.sum_x2 += center_twice.0 as u64;
        self.sum_y2 += center_twice.1 as u64;
    }

    fn center_tile(&self) -> Tile {
        (
            ((self.sum_x2 / self.count as u64) / 2) as u32,
            ((self.sum_y2 / self.count as u64) / 2) as u32,
        )
    }

    fn width_tiles(&self) -> u16 {
        self.count.min(u16::MAX as u32) as u16
    }
}

fn cardinal_neighbors(map: &Map, x: u32, y: u32) -> impl Iterator<Item = Tile> + '_ {
    [
        (x.checked_sub(1), Some(y)),
        (x.checked_add(1).filter(|nx| *nx < map.size), Some(y)),
        (Some(x), y.checked_sub(1)),
        (Some(x), y.checked_add(1).filter(|ny| *ny < map.size)),
    ]
    .into_iter()
    .filter_map(|(x, y)| Some((x?, y?)))
}

#[cfg(test)]
mod atlas_tests;
