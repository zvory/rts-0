//! Jeff's AI V1.1's push. V1.0 weighed its push against the most enemy Tanks seen lately, needed a
//! Scout Car to leave, and marched straight down the route at the enemy natural, into whatever
//! stood in the middle of the map; against Pressure AI, which fields Riflemen, guns and Panzerfausts
//! and runs five bases, it lost or drew all 40 games and never reached the enemy half.
//!
//! - **Whole armies**: every enemy fighter seen is remembered at its cost (`unit_value`), where it
//!   was, fading out over `ARMY_FADE_TICKS`, or at once when its last position is in sight without
//!   it. The launch weighs the push (Tanks, Riflemen, spotter) against those seen near the route.
//! - **Spotter**: with no Scout Car a Rifleman leads.
//! - **Raids onto the enemy half**: every enemy base seen (Depots and extractors are remembered
//!   too), the enemy main, and the resource fields nearer the enemy start that Jeff has not seen
//!   empty are targets. The push takes the one most worth hitting for the least defence near it,
//!   among those it outweighs `RAID_RATIO` times: a few guns at a far base before a Tank cluster at
//!   the natural. When remembered enemies stand on the way in, it takes the side lane with the
//!   least on it, sidestepping them; a target with no clear way is passed over.
//! - **The enemy answers**: targets are weighed again every `RETARGET_TICKS`. A target whose
//!   defence outgrows the push, or where the enemy in front outweighs it, is dropped for
//!   `HOT_TICKS` and the push turns to another, falling back home only when routed.
//! - **Rolling line**: with no target worth a raid, the push holds anchor after anchor along the
//!   enemy's route (`ANCHOR_SHARES` of the way, each moved onto a choke near it, then the enemy
//!   natural), stepping forward once it holds an anchor and is ahead, and back one when the enemy
//!   in front outweighs it. New Tanks rally to the push and join it there.
//! - **Strike the gathering wave**: Pressure AI gathers its waves about 25 tiles out from Jeff's
//!   natural and waits there for 5, 10, then 20 arrivals. A group within `STRIKE_TILES` of a Jeff
//!   base that the push outweighs `STRIKE_RATIO` times is hit while it gathers.

use super::*;
use crate::ai_core::decision::frontal::approach::{self, Approach};
use crate::ai_core::decision::geometry::{dist2, squared};
use crate::ai_core::observation::AiPlayerSummary;

/// A remembered enemy fighter counts fully when just seen and fades to nothing over this long.
const ARMY_FADE_TICKS: u32 = config::TICK_HZ * 90;
/// A remembered enemy whose last position is this close to one of Jeff's units or buildings is in
/// sight, so it is gone if it is not seen there.
const SIGHT_TILES: f32 = 8.0;
/// The push leaves, and steps forward, once it outweighs the remembered enemy army this much.
const ADVANCE_RATIO: f32 = 1.2;
/// Enemies seen this close to the enemy's route to Jeff, or to the push, count against its launch.
const ROUTE_TILES: f32 = 20.0;
/// Enemies this close to a push unit are the fight in front of it.
const FRONT_TILES: f32 = 16.0;
/// Anchors along the enemy's route, as shares of its length from Jeff's start, each moved onto the
/// map choke within `CHOKE_SNAP_TILES` of it.
const ANCHOR_SHARES: [f32; 3] = [0.3, 0.5, 0.7];
const CHOKE_SNAP_TILES: f32 = 12.0;
/// The push holds an anchor once its centre is this close to it, for `ANCHOR_HOLD_TICKS`.
const ANCHOR_REACHED_TILES: f32 = 10.0;
const ANCHOR_HOLD_TICKS: u32 = config::TICK_HZ * 8;
/// At most one step, forward or back, this often.
const STEP_TICKS: u32 = config::TICK_HZ * 10;
/// Free Tanks this close to the push's rally point join it.
const JOIN_TILES: f32 = 8.0;
const STRIKE_TILES: f32 = 30.0;
const STRIKE_RATIO: f32 = 1.5;
/// Enemies this close to one another are one group.
const GROUP_TILES: f32 = 8.0;
/// A strike holds its target point this long after the group was last seen.
const STRIKE_MEMORY_TICKS: u32 = config::TICK_HZ * 10;
/// Enemy buildings this close to an enemy Depot are its base.
const BASE_TILES: f32 = 12.0;
/// Enemy fighters this close to a target defend it.
const DEFENCE_TILES: f32 = 14.0;
/// Enemy fighters this close to a point on the way in stand on it.
const THREAT_TILES: f32 = 10.0;
/// The push raids a target it outweighs this many times over its defence.
const RAID_RATIO: f32 = 1.3;
/// Enemies on the direct way in worth this share of the push send it round by a side lane, which
/// must carry at most half as much; worth `BLOCKED_SHARE` with no such lane, the target is passed.
const SIDESTEP_SHARE: f32 = 0.5;
const BLOCKED_SHARE: f32 = 0.8;
/// Targets are weighed again this often; another replaces the current one only when it scores this
/// many times as well.
const RETARGET_TICKS: u32 = config::TICK_HZ * 5;
const SWITCH_RATIO: f32 = 1.5;
/// A target dropped as too hot is left alone this long.
const HOT_TICKS: u32 = config::TICK_HZ * 90;
/// A resource field seen with no enemy building at it is passed over this long.
const EMPTY_TICKS: u32 = config::TICK_HZ * 120;
/// What an unseen enemy main and a resource field that may hold a base are worth as targets.
const MAIN_WORTH: f32 = 1500.0;
const SUSPECTED_WORTH: f32 = 300.0;
/// Fields this close to a Jeff building are Jeff's.
const OWN_FIELD_TILES: f32 = 15.0;
/// Outweighed this many times on a raid, the push falls back home instead of turning elsewhere.
const ROUT_RATIO: f32 = 1.5;
/// Travel to a target costs its score one share per this many tiles.
const TRAVEL_TILES: f32 = 60.0;

