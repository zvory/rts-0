//! Jeff's AI V1.1 keeps a Scout Car looking at the enemy's bases. V1.0's Scout Car went with the
//! push or stood at home: against Pressure AI, which ran five bases, it never saw one of them, so
//! the push had nothing to raid but the natural it marched at. The scout visits the enemy bases
//! seen and the resource fields nearer the enemy start, longest unvisited first, passing over those
//! with a remembered army at them, and backs off `FLEE_BACK_TILES` from enemy fighters that come
//! within `FLEE_TILES`. What it sees feeds the push's memory of enemy bases and armies (`push`).

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

/// Enemy fighters this close (Tanks, which hit from 14 tiles once parked, this close) send the
/// scout back this far, and it holds there this long.
const FLEE_TILES: f32 = 9.0;
const FLEE_TANK_TILES: f32 = 15.0;
const FLEE_BACK_TILES: f32 = 12.0;
const FLEE_HOLD_TICKS: u32 = config::TICK_HZ * 6;
/// The scout has seen a point once this close to it.
const ARRIVED_TILES: f32 = 5.0;
/// Two points this close are the same place.
const SAME_PLACE_TILES: f32 = 6.0;
/// A point with this much remembered enemy strength within `DANGER_TILES` is passed over.
const DANGER_VALUE: f32 = 400.0;
const DANGER_TILES: f32 = 10.0;
/// The scout's move is sent again this often, or as soon as it stands idle.
const REORDER_TICKS: u32 = config::TICK_HZ * 5;
/// `scoutsafe`: a Scout Car sees 15 tiles, a Tank 9 and hits from 14 once parked, so the scout
/// keeps this far from Tanks and `SAFE_OTHER_TILES` from other fighters, backing off until
/// `SAFE_MARGIN_TILES` clear. It looks at a place from `VANTAGE_TILES` off rather than driving in
/// (it died moving in 17 of 33 losses against Pressure AI and Rush AI, Tanks within 14 tiles), and
/// drives there by the way past the least remembered enemy strength, straight or swung out to
/// either side by `DETOUR_SHARE` of the way.
const SAFE_TANK_TILES: f32 = 16.0;
const SAFE_OTHER_TILES: f32 = 11.0;
const SAFE_MARGIN_TILES: f32 = 4.0;
const VANTAGE_TILES: f32 = 11.0;
const DETOUR_SHARE: f32 = 0.35;
/// Remembered enemy strength within this of a path point counts against the path.
const PATH_THREAT_TILES: f32 = 12.0;
const PATH_STRIDE_TILES: f32 = 8.0;

/// V1.1's scout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct ScoutMemory {
    pub(in crate::ai_core::decision) unit: Option<u32>,
    target: Option<(i32, i32)>,
    /// When each place was last seen.
    visited: BTreeMap<(i32, i32), u32>,
    ordered: u32,
    fleeing_until: u32,
}

impl ScoutMemory {
    fn last_visit(&self, at: (f32, f32), ts: f32) -> u32 {
        self.visited
            .iter()
            .filter(|(place, _)| {
                dist2(place.0 as f32, place.1 as f32, at.0, at.1)
                    <= squared(SAME_PLACE_TILES * ts)
            })
            .map(|(_, tick)| *tick)
            .max()
            .unwrap_or(0)
    }
}

fn stored((x, y): (f32, f32)) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

