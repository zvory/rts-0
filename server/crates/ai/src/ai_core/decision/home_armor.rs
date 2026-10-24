//! Where Jeff's home Tanks stand, and keeping them there.
//!
//! The home Tanks used to hold a line eight tiles from the main's steel toward the enemy. That line
//! was placed from the steel still in the ground, so once the main mined out (12,500-16,800 ticks)
//! it was gone and the home Tanks got no more orders; after a push left, the Tanks kept home got
//! none either. And it never covered the natural, 22-26 tiles from the HQ on The River and
//! Classic, where AI 2.1 shelled it from beyond the home Tanks' range while they stood idle.
//!
//! The home post keeps the main's line after the steel is gone and, once the natural stands close
//! enough, sits halfway between the two so a response to either is short. Resting Tanks with
//! nothing else to do gather there.

use super::geometry::{clamp_to_map, dist2, normalized_direction, squared, tile_center};
use super::*;

/// The natural is covered from the home post when it lies at most this far from the main's line.
/// Farther (Crossroads' natural lies 45 tiles from the HQ) a post between them would cover neither.
const NATURAL_COVER_MAX_TILES: f32 = 24.0;
/// Tanks this close to the home post are home.
pub(super) const HOME_POST_RADIUS_TILES: f32 = 8.0;
/// A post on blocked ground moves to the nearest open tile this close.
const HOME_POST_SNAP_TILES: i32 = 4;
/// The live Jeff keeps its post this many tiles clear of forest. Tree trunks stop Tanks, so a post
/// in a forest sends them to the nearest spot they can reach: on Crossroads' east start that was
/// across the water wall from the base, where 15-20 Tanks sat for the rest of the game.
const HOME_POST_FOREST_MARGIN_TILES: i32 = 1;
/// The home Tank waits this far behind the centre of the home line.
const HOME_TANK_BEHIND_TILES: f32 = 2.0;
/// Tanks within this distance of the HQ are in the main.
const MAIN_AREA_TILES: f32 = 16.0;
/// A Tank this close to another of Jeff's bases, or to a base being taken, is covering it.
const BASE_COVER_TILES: f32 = 12.0;
/// Moving less than this between decisions counts as standing still.
const STANDING_STILL_TILES: f32 = 0.25;
/// A Tank sent to the post is not sent again for this long.
const HOME_ORDER_COOLDOWN_TICKS: u32 = config::TICK_HZ * 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HomePost {
    center: (i32, i32),
    /// Unit direction the home line faces, in thousandths.
    facing: (i32, i32),
    /// The post is the main's line on steel still in the ground, not moved toward the natural.
    pub(super) on_main_line: bool,
}

impl HomePost {
    pub(super) fn center(self) -> (f32, f32) {
        (self.center.0 as f32, self.center.1 as f32)
    }

    pub(super) fn facing(self) -> (f32, f32) {
        (self.facing.0 as f32 / 1000.0, self.facing.1 as f32 / 1000.0)
    }

    pub(super) fn contains(self, observation: &AiObservation, point: (f32, f32)) -> bool {
        let center = self.center();
        dist2(point.0, point.1, center.0, center.1)
            <= squared(HOME_POST_RADIUS_TILES * observation.map.tile_size as f32)
    }

    /// Where the home Tank waits: just behind the line, toward the base.
    pub(super) fn home_tank_point(self, observation: &AiObservation) -> (f32, f32) {
        let center = self.center();
        let facing = self.facing();
        let back = HOME_TANK_BEHIND_TILES * observation.map.tile_size as f32;
        clamp_to_map(
            (center.0 - facing.0 * back, center.1 - facing.1 * back),
            observation.map,
        )
    }
}

/// Work out this decision's home post. `distance_tiles` is how far the main's line stands from the
/// main's steel. With `avoid_forest` the post keeps clear of forest Tanks cannot enter.
pub(super) fn update_home_post(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    enemy_base: Option<EnemyBaseFact>,
    distance_tiles: f32,
    avoid_forest: bool,
) {
    let forest = map_analysis.filter(|_| avoid_forest);
    let live_steel = defense::main_steel_cluster_center(observation);
    if let Some(steel) = live_steel {
        memory.home_steel_anchor = Some((steel.0.round() as i32, steel.1.round() as i32));
    }
    let steel = live_steel.or_else(|| memory.home_steel_anchor.map(|(x, y)| (x as f32, y as f32)));
    memory.home_post = steel.zip(enemy_base).and_then(|(steel, enemy_base)| {
        let line = defense::main_steel_line_center(observation, steel, enemy_base, distance_tiles)?;
        let facing = normalized_direction(steel, (enemy_base.x, enemy_base.y))?;
        let (center, on_main_line) =
            match natural_cover_point(observation, map_analysis, forest, line) {
                Some(point) => (point, false),
                None => (
                    forest
                        .and_then(|_| open_point_near(observation, map_analysis, forest, line))
                        .unwrap_or(line),
                    live_steel.is_some(),
                ),
            };
        Some(HomePost {
            center: (center.0.round() as i32, center.1.round() as i32),
            facing: (
                (facing.0 * 1000.0).round() as i32,
                (facing.1 * 1000.0).round() as i32,
            ),
            on_main_line,
        })
    });
}

/// Halfway between the main's line and the natural, when the natural stands close enough for one
/// post to cover both.
fn natural_cover_point(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    forest: Option<&AiMapAnalysis>,
    line: (f32, f32),
) -> Option<(f32, f32)> {
    let natural = natural_depot(observation)?;
    let ts = observation.map.tile_size as f32;
    if dist2(line.0, line.1, natural.0, natural.1) > squared(NATURAL_COVER_MAX_TILES * ts) {
        return None;
    }
    let midpoint = ((line.0 + natural.0) * 0.5, (line.1 + natural.1) * 0.5);
    open_point_near(observation, map_analysis, forest, midpoint)
}

