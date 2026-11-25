//! `fieldgate`: Jeff's AI V1.1 takes another base past the dry-well count when its fields are about
//! to run short (A) and its army can spare the escort (B). In the 120 the tick-cap draws ended with
//! Jeff's army ahead but his extractors behind: against Pressure AI a median of 16 to 24, his main's
//! and natural's Steel mined out while the dry-well gate held him to three bases until ~10:50, and a
//! new base takes 4-5 minutes to reach full output. Lifting the gate outright (`nowellgate`, base 4
//! at 5:30) cost the army and lost games, so the extra base waits for both:
//!
//! - **A, fields running short (exact):** of the Steel and Oil nodes at Jeff's complete Depots, fewer
//!   than `fgkeep` (75%) of the most there have been will still hold `fgmin` (4) minutes of mining.
//! - **B, the army can spare it (estimated):** Jeff's army value is `fgarmy` (2.5) times the enemy
//!   fighters he remembers (the remembered army ran a median of ~60% of the real one in the draws;
//!   floored at `fgfloor`), he killed at least the value he lost over the last 3 minutes, and no
//!   enemy fighter was seen within `HOME_TILES` of his Depots in the last minute.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

/// Mined per extractor per tick (`STEEL_LOAD` and `OIL_LOAD` every `HARVEST_TICKS`).
const MINED_PER_TICK: f32 = 2.0 / 40.0;
const LOOKAHEAD_MINUTES: f32 = 4.0;
const KEEP_SHARE: f32 = 0.75;
const ARMY_FACTOR: f32 = 2.5;
/// The enemy army is never taken for less than this, so the gate also needs Jeff's army at `fgarmy`
/// times it (10,000, about 13 Tanks): at 1,000 base 4 came at 6:23 against Pressure AI and the army
/// stalled; at 4,000 it came at 10:44 and Jeff ended 28 extractors to 15 (16 smoke games).
const ENEMY_FLOOR: f32 = 4000.0;
/// The trade is judged over this window, from marks taken this often.
const TRADE_WINDOW_TICKS: u32 = config::TICK_HZ * 180;
const MARK_TICKS: u32 = config::TICK_HZ * 15;
/// Enemy fighters seen this close to a Depot this recently make the home not quiet.
const HOME_TILES: f32 = 20.0;
const HOME_QUIET_TICKS: u32 = config::TICK_HZ * 60;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct FieldGateMemory {
    /// The most live nodes Jeff's Depots have had.
    peak: u32,
    /// (tick, value killed, value lost), one every `MARK_TICKS`.
    marks: Vec<(u32, u32, u32)>,
    /// Diagnostic: the last evaluation of A and B, and what later bases waited on.
    pub(in crate::ai_core::decision) note: String,
    pub(in crate::ai_core::decision) wait: String,
    /// Diagnostic: later-base attempts given up, as `reason@x,y@tick`.
    pub(in crate::ai_core::decision) abandons: Vec<String>,
}

impl FieldGateMemory {
    /// Trace tokens for the strike line.
    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        let recent = &self.abandons[self.abandons.len().saturating_sub(3)..];
        format!("fg={} lb={} ab={}", self.note, self.wait, recent.join(";"))
    }

    pub(in crate::ai_core::decision) fn note_abandon(
        &mut self,
        reason: &str,
        site: (u32, u32),
        tick: u32,
    ) {
        self.abandons
            .push(format!("{reason}@{},{}@{tick}", site.0, site.1));
    }
}

fn param(name: &str, default: f32) -> f32 {
    crate::ai_core::profiles::jeff_v1_1_param(name).unwrap_or(default)
}

/// Whether another base is due by the field gate (see the module docs). Updates its memory every
/// decision, so call it every decision whether or not a base is otherwise due.
pub(in crate::ai_core::decision) fn base_due(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    mining_range_tiles: f32,
) -> bool {
    if !memory.profile_id().is_some_and(is_jeff_v1_1) || super::off("fieldgate") {
        return false;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    // A: the nodes at Jeff's Depots now, and those that will still be mining after the lookahead.
    let need = MINED_PER_TICK * param("fgmin", LOOKAHEAD_MINUTES) * 60.0 * config::TICK_HZ as f32;
    let (mut now, mut later) = (0_u32, 0_u32);
    for node in observation.resources.iter().filter(|node| {
        matches!(node.kind, EntityKind::Steel | EntityKind::Oil) && node.remaining > 0
    }) {
        if !depots.iter().any(|depot| {
            dist2(node.x, node.y, depot.0, depot.1) <= squared(mining_range_tiles * ts)
        }) {
            continue;
        }
        now += 1;
        if node.remaining as f32 >= need {
            later += 1;
        }
    }
    let state = &mut memory.jeff_v1_1.field_gate;
    state.peak = state.peak.max(now);
    let short = state.peak > 0 && (later as f32) < param("fgkeep", KEEP_SHARE) * state.peak as f32;
    // B: the trade over the window.
    let (killed, lost) = (memory.jeff_v1_1.value_killed, memory.jeff_v1_1.value_lost);
    let state = &mut memory.jeff_v1_1.field_gate;
    if state
        .marks
        .last()
        .is_none_or(|mark| tick.saturating_sub(mark.0) >= MARK_TICKS)
    {
        state.marks.push((tick, killed, lost));
    }
    state
        .marks
        .retain(|mark| tick.saturating_sub(mark.0) <= TRADE_WINDOW_TICKS + MARK_TICKS);
    let start = state
        .marks
        .iter()
        .find(|mark| tick.saturating_sub(mark.0) >= TRADE_WINDOW_TICKS)
        .or(state.marks.first())
        .copied()
        .unwrap_or((tick, killed, lost));
    let trading = killed.saturating_sub(start.1) >= lost.saturating_sub(start.2);
    // B: the army against the one remembered, and a quiet home.
    let ours: u32 = observation
        .owned
        .iter()
        .filter(|unit| {
            unit.is_complete
                && unit.hp > 0
                && unit.kind.is_unit()
                && !matches!(unit.kind, EntityKind::Worker | EntityKind::Golem)
        })
        .map(|unit| push::unit_value(unit.kind))
        .fold(0_u32, u32::saturating_add);
    let theirs = (push::remembered_army(memory) as f32).max(param("fgfloor", ENEMY_FLOOR));
    let strong = ours as f32 >= param("fgarmy", ARMY_FACTOR) * theirs;
    let quiet = !push::army_seen_near(
        memory,
        &depots,
        HOME_TILES * ts,
        tick.saturating_sub(HOME_QUIET_TICKS),
    );
    let due = short && trading && strong && quiet;
    let peak = memory.jeff_v1_1.field_gate.peak;
    memory.jeff_v1_1.field_gate.note = format!(
        "{}:A{}({}/{})T{}S{}({}/{})Q{}",
        u8::from(due),
        u8::from(short),
        later,
        peak,
        u8::from(trading),
        u8::from(strong),
        ours,
        theirs as u32,
        u8::from(quiet)
    );
    due
}
