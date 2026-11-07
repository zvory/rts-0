//! Jeff's AI V1.1 stations the Tanks it is not using at its bases (`stations`). Every Tank waiting
//! for the push and every resting Tank used to stand on the one home post: halfway to the natural
//! when that lay near the main, on the main's line when it did not. On Crossroads, whose natural
//! lies 39 tiles from the main, Pressure AI's first push of 2-4 Tanks and 2-7 Panzerfausts took the
//! natural at 5:45-7:40 in all nine games lost or drawn behind (sitefix 120), with at most one Jeff
//! Tank there and 3-9 on the home post; on Schone Tage the third, 60 tiles out, fell at 10:25-11:05
//! to 3-5 Tanks while 23-24 Jeff Tanks stood elsewhere. Of 34 outer bases lost in those 21 games,
//! 23 had at most one Jeff Tank within 14 tiles.
//!
//! - **Stations**: the home post is the main's station, and every finished Depot past it gets one
//!   behind the line in front of its mines on the enemy's way in (`base_line::base_lines`). A base
//!   whose station lies within `COVER_TILES` of another's shares that one: a Tank drives that far
//!   in about ten seconds.
//! - **Weight**: a station is worth its bases' live extractors, `MAIN_WEIGHT` more at the main for
//!   its production, the ones past the main times `1 + tiles from the home post / REMOTE_TILES`: a
//!   far base waits longest for help.
//! - **Threat**: the heaviest enemy force seen within `THREAT_TILES` of a base in the last
//!   `THREAT_TICKS` keeps `THREAT_SHARE` of it, in whole Tanks, at its station before the rest is
//!   split.
//! - **Groups**: at most one station per `TANK_GROUP` Tanks holds any, so none holds a pair for a
//!   push to pick off: the stations under threat first, then the heaviest. Short of Tanks for
//!   every station, the two nearest share a station at the point halfway between them, when that
//!   point is open, on the ground way from the main, and within `COVER_TILES` of both.
//! - **Stable**: a Tank keeps its station while that station is short of its share, and its place
//!   in it while it stands there; the others go to the station furthest short.
//! - **Return**: a Tank another system takes (home defense, the push, a raid, the natural's Tank)
//!   keeps its station while away. Once it is free again and no enemy stands within
//!   `FIGHTING_TILES` of it, it is placed with the rest, back on its station unless the split
//!   moved: the base just attacked holds on to the Tanks its threat calls for, and the rest go back.

use super::*;
use crate::ai_core::decision::geometry::{clamp_to_map, dist2, squared, tile_center};
use rts_rules::math::GameplayMath as _;

/// A base whose station lies this close to another's shares it.
const COVER_TILES: f32 = 20.0;
/// The main's station is worth this many extractors more, for its production.
const MAIN_WEIGHT: f32 = 4.0;
/// A station past the main counts `1 + tiles from the home post / REMOTE_TILES` times.
const REMOTE_TILES: f32 = 20.0;
/// Enemy fighters this close to a base's Depot or extractors threaten it...
const THREAT_TILES: f32 = 16.0;
/// ... and the heaviest such force is remembered this long...
const THREAT_TICKS: u32 = config::TICK_HZ * 120;
/// ... and keeps this share of itself, in Tanks, at the base's station.
const THREAT_SHARE: f32 = 0.75;
const TANK_VALUE: f32 = 640.0;
/// At most one station per this many Tanks holds any.
const TANK_GROUP: usize = 3;
/// Extractors this close to a Depot, and nearer it than any other, are its base's.
const MINE_TILES: f32 = 12.0;
/// A Depot this close to Jeff's start is the main.
const MAIN_TILES: f32 = 8.0;
/// A Tank with a visible enemy unit this close is fighting: it gets no station order, and one
/// coming back from elsewhere is not placed yet.
const FIGHTING_TILES: f32 = 14.0;
/// A Tank this close to its place holds it.
const ON_POST_TILES: f32 = 1.5;
/// Two places this close are one.
const SAME_POST_TILES: f32 = 1.0;
/// Tanks at a station stand this far apart, `RANK_WIDTH` to a rank, ranks this far apart.
const TANK_SPACING_TILES: f32 = 2.5;
const RANK_WIDTH: usize = 5;
/// Places worked out beyond the Tanks to place, for places that clear onto one another.
const SPARE_POINTS: usize = 4;
/// A Tank driving to its place is sent again at most this often, or sooner once it stands idle,
/// but never sooner than `MIN_REORDER_TICKS`.
const REORDER_TICKS: u32 = config::TICK_HZ * 10;
const MIN_REORDER_TICKS: u32 = config::TICK_HZ * 2;
/// Ground distances along the way from the main may run this much longer than `COVER_TILES` for
/// the halfway point of two stations.
const WAY_SLACK: f32 = 1.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Post {
    /// The Depot the station is named for.
    anchor: u32,
    at: (i32, i32),
    ordered: u32,
}

