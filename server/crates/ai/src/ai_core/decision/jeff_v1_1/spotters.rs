//! Jeff's AI V1.1 posts a Rifleman out on the enemy's way to each of its bases (`spotters`). Jeff's
//! responders, the roaming Riflemen (`rovers`) and the push answering a base's call, act only on
//! enemies in sight within 14-16 tiles of a Depot or extractor, so a raid was seen when it was
//! already at the base: on Crossroads the third's guards died at 3:40-4:40 to raiders no one saw
//! coming, and in the bench most failed later bases lost their guards that way.
//!
//! - **Posts**: walking distance from the enemy start is worked out once a game; each base past the
//!   main (and the site being taken) gets a post `SPOT_TILES` back along the enemy's shortest walk
//!   to it, the nearest bases to the enemy first, at most `MAX_SPOTTERS`.
//! - **Spotters**: one free Rifleman per post, from the rovers' pool once Jeff has a Tank (the 6
//!   nearest the main stay home), kept until it dies or another system claims it.
//! - **Hold**: a spotter stands on its post and backs `FLEE_BACK_TILES` toward its base from enemy
//!   fighters within `FLEE_TILES` (Tanks `FLEE_TANK_TILES`), going back once they are gone.
//! - **Warning**: enemies in sight near a post count as a raid on its base, for the rovers and for
//!   the push's base call (`posts`).

use super::*;
use crate::ai_core::decision::geometry::{
    building_center, clamp_to_map, dist2, normalized_direction, squared, tile_center,
};

const SPOT_TILES: u32 = 18;
const MAX_SPOTTERS: usize = 4;
const HOME_RIFLES: usize = 6;
const FLEE_TILES: f32 = 9.0;
const FLEE_TANK_TILES: f32 = 15.0;
const FLEE_BACK_TILES: f32 = 8.0;
const FLEE_HOLD_TICKS: u32 = config::TICK_HZ * 6;
const REORDER_TICKS: u32 = config::TICK_HZ * 4;
const ON_POST_TILES: f32 = 2.0;
/// Bases this close to the main are the main.
const MAIN_TILES: f32 = 8.0;

/// One spotter: its base, its post, when it was last ordered and how long it holds back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Spotter {
    base: (i32, i32),
    post: (i32, i32),
    ordered: u32,
    back_until: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct SpotterMemory {
    /// Walking distance in tiles from the enemy start, row-major; empty until worked out.
    field: Vec<u32>,
    width: u32,
    spotters: BTreeMap<u32, Spotter>,
    /// Every base's post this decision, spotted or not: (base, post).
    posts: Vec<((i32, i32), (i32, i32))>,
}

impl SpotterMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.spotters.contains_key(&id)
    }

    /// Trace token: manned posts / posts.
    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        format!("sp={}/{}", self.spotters.len(), self.posts.len())
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.spotters.keys().copied()
    }

    /// The manned posts (pixels) the rovers treat as their bases (`spalarm` 1 or 2).
    pub(in crate::ai_core::decision) fn posts(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        let on = alarm_level() >= 1;
        self.spotters
            .values()
            .filter(move |_| on)
            .map(|s| (s.post.0 as f32, s.post.1 as f32))
    }

    /// The manned post watching the base at `at`, for the push's base call (`spalarm` 2).
    pub(in crate::ai_core::decision) fn post_for(&self, at: (f32, f32), ts: f32) -> Option<(f32, f32)> {
        if alarm_level() < 2 {
            return None;
        }
        self.spotters
            .values()
            .find(|s| dist2(s.base.0 as f32, s.base.1 as f32, at.0, at.1) <= squared(3.0 * ts))
            .map(|s| (s.post.0 as f32, s.post.1 as f32))
    }
}

/// What a sighting at a post sets off (`spalarm`): 0 nothing (the spotters only see), 1 the
/// rovers, 2 the rovers and the push's base call.
fn alarm_level() -> u32 {
    crate::ai_core::profiles::jeff_v1_1_param("spalarm").map_or(2, |v| v.max(0.0) as u32)
}

pub(in crate::ai_core::decision) fn active(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("spotters")
}

fn stored((x, y): (f32, f32)) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

