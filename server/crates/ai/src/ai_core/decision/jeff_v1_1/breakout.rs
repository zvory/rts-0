//! Jeff's AI V1.1 breaks the enemy's contain (`breakout`). On Crossroads against Pressure AI (b4
//! riflest-fix 120), from 5:00 to 8:00 Pressure kept 1-4 Tanks and some infantry 20-25 tiles outside
//! Jeff's bases, on the way to his next sites, while 5-11 Jeff Tanks (2-4 times its value) stood on
//! their stations and never went at it: the base alarm and the stations' threat look 14-16 tiles out
//! and the push waits to be big enough for the enemy's whole army. Pressure went from 3 Depots to 5
//! while Jeff stayed on 2, and his third's Engineer died or turned back in 7 of 8 games.
//!
//! - **Contain**: the enemy fighters remembered in the last `FRESH_TICKS` on Jeff's side of the map,
//!   `OUTSIDE_TILES`-`CONTAIN_TILES` from his nearest Depot, and those within `GROUP_TILES` of the
//!   one nearest a Depot.
//! - **Odds**: the stations' Tanks within `REACH_TILES` of it and the main's free Riflemen and
//!   Panzerfausts past `HOME_KEEP` (at least `MIN_INFANTRY`) go out when they are worth `ODDS` times
//!   the contain and have more Tanks than it, and nothing is inside a base. Infantry always goes:
//!   Tank-only sorties lost whole groups of Tanks to Panzerfausts. With the infantry dead the Tanks
//!   come home.
//! - **Gather**: the infantry in a line across the way from the main, `GATHER_TILES` short of the
//!   contain; the Tanks `TANK_BACK_TILES` behind them.
//! - **Attack**: once most have gathered, the infantry attack-move onto the contain and the Tanks
//!   follow `FOLLOW_TILES` behind the infantry, so Panzerfausts meet the infantry first.
//! - **End**: the contain gone, past `LEASH_TILES` from Jeff's bases or on the enemy's side, the
//!   fight near the sortie turned against it (`ABORT_ODDS`), or `MAX_TICKS` out. The units are let
//!   go: the stations place their Tanks again and the Rifle split its Riflemen, and no new sortie
//!   starts for `COOLDOWN_TICKS`.
//! - **Peek** (`peek`): a contain goes stale `FRESH_TICKS` after it was last seen, and Pressure AI
//!   parks one just out of sight for minutes (Classic, 6-9 Panzerfausts 27-30 tiles out from 5:20 to
//!   7:00; the sortie that won went only because one wandered into view). Up to `PEEK_MEMORY_TICKS`
//!   after the last contain went stale, while the sortie would go at it without the scout, one of the
//!   sortie's own Riflemen walks out to look: full health, no enemy near it, nearest the contain. It
//!   walks `PEEK_PAST_TILES` past where the contain was seen, away from the nearest Depot, turns back
//!   the moment it sees an enemy fighter (or is hit, the base is attacked, the sortie goes, it finds
//!   nothing, or `PEEK_OUT_TICKS` pass), walks back to where it was taken from, and is let go there
//!   for the stations to place again. A scout killed or hit on the way out holds the sortie back
//!   `COOLDOWN_TICKS` and ends the looks at that contain (`peek_losses`).

use super::push::unit_value;
use super::stations::RiflePost;
use super::*;
use crate::ai_core::decision::geometry::{clamp_to_map, dist2, squared, tile_center};
use rts_rules::math::GameplayMath as _;

