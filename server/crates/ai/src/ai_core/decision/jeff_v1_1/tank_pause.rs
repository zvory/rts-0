//! Jeff's AI V1.1 (`tankpause`) keeps its marching push's Tanks behind its Riflemen. The push
//! moved on as soon as its Tanks reached each waypoint, so its Riflemen trailed: with the Riflemen
//! ordered in front (`column`), 81% of them still walked behind the front Tank against Pressure AI,
//! and 86% of the Panzerfaust shots that hit a push Tank came with no Rifleman in front of it.
//!
//! A push Tank that reaches the back of its Riflemen (the rearmost Rifleman near the push, measured
//! along the march) stops for `PAUSE_TICKS` and stays stopped while it is still there, so the
//! Riflemen walk on ahead; it marches on once they are in front again. A Tank stopped `WAIT_LIMIT_TICKS`
//! in a row marches on for `CATCH_UP_TICKS` regardless, so Riflemen held up behind never stall the
//! push.

use super::*;
use crate::ai_core::decision::geometry::dist2;

/// How long a Tank stops each time it reaches the back of the Riflemen.
const PAUSE_TICKS: u32 = config::TICK_HZ / 2;
/// A Tank this close behind the rearmost Rifleman, or past it, has reached the back.
const REACH_TILES: f32 = 1.0;
/// Riflemen this far from the push's Tanks have left it (`retain_nearby_rifle_escorts`) and do not count.
const NEAR_TILES: f32 = super::screen::COLUMN_KEEP_TILES;
/// A Tank stopped this long in a row marches on for `CATCH_UP_TICKS`.
const WAIT_LIMIT_TICKS: u32 = config::TICK_HZ * 6;
const CATCH_UP_TICKS: u32 = config::TICK_HZ * 3;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct TankPauseMemory {
    /// Stopped Tanks: until when, and since when they have stood.
    paused: BTreeMap<u32, (u32, u32)>,
    /// Tanks marching on after a long wait, until when.
    catching_up: BTreeMap<u32, u32>,
    /// Stops ordered so far, for the trace.
    stops: u32,
}

impl TankPauseMemory {
    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        format!("tankpause paused={} stops={}", self.paused.len(), self.stops)
    }
}

/// Whether this profile paces its push's Tanks behind its Riflemen.
pub(in crate::ai_core::decision) fn active(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("tankpause")
}

/// Paces the marching push (see the module docs). `slots` are the Tanks' formation slots, where a
/// Tank marches on to; `toward` is the march direction; `reissued` says whether the push ordered
/// its formation this decision, which moved any stopped Tank on again.
#[allow(clippy::too_many_arguments)]
pub(in crate::ai_core::decision) fn pace(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    slots: &[(u32, (f32, f32))],
    riflemen: &[u32],
    toward: (f32, f32),
    reissued: bool,
) {
    if !active(memory) {
        return;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.hp > 0 && slots.iter().any(|(id, _)| *id == unit.id))
        .collect();
    let pause = &mut memory.jeff_v1_1.tank_pause;
    // A pause left over from an earlier march, before the push stopped to hold, is dropped.
    pause.paused.retain(|id, (until, _)| {
        tick <= until.saturating_add(config::TICK_HZ) && tanks.iter().any(|tank| tank.id == *id)
    });
    pause.catching_up.retain(|id, until| *until > tick && tanks.iter().any(|tank| tank.id == *id));
    if tanks.is_empty() {
        return;
    }
    let center = (
        tanks.iter().map(|tank| tank.x).sum::<f32>() / tanks.len() as f32,
        tanks.iter().map(|tank| tank.y).sum::<f32>() / tanks.len() as f32,
    );
    let along = |x: f32, y: f32| (x - center.0) * toward.0 + (y - center.1) * toward.1;
    let rear = observation
        .owned
        .iter()
        .filter(|unit| unit.hp > 0 && riflemen.contains(&unit.id))
        .filter(|unit| dist2(unit.x, unit.y, center.0, center.1) <= (NEAR_TILES * ts).powi(2))
        .map(|unit| along(unit.x, unit.y))
        .min_by(f32::total_cmp);
    let mut stop = Vec::new();
    let mut resume = Vec::new();
    for tank in &tanks {
        if pause.catching_up.contains_key(&tank.id) {
            continue;
        }
        let at_back = rear.is_some_and(|rear| along(tank.x, tank.y) >= rear - REACH_TILES * ts);
        match pause.paused.get(&tank.id).copied() {
            Some((until, _)) if tick < until => {
                // The push's formation order this decision moved it on: stop it again.
                if reissued {
                    stop.push(tank.id);
                }
            }
            Some((_, since)) => {
                let waited = tick.saturating_sub(since);
                if at_back && waited < WAIT_LIMIT_TICKS {
                    // Still at the back: another pause.
                    pause.paused.insert(tank.id, (tick + PAUSE_TICKS, since));
                    if reissued {
                        stop.push(tank.id);
                    }
                } else {
                    pause.paused.remove(&tank.id);
                    if at_back {
                        pause.catching_up.insert(tank.id, tick + CATCH_UP_TICKS);
                    }
                    resume.push(tank.id);
                }
            }
            None if at_back => {
                pause.paused.insert(tank.id, (tick + PAUSE_TICKS, tick));
                pause.stops += 1;
                stop.push(tank.id);
            }
            None => {}
        }
    }
    actions::hold_position_units(actions, stop);
    // A Tank marching on gets its formation slot again, unless the formation order just gave it.
    if !reissued {
        for id in resume {
            if let Some((_, slot)) = slots.iter().find(|(tank, _)| *tank == id) {
                actions::attack_move_units(actions, [id], slot.0, slot.1);
            }
        }
    }
}
