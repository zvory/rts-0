//! Jeff's AI V1.1 holds a line across the enemy's way in and walks it forward. V1.0 kept its home
//! Tanks in a ball between its bases (against Pressure AI on Classic, 3-4 Tanks 1.4-2.2 tiles apart
//! from 6:00 to 7:00) and its 35-50 Riflemen spread around the Depots 3-8 tiles forward of them,
//! then sent the Tanks out as one push.
//!
//! - **Line**: `AHEAD_TILES` in front of Jeff's frontier, across the way from its main to the enemy
//!   main: the frontier is the most forward of its Steel Mines and Pump Jacks and of the Steel and
//!   Oil at the sites it is about to take (the natural's, the next later base's), not the Depots,
//!   which stand behind them, so the line moves up as Jeff expands, and its middle sits
//!   across the bases near that front. Tanks `TANK_SPACING_TILES` apart, Riflemen `RIFLE_SPACING_TILES` apart
//!   `RIFLE_AHEAD_TILES` in front of them, each post pulled back onto open ground. Its centre leans
//!   up to `LEAN_TILES` toward the side where more enemy strength was seen lately.
//! - **Units**: every Tank no other system holds, V1.0's home Tank and the raid aside, and the
//!   Riflemen past the `HOME_RIFLES` nearest the main. Each goes to its post and holds there; a unit
//!   in a fight is left to it.
//! - **Give and take**: losing `FALL_LOSS_SHARE` of its strength over `LOSS_TICKS` or facing
//!   `FALL_RATIO` of it in sight, every `STEP_TICKS` it steps `STEP_TILES` back toward the frontier,
//!   up to `MAX_BACK_TILES`; holding with little loss and the enemy near it under `ADVANCE_RATIO`
//!   of it, it steps forward again.
//! - **Breaking**: facing `BREAK_RATIO` of its strength in sight, or having lost `BREAK_LOSS_SHARE`
//!   of it over `LOSS_TICKS`, the line breaks: for `BROKEN_TICKS` it holds no unit, and every unit
//!   falls back to V1.0's defence of the bases (the home pocket, raid response, the resting Tanks'
//!   gathering), then the line forms again. It breaks the same way when a base behind it is hit:
//!   enemy strength within `BASE_ATTACK_TILES` of the main's Depot worth `MAIN_BREAK_VALUE`, or of
//!   any Depot worth `BASE_BREAK_SHARE` of the line's. Against Pressure AI, with the line out at
//!   the forward bases, 5 of 16 games ended 7:15-7:55 with the main's Depot shot down.

use super::*;
use crate::ai_core::decision::geometry::{building_center, dist2, squared, tile_center};