/// An enemy fighter as last seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct Sighting {
    tick: u32,
    value: u32,
    at: (i32, i32),
}

/// An enemy building as last seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct BuildingSighting {
    kind: EntityKind,
    at: (i32, i32),
}

/// The base the push raids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct RaidTarget {
    at: (i32, i32),
    side: Option<Approach>,
    /// Its score when chosen, in thousandths.
    score: i64,
}

/// What V1.1's push remembers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct PushMemory {
    enemies: BTreeMap<u32, Sighting>,
    buildings: BTreeMap<u32, BuildingSighting>,
    /// The anchor the push holds or makes for; past the last, the enemy natural.
    anchor: usize,
    last_step: u32,
    /// Since when the push has stood at its anchor.
    holding_since: Option<u32>,
    /// A gathering enemy group being struck, and when it was last seen.
    strike: Option<((i32, i32), u32)>,
    target: Option<RaidTarget>,
    retargeted: u32,
    /// Targets dropped as too hot, until when.
    hot: BTreeMap<(i32, i32), u32>,
    /// Resource fields seen empty, by cluster id, until when.
    empty: BTreeMap<u32, u32>,
    /// Whether the push holds short of its target, at an anchor or a strike.
    short: bool,
    /// Where new Tanks gather to join the launched push.
    rally: Option<(i32, i32)>,
}

impl PushMemory {
    /// Whether a gathering enemy group is being struck.
    pub(in crate::ai_core::decision) fn striking(&self) -> bool {
        self.strike.is_some()
    }
}

fn v1_1(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1)
}

fn is_fighter(kind: EntityKind) -> bool {
    kind.is_unit() && !matches!(kind, EntityKind::Worker | EntityKind::Golem)
}

fn point((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32, y as f32)
}

fn stored((x, y): (f32, f32)) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

/// A unit's weight in a fight: its cost, Oil counted double.
pub(in crate::ai_core::decision) fn unit_value(kind: EntityKind) -> u32 {
    let (steel, oil) = rts_rules::economy::cost(kind);
    steel + oil * 2
}

