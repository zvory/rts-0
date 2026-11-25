//! Jeff's AI V1.1 raids the enemy's outer bases with the Riflemen idle in its main (`rifleraid`).
//! In the bad games against Pressure AI (all-maps bench, nog3) Pressure ran five bases by 6:00
//! against Jeff's three while Jeff's army led 1.3-2.5 times: 29-42 Meth Riflemen stood in the main's
//! pocket while Pressure's 10-16 Panzerfausts (Riflemen with one anti-Tank shot) and at most one
//! Tank held the map, and the push and the Tank raid waited for Tanks (the push left at 7:30-9:00).
//! Jeff killed 8 enemy Depots before 9:00 in 34 bad games, 94 in 73 good ones.
//!
//! - **Party**: once Meth is in, the healthy Riflemen the Rifle split put past what each post keeps
//!   (`stations::place_riflemen`): the main's floor in its pocket, `BASE_KEEP` on each line past the
//!   natural, which took every Rifleman left over (60 at one Doppelganger third by 10:00);
//!   `MIN_PARTY` or more, at most `MAX_PARTY`.
//! - **Target**: an enemy Depot seen and not since seen gone, the enemy main's left out
//!   (`push::known_depots`), not dropped lately: the least defended for the way, among those whose
//!   remembered defence (`DEFENCE_TILES`) and the enemies remembered along the straight way there
//!   (`ROUTE_TILES`) the party outweighs `LAUNCH_RATIO` times, counted in Riflemen (`weight`).
//! - **Out**: attack-moving at the Depot (Meth Riflemen fire on the move).
//! - **Hit**: within `ARRIVE_TILES`, every Rifleman on the Depot while no enemy fighter stands near it
//!   (180 HP, armoured: a rifle round does 1, so 20 Riflemen take ~9 s), then the extractors,
//!   foundations and Engineers within `SITE_TILES`; then on to the next target that passes, or home.
//! - **Run**: enemies in sight near the party worth `RUN_RATIO` of it, or the party down to
//!   `LOSS_SHARE` of its start: home, and the target is left alone for `HOT_TICKS`.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared, tile_center};
use rts_rules::math::GameplayMath as _;

const MIN_PARTY: usize = 12;
const MAX_PARTY: usize = 30;
/// Each base line past the natural keeps this many of its Riflemen (`rrkeep`).
const BASE_KEEP: usize = 6;
/// A Rifleman sets out with this share of its health or more.
const SET_OUT_HP: f32 = 0.8;
const LAUNCH_RATIO: f32 = 2.0;
const DEFENCE_TILES: f32 = 16.0;
const ROUTE_TILES: f32 = 10.0;
/// The straight way is sampled this often.
const ROUTE_STEP_TILES: f32 = 6.0;
/// A remembered enemy counts fully when just seen and fades to nothing over this long (the push's).
const FADE_TICKS: u32 = config::TICK_HZ * 90;
const RUN_RATIO: f32 = 1.0;
const DANGER_TILES: f32 = 14.0;
const LOSS_SHARE: f32 = 0.5;
const ARRIVE_TILES: f32 = 9.0;
/// A Depot this close to the target point is the target's.
const DEPOT_TILES: f32 = 10.0;
/// A known Depot is worth this many likely base sites.
const KNOWN_WORTH: f32 = 3.0;
/// `rrsus`: a field seen empty is passed over this long; fields this close to a Jeff building are
/// Jeff's.
const EMPTY_TICKS: u32 = config::TICK_HZ * 120;
const OWN_FIELD_TILES: f32 = 15.0;
const SITE_TILES: f32 = 12.0;
/// Enemy fighters this close to the target are fought before the Depot.
const GUARD_TILES: f32 = 14.0;
/// The target point is in sight with a party Rifleman this close.
const SIGHT_TILES: f32 = 7.0;
const OUT_TICKS: u32 = config::TICK_HZ * 120;
const HIT_TICKS: u32 = config::TICK_HZ * 45;
const HOT_TICKS: u32 = config::TICK_HZ * 120;
const REST_TICKS: u32 = config::TICK_HZ * 30;
const REORDER_TICKS: u32 = config::TICK_HZ * 2;
/// Home once the party's centre is this close to a Jeff Depot.
const HOME_TILES: f32 = 14.0;
/// A Rifleman with an enemy fighter this close is fighting and keeps its own target.
const FIGHT_TILES: f32 = 7.0;
/// Travel to a target costs its score one share per this many tiles.
const TRAVEL_TILES: f32 = 60.0;
/// Depots this close to the enemy main are left alone (`rrmainx`): its natural, next to its
/// production, turned back or wiped the first raids on Doppelganger and Afrika.
const MAIN_CLEAR_TILES: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Out,
    Hit,
    Home,
}

