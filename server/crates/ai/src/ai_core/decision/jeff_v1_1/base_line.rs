//! Jeff's AI V1.1 holds a line in front of the mines at every base. V1.0's home Riflemen stood in
//! a pocket in front of the main's production, its Tanks and infantry rested behind or beside the
//! natural, and 37-53% of the mines it lost had no Jeff unit within 6 tiles.
//!
//! - **Threat axis**: the way the enemy's shortest ground route reaches the base, the tile
//!   `AXIS_RING_TILES` out from the Depot the enemy walks to soonest, not the straight line to the
//!   enemy start.
//! - **The line**: `LINE_AHEAD_TILES` past the forward-most mine along that axis, as wide as the
//!   mines' sideways spread and `LINE_WING_TILES` more each side. Riflemen stand
//!   `RIFLE_SPACING_TILES` apart from its centre out, in ranks; Machine Gunners hold its two wings;
//!   one Rifleman stands picket `PICKET_TILES` further out on the axis once a base has
//!   `PICKET_MIN_RIFLES`.
//! - **Units per base**: Riflemen split over the bases by exposure, the base's mines doubled when
//!   it is on the enemy's route to the main; the guns past the home pocket's split the same way
//!   over the bases past the main, or hold the main's wings while there are none.
//! - **Stable posts**: a unit keeps its base while that base is at most one over its share, and a
//!   unit on a post keeps it, so new units and losses never walk dug-in units out of their trenches.
//!   Units with an enemy in sight nearby are left to fight.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

/// Extractors this close to a Depot belong to its base.
const MINE_TILES: f32 = 12.0;
/// A base is a Depot this far from the main's start or more; a nearer one is the main.
const MAIN_TILES: f32 = 8.0;
/// The threat axis points at the tile this far from the Depot that the enemy reaches first, among
/// `AXIS_SAMPLES` around it.
const AXIS_RING_TILES: f32 = 10.0;
const AXIS_SAMPLES: usize = 32;
/// The line stands this far past the forward-most mine, and at least `MIN_LINE_DEPTH_TILES` in
/// front of the Depot.
const LINE_AHEAD_TILES: f32 = 2.0;
const MIN_LINE_DEPTH_TILES: f32 = 3.0;
/// The line reaches this far past the outermost mines on each side, and is at least
/// `MIN_HALF_WIDTH_TILES` and at most `MAX_HALF_WIDTH_TILES` either side of its centre.
const LINE_WING_TILES: f32 = 1.0;
const MIN_HALF_WIDTH_TILES: f32 = 3.0;
const MAX_HALF_WIDTH_TILES: f32 = 8.0;
const RIFLE_SPACING_TILES: f32 = 1.75;
/// Each further rank, and each further pair of guns, stands this far behind the one before.
const RANK_BACK_TILES: f32 = 1.5;
/// The wing guns stand this far behind the Riflemen's front rank.
const GUN_BACK_TILES: f32 = 0.5;
/// The picket stands this far past the line on the axis, for vision.
const PICKET_TILES: f32 = 7.0;
const PICKET_MIN_RIFLES: usize = 4;
/// A base within this distance of the enemy's route to the main is on it.
const ON_ROUTE_TILES: f32 = 12.0;
/// A unit this close to its post holds it.
const ON_POST_TILES: f32 = 1.5;
/// Two posts this close are one.
const SAME_POST_TILES: f32 = 1.0;
/// A unit with a visible enemy unit this close is fighting and gets no post order.
const FIGHTING_TILES: f32 = 14.0;
/// A unit walking to its post is sent again at most this often, or sooner once it stands idle,
/// but never sooner than `MIN_REORDER_TICKS`.
const REORDER_TICKS: u32 = config::TICK_HZ * 10;
const MIN_REORDER_TICKS: u32 = config::TICK_HZ * 2;

/// The post a line unit was last sent to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct LinePost {
    depot: u32,
    at: (i32, i32),
    ordered: u32,
}