/// Where V1.1's stationed Tanks stand, which are away, and the threats each base has seen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct StationMemory {
    posts: BTreeMap<u32, Post>,
    /// Stationed Tanks another system holds, and since when.
    away: BTreeMap<u32, u32>,
    /// Per base Depot: the heaviest enemy force seen near it lately (value) and when.
    threats: BTreeMap<u32, (u32, u32)>,
    summary: String,
    /// `riflestations`: where each free Rifleman stands this decision.
    rifles: BTreeMap<u32, RiflePlace>,
    rifle_summary: String,
}

/// `riflestations`: where a free Rifleman stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::ai_core::decision) enum RiflePost {
    /// The home pocket at the main (`jeff::stage_home_riflemen`).
    Main,
    /// The natural's line (`natural::support`).
    Natural,
    /// The line in front of the mines of the base with this Depot.
    Base(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RiflePlace {
    post: RiflePost,
    at: (i32, i32),
    ordered: u32,
}

impl StationMemory {
    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        format!("v1_1_stations {}{}", self.summary, self.rifle_summary)
    }

    /// `riflestations`: the post the Rifle split gave this Rifleman.
    pub(in crate::ai_core::decision) fn rifle_post(&self, id: u32) -> Option<RiflePost> {
        self.rifles.get(&id).map(|place| place.post)
    }

    /// `riflestations`: whether this Rifleman holds a base past the natural.
    pub(in crate::ai_core::decision) fn posted_out(&self, id: u32) -> bool {
        matches!(self.rifle_post(id), Some(RiflePost::Base(_)))
    }

    /// Tanks on a station now, not away with another system (`breakout` lends them out).
    pub(in crate::ai_core::decision) fn stationed(&self) -> impl Iterator<Item = u32> + '_ {
        self.posts
            .keys()
            .copied()
            .filter(|id| !self.away.contains_key(id))
    }
}

/// Whether Jeff's AI V1.1 stations its Tanks.
pub(in crate::ai_core::decision) fn active(profile_id: &str) -> bool {
    is_jeff_v1_1(profile_id) && !super::off("stations")
}

/// `active` for the profile this memory belongs to.
pub(in crate::ai_core::decision) fn on(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(active)
}

struct Station {
    anchor: u32,
    bases: Vec<u32>,
    center: (f32, f32),
    /// Toward where the enemy comes.
    facing: (f32, f32),
    /// The base line its places come from, or none for the home post and shared halfway points.
    line: Option<usize>,
    remote: f32,
    weight: f32,
    floor: usize,
    /// Whether it covers the main.
    main: bool,
}

/// Each finished Depot, its extractors, and whether it is the main.
struct Base {
    depot: u32,
    at: (f32, f32),
    extractors: Vec<(f32, f32)>,
    main: bool,
}

fn bases(observation: &AiObservation) -> Vec<Base> {
    let ts = observation.map.tile_size as f32;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let depots: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .collect();
    depots
        .iter()
        .map(|depot| Base {
            depot: depot.id,
            at: (depot.x, depot.y),
            extractors: observation
                .owned
                .iter()
                .filter(|unit| {
                    matches!(unit.kind, EntityKind::SteelMine | EntityKind::PumpJack)
                        && unit.hp > 0
                        && dist2(unit.x, unit.y, depot.x, depot.y) <= squared(MINE_TILES * ts)
                        && depots.iter().all(|other| {
                            dist2(unit.x, unit.y, depot.x, depot.y)
                                <= dist2(unit.x, unit.y, other.x, other.y)
                        })
                })
                .map(|unit| (unit.x, unit.y))
                .collect(),
            main: dist2(depot.x, depot.y, home.0, home.1) <= squared(MAIN_TILES * ts),
        })
        .collect()
}

/// The value of the visible enemy fighters within `THREAT_TILES` of the base.
fn threat_at(observation: &AiObservation, base: &Base) -> u32 {
    let reach = squared(THREAT_TILES * observation.map.tile_size as f32);
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && !matches!(enemy.kind, EntityKind::Worker | EntityKind::Golem)
        })
        .filter(|enemy| {
            std::iter::once(&base.at)
                .chain(base.extractors.iter())
                .any(|site| dist2(enemy.x, enemy.y, site.0, site.1) <= reach)
        })
        .map(|enemy| super::push::unit_value(enemy.kind))
        .sum()
}

/// Whether forest lies within a tile of `point`: Tanks cannot drive through tree trunks.
fn near_forest(observation: &AiObservation, analysis: &AiMapAnalysis, point: (f32, f32)) -> bool {
    let ts = observation.map.tile_size as f32;
    let (tx, ty) = ((point.0 / ts).floor() as i32, (point.1 / ts).floor() as i32);
    (-1..=1).any(|dy| {
        (-1..=1).any(|dx| {
            let (x, y) = (tx + dx, ty + dy);
            x >= 0 && y >= 0 && analysis.tile_is_concealment(x as u32, y as u32)
        })
    })
}

