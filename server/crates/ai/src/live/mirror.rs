//! Jeff's AI V1.1 plays every start of a symmetric map the same way (`mirror`). The game's maps are
//! mirror or half-turn images of themselves, but Jeff's rules are not: placement searches, sides
//! picked for flanks and approaches, and ties broken by position all fall differently once left and
//! right swap. Over six 120s on beta bee2b6c (2026-11-06) Jeff V1.1 ended good/bad 26/3 from
//! Crossroads' top start against Pressure AI and 0/29 from its right start, and 30/0 from Schone
//! Tage's east start against Rush AI and 3/19 from its west; from the right start his first Factory
//! stood 23 tiles from where the top start's mirrored one did.
//!
//! - **One frame**: on a two-player map whose terrain maps one start onto the other (half-turn,
//!   left-right or up-down mirror, or either diagonal), Jeff decides as if he stood on the
//!   canonical start. Everything he reads is mapped into that frame first, the map once a match and
//!   each decision's snapshot, and every order he gives is mapped back. All his memory lives in the
//!   canonical frame.
//! - **Canonical start**: on the maps played, the start Jeff won from (`CANONICAL_STARTS`); on
//!   others, the first start by row then column.
//! - **Footprints**: a Build order keeps its building's centre; across a diagonal a 3x2 building
//!   cannot turn, so its anchor rounds half a tile.

use std::sync::Arc;

use rts_rules::terrain::{
    MAP_TERRAIN_ROAD_DIAGONAL_NE_SW, MAP_TERRAIN_ROAD_DIAGONAL_NW_SE, MAP_TERRAIN_ROAD_HORIZONTAL,
    MAP_TERRAIN_ROAD_VERTICAL,
};
use rts_sim::game::command::SimCommand;
use rts_sim::protocol::{MapTile, Snapshot, StartPayload};

use crate::config;

/// A map by its size and two starts, and its canonical start.
type CanonicalStart = ((u32, u32), [(u32, u32); 2], (u32, u32));

/// Map size, both starts, and the start the other replicates: the side Jeff V1.1 won more from in
/// the 120s above (Crossroads, Schone Tage, Classic, The River).
const CANONICAL_STARTS: [CanonicalStart; 4] = [
    ((126, 126), [(47, 8), (117, 78)], (47, 8)),
    ((166, 166), [(157, 47), (8, 47)], (157, 47)),
    ((189, 126), [(40, 9), (147, 116)], (147, 116)),
    ((126, 126), [(9, 9), (116, 116)], (116, 116)),
];
/// A symmetry holds when at most this share of the map's tiles differ from their images.
const MAX_MISMATCH_SHARE: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Symmetry {
    HalfTurn,
    MirrorX,
    MirrorY,
    AntiDiagonal,
    MainDiagonal,
}

const SYMMETRIES: [Symmetry; 5] = [
    Symmetry::HalfTurn,
    Symmetry::MirrorX,
    Symmetry::MirrorY,
    Symmetry::AntiDiagonal,
    Symmetry::MainDiagonal,
];

/// The map's symmetry, carrying one start onto the other. Every one is its own inverse, so the same
/// mapping takes the world into the canonical frame and orders back out of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MirrorFrame {
    symmetry: Symmetry,
    width: u32,
    height: u32,
    tile_size: u32,
}

/// The frame and the start payload as seen from the canonical start.
#[derive(Clone, Debug)]
pub(super) struct Mirror {
    pub(super) frame: MirrorFrame,
    pub(super) start: Arc<StartPayload>,
}

impl MirrorFrame {
    fn tile(&self, (x, y): (u32, u32)) -> (u32, u32) {
        let (w, h) = (self.width.saturating_sub(1), self.height.saturating_sub(1));
        let (x, y) = (x.min(w), y.min(h));
        match self.symmetry {
            Symmetry::HalfTurn => (w - x, h - y),
            Symmetry::MirrorX => (w - x, y),
            Symmetry::MirrorY => (x, h - y),
            Symmetry::AntiDiagonal => (h - y, w - x),
            Symmetry::MainDiagonal => (y, x),
        }
    }

    /// A point in tile units, continuous.
    fn tile_point(&self, x: f32, y: f32) -> (f32, f32) {
        let (w, h) = (self.width as f32, self.height as f32);
        match self.symmetry {
            Symmetry::HalfTurn => (w - x, h - y),
            Symmetry::MirrorX => (w - x, y),
            Symmetry::MirrorY => (x, h - y),
            Symmetry::AntiDiagonal => (h - y, w - x),
            Symmetry::MainDiagonal => (y, x),
        }
    }