/// Walking distance (8-way steps) from `from` over passable tiles.
fn walk_field(analysis: &AiMapAnalysis, width: u32, height: u32, from: (u32, u32)) -> Vec<u32> {
    let mut dist = vec![u32::MAX; (width * height) as usize];
    let mut queue = std::collections::VecDeque::new();
    if from.0 < width && from.1 < height {
        dist[(from.1 * width + from.0) as usize] = 0;
        queue.push_back(from);
    }
    while let Some((x, y)) = queue.pop_front() {
        let d = dist[(y * width + x) as usize];
        for dy in -1_i32..=1 {
            for dx in -1_i32..=1 {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if (dx == 0 && dy == 0) || nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                let i = (ny * width + nx) as usize;
                if dist[i] != u32::MAX || !analysis.tile_is_passable(nx, ny) {
                    continue;
                }
                dist[i] = d + 1;
                queue.push_back((nx, ny));
            }
        }
    }
    dist
}

/// The tile `SPOT_TILES` steps from `base` toward the enemy start, down the walking field.
fn post_tile(field: &[u32], width: u32, height: u32, base: (u32, u32)) -> Option<(u32, u32)> {
    let at = |t: (u32, u32)| field.get((t.1 * width + t.0) as usize).copied().unwrap_or(u32::MAX);
    // A Depot's footprint may sit off the walkable grid: start from the nearest walkable tile.
    let mut current = (0..=3_i32)
        .flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy))))
        .map(|(dx, dy)| (base.0 as i32 + dx, base.1 as i32 + dy))
        .filter(|(x, y)| *x >= 0 && *y >= 0 && (*x as u32) < width && (*y as u32) < height)
        .map(|(x, y)| (x as u32, y as u32))
        .find(|t| at(*t) != u32::MAX)?;
    for _ in 0..SPOT_TILES {
        let here = at(current);
        let next = (-1_i32..=1)
            .flat_map(|dy| (-1_i32..=1).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| (current.0 as i32 + dx, current.1 as i32 + dy))
            .filter(|(x, y)| *x >= 0 && *y >= 0 && (*x as u32) < width && (*y as u32) < height)
            .map(|(x, y)| (x as u32, y as u32))
            .filter(|t| at(*t) < here)
            .min_by_key(|t| at(*t));
        match next {
            Some(t) => current = t,
            None => break,
        }
    }
    Some(current)
}