/// One base's line, in world pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ai_core::decision) struct BaseLine {
    pub(in crate::ai_core::decision) depot: u32,
    pub(in crate::ai_core::decision) main: bool,
    origin: (f32, f32),
    /// Unit vector toward where the enemy arrives, and the unit vector across it.
    axis: (f32, f32),
    across: (f32, f32),
    /// How far the line stands in front of the Depot along the axis.
    depth: f32,
    /// Its extent across the axis, from the Depot's centre line.
    lateral: (f32, f32),
    pub(in crate::ai_core::decision) exposure: usize,
}

impl BaseLine {
    fn at(&self, ahead: f32, side: f32) -> (f32, f32) {
        (
            self.origin.0 + self.axis.0 * ahead + self.across.0 * side,
            self.origin.1 + self.axis.1 * ahead + self.across.1 * side,
        )
    }

    /// Where the `count` Riflemen stand: the front rank from its centre out, then ranks behind it,
    /// and the picket out on the axis from `PICKET_MIN_RIFLES` on.
    pub(in crate::ai_core::decision) fn rifle_points(
        &self,
        count: usize,
        ts: f32,
    ) -> Vec<(f32, f32)> {
        let picket = count >= PICKET_MIN_RIFLES;
        let spacing = RIFLE_SPACING_TILES * ts;
        let half = (self.lateral.1 - self.lateral.0) * 0.5;
        let middle = (self.lateral.0 + self.lateral.1) * 0.5;
        let per_rank = ((half / spacing).floor() as usize) * 2 + 1;
        let mut points: Vec<(f32, f32)> = (0..count - usize::from(picket))
            .map(|index| {
                let (rank, slot) = (index / per_rank, index % per_rank);
                let step = slot.div_ceil(2) as f32 * spacing;
                let side = if slot % 2 == 1 { step } else { -step };
                self.at(
                    self.depth - rank as f32 * RANK_BACK_TILES * ts,
                    middle + side,
                )
            })
            .collect();
        if picket {
            points.push(self.at(self.depth + PICKET_TILES * ts, middle));
        }
        points
    }

    /// Where the `count` guns stand: the two wings first, then pairs further in and back.
    pub(in crate::ai_core::decision) fn gun_points(
        &self,
        count: usize,
        ts: f32,
    ) -> Vec<(f32, f32)> {
        (0..count)
            .map(|index| {
                let pair = (index / 2) as f32;
                let inward = (pair * RIFLE_SPACING_TILES * ts).min(self.lateral.1 - self.lateral.0);
                let side = if index % 2 == 0 {
                    self.lateral.1 - inward
                } else {
                    self.lateral.0 + inward
                };
                let back = (GUN_BACK_TILES + pair * RANK_BACK_TILES) * ts;
                self.at(self.depth - back, side)
            })
            .collect()
    }
}