    /// A point in world pixels.
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        let ts = self.tile_size.max(1) as f32;
        let (tx, ty) = self.tile_point(x / ts, y / ts);
        (tx * ts, ty * ts)
    }

    /// Roads keep their way across the turned map.
    fn terrain_code(&self, code: u8) -> u8 {
        let transposes = matches!(
            self.symmetry,
            Symmetry::AntiDiagonal | Symmetry::MainDiagonal
        );
        let mirrors = matches!(self.symmetry, Symmetry::MirrorX | Symmetry::MirrorY);
        match code {
            MAP_TERRAIN_ROAD_HORIZONTAL if transposes => MAP_TERRAIN_ROAD_VERTICAL,
            MAP_TERRAIN_ROAD_VERTICAL if transposes => MAP_TERRAIN_ROAD_HORIZONTAL,
            MAP_TERRAIN_ROAD_DIAGONAL_NW_SE if mirrors => MAP_TERRAIN_ROAD_DIAGONAL_NE_SW,
            MAP_TERRAIN_ROAD_DIAGONAL_NE_SW if mirrors => MAP_TERRAIN_ROAD_DIAGONAL_NW_SE,
            other => other,
        }
    }

    /// A row-major grid of one byte per tile.
    fn grid(&self, grid: &[u8], map_codes: bool) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        if grid.len() != w * h {
            return grid.to_vec();
        }
        let mut out = vec![0; grid.len()];
        for y in 0..h {
            for x in 0..w {
                let (tx, ty) = self.tile((x as u32, y as u32));
                let value = grid[y * w + x];
                out[ty as usize * w + tx as usize] = if map_codes {
                    self.terrain_code(value)
                } else {
                    value
                };
            }
        }
        out
    }

    fn tiles(&self, tiles: &[MapTile]) -> Vec<MapTile> {
        tiles
            .iter()
            .map(|tile| {
                let (x, y) = self.tile((tile.x, tile.y));
                MapTile { x, y }
            })
            .collect()
    }

    fn start_payload(&self, start: &StartPayload) -> StartPayload {
        let mut out = start.clone();
        let map = &mut out.map;
        map.terrain = self.grid(&start.map.terrain, true);
        map.elevation = self.grid(&start.map.elevation, false);
        for resource in &mut map.resources {
            (resource.x, resource.y) = self.point(resource.x, resource.y);
        }
        for doodad in &mut map.doodads {
            let (x, y) = self.point(doodad.x as f32, doodad.y as f32);
            (doodad.x, doodad.y) = (x.max(0.0).round() as u32, y.max(0.0).round() as u32);
        }
        map.concealment_tiles = self.tiles(&start.map.concealment_tiles);
        map.no_vehicle_tiles = self.tiles(&start.map.no_vehicle_tiles);
        map.no_building_tiles = self.tiles(&start.map.no_building_tiles);
        map.no_entrenchment_tiles = self.tiles(&start.map.no_entrenchment_tiles);
        map.damage_reduction_tiles = self.tiles(&start.map.damage_reduction_tiles);
        map.slow_movement_tiles = self.tiles(&start.map.slow_movement_tiles);
        for player in &mut out.players {
            (player.start_tile_x, player.start_tile_y) =
                self.tile((player.start_tile_x, player.start_tile_y));
        }
        out
    }

    /// The snapshot as seen from the canonical start: every position the AI reads.
    pub(super) fn snapshot(&self, snapshot: &Snapshot) -> Snapshot {
        let mut out = snapshot.clone();
        for entity in &mut out.entities {
            (entity.x, entity.y) = self.point(entity.x, entity.y);
            if let Some([x, y]) = entity.rally {
                let (x, y) = self.point(x, y);
                entity.rally = Some([x, y]);
            }
        }
        for building in &mut out.remembered_buildings {
            (building.x, building.y) = self.point(building.x, building.y);
            for tile in &mut building.footprint {
                let (x, y) = self.tile((tile[0], tile[1]));
                *tile = [x, y];
            }
        }
        for gun in &mut out.remembered_anti_tank_guns {
            (gun.x, gun.y) = self.point(gun.x, gun.y);
        }
        for smoke in &mut out.smokes {
            (smoke.x, smoke.y) = self.point(smoke.x, smoke.y);
        }
        if let Some([x, y]) = out.world_combat_position {
            let (x, y) = self.point(x, y);
            out.world_combat_position = Some([x, y]);
        }
        out.visible_tiles = self.grid(&snapshot.visible_tiles, false);
        out.explored_tiles = self.grid(&snapshot.explored_tiles, false);
        out
    }

    /// An order given in the canonical frame, for the real map.
    pub(super) fn command(&self, command: SimCommand) -> SimCommand {
        let point = |x: f32, y: f32| self.point(x, y);
        match command {
            SimCommand::Move {
                units,
                x,
                y,
                queued,
            } => {
                let (x, y) = point(x, y);
                SimCommand::Move {
                    units,
                    x,
                    y,
                    queued,
                }
            }
            SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            } => {
                let (x, y) = point(x, y);
                SimCommand::AttackMove {
                    units,
                    x,
                    y,
                    queued,
                }
            }
            SimCommand::FormationMove {
                units,
                points,
                attack_move,
                queued,
            } => SimCommand::FormationMove {
                units,
                points: points.into_iter().map(|(x, y)| point(x, y)).collect(),
                attack_move,
                queued,
            },
            SimCommand::SetupAntiTankGuns {
                units,
                x,
                y,
                queued,
            } => {
                let (x, y) = point(x, y);
                SimCommand::SetupAntiTankGuns {
                    units,
                    x,
                    y,
                    queued,
                }
            }
            SimCommand::UseAbility {
                ability,
                units,
                x: Some(x),
                y: Some(y),
                queued,
            } => {
                let (x, y) = point(x, y);
                SimCommand::UseAbility {
                    ability,
                    units,
                    x: Some(x),
                    y: Some(y),
                    queued,
                }
            }
            SimCommand::ArtilleryFire {
                units,
                x,
                y,
                radius_tiles,
                queued,
            } => {
                let (x, y) = point(x, y);
                SimCommand::ArtilleryFire {
                    units,
                    x,
                    y,
                    radius_tiles,
                    queued,
                }
            }
            SimCommand::PointTanks {
                units,
                x,
                y,
                queued,
            } => {
                let (x, y) = point(x, y);
                SimCommand::PointTanks {
                    units,
                    x,
                    y,
                    queued,
                }
            }
            SimCommand::SetRally {
                building,
                x,
                y,
                kind,
                queued,
            } => {
                let (x, y) = point(x, y);
                SimCommand::SetRally {
                    building,
                    x,
                    y,
                    kind,
                    queued,
                }
            }
            SimCommand::Build {
                units,
                building,
                tile_x,
                tile_y,
                queued,
            } => {
                let (fw, fh) = config::building_stats(building)
                    .map_or((1, 1), |stats| (stats.foot_w, stats.foot_h));
                let (cx, cy) = self.tile_point(
                    tile_x as f32 + fw as f32 * 0.5,
                    tile_y as f32 + fh as f32 * 0.5,
                );
                let anchor = |center: f32, size: u32, limit: u32| {
                    (center - size as f32 * 0.5)
                        .round()
                        .clamp(0.0, limit.saturating_sub(size) as f32) as u32
                };
                SimCommand::Build {
                    units,
                    building,
                    tile_x: anchor(cx, fw, self.width),
                    tile_y: anchor(cy, fh, self.height),
                    queued,
                }
            }
            other => other,
        }
    }
}