/// Each wanted point cleared to open ground away from forest; one that clears onto an earlier
/// point is dropped.
fn clear_points(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    wanted: Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let mut points: Vec<(f32, f32)> = Vec::new();
    for point in wanted {
        let point = clamp_to_map(point, observation.map);
        let Some(cleared) =
            defense::clear_mobile_defensive_position(observation, map_analysis, point)
        else {
            continue;
        };
        if map_analysis.is_some_and(|analysis| near_forest(observation, analysis, cleared)) {
            continue;
        }
        if points.iter().all(|other| {
            dist2(cleared.0, cleared.1, other.0, other.1) > squared(SAME_POST_TILES * ts)
        }) {
            points.push(cleared);
        }
    }
    points
}

/// `count` places around `center`, across `facing` from the centre out, ranks behind it.
fn spread(center: (f32, f32), facing: (f32, f32), count: usize, ts: f32) -> Vec<(f32, f32)> {
    let across = (-facing.1, facing.0);
    let spacing = TANK_SPACING_TILES * ts;
    (0..count)
        .map(|index| {
            let (rank, slot) = (index / RANK_WIDTH, index % RANK_WIDTH);
            let step = slot.div_ceil(2) as f32 * spacing;
            let side = if slot % 2 == 1 { step } else { -step };
            let back = rank as f32 * spacing;
            (
                center.0 + across.0 * side - facing.0 * back,
                center.1 + across.1 * side - facing.1 * back,
            )
        })
        .collect()
}

/// Each Tank's place among `points`: one standing on a place keeps it, and the other places, in
/// order, take the nearest Tank left.
fn seat(units: &[&AiEntitySummary], points: &[(f32, f32)], ts: f32) -> Vec<(u32, (f32, f32))> {
    let mut seated = Vec::new();
    let mut taken = BTreeSet::new();
    let mut open = Vec::new();
    for point in points {
        match units.iter().find(|unit| {
            !taken.contains(&unit.id)
                && dist2(unit.x, unit.y, point.0, point.1) <= squared(ON_POST_TILES * ts)
        }) {
            Some(unit) => {
                taken.insert(unit.id);
                seated.push((unit.id, *point));
            }
            None => open.push(*point),
        }
    }
    for point in open {
        let Some(unit) = units
            .iter()
            .filter(|unit| !taken.contains(&unit.id))
            .min_by(|a, b| {
                dist2(a.x, a.y, point.0, point.1)
                    .total_cmp(&dist2(b.x, b.y, point.0, point.1))
                    .then(a.id.cmp(&b.id))
            })
        else {
            break;
        };
        taken.insert(unit.id);
        seated.push((unit.id, point));
    }
    seated
}

/// The ground distance from Jeff's start to `point`, in tiles.
fn ground(observation: &AiObservation, analysis: &AiMapAnalysis, point: (f32, f32)) -> Option<f32> {
    let ts = observation.map.tile_size as f32;
    if point.0 < 0.0 || point.1 < 0.0 {
        return None;
    }
    analysis.ground_distance_from_start(
        observation.player_id,
        ((point.0 / ts) as u32, (point.1 / ts) as u32),
    )
}

/// The open point halfway between stations `a` and `b` that can cover both, when there is one: a
/// point within `COVER_TILES` of each and, with the map's ground distances, no further from the
/// main by ground than the way from the nearer station to the further allows.
fn halfway(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    a: &Station,
    b: &Station,
) -> Option<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let cover = COVER_TILES * ts;
    // `wayhalf`: halfway between the main and a base past it along the ground way there.
    if let Some(analysis) = map_analysis.filter(|_| (a.main || b.main) && !super::off("wayhalf")) {
        let (main, other) = if a.main { (a, b) } else { (b, a) };
        return way_halfway(
            observation,
            map_analysis,
            analysis,
            main.center,
            other.center,
        );
    }
    if dist2(a.center.0, a.center.1, b.center.0, b.center.1) > squared(2.0 * cover) {
        return None;
    }
    let mid = (
        (a.center.0 + b.center.0) * 0.5,
        (a.center.1 + b.center.1) * 0.5,
    );
    let point = *clear_points(observation, map_analysis, vec![mid]).first()?;
    let reach = squared(cover * 1.1);
    if dist2(point.0, point.1, a.center.0, a.center.1) > reach
        || dist2(point.0, point.1, b.center.0, b.center.1) > reach
    {
        return None;
    }
    if let Some(analysis) = map_analysis {
        let (near, far) = {
            let ga = ground(observation, analysis, a.center)?;
            let gb = ground(observation, analysis, b.center)?;
            if ga <= gb {
                (ga, gb)
            } else {
                (gb, ga)
            }
        };
        let at = ground(observation, analysis, point)?;
        let slack = COVER_TILES * WAY_SLACK;
        if at > near + slack || far > at + slack {
            return None;
        }
    }
    Some(point)
}