const TANK_SPACING_TILES: f32 = 8.0;
const RIFLE_SPACING_TILES: f32 = 4.0;
const RIFLE_AHEAD_TILES: f32 = 3.0;
const HOME_RIFLES: usize = 6;
const MAX_RIFLES: usize = 24;
const AHEAD_TILES: f32 = 3.0;
/// Steel and Oil this close to a site being taken are that base's.
const SITE_FIELD_TILES: f32 = 10.0;
/// Depots and sites this far behind the frontier still set where the line's middle sits.
const FRONT_BAND_TILES: f32 = 25.0;
const STEP_TILES: f32 = 4.0;
const STEP_TICKS: u32 = config::TICK_HZ * 20;
const MAX_BACK_TILES: f32 = 12.0;
const LOSS_TICKS: u32 = config::TICK_HZ * 30;
const HOLD_LOSS_SHARE: f32 = 0.1;
const FALL_LOSS_SHARE: f32 = 0.3;
const ADVANCE_RATIO: f32 = 0.6;
const FALL_RATIO: f32 = 1.2;
const BREAK_RATIO: f32 = 1.5;
const BREAK_LOSS_SHARE: f32 = 0.4;
const BROKEN_TICKS: u32 = config::TICK_HZ * 30;
const BASE_ATTACK_TILES: f32 = 16.0;
/// Two Tanks' worth (each its cost, Oil counted double).
const MAIN_BREAK_VALUE: f32 = 1500.0;
const BASE_BREAK_SHARE: f32 = 0.6;
/// Enemy fighters in sight this close to the line's centre weigh against it.
const NEAR_LINE_TILES: f32 = 18.0;
/// Most units within this of their posts: the line is settled.
const ON_POST_TILES: f32 = 3.0;
const SETTLED_SHARE: f32 = 0.6;
const LEAN_TILES: f32 = 8.0;
/// A unit with an enemy fighter this close fights where it stands.
const FIGHT_TILES: f32 = 7.0;
const REORDER_TICKS: u32 = config::TICK_HZ * 3;
/// Enemy fighters this close to a Depot or a line unit are an attack coming in.
const CONTACT_TILES: f32 = 25.0;
const CONTACT_TICKS: u32 = config::TICK_HZ * 20;
const RESERVE_BACK_TILES: f32 = 5.0;
const RESERVE_SPACING_TILES: f32 = 3.0;
/// A base's extractors and fields this close to its Depot or site.
const BASE_FIELD_TILES: f32 = 14.0;
/// A stretched line spans the front bases and this much more.
const FIT_MARGIN_TILES: f32 = 8.0;
const MAX_FIT_SPACING_TILES: f32 = 20.0;
/// Grouped (`linegroups`): Tanks to a base's group, and how far apart they stand. One or two to a
/// base, 8 tiles apart, each Tank was alone: against Pressure AI 79 of the 185 Tanks the line lost
/// had no other Jeff Tank within 8 tiles.
const GROUP_TANKS: usize = 3;
const GROUP_SPACING_TILES: f32 = 2.5;
/// Enemy fighters in sight this close to a base or its segment are contact there, from this value.
const ROLL_TILES: f32 = 20.0;
const ROLL_MIN_VALUE: f32 = 400.0;
/// The base in contact takes Tanks until they are worth this much of the enemy there.
const ROLL_MATCH: f32 = 1.2;
const GROUP_ON_POST_TILES: f32 = 2.5;
/// An enemy Tank this close to a grouped line Tank holds it on its spot, up to this far off its post.
const ENGAGED_TILES: f32 = 15.0;
const LEASH_TILES: f32 = 6.0;

/// V1.1's front line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct FrontLineMemory {
    /// How far the line stands back from `AHEAD_TILES` in front of the frontier, in tenths of a tile.
    back: u32,
    last_step: u32,
    /// Each line unit and its value, last decision.
    members: BTreeMap<u32, u32>,
    /// Value lost from the line, and when.
    losses: Vec<(u32, u32)>,
    /// When each unit was last sent to its post.
    ordered: BTreeMap<u32, u32>,
    /// Where and when enemy fighters last came near a Depot or the line.
    contact: Option<((i32, i32), u32)>,
    /// The line's Tanks, last decision.
    tanks: usize,
    /// The line stands broken until then.
    broken_until: u32,
    /// The line's middle, last decision, where a raid falls back to.
    middle: Option<(i32, i32)>,
    /// Why the line last broke, for the decision trace.
    why: &'static str,
    /// Tanks V1.0's home defense took, and when it last had them (`lineyield`).
    yielded: BTreeMap<u32, u32>,
}

impl FrontLineMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.members.contains_key(&id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.members.keys().copied()
    }

    /// Where the line stands, while it holds.
    pub(in crate::ai_core::decision) fn rally(&self) -> Option<(f32, f32)> {
        self.middle.map(|(x, y)| (x as f32, y as f32))
    }

    /// The decision-trace note: broken until when and why, how far back, the Tanks yielded.
    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        format!(
            "broken_until={} why={} back={} yielded={:?}",
            self.broken_until,
            self.why,
            self.back,
            self.yielded.keys().collect::<Vec<_>>()
        )
    }

    /// Whether V1.0's home defense had this Tank within `YIELD_TICKS`: the line and the home guard
    /// leave it there.
    pub(in crate::ai_core::decision) fn yielded(&self, id: u32) -> bool {
        self.yielded.contains_key(&id)
    }
}

/// A Tank V1.0's home defense takes stays out of the line and the home guard this long. Both took
/// it back the next decision: against Pressure AI two Tanks swapped between the line and the
/// defense's attack every decision, driving between the two, and never parked for their 14-tile
/// charge.
const YIELD_TICKS: u32 = config::TICK_HZ * 10;

