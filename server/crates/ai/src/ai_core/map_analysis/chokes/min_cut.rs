use super::*;

pub(super) fn tile_in_bounds(tile: AiTile, bounds: AiTileBounds) -> bool {
    tile.x >= bounds.min.x
        && tile.x <= bounds.max.x
        && tile.y >= bounds.min.y
        && tile.y <= bounds.max.y
}

pub(super) fn local_min_vertex_cut(
    width: u32,
    height: u32,
    passable: &[bool],
    clearance: &[u16],
    bounds: AiTileBounds,
    source_tiles: &[AiTile],
    sink_tiles: &[AiTile],
) -> Vec<AiTile> {
    let source_set: BTreeSet<_> = source_tiles
        .iter()
        .copied()
        .filter(|tile| tile_in_bounds(*tile, bounds))
        .collect();
    let sink_set: BTreeSet<_> = sink_tiles
        .iter()
        .copied()
        .filter(|tile| tile_in_bounds(*tile, bounds))
        .collect();
    if source_set.is_empty() || sink_set.is_empty() {
        return Vec::new();
    }

    let mut passable_tiles = Vec::new();
    let mut local_by_tile = BTreeMap::new();
    for y in bounds.min.y..=bounds.max.y {
        for x in bounds.min.x..=bounds.max.x {
            let Some(idx) = tile_index(width, height, x, y) else {
                continue;
            };
            if passable.get(idx).copied() != Some(true) {
                continue;
            }
            let tile = AiTile::new(x, y);
            local_by_tile.insert(tile, passable_tiles.len());
            passable_tiles.push(tile);
        }
    }
    if passable_tiles.is_empty() {
        return Vec::new();
    }

    let source = passable_tiles.len().saturating_mul(2);
    let sink = source.saturating_add(1);
    let mut flow = Dinic::new(sink.saturating_add(1));
    for (local_idx, &tile) in passable_tiles.iter().enumerate() {
        let in_node = local_idx.saturating_mul(2);
        let out_node = in_node.saturating_add(1);
        let tile_clearance = tile_index(width, height, tile.x, tile.y)
            .and_then(|idx| clearance.get(idx).copied())
            .unwrap_or(0);
        let protected_tile = source_set.contains(&tile)
            || sink_set.contains(&tile)
            || tile_clearance >= GAMEPLAY_MIN_CUT_PROTECTED_CLEARANCE_TILES;
        flow.add_edge(
            in_node,
            out_node,
            if protected_tile { GAMEPLAY_FLOW_INF } else { 1 },
        );
        if source_set.contains(&tile) {
            flow.add_edge(source, in_node, GAMEPLAY_FLOW_INF);
        }
        if sink_set.contains(&tile) {
            flow.add_edge(out_node, sink, GAMEPLAY_FLOW_INF);
        }
        for neighbor in passable_neighbors(width, height, passable, tile) {
            if !tile_in_bounds(neighbor, bounds) {
                continue;
            }
            let Some(&neighbor_idx) = local_by_tile.get(&neighbor) else {
                continue;
            };
            flow.add_edge(out_node, neighbor_idx.saturating_mul(2), GAMEPLAY_FLOW_INF);
        }
    }

    let max_flow = flow.max_flow(source, sink);
    if max_flow <= 0 || max_flow >= GAMEPLAY_FLOW_INF {
        return Vec::new();
    }
    let reachable = flow.reachable_from(source);
    passable_tiles
        .into_iter()
        .enumerate()
        .filter_map(|(local_idx, tile)| {
            let in_node = local_idx.saturating_mul(2);
            let out_node = in_node.saturating_add(1);
            (reachable.get(in_node).copied() == Some(true)
                && reachable.get(out_node).copied() != Some(true))
            .then_some(tile)
        })
        .collect()
}