/// Moves V1.1's scout (see the module docs). `held`: units other systems ordered or keep this
/// decision.
pub(in crate::ai_core::decision) fn scout(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    held: &BTreeSet<u32>,
) {
    if !is_jeff_v1_1(profile_id) || super::off("scout") {
        return;
    }
    let Some(analysis) = map_analysis else {
        return;
    };
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let usable = |unit: &&AiEntitySummary| {
        unit.kind == EntityKind::ScoutCar
            && unit.is_complete
            && unit.hp > 0
            && !held.contains(&unit.id)
            && memory.containment.active_scout != Some(unit.id)
            && !memory.third_pressure.is_reserved(unit.id)
            && !memory.trap_breach.is_reserved(unit.id)
            // The raid borrows the Scout Car while it is out.
            && !memory.jeff_v1_1.tank_raid.holds(unit.id)
    };
    let current = memory.jeff_v1_1.scout.unit;
    let scout = observation
        .owned
        .iter()
        .filter(usable)
        .find(|unit| Some(unit.id) == current)
        .or_else(|| observation.owned.iter().filter(usable).min_by_key(|unit| unit.id))
        .cloned();
    memory.jeff_v1_1.scout.unit = scout.as_ref().map(|unit| unit.id);
    let Some(unit) = scout else {
        return;
    };
    let at = (unit.x, unit.y);
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let safe = !super::off("scoutsafe");
    let keep_off = |kind: EntityKind| match (safe, kind == EntityKind::Tank) {
        (true, true) => SAFE_TANK_TILES,
        (true, false) => SAFE_OTHER_TILES,
        (false, true) => FLEE_TANK_TILES,
        (false, false) => FLEE_TILES,
    };
    let threats: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            enemy.kind.is_unit()
                && !matches!(enemy.kind, EntityKind::Worker | EntityKind::Golem)
                && enemy.hp > 0
                && dist2(enemy.x, enemy.y, at.0, at.1) <= squared(keep_off(enemy.kind) * ts)
        })
        .collect();
    let state = &mut memory.jeff_v1_1.scout;
    if !threats.is_empty() {
        let n = threats.len() as f32;
        let threat = (
            threats.iter().map(|enemy| enemy.x).sum::<f32>() / n,
            threats.iter().map(|enemy| enemy.y).sum::<f32>() / n,
        );
        if let Some(target) = state.target.take() {
            state.visited.insert(target, tick);
        }
        let away = normalized_direction(threat, at)
            .or_else(|| normalized_direction(at, home))
            .unwrap_or((0.0, 0.0));
        // `scoutsafe`: back far enough to stand `SAFE_MARGIN_TILES` clear of the nearest threat's
        // keep-off distance.
        let back_tiles = if safe {
            threats
                .iter()
                .map(|enemy| {
                    keep_off(enemy.kind) + SAFE_MARGIN_TILES
                        - dist2(enemy.x, enemy.y, at.0, at.1).sqrt() / ts
                })
                .fold(SAFE_MARGIN_TILES, f32::max)
        } else {
            FLEE_BACK_TILES
        };
        let back = clamp_to_map(
            (at.0 + away.0 * back_tiles * ts, at.1 + away.1 * back_tiles * ts),
            observation.map,
        );
        if state.fleeing_until <= tick {
            if let Some(units) = actions::move_units(actions, [unit.id], back.0, back.1) {
                intents.push(AiIntent::Move { units });
            }
        }
        state.fleeing_until = tick + FLEE_HOLD_TICKS;
        return;
    }
    if tick < state.fleeing_until {
        return;
    }
    let arrived_tiles = if safe { VANTAGE_TILES + 2.0 } else { ARRIVED_TILES };
    if let Some(target) = state.target {
        if dist2(at.0, at.1, target.0 as f32, target.1 as f32) <= squared(arrived_tiles * ts) {
            state.visited.insert(target, tick);
            state.target = None;
        }
    }
    if state.target.is_none() {
        let enemy_main = observation
            .players
            .iter()
            .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
            .map(|player| tile_center(player.start_tile, observation.map.tile_size))
            .min_by(|a, b| {
                dist2(a.0, a.1, home.0, home.1).total_cmp(&dist2(b.0, b.1, home.0, home.1))
            });
        let Some(enemy_main) = enemy_main else {
            return;
        };
        let points = super::push::scout_points(observation, analysis, memory, enemy_main);
        let next = points
            .into_iter()
            .filter(|point| {
                super::push::threat_near(observation, memory, *point, DANGER_TILES) < DANGER_VALUE
            })
            .min_by(|a, b| {
                let state = &memory.jeff_v1_1.scout;
                state
                    .last_visit(*a, ts)
                    .cmp(&state.last_visit(*b, ts))
                    .then(dist2(a.0, a.1, at.0, at.1).total_cmp(&dist2(b.0, b.1, at.0, at.1)))
            });
        let state = &mut memory.jeff_v1_1.scout;
        state.target = next.map(stored);
        state.ordered = 0;
    }
    let state = &memory.jeff_v1_1.scout;
    let Some(target) = state.target else {
        return;
    };
    if tick.saturating_sub(state.ordered) < REORDER_TICKS && unit.state != AiEntityState::Idle {
        return;
    }
    let target = (target.0 as f32, target.1 as f32);
    if !safe {
        if let Some(units) = actions::move_units(actions, [unit.id], target.0, target.1) {
            memory.jeff_v1_1.scout.ordered = tick;
            intents.push(AiIntent::Move { units });
        }
        return;
    }
    // `scoutsafe`: to a vantage point short of the place, by the safest way.
    let toward = normalized_direction(target, at).unwrap_or((0.0, 0.0));
    let vantage = clamp_to_map(
        (target.0 + toward.0 * VANTAGE_TILES * ts, target.1 + toward.1 * VANTAGE_TILES * ts),
        observation.map,
    );
    let vantage = analysis.open_ground_near(at, vantage, 1, 3).unwrap_or(vantage);
    let way = safe_way(observation, analysis, memory, at, vantage);
    for (index, leg) in way.iter().enumerate() {
        if let Some(units) = actions::move_units_with_queue(actions, [unit.id], leg.0, leg.1, index > 0) {
            intents.push(AiIntent::Move { units });
        }
    }
    memory.jeff_v1_1.scout.ordered = tick;
}