/// V1.1's Rifle raid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct RifleRaidMemory {
    party: BTreeSet<u32>,
    phase: Option<Phase>,
    target: Option<(i32, i32)>,
    /// The party's size when it set out, and when the current phase began.
    start: usize,
    since: u32,
    ordered: u32,
    /// When the launch was last weighed.
    checked: u32,
    /// `rrsus`: resource fields seen with no enemy building, by cluster id, until when.
    empty: BTreeMap<u32, u32>,
    hot: BTreeMap<(i32, i32), u32>,
    rest_until: u32,
    /// Whether the target's Depot was seen standing since the party arrived.
    depot_seen: bool,
    /// The party's weight and the danger near it at the last decision, for the trace.
    weighed: (u32, u32),
    /// Raids sent and Depots killed so far, and why the last raid ended.
    sent: u32,
    kills: u32,
    why: &'static str,
    /// Who holds Jeff's Riflemen, and the last launch look (diagnostics).
    who: String,
    look: String,
}

impl RifleRaidMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.party.contains(&id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.party.iter().copied()
    }

    /// The decision-log line.
    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        format!(
            "v1_1_rraid phase={:?} n={}/{} target={:?} w={}/{} sent={} kills={} why={} hot={} who={} look={}",
            self.phase,
            self.party.len(),
            self.start,
            self.target,
            self.weighed.0,
            self.weighed.1,
            self.sent,
            self.kills,
            self.why,
            self.hot.len(),
            self.who,
            self.look,
        )
    }
}

/// Whether Jeff's AI V1.1 raids with its idle Riflemen.
pub(in crate::ai_core::decision) fn active(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("rifleraid")
}

fn param(name: &str, default: f32) -> f32 {
    crate::ai_core::profiles::jeff_v1_1_param(name).unwrap_or(default)
}

fn point((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32, y as f32)
}

fn stored((x, y): (f32, f32)) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

/// A fighter's weight against Meth Riflemen, in Riflemen.
fn weight(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::Rifleman | EntityKind::Panzerfaust => 1.0,
        EntityKind::MachineGunner => 2.5,
        EntityKind::ScoutCar => 3.0,
        EntityKind::Tank => 6.0,
        other => super::push::unit_value(other) as f32 / 35.0,
    }
}

fn is_fighter(kind: EntityKind) -> bool {
    kind.is_unit() && !matches!(kind, EntityKind::Worker | EntityKind::Golem)
}

fn hp_share(unit: &AiEntitySummary) -> f32 {
    let max = config::unit_stats(unit.kind).map_or(1, |stats| stats.hp.max(1));
    unit.hp as f32 / max as f32
}

fn center(units: &[&AiEntitySummary]) -> (f32, f32) {
    let n = units.len().max(1) as f32;
    (
        units.iter().map(|unit| unit.x).sum::<f32>() / n,
        units.iter().map(|unit| unit.y).sum::<f32>() / n,
    )
}

/// The remembered enemy weight within `tiles` of any of `points`, faded with age.
fn remembered_near(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    points: &[(f32, f32)],
    tiles: f32,
) -> f32 {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    memory
        .jeff_v1_1
        .push
        .remembered()
        .filter(|(_, _, _, at, _)| {
            points
                .iter()
                .any(|p| dist2(p.0, p.1, at.0, at.1) <= squared(tiles * ts))
        })
        .map(|(_, kind, _, _, seen)| {
            let age = tick.saturating_sub(seen) as f32;
            weight(kind) * (1.0 - age / FADE_TICKS as f32).max(0.0)
        })
        .sum()
}

/// Points every `ROUTE_STEP_TILES` along the straight way from `from` to `to`.
fn way(from: (f32, f32), to: (f32, f32), ts: f32) -> Vec<(f32, f32)> {
    let length = dist2(from.0, from.1, to.0, to.1).det_sqrt();
    let steps = (length / (ROUTE_STEP_TILES * ts)).ceil().max(1.0) as usize;
    (0..=steps)
        .map(|step| {
            let share = step as f32 / steps as f32;
            (
                from.0 + (to.0 - from.0) * share,
                from.1 + (to.1 - from.1) * share,
            )
        })
        .collect()
}