/// Enemy fighters seen this recently count.
const FRESH_TICKS: u32 = config::TICK_HZ * 20;
/// A contain stands this far from Jeff's nearest Depot: nearer is the base alarm's.
const OUTSIDE_TILES: f32 = 14.0;
const CONTAIN_TILES: f32 = 40.0;
/// Enemy fighters this close to the contain's first one are part of it.
const GROUP_TILES: f32 = 12.0;
/// The smallest contain worth going out for, in unit value.
const MIN_CONTAIN_VALUE: u32 = 300;
/// The fewest Riflemen and Panzerfausts in front of the Tanks.
const MIN_INFANTRY: usize = 3;
/// Stationed Tanks this close to the contain can go.
const REACH_TILES: f32 = 55.0;
/// Free Riflemen the main always keeps.
const HOME_KEEP: usize = 4;
/// The sortie goes when worth this many times the contain...
const ODDS: f32 = 1.6;
/// ... and turns back when the enemy near it is worth more than it over this.
const ABORT_ODDS: f32 = 0.9;
/// Enemies this close to the sortie are the fight it is in.
const FIGHT_TILES: f32 = 12.0;
/// The gathering line stands this far short of the contain, the Tanks this far behind it.
const GATHER_TILES: f32 = 12.0;
const TANK_BACK_TILES: f32 = 3.0;
const RIFLE_SPACING_TILES: f32 = 1.5;
const RIFLE_ROW: usize = 6;
const TANK_SPACING_TILES: f32 = 2.5;
/// A member this close to its gathering place has gathered.
const GATHERED_TILES: f32 = 3.0;
/// The attack goes once this share has gathered, or after `GATHER_MAX_TICKS`.
const GATHERED_SHARE: f32 = 0.7;
const GATHER_MAX_TICKS: u32 = config::TICK_HZ * 25;
/// Attacking, the Tanks keep this far behind the infantry's centre.
const FOLLOW_TILES: f32 = 2.5;
/// Orders go again when their point moved this far or the unit stands idle, at most this often.
const REAIM_TILES: f32 = 2.0;
const REORDER_TICKS: u32 = config::TICK_HZ;
/// The sortie ends past this far from Jeff's nearest Depot, or after this long.
const LEASH_TILES: f32 = 48.0;
const MAX_TICKS: u32 = config::TICK_HZ * 100;
/// The contain is gone when none of it is remembered within this of where it was.
const LOST_TILES: f32 = 18.0;
const COOLDOWN_TICKS: u32 = config::TICK_HZ * 30;
/// `peek`: a stale contain is looked at again up to this long after it went stale...
const PEEK_MEMORY_TICKS: u32 = config::TICK_HZ * 150;
/// ... by a Rifleman picked with no enemy fighter this close, which walks this far past where it
/// was seen, away from Jeff's nearest Depot (but no farther out than `PEEK_REACH_TILES`): a contain
/// is mostly seen walking in, and settles further out (Classic, seen 15 tiles out, parked 27-30).
/// It turns back at the first enemy fighter it sees; both sides see 11 tiles and fire from 5.
const PEEK_SAFE_TILES: f32 = 12.0;
const PEEK_PAST_TILES: f32 = 14.0;
const PEEK_REACH_TILES: f32 = 34.0;
const PEEK_ARRIVED_TILES: f32 = 2.5;
/// It turns back after this long out, and is let go this close to where it was taken from, or
/// after this long walking back.
const PEEK_OUT_TICKS: u32 = config::TICK_HZ * 40;
const PEEK_HOME_TILES: f32 = 3.0;
const PEEK_BACK_TICKS: u32 = config::TICK_HZ * 45;
/// No new look for this long after one ends.
const PEEK_COOLDOWN_TICKS: u32 = config::TICK_HZ * 30;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    Gather,
    Attack,
}

/// The sortie: its units, where it gathers and what it goes at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct BreakoutMemory {
    phase: Phase,
    tanks: BTreeSet<u32>,
    infantry: BTreeSet<u32>,
    target: Option<(i32, i32)>,
    gather: Option<(i32, i32)>,
    since: u32,
    /// Per unit: when it was last ordered and to where.
    ordered: BTreeMap<u32, (u32, (i32, i32))>,
    cooldown_until: u32,
    why: &'static str,
    note: String,
    sorties: u32,
    /// `peek`: the last contain seen (where, its value and Tanks, when), the Rifleman out looking
    /// at it, no new look before this tick, looks so far and why the last one turned back.
    last_contain: Option<((i32, i32), u32, usize, u32)>,
    peek: Option<Peek>,
    peek_cooldown_until: u32,
    peeks: u32,
    peek_why: &'static str,
}

/// `peek`: the Rifleman looking at a stale contain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Peek {
    unit: u32,
    /// Where it was taken from, where it looks from, and the contain it looks for.
    origin: (i32, i32),
    vantage: (i32, i32),
    spot: (i32, i32),
    back: bool,
    /// When it went out, or turned back.
    since: u32,
    /// Its health when it went out: any hit sends it back.
    hp: u32,
    /// When it was last ordered (0: not yet this leg).
    ordered: u32,
}