/// Notes the enemy fighters and buildings in sight and forgets those gone. V1.1 only.
pub(in crate::ai_core::decision) fn note_enemy_army(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) {
    if !v1_1(memory) {
        return;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let push = &mut memory.jeff_v1_1.push;
    let mut seen = BTreeSet::new();
    for enemy in observation.visible_enemies.iter().filter(|enemy| enemy.hp > 0) {
        seen.insert(enemy.id);
        let at = stored((enemy.x, enemy.y));
        if is_fighter(enemy.kind) {
            let value = unit_value(enemy.kind);
            push.enemies.insert(enemy.id, Sighting { tick, value, at });
        } else if enemy.kind.is_building() {
            let kind = enemy.kind;
            push.buildings.insert(enemy.id, BuildingSighting { kind, at });
        }
    }
    let in_sight = |at: (i32, i32)| {
        let at = point(at);
        observation
            .owned
            .iter()
            .any(|unit| dist2(unit.x, unit.y, at.0, at.1) <= squared(SIGHT_TILES * ts))
    };
    push.enemies.retain(|id, sighting| {
        seen.contains(id)
            || (!in_sight(sighting.at) && tick.saturating_sub(sighting.tick) < ARMY_FADE_TICKS)
    });
    push.buildings
        .retain(|id, building| seen.contains(id) || !in_sight(building.at));
}

/// Remembered enemy strength within `radius_tiles` of `at`, each fighter fading with age.
fn threat_at(observation: &AiObservation, push: &PushMemory, at: (f32, f32), radius_tiles: f32) -> f32 {
    let ts = observation.map.tile_size as f32;
    push.enemies
        .values()
        .filter(|sighting| {
            let seen = point(sighting.at);
            dist2(seen.0, seen.1, at.0, at.1) <= squared(radius_tiles * ts)
        })
        .map(|sighting| faded(observation, sighting))
        .sum()
}

fn faded(observation: &AiObservation, sighting: &Sighting) -> f32 {
    let age = observation.tick.saturating_sub(sighting.tick) as f32;
    sighting.value as f32 * (1.0 - age / ARMY_FADE_TICKS as f32).max(0.0)
}

/// The nearest living enemy player.
fn enemy_player(observation: &AiObservation) -> Option<&AiPlayerSummary> {
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .min_by(|a, b| {
            let at = |tile: (u32, u32)| tile_center(tile, observation.map.tile_size);
            let (a, b) = (at(a.start_tile), at(b.start_tile));
            dist2(a.0, a.1, home.0, home.1).total_cmp(&dist2(b.0, b.1, home.0, home.1))
        })
}

/// The remembered enemy army near the enemy's route to Jeff or near the push, faded.
fn route_army(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &AiDecisionMemory,
) -> f32 {
    let ts = observation.map.tile_size as f32;
    let reach = squared(ROUTE_TILES * ts);
    let route: Vec<(f32, f32)> = map_analysis
        .and_then(|analysis| analysis.base_route_tiles(observation.player_id))
        .map(|route| {
            route
                .iter()
                .step_by(3)
                .map(|tile| tile_center((tile.x, tile.y), observation.map.tile_size))
                .collect()
        })
        .unwrap_or_default();
    let units = push_units(memory);
    let pushing: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| units.contains(&unit.id))
        .map(|unit| (unit.x, unit.y))
        .collect();
    memory
        .jeff_v1_1
        .push
        .enemies
        .values()
        .filter(|sighting| {
            let at = point(sighting.at);
            route.is_empty()
                || route
                    .iter()
                    .chain(pushing.iter())
                    .any(|near| dist2(near.0, near.1, at.0, at.1) <= reach)
        })
        .map(|sighting| faded(observation, sighting))
        .sum()
}

/// The push's weight: its units alive, and `joining` Tanks more.
pub(in crate::ai_core::decision) fn push_army(
    observation: &AiObservation,
    units: &[u32],
    joining: usize,
) -> f32 {
    let own: u32 = observation
        .owned
        .iter()
        .filter(|unit| units.contains(&unit.id) && unit.hp > 0)
        .map(|unit| unit_value(unit.kind))
        .sum();
    (own + joining as u32 * unit_value(EntityKind::Tank)) as f32
}

/// The push units: Tanks, spotter and Riflemen.
fn push_units(memory: &AiDecisionMemory) -> Vec<u32> {
    let push = &memory.containment;
    push.active_tanks
        .iter()
        .copied()
        .chain(push.active_scout)
        .chain(push.active_riflemen.iter().copied())
        .collect()
}

/// V1.1's launch gate: the push outweighs the enemy army seen near the route `ADVANCE_RATIO` times,
/// or a gathering group is there to strike. `None` for V1.0, which keeps its Tank-count gates.
pub(in crate::ai_core::decision) fn launch_ready(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    joining: usize,
) -> Option<bool> {
    if !v1_1(memory) {
        return None;
    }
    let units = push_units(memory);
    let ours = push_army(observation, &units, joining);
    let push = &mut memory.jeff_v1_1.push;
    push.anchor = 0;
    push.holding_since = None;
    push.target = None;
    push.rally = None;
    note_strike(observation, memory, ours);
    Some(
        memory.jeff_v1_1.push.strike.is_some()
            || ours >= route_army(observation, map_analysis, memory) * ADVANCE_RATIO,
    )
}

