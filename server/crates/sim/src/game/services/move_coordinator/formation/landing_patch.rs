use std::collections::{BTreeSet, VecDeque};

use super::reachability::tile_passable_for_kind;
use super::{FormationUnit, Map, Occupancy};

const LANDING_PATCH_PADDING_TILES: i32 = 6;

const NEIGHBORS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

pub(super) struct LandingPatch {
    width: u32,
    allowed: Vec<bool>,
}

impl LandingPatch {
    pub(super) fn contains(&self, tile: (u32, u32)) -> bool {
        tile.0 < self.width
            && self
                .allowed
                .get((tile.1.saturating_mul(self.width) + tile.0) as usize)
                .copied()
                .unwrap_or(false)
    }
}

/// Choose one locally connected destination patch for the complete selection. Global terrain
/// connectivity is intentionally insufficient here: opposite river banks may connect only after a
/// long detour around an endpoint and should not split one compact formation.
pub(super) fn cohesive_landing_patch<F>(
    map: &Map,
    occ: &Occupancy<'_>,
    units: &[FormationUnit],
    click: (f32, f32),
    desired_points: &[(f32, f32)],
    is_goal_reachable: &mut F,
) -> LandingPatch
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    let mut allowed = vec![false; map.width.saturating_mul(map.height) as usize];
    if units.is_empty() || map.width == 0 || map.height == 0 {
        return LandingPatch {
            width: map.width,
            allowed,
        };
    }

    let kinds = units.iter().map(|unit| unit.kind).collect::<BTreeSet<_>>();
    let click_tile = map.tile_of(click.0, click.1);
    let mut min_tx = click_tile.0 as i32;
    let mut max_tx = min_tx;
    let mut min_ty = click_tile.1 as i32;
    let mut max_ty = min_ty;
    for point in desired_points {
        let tile = map.tile_of(point.0, point.1);
        min_tx = min_tx.min(tile.0 as i32);
        max_tx = max_tx.max(tile.0 as i32);
        min_ty = min_ty.min(tile.1 as i32);
        max_ty = max_ty.max(tile.1 as i32);
    }
    min_tx = (min_tx - LANDING_PATCH_PADDING_TILES).max(0);
    min_ty = (min_ty - LANDING_PATCH_PADDING_TILES).max(0);
    max_tx = (max_tx + LANDING_PATCH_PADDING_TILES).min(map.width as i32 - 1);
    max_ty = (max_ty + LANDING_PATCH_PADDING_TILES).min(map.height as i32 - 1);

    let window_width = (max_tx - min_tx + 1) as u32;
    let window_height = (max_ty - min_ty + 1) as u32;
    let mut passable = vec![false; window_width.saturating_mul(window_height) as usize];
    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            let index = window_index(tx, ty, min_tx, min_ty, window_width);
            passable[index] = kinds
                .iter()
                .all(|kind| tile_passable_for_kind(map, occ, *kind, tx, ty));
        }
    }

    let mut visited = vec![false; passable.len()];
    let mut components = Vec::new();
    let mut queue = VecDeque::new();
    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            let index = window_index(tx, ty, min_tx, min_ty, window_width);
            if !passable[index] || visited[index] {
                continue;
            }
            visited[index] = true;
            queue.push_back((tx, ty));
            let mut component = Vec::new();
            while let Some((cx, cy)) = queue.pop_front() {
                component.push((cx as u32, cy as u32));
                for (dx, dy) in NEIGHBORS {
                    let nx = cx + dx;
                    let ny = cy + dy;
                    if nx < min_tx || nx > max_tx || ny < min_ty || ny > max_ty {
                        continue;
                    }
                    let next = window_index(nx, ny, min_tx, min_ty, window_width);
                    if visited[next]
                        || !passable[next]
                        || !step_allowed(&passable, window_width, min_tx, min_ty, cx, cy, dx, dy)
                    {
                        continue;
                    }
                    visited[next] = true;
                    queue.push_back((nx, ny));
                }
            }
            components.push(component);
        }
    }

    let centroid = units.iter().fold((0.0, 0.0), |sum, unit| {
        (sum.0 + unit.pos.0, sum.1 + unit.pos.1)
    });
    let centroid = (
        centroid.0 / units.len() as f32,
        centroid.1 / units.len() as f32,
    );
    let selected = components
        .iter()
        .find(|component| component.contains(&click_tile))
        .or_else(|| {
            components
                .iter()
                .map(|component| {
                    let reachable_units = units
                        .iter()
                        .filter(|unit| {
                            component
                                .iter()
                                .copied()
                                .any(|tile| is_goal_reachable(unit, tile))
                        })
                        .count();
                    (
                        component_score(
                            map,
                            component,
                            click,
                            centroid,
                            units.len(),
                            reachable_units,
                        ),
                        component,
                    )
                })
                .min_by_key(|(score, _)| *score)
                .map(|(_, component)| component)
        });

    if let Some(component) = selected {
        for &(tx, ty) in component {
            allowed[map.index(tx, ty)] = true;
        }
    }
    LandingPatch {
        width: map.width,
        allowed,
    }
}

fn component_score(
    map: &Map,
    component: &[(u32, u32)],
    click: (f32, f32),
    centroid: (f32, f32),
    unit_count: usize,
    reachable_units: usize,
) -> (
    std::cmp::Reverse<usize>,
    bool,
    u32,
    u32,
    std::cmp::Reverse<usize>,
    (u32, u32),
) {
    let mut click_distance = u32::MAX;
    let mut centroid_distance = u32::MAX;
    let mut first = (u32::MAX, u32::MAX);
    for &tile in component {
        let center = map.tile_center(tile.0, tile.1);
        click_distance = click_distance.min(distance_score(center, click));
        centroid_distance = centroid_distance.min(distance_score(center, centroid));
        first = first.min(tile);
    }
    (
        std::cmp::Reverse(reachable_units),
        component.len() < unit_count,
        click_distance,
        centroid_distance,
        std::cmp::Reverse(component.len()),
        first,
    )
}

fn distance_score(a: (f32, f32), b: (f32, f32)) -> u32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx.mul_add(dx, dy * dy).max(0.0).min(u32::MAX as f32)) as u32
}

fn window_index(tx: i32, ty: i32, min_tx: i32, min_ty: i32, width: u32) -> usize {
    ((ty - min_ty) as u32 * width + (tx - min_tx) as u32) as usize
}

#[allow(clippy::too_many_arguments)]
fn step_allowed(
    passable: &[bool],
    width: u32,
    min_tx: i32,
    min_ty: i32,
    tx: i32,
    ty: i32,
    dx: i32,
    dy: i32,
) -> bool {
    if dx == 0 || dy == 0 {
        return true;
    }
    passable[window_index(tx + dx, ty, min_tx, min_ty, width)]
        && passable[window_index(tx, ty + dy, min_tx, min_ty, width)]
}