/// V1.1's lines at every finished Depot, the main's first.
pub(in crate::ai_core::decision) fn base_lines(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
) -> Vec<BaseLine> {
    let ts = observation.map.tile_size as f32;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy = nearest_enemy(observation);
    let depots: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .collect();
    let route = map_analysis.and_then(|analysis| analysis.base_route_tiles(observation.player_id));
    let mut lines: Vec<BaseLine> = depots
        .iter()
        .filter_map(|depot| {
            let origin = (depot.x, depot.y);
            let axis = threat_axis(observation, map_analysis, origin, enemy?)?;
            let across = (-axis.1, axis.0);
            let mines: Vec<(f32, f32)> = observation
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
                .map(|unit| (unit.x - depot.x, unit.y - depot.y))
                .collect();
            let ahead = |point: &(f32, f32)| point.0 * axis.0 + point.1 * axis.1;
            let side = |point: &(f32, f32)| point.0 * across.0 + point.1 * across.1;
            let forward = mines.iter().map(ahead).fold(0.0_f32, f32::max);
            let depth = (forward + LINE_AHEAD_TILES * ts).max(MIN_LINE_DEPTH_TILES * ts);
            let wing = LINE_WING_TILES * ts;
            let mut lateral = (
                mines.iter().map(side).fold(0.0_f32, f32::min) - wing,
                mines.iter().map(side).fold(0.0_f32, f32::max) + wing,
            );
            let middle = (lateral.0 + lateral.1) * 0.5;
            let half = ((lateral.1 - lateral.0) * 0.5)
                .clamp(MIN_HALF_WIDTH_TILES * ts, MAX_HALF_WIDTH_TILES * ts);
            lateral = (middle - half, middle + half);
            let on_route = route.is_some_and(|route| {
                route.iter().any(|tile| {
                    let point = tile_center((tile.x, tile.y), observation.map.tile_size);
                    dist2(point.0, point.1, depot.x, depot.y) <= squared(ON_ROUTE_TILES * ts)
                })
            });
            Some(BaseLine {
                depot: depot.id,
                main: dist2(depot.x, depot.y, home.0, home.1) < squared(MAIN_TILES * ts),
                origin,
                axis,
                across,
                depth,
                lateral,
                exposure: mines.len().max(1) * if on_route { 2 } else { 1 },
            })
        })
        .collect();
    lines.sort_by(|a, b| b.main.cmp(&a.main).then(a.depot.cmp(&b.depot)));
    lines
}

/// The nearest living enemy: its player id and start, in world pixels.
fn nearest_enemy(observation: &AiObservation) -> Option<(u32, (f32, f32))> {
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .map(|player| {
            let start = tile_center(player.start_tile, observation.map.tile_size);
            (player.id, start)
        })
        .min_by(|a, b| {
            dist2(a.1 .0, a.1 .1, home.0, home.1)
                .total_cmp(&dist2(b.1 .0, b.1 .1, home.0, home.1))
                .then(a.0.cmp(&b.0))
        })
}

/// Toward the tile around `origin` the enemy's units reach soonest over the ground, or toward the
/// enemy start without the map's ground distances.
fn threat_axis(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    origin: (f32, f32),
    (enemy_id, enemy_start): (u32, (f32, f32)),
) -> Option<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let nearest = map_analysis.and_then(|analysis| {
        (0..AXIS_SAMPLES)
            .filter_map(|sample| {
                let angle = sample as f32 * std::f32::consts::TAU / AXIS_SAMPLES as f32;
                let point = (
                    origin.0 + angle.cos() * AXIS_RING_TILES * ts,
                    origin.1 + angle.sin() * AXIS_RING_TILES * ts,
                );
                if point.0 < 0.0 || point.1 < 0.0 {
                    return None;
                }
                let tile = ((point.0 / ts) as u32, (point.1 / ts) as u32);
                let walk = analysis.ground_distance_from_start(enemy_id, tile)?;
                Some((walk, point))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, point)| point)
    });
    normalized_direction(origin, nearest.unwrap_or(enemy_start))
}

/// `total` units split by `weights`, largest remainders first, ties to the earlier entry.
pub(in crate::ai_core::decision) fn shares(weights: &[usize], total: usize) -> Vec<usize> {
    let sum: usize = weights.iter().sum();
    if sum == 0 {
        return vec![0; weights.len()];
    }
    let mut shares: Vec<usize> = weights.iter().map(|weight| total * weight / sum).collect();
    let mut rest: Vec<(usize, usize)> = weights
        .iter()
        .enumerate()
        .map(|(index, weight)| (total * weight % sum, index))
        .collect();
    rest.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let left = total - shares.iter().sum::<usize>();
    for (_, index) in rest.into_iter().take(left) {
        shares[index] += 1;
    }
    shares
}

