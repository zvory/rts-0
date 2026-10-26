//! Where the breach's Tanks stand: posts in sight of a trap on Jeff's side, the concave and the
//! volley posts for a contested trap, and the geometry they share.

use super::*;

/// The route's heading toward the enemy near `point`.
pub(super) fn route_heading(route: &[(f32, f32)], point: (f32, f32)) -> Option<(f32, f32)> {
    let index = (0..route.len()).min_by(|a, b| {
        dist2(route[*a].0, route[*a].1, point.0, point.1).total_cmp(&dist2(
            route[*b].0,
            route[*b].1,
            point.0,
            point.1,
        ))
    })?;
    let from = route[index.saturating_sub(3)];
    let to = route[(index + 3).min(route.len() - 1)];
    super::geometry::normalized_direction(from, to)
}

/// Whether `point` lies nearer the enemy's end of the route than Jeff's.
pub(super) fn on_enemy_half(route: &[(f32, f32)], point: (f32, f32)) -> bool {
    match (route.first(), route.last()) {
        (Some(jeff), Some(enemy)) => {
            dist2(point.0, point.1, enemy.0, enemy.1) < dist2(point.0, point.1, jeff.0, jeff.1)
        }
        _ => true,
    }
}

/// From a trap toward Jeff's side of its line, as a unit vector.
pub(super) fn back_direction(route: &[(f32, f32)], trap: (f32, f32), ts: f32) -> (f32, f32) {
    let toward = jeff_side_point(route, trap, ts);
    super::geometry::normalized_direction(trap, toward).unwrap_or((0.0, 0.0))
}

/// Points around `center` along `back` at `depth` tiles from the trap, fanned across it.
pub(super) fn fan(
    trap: (f32, f32),
    back: (f32, f32),
    depth: f32,
    angles: &[f32],
    ts: f32,
) -> Vec<(f32, f32)> {
    angles
        .iter()
        .map(|angle| {
            let (sin, cos) = angle.to_radians().sin_cos();
            let dir = (back.0 * cos - back.1 * sin, back.0 * sin + back.1 * cos);
            (trap.0 + dir.0 * depth * ts, trap.1 + dir.1 * depth * ts)
        })
        .collect()
}

pub(super) fn usable(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    point: (f32, f32),
    toward_jeff: (f32, f32),
    standing: &[(f32, f32)],
    ts: f32,
) -> bool {
    defense::defensive_position_is_open(observation, map_analysis, point.0, point.1)
        && !crosses_traps(point, toward_jeff, standing, ts)
        && !standing
            .iter()
            .any(|t| dist2(t.0, t.1, point.0, point.1) <= squared(1.5 * ts))
}

/// The concave for a contested trap: an arc `CONCAVE_TILES` back, beyond the reach of Tanks
/// parked behind the trap.
pub(super) fn concave_posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    trap: (f32, f32),
    back: (f32, f32),
    standing: &[(f32, f32)],
    enemy_tanks: &[(f32, f32)],
    from: Option<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let toward_jeff = (
        trap.0 + back.0 * (CONCAVE_TILES + 8.0) * ts,
        trap.1 + back.1 * (CONCAVE_TILES + 8.0) * ts,
    );
    let mut posts: Vec<(f32, f32)> = Vec::new();
    for depth in [CONCAVE_TILES, CONCAVE_TILES + 2.5] {
        let angles: Vec<f32> = (-5..=5).map(|k| k as f32 * 7.0).collect();
        for p in fan(trap, back, depth, &angles, ts) {
            let crowded = posts
                .iter()
                .any(|q| dist2(q.0, q.1, p.0, p.1) < squared(POST_SPACING_TILES * ts));
            let clear = clearance_of(p, p, enemy_tanks, ts) >= CONCAVE_CLEARANCE_TILES
                && from.is_none_or(|from| {
                    clearance_of(from, p, enemy_tanks, ts) >= TRAVEL_CLEARANCE_TILES
                });
            if !crowded && clear && usable(observation, map_analysis, p, toward_jeff, standing, ts)
            {
                posts.push(p);
            }
        }
    }
    posts.truncate(MAX_TANKS);
    posts
}

