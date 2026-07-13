//! Grid pathfinding. See `docs/design/server-sim.md` (`pathfinding.rs`).
//!
//! An 8-direction A* over the tile grid. A tile is blocked when its terrain is impassable
//! (ROCK / WATER) or when a building footprint occupies it. Units do NOT block each other —
//! movement allows soft overlap and is resolved by the movement system, so pathing only has
//! to route around static obstacles.
//!
//! For safety in the fixed-rate tick loop the search caps the number of expanded nodes. If
//! the goal is unreachable (or the cap is hit) we fall back to a best-effort: the path toward
//! the explored tile that ended up closest to the goal, so units still make progress instead
//! of freezing.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::config;

/// A passability oracle the pathfinder queries per tile: terrain AND dynamic building
/// footprints. Implemented by `systems`/`mod` which own the occupancy grid.
pub trait Passability {
    /// Whether a unit may stand on / traverse this tile.
    fn passable(&self, tx: i32, ty: i32) -> bool;

    /// Additional deterministic cost for entering this tile. Defaults to zero so callers that
    /// only need pass/fail behavior keep legacy path scoring.
    fn movement_cost(&self, _tx: i32, _ty: i32) -> u32 {
        0
    }
}

/// A* node in the open set, ordered by `f = g + h` (min-heap via `Reverse`-style `Ord`).
#[derive(Copy, Clone)]
struct Node {
    /// Estimated total cost (g + h), scaled to integer for a stable ordering.
    f: u32,
    /// Cost from start so far.
    g: u32,
    tx: i32,
    ty: i32,
    dir: u8,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.f == other.f
            && self.g == other.g
            && self.tx == other.tx
            && self.ty == other.ty
            && self.dir == other.dir
    }
}
impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse so `BinaryHeap` (a max-heap) yields the smallest score first, with a total
        // coordinate tie-break so replay cannot drift on equal-cost paths.
        other
            .f
            .cmp(&self.f)
            .then_with(|| other.g.cmp(&self.g))
            .then_with(|| other.ty.cmp(&self.ty))
            .then_with(|| other.tx.cmp(&self.tx))
            .then_with(|| other.dir.cmp(&self.dir))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Octile heuristic, scaled by 10 to keep integer math (cardinal = 10, diagonal = 14).
#[inline]
fn heuristic(ax: i32, ay: i32, bx: i32, by: i32) -> u32 {
    let dx = (ax - bx).unsigned_abs();
    let dy = (ay - by).unsigned_abs();
    let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
    // hi - lo straight moves at 10, lo diagonal moves at 14.
    14 * lo + 10 * (hi - lo)
}

/// The 8 neighbor offsets paired with their step cost (×10 scaling).
const NEIGHBORS: [(i32, i32, u32); 8] = [
    (1, 0, 10),
    (-1, 0, 10),
    (0, 1, 10),
    (0, -1, 10),
    (1, 1, 14),
    (1, -1, 14),
    (-1, 1, 14),
    (-1, -1, 14),
];

const NO_INCOMING_DIR: u8 = u8::MAX;

type SearchKey = (i32, i32, u8);

/// Reusable A* working storage owned by the pathing service.
///
/// Searches are strictly sequential inside one room. Clearing these containers between requests
/// preserves their allocations without making any search result depend on prior requests.
#[derive(Default)]
pub(super) struct SearchScratch {
    open: BinaryHeap<Node>,
    came_from: HashMap<SearchKey, SearchKey>,
    g_score: HashMap<SearchKey, u32>,
}

impl Clone for SearchScratch {
    fn clone(&self) -> Self {
        debug_assert!(self.open.is_empty());
        debug_assert!(self.came_from.is_empty());
        debug_assert!(self.g_score.is_empty());
        Self::default()
    }
}

impl SearchScratch {
    fn clear(&mut self) {
        self.open.clear();
        self.came_from.clear();
        self.g_score.clear();
    }

    #[cfg(test)]
    pub(super) fn retained_capacity(&self) -> usize {
        self.came_from.capacity() + self.g_score.capacity()
    }
}

/// Find a tile path from `(sx, sy)` to `(gx, gy)` with a configurable expansion cap.
///
/// Returns the sequence of tile coordinates to traverse, EXCLUDING the start tile and
/// INCLUDING the goal tile (or the closest reachable tile on best-effort). An empty vec means
/// "already there" or "nowhere useful to go". Diagonal moves are forbidden when they would
/// cut a corner between two blocked tiles (prevents clipping through walls).
/// Find a tile path with an optional deterministic direction-change penalty.
///
/// `turn_penalty` is added whenever a move's direction differs from the incoming direction.
/// A value of `0` preserves the legacy tile-only A* scoring.
#[allow(dead_code)]
pub fn find_path_with_budget_and_turn_cost<P: Passability>(
    pass: &P,
    sx: i32,
    sy: i32,
    gx: i32,
    gy: i32,
    max_expanded: usize,
    turn_penalty: u32,
) -> Vec<(i32, i32)> {
    let mut scratch = SearchScratch::default();
    find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
        pass,
        (sx, sy),
        (gx, gy),
        max_expanded,
        turn_penalty,
        &mut scratch,
    )
    .0
}