/// Notes the Tanks V1.0's home defense took this decision (`defended`), with `lineyield`.
pub(in crate::ai_core::decision) fn note_yielded(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    defended: &BTreeSet<u32>,
) {
    let on = memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("lineyield");
    let state = &mut memory.jeff_v1_1.front_line;
    if !on {
        state.yielded.clear();
        return;
    }
    let tick = observation.tick;
    for unit in observation.owned.iter() {
        if unit.kind == EntityKind::Tank && defended.contains(&unit.id) {
            state.yielded.insert(unit.id, tick);
        }
    }
    state
        .yielded
        .retain(|_, at| tick.saturating_sub(*at) <= YIELD_TICKS);
}

/// Whether V1.1 holds a front line.
pub(in crate::ai_core::decision) fn active(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("frontline")
}

/// Picks, walks and posts V1.1's front line (see the module docs). `held`: units other systems keep
/// this decision.
pub(in crate::ai_core::decision) fn hold(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    held: &BTreeSet<u32>,
) {
    if !active(memory) {
        memory.jeff_v1_1.front_line = FrontLineMemory::default();
        return;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let Some(enemy_main) = super::push::enemy_main(observation) else {
        return;
    };
    let by_id: BTreeMap<u32, &AiEntitySummary> =
        observation.owned.iter().map(|unit| (unit.id, unit)).collect();
    // Losses since the last decision.
    {
        let state = &mut memory.jeff_v1_1.front_line;
        let lost: u32 = state
            .members
            .iter()
            .filter(|(id, _)| by_id.get(id).is_none_or(|unit| unit.hp == 0))
            .map(|(_, value)| *value)
            .sum();
        if lost > 0 {
            state.losses.push((tick, lost));
        }
        state.losses.retain(|(at, _)| tick.saturating_sub(*at) <= LOSS_TICKS);
    }
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let reserved = memory.squad_reserved();
    let free = |unit: &AiEntitySummary| {
        unit.is_complete
            && unit.hp > 0
            && !held.contains(&unit.id)
            && !reserved.contains(&unit.id)
            && !memory.containment.active_tanks.contains(&unit.id)
            && !memory.containment.active_riflemen.contains(&unit.id)
            && memory.containment.active_scout != Some(unit.id)
            && memory.home_defensive_tank != Some(unit.id)
            && !memory.later_bases.guards.contains(&unit.id)
            && !memory.expansion_security.riflemen.contains(&unit.id)
            && !memory.jeff_v1_1.natural.holds(unit.id)
            && !memory.jeff_v1_1.tank_raid.holds(unit.id)
            && !memory.jeff_v1_1.deny.holds(unit.id)
            && !memory.jeff_v1_1.base_lines.posted(unit.id)
            && !memory.jeff_v1_1.home_guard.holds(unit.id)
            && !memory.jeff_v1_1.rovers.holds(unit.id)
            && !memory.jeff_v1_1.front_line.yielded(unit.id)
    };
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && free(unit))
        .collect();
    let mut riflemen: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && free(unit))
        .collect();
    // The Riflemen nearest the main stay home.
    riflemen.sort_by(|a, b| {
        dist2(a.x, a.y, main.0, main.1)
            .total_cmp(&dist2(b.x, b.y, main.0, main.1))
            .then(a.id.cmp(&b.id))
    });
    // With the roaming force (`rovers`) the Riflemen are its, not the line's.
    let max_rifles = if super::rovers::active(memory) { 0 } else { MAX_RIFLES };
    let riflemen: Vec<&AiEntitySummary> = riflemen
        .into_iter()
        .skip(HOME_RIFLES)
        .take(max_rifles)
        .collect();
    if tanks.is_empty() {
        // No line without Tanks: the infantry goes back to V1.0's posts.
        memory.jeff_v1_1.front_line.members.clear();
        return;
    }
    let Some(axis) = normalized_direction(main, enemy_main) else {
        return;
    };
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let main_depot = depots
        .iter()
        .copied()
        .find(|depot| dist2(depot.0, depot.1, main.0, main.1) <= squared(4.0 * ts));
    let across = (-axis.1, axis.0);
    // The frontier: Jeff's extractors, and the Steel and Oil at the sites it is about to take.
    let mut anchors: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| matches!(unit.kind, EntityKind::SteelMine | EntityKind::PumpJack))
        .filter(|unit| unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let sites: Vec<(f32, f32)> = [memory.expansion_security.site, memory.later_bases.site()]
        .into_iter()
        .flatten()
        .filter_map(|site| building_center(site, EntityKind::ResourceDepot, observation.map.tile_size))
        .collect();
    anchors.extend(
        observation
            .resources
            .iter()
            .filter(|node| matches!(node.kind, EntityKind::Steel | EntityKind::Oil))
            .filter(|node| {
                sites.iter().any(|site| {
                    dist2(node.x, node.y, site.0, site.1) <= squared(SITE_FIELD_TILES * ts)
                })
            })
            .map(|node| (node.x, node.y)),
    );
    let depth = |at: (f32, f32)| (at.0 - main.0) * axis.0 + (at.1 - main.1) * axis.1;
    let lateral = |at: (f32, f32)| (at.0 - main.0) * across.0 + (at.1 - main.1) * across.1;
    let frontier = anchors.iter().map(|at| depth(*at)).fold(f32::MIN, f32::max);
    if frontier == f32::MIN {
        return;
    }
    let front: Vec<&(f32, f32)> = anchors
        .iter()
        .filter(|at| depth(**at) >= frontier - FRONT_BAND_TILES * ts)
        .collect();
    let middle = front.iter().map(|at| lateral(**at)).sum::<f32>() / front.len().max(1) as f32;
    let strength: f32 = tanks
        .iter()
        .chain(riflemen.iter())
        .map(|unit| super::push::unit_value(unit.kind) as f32)
        .sum();
    // Walk the line.
    {
        let state = &mut memory.jeff_v1_1.front_line;
        let line_depth = frontier + (AHEAD_TILES - state.back as f32 / 10.0) * ts;
        let centre = (
            main.0 + axis.0 * line_depth + across.0 * middle,
            main.1 + axis.1 * line_depth + across.1 * middle,
        );
        let enemy_near: f32 = observation
            .visible_enemies
            .iter()
            .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
            .filter(|enemy| {
                dist2(enemy.x, enemy.y, centre.0, centre.1) <= squared(NEAR_LINE_TILES * ts)
            })
            .map(|enemy| super::push::unit_value(enemy.kind) as f32)
            .sum();
        let lost: f32 = state.losses.iter().map(|(_, value)| *value as f32).sum();
        let on_post = state
            .members
            .keys()
            .filter(|id| by_id.get(id).is_some_and(|unit| unit.state != AiEntityState::Move))
            .count();
        let settled = !state.members.is_empty()
            && on_post as f32 >= state.members.len() as f32 * SETTLED_SHARE;
        let main_hit = attack_value(observation, main_depot, ts);
        let base_hit = depots
            .iter()
            .map(|depot| attack_value(observation, Some(*depot), ts))
            .fold(0.0, f32::max);
        let base_attacked = !super::off("linehome")
            && (main_hit >= MAIN_BREAK_VALUE || base_hit >= strength * BASE_BREAK_SHARE);
        if !super::off("linebreak")
            && (base_attacked
                || lost >= strength * BREAK_LOSS_SHARE
                || enemy_near >= strength * BREAK_RATIO)
        {
            state.why = if main_hit >= MAIN_BREAK_VALUE && !super::off("linehome") {
                "main"
            } else if base_attacked {
                "base"
            } else if lost >= strength * BREAK_LOSS_SHARE {
                "losses"
            } else {
                "outnumbered"
            };
            state.broken_until = tick + BROKEN_TICKS;
            state.back = (MAX_BACK_TILES * 10.0) as u32;
            state.losses.clear();
        }
        if tick.saturating_sub(state.last_step) >= STEP_TICKS {
            if lost >= strength * FALL_LOSS_SHARE || enemy_near >= strength * FALL_RATIO {
                state.back = (state.back as f32 + STEP_TILES * 10.0).min(MAX_BACK_TILES * 10.0) as u32;
                state.last_step = tick;
                state.losses.clear();
            } else if settled
                && lost <= strength * HOLD_LOSS_SHARE
                && enemy_near <= strength * ADVANCE_RATIO
            {
                state.back = (state.back as f32 - STEP_TILES * 10.0).max(0.0) as u32;
                state.last_step = tick;
            }
        }
    }
    // Broken, the line lets its units go back to the bases' defence until it forms again.
    if tick < memory.jeff_v1_1.front_line.broken_until {
        let state = &mut memory.jeff_v1_1.front_line;
        state.members.clear();
        state.ordered.clear();
        state.tanks = 0;
        state.middle = None;
        return;
    }
    let line_depth = frontier + (AHEAD_TILES - memory.jeff_v1_1.front_line.back as f32 / 10.0) * ts;
    let direction = axis;
    let centre = (
        main.0 + axis.0 * line_depth + across.0 * middle,
        main.1 + axis.1 * line_depth + across.1 * middle,
    );
    // Lean toward the side where more enemy strength was seen lately.
    let side_threat = |side: f32| {
        super::push::threat_near(
            observation,
            memory,
            (
                centre.0 + across.0 * side * LEAN_TILES * ts,
                centre.1 + across.1 * side * LEAN_TILES * ts,
            ),
            12.0,
        )
    };
    let (left, right) = (side_threat(-1.0), side_threat(1.0));
    let lean = if left > right * 1.5 {
        -LEAN_TILES
    } else if right > left * 1.5 {
        LEAN_TILES
    } else {
        0.0
    };
    let centre = clamp_to_map(
        (centre.0 + across.0 * lean * ts, centre.1 + across.1 * lean * ts),
        observation.map,
    );
    let params = LineParams::read();
    let groups = !super::off("linegroups");
    // Where the enemy last came at Jeff: fighters in sight near a Depot or the line, for a while.
    let near_ours = |x: f32, y: f32| {
        observation.owned.iter().any(|unit| {
            (unit.kind == EntityKind::ResourceDepot || memory.jeff_v1_1.front_line.holds(unit.id))
                && dist2(unit.x, unit.y, x, y) <= squared(CONTACT_TILES * ts)
        })
    };
    let attackers: Vec<(f32, f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| near_ours(enemy.x, enemy.y))
        .map(|enemy| (enemy.x, enemy.y))
        .collect();
    if !attackers.is_empty() {
        let n = attackers.len() as f32;
        let at = (
            (attackers.iter().map(|a| a.0).sum::<f32>() / n).round() as i32,
            (attackers.iter().map(|a| a.1).sum::<f32>() / n).round() as i32,
        );
        memory.jeff_v1_1.front_line.contact = Some((at, tick));
    }
    let contact = memory
        .jeff_v1_1
        .front_line
        .contact
        .filter(|(_, at)| tick.saturating_sub(*at) <= CONTACT_TICKS)
        .map(|((x, y), _)| (x as f32, y as f32));
    // The roaming reserve: the Tanks nearest where the enemy came, or the line's middle.
    let focus = contact.unwrap_or(centre);
    let mut tanks = tanks;
    tanks.sort_by(|a, b| {
        dist2(a.x, a.y, focus.0, focus.1)
            .total_cmp(&dist2(b.x, b.y, focus.0, focus.1))
            .then(a.id.cmp(&b.id))
    });
    let reserve_count = params.reserve.min(tanks.len().saturating_sub(1));
    let reserve: Vec<&AiEntitySummary> = tanks.drain(..reserve_count).collect();
    let mut assignments: Vec<(&AiEntitySummary, (f32, f32))> = Vec::new();
    match params.shape {
        Shape::PerBase => {
            // A segment in front of each base, two or more Tanks to a base, the most threatened first.
            let mut bases: Vec<Base> = observation
                .owned
                .iter()
                .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
                .map(|unit| (unit.x, unit.y))
                .chain(sites.iter().copied())
                .map(|at| {
                    let field: Vec<(f32, f32)> = anchors
                        .iter()
                        .copied()
                        .filter(|a| dist2(a.0, a.1, at.0, at.1) <= squared(BASE_FIELD_TILES * ts))
                        .collect();
                    let front = field.iter().map(|a| depth(*a)).fold(depth(at), f32::max);
                    let side = if field.is_empty() {
                        lateral(at)
                    } else {
                        field.iter().map(|a| lateral(*a)).sum::<f32>() / field.len() as f32
                    };
                    let threat = super::push::threat_near(observation, memory, at, 25.0);
                    Base { at, front, side, threat, contact: 0.0 }
                })
                .collect();
            if bases.len() > 1 {
                // The main has V1.0's pocket and home Tank.
                bases.retain(|base| dist2(base.at.0, base.at.1, main.0, main.1) > squared(8.0 * ts));
            }
            let back = memory.jeff_v1_1.front_line.back as f32 / 10.0;
            let segment_of = |base: &Base| {
                (
                    main.0 + axis.0 * (base.front + (AHEAD_TILES - back) * ts) + across.0 * base.side,
                    main.1 + axis.1 * (base.front + (AHEAD_TILES - back) * ts) + across.1 * base.side,
                )
            };
            // Grouped (`linegroups`): enemy fighters in sight at a base now weigh before the threat
            // remembered there, and draw the line's Tanks to it.
            if groups {
                for base in &mut bases {
                    let segment = segment_of(base);
                    let value: f32 = observation
                        .visible_enemies
                        .iter()
                        .filter(|enemy| {
                            enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker
                        })
                        .filter(|enemy| {
                            dist2(enemy.x, enemy.y, base.at.0, base.at.1) <= squared(ROLL_TILES * ts)
                                || dist2(enemy.x, enemy.y, segment.0, segment.1)
                                    <= squared(ROLL_TILES * ts)
                        })
                        .map(|enemy| super::push::unit_value(enemy.kind) as f32)
                        .sum();
                    base.contact = if value >= ROLL_MIN_VALUE { value } else { 0.0 };
                }
            }
            bases.sort_by(|a, b| {
                b.contact
                    .total_cmp(&a.contact)
                    .then(b.threat.total_cmp(&a.threat))
                    .then(b.front.total_cmp(&a.front))
            });
            let per_base = if groups { GROUP_TANKS } else { 2 };
            let covered = bases.len().min((tanks.len() / per_base).max(1));
            bases.truncate(covered);
            let share_of = |total: usize, index: usize| total / covered + usize::from(index < total % covered);
            let mut tank_counts: Vec<usize> = (0..covered).map(|index| share_of(tanks.len(), index)).collect();
            // Support rolls in: the base in contact takes Tanks from the others until it has
            // `ROLL_MATCH` of the enemy strength there.
            if groups && covered > 1 && bases[0].contact > 0.0 {
                let tank_value = super::push::unit_value(EntityKind::Tank) as f32;
                let need = (bases[0].contact * ROLL_MATCH / tank_value).ceil() as usize;
                let first = need.clamp(tank_counts[0], tanks.len());
                let rest = tanks.len() - first;
                let others = covered - 1;
                tank_counts[0] = first;
                for (index, count) in tank_counts.iter_mut().enumerate().skip(1) {
                    *count = rest / others + usize::from(index - 1 < rest % others);
                }
            }
            let tank_spacing = if groups { GROUP_SPACING_TILES } else { params.spacing };
            let mut left_tanks: Vec<&AiEntitySummary> = tanks.clone();
            let mut left_rifles: Vec<&AiEntitySummary> = riflemen.clone();
            for (index, base) in bases.iter().enumerate() {
                let share = |total: usize| share_of(total, index);
                let segment = segment_of(base);
                for (pool, count, spacing, ahead) in [
                    (&mut left_tanks, tank_counts[index], tank_spacing, 0.0),
                    (&mut left_rifles, share(riflemen.len()), RIFLE_SPACING_TILES, RIFLE_AHEAD_TILES),
                ] {
                    pool.sort_by(|a, b| {
                        dist2(a.x, a.y, segment.0, segment.1)
                            .total_cmp(&dist2(b.x, b.y, segment.0, segment.1))
                            .then(a.id.cmp(&b.id))
                    });
                    let mine: Vec<&AiEntitySummary> = pool.drain(..count.min(pool.len())).collect();
                    let at = (segment.0 + axis.0 * ahead * ts, segment.1 + axis.1 * ahead * ts);
                    let slots = posts(observation, map_analysis, at, axis, mine.len(), spacing);
                    assignments.extend(seat(&mine, &slots, across));
                }
            }
        }
        Shape::Axis | Shape::Fit => {
            // One line; stretched, it spans the bases near the front.
            let span = front.iter().map(|at| lateral(**at)).fold(f32::MIN, f32::max)
                - front.iter().map(|at| lateral(**at)).fold(f32::MAX, f32::min);
            let spacing = if params.shape == Shape::Fit && tanks.len() > 1 {
                ((span / ts + FIT_MARGIN_TILES) / (tanks.len() - 1) as f32)
                    .clamp(params.spacing, MAX_FIT_SPACING_TILES)
            } else {
                params.spacing
            };
            let rifle_centre = (
                centre.0 + direction.0 * RIFLE_AHEAD_TILES * ts,
                centre.1 + direction.1 * RIFLE_AHEAD_TILES * ts,
            );
            let tank_posts = posts(observation, map_analysis, centre, direction, tanks.len(), spacing);
            let rifle_posts = posts(
                observation,
                map_analysis,
                rifle_centre,
                direction,
                riflemen.len(),
                RIFLE_SPACING_TILES,
            );
            assignments.extend(seat(&tanks, &tank_posts, across));
            assignments.extend(seat(&riflemen, &rifle_posts, across));
        }
    }
    // The reserve stands behind the line where the enemy came, or behind its middle.
    if !reserve.is_empty() {
        let behind = |at: (f32, f32)| {
            clamp_to_map(
                (at.0 - axis.0 * RESERVE_BACK_TILES * ts, at.1 - axis.1 * RESERVE_BACK_TILES * ts),
                observation.map,
            )
        };
        let anchor = contact
            .and_then(|contact| {
                assignments
                    .iter()
                    .filter(|(unit, _)| unit.kind == EntityKind::Tank)
                    .map(|(_, post)| *post)
                    .min_by(|a, b| {
                        dist2(a.0, a.1, contact.0, contact.1)
                            .total_cmp(&dist2(b.0, b.1, contact.0, contact.1))
                    })
            })
            .unwrap_or(centre);
        let slots = posts(observation, map_analysis, behind(anchor), axis, reserve.len(), RESERVE_SPACING_TILES);
        assignments.extend(seat(&reserve, &slots, across));
    }
    let charging = memory.jeff_v1_1.charge.riflemen.clone();
    let state = &mut memory.jeff_v1_1.front_line;
    state.middle = Some((centre.0.round() as i32, centre.1.round() as i32));
    state.members = assignments
        .iter()
        .map(|(unit, _)| (unit.id, super::push::unit_value(unit.kind)))
        .collect();
    state.tanks = assignments
        .iter()
        .filter(|(unit, _)| unit.kind == EntityKind::Tank)
        .count();
    state.ordered.retain(|id, _| state.members.contains_key(id));
    let mut sent = Vec::new();
    for (unit, post) in assignments {
        if charging.contains(&unit.id) {
            continue;
        }
        let fighting = observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && enemy.kind != EntityKind::Worker
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(FIGHT_TILES * ts)
        });
        if fighting {
            continue;
        }
        // Grouped, a Tank in a parked enemy Tank's reach stays parked unless far off its post:
        // moving drops its own reach to 5 tiles.
        let off_post = dist2(unit.x, unit.y, post.0, post.1);
        if groups
            && unit.kind == EntityKind::Tank
            && off_post <= squared(LEASH_TILES * ts)
            && observation.visible_enemies.iter().any(|enemy| {
                enemy.kind == EntityKind::Tank
                    && enemy.hp > 0
                    && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(ENGAGED_TILES * ts)
            })
        {
            continue;
        }
        let on_post_tiles = if groups { GROUP_ON_POST_TILES } else { 1.5 };
        let near = off_post <= squared(on_post_tiles * ts);
        if near {
            if unit.state == AiEntityState::Move {
                actions::hold_position_units(actions, [unit.id]);
            }
            continue;
        }
        if state
            .ordered
            .get(&unit.id)
            .is_some_and(|at| tick.saturating_sub(*at) < REORDER_TICKS)
        {
            continue;
        }
        actions::attack_move_units(actions, [unit.id], post.0, post.1);
        state.ordered.insert(unit.id, tick);
        sent.push(unit.id);
    }
    if !sent.is_empty() {
        intents.push(AiIntent::Assemble { units: sent });
    }
}

