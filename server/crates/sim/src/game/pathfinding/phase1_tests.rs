use super::*;
use std::collections::{BinaryHeap, HashMap};
use std::time::Instant;

const CORPUS_VERSION: u32 = 1;
const CAPS: [usize; 7] = [0, 1, 2, 8, 64, 4096, 32768];

#[derive(Clone)]
struct Grid {
    width: i32,
    height: i32,
    blocked: Vec<bool>,
}

impl Grid {
    fn patterned(width: i32, height: i32, seed: u32) -> Self {
        let mut blocked = vec![false; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let edge = x == 0 || y == 0 || x == width - 1 || y == height - 1;
                let stripe = x > 4
                    && y > 4
                    && ((x * 17 + y * 31 + seed as i32 * 13) % 47 == 0)
                    && (x + y) % 5 != 0;
                blocked[(y * width + x) as usize] = edge || stripe;
            }
        }
        Self {
            width,
            height,
            blocked,
        }
    }

    fn open(width: i32, height: i32) -> Self {
        let mut grid = Self::patterned(width, height, 0);
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                grid.blocked[(y * width + x) as usize] = false;
            }
        }
        grid
    }
}

impl Passability for Grid {
    fn passable(&self, tx: i32, ty: i32) -> bool {
        tx >= 0
            && ty >= 0
            && tx < self.width
            && ty < self.height
            && !self.blocked[(ty * self.width + tx) as usize]
    }
}

#[derive(Clone, Copy)]
struct Query {
    start: (i32, i32),
    goal: (i32, i32),
    cap: usize,
    turn_penalty: u32,
}

fn corpus_queries(width: i32, height: i32) -> Vec<Query> {
    let pairs = [
        ((1, 1), (width - 2, height - 2)),
        ((width - 2, 1), (1, height - 2)),
        ((2, height / 2), (width - 3, height / 2 + 7)),
        ((width / 2, 2), (width / 2 - 9, height - 3)),
        ((3, 3), (width - 7, height - 11)),
        ((width - 5, height - 5), (6, 9)),
    ];
    let mut queries = Vec::new();
    for (index, (start, goal)) in pairs.into_iter().enumerate() {
        for cap in CAPS {
            queries.push(Query {
                start,
                goal,
                cap,
                turn_penalty: if index % 2 == 0 { 0 } else { 5 },
            });
        }
    }
    queries
}

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn workload_hash(grids: &[Grid], query_sets: &[Vec<Query>]) -> u64 {
    let mut hash = fnv(0xcbf2_9ce4_8422_2325, &CORPUS_VERSION.to_le_bytes());
    for (grid, queries) in grids.iter().zip(query_sets) {
        hash = fnv(hash, &grid.width.to_le_bytes());
        hash = fnv(hash, &grid.height.to_le_bytes());
        for blocked in &grid.blocked {
            hash = fnv(hash, &[*blocked as u8]);
        }
        for query in queries {
            for value in [query.start.0, query.start.1, query.goal.0, query.goal.1] {
                hash = fnv(hash, &value.to_le_bytes());
            }
            hash = fnv(hash, &(query.cap as u64).to_le_bytes());
            hash = fnv(hash, &query.turn_penalty.to_le_bytes());
        }
    }
    hash
}

fn semantic_hash(grids: &[Grid], query_sets: &[Vec<Query>]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    let mut scratch = SearchScratch::default();
    for (grid, queries) in grids.iter().zip(query_sets) {
        for query in queries {
            let (path, expanded, exhausted) =
                find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                    grid,
                    query.start,
                    query.goal,
                    query.cap,
                    query.turn_penalty,
                    &mut scratch,
                );
            hash = fnv(hash, &(expanded as u64).to_le_bytes());
            hash = fnv(hash, &[exhausted as u8]);
            hash = fnv(hash, &(path.len() as u64).to_le_bytes());
            for (tx, ty) in &path {
                hash = fnv(hash, &tx.to_le_bytes());
                hash = fnv(hash, &ty.to_le_bytes());
            }
            for (x, y) in to_world_waypoints(&path) {
                hash = fnv(hash, &x.to_bits().to_le_bytes());
                hash = fnv(hash, &y.to_bits().to_le_bytes());
            }
        }
    }
    hash
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct DijkstraNode {
    cost: u64,
    tx: i32,
    ty: i32,
    dir: u8,
}

impl Ord for DijkstraNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| other.ty.cmp(&self.ty))
            .then_with(|| other.tx.cmp(&self.tx))
            .then_with(|| other.dir.cmp(&self.dir))
    }
}