pub(super) fn find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch<P: Passability>(
    pass: &P,
    start: (i32, i32),
    goal: (i32, i32),
    max_expanded: usize,
    turn_penalty: u32,
    scratch: &mut SearchScratch,
) -> (Vec<(i32, i32)>, usize, bool) {
    scratch.clear();
    let (sx, sy) = start;
    let (gx, gy) = goal;
    if sx == gx && sy == gy {
        return (Vec::new(), 0, false);
    }

    // If the goal tile itself is blocked, retarget to the nearest passable tile around it so
    // we still walk up adjacent (e.g. building a structure, mining a node on a rock edge).
    let (gx, gy) = nearest_passable(pass, gx, gy).unwrap_or((gx, gy));

    // came_from[state] = predecessor state. State includes incoming direction when turn costs
    // are enabled, so paths to the same tile with different headings do not overwrite each other.
    // best known g per search state.
    let start_key = (sx, sy, NO_INCOMING_DIR);

    scratch.open.push(Node {
        f: heuristic(sx, sy, gx, gy),
        g: 0,
        tx: sx,
        ty: sy,
        dir: NO_INCOMING_DIR,
    });
    scratch.g_score.insert(start_key, 0);

    // Track the explored tile closest to the goal for the best-effort fallback.
    let mut best_key = start_key;
    let mut best_h = heuristic(sx, sy, gx, gy);

    let mut expanded = 0usize;
    let mut budget_exhausted = false;

    while let Some(cur) = scratch.open.pop() {
        let cur_key = (cur.tx, cur.ty, cur.dir);
        if cur.tx == gx && cur.ty == gy {
            let path = reconstruct(&scratch.came_from, cur_key);
            scratch.clear();
            return (path, expanded, budget_exhausted);
        }

        // Skip stale heap entries (a better g was found after this was pushed).
        if let Some(&best_g) = scratch.g_score.get(&cur_key) {
            if cur.g > best_g {
                continue;
            }
        }

        expanded += 1;
        if expanded > max_expanded {
            budget_exhausted = true;
            break;
        }

        for (dir, &(dx, dy, cost)) in NEIGHBORS.iter().enumerate() {
            let nx = cur.tx + dx;
            let ny = cur.ty + dy;
            if !pass.passable(nx, ny) {
                continue;
            }
            // No corner-cutting on diagonals: both orthogonally-adjacent tiles must be open.
            if dx != 0
                && dy != 0
                && (!pass.passable(cur.tx + dx, cur.ty) || !pass.passable(cur.tx, cur.ty + dy))
            {
                continue;
            }

            let dir = dir as u8;
            let turn_cost = if turn_penalty > 0 && cur.dir != NO_INCOMING_DIR && cur.dir != dir {
                turn_penalty
            } else {
                0
            };
            let next_dir = if turn_penalty > 0 {
                dir
            } else {
                NO_INCOMING_DIR
            };
            let next_key = (nx, ny, next_dir);
            let tentative = cur
                .g
                .saturating_add(cost)
                .saturating_add(turn_cost)
                .saturating_add(pass.movement_cost(nx, ny));
            let better = match scratch.g_score.get(&next_key) {
                Some(&existing) => tentative < existing,
                None => true,
            };
            if better {
                scratch.came_from.insert(next_key, cur_key);
                scratch.g_score.insert(next_key, tentative);
                let h = heuristic(nx, ny, gx, gy);
                if h < best_h {
                    best_h = h;
                    best_key = next_key;
                }
                scratch.open.push(Node {
                    f: tentative + h,
                    g: tentative,
                    tx: nx,
                    ty: ny,
                    dir: next_dir,
                });
            }
        }
    }

    // No complete path: head toward whatever we got closest to.
    let path = if (best_key.0, best_key.1) != (sx, sy) {
        reconstruct(&scratch.came_from, best_key)
    } else {
        Vec::new()
    };
    scratch.clear();
    (path, expanded, budget_exhausted)
}

/// Convert a tile path into world-pixel waypoints (tile centers), stored in REVERSE order so
/// the movement system can cheaply `pop` the next waypoint off the end.
pub fn to_world_waypoints(path: &[(i32, i32)]) -> Vec<(f32, f32)> {
    let ts = config::TILE_SIZE as f32;
    path.iter()
        .rev()
        .map(|&(tx, ty)| (tx as f32 * ts + ts * 0.5, ty as f32 * ts + ts * 0.5))
        .collect()
}

/// Find the nearest passable tile to `(tx, ty)` via an expanding ring search (radius up to 6).
/// Returns the tile itself if it is already passable.
fn nearest_passable<P: Passability>(pass: &P, tx: i32, ty: i32) -> Option<(i32, i32)> {
    if pass.passable(tx, ty) {
        return Some((tx, ty));
    }
    for r in 1i32..=6 {
        for dy in -r..=r {
            for dx in -r..=r {
                // Only the ring at exactly radius `r` (Chebyshev) to search outward in shells.
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                if pass.passable(tx + dx, ty + dy) {
                    return Some((tx + dx, ty + dy));
                }
            }
        }
    }
    None
}

/// Walk the `came_from` chain from `goal` back to the start, returning tiles in forward order
/// excluding the start tile.
fn reconstruct(came_from: &HashMap<SearchKey, SearchKey>, goal: SearchKey) -> Vec<(i32, i32)> {
    let mut path = vec![(goal.0, goal.1)];
    let mut cur = goal;
    while let Some(&prev) = came_from.get(&cur) {
        path.push((prev.0, prev.1));
        cur = prev;
    }
    // path is goal..start; drop the start tile and reverse to start..goal forward order.
    path.pop(); // remove the start tile
    path.reverse();
    path
}