/// Which line each unit holds. `units` are (id, position, base it held before), ids ascending.
/// A unit keeps its base while that base is at most one over its share, units already on a post
/// first; the others go to the base furthest under its share, the nearest one on a tie.
pub(in crate::ai_core::decision) fn allocate(
    units: &[(u32, (f32, f32), Option<u32>)],
    lines: &[BaseLine],
    weights: &[usize],
    on_post: &BTreeSet<u32>,
) -> Vec<(u32, usize)> {
    let shares = shares(weights, units.len());
    let mut counts = vec![0_usize; lines.len()];
    let mut placed = Vec::new();
    let mut keepers: Vec<&(u32, (f32, f32), Option<u32>)> = units.iter().collect();
    keepers.sort_by_key(|(id, _, _)| (!on_post.contains(id), *id));
    let mut rest = Vec::new();
    for unit in keepers {
        let kept = unit
            .2
            .and_then(|depot| lines.iter().position(|line| line.depot == depot))
            .filter(|index| counts[*index] < shares[*index] + 1 && weights[*index] > 0);
        match kept {
            Some(index) => {
                counts[index] += 1;
                placed.push((unit.0, index));
            }
            None => rest.push(unit),
        }
    }
    rest.sort_by_key(|(id, _, _)| *id);
    for (id, at, _) in rest {
        let best =
            (0..lines.len())
                .filter(|index| weights[*index] > 0)
                .min_by(|a, b| {
                    let short = |index: usize| shares[index] as i64 - counts[index] as i64;
                    short(*b).cmp(&short(*a)).then(
                        dist2(at.0, at.1, lines[*a].origin.0, lines[*a].origin.1)
                            .total_cmp(&dist2(at.0, at.1, lines[*b].origin.0, lines[*b].origin.1)),
                    )
                });
        if let Some(index) = best {
            counts[index] += 1;
            placed.push((*id, index));
        }
    }
    placed.sort_unstable();
    placed
}

/// Each unit's post among `points`: a unit standing on one keeps it, and the other posts, in order,
/// take the nearest unit left.
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

/// Clears each wanted point to open ground and drops one that clears onto an earlier post.
fn clear_points(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    wanted: Vec<(f32, f32)>,
    gun_facing: Option<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let mut points: Vec<(f32, f32)> = Vec::new();
    for point in wanted {
        let point = clamp_to_map(point, observation.map);
        let cleared = match gun_facing {
            Some(facing) => {
                defense::clear_machine_gunner_position(observation, map_analysis, point, facing)
            }
            None => defense::clear_mobile_defensive_position(observation, map_analysis, point),
        };
        if let Some(cleared) = cleared.filter(|cleared| {
            points.iter().all(|other| {
                dist2(cleared.0, cleared.1, other.0, other.1) > squared(SAME_POST_TILES * ts)
            })
        }) {
            points.push(cleared);
        }
    }
    points
}

/// The units V1.1 posts on its base lines, with the base each held before.
fn line_units<'a>(
    observation: &'a AiObservation,
    memory: &AiDecisionMemory,
    kind: EntityKind,
    eligible: impl Fn(&AiEntitySummary) -> bool,
) -> Vec<&'a AiEntitySummary> {
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == kind && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !memory.containment.active_riflemen.contains(&unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.later_bases.guards.contains(&unit.id)
                && !memory.opening_rush.is_reserved(unit.id)
                && !memory.stays_out_of_home_defense(unit.id)
                && memory.route_line.picket() != Some(unit.id)
        })
        .filter(|unit| eligible(unit))
        .collect()
}

