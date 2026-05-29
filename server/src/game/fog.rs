//! Per-player fog of war. See `DESIGN.md` §3 (`fog.rs`).
//!
//! The server is authoritative about visibility: each tick we recompute, for every player, a
//! boolean grid of which tiles that player can currently see. A tile is visible if it falls
//! within the sight circle of any of that player's entities (`sight_tiles`). The snapshot
//! layer uses this to withhold neutral/enemy entities standing on non-visible tiles, making
//! the fog cheat-proof.
//!
//! Note the server only needs *currently visible* — the client maintains the "explored but
//! not currently visible" dimming locally (see `DESIGN.md` §4). So this module tracks only
//! the per-tick visible set.

use std::collections::HashMap;

use crate::config;
use crate::game::entity::{Entity, EntityStore};

/// Visible-tile grids, one per player. Recomputed every tick from scratch (cheap at our map
/// sizes) so it always reflects current entity positions and never leaks stale visibility.
#[derive(Default)]
pub struct Fog {
    size: u32,
    /// player id -> row-major visibility grid (`true` = visible this tick).
    grids: HashMap<u32, Vec<bool>>,
}

impl Fog {
    pub fn new(size: u32) -> Self {
        Fog {
            size,
            grids: HashMap::new(),
        }
    }

    /// Recompute visibility for all `players` from the union of their entities' sight circles.
    /// Players with no entities get an all-dark grid.
    pub fn recompute(&mut self, players: &[u32], store: &EntityStore) {
        let cells = (self.size * self.size) as usize;
        // Reset / allocate a grid per player.
        for &p in players {
            let g = self.grids.entry(p).or_insert_with(|| vec![false; cells]);
            if g.len() != cells {
                *g = vec![false; cells];
            } else {
                g.iter_mut().for_each(|v| *v = false);
            }
        }

        for e in store.iter() {
            if e.owner == 0 {
                continue; // neutral resource nodes do not grant a player vision
            }
            // Only stamp sight for players we are tracking this tick.
            let Some(grid) = self.grids.get_mut(&e.owner) else {
                continue;
            };
            stamp_sight(grid, self.size, e);
        }
    }

    /// Whether `player` can currently see the tile `(tx, ty)`.
    pub fn is_visible(&self, player: u32, tx: u32, ty: u32) -> bool {
        if tx >= self.size || ty >= self.size {
            return false;
        }
        match self.grids.get(&player) {
            Some(g) => g[(ty * self.size + tx) as usize],
            None => false,
        }
    }

    /// Whether `player` can currently see the world-pixel point `(x, y)`.
    pub fn is_visible_world(&self, player: u32, x: f32, y: f32) -> bool {
        let ts = config::TILE_SIZE as f32;
        if x < 0.0 || y < 0.0 {
            return false;
        }
        let tx = (x / ts).floor() as i64;
        let ty = (y / ts).floor() as i64;
        if tx < 0 || ty < 0 || tx as u32 >= self.size || ty as u32 >= self.size {
            return false;
        }
        self.is_visible(player, tx as u32, ty as u32)
    }
}

/// Mark every tile within an entity's sight radius (a filled circle in tile space) as visible.
fn stamp_sight(grid: &mut [bool], size: u32, e: &Entity) {
    let r = e.sight_tiles() as i32;
    if r <= 0 {
        return;
    }
    let ts = config::TILE_SIZE as f32;
    let cx = (e.pos_x / ts).floor() as i32;
    let cy = (e.pos_y / ts).floor() as i32;
    let r2 = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let tx = cx + dx;
            let ty = cy + dy;
            if tx < 0 || ty < 0 || tx as u32 >= size || ty as u32 >= size {
                continue;
            }
            grid[(ty as u32 * size + tx as u32) as usize] = true;
        }
    }
}