/// The enemy fighters near a Jeff base forming a group the push outweighs `STRIKE_RATIO` times:
/// its centre is struck. Groups seen earlier are held for `STRIKE_MEMORY_TICKS`.
fn note_strike(observation: &AiObservation, memory: &mut AiDecisionMemory, ours: f32) {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let near: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy.kind) && enemy.hp > 0)
        .filter(|enemy| {
            depots.iter().any(|depot| {
                dist2(enemy.x, enemy.y, depot.0, depot.1) <= squared(STRIKE_TILES * ts)
            })
        })
        .collect();
    // The largest group: the enemies within `GROUP_TILES` of its heaviest member.
    let group = near
        .iter()
        .map(|center| {
            let members: Vec<&&AiEntitySummary> = near
                .iter()
                .filter(|other| {
                    dist2(other.x, other.y, center.x, center.y) <= squared(GROUP_TILES * ts)
                })
                .collect();
            let value: u32 = members.iter().map(|member| unit_value(member.kind)).sum();
            let n = members.len() as f32;
            let at = (
                members.iter().map(|member| member.x).sum::<f32>() / n,
                members.iter().map(|member| member.y).sum::<f32>() / n,
            );
            (value, at)
        })
        .max_by(|a, b| a.0.cmp(&b.0).then(a.1 .0.total_cmp(&b.1 .0)));
    let push = &mut memory.jeff_v1_1.push;
    match group {
        Some((value, at)) if ours >= value as f32 * STRIKE_RATIO => {
            push.strike = Some((stored(at), tick));
        }
        Some(_) => push.strike = None,
        None => {
            push.strike = push
                .strike
                .filter(|(_, seen)| tick.saturating_sub(*seen) < STRIKE_MEMORY_TICKS);
        }
    }
}

/// A place the push might raid: where, and what is there.
struct Candidate {
    at: (f32, f32),
    worth: f32,
}

/// Every enemy base seen, the enemy main, and the resource fields nearer the enemy start that have
/// not been seen empty.
fn candidates(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    push: &mut PushMemory,
    enemy_main: (f32, f32),
) -> Vec<Candidate> {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let near = |a: (f32, f32), b: (f32, f32), tiles: f32| {
        dist2(a.0, a.1, b.0, b.1) <= squared(tiles * ts)
    };
    let buildings: Vec<(EntityKind, (f32, f32))> = push
        .buildings
        .values()
        .map(|building| (building.kind, point(building.at)))
        .collect();
    let worth_near = |at: (f32, f32)| -> f32 {
        buildings
            .iter()
            .filter(|(_, building)| near(*building, at, BASE_TILES))
            .map(|(kind, _)| unit_value(*kind) as f32)
            .sum()
    };
    let depots: Vec<(f32, f32)> = buildings
        .iter()
        .filter(|(kind, _)| *kind == EntityKind::ResourceDepot)
        .map(|(_, at)| *at)
        .collect();
    let mut found: Vec<Candidate> = depots
        .iter()
        .map(|at| Candidate {
            at: *at,
            worth: worth_near(*at),
        })
        .collect();
    // Extractors seen without their Depot.
    for (kind, at) in &buildings {
        if matches!(kind, EntityKind::SteelMine | EntityKind::PumpJack)
            && !found.iter().any(|base| near(base.at, *at, BASE_TILES))
        {
            found.push(Candidate {
                at: *at,
                worth: worth_near(*at),
            });
        }
    }
    if !found.iter().any(|base| near(base.at, enemy_main, BASE_TILES)) {
        found.push(Candidate {
            at: enemy_main,
            worth: MAIN_WORTH,
        });
    }
    // Resource fields that may hold a base: seen with nothing there, they are passed over a while.
    let enemy_id = enemy_player(observation).map(|player| player.id);
    push.empty.retain(|_, until| tick < *until);
    for cluster in analysis.resource_clusters() {
        if cluster.nearest_start_player_id != enemy_id || push.empty.contains_key(&cluster.id) {
            continue;
        }
        let at = tile_center(
            (cluster.center_tile.x, cluster.center_tile.y),
            observation.map.tile_size,
        );
        if found.iter().any(|base| near(base.at, at, BASE_TILES))
            || observation
                .owned
                .iter()
                .any(|unit| unit.kind.is_building() && near((unit.x, unit.y), at, OWN_FIELD_TILES))
        {
            continue;
        }
        if observation
            .owned
            .iter()
            .any(|unit| near((unit.x, unit.y), at, SIGHT_TILES))
        {
            push.empty.insert(cluster.id, tick + EMPTY_TICKS);
            continue;
        }
        found.push(Candidate {
            at,
            worth: SUSPECTED_WORTH,
        });
    }
    found
}

