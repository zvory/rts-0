use super::*;
const INF: u32 = 100_000_000;
#[derive(Clone)]
pub(super) struct Cut {
    pub components: Vec<Vec<usize>>,
    pub demand: usize,
    pub cost: usize,
}
#[derive(Clone)]
struct Group {
    bases: Vec<AiBaseSiteId>,
    cut: Option<Cut>,
}
impl Group {
    fn demand(&self) -> usize {
        self.cut.as_ref().map_or(3, |c| c.demand)
    }
    fn cost(&self) -> usize {
        self.cut.as_ref().map_or(43, |c| c.cost)
    }
}
fn cut(planner: &mut DefensePlanner, frame: &AiFrame, ids: &[AiBaseSiteId]) -> Option<Cut> {
    if let Some(c) = planner.cuts.get(ids) {
        return c.clone();
    }
    let g = planner.grid.as_ref()?;
    for id in ids {
        if !planner.fields.contains_key(id) {
            let site = frame.base_sites().iter().find(|b| b.id == *id)?;
            let anchor = g.near(site.position)?;
            planner.fields.insert(*id, g.field(&[anchor], false));
        }
    }
    let mut distance = vec![FAR; g.walk.len()];
    for id in ids {
        if let Some(f) = planner.fields.get(id) {
            for (d, v) in distance.iter_mut().zip(f) {
                *d = (*d).min(*v);
            }
        }
    }
    let mut protected = distance.iter().map(|d| *d <= 60).collect::<Vec<_>>();
    let resources = ids
        .iter()
        .filter_map(|id| frame.base_sites().iter().find(|b| b.id == *id))
        .flat_map(|s| s.resource_ids.iter())
        .copied()
        .collect::<BTreeSet<_>>();
    for r in frame
        .resources()
        .iter()
        .filter(|r| resources.contains(&r.id))
    {
        if let Some(a) = g.index(r.position) {
            let (x, y) = g.tile(a);
            for dx in -2i32..=2 {
                for dy in -2i32..=2 {
                    if dx * dx + dy * dy > 4 {
                        continue;
                    }
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && ny >= 0 && (nx as usize) < g.w {
                        let j = ny as usize * g.w + nx as usize;
                        if g.walk.get(j).copied().unwrap_or(false) {
                            protected[j] = true;
                        }
                    }
                }
            }
        }
    }
    let area = (0..g.walk.len())
        .filter(|i| g.walk[*i] && (distance[*i] <= 320 || protected[*i]))
        .collect::<Vec<_>>();
    // Bound worst-case graph memory/work on custom maps. Fallback is an honest open garrison.
    if area.is_empty() || area.len() > 20_000 {
        planner.cuts.insert(ids.to_vec(), None);
        return None;
    }
    let mut indices = vec![usize::MAX; g.walk.len()];
    for (j, &i) in area.iter().enumerate() {
        indices[i] = j;
    }
    let source = area.len() * 2;
    let sink = source + 1;
    let mut f = flow::Flow::new(sink + 1);
    for (j, &i) in area.iter().enumerate() {
        let outer = g.neighbors(i).any(|(z, _)| indices[z] == usize::MAX);
        let cap = if protected[i] || outer {
            INF
        } else {
            100 + ((distance[i] as f32 / 10. - 12.).abs() * 0.5).round() as u32
        };
        f.add(j * 2, j * 2 + 1, cap);
        if protected[i] {
            f.add(j * 2 + 1, sink, INF);
        }
        if outer {
            f.add(source, j * 2, INF);
        }
        for (z, _) in g.neighbors(i) {
            if indices[z] != usize::MAX {
                f.add(j * 2 + 1, indices[z] * 2, INF);
            }
        }
    }
    planner.geometry_builds += 1;
    let result = f.cut(source, sink, INF).and_then(|seen| {
        let tiles = area
            .iter()
            .enumerate()
            .filter(|(j, _)| seen[j * 2] && !seen[j * 2 + 1])
            .map(|(_, i)| *i)
            .collect::<Vec<_>>();
        if tiles.is_empty() {
            return None;
        }
        let components = g.components(&tiles);
        Some(Cut {
            demand: components.iter().map(|c| 2.max(c.len().div_ceil(6))).sum(),
            cost: tiles.len() + 8 * components.len(),
            components,
        })
    });
    planner.cuts.insert(ids.to_vec(), result.clone());
    result
}
fn partitions(ids: &[AiBaseSiteId]) -> Vec<Vec<Vec<AiBaseSiteId>>> {
    if ids.is_empty() {
        return vec![vec![]];
    }
    let mut out = vec![];
    for p in partitions(&ids[1..]) {
        let mut q = p.clone();
        q.insert(0, vec![ids[0]]);
        out.push(q);
        for i in 0..p.len() {
            let mut q = p.clone();
            q[i].insert(0, ids[0]);
            out.push(q);
        }
    }
    out
}
fn alternatives(
    planner: &mut DefensePlanner,
    frame: &AiFrame,
    blocks: &[Vec<AiBaseSiteId>],
    index: usize,
    current: &mut Vec<Group>,
    out: &mut Vec<Vec<Group>>,
) {
    if index == blocks.len() {
        out.push(current.clone());
        return;
    }
    let ids = &blocks[index];
    if let Some(c) = cut(planner, frame, ids) {
        current.push(Group {
            bases: ids.clone(),
            cut: Some(c),
        });
        alternatives(planner, frame, blocks, index + 1, current, out);
        current.pop();
    }
    if ids.len() == 1 {
        current.push(Group {
            bases: ids.clone(),
            cut: None,
        });
        alternatives(planner, frame, blocks, index + 1, current, out);
        current.pop();
    }
}
pub(super) fn build(planner: &mut DefensePlanner, frame: &AiFrame) -> Vec<SectorGeometry> {
    let ids = planner.owned.clone();
    if ids.is_empty() {
        return vec![];
    }
    let mut groups = if ids.len() <= 4 {
        let mut options = vec![];
        for blocks in partitions(&ids) {
            alternatives(planner, frame, &blocks, 0, &mut vec![], &mut options);
        }
        let feasible = options
            .iter()
            .any(|p| p.iter().map(Group::demand).sum::<usize>() <= planner.geometry_capacity);
        options
            .into_iter()
            .filter(|p| {
                !feasible || p.iter().map(Group::demand).sum::<usize>() <= planner.geometry_capacity
            })
            .min_by_key(|p| {
                (
                    p.iter().map(Group::cost).sum::<usize>(),
                    usize::MAX
                        - p.iter()
                            .filter(|g| g.cut.is_some())
                            .map(|g| g.bases.len())
                            .sum::<usize>(),
                    p.len(),
                )
            })
            .unwrap_or_default()
    } else {
        // Bounded agglomeration for arbitrary owned-site counts. The closest 32 candidate merges
        // are evaluated, each exact cut cached. Never enumerate exponential subsets above four.
        let mut groups = ids
            .iter()
            .map(|id| {
                let c = cut(planner, frame, &[*id]);
                Group {
                    bases: vec![*id],
                    cut: c.filter(|c| c.cost < 43),
                }
            })
            .collect::<Vec<_>>();
        let mut pairs = vec![];
        for i in 0..groups.len() {
            for j in i + 1..groups.len() {
                let a = frame.base_sites().iter().find(|b| b.id == ids[i]);
                let b = frame.base_sites().iter().find(|b| b.id == ids[j]);
                if let (Some(a), Some(b)) = (a, b) {
                    pairs.push((dist2(a.position, b.position), i, j));
                }
            }
        }
        pairs.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| (a.1, a.2).cmp(&(b.1, b.2)))
        });
        for (_, i, j) in pairs.into_iter().take(32) {
            if groups[i].bases.is_empty() || groups[j].bases.is_empty() {
                continue;
            }
            let mut merged = groups[i].bases.clone();
            merged.extend_from_slice(&groups[j].bases);
            merged.sort_unstable();
            if let Some(c) = cut(planner, frame, &merged) {
                if c.cost < groups[i].cost() + groups[j].cost() {
                    groups[i] = Group {
                        bases: merged,
                        cut: Some(c),
                    };
                    groups[j].bases.clear();
                }
            }
        }
        groups.retain(|g| !g.bases.is_empty());
        groups
    };
    groups.sort_by_key(|g| g.bases[0]);
    let Some(grid) = planner.grid.as_ref() else {
        return vec![];
    };
    let mut out = vec![];
    for group in groups {
        if let Some(c) = group.cut {
            for component in c.components {
                for points in split(grid, &component) {
                    let center = medoid(grid, &points);
                    out.push(SectorGeometry {
                        mode: DefenseMode::Cut,
                        bases: group.bases.clone(),
                        points,
                        center,
                    });
                }
            }
        } else {
            let Some(base) = frame.base_sites().iter().find(|b| b.id == group.bases[0]) else {
                continue;
            };
            let Some(anchor) = grid.near(base.position) else {
                continue;
            };
            let resources = frame
                .resources()
                .iter()
                .filter(|r| base.resource_ids.contains(&r.id))
                .collect::<Vec<_>>();
            let center = (0..grid.walk.len())
                .filter(|i| grid.entrench[*i])
                .filter(|i| {
                    let d = dist2(grid.point(*i), base.position) / (grid.size * grid.size);
                    (25. ..=81.).contains(&d) && planner.enemy[*i] < FAR
                })
                .max_by(|a, b| {
                    let quality = |i: usize| {
                        let p = grid.point(i);
                        let visible = resources
                            .iter()
                            .filter(|r| dist2(p, r.position) <= 49. * grid.size * grid.size)
                            .count() as f32;
                        let forward =
                            (planner.enemy[anchor] as f32 - planner.enemy[i] as f32) / 120.;
                        visible + 0.25 * forward
                            - 0.5 * (dist2(p, base.position).det_sqrt() / grid.size - 6.).abs()
                    };
                    quality(*a).total_cmp(&quality(*b)).then_with(|| b.cmp(a))
                })
                .unwrap_or(anchor);
            out.push(SectorGeometry {
                mode: DefenseMode::Garrison,
                bases: group.bases,
                points: vec![center],
                center,
            });
        }
    }
    out
}
fn medoid(g: &Grid, p: &[usize]) -> usize {
    let sum = p.iter().fold((0., 0.), |a, i| {
        let q = g.point(*i);
        (a.0 + q.0, a.1 + q.1)
    });
    let mean = (sum.0 / p.len() as f32, sum.1 / p.len() as f32);
    p.iter()
        .copied()
        .min_by(|a, b| {
            dist2(g.point(*a), mean)
                .total_cmp(&dist2(g.point(*b), mean))
                .then_with(|| a.cmp(b))
        })
        .unwrap_or(0)
}
fn split(g: &Grid, component: &[usize]) -> Vec<Vec<usize>> {
    // Prototype B's farthest-seed cut-graph Voronoi catchments, recursively split to twelve
    // tiles. These are frontage sectors, not twelve-unit groups; staffing remains arbitrary.
    if component.len() <= 12 {
        return vec![component.to_vec()];
    }
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let tiles: BTreeSet<_> = component.iter().copied().collect();
    let field = |seed: usize| {
        let mut d = BTreeMap::from([(seed, 0u32)]);
        let mut q = BinaryHeap::from([Reverse((0u32, seed))]);
        while let Some(Reverse((cost, i))) = q.pop() {
            if d.get(&i) != Some(&cost) {
                continue;
            }
            let (x, y) = g.tile(i);
            for dx in -1i32..=1 {
                for dy in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= g.w {
                        continue;
                    }
                    let j = ny as usize * g.w + nx as usize;
                    if !tiles.contains(&j) {
                        continue;
                    }
                    let nc = cost + if dx != 0 && dy != 0 { 14 } else { 10 };
                    if nc < d.get(&j).copied().unwrap_or(FAR) {
                        d.insert(j, nc);
                        q.push(Reverse((nc, j)));
                    }
                }
            }
        }
        d
    };
    let Some(seed) = component.iter().copied().min_by_key(|i| g.tile(*i)) else {
        return vec![];
    };
    let mut fields = vec![field(seed)];
    for _ in 1..component.len().div_ceil(12) {
        if let Some(seed) = component.iter().copied().max_by(|a, b| {
            let nearest = |i| {
                fields
                    .iter()
                    .filter_map(|d| d.get(&i))
                    .copied()
                    .min()
                    .unwrap_or(FAR)
            };
            nearest(*a)
                .cmp(&nearest(*b))
                .then_with(|| g.tile(*b).cmp(&g.tile(*a)))
        }) {
            fields.push(field(seed));
        }
    }
    let mut clusters = vec![vec![]; fields.len()];
    for &i in component {
        let best = (0..fields.len())
            .min_by_key(|j| (fields[*j].get(&i).copied().unwrap_or(FAR), *j))
            .unwrap_or(0);
        clusters[best].push(i);
    }
    clusters
        .into_iter()
        .filter(|c| !c.is_empty())
        .flat_map(|c| {
            if c.len() > 12 && c.len() < component.len() {
                split(g, &c)
            } else {
                vec![c]
            }
        })
        .collect()
}