/// `wayhalf`: the open point halfway along the ground way from the main's station to `other`,
/// when each half is at most `COVER_TILES * WAY_SLACK` long. The straight-line halfway point and
/// its checks never held on Crossroads, whose natural lies 39 tiles from the main around the water
/// walls: 3-5 Tanks waited at one of the two while the natural fell with 1-4 Tanks out.
///
/// The point is found by walking back from `other` downhill on the ground distances from Jeff's
/// start, so it lies on the way there, until the walk reaches halfway between the two stations'
/// ground distances.
fn way_halfway(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    analysis: &AiMapAnalysis,
    main: (f32, f32),
    other: (f32, f32),
) -> Option<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let slack = COVER_TILES * WAY_SLACK;
    let at_main = ground(observation, analysis, main)?;
    let at_other = ground(observation, analysis, other)?;
    let half = (at_other - at_main) * 0.5;
    if half <= 0.0 || half > slack {
        return None;
    }
    let target = at_main + half;
    let walk = |tile: (i32, i32)| {
        (tile.0 >= 0
            && tile.1 >= 0
            && (tile.0 as u32) < observation.map.width
            && (tile.1 as u32) < observation.map.height)
            .then(|| {
                analysis.ground_distance_from_start(
                    observation.player_id,
                    (tile.0 as u32, tile.1 as u32),
                )
            })
            .flatten()
    };
    let mut tile = ((other.0 / ts).floor() as i32, (other.1 / ts).floor() as i32);
    let mut here = walk(tile)?;
    let mut steps = 0;
    while here > target {
        steps += 1;
        if steps > 512 {
            return None;
        }
        let mut best: Option<(f32, (i32, i32))> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let next = (tile.0 + dx, tile.1 + dy);
                if let Some(distance) = walk(next).filter(|distance| *distance < here) {
                    if best.is_none_or(|(lowest, _)| distance < lowest) {
                        best = Some((distance, next));
                    }
                }
            }
        }
        let (distance, next) = best?;
        here = distance;
        tile = next;
    }
    let raw = ((tile.0 as f32 + 0.5) * ts, (tile.1 as f32 + 0.5) * ts);
    let point = *clear_points(observation, map_analysis, vec![raw]).first()?;
    let at = ground(observation, analysis, point)?;
    (at - at_main <= slack && at_other - at <= slack).then_some(point)
}