/// V1.1's Riflemen and the guns past the home pocket's (`home_guns`) hold the lines in front of
/// each base's mines (see the module docs). `held` are the units other systems ordered or keep
/// this decision.
pub(in crate::ai_core::decision) fn hold(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    home_guns: &[u32],
    held: &BTreeSet<u32>,
) {
    if !is_jeff_v1_1(profile.id) {
        return;
    }
    let ts = observation.map.tile_size as f32;
    let lines = base_lines(observation, map_analysis);
    if lines.is_empty() {
        memory.jeff_v1_1.base_lines.clear();
        return;
    }
    let healthy = profile
        .defensive_machine_gunners
        .and_then(|policy| policy.replacement_health_percent)
        .unwrap_or(0);
    let riflemen = line_units(observation, memory, EntityKind::Rifleman, |unit| {
        !held.contains(&unit.id)
    });
    let guns = line_units(observation, memory, EntityKind::MachineGunner, |unit| {
        !held.contains(&unit.id)
            && !home_guns.contains(&unit.id)
            && defense::machine_gunner_meets_replacement_health(unit.hp, healthy)
    });
    let rifle_weights: Vec<usize> = lines.iter().map(|line| line.exposure).collect();
    let outer = lines.iter().any(|line| !line.main);
    let gun_weights: Vec<usize> = lines
        .iter()
        .map(|line| if outer && line.main { 0 } else { line.exposure })
        .collect();
    let previous = &memory.jeff_v1_1.base_lines;
    let near_post = |unit: &AiEntitySummary| {
        previous.get(&unit.id).is_some_and(|post| {
            dist2(unit.x, unit.y, post.at.0 as f32, post.at.1 as f32) <= squared(ON_POST_TILES * ts)
        })
    };
    let mut seated: Vec<(u32, u32, (f32, f32), bool)> = Vec::new();
    for (units, weights, guns) in [
        (&riflemen, &rifle_weights, false),
        (&guns, &gun_weights, true),
    ] {
        let entries: Vec<(u32, (f32, f32), Option<u32>)> = units
            .iter()
            .map(|unit| {
                (
                    unit.id,
                    (unit.x, unit.y),
                    previous.get(&unit.id).map(|post| post.depot),
                )
            })
            .collect();
        let on_post: BTreeSet<u32> = units
            .iter()
            .filter(|unit| near_post(unit))
            .map(|unit| unit.id)
            .collect();
        let placed = allocate(&entries, &lines, weights, &on_post);
        for (index, line) in lines.iter().enumerate() {
            let members: Vec<&AiEntitySummary> = placed
                .iter()
                .filter(|(_, at)| *at == index)
                .filter_map(|(id, _)| units.iter().find(|unit| unit.id == *id).copied())
                .collect();
            if members.is_empty() {
                continue;
            }
            // A few spare points, for posts that clear onto one another.
            let wanted = if guns {
                line.gun_points(members.len() + 4, ts)
            } else {
                line.rifle_points(members.len() + 4, ts)
            };
            let facing = guns.then_some(line.axis);
            let points = clear_points(observation, map_analysis, wanted, facing);
            let points = &points[..points.len().min(members.len())];
            for (id, point) in seat(&members, points, ts) {
                seated.push((id, line.depot, point, guns));
            }
        }
    }
    let tick = observation.tick;
    let fighting = |unit: &AiEntitySummary| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.kind.is_unit()
                && enemy.hp > 0
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(FIGHTING_TILES * ts)
        })
    };
    let mut posts = BTreeMap::new();
    let mut ordered = Vec::new();
    for (id, depot, point, _) in seated {
        let Some(unit) = observation.owned.iter().find(|unit| unit.id == id) else {
            continue;
        };
        let at = (point.0.round() as i32, point.1.round() as i32);
        let last = memory.jeff_v1_1.base_lines.get(&id).copied();
        let same = last.filter(|post| {
            post.depot == depot
                && dist2(post.at.0 as f32, post.at.1 as f32, point.0, point.1)
                    <= squared(SAME_POST_TILES * ts)
        });
        let mut post = LinePost {
            depot,
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
                ordered.extend(units);
                post.ordered = tick;
            }
        }
        posts.insert(id, post);
    }
    memory.jeff_v1_1.base_lines = posts;
    if !ordered.is_empty() {
        // Clears the live adapter's cached staging, so a unit moved to a new post gets the order.
        intents.push(AiIntent::Assemble { units: ordered });
    }
}