/// The targets: the enemy Depots seen (worth `KNOWN_WORTH`), and under `rrsus` also the enemy's
/// extractors seen without their Depot and the resource fields nearer the enemy start not seen
/// empty (`push::raid_candidates`, worth 1): in the bad games Jeff had seen none of Pressure's
/// bases past its natural before 8:00.
fn targets(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    enemy_main: (f32, f32),
) -> Vec<((f32, f32), f32)> {
    let ts = observation.map.tile_size as f32;
    let known: Vec<((f32, f32), f32)> = super::push::known_depots(observation, memory, enemy_main)
        .into_iter()
        .map(|at| (at, KNOWN_WORTH))
        .collect();
    let Some(analysis) = analysis.filter(|_| !super::off("rrsus")) else {
        return known;
    };
    // The raid's own list (the push's would be written to as it looks): fields nearer the enemy
    // start, clear of Jeff's buildings and of the enemy main, not seen empty in `EMPTY_TICKS`.
    let tick = observation.tick;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy_id = observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .min_by(|a, b| {
            let at = |tile: (u32, u32)| tile_center(tile, observation.map.tile_size);
            let (a, b) = (at(a.start_tile), at(b.start_tile));
            dist2(a.0, a.1, home.0, home.1).total_cmp(&dist2(b.0, b.1, home.0, home.1))
        })
        .map(|player| player.id);
    let near = |a: (f32, f32), b: (f32, f32), tiles: f32| dist2(a.0, a.1, b.0, b.1) <= squared(tiles * ts);
    let state = &mut memory.jeff_v1_1.rifle_raid;
    state.empty.retain(|_, until| tick < *until);
    let mut all = known;
    for cluster in analysis.resource_clusters() {
        if cluster.nearest_start_player_id != enemy_id || state.empty.contains_key(&cluster.id) {
            continue;
        }
        let at = tile_center(
            (cluster.center_tile.x, cluster.center_tile.y),
            observation.map.tile_size,
        );
        if near(at, enemy_main, DEPOT_TILES)
            || all.iter().any(|(seen, _)| near(*seen, at, DEPOT_TILES))
            || observation
                .owned
                .iter()
                .any(|unit| unit.kind.is_building() && near((unit.x, unit.y), at, OWN_FIELD_TILES))
        {
            continue;
        }
        let enemy_building = observation
            .visible_enemies
            .iter()
            .any(|enemy| enemy.hp > 0 && enemy.kind.is_building() && near((enemy.x, enemy.y), at, DEPOT_TILES));
        if !enemy_building && observation.owned.iter().any(|unit| near((unit.x, unit.y), at, SIGHT_TILES)) {
            state.empty.insert(cluster.id, tick + EMPTY_TICKS);
            continue;
        }
        all.push((at, 1.0));
    }
    all
}

/// The target the party of `strength` takes from `from`, if any passes.
fn pick_target(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    from: (f32, f32),
    strength: f32,
) -> Option<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let enemy_main = super::push::enemy_main(observation)?;
    let ratio = param("rrratio", LAUNCH_RATIO);
    let main_clear = param("rrmainx", MAIN_CLEAR_TILES);
    let found = targets(observation, analysis, memory, enemy_main);
    let hot = &memory.jeff_v1_1.rifle_raid.hot;
    let memory: &AiDecisionMemory = memory;
    found
        .into_iter()
        .filter(|(at, _)| hot.get(&stored(*at)).map_or(true, |until| tick >= *until))
        .filter(|(at, _)| dist2(at.0, at.1, enemy_main.0, enemy_main.1) > squared(main_clear * ts))
        .filter_map(|(at, worth)| {
            let defence = remembered_near(observation, memory, &[at], DEFENCE_TILES);
            let route = remembered_near(observation, memory, &way(from, at, ts), ROUTE_TILES);
            let threat = defence.max(route);
            (strength >= ratio * threat).then(|| {
                let travel = dist2(from.0, from.1, at.0, at.1).det_sqrt() / (TRAVEL_TILES * ts);
                (at, worth / ((1.0 + threat) * (1.0 + travel)))
            })
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0 .0.total_cmp(&a.0 .0)))
        .map(|(at, _)| at)
}