/// Places V1.1's free Tanks at its stations and sends them there (see the module docs).
/// `excluded`: units home defense or another system holds this decision.
pub(in crate::ai_core::decision) fn station(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    excluded: &BTreeSet<u32>,
) {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let mut memo = std::mem::take(&mut memory.jeff_v1_1.stations);
    let ordered: BTreeSet<u32> = actions
        .unit_orders_since(0)
        .into_iter()
        .map(|(unit, _)| unit)
        .collect();
    let picket = memory.route_line.picket();
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .collect();
    let free = |unit: &AiEntitySummary| {
        unit.free_for_combat
            && !excluded.contains(&unit.id)
            && !ordered.contains(&unit.id)
            && !memory.containment.active_tanks.contains(&unit.id)
            && !memory.containment.opening_tanks.contains(&unit.id)
            && memory.home_defensive_tank != Some(unit.id)
            && !memory.later_bases.guards.contains(&unit.id)
            && picket != Some(unit.id)
            && !memory.stays_out_of_home_defense(unit.id)
    };
    let fighting = |unit: &AiEntitySummary| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && enemy.kind != EntityKind::Worker
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(FIGHTING_TILES * ts)
        })
    };
    let alive: BTreeSet<u32> = tanks.iter().map(|unit| unit.id).collect();
    memo.posts.retain(|id, _| alive.contains(id));
    memo.away.retain(|id, _| alive.contains(id));
    let mut available: Vec<&AiEntitySummary> = Vec::new();
    for unit in &tanks {
        if !free(unit) {
            if memo.posts.contains_key(&unit.id) {
                memo.away.entry(unit.id).or_insert(tick);
            }
            continue;
        }
        if memo.away.contains_key(&unit.id) {
            // Back from elsewhere only once the fight it was in is over.
            if fighting(unit) {
                continue;
            }
            memo.away.remove(&unit.id);
        }
        available.push(unit);
    }

    // The bases and the threats each has seen.
    let bases = bases(observation);
    memo.threats.retain(|depot, (_, seen)| {
        tick.saturating_sub(*seen) < THREAT_TICKS && bases.iter().any(|base| base.depot == *depot)
    });
    for base in &bases {
        let value = threat_at(observation, base);
        if value == 0 {
            continue;
        }
        let entry = memo.threats.entry(base.depot).or_insert((0, tick));
        if value >= entry.0 {
            *entry = (value, tick);
        }
    }
    // Rounded, not up: rounded up, one scouting Rifleman at the main held its only Tanks there
    // while Pressure AI's push took the natural (Crossroads seed 3).
    let floor_of = |depot: u32| {
        memo.threats.get(&depot).map_or(0, |(value, _)| {
            (THREAT_SHARE * *value as f32 / TANK_VALUE).round() as usize
        })
    };

    // The stations: the home post first, then each base past it not covered by an earlier one.
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy_start = observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))
        .min_by(|a, b| dist2(a.0, a.1, home.0, home.1).total_cmp(&dist2(b.0, b.1, home.0, home.1)));
    let main = bases.iter().find(|base| base.main);
    let main_center = memory
        .home_post
        .map(|post| post.center())
        .or(main.map(|base| base.at))
        .unwrap_or(home);
    let main_facing = memory
        .home_post
        .map(|post| post.facing())
        .or_else(|| enemy_start.and_then(|start| normalized_direction(home, start)))
        .unwrap_or((1.0, 0.0));
    let mut stations: Vec<Station> = Vec::new();
    let lines = super::base_line::base_lines(
        observation,
        map_analysis,
        &memory.jeff_v1_1.base_lines.contacts,
    );
    // `mainfront`: the main's Tanks stand behind the main's line, which faces where the enemy walks
    // in, so its infantry stands in front of them; the home post faced the straight line to the
    // enemy start and put them out in front of the infantry.
    let main_line = (!super::off("mainfront"))
        .then(|| lines.iter().position(|line| line.main))
        .flatten();
    if let Some(main) = main {
        let (center, facing) = match main_line {
            Some(index) => (
                lines[index]
                    .tank_points(1, ts)
                    .first()
                    .copied()
                    .unwrap_or(main_center),
                lines[index].facing(),
            ),
            None => (main_center, main_facing),
        };
        stations.push(Station {
            anchor: main.depot,
            bases: vec![main.depot],
            center,
            facing,
            line: main_line,
            remote: 1.0,
            weight: main.extractors.len() as f32 + MAIN_WEIGHT,
            floor: floor_of(main.depot),
            main: true,
        });
    }
    for (index, line) in lines.iter().enumerate() {
        if line.main {
            continue;
        }
        let Some(base) = bases.iter().find(|base| base.depot == line.depot) else {
            continue;
        };
        let Some(point) = line.tank_points(1, ts).first().copied() else {
            continue;
        };
        let mines = base.extractors.len() as f32;
        let floor = floor_of(base.depot);
        if let Some(station) = stations.iter_mut().find(|station| {
            dist2(station.center.0, station.center.1, point.0, point.1) <= squared(COVER_TILES * ts)
        }) {
            station.bases.push(base.depot);
            station.weight += mines * station.remote;
            station.floor = station.floor.max(floor);
            continue;
        }
        let remote = 1.0
            + dist2(point.0, point.1, main_center.0, main_center.1).det_sqrt()
                / (REMOTE_TILES * ts);
        stations.push(Station {
            anchor: base.depot,
            bases: vec![base.depot],
            center: point,
            facing: normalized_direction(base.at, point).unwrap_or(main_facing),
            line: Some(index),
            remote,
            weight: mines * remote,
            floor,
            main: false,
        });
    }
    if stations.is_empty() {
        memo.summary = "none".to_string();
        memory.jeff_v1_1.stations = memo;
        return;
    }

    // Short of a group for every station, the two nearest that can share a halfway point do.
    let n = available.len();
    let groups = (n / TANK_GROUP).max(1);
    while stations.len() > groups {
        let mut best: Option<(f32, usize, usize, (f32, f32))> = None;
        for i in 0..stations.len() {
            for j in (i + 1)..stations.len() {
                let (a, b) = (&stations[i], &stations[j]);
                let apart = dist2(a.center.0, a.center.1, b.center.0, b.center.1);
                if best.is_some_and(|(nearest, ..)| apart >= nearest) {
                    continue;
                }
                if let Some(point) = halfway(observation, map_analysis, a, b) {
                    best = Some((apart, i, j, point));
                }
            }
        }
        let Some((_, i, j, point)) = best else {
            break;
        };
        let b = stations.remove(j);
        let a = &mut stations[i];
        a.bases.extend(b.bases);
        a.center = point;
        a.facing = enemy_start
            .and_then(|start| normalized_direction(point, start))
            .unwrap_or(a.facing);
        a.line = None;
        a.remote = a.remote.max(b.remote);
        a.weight += b.weight;
        a.floor = a.floor.max(b.floor);
        a.main |= b.main;
    }

    // Which stations hold Tanks, and how many each: threats first, then by weight.
    let mut order: Vec<usize> = (0..stations.len()).collect();
    order.sort_by(|a, b| {
        stations[*b]
            .floor
            .cmp(&stations[*a].floor)
            .then(stations[*b].weight.total_cmp(&stations[*a].weight))
            .then(a.cmp(b))
    });
    let mut chosen: Vec<usize> = order.into_iter().take(groups.min(stations.len())).collect();
    let quotas = loop {
        let mut quota = vec![0_usize; stations.len()];
        let mut left = n;
        for index in &chosen {
            let floor = stations[*index].floor.min(left);
            quota[*index] = floor;
            left -= floor;
        }
        let weights: Vec<usize> = (0..stations.len())
            .map(|index| {
                if chosen.contains(&index) {
                    (stations[index].weight * 10.0).round().max(1.0) as usize
                } else {
                    0
                }
            })
            .collect();
        for (index, extra) in super::base_line::shares(&weights, left)
            .into_iter()
            .enumerate()
        {
            quota[index] += extra;
        }
        // A station left with fewer than a group gives its Tanks to the others.
        let thin = chosen.len() > 1
            && chosen
                .iter()
                .any(|index| quota[*index] > 0 && quota[*index] < TANK_GROUP.min(n));
        if !thin {
            break quota;
        }
        chosen.pop();
    };

    // Each Tank's station: its own while that one is short, else the one furthest short.
    let station_of = |anchor: u32| {
        stations
            .iter()
            .position(|station| station.anchor == anchor || station.bases.contains(&anchor))
    };
    let on_post = |unit: &AiEntitySummary| {
        memo.posts.get(&unit.id).is_some_and(|post| {
            dist2(unit.x, unit.y, post.at.0 as f32, post.at.1 as f32) <= squared(ON_POST_TILES * ts)
        })
    };
    let mut keepers: Vec<&AiEntitySummary> = available.clone();
    keepers.sort_by_key(|unit| (!on_post(unit), unit.id));
    let mut counts = vec![0_usize; stations.len()];
    let mut placed: Vec<(u32, usize)> = Vec::new();
    let mut rest: Vec<&AiEntitySummary> = Vec::new();
    for unit in keepers {
        let kept = memo
            .posts
            .get(&unit.id)
            .and_then(|post| station_of(post.anchor))
            .filter(|index| counts[*index] < quotas[*index]);
        match kept {
            Some(index) => {
                counts[index] += 1;
                placed.push((unit.id, index));
            }
            None => rest.push(unit),
        }
    }
    rest.sort_by_key(|unit| unit.id);
    for unit in rest {
        let best = (0..stations.len())
            .filter(|index| counts[*index] < quotas[*index])
            .min_by(|a, b| {
                let short = |index: usize| quotas[index] as i64 - counts[index] as i64;
                short(*b).cmp(&short(*a)).then(
                    dist2(unit.x, unit.y, stations[*a].center.0, stations[*a].center.1).total_cmp(
                        &dist2(unit.x, unit.y, stations[*b].center.0, stations[*b].center.1),
                    ),
                )
            });
        if let Some(index) = best {
            counts[index] += 1;
            placed.push((unit.id, index));
        }
    }

    // Places at each station, and the orders to them.
    let mut posts: BTreeMap<u32, Post> = BTreeMap::new();
    let mut sent = Vec::new();
    for (index, station) in stations.iter().enumerate() {
        let members: Vec<&AiEntitySummary> = placed
            .iter()
            .filter(|(_, at)| *at == index)
            .filter_map(|(id, _)| available.iter().find(|unit| unit.id == *id).copied())
            .collect();
        if members.is_empty() {
            continue;
        }
        let wanted = members.len() + SPARE_POINTS;
        let raw = match station.line {
            Some(line) => lines[line].tank_points(wanted, ts),
            None => spread(station.center, station.facing, wanted, ts),
        };
        let points = clear_points(observation, map_analysis, raw);
        let points = &points[..points.len().min(members.len())];
        for (id, point) in seat(&members, points, ts) {
            let Some(unit) = members.iter().find(|unit| unit.id == id) else {
                continue;
            };
            let at = (point.0.round() as i32, point.1.round() as i32);
            let last = memo.posts.get(&id).copied();
            let same = last.filter(|post| {
                station_of(post.anchor) == Some(index)
                    && dist2(post.at.0 as f32, post.at.1 as f32, point.0, point.1)
                        <= squared(SAME_POST_TILES * ts)
            });
            let mut post = Post {
                anchor: station.anchor,
                at,
                ordered: same.map_or(0, |post| post.ordered),
            };
            let on = dist2(unit.x, unit.y, point.0, point.1) <= squared(ON_POST_TILES * ts);
            let since = same.map(|post| tick.saturating_sub(post.ordered));
            let waiting = since.is_some_and(|since| {
                since < MIN_REORDER_TICKS
                    || (since < REORDER_TICKS && unit.state != AiEntityState::Idle)
            });
            if !on && !waiting && !fighting(unit) {
                if let Some(units) = actions::attack_move_units(actions, [id], point.0, point.1) {
                    sent.extend(units);
                    post.ordered = tick;
                }
            }
            posts.insert(id, post);
        }
    }
    // Tanks away keep the station they had.
    for (id, post) in &memo.posts {
        if memo.away.contains_key(id) {
            posts.entry(*id).or_insert(*post);
        }
    }
    memo.posts = posts;
    let parts: Vec<String> = stations
        .iter()
        .enumerate()
        .map(|(index, station)| {
            format!(
                "{}:{}/{}w{:.0}f{}",
                station.anchor, counts[index], quotas[index], station.weight, station.floor
            )
        })
        .collect();
    memo.summary = format!(
        "n={n} g={groups} [{}] away={} sent={}",
        parts.join(" "),
        memo.away.len(),
        sent.len()
    );
    memory.jeff_v1_1.stations = memo;
    if !sent.is_empty() {
        // Clears the live adapter's cached staging, so a Tank staged before gets the order.
        intents.push(AiIntent::Assemble { units: sent });
    }
}