impl BreakoutMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.tanks.contains(&id)
            || self.infantry.contains(&id)
            || self.peek.is_some_and(|peek| peek.unit == id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.tanks
            .iter()
            .chain(self.infantry.iter())
            .copied()
            .chain(self.peek.map(|peek| peek.unit))
    }

    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        let peek = self.peek.map_or_else(
            || "-".to_string(),
            |peek| format!("{}{}", peek.unit, if peek.back { "<" } else { ">" }),
        );
        format!(
            "v1_1_breakout {:?} sorties={} why={} target={:?} tanks={:?} inf={:?} peek={} n{} {} {}",
            self.phase,
            self.sorties,
            self.why,
            self.target,
            self.tanks,
            self.infantry,
            peek,
            self.peeks,
            self.peek_why,
            self.note
        )
    }

    /// Ends the sortie (not a look under way) and returns its units.
    fn end(&mut self, tick: u32, why: &'static str) -> Vec<u32> {
        let released: Vec<u32> = self.tanks.iter().chain(self.infantry.iter()).copied().collect();
        self.phase = Phase::Idle;
        self.tanks.clear();
        self.infantry.clear();
        self.ordered.clear();
        self.target = None;
        self.gather = None;
        self.cooldown_until = tick + COOLDOWN_TICKS;
        self.why = why;
        released
    }
}

struct Contain {
    center: (f32, f32),
    value: u32,
    tanks: usize,
}

/// Runs the sortie (see the module docs). Returns the units it holds, which no later system of
/// this decision orders.
pub(in crate::ai_core::decision) fn plan(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    held: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    if !is_jeff_v1_1(profile_id) || super::off("breakout") {
        memory.jeff_v1_1.breakout = BreakoutMemory::default();
        return BTreeSet::new();
    }
    let mut state = std::mem::take(&mut memory.jeff_v1_1.breakout);
    if !super::off("peek") {
        peek_losses(observation, &mut state);
    }
    let mut holding = run(
        (&mut *actions, observation, map_analysis, &mut *intents),
        memory,
        &mut state,
        held,
    );
    if super::off("peek") {
        state.peek = None;
        state.last_contain = None;
    } else {
        holding.extend(peek(
            (actions, observation, map_analysis, intents),
            memory,
            &mut state,
            held,
        ));
    }
    memory.jeff_v1_1.breakout = state;
    holding
}