pub(super) fn linearity_score(tiles: &[AiTile]) -> i32 {
    if tiles.len() < 2 {
        return 0;
    }
    let len = tiles.len() as f64;
    let mean_x = tiles.iter().map(|tile| f64::from(tile.x)).sum::<f64>() / len;
    let mean_y = tiles.iter().map(|tile| f64::from(tile.y)).sum::<f64>() / len;
    let mut xx = 0.0_f64;
    let mut yy = 0.0_f64;
    let mut xy = 0.0_f64;
    for tile in tiles {
        let dx = f64::from(tile.x) - mean_x;
        let dy = f64::from(tile.y) - mean_y;
        xx += dx * dx;
        yy += dy * dy;
        xy += dx * dy;
    }
    let trace = xx + yy;
    let determinant = xx * yy - xy * xy;
    let root = (trace * trace - 4.0 * determinant).max(0.0).sqrt();
    let major = (trace + root) / 2.0;
    let minor = (trace - root) / 2.0;
    if major <= 0.0 {
        return 0;
    }
    (100.0 * (1.0 - minor / major)).round() as i32
}

#[derive(Clone, Debug)]
struct FlowEdge {
    to: usize,
    rev: usize,
    cap: i32,
}

#[derive(Clone, Debug)]
struct Dinic {
    graph: Vec<Vec<FlowEdge>>,
}

impl Dinic {
    fn new(size: usize) -> Self {
        Self {
            graph: vec![Vec::new(); size],
        }
    }

    fn add_edge(&mut self, from: usize, to: usize, cap: i32) {
        if from >= self.graph.len() || to >= self.graph.len() {
            return;
        }
        let fwd = FlowEdge {
            to,
            rev: self.graph[to].len(),
            cap,
        };
        let rev = FlowEdge {
            to: from,
            rev: self.graph[from].len(),
            cap: 0,
        };
        self.graph[from].push(fwd);
        self.graph[to].push(rev);
    }

    fn max_flow(&mut self, source: usize, sink: usize) -> i32 {
        let mut total = 0_i32;
        loop {
            let levels = self.levels(source);
            if levels.get(sink).copied().unwrap_or(-1) < 0 {
                return total;
            }
            let mut iters = vec![0_usize; self.graph.len()];
            loop {
                let pushed = self.dfs(source, sink, GAMEPLAY_FLOW_INF, &levels, &mut iters);
                if pushed <= 0 {
                    break;
                }
                total = total.saturating_add(pushed);
            }
        }
    }

    fn levels(&self, source: usize) -> Vec<i32> {
        let mut levels = vec![-1_i32; self.graph.len()];
        if source >= self.graph.len() {
            return levels;
        }
        let mut queue = VecDeque::from([source]);
        levels[source] = 0;
        while let Some(node) = queue.pop_front() {
            for edge in &self.graph[node] {
                if edge.cap <= 0 || levels.get(edge.to).copied().unwrap_or(0) >= 0 {
                    continue;
                }
                levels[edge.to] = levels[node].saturating_add(1);
                queue.push_back(edge.to);
            }
        }
        levels
    }

    fn dfs(
        &mut self,
        node: usize,
        sink: usize,
        pushed: i32,
        levels: &[i32],
        iters: &mut [usize],
    ) -> i32 {
        if node == sink {
            return pushed;
        }
        while iters[node] < self.graph[node].len() {
            let edge_idx = iters[node];
            let edge = &self.graph[node][edge_idx];
            let to = edge.to;
            let cap = edge.cap;
            let rev = edge.rev;
            if cap > 0 && levels[node] < levels[to] {
                let next = self.dfs(to, sink, pushed.min(cap), levels, iters);
                if next > 0 {
                    self.graph[node][edge_idx].cap -= next;
                    self.graph[to][rev].cap += next;
                    return next;
                }
            }
            iters[node] = iters[node].saturating_add(1);
        }
        0
    }

    fn reachable_from(&self, source: usize) -> Vec<bool> {
        let mut seen = vec![false; self.graph.len()];
        if source >= self.graph.len() {
            return seen;
        }
        let mut queue = VecDeque::from([source]);
        seen[source] = true;
        while let Some(node) = queue.pop_front() {
            for edge in &self.graph[node] {
                if edge.cap <= 0 || seen.get(edge.to).copied() == Some(true) {
                    continue;
                }
                seen[edge.to] = true;
                queue.push_back(edge.to);
            }
        }
        seen
    }
}