/// Whether forest lies within `HOME_POST_FOREST_MARGIN_TILES` of `point`.
fn near_forest(observation: &AiObservation, analysis: &AiMapAnalysis, point: (f32, f32)) -> bool {
    let ts = observation.map.tile_size as f32;
    let (tx, ty) = ((point.0 / ts).floor() as i32, (point.1 / ts).floor() as i32);
    let margin = HOME_POST_FOREST_MARGIN_TILES;
    (-margin..=margin).any(|dy| {
        (-margin..=margin).any(|dx| {
            let (x, y) = (tx + dx, ty + dy);
            x >= 0 && y >= 0 && analysis.tile_is_concealment(x as u32, y as u32)
        })
    })
}

/// Jeff's nearest finished Depot other than the HQ.
fn natural_depot(observation: &AiObservation) -> Option<(f32, f32)> {
    let hq = tile_center(observation.own_start_tile, observation.map.tile_size);
    let not_hq2 = squared(3.0 * observation.map.tile_size as f32);
    observation
        .owned
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::ResourceDepot
                && entity.is_complete
                && entity.hp > 0
                && dist2(entity.x, entity.y, hq.0, hq.1) > not_hq2
        })
        .min_by(|left, right| {
            dist2(left.x, left.y, hq.0, hq.1)
                .total_cmp(&dist2(right.x, right.y, hq.0, hq.1))
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|depot| (depot.x, depot.y))
}

/// `point` if units can stand there, else the nearest open tile within `HOME_POST_SNAP_TILES`.
/// With `forest`, open also means clear of its forest.
fn open_point_near(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    forest: Option<&AiMapAnalysis>,
    point: (f32, f32),
) -> Option<(f32, f32)> {
    let open = |x: f32, y: f32| {
        defense::defensive_position_is_open(observation, map_analysis, x, y)
            && forest.is_none_or(|analysis| !near_forest(observation, analysis, (x, y)))
    };
    if open(point.0, point.1) {
        return Some(point);
    }
    let ts = observation.map.tile_size as f32;
    for radius in 1..=HOME_POST_SNAP_TILES {
        let mut best: Option<((f32, f32), f32)> = None;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let candidate = clamp_to_map(
                    (point.0 + dx as f32 * ts, point.1 + dy as f32 * ts),
                    observation.map,
                );
                if !open(candidate.0, candidate.1) {
                    continue;
                }
                let distance = dist2(candidate.0, candidate.1, point.0, point.1);
                if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                    best = Some((candidate, distance));
                }
            }
        }
        if let Some((candidate, _)) = best {
            return Some(candidate);
        }
    }
    None
}

/// Free Tanks that stood still since the last decision, got no other order this decision and are
/// neither at the post nor covering another base head to the post: those resting elsewhere in the
/// main, and those left somewhere out on the map. Tanks at the natural or a later base stay there.
/// Returns the Tanks sent.
pub(super) fn gather_resting_tanks(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    excluded: &BTreeSet<u32>,
    ordered: &BTreeSet<u32>,
    covered_sites: &[(f32, f32)],
) -> Vec<u32> {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .collect();
    let previous = std::mem::take(&mut memory.home_tank_positions);
    memory.home_tank_positions = tanks
        .iter()
        .map(|unit| (unit.id, (unit.x.round() as i32, unit.y.round() as i32)))
        .collect();
    memory
        .home_tank_orders
        .retain(|_, sent| tick.saturating_sub(*sent) < HOME_ORDER_COOLDOWN_TICKS);
    let Some(post) = memory.home_post else {
        return Vec::new();
    };
    let hq = tile_center(observation.own_start_tile, observation.map.tile_size);
    let other_bases: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::ResourceDepot && entity.hp > 0)
        .map(|depot| (depot.x, depot.y))
        .filter(|depot| dist2(depot.0, depot.1, hq.0, hq.1) > squared(3.0 * ts))
        .chain(covered_sites.iter().copied())
        .collect();
    let still2 = squared(STANDING_STILL_TILES * ts);
    let sent: Vec<u32> = tanks
        .iter()
        .filter(|unit| {
            unit.free_for_combat
                && !excluded.contains(&unit.id)
                && !ordered.contains(&unit.id)
                && !memory.containment.active_tanks.contains(&unit.id)
                && !memory.containment.opening_tanks.contains(&unit.id)
                && !memory.home_tank_orders.contains_key(&unit.id)
        })
        .filter(|unit| {
            previous
                .get(&unit.id)
                .is_some_and(|(x, y)| dist2(unit.x, unit.y, *x as f32, *y as f32) <= still2)
        })
        .filter(|unit| !post.contains(observation, (unit.x, unit.y)))
        .filter(|unit| {
            let in_main = dist2(unit.x, unit.y, hq.0, hq.1) <= squared(MAIN_AREA_TILES * ts);
            let covering_base = other_bases.iter().any(|base| {
                dist2(unit.x, unit.y, base.0, base.1) <= squared(BASE_COVER_TILES * ts)
            });
            in_main || !covering_base
        })
        .map(|unit| unit.id)
        .collect();
    let destination = post.center();
    let Some(sent) = actions::attack_move_units(actions, sent, destination.0, destination.1) else {
        return Vec::new();
    };
    for id in &sent {
        memory.home_tank_orders.insert(*id, tick);
    }
    sent
}

#[cfg(test)]
#[path = "home_armor_tests.rs"]
mod tests;