fn run(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &AiDecisionMemory,
    state: &mut BreakoutMemory,
    held: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let Some(enemy_main) = observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))
        .min_by(|a, b| dist2(a.0, a.1, main.0, main.1).total_cmp(&dist2(b.0, b.1, main.0, main.1)))
    else {
        return BTreeSet::new();
    };
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    if depots.is_empty() {
        return BTreeSet::new();
    }
    let from_depots = |at: (f32, f32)| {
        depots
            .iter()
            .map(|depot| dist2(at.0, at.1, depot.0, depot.1).det_sqrt() / ts)
            .fold(f32::INFINITY, f32::min)
    };
    let ours_side = |at: (f32, f32)| {
        dist2(at.0, at.1, main.0, main.1) < dist2(at.0, at.1, enemy_main.0, enemy_main.1)
    };
    let sightings: Vec<(EntityKind, u32, (f32, f32))> = memory
        .jeff_v1_1
        .push
        .remembered()
        .filter(|(_, _, _, _, seen)| tick.saturating_sub(*seen) <= FRESH_TICKS)
        .map(|(_, kind, value, at, _)| (kind, value, at))
        .collect();
    let group_at = |anchor: (f32, f32)| -> Contain {
        let members: Vec<&(EntityKind, u32, (f32, f32))> = sightings
            .iter()
            .filter(|(_, _, at)| dist2(at.0, at.1, anchor.0, anchor.1) <= squared(GROUP_TILES * ts))
            .collect();
        let n = members.len().max(1) as f32;
        let center = (
            members.iter().map(|(_, _, at)| at.0).sum::<f32>() / n,
            members.iter().map(|(_, _, at)| at.1).sum::<f32>() / n,
        );
        Contain {
            center: if members.is_empty() { anchor } else { center },
            value: members.iter().map(|(_, value, _)| *value).sum(),
            tanks: members
                .iter()
                .filter(|(kind, _, _)| *kind == EntityKind::Tank)
                .count(),
        }
    };
    state.tanks.retain(|id| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == *id && unit.hp > 0)
    });
    state.infantry.retain(|id| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == *id && unit.hp > 0)
    });
    // `bkheld`: a unit another system took mid-sortie is no longer the sortie's. The third-base raid
    // took a sortie's last two Riflemen as scouts while the sortie still counted them, so its Tanks
    // followed the scouts east and stayed out of home defense for 83 s (Wald des Todes, 5:16).
    // With its whole screen taken the sortie ends: Tanks never go on alone.
    if state.phase != Phase::Idle && !super::off("bkheld") {
        let taken: BTreeSet<u32> = memory
            .squad_reserved()
            .into_iter()
            .chain(memory.later_bases.guards.iter().copied())
            .collect();
        let had_infantry = !state.infantry.is_empty();
        state.tanks.retain(|id| !taken.contains(id));
        state.infantry.retain(|id| !taken.contains(id));
        if had_infantry && state.infantry.is_empty() {
            let released = state.end(tick, "taken");
            release(intents, released);
            return BTreeSet::new();
        }
    }

    if state.phase == Phase::Idle {
        if tick < state.cooldown_until {
            return BTreeSet::new();
        }
        // Something inside a base is the alarm's: no sortie then.
        let inside = observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && enemy.kind != EntityKind::Worker
                && from_depots((enemy.x, enemy.y)) < OUTSIDE_TILES
        });
        if inside {
            return BTreeSet::new();
        }
        let Some(anchor) = sightings
            .iter()
            .filter(|(_, _, at)| ours_side(*at))
            .map(|(_, _, at)| (*at, from_depots(*at)))
            .filter(|(_, tiles)| (OUTSIDE_TILES..=CONTAIN_TILES).contains(tiles))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(at, _)| at)
        else {
            return BTreeSet::new();
        };
        let contain = group_at(anchor);
        if contain.value < MIN_CONTAIN_VALUE {
            return BTreeSet::new();
        }
        // `peek`: remembered for a look once it goes stale.
        state.last_contain = Some((stored(contain.center), contain.value, contain.tanks, tick));
        let peeking = state.peek.map(|peek| peek.unit);
        let (tanks, infantry) = force(observation, memory, held, peeking, contain.center, main);
        let value: u32 = tanks
            .iter()
            .chain(infantry.iter())
            .map(|unit| unit_value(unit.kind))
            .sum();
        state.note = format!(
            "contain={:.0},{:.0} v{} t{} ours v{} t{} i{}",
            contain.center.0 / ts,
            contain.center.1 / ts,
            contain.value,
            contain.tanks,
            value,
            tanks.len(),
            infantry.len()
        );
        // Infantry goes in front, always: Tank-only sorties lost whole groups to Panzerfausts (the
        // contain's remembered part often showed none), 3-5 Tanks at a time on Crossroads.
        let screened = infantry.len() >= MIN_INFANTRY;
        if (value as f32) < ODDS * contain.value as f32 || tanks.len() <= contain.tanks || !screened
        {
            return BTreeSet::new();
        }
        // Gather on the way from the main, `GATHER_TILES` short of the contain.
        let route = map_analysis
            .map(|analysis| analysis.compact_group_route(main, contain.center, 2))
            .unwrap_or_default();
        let gather = route
            .iter()
            .rev()
            .find(|at| {
                dist2(at.0, at.1, contain.center.0, contain.center.1) >= squared(GATHER_TILES * ts)
            })
            .copied()
            .unwrap_or(main);
        state.phase = Phase::Gather;
        state.since = tick;
        state.sorties += 1;
        state.why = "out";
        state.target = Some(stored(contain.center));
        state.gather = Some(stored(gather));
        state.tanks = tanks.iter().map(|unit| unit.id).collect();
        state.infantry = infantry.iter().map(|unit| unit.id).collect();
        state.ordered.clear();
        // `peek`: the sortie deals with this one; a contain left after it is seen again.
        state.last_contain = None;
    }

    // Under way: follow the contain, and end when it is gone or the fight turns.
    let (Some(target), Some(gather)) = (state.target.map(point), state.gather.map(point)) else {
        let released = state.end(tick, "lost");
        release(intents, released);
        return BTreeSet::new();
    };
    let follow = sightings
        .iter()
        .filter(|(_, _, at)| dist2(at.0, at.1, target.0, target.1) <= squared(LOST_TILES * ts))
        .min_by(|a, b| {
            dist2(a.2 .0, a.2 .1, target.0, target.1)
                .total_cmp(&dist2(b.2 .0, b.2 .1, target.0, target.1))
        })
        .map(|(_, _, at)| group_at(*at));
    let members: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| state.tanks.contains(&unit.id) || state.infantry.contains(&unit.id))
        .collect();
    let mid = center(members.iter().copied());
    let ours: u32 = members.iter().map(|unit| unit_value(unit.kind)).sum();
    let theirs: u32 = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| dist2(enemy.x, enemy.y, mid.0, mid.1) <= squared(FIGHT_TILES * ts))
        .map(|enemy| unit_value(enemy.kind))
        .sum();
    // With its infantry dead the sortie turns back, unless (`baregun`) no Panzerfaust or Anti-Tank
    // Gun is in sight of it: against Rush AI's dug-in guns 3 Riflemen died in 4 s and 3 Tanks the
    // guns could not hurt went home.
    let anti_tank_near = observation.visible_enemies.iter().any(|enemy| {
        enemy.hp > 0
            && matches!(
                enemy.kind,
                EntityKind::Panzerfaust | EntityKind::AntiTankGun
            )
            && dist2(enemy.x, enemy.y, mid.0, mid.1) <= squared(FIGHT_TILES * ts)
    });
    let screen_gone = !state
        .infantry
        .iter()
        .any(|id| members.iter().any(|unit| unit.id == *id))
        && (super::off("baregun") || anti_tank_near);
    let why = if members.is_empty() || state.tanks.is_empty() {
        Some("lost")
    } else if screen_gone {
        Some("bare")
    } else if follow.is_none() {
        Some("gone")
    } else if follow
        .as_ref()
        .is_some_and(|c| from_depots(c.center) > LEASH_TILES || !ours_side(c.center))
    {
        Some("leash")
    } else if theirs > 0 && (ours as f32) < ABORT_ODDS * theirs as f32 {
        Some("odds")
    } else if tick.saturating_sub(state.since) >= MAX_TICKS {
        Some("time")
    } else {
        None
    };
    if let Some(why) = why {
        let released = state.end(tick, why);
        release(intents, released);
        return BTreeSet::new();
    }
    let target = follow.map_or(target, |c| c.center);
    state.target = Some(stored(target));
    state.note = format!(
        "target={:.0},{:.0} ours v{} near v{}",
        target.0 / ts,
        target.1 / ts,
        ours,
        theirs
    );
    let facing = normalized_direction(gather, target).unwrap_or((1.0, 0.0));
    let side = (-facing.1, facing.0);
    let rifles: Vec<&AiEntitySummary> = members
        .iter()
        .copied()
        .filter(|unit| state.infantry.contains(&unit.id))
        .collect();
    let tanks: Vec<&AiEntitySummary> = members
        .iter()
        .copied()
        .filter(|unit| state.tanks.contains(&unit.id))
        .collect();

    if state.phase == Phase::Gather {
        let rifle_slots = line(
            gather,
            facing,
            side,
            rifles.len(),
            RIFLE_SPACING_TILES,
            RIFLE_ROW,
            ts,
        );
        let tank_back = (
            gather.0 - facing.0 * TANK_BACK_TILES * ts,
            gather.1 - facing.1 * TANK_BACK_TILES * ts,
        );
        let tank_slots = line(
            tank_back,
            facing,
            side,
            tanks.len(),
            TANK_SPACING_TILES,
            5,
            ts,
        );
        let mut gathered = 0;
        for (unit, slot) in rifles
            .iter()
            .zip(&rifle_slots)
            .chain(tanks.iter().zip(&tank_slots))
        {
            let slot = clear(observation, map_analysis, (unit.x, unit.y), *slot);
            if dist2(unit.x, unit.y, slot.0, slot.1) <= squared(GATHERED_TILES * ts) {
                gathered += 1;
            }
            order(actions, intents, state, observation, unit, slot, false);
        }
        let enemy_close = theirs > 0;
        let share = gathered as f32 / members.len().max(1) as f32;
        if share >= GATHERED_SHARE
            || enemy_close
            || tick.saturating_sub(state.since) >= GATHER_MAX_TICKS
        {
            state.phase = Phase::Attack;
            state.ordered.clear();
        }
    } else {
        for unit in &rifles {
            order(actions, intents, state, observation, unit, target, true);
        }
        let lead = if rifles.is_empty() {
            target
        } else {
            let front = center(rifles.iter().copied());
            let back = normalized_direction(front, target).unwrap_or(facing);
            clamp_to_map(
                (
                    front.0 - back.0 * FOLLOW_TILES * ts,
                    front.1 - back.1 * FOLLOW_TILES * ts,
                ),
                observation.map,
            )
        };
        for unit in &tanks {
            order(actions, intents, state, observation, unit, lead, true);
        }
    }
    state.units().collect()
}

