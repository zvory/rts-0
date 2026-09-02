use std::collections::BTreeMap;

use super::*;

/// Find the least-cost path to any goal using one bounded search. Goals are ordered so equal-cost
/// arrivals retain caller-defined deterministic preference. The returned goal index refers to the
/// original goal slice; `None` means the bounded best-effort path did not reach a goal.
pub(in crate::game) fn find_path_to_any_with_budget_and_turn_cost_with_diagnostics_and_scratch<
    P: Passability,
>(
    pass: &P,
    start: (i32, i32),
    goals: &[(i32, i32)],
    max_expanded: usize,
    turn_penalty: u32,
    scratch: &mut SearchScratch,
) -> (Vec<(i32, i32)>, usize, bool, Option<usize>) {
    scratch.begin(pass.dimensions(), start, turn_penalty > 0);
    let (sx, sy) = start;
    let mut resolved_goals = BTreeMap::new();
    for (index, &(gx, gy)) in goals.iter().enumerate() {
        if !pass.passable(gx, gy) {
            continue;
        }
        resolved_goals
            .entry((gx, gy))
            .and_modify(|current: &mut usize| *current = (*current).min(index))
            .or_insert(index);
    }
    if resolved_goals.is_empty() {
        scratch.finish();
        return (Vec::new(), 0, false, None);
    }
    if let Some(&goal_index) = resolved_goals.get(&start) {
        scratch.finish();
        return (Vec::new(), 0, false, Some(goal_index));
    }
    let min_x = resolved_goals.keys().map(|goal| goal.0).min().unwrap_or(sx);
    let max_x = resolved_goals.keys().map(|goal| goal.0).max().unwrap_or(sx);
    let min_y = resolved_goals.keys().map(|goal| goal.1).min().unwrap_or(sy);
    let max_y = resolved_goals.keys().map(|goal| goal.1).max().unwrap_or(sy);
    let start_key = (sx, sy, NO_INCOMING_DIR);
    let (heuristic_cardinal, heuristic_diagonal) = pass.heuristic_step_costs();
    let distance_to_goal_bounds = |tx: i32, ty: i32, cardinal, diagonal| {
        let nearest_x = tx.clamp(min_x, max_x);
        let nearest_y = ty.clamp(min_y, max_y);
        heuristic_with_costs(tx, ty, nearest_x, nearest_y, cardinal, diagonal)
    };
    let production_heuristic = |tx, ty| {
        distance_to_goal_bounds(tx, ty, heuristic_cardinal, heuristic_diagonal)
    };
    scratch.open.push(Node {
        f: production_heuristic(sx, sy),
        g: 0,
        tx: sx,
        ty: sy,
        dir: NO_INCOMING_DIR,
    });
    if let Some(start_index) = scratch.index(start_key) {
        scratch.set_start(start_index);
    } else {
        scratch.finish();
        return (Vec::new(), 0, false, None);
    }
    let mut best_key = start_key;
    let mut best_h = distance_to_goal_bounds(sx, sy, 10, 14);
    let mut expanded = 0usize;
    let mut budget_exhausted = false;
    let mut reached: Option<(u32, usize, SearchKey)> = None;
    while let Some(cur) = scratch.open.pop() {
        let cur_key = (cur.tx, cur.ty, cur.dir);
        if reached.is_some_and(|(cost, _, _)| cur.f > cost) {
            break;
        }
        if scratch.get_g(cur_key).is_some_and(|best_g| cur.g > best_g) {
            continue;
        }
        if let Some(&goal_index) = resolved_goals.get(&(cur.tx, cur.ty)) {
            let candidate = (cur.g, goal_index, cur_key);
            if reached.is_none_or(|best| (candidate.0, candidate.1) < (best.0, best.1)) {
                reached = Some(candidate);
            }
            continue;
        }
        expanded += 1;
        if expanded > max_expanded {
            budget_exhausted = true;
            break;
        }
        for (dir, &(dx, dy, cost)) in NEIGHBORS.iter().enumerate() {
            let nx = cur.tx + dx;
            let ny = cur.ty + dy;
            let Some(edge_cost) = pass.edge_cost(cur.tx, cur.ty, dx, dy, cost) else {
                continue;
            };
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
            let tentative = cur.g.saturating_add(edge_cost).saturating_add(turn_cost);
            if scratch.get_g(next_key).is_none_or(|existing| tentative < existing) {
                scratch.set(next_key, tentative, cur_key);
                let fallback_h = distance_to_goal_bounds(nx, ny, 10, 14);
                if fallback_h < best_h {
                    best_h = fallback_h;
                    best_key = next_key;
                }
                scratch.open.push(Node {
                    f: tentative.saturating_add(production_heuristic(nx, ny)),
                    g: tentative,
                    tx: nx,
                    ty: ny,
                    dir: next_dir,
                });
            }
        }
    }
    if let Some((_, goal_index, goal_key)) = reached {
        let path = scratch.reconstruct(goal_key);
        scratch.finish();
        return (path, expanded, budget_exhausted, Some(goal_index));
    }
    let path = if (best_key.0, best_key.1) != (sx, sy) {
        scratch.reconstruct(best_key)
    } else {
        Vec::new()
    };
    scratch.finish();
    (path, expanded, budget_exhausted, None)
}