impl PartialOrd for DijkstraNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn dijkstra_cost(grid: &Grid, query: Query) -> Option<u64> {
    let goal = nearest_passable(grid, query.goal.0, query.goal.1).unwrap_or(query.goal);
    let start = (query.start.0, query.start.1, NO_INCOMING_DIR);
    let mut costs = HashMap::from([(start, 0u64)]);
    let mut open = BinaryHeap::from([DijkstraNode {
        cost: 0,
        tx: query.start.0,
        ty: query.start.1,
        dir: NO_INCOMING_DIR,
    }]);
    while let Some(cur) = open.pop() {
        let key = (cur.tx, cur.ty, cur.dir);
        if costs.get(&key).copied() != Some(cur.cost) {
            continue;
        }
        if (cur.tx, cur.ty) == goal {
            return Some(cur.cost);
        }
        for (dir, &(dx, dy, step)) in NEIGHBORS.iter().enumerate() {
            let next = (cur.tx + dx, cur.ty + dy);
            if !grid.passable(next.0, next.1)
                || (dx != 0
                    && dy != 0
                    && (!grid.passable(cur.tx + dx, cur.ty)
                        || !grid.passable(cur.tx, cur.ty + dy)))
            {
                continue;
            }
            let dir = dir as u8;
            let turn = if query.turn_penalty > 0
                && cur.dir != NO_INCOMING_DIR
                && cur.dir != dir
            {
                query.turn_penalty
            } else {
                0
            };
            let next_dir = if query.turn_penalty > 0 {
                dir
            } else {
                NO_INCOMING_DIR
            };
            let next_key = (next.0, next.1, next_dir);
            let next_cost = cur.cost + u64::from(step + turn);
            if costs.get(&next_key).is_none_or(|old| next_cost < *old) {
                costs.insert(next_key, next_cost);
                open.push(DijkstraNode {
                    cost: next_cost,
                    tx: next.0,
                    ty: next.1,
                    dir: next_dir,
                });
            }
        }
    }
    None
}

fn path_cost(path: &[(i32, i32)], start: (i32, i32), turn_penalty: u32) -> u64 {
    let mut previous = start;
    let mut incoming = NO_INCOMING_DIR;
    let mut total = 0u64;
    for &tile in path {
        let dx = tile.0 - previous.0;
        let dy = tile.1 - previous.1;
        let (dir, step) = NEIGHBORS
            .iter()
            .enumerate()
            .find_map(|(dir, &(nx, ny, cost))| (nx == dx && ny == dy).then_some((dir as u8, cost)))
            .expect("oracle path edge must be a neighbor");
        total += u64::from(step);
        if turn_penalty > 0 && incoming != NO_INCOMING_DIR && incoming != dir {
            total += u64::from(turn_penalty);
        }
        incoming = dir;
        previous = tile;
    }
    total
}

#[test]
fn phase1_corpus_is_deterministic_and_goal_paths_match_dijkstra() {
    let grids = [Grid::open(19, 17), Grid::patterned(31, 29, 7)];
    let query_sets: Vec<_> = grids
        .iter()
        .map(|grid| corpus_queries(grid.width, grid.height))
        .collect();
    let first = semantic_hash(&grids, &query_sets);
    assert_eq!(first, semantic_hash(&grids, &query_sets));

    let mut scratch = SearchScratch::default();
    for (grid, queries) in grids.iter().zip(&query_sets) {
        for &query in queries.iter().filter(|query| query.cap == 32768) {
            let (path, _, exhausted) =
                find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                    grid,
                    query.start,
                    query.goal,
                    query.cap,
                    query.turn_penalty,
                    &mut scratch,
                );
            assert!(!exhausted);
            let goal = nearest_passable(grid, query.goal.0, query.goal.1).unwrap_or(query.goal);
            match dijkstra_cost(grid, query) {
                Some(expected) => {
                    assert_eq!(path.last().copied(), Some(goal));
                    assert_eq!(path_cost(&path, query.start, query.turn_penalty), expected);
                }
                None => assert_ne!(path.last().copied(), Some(goal)),
            }
        }
    }
    eprintln!(
        "phase1 corpus v{CORPUS_VERSION}: workload={:016x} semantic={first:016x}",
        workload_hash(&grids, &query_sets)
    );
}

#[test]
#[ignore = "release-only wall-clock evidence; run with --ignored --nocapture"]
fn phase1_release_path_corpus_benchmark() {
    assert!(!cfg!(debug_assertions), "benchmark must use --release");
    let grids = [Grid::patterned(126, 126, 11), Grid::patterned(196, 196, 23)];
    let query_sets: Vec<_> = grids
        .iter()
        .map(|grid| {
            corpus_queries(grid.width, grid.height)
                .into_iter()
                .filter(|query| query.cap == 32768)
                .collect::<Vec<_>>()
        })
        .collect();
    let workload = workload_hash(&grids, &query_sets);
    let semantic = semantic_hash(&grids, &query_sets);
    let mut scratch = SearchScratch::default();
    let mut requests = 0u64;
    let mut expanded = 0u64;
    let mut waypoint_bits = 0u64;

    for _ in 0..2 {
        for (grid, queries) in grids.iter().zip(&query_sets) {
            for query in queries {
                let _ = find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                    grid,
                    query.start,
                    query.goal,
                    query.cap,
                    query.turn_penalty,
                    &mut scratch,
                );
            }
        }
    }

    let started = Instant::now();
    for _ in 0..20 {
        for (grid, queries) in grids.iter().zip(&query_sets) {
            for query in queries {
                let (path, count, _) =
                    find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                        grid,
                        query.start,
                        query.goal,
                        query.cap,
                        query.turn_penalty,
                        &mut scratch,
                    );
                for (x, y) in to_world_waypoints(&path) {
                    waypoint_bits ^= u64::from(x.to_bits()) << 32 | u64::from(y.to_bits());
                }
                requests += 1;
                expanded += count as u64;
            }
        }
    }
    let elapsed = started.elapsed();
    println!(
        "PATHING_BENCH_JSON={{\"version\":{CORPUS_VERSION},\"workloadHash\":\"{workload:016x}\",\"semanticHash\":\"{semantic:016x}\",\"requests\":{requests},\"expanded\":{expanded},\"elapsedNs\":{},\"requestsPerSecond\":{:.3},\"waypointBits\":\"{waypoint_bits:016x}\"}}",
        elapsed.as_nanos(),
        requests as f64 / elapsed.as_secs_f64()
    );
}