/// The least distance, in tiles, from the way from `a` to `b` to any of `enemies` (40 if none).
pub(super) fn clearance_of(a: (f32, f32), b: (f32, f32), enemies: &[(f32, f32)], ts: f32) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    enemies
        .iter()
        .map(|e| {
            let along = if len2 > 0.0 {
                (((e.0 - a.0) * dx + (e.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            dist2(a.0 + dx * along, a.1 + dy * along, e.0, e.1).sqrt() / ts
        })
        .fold(40.0f32, f32::min)
}

/// The centre of the given Tanks.
pub(super) fn group_center(tanks: &[&AiEntitySummary]) -> Option<(f32, f32)> {
    let n = tanks.len();
    (n > 0).then(|| {
        (
            tanks.iter().map(|tank| tank.x).sum::<f32>() / n as f32,
            tanks.iter().map(|tank| tank.y).sum::<f32>() / n as f32,
        )
    })
}

/// Firing posts for a volley: as far from the trap as it can be shot, on Jeff's side, the ones
/// farthest from the enemy Tanks seen lately first.
pub(super) fn fire_posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    trap: (f32, f32),
    back: (f32, f32),
    standing: &[(f32, f32)],
    (enemy_tanks, min_clearance): (&[(f32, f32)], f32),
    ts: f32,
) -> Vec<(f32, f32)> {
    let toward_jeff = (
        trap.0 + back.0 * (CONCAVE_TILES + 8.0) * ts,
        trap.1 + back.1 * (CONCAVE_TILES + 8.0) * ts,
    );
    let clearance = |p: &(f32, f32)| {
        enemy_tanks
            .iter()
            .map(|e| dist2(e.0, e.1, p.0, p.1).sqrt() / ts)
            .fold(40.0f32, f32::min)
    };
    let angles: Vec<f32> = (-9..=9).map(|k| k as f32 * 10.0).collect();
    let mut candidates: Vec<((f32, f32), f32)> = [FIRE_TILES, FIRE_TILES - 1.0]
        .into_iter()
        .flat_map(|depth| fan(trap, back, depth, &angles, ts))
        .filter(|p| usable(observation, map_analysis, *p, toward_jeff, standing, ts))
        .map(|p| (p, clearance(&p)))
        .filter(|(_, clear)| *clear >= min_clearance)
        .collect();
    // The most clearance first, up to `FIRE_CLEARANCE_TILES`; the sort is stable, so ties keep
    // the fan order (straight back first).
    let capped = |clear: f32| clear.min(FIRE_CLEARANCE_TILES);
    candidates.sort_by(|a, b| capped(b.1).total_cmp(&capped(a.1)));
    let mut posts: Vec<(f32, f32)> = Vec::new();
    for (p, _) in candidates {
        let crowded = posts
            .iter()
            .any(|q| dist2(q.0, q.1, p.0, p.1) < squared(POST_SPACING_TILES * ts));
        if !crowded {
            posts.push(p);
        }
        if posts.len() >= VOLLEY_TANKS {
            break;
        }
    }
    posts
}

/// Posts around `trap`: within sight of it, on Jeff's side of its line, open ground, outside the
/// reach of every enemy Tank seen lately and of parked groups, spaced apart.
#[allow(clippy::too_many_arguments)]
pub(super) fn safe_posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    route: &[(f32, f32)],
    enemy_tanks: &[(f32, f32)],
    zones: &[super::tank_micro::Zone],
    standing: &[(f32, f32)],
    trap: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let Some(heading) = route_heading(route, trap) else {
        return Vec::new();
    };
    let along = |p: &(f32, f32)| (p.0 - trap.0) * heading.0 + (p.1 - trap.1) * heading.1;
    let jeff_side = jeff_side_point(route, trap, ts);
    let mut posts: Vec<(f32, f32)> = Vec::new();
    for radius in POST_RINGS {
        for step in 0..18 {
            let angle = step as f32 * std::f32::consts::TAU / 18.0;
            let p = (
                trap.0 + angle.cos() * radius * ts,
                trap.1 + angle.sin() * radius * ts,
            );
            if crosses_traps(p, jeff_side, standing, ts)
                || standing
                    .iter()
                    .any(|t| dist2(t.0, t.1, p.0, p.1) <= squared(1.5 * ts))
            {
                continue;
            }
            let exposed = enemy_tanks
                .iter()
                .any(|e| dist2(e.0, e.1, p.0, p.1) <= squared(ENEMY_TANK_CLEARANCE_TILES * ts))
                || zones.iter().any(|zone| {
                    dist2(zone.center.0, zone.center.1, p.0, p.1) <= squared(zone.radius)
                });
            let crowded = posts
                .iter()
                .any(|q| dist2(q.0, q.1, p.0, p.1) < squared(POST_SPACING_TILES * ts));
            if exposed
                || crowded
                || !defense::defensive_position_is_open(observation, map_analysis, p.0, p.1)
            {
                continue;
            }
            posts.push(p);
        }
    }
    // The posts deepest on Jeff's side first.
    posts.sort_by(|a, b| along(a).total_cmp(&along(b)));
    posts.truncate(MAX_POSTS);
    posts
}

/// Where the breach works: the centre of its posts, else of its Tanks.
pub(super) fn work_site(breach: &TrapBreach, tanks: &[&AiEntitySummary]) -> Option<(f32, f32)> {
    let points: Vec<(f32, f32)> = if breach.posts.is_empty() {
        tanks.iter().map(|tank| (tank.x, tank.y)).collect()
    } else {
        breach.posts.values().map(|p| world(*p)).collect()
    };
    let n = points.len();
    (n > 0).then(|| {
        let (sx, sy) = points
            .iter()
            .fold((0.0, 0.0), |acc, p| (acc.0 + p.0, acc.1 + p.1));
        (sx / n as f32, sy / n as f32)
    })
}

/// A point on Jeff's side of the line at `trap`: back along the route from where it passes nearest.
pub(super) fn jeff_side_point(route: &[(f32, f32)], trap: (f32, f32), ts: f32) -> (f32, f32) {
    let index = (0..route.len())
        .min_by(|a, b| {
            dist2(route[*a].0, route[*a].1, trap.0, trap.1).total_cmp(&dist2(
                route[*b].0,
                route[*b].1,
                trap.0,
                trap.1,
            ))
        })
        .unwrap_or(0);
    // Route points are a tile apart; step back toward Jeff.
    let back = index.saturating_sub((JEFF_SIDE_BACK_TILES + ROUTE_TILES) as usize);
    let _ = ts;
    route[back]
}

/// Whether the straight way from `a` to `b` passes a standing trap closer than
/// `TRAP_CLEARANCE_TILES`.
pub(super) fn crosses_traps(
    a: (f32, f32),
    b: (f32, f32),
    standing: &[(f32, f32)],
    ts: f32,
) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    standing.iter().any(|t| {
        let along = if len2 > 0.0 {
            (((t.0 - a.0) * dx + (t.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (px, py) = (a.0 + dx * along, a.1 + dy * along);
        dist2(px, py, t.0, t.1) <= squared(TRAP_CLEARANCE_TILES * ts)
    })
}

/// Gives the posts to the nearest Tanks.
pub(super) fn assign_posts(
    breach: &mut TrapBreach,
    tanks: &[&AiEntitySummary],
    posts: &[(f32, f32)],
) {
    breach.posts.clear();
    let mut free: Vec<&AiEntitySummary> = tanks.to_vec();
    for post in posts {
        let Some(index) = (0..free.len()).min_by(|a, b| {
            dist2(free[*a].x, free[*a].y, post.0, post.1)
                .total_cmp(&dist2(free[*b].x, free[*b].y, post.0, post.1))
                .then_with(|| free[*a].id.cmp(&free[*b].id))
        }) else {
            return;
        };
        let tank = free.remove(index);
        breach.posts.insert(tank.id, stored(*post));
    }
}