/// The base the launched push raids, and the side it comes in from (see the module docs).
#[allow(clippy::too_many_arguments)]
fn raid_target(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    center: (f32, f32),
    enemy_main: (f32, f32),
    ours: f32,
    standoff_tiles: f32,
) -> Option<RaidTarget> {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let analysis = map_analysis?;
    let push = &mut memory.jeff_v1_1.push;
    push.hot.retain(|_, until| tick < *until);
    if push.target.is_some() && tick.saturating_sub(push.retargeted) < RETARGET_TICKS {
        return push.target;
    }
    push.retargeted = tick;
    let found = candidates(observation, analysis, push, enemy_main);
    let push = &memory.jeff_v1_1.push;
    let mut scored: Vec<RaidTarget> = Vec::new();
    for candidate in found {
        let key = stored(candidate.at);
        if push.hot.keys().any(|hot| {
            dist2(point(*hot).0, point(*hot).1, candidate.at.0, candidate.at.1)
                <= squared(BASE_TILES * ts)
        }) {
            continue;
        }
        let defence = threat_at(observation, push, candidate.at, DEFENCE_TILES);
        if ours < defence * RAID_RATIO {
            continue;
        }
        let around = |at: (f32, f32)| {
            if dist2(at.0, at.1, candidate.at.0, candidate.at.1) <= squared(DEFENCE_TILES * ts) {
                0.0
            } else {
                threat_at(observation, push, at, THREAT_TILES)
            }
        };
        let direct = approach::route_threat(analysis, center, candidate.at, &around);
        let (side, path) = if direct > ours * SIDESTEP_SHARE {
            match approach::safest_side(
                analysis,
                observation,
                center,
                enemy_main,
                candidate.at,
                standoff_tiles,
                &around,
            ) {
                Some((side, risk)) if risk <= direct * 0.5 => (Some(side), risk),
                _ if direct > ours * BLOCKED_SHARE => continue,
                _ => (None, direct),
            }
        } else {
            (None, direct)
        };
        let travel = dist2(center.0, center.1, candidate.at.0, candidate.at.1).sqrt() / ts;
        let score = candidate.worth / (1.0 + defence + 0.5 * path) / (1.0 + travel / TRAVEL_TILES);
        scored.push(RaidTarget {
            at: key,
            side,
            score: (score * 1000.0) as i64,
        });
    }
    let best = scored.iter().max_by_key(|target| target.score).copied();
    let current = push.target.and_then(|current| {
        scored
            .iter()
            .find(|target| target.at == current.at)
            .copied()
    });
    let chosen = match (current, best) {
        (Some(current), Some(best))
            if (best.score as f32) < current.score as f32 * SWITCH_RATIO =>
        {
            Some(current)
        }
        (_, best) => best,
    };
    memory.jeff_v1_1.push.target = chosen;
    chosen
}