/// Sends, steers and brings home V1.1's Rifle raid (see the module docs). `held`: units other
/// systems keep this decision.
pub(in crate::ai_core::decision) fn raid(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    held: &BTreeSet<u32>,
) {
    if !active(memory) {
        memory.jeff_v1_1.rifle_raid = RifleRaidMemory::default();
        return;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.hp > 0)
        .map(|unit| (unit.id, unit))
        .collect();
    // Diagnostics: who holds the Riflemen.
    {
        let mut counts = [0usize; 11];
        for unit in observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        {
            let id = unit.id;
            let slot = if memory.jeff_v1_1.rifle_raid.holds(id) {
                0
            } else if memory.jeff_v1_1.stations.rifle_post(id).is_some() {
                1
            } else if memory.expansion_security.riflemen.contains(&id) {
                2
            } else if memory.later_bases.guards.contains(&id) {
                3
            } else if memory.jeff_v1_1.natural.holds(id) {
                4
            } else if memory.jeff_v1_1.base_lines.posted(id) {
                5
            } else if memory.jeff_v1_1.natclear.holds(id) || memory.jeff_v1_1.breakout.holds(id) {
                6
            } else if memory.jeff_v1_1.front_line.holds(id) || memory.jeff_v1_1.spotters.holds(id) {
                7
            } else if memory.rush_reserved(id) {
                8
            } else if memory.containment.active_riflemen.contains(&id) {
                9
            } else {
                10
            };
            counts[slot] += 1;
        }
        memory.jeff_v1_1.rifle_raid.who = format!(
            "rr{} st{} es{} lg{} nat{} bl{} nc{} fl{} ms{} ca{} x{}",
            counts[0],
            counts[1],
            counts[2],
            counts[3],
            counts[4],
            counts[5],
            counts[6],
            counts[7],
            counts[8],
            counts[9],
            counts[10]
        );
    }
    {
        let state = &mut memory.jeff_v1_1.rifle_raid;
        state.hot.retain(|_, until| tick < *until);
        state.party.retain(|id| by_id.contains_key(id));
        if state.party.is_empty() && state.phase.is_some() {
            state.phase = None;
            state.why = "wiped";
            state.rest_until = tick + REST_TICKS;
        }
    }
    if memory.jeff_v1_1.rifle_raid.phase.is_none() {
        launch(observation, analysis, memory, held, &by_id);
        if memory.jeff_v1_1.rifle_raid.phase.is_none() {
            return;
        }
    }
    let party: Vec<&AiEntitySummary> = memory
        .jeff_v1_1
        .rifle_raid
        .party
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .collect();
    let mid = center(&party);
    let strength: f32 = party.iter().map(|unit| hp_share(unit)).sum();
    let danger: f32 = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && is_fighter(enemy.kind))
        .filter(|enemy| {
            party
                .iter()
                .any(|unit| dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(DANGER_TILES * ts))
        })
        .map(|enemy| weight(enemy.kind) * hp_share(enemy))
        .sum();
    let ids: Vec<u32> = party.iter().map(|unit| unit.id).collect();
    let fighting = |unit: &AiEntitySummary| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && is_fighter(enemy.kind)
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(FIGHT_TILES * ts)
        })
    };
    let state = &mut memory.jeff_v1_1.rifle_raid;
    state.weighed = (strength.round() as u32, danger.round() as u32);
    let phase = state.phase.unwrap_or(Phase::Home);
    let target = state.target.map(point);
    // Outmatched, worn down, or too long: home, and the target is left alone a while.
    if phase != Phase::Home {
        let why = if danger > strength * param("rrrun", RUN_RATIO) {
            Some("outmatched")
        } else if (party.len() as f32) < state.start as f32 * LOSS_SHARE {
            Some("losses")
        } else if phase == Phase::Out && tick.saturating_sub(state.since) > OUT_TICKS {
            Some("slow")
        } else if phase == Phase::Hit && tick.saturating_sub(state.since) > HIT_TICKS {
            Some("time")
        } else {
            None
        };
        if let Some(why) = why {
            if let Some(at) = state.target {
                state.hot.insert(at, tick + HOT_TICKS);
            }
            state.phase = Some(Phase::Home);
            state.why = why;
            state.since = tick;
            state.ordered = 0;
        }
    }
    let phase = state.phase.unwrap_or(Phase::Home);
    match (phase, target) {
        (Phase::Out, Some(at)) => {
            if dist2(mid.0, mid.1, at.0, at.1) <= squared(ARRIVE_TILES * ts) {
                state.phase = Some(Phase::Hit);
                state.since = tick;
                state.ordered = 0;
                state.depot_seen = false;
            } else if tick.saturating_sub(state.ordered) >= REORDER_TICKS {
                let sent: Vec<u32> = party
                    .iter()
                    .filter(|unit| !fighting(unit))
                    .map(|unit| unit.id)
                    .collect();
                if let Some(units) = actions::attack_move_units(actions, sent, at.0, at.1) {
                    intents.push(AiIntent::Attack { units });
                }
                state.ordered = tick;
            }
        }
        (Phase::Hit, Some(at)) => hit(actions, observation, analysis, memory, intents, &party, at),
        _ => {
            // Home: to the nearest Jeff Depot, moving, not fighting the way back.
            let depot = observation
                .owned
                .iter()
                .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
                .map(|unit| (unit.x, unit.y))
                .min_by(|a, b| {
                    dist2(a.0, a.1, mid.0, mid.1).total_cmp(&dist2(b.0, b.1, mid.0, mid.1))
                })
                .unwrap_or_else(|| {
                    tile_center(observation.own_start_tile, observation.map.tile_size)
                });
            let state = &mut memory.jeff_v1_1.rifle_raid;
            if ids.is_empty() || dist2(mid.0, mid.1, depot.0, depot.1) <= squared(HOME_TILES * ts) {
                state.party.clear();
                state.phase = None;
                state.target = None;
                state.rest_until = tick + REST_TICKS;
                if !ids.is_empty() {
                    intents.push(AiIntent::Assemble { units: ids });
                }
            } else if tick.saturating_sub(state.ordered) >= REORDER_TICKS * 2 {
                if let Some(units) = actions::move_units(actions, ids, depot.0, depot.1) {
                    intents.push(AiIntent::Assemble { units });
                }
                state.ordered = tick;
            }
        }
    }
}