/// The sortie's force against a contain at `at`: the stationed Tanks within reach, and the main's
/// free infantry past the `HOME_KEEP` Riflemen nearest its start. `skip`: the `peek` Rifleman, which
/// is the breakout's already.
fn force<'o>(
    observation: &'o AiObservation,
    memory: &AiDecisionMemory,
    held: &BTreeSet<u32>,
    skip: Option<u32>,
    at: (f32, f32),
    main: (f32, f32),
) -> (Vec<&'o AiEntitySummary>, Vec<&'o AiEntitySummary>) {
    let ts = observation.map.tile_size as f32;
    // A natural clearing group stuck short of the Riflemen it needs comes too (`ncbreak`): on
    // Classic against Rush AI 8 of them waited 4 minutes for 21 while Rush's dug-in guns between
    // the main and the natural grew from 4 to 11 and 5-8 station Tanks stood by.
    let take_clearing = !super::off("ncbreak") && memory.jeff_v1_1.natclear.stuck();
    let clearing = |unit: &AiEntitySummary| memory.jeff_v1_1.natclear.holds(unit.id);
    let usable = |unit: &AiEntitySummary| {
        unit.is_complete
            && unit.hp > 0
            && unit.free_for_combat
            && Some(unit.id) != skip
            && if take_clearing && clearing(unit) {
                true
            } else {
                !held.contains(&unit.id)
                    && !memory.stays_out_of_home_defense(unit.id)
                    && !clearing(unit)
            }
            && !memory.jeff_v1_1.natural.holds(unit.id)
    };
    let stationed: BTreeSet<u32> = memory.jeff_v1_1.stations.stationed().collect();
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && stationed.contains(&unit.id))
        .filter(|unit| usable(unit))
        .filter(|unit| dist2(unit.x, unit.y, at.0, at.1) <= squared(REACH_TILES * ts))
        .collect();
    let mut infantry: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| matches!(unit.kind, EntityKind::Rifleman | EntityKind::Panzerfaust))
        .filter(|unit| usable(unit))
        .filter(|unit| {
            (take_clearing && clearing(unit))
                || match memory.jeff_v1_1.stations.rifle_post(unit.id) {
                    Some(post) => post == RiflePost::Main,
                    None => dist2(unit.x, unit.y, main.0, main.1) <= squared(22.0 * ts),
                }
        })
        .collect();
    // The main keeps the `HOME_KEEP` Riflemen nearest its start.
    infantry.sort_by(|a, b| {
        dist2(a.x, a.y, main.0, main.1)
            .total_cmp(&dist2(b.x, b.y, main.0, main.1))
            .then(a.id.cmp(&b.id))
    });
    let kept = infantry
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman)
        .take(HOME_KEEP)
        .map(|unit| unit.id)
        .collect::<BTreeSet<_>>();
    infantry.retain(|unit| !kept.contains(&unit.id));
    (tanks, infantry)
}