/// A base the line stands in front of: its Depot or site, how far toward the enemy its extractors
/// reach, where they sit across the way, and the enemy strength remembered near it.
struct Base {
    at: (f32, f32),
    front: f32,
    side: f32,
    threat: f32,
    /// Enemy fighters in sight there now, by value (`linegroups`).
    contact: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    /// One line in front of the frontier.
    Axis,
    /// One line stretched across the bases near the front.
    Fit,
    /// A segment in front of each base.
    PerBase,
}

/// The line's shape, its Tanks' spacing and its roaming reserve, set by `JEFF_V11_LINE` for
/// testing (`shape=0|1|2,spacing=8,reserve=2,cover=2`).
struct LineParams {
    shape: Shape,
    spacing: f32,
    reserve: usize,
}

impl LineParams {
    fn read() -> Self {
        let param = crate::ai_core::profiles::jeff_v1_1_param;
        Self {
            shape: match param("shape").map(|value| value as u32) {
                Some(1) => Shape::Fit,
                Some(2) => Shape::PerBase,
                _ => Shape::Axis,
            },
            spacing: param("spacing").unwrap_or(TANK_SPACING_TILES),
            reserve: param("reserve").map_or(0, |value| value as usize),
        }
    }
}

/// Whether the line has the Tanks to cover the next base: `cover` (`JEFF_V11_LINE`, testing) for
/// each base past the main once it is taken. Without it, or without the line, any time.
pub(in crate::ai_core::decision) fn covers_next_base(memory: &AiDecisionMemory, depots: usize) -> bool {
    let cover = crate::ai_core::profiles::jeff_v1_1_param("cover").map_or(0, |value| value as usize);
    !active(memory) || cover == 0 || memory.jeff_v1_1.front_line.tanks >= cover * depots
}