/// The party at its target: the guards, then the Depot, then the rest of the base; cleared, on to
/// the next target or home.
fn hit(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    party: &[&AiEntitySummary],
    at: (f32, f32),
) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let near = |x: f32, y: f32, tiles: f32| dist2(x, y, at.0, at.1) <= squared(tiles * ts);
    let ids: Vec<u32> = party.iter().map(|unit| unit.id).collect();
    let guards = observation
        .visible_enemies
        .iter()
        .any(|enemy| enemy.hp > 0 && is_fighter(enemy.kind) && near(enemy.x, enemy.y, GUARD_TILES));
    let depot = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind == EntityKind::ResourceDepot)
        .find(|enemy| near(enemy.x, enemy.y, DEPOT_TILES));
    let state = &mut memory.jeff_v1_1.rifle_raid;
    state.depot_seen |= depot.is_some();
    let reorder = tick.saturating_sub(state.ordered) >= REORDER_TICKS;
    if guards {
        if reorder {
            if let Some(units) = actions::attack_move_units(actions, ids, at.0, at.1) {
                intents.push(AiIntent::Attack { units });
            }
            state.ordered = tick;
        }
        return;
    }
    if let Some(depot) = depot {
        if reorder {
            if let Some(units) = actions::attack_units(actions, ids, depot.id) {
                intents.push(AiIntent::Attack { units });
            }
            state.ordered = tick;
        }
        return;
    }
    let in_sight = party
        .iter()
        .any(|unit| near(unit.x, unit.y, SIGHT_TILES));
    if !in_sight {
        if reorder {
            if let Some(units) = actions::attack_move_units(actions, ids, at.0, at.1) {
                intents.push(AiIntent::Attack { units });
            }
            state.ordered = tick;
        }
        return;
    }
    // The Depot is down or gone: the extractors, foundations and Engineers left.
    if state.depot_seen {
        state.kills += 1;
        state.depot_seen = false;
        super::deny::note_kill(memory, at, tick);
    }
    let mid = center(party);
    let rest = observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            enemy.hp > 0
                && (enemy.kind.is_building() || enemy.kind == EntityKind::Worker)
                && near(enemy.x, enemy.y, SITE_TILES)
        })
        .min_by(|a, b| {
            dist2(a.x, a.y, mid.0, mid.1)
                .total_cmp(&dist2(b.x, b.y, mid.0, mid.1))
                .then(a.id.cmp(&b.id))
        });
    let state = &mut memory.jeff_v1_1.rifle_raid;
    if let Some(rest) = rest {
        if reorder {
            if let Some(units) = actions::attack_units(actions, ids, rest.id) {
                intents.push(AiIntent::Attack { units });
            }
            state.ordered = tick;
        }
        return;
    }
    // Cleared: on to the next target that passes, or home.
    let cleared = state.target;
    if let Some(cleared) = cleared {
        state.hot.insert(cleared, tick + HOT_TICKS);
    }
    let strength: f32 = party.iter().map(|unit| hp_share(unit)).sum();
    let next = (!super::off("rrchain"))
        .then(|| pick_target(observation, analysis, memory, mid, strength))
        .flatten();
    let state = &mut memory.jeff_v1_1.rifle_raid;
    state.since = tick;
    state.ordered = 0;
    if let Some(next) = next {
        state.target = Some(stored(next));
        state.phase = Some(Phase::Out);
    } else {
        state.phase = Some(Phase::Home);
        state.why = "cleared";
    }
}

