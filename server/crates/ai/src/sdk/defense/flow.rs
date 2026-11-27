//! Integer-capacity Dinic, used only on cached topology changes.
use std::collections::VecDeque;
#[derive(Clone)]
struct Edge {
    to: usize,
    rev: usize,
    cap: u32,
}
pub(super) struct Flow {
    edges: Vec<Vec<Edge>>,
}
impl Flow {
    pub fn new(n: usize) -> Self {
        Self {
            edges: vec![vec![]; n],
        }
    }
    pub fn add(&mut self, a: usize, b: usize, cap: u32) {
        let ar = self.edges[b].len();
        let br = self.edges[a].len();
        self.edges[a].push(Edge {
            to: b,
            rev: ar,
            cap,
        });
        self.edges[b].push(Edge {
            to: a,
            rev: br,
            cap: 0,
        });
    }
    fn push(
        &mut self,
        source: usize,
        target: usize,
        amount: u32,
        level: &[i32],
        next: &mut [usize],
    ) -> u32 {
        // Explicit path stack: user-authored corridors cannot overflow the simulation stack.
        let mut nodes = vec![source];
        let mut path: Vec<(usize, usize)> = vec![];
        let mut available = vec![amount];
        while let Some(&i) = nodes.last() {
            if i == target {
                let sent = available.last().copied().unwrap_or(0);
                for &(from, edge) in &path {
                    let to = self.edges[from][edge].to;
                    let rev = self.edges[from][edge].rev;
                    self.edges[from][edge].cap -= sent;
                    self.edges[to][rev].cap += sent;
                }
                return sent;
            }
            while next[i] < self.edges[i].len() {
                let e = &self.edges[i][next[i]];
                if e.cap > 0 && level[e.to] == level[i] + 1 {
                    break;
                }
                next[i] += 1;
            }
            if next[i] == self.edges[i].len() {
                nodes.pop();
                available.pop();
                if let Some((parent, _)) = path.pop() {
                    next[parent] += 1;
                }
            } else {
                let e = &self.edges[i][next[i]];
                available.push(available.last().copied().unwrap_or(0).min(e.cap));
                path.push((i, next[i]));
                nodes.push(e.to);
            }
        }
        0
    }
    pub fn cut(&mut self, s: usize, t: usize, limit: u32) -> Option<Vec<bool>> {
        let mut total = 0;
        loop {
            let mut level = vec![-1; self.edges.len()];
            level[s] = 0;
            let mut q = VecDeque::from([s]);
            while let Some(i) = q.pop_front() {
                for e in &self.edges[i] {
                    if e.cap > 0 && level[e.to] < 0 {
                        level[e.to] = level[i] + 1;
                        q.push_back(e.to);
                    }
                }
            }
            if level[t] < 0 {
                break;
            }
            let mut next = vec![0; self.edges.len()];
            loop {
                let v = self.push(s, t, limit, &level, &mut next);
                if v == 0 {
                    break;
                }
                total += v;
                if total >= limit {
                    return None;
                }
            }
        }
        let mut seen = vec![false; self.edges.len()];
        seen[s] = true;
        let mut q = VecDeque::from([s]);
        while let Some(i) = q.pop_front() {
            for e in &self.edges[i] {
                if e.cap > 0 && !seen[e.to] {
                    seen[e.to] = true;
                    q.push_back(e.to);
                }
            }
        }
        Some(seen)
    }
}