/// `peek`: a scout killed or hit on its way out met something it could not see, as a parked Tank
/// hits from 14 tiles and a Rifleman sees 11. The shot shows the contain, and the sortie went at it
/// in the same decision, into Tanks no one had seen (Classic and The River against Pressure AI: 2
/// of 2 such sorties lost their infantry or turned back on the odds). No sortie for
/// `COOLDOWN_TICKS` then, so the shot alone does not send one, and no more looks at that contain.
/// Runs before the sortie's own step.
fn peek_losses(observation: &AiObservation, state: &mut BreakoutMemory) {
    let Some(peek) = state.peek.filter(|peek| !peek.back) else {
        return;
    };
    let hit = observation
        .owned
        .iter()
        .find(|unit| unit.id == peek.unit && unit.hp > 0)
        .is_none_or(|unit| unit.hp < peek.hp);
    if hit {
        let tick = observation.tick;
        state.cooldown_until = state.cooldown_until.max(tick + COOLDOWN_TICKS);
        state.last_contain = None;
    }
}

/// `peek` (see the module docs): runs the look under way, or starts one at the last contain once
/// it is stale. Returns the Rifleman out looking, which no later system of this decision orders.
/// `held`: units other systems keep this decision, never picked.
fn peek(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &AiDecisionMemory,
    state: &mut BreakoutMemory,
    held: &BTreeSet<u32>,
) -> Option<u32> {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let from_depots = |at: (f32, f32)| {
        depots
            .iter()
            .map(|depot| dist2(at.0, at.1, depot.0, depot.1).det_sqrt() / ts)
            .fold(f32::INFINITY, f32::min)
    };
    let fighters = || {
        observation.visible_enemies.iter().filter(|enemy| {
            enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker
        })
    };
    let alarm = fighters().any(|enemy| from_depots((enemy.x, enemy.y)) < OUTSIDE_TILES);

    if let Some(mut peek) = state.peek {
        let unit = observation
            .owned
            .iter()
            .find(|unit| unit.id == peek.unit && unit.hp > 0);
        // Dead, or taken by a system that outranks the breakout: either way no longer ours.
        let Some(unit) = unit.filter(|unit| !memory.stays_out_of_home_defense(unit.id)) else {
            state.peek = None;
            state.peek_why = if unit.is_some() { "taken" } else { "dead" };
            state.peek_cooldown_until = tick + PEEK_COOLDOWN_TICKS;
            return None;
        };
        if !peek.back {
            let spot = point(peek.spot);
            let vantage = point(peek.vantage);
            let seen = fighters().any(|enemy| {
                [spot, vantage, (unit.x, unit.y)]
                    .iter()
                    .any(|p| dist2(enemy.x, enemy.y, p.0, p.1) <= squared(PEEK_SAFE_TILES * ts))
            });
            let arrived =
                dist2(unit.x, unit.y, vantage.0, vantage.1) <= squared(PEEK_ARRIVED_TILES * ts);
            let why = if seen {
                Some("seen")
            } else if state.phase != Phase::Idle {
                Some("sortie")
            } else if alarm {
                Some("alarm")
            } else if unit.hp < peek.hp {
                Some("hurt")
            } else if arrived {
                // Nothing where the contain stood: no more looks for it.
                state.last_contain = None;
                Some("empty")
            } else if tick.saturating_sub(peek.since) >= PEEK_OUT_TICKS {
                Some("time")
            } else {
                None
            };
            if let Some(why) = why {
                peek.back = true;
                peek.since = tick;
                peek.ordered = 0;
                state.peek_why = why;
            }
        }
        let to = point(if peek.back { peek.origin } else { peek.vantage });
        if peek.back
            && (dist2(unit.x, unit.y, to.0, to.1) <= squared(PEEK_HOME_TILES * ts)
                || tick.saturating_sub(peek.since) >= PEEK_BACK_TICKS)
        {
            // Back where it was taken from: the stations and the Rifle split place it again.
            release(intents, vec![peek.unit]);
            state.peek = None;
            state.peek_cooldown_until = tick + PEEK_COOLDOWN_TICKS;
            return None;
        }
        let stalled = unit.state == AiEntityState::Idle
            && tick.saturating_sub(peek.ordered) >= REORDER_TICKS;
        if peek.ordered == 0 || stalled {
            send_peek(actions, intents, &mut peek, to, tick);
        }
        state.peek = Some(peek);
        return Some(peek.unit);
    }

    // A new look: the sortie idle and free to go, nothing inside a base, a contain gone stale.
    if state.phase != Phase::Idle
        || tick < state.cooldown_until
        || tick < state.peek_cooldown_until
        || alarm
        || depots.is_empty()
    {
        return None;
    }
    let Some((spot, value, tanks_there, seen)) = state.last_contain else {
        return None;
    };
    let age = tick.saturating_sub(seen);
    if age < FRESH_TICKS {
        return None;
    }
    let at = point(spot);
    if age > PEEK_MEMORY_TICKS || !(OUTSIDE_TILES..=CONTAIN_TILES).contains(&from_depots(at)) {
        state.last_contain = None;
        return None;
    }
    // The scout is one of the sortie's own Riflemen: full health, no enemy fighter near it,
    // nearest the contain. The sortie must still go at the contain as last seen without it.
    let (tanks, infantry) = force(observation, memory, held, None, at, main);
    let full = config::unit_stats(EntityKind::Rifleman).map_or(1, |stats| stats.hp);
    let Some(scout) = infantry
        .iter()
        .copied()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.hp >= full)
        .filter(|unit| {
            !fighters().any(|enemy| {
                dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(PEEK_SAFE_TILES * ts)
            })
        })
        .min_by(|a, b| {
            dist2(a.x, a.y, at.0, at.1)
                .total_cmp(&dist2(b.x, b.y, at.0, at.1))
                .then(a.id.cmp(&b.id))
        })
    else {
        return None;
    };
    let rest = infantry.iter().filter(|unit| unit.id != scout.id).count();
    let worth: u32 = tanks
        .iter()
        .chain(infantry.iter())
        .filter(|unit| unit.id != scout.id)
        .map(|unit| unit_value(unit.kind))
        .sum();
    if (worth as f32) < ODDS * value as f32 || tanks.len() <= tanks_there || rest < MIN_INFANTRY {
        return None;
    }
    // It walks `PEEK_PAST_TILES` past the contain, away from the nearest Depot.
    let from = (scout.x, scout.y);
    let Some(depot) = depots.iter().copied().min_by(|a, b| {
        dist2(a.0, a.1, at.0, at.1).total_cmp(&dist2(b.0, b.1, at.0, at.1))
    }) else {
        return None;
    };
    let out = from_depots(at);
    let vantage = match normalized_direction(depot, at) {
        Some(away) if out < PEEK_REACH_TILES => {
            let reach = (out + PEEK_PAST_TILES).min(PEEK_REACH_TILES) * ts;
            (depot.0 + away.0 * reach, depot.1 + away.1 * reach)
        }
        _ => at,
    };
    let vantage = clear(observation, map_analysis, from, clamp_to_map(vantage, observation.map));
    let mut peek = Peek {
        unit: scout.id,
        origin: stored(from),
        vantage: stored(vantage),
        spot,
        back: false,
        since: tick,
        hp: scout.hp,
        ordered: 0,
    };
    send_peek(actions, intents, &mut peek, vantage, tick);
    state.peek = Some(peek);
    state.peeks += 1;
    state.peek_why = "out";
    Some(peek.unit)
}