/// Sets out when the main's spare Riflemen outweigh a known enemy outer base and the way there.
fn launch(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    held: &BTreeSet<u32>,
    by_id: &BTreeMap<u32, &AiEntitySummary>,
) {
    let tick = observation.tick;
    if tick < memory.jeff_v1_1.rifle_raid.rest_until
        || tick.saturating_sub(memory.jeff_v1_1.rifle_raid.checked) < REORDER_TICKS
        || !observation
            .upgrades
            .contains(&UpgradeKind::Methamphetamines)
    {
        return;
    }
    memory.jeff_v1_1.rifle_raid.checked = tick;
    let keep_base = param("rrkeep", BASE_KEEP as f32) as usize;
    let mut pocket: Vec<&AiEntitySummary> = memory
        .jeff_v1_1
        .stations
        .rifle_surplus(keep_base)
        .into_iter()
        .filter(|id| !held.contains(id))
        .filter_map(|id| by_id.get(&id).copied())
        .filter(|unit| hp_share(unit) >= SET_OUT_HP)
        .collect();
    let min_party = param("rrmin", MIN_PARTY as f32) as usize;
    pocket.truncate(param("rrmax", MAX_PARTY as f32) as usize);
    let from = center(&pocket);
    let strength: f32 = pocket.iter().map(|unit| hp_share(unit)).sum();
    memory.jeff_v1_1.rifle_raid.look = look(observation, analysis, memory, from, pocket.len());
    if pocket.len() < min_party {
        return;
    }
    let Some(target) = pick_target(observation, analysis, memory, from, strength) else {
        return;
    };
    let state = &mut memory.jeff_v1_1.rifle_raid;
    state.party = pocket.iter().map(|unit| unit.id).collect();
    state.start = state.party.len();
    state.target = Some(stored(target));
    state.phase = Some(Phase::Out);
    state.since = tick;
    state.ordered = 0;
    state.sent += 1;
    state.why = "";
}

/// Diagnostics: the spare Riflemen, the known targets past the enemy main's reach, and the least
/// threat among them.
fn look(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    from: (f32, f32),
    spare: usize,
) -> String {
    let ts = observation.map.tile_size as f32;
    let Some(enemy_main) = super::push::enemy_main(observation) else {
        return format!("s{spare} nomain");
    };
    let main_clear = param("rrmainx", MAIN_CLEAR_TILES);
    let all: Vec<(f32, f32)> = targets(observation, analysis, memory, enemy_main)
        .into_iter()
        .map(|(at, _)| at)
        .collect();
    let memory: &AiDecisionMemory = memory;
    let out: Vec<(f32, f32)> = all
        .iter()
        .copied()
        .filter(|at| dist2(at.0, at.1, enemy_main.0, enemy_main.1) > squared(main_clear * ts))
        .collect();
    let threat = out
        .iter()
        .map(|at| {
            let defence = remembered_near(observation, memory, &[*at], DEFENCE_TILES);
            let route = remembered_near(observation, memory, &way(from, *at, ts), ROUTE_TILES);
            (defence, route)
        })
        .min_by(|a, b| a.0.max(a.1).total_cmp(&b.0.max(b.1)));
    match threat {
        Some((defence, route)) => format!("s{spare} k{}/{} d{defence:.0} r{route:.0}", out.len(), all.len()),
        None => format!("s{spare} k0/{}", all.len()),
    }
}