/// Where V1.1's launched push makes for: a gathering group it strikes, else the base it raids,
/// else the anchor it holds along the enemy's route, else `fallback` (see the module docs). Sets
/// the side the push comes in from. `None` for V1.0.
#[allow(clippy::too_many_arguments)]
pub(in crate::ai_core::decision) fn objective(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    own_base: (f32, f32),
    enemy_main: (f32, f32),
    fallback: (f32, f32),
    standoff_tiles: f32,
) -> Option<(f32, f32)> {
    if !v1_1(memory) {
        return None;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    join_reinforcements(observation, memory);
    let units = push_units(memory);
    let ours = push_army(observation, &units, 0);
    note_strike(observation, memory, ours);
    memory.jeff_v1_1.push.short = memory.jeff_v1_1.push.strike.is_some();
    if let Some((at, _)) = memory.jeff_v1_1.push.strike {
        memory.approach.force(None);
        return Some(point(at));
    }
    let tanks: Vec<u32> = memory.containment.active_tanks.iter().copied().collect();
    let center = super::super::frontal::group_center(observation, &tanks)?;
    let anchors = anchors(observation, map_analysis, own_base, fallback);
    let target = |anchor: usize| anchors.get(anchor).copied().unwrap_or(fallback);
    if let Some(raid) = raid_target(
        observation,
        map_analysis,
        memory,
        center,
        enemy_main,
        ours,
        standoff_tiles,
    ) {
        memory.approach.force(raid.side);
        // Reinforcements gather at the last anchor behind the push.
        let behind = (0..anchors.len())
            .rev()
            .find(|anchor| {
                let at = target(*anchor);
                dist2(at.0, at.1, own_base.0, own_base.1)
                    <= dist2(center.0, center.1, own_base.0, own_base.1)
            })
            .map(target);
        memory.jeff_v1_1.push.rally = behind.map(stored);
        return Some(point(raid.at));
    }
    memory.approach.force(None);
    let front = front_value(observation, &units);
    let ahead = ours >= route_army(observation, map_analysis, memory) * ADVANCE_RATIO && ours >= front;
    let push = &mut memory.jeff_v1_1.push;
    let at = target(push.anchor);
    let reached = dist2(center.0, center.1, at.0, at.1) <= squared(ANCHOR_REACHED_TILES * ts);
    if reached {
        push.holding_since.get_or_insert(tick);
    } else {
        push.holding_since = None;
    }
    let step_due = tick.saturating_sub(push.last_step) >= STEP_TICKS;
    let held = push
        .holding_since
        .is_some_and(|since| tick.saturating_sub(since) >= ANCHOR_HOLD_TICKS);
    if step_due && front > ours && push.anchor > 0 {
        push.anchor -= 1;
        push.last_step = tick;
        push.holding_since = None;
    } else if step_due && ahead && held && push.anchor < anchors.len() {
        push.anchor += 1;
        push.last_step = tick;
        push.holding_since = None;
    }
    push.rally = Some(stored(target(push.anchor)));
    push.short = push.anchor < anchors.len();
    Some(target(push.anchor))
}

/// The value of the visible enemy fighters within `FRONT_TILES` of the push units.
fn front_value(observation: &AiObservation, units: &[u32]) -> f32 {
    let ts = observation.map.tile_size as f32;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy.kind) && enemy.hp > 0)
        .filter(|enemy| {
            observation.owned.iter().any(|unit| {
                units.contains(&unit.id)
                    && dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(FRONT_TILES * ts)
            })
        })
        .map(|enemy| unit_value(enemy.kind) as f32)
        .sum()
}

/// Free Tanks that reached the launched push's rally point join it, with two Riflemen each from
/// those that came along.
fn join_reinforcements(observation: &AiObservation, memory: &mut AiDecisionMemory) {
    let Some(rally) = memory.jeff_v1_1.push.rally.map(point) else {
        return;
    };
    let ts = observation.map.tile_size as f32;
    let reserved = memory.squad_reserved();
    let free = |unit: &&AiEntitySummary, kind: EntityKind| {
        unit.kind == kind
            && unit.is_complete
            && unit.hp > 0
            && dist2(unit.x, unit.y, rally.0, rally.1) <= squared(JOIN_TILES * ts)
            && Some(unit.id) != memory.home_defensive_tank
            && memory.containment.active_scout != Some(unit.id)
            && !memory.containment.active_tanks.contains(&unit.id)
            && !memory.containment.active_riflemen.contains(&unit.id)
            && !memory.containment.stuck_tanks.contains_key(&unit.id)
            && !memory.later_bases.guards.contains(&unit.id)
            && !memory.expansion_security.riflemen.contains(&unit.id)
            && !memory.jeff_v1_1.strike.party.contains(&unit.id)
            && !reserved.contains(&unit.id)
    };
    let tanks: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| free(unit, EntityKind::Tank))
        .map(|unit| unit.id)
        .collect();
    let riflemen: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| free(unit, EntityKind::Rifleman))
        .map(|unit| unit.id)
        .take(tanks.len() * super::RIFLES_PER_TANK)
        .collect();
    memory.containment.active_tanks.extend(tanks);
    memory.containment.active_riflemen.extend(riflemen);
}