/// The frame Jeff decides in when he does not stand on the canonical start; `None` on the
/// canonical start and on maps without a symmetry carrying one start onto the other.
pub(super) fn mirror_for(start: &StartPayload, player: u32) -> Option<Mirror> {
    if start.players.len() != 2 {
        return None;
    }
    let own = start.players.iter().find(|p| p.id == player)?;
    let other = start.players.iter().find(|p| p.id != player)?;
    let own_start = (own.start_tile_x, own.start_tile_y);
    let other_start = (other.start_tile_x, other.start_tile_y);
    let size = (start.map.width, start.map.height);
    let canonical = CANONICAL_STARTS
        .iter()
        .find(|(map, starts, _)| {
            *map == size && starts.contains(&own_start) && starts.contains(&other_start)
        })
        .map(|(_, _, canonical)| *canonical)
        .unwrap_or_else(|| {
            if (own_start.1, own_start.0) <= (other_start.1, other_start.0) {
                own_start
            } else {
                other_start
            }
        });
    if canonical == own_start {
        return None;
    }
    let near = |a: (u32, u32), b: (u32, u32)| a.0.abs_diff(b.0) <= 1 && a.1.abs_diff(b.1) <= 1;
    let tiles = (size.0 * size.1) as usize;
    let frame = SYMMETRIES
        .iter()
        .filter(|symmetry| {
            !matches!(symmetry, Symmetry::AntiDiagonal | Symmetry::MainDiagonal) || size.0 == size.1
        })
        .map(|symmetry| MirrorFrame {
            symmetry: *symmetry,
            width: size.0,
            height: size.1,
            tile_size: start.map.tile_size,
        })
        .filter(|frame| near(frame.tile(own_start), other_start))
        .map(|frame| {
            let mismatches = if start.map.terrain.len() == tiles {
                let mirrored = frame.grid(&start.map.terrain, true);
                mirrored
                    .iter()
                    .zip(&start.map.terrain)
                    .filter(|(a, b)| a != b)
                    .count()
            } else {
                tiles
            };
            (frame, mismatches)
        })
        .filter(|(_, mismatches)| (*mismatches as f32) <= tiles as f32 * MAX_MISMATCH_SHARE)
        .min_by_key(|(_, mismatches)| *mismatches)
        .map(|(frame, _)| frame)?;
    Some(Mirror {
        frame,
        start: Arc::new(frame.start_payload(start)),
    })
}