/// Posts and moves V1.1's spotters (see the module docs). `held`: units other systems keep this
/// decision.
pub(in crate::ai_core::decision) fn post(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    held: &BTreeSet<u32>,
) {
    if !active(memory) {
        memory.jeff_v1_1.spotters = SpotterMemory::default();
        return;
    }
    let Some(analysis) = map_analysis else {
        return;
    };
    // Like the rovers, the opening's Riflemen stay V1.0's until the first Tank.
    if !observation
        .owned
        .iter()
        .any(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
    {
        memory.jeff_v1_1.spotters.spotters.clear();
        memory.jeff_v1_1.spotters.posts.clear();
        return;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let (width, height) = (observation.map.width, observation.map.height);
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    if memory.jeff_v1_1.spotters.field.is_empty() {
        let Some(enemy) = observation
            .players
            .iter()
            .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
            .map(|player| player.start_tile)
            .min_by_key(|start| {
                let c = tile_center(*start, observation.map.tile_size);
                dist2(c.0, c.1, main.0, main.1) as i64
            })
        else {
            return;
        };
        memory.jeff_v1_1.spotters.field = walk_field(analysis, width, height, enemy);
        memory.jeff_v1_1.spotters.width = width;
    }
    // Bases to watch: Depots past the main, and the site being taken.
    let mut bases: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .filter(|unit| dist2(unit.x, unit.y, main.0, main.1) > squared(MAIN_TILES * ts))
        .map(|unit| (unit.x, unit.y))
        .collect();
    if let Some(site) = memory.later_bases.site() {
        if let Some(center) = building_center(site, EntityKind::ResourceDepot, observation.map.tile_size) {
            if !bases.iter().any(|b| dist2(b.0, b.1, center.0, center.1) <= squared(4.0 * ts)) {
                bases.push(center);
            }
        }
    }
    let state = &memory.jeff_v1_1.spotters;
    let field_at = |p: (f32, f32)| {
        let (tx, ty) = ((p.0 / ts) as u32, (p.1 / ts) as u32);
        state.field.get((ty * state.width + tx) as usize).copied().unwrap_or(u32::MAX)
    };
    // Nearest the enemy first.
    bases.sort_by_key(|b| field_at(*b));
    let posts: Vec<((i32, i32), (i32, i32))> = bases
        .iter()
        .filter_map(|b| {
            let tile = ((b.0 / ts) as u32, (b.1 / ts) as u32);
            post_tile(&state.field, width, height, tile).map(|t| {
                (stored(*b), stored(tile_center(t, observation.map.tile_size)))
            })
        })
        .take(MAX_SPOTTERS)
        .collect();
    // Free Riflemen: the rovers' pool, past the 6 nearest the main.
    let reserved = memory.squad_reserved();
    let free = |unit: &AiEntitySummary| {
        unit.kind == EntityKind::Rifleman
            && unit.is_complete
            && unit.hp > 0
            && !held.contains(&unit.id)
            && !reserved.contains(&unit.id)
            && !memory.containment.active_riflemen.contains(&unit.id)
            && !memory.expansion_security.riflemen.contains(&unit.id)
            && !memory.later_bases.guards.contains(&unit.id)
            && !memory.rush_reserved(unit.id)
            && memory.route_line.picket() != Some(unit.id)
            && !memory.jeff_v1_1.natural.holds(unit.id)
            && !memory.jeff_v1_1.deny.holds(unit.id)
            && !memory.jeff_v1_1.front_line.holds(unit.id)
            && !memory.jeff_v1_1.base_lines.posted(unit.id)
            && !memory.jeff_v1_1.strike.party.contains(&unit.id)
            && !memory.jeff_v1_1.hunt.hunters.contains_key(&unit.id)
    };
    let mut pool: Vec<&AiEntitySummary> = observation.owned.iter().filter(|u| free(u)).collect();
    pool.sort_by(|a, b| {
        dist2(a.x, a.y, main.0, main.1)
            .total_cmp(&dist2(b.x, b.y, main.0, main.1))
            .then(a.id.cmp(&b.id))
    });
    let pool: Vec<&AiEntitySummary> = pool.into_iter().skip(HOME_RIFLES).collect();
    // Keep spotters whose unit is still free and whose base still has a post.
    let mut previous = std::mem::take(&mut memory.jeff_v1_1.spotters.spotters);
    previous.retain(|id, s| {
        pool.iter().any(|u| u.id == *id) && posts.iter().any(|(base, _)| *base == s.base)
    });
    let mut spotters: BTreeMap<u32, Spotter> = BTreeMap::new();
    for (base, post) in &posts {
        if let Some((id, mut s)) = previous.iter().find(|(_, s)| s.base == *base).map(|(id, s)| (*id, s.clone())) {
            s.post = *post;
            spotters.insert(id, s);
            continue;
        }
        let p = (post.0 as f32, post.1 as f32);
        let pick = pool
            .iter()
            .filter(|u| !spotters.contains_key(&u.id) && !previous.contains_key(&u.id))
            .min_by(|a, b| {
                dist2(a.x, a.y, p.0, p.1)
                    .total_cmp(&dist2(b.x, b.y, p.0, p.1))
                    .then(a.id.cmp(&b.id))
            });
        if let Some(unit) = pick {
            spotters.insert(unit.id, Spotter { base: *base, post: *post, ordered: 0, back_until: 0 });
        }
    }
    // Hold the posts, backing off from enemy fighters.
    let mut sent = Vec::new();
    for (id, s) in spotters.iter_mut() {
        let Some(unit) = observation.owned.iter().find(|u| u.id == *id) else {
            continue;
        };
        let at = (unit.x, unit.y);
        let base = (s.base.0 as f32, s.base.1 as f32);
        let threat = observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && !matches!(enemy.kind, EntityKind::Worker | EntityKind::Golem)
                && dist2(enemy.x, enemy.y, at.0, at.1)
                    <= squared(if enemy.kind == EntityKind::Tank { FLEE_TANK_TILES } else { FLEE_TILES } * ts)
        });
        if threat {
            if s.back_until <= tick {
                let back = normalized_direction(at, base)
                    .map(|d| (at.0 + d.0 * FLEE_BACK_TILES * ts, at.1 + d.1 * FLEE_BACK_TILES * ts))
                    .unwrap_or(base);
                let back = clamp_to_map(back, observation.map);
                if let Some(units) = actions::move_units(actions, [*id], back.0, back.1) {
                    intents.push(AiIntent::Move { units });
                }
                s.ordered = tick;
            }
            s.back_until = tick + FLEE_HOLD_TICKS;
            continue;
        }
        if tick < s.back_until {
            continue;
        }
        let post = (s.post.0 as f32, s.post.1 as f32);
        let on = dist2(at.0, at.1, post.0, post.1) <= squared(ON_POST_TILES * ts);
        if !on && tick.saturating_sub(s.ordered) >= REORDER_TICKS {
            if let Some(units) = actions::move_units(actions, [*id], post.0, post.1) {
                sent.extend(units);
            }
            s.ordered = tick;
        }
    }
    if !sent.is_empty() {
        intents.push(AiIntent::Move { units: sent });
    }
    memory.jeff_v1_1.spotters.spotters = spotters;
    memory.jeff_v1_1.spotters.posts = posts;
}