/// Walks the `peek` Rifleman to `to`: a plain move, so it does not stop to fight on the way.
fn send_peek(
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
    peek: &mut Peek,
    to: (f32, f32),
    tick: u32,
) {
    if let Some(units) = actions::move_units(actions, [peek.unit], to.0, to.1) {
        intents.push(AiIntent::Move { units });
        peek.ordered = tick.max(1);
    }
}

/// Sends `unit` to `point` (attack-move when `attack`), again only when the point moved
/// `REAIM_TILES` or the unit stands idle, and never sooner than `REORDER_TICKS`.
fn order(
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
    state: &mut BreakoutMemory,
    observation: &AiObservation,
    unit: &AiEntitySummary,
    point: (f32, f32),
    attack: bool,
) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    if let Some((at, to)) = state.ordered.get(&unit.id) {
        let to = (to.0 as f32, to.1 as f32);
        let moved = dist2(to.0, to.1, point.0, point.1) > squared(REAIM_TILES * ts);
        let idle = unit.state == AiEntityState::Idle
            && dist2(unit.x, unit.y, point.0, point.1) > squared(GATHERED_TILES * ts);
        if tick.saturating_sub(*at) < REORDER_TICKS || !(moved || idle) {
            return;
        }
    }
    let sent = if attack {
        actions::attack_move_units(actions, [unit.id], point.0, point.1)
    } else {
        actions::move_units(actions, [unit.id], point.0, point.1)
    };
    if let Some(units) = sent {
        intents.push(if attack {
            AiIntent::Attack { units }
        } else {
            AiIntent::Move { units }
        });
        state.ordered.insert(unit.id, (tick, stored(point)));
    }
}