/// The way from `from` to `to` past the least remembered enemy strength: straight, or swung out to
/// either side by `DETOUR_SHARE` of the way at its middle, as route points about
/// `PATH_STRIDE_TILES` apart.
fn safe_way(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    memory: &AiDecisionMemory,
    from: (f32, f32),
    to: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let distance = dist2(from.0, from.1, to.0, to.1).sqrt();
    let mut options = vec![analysis.compact_group_route(from, to, 4)];
    if let Some(direction) = normalized_direction(from, to) {
        let across = (-direction.1, direction.0);
        for side in [-1.0, 1.0] {
            let swing = clamp_to_map(
                (
                    (from.0 + to.0) * 0.5 + across.0 * side * distance * DETOUR_SHARE,
                    (from.1 + to.1) * 0.5 + across.1 * side * distance * DETOUR_SHARE,
                ),
                observation.map,
            );
            let mut way = analysis.compact_group_route(from, swing, 4);
            way.extend(analysis.compact_group_route(swing, to, 4));
            options.push(way);
        }
    }
    let way = options
        .into_iter()
        .filter(|way| !way.is_empty())
        .map(|way| {
            let threat = way
                .iter()
                .map(|at| super::push::threat_near(observation, memory, *at, PATH_THREAT_TILES))
                .fold(0.0, f32::max);
            (threat, way)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.len().cmp(&b.1.len())))
        .map(|(_, way)| way)
        .unwrap_or_else(|| vec![to]);
    let mut kept: Vec<(f32, f32)> = Vec::new();
    for at in &way {
        if kept
            .last()
            .is_none_or(|last| dist2(last.0, last.1, at.0, at.1) >= squared(PATH_STRIDE_TILES * ts))
        {
            kept.push(*at);
        }
    }
    if let (Some(last), Some(end)) = (kept.last(), way.last()) {
        if last != end {
            kept.push(*end);
        }
    }
    kept
}