/// Where V1.1's new Tanks rally while its push is out: the push's rally point.
pub(in crate::ai_core::decision) fn reinforcement_rally(
    memory: &AiDecisionMemory,
) -> Option<(f32, f32)> {
    let launched = memory.containment.wave_launched && !memory.containment.recovery_active;
    memory
        .jeff_v1_1
        .push
        .rally
        .filter(|_| launched && v1_1(memory))
        .map(point)
}

/// The fewest Tanks that march in legs: V1.1 takes a side lane with two.
pub(in crate::ai_core::decision) fn legs_min_tanks(
    memory: &AiDecisionMemory,
    default: usize,
) -> usize {
    if v1_1(memory) && memory.jeff_v1_1.push.target.is_some_and(|target| target.side.is_some()) {
        2
    } else {
        default
    }
}

/// Whether V1.1's push holds short of its target, at an anchor or a strike, where the march
/// watchdog must not drive it on.
pub(in crate::ai_core::decision) fn holding_short(memory: &AiDecisionMemory) -> bool {
    v1_1(memory) && memory.jeff_v1_1.push.short
}

/// The anchors along the enemy's route toward `last`, nearest Jeff first, each moved onto a map
/// choke near it.
fn anchors(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    own_base: (f32, f32),
    last: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let enemy = enemy_player(observation);
    let route = enemy.and_then(|enemy| {
        map_analysis.and_then(|analysis| analysis.base_route_tiles(enemy.id))
    });
    let chokes: Vec<(f32, f32)> = map_analysis
        .map(|analysis| {
            analysis
                .chokes()
                .iter()
                .map(|choke| {
                    tile_center(
                        (choke.center_tile.x, choke.center_tile.y),
                        observation.map.tile_size,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let limit = dist2(last.0, last.1, own_base.0, own_base.1);
    ANCHOR_SHARES
        .iter()
        .map(|share| match route.filter(|route| !route.is_empty()) {
            Some(route) => {
                let index = ((route.len() - 1) as f32 * share).round() as usize;
                tile_center((route[index].x, route[index].y), observation.map.tile_size)
            }
            None => (
                own_base.0 + (last.0 - own_base.0) * share,
                own_base.1 + (last.1 - own_base.1) * share,
            ),
        })
        .map(|at| {
            chokes
                .iter()
                .filter(|choke| {
                    dist2(choke.0, choke.1, at.0, at.1) <= squared(CHOKE_SNAP_TILES * ts)
                })
                .min_by(|a, b| {
                    dist2(a.0, a.1, at.0, at.1).total_cmp(&dist2(b.0, b.1, at.0, at.1))
                })
                .copied()
                .unwrap_or(at)
        })
        .filter(|at| dist2(at.0, at.1, own_base.0, own_base.1) < limit)
        .collect()
}

/// V1.1's spotter when no Scout Car is ready: the free Rifleman nearest `rally`.
pub(in crate::ai_core::decision) fn spotter(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    rally: (f32, f32),
) -> Option<u32> {
    if !v1_1(memory) {
        return None;
    }
    let reserved = memory.squad_reserved();
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !memory.containment.active_riflemen.contains(&unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.later_bases.guards.contains(&unit.id)
                && !reserved.contains(&unit.id)
        })
        .min_by(|a, b| {
            dist2(a.x, a.y, rally.0, rally.1)
                .total_cmp(&dist2(b.x, b.y, rally.0, rally.1))
                .then(a.id.cmp(&b.id))
        })
        .map(|unit| unit.id)
}

/// V1.1's field check in place of the Tank count: whether the push falls back home. Outweighed on
/// a raid, it drops the target as too hot and turns elsewhere, falling back only when routed;
/// outweighed on the rolling line short of its first anchor, it falls back; further out it steps
/// back an anchor instead (`objective`). `None` for V1.0.
pub(in crate::ai_core::decision) fn outweighed(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) -> Option<bool> {
    if !v1_1(memory) {
        return None;
    }
    let units = push_units(memory);
    let ours = push_army(observation, &units, 0);
    let front = front_value(observation, &units);
    if front <= ours {
        return Some(false);
    }
    let tick = observation.tick;
    let push = &mut memory.jeff_v1_1.push;
    if let Some(target) = push.target.take() {
        push.hot.insert(target.at, tick + HOT_TICKS);
        push.retargeted = 0;
        return Some(front > ours * ROUT_RATIO);
    }
    Some(push.anchor == 0)
}