fn release(intents: &mut Vec<AiIntent>, units: Vec<u32>) {
    if !units.is_empty() {
        // Clears the live adapter's cached staging so the stations and the Rifle split place them.
        intents.push(AiIntent::Assemble { units });
    }
}

/// `count` points in rows of `row` across `facing` at `at`, `spacing` tiles apart, each row a tile
/// and a half behind the one before.
fn line(
    at: (f32, f32),
    facing: (f32, f32),
    side: (f32, f32),
    count: usize,
    spacing: f32,
    row: usize,
    ts: f32,
) -> Vec<(f32, f32)> {
    (0..count)
        .map(|index| {
            let rank = (index / row) as f32;
            let in_row = (count - (index / row) * row).min(row);
            let slot = (index % row) as f32 - (in_row as f32 - 1.0) * 0.5;
            (
                at.0 + side.0 * slot * spacing * ts - facing.0 * rank * 1.5 * ts,
                at.1 + side.1 * slot * spacing * ts - facing.1 * rank * 1.5 * ts,
            )
        })
        .collect()
}

fn clear(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    from: (f32, f32),
    point: (f32, f32),
) -> (f32, f32) {
    let point = clamp_to_map(point, observation.map);
    map_analysis
        .and_then(|analysis| analysis.open_ground_near(from, point, 1, 3))
        .unwrap_or(point)
}

fn center<'a>(units: impl Iterator<Item = &'a AiEntitySummary>) -> (f32, f32) {
    let (mut x, mut y, mut n) = (0.0, 0.0, 0.0);
    for unit in units {
        x += unit.x;
        y += unit.y;
        n += 1.0;
    }
    if n > 0.0 {
        (x / n, y / n)
    } else {
        (0.0, 0.0)
    }
}

fn stored((x, y): (f32, f32)) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

fn point((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32, y as f32)
}