/// `count` posts across `direction` through `centre`, `spacing` tiles apart, each pulled back
/// toward the line's own side until the ground is open.
fn posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    centre: (f32, f32),
    direction: (f32, f32),
    count: usize,
    spacing: f32,
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let across = (-direction.1, direction.0);
    (0..count)
        .map(|index| {
            let side = (index as f32 - (count as f32 - 1.0) * 0.5) * spacing;
            let mut back = 0.0;
            loop {
                let point = clamp_to_map(
                    (
                        centre.0 + (across.0 * side - direction.0 * back) * ts,
                        centre.1 + (across.1 * side - direction.1 * back) * ts,
                    ),
                    observation.map,
                );
                if back >= 6.0
                    || defense::defensive_position_is_open(observation, map_analysis, point.0, point.1)
                {
                    break point;
                }
                back += 1.0;
            }
        })
        .collect()
}

/// Units to posts, both ordered across the line so none crosses another.
fn seat<'u>(
    units: &[&'u AiEntitySummary],
    posts: &[(f32, f32)],
    across: (f32, f32),
) -> Vec<(&'u AiEntitySummary, (f32, f32))> {
    let lateral = |x: f32, y: f32| x * across.0 + y * across.1;
    let mut units = units.to_vec();
    units.sort_by(|a, b| lateral(a.x, a.y).total_cmp(&lateral(b.x, b.y)).then(a.id.cmp(&b.id)));
    let mut posts = posts.to_vec();
    posts.sort_by(|a, b| lateral(a.0, a.1).total_cmp(&lateral(b.0, b.1)));
    units.into_iter().zip(posts).collect()
}

/// Enemy fighters in sight within `BASE_ATTACK_TILES` of `depot`, by value.
fn attack_value(observation: &AiObservation, depot: Option<(f32, f32)>, ts: f32) -> f32 {
    let Some(depot) = depot else {
        return 0.0;
    };
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| dist2(enemy.x, enemy.y, depot.0, depot.1) <= squared(BASE_ATTACK_TILES * ts))
        .map(|enemy| super::push::unit_value(enemy.kind) as f32)
        .sum()
}