/// `riflestations`: the main keeps at least this many free Riflemen...
const MAIN_MIN_RIFLES: usize = 8;
/// ... and bases past the natural hold them only this many or more together.
const RIFLE_GROUP: usize = 3;
/// A Rifleman with a visible enemy unit this close fights where it stands.
const RIFLE_FIGHTING_TILES: f32 = 8.0;
/// A Depot this close to the natural site is the natural's.
const NATURAL_DEPOT_TILES: f32 = 2.0;

/// Whether Jeff's AI V1.1 splits its free Riflemen over its bases (`riflestations`).
pub(in crate::ai_core::decision) fn rifles_on(profile_id: &str) -> bool {
    is_jeff_v1_1(profile_id) && !super::off("riflestations")
}

/// `riflestations`: the free Riflemen in one split over the bases, in the same terms as the Tank
/// stations. `postreturn` held a supporter's place at the natural while home defense had it, and
/// the main ran short (Classic, 3-5 wins fewer in two 20-game sets). Here a Rifleman home defense
/// or anything else holds is simply out of the split, and is placed again with the rest once free.
///
/// - **Natural**: its line (`natural::support`) gets the Riflemen it wants first, its usual size,
///   nearest it first.
/// - **Main**: keeps `THREAT_SHARE` of the heaviest enemy force seen at it in the last
///   `THREAT_TICKS` (the Tank stations' memory), in Riflemen, and never fewer than
///   `MAIN_MIN_RIFLES`. It holds them in the home pocket, with every Rifleman left over.
/// - **Bases past the natural**: the rest, at most one base per `RIFLE_GROUP`, by the Tank
///   stations' weight (extractors, more the further from the main), each on the line in front of
///   its mines (`base_line`).
///
/// A Rifleman keeps its post while that post is short; the others fill the posts nearest first.
/// `held`: units other systems ordered or keep this decision.
pub(in crate::ai_core::decision) fn place_riflemen(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    held: &BTreeSet<u32>,
) {
    if !rifles_on(profile_id) {
        memory.jeff_v1_1.stations.rifles.clear();
        memory.jeff_v1_1.stations.rifle_summary.clear();
        return;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let reserved = memory.squad_reserved();
    let picket = memory.route_line.picket();
    let pool: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !held.contains(&unit.id)
                && !reserved.contains(&unit.id)
                && !memory.containment.active_riflemen.contains(&unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.jeff_v1_1.natclear.holds(unit.id)
                && !memory.stays_out_of_home_defense(unit.id)
                && !memory.later_bases.guards.contains(&unit.id)
                && !memory.jeff_v1_1.deny.holds(unit.id)
                && !memory.rush_reserved(unit.id)
                && picket != Some(unit.id)
                && !memory.jeff_v1_1.charge.riflemen.contains(&unit.id)
                && !memory.jeff_v1_1.hunt.hunters.contains_key(&unit.id)
                && !memory.jeff_v1_1.strike.party.contains(&unit.id)
                && !super::south_route::push_spotter(memory, unit.id)
        })
        .collect();
    let bases = bases(observation);
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let main = bases.iter().find(|base| base.main);
    let main_at = main.map_or(home, |base| base.at);
    let natural_center = memory.expansion_security.site.and_then(|site| {
        crate::ai_core::decision::geometry::building_center(
            site,
            EntityKind::ResourceDepot,
            observation.map.tile_size,
        )
    });
    let natural_depot = natural_center.and_then(|center| {
        bases
            .iter()
            .find(|base| {
                dist2(base.at.0, base.at.1, center.0, center.1) <= squared(NATURAL_DEPOT_TILES * ts)
            })
            .map(|base| base.depot)
    });
    let main_threat = main
        .and_then(|base| memory.jeff_v1_1.stations.threats.get(&base.depot))
        .map_or(0, |(value, _)| *value);
    let rifle_value = super::push::unit_value(EntityKind::Rifleman).max(1) as f32;
    let main_need =
        MAIN_MIN_RIFLES.max((THREAT_SHARE * main_threat as f32 / rifle_value).ceil() as usize);
    let n = pool.len();
    let natural_quota = super::natural::support_wanted(memory, profile_id).min(n);
    let main_floor = main_need.min(n - natural_quota);
    let left = n - natural_quota - main_floor;
    // Bases past the natural, by weight.
    let mut outer: Vec<(u32, f32)> = bases
        .iter()
        .filter(|base| {
            !base.main && Some(base.depot) != natural_depot && !base.extractors.is_empty()
        })
        .map(|base| {
            let remote = 1.0
                + dist2(base.at.0, base.at.1, main_at.0, main_at.1).det_sqrt()
                    / (REMOTE_TILES * ts);
            (base.depot, base.extractors.len() as f32 * remote)
        })
        .collect();
    outer.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    outer.truncate(left / RIFLE_GROUP);
    let outer_quotas = loop {
        let weights: Vec<usize> = outer
            .iter()
            .map(|(_, weight)| (weight * 10.0).round().max(1.0) as usize)
            .collect();
        let quotas = super::base_line::shares(&weights, left);
        if outer.len() <= 1 || quotas.iter().all(|quota| *quota >= RIFLE_GROUP) {
            break quotas;
        }
        outer.pop();
    };
    let mut quota: BTreeMap<RiflePost, usize> = BTreeMap::new();
    quota.insert(RiflePost::Natural, natural_quota);
    for ((depot, _), share) in outer.iter().zip(&outer_quotas) {
        quota.insert(RiflePost::Base(*depot), *share);
    }
    // Keepers first, then each post fills from the nearest Riflemen left.
    let mut counts: BTreeMap<RiflePost, usize> = BTreeMap::new();
    let mut posts: BTreeMap<u32, RiflePost> = BTreeMap::new();
    for unit in &pool {
        let Some(post) = memory
            .jeff_v1_1
            .stations
            .rifles
            .get(&unit.id)
            .map(|place| place.post)
        else {
            continue;
        };
        if post == RiflePost::Main {
            continue;
        }
        let count = counts.entry(post).or_insert(0);
        if *count < quota.get(&post).copied().unwrap_or(0) {
            *count += 1;
            posts.insert(unit.id, post);
        }
    }
    let base_at = |depot: u32| {
        bases
            .iter()
            .find(|base| base.depot == depot)
            .map(|base| base.at)
    };
    for (post, wanted) in &quota {
        let at = match post {
            RiflePost::Natural => natural_center,
            RiflePost::Base(depot) => base_at(*depot),
            RiflePost::Main => None,
        };
        let Some(at) = at else {
            continue;
        };
        let mut free: Vec<&&AiEntitySummary> = pool
            .iter()
            .filter(|unit| !posts.contains_key(&unit.id))
            .collect();
        free.sort_by(|a, b| {
            dist2(a.x, a.y, at.0, at.1)
                .total_cmp(&dist2(b.x, b.y, at.0, at.1))
                .then(a.id.cmp(&b.id))
        });
        let have = counts.get(post).copied().unwrap_or(0);
        for unit in free.into_iter().take(wanted.saturating_sub(have)) {
            posts.insert(unit.id, *post);
        }
    }
    for unit in &pool {
        posts.entry(unit.id).or_insert(RiflePost::Main);
    }

    // Orders for the bases past the natural: the line in front of each one's mines.
    let lines = super::base_line::base_lines(
        observation,
        map_analysis,
        &memory.jeff_v1_1.base_lines.contacts,
    );
    let previous = std::mem::take(&mut memory.jeff_v1_1.stations.rifles);
    let mut places: BTreeMap<u32, RiflePlace> = BTreeMap::new();
    let mut sent = Vec::new();
    let fighting = |unit: &AiEntitySummary| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && enemy.kind != EntityKind::Worker
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(RIFLE_FIGHTING_TILES * ts)
        })
    };
    let mut parts = Vec::new();
    for (depot, _) in &outer {
        let members: Vec<&AiEntitySummary> = pool
            .iter()
            .filter(|unit| posts.get(&unit.id) == Some(&RiflePost::Base(*depot)))
            .copied()
            .collect();
        parts.push(format!("{depot}:{}", members.len()));
        let Some(line) = lines.iter().find(|line| line.depot == *depot) else {
            continue;
        };
        if members.is_empty() {
            continue;
        }
        let wanted = line.rifle_points(members.len(), SPARE_POINTS, ts);
        let points = clear_points(observation, map_analysis, wanted);
        let points = &points[..points.len().min(members.len())];
        for (id, point) in seat(&members, points, ts) {
            let Some(unit) = members.iter().find(|unit| unit.id == id) else {
                continue;
            };
            let at = (point.0.round() as i32, point.1.round() as i32);
            let same = previous.get(&id).filter(|place| {
                place.post == RiflePost::Base(*depot)
                    && dist2(place.at.0 as f32, place.at.1 as f32, point.0, point.1)
                        <= squared(SAME_POST_TILES * ts)
            });
            let mut place = RiflePlace {
                post: RiflePost::Base(*depot),
                at,
                ordered: same.map_or(0, |place| place.ordered),
            };
            let on = dist2(unit.x, unit.y, point.0, point.1) <= squared(ON_POST_TILES * ts);
            let since = same.map(|place| tick.saturating_sub(place.ordered));
            let waiting = since.is_some_and(|since| {
                since < MIN_REORDER_TICKS
                    || (since < REORDER_TICKS && unit.state != AiEntityState::Idle)
            });
            if !on && !waiting && !fighting(unit) {
                if let Some(units) = actions::attack_move_units(actions, [id], point.0, point.1) {
                    sent.extend(units);
                    place.ordered = tick;
                }
            }
            places.insert(id, place);
        }
    }
    for (id, post) in &posts {
        places.entry(*id).or_insert(RiflePlace {
            post: *post,
            at: (0, 0),
            ordered: 0,
        });
    }
    let count = |post: RiflePost| places.values().filter(|place| place.post == post).count();
    memory.jeff_v1_1.stations.rifle_summary = format!(
        " rifles n={n} main={}/{main_floor} nat={}/{natural_quota} [{}]",
        count(RiflePost::Main),
        count(RiflePost::Natural),
        parts.join(" ")
    );
    memory.jeff_v1_1.stations.rifles = places;
    if !sent.is_empty() {
        intents.push(AiIntent::Assemble { units: sent });
    }
}
