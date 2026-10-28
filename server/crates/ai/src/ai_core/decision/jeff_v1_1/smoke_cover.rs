//! Jeff's AI V1.1's Scout Car smokes the enemy Tanks shooting at its Tanks as they fall back. A
//! parked Tank reaches 14 tiles after 3 s standing; inside smoke it cannot fire out, and a shot or
//! sight line through a cloud is blocked, so a smoked Tank must move to fire again, and moving it
//! loses its range back to 5 tiles. The Scout Car's smoke reaches 14 tiles: two charges, each back
//! after 15 s, a 2-tile cloud for 5 s (4 tiles for 10 s with Smoke Plus). Raids against Pressure AI
//! lost 39 of their 44 Tanks on the way home, chased by its Tanks.
//!
//! An enemy Tank in sight, standing (not moving), within `REACH_TILES` of one of the Tanks covered
//! and within the smoke's reach of the Scout Car, and not already in a cloud, is smoked: the one
//! with the most covered Tanks in its reach first. Running and with no such Tank, a cloud goes
//! between the rearmost covered Tank and the nearest enemy Tank chasing it. At most one cloud every
//! `SMOKE_GAP_TICKS`.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};
use rts_rules::faction::AbilityKind;

const REACH_TILES: f32 = 14.5;
/// The smoke's reach from the Scout Car.
const SMOKE_RANGE_TILES: f32 = 13.5;
/// A cloud this close to a point already covers it.
const COVERED_TILES: f32 = 1.5;
const SMOKE_GAP_TICKS: u32 = config::TICK_HZ * 3 / 2;
/// Chasers this close to the rearmost Tank are cut off.
const CHASE_TILES: f32 = 14.0;

/// Whether this profile's Scout Cars smoke for its falling-back Tanks.
pub(in crate::ai_core::decision) fn active(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("smokecover")
}

/// Smokes for `tanks` with `scout` (see the module docs); `running` when they fall back. `last`:
/// when this cover last smoked. V1.1 only.
pub(in crate::ai_core::decision) fn cover(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    scout: Option<u32>,
    tanks: &[u32],
    running: bool,
    last: &mut u32,
) {
    if !active(memory) || observation.tick.saturating_sub(*last) < SMOKE_GAP_TICKS {
        return;
    }
    let ts = observation.map.tile_size as f32;
    let Some(car) = scout.and_then(|id| observation.owned.iter().find(|unit| unit.id == id && unit.hp > 0))
    else {
        return;
    };
    if !ready(observation, car.id) {
        return;
    }
    let ours: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id) && unit.hp > 0)
        .collect();
    if ours.is_empty() {
        return;
    }
    let covered = |at: (f32, f32)| {
        observation
            .smokes
            .iter()
            .any(|cloud| dist2(cloud.x, cloud.y, at.0, at.1) <= squared(COVERED_TILES * ts))
    };
    let in_range = |at: (f32, f32)| dist2(car.x, car.y, at.0, at.1) <= squared(SMOKE_RANGE_TILES * ts);
    let enemy_tanks: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .collect();
    let reaching = |enemy: &AiEntitySummary| {
        ours.iter()
            .filter(|unit| dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(REACH_TILES * ts))
            .count()
    };
    // A standing enemy Tank that reaches ours.
    let parked = enemy_tanks
        .iter()
        .filter(|enemy| enemy.state != AiEntityState::Move)
        .filter(|enemy| reaching(enemy) > 0)
        .filter(|enemy| in_range((enemy.x, enemy.y)) && !covered((enemy.x, enemy.y)))
        .max_by(|a, b| {
            reaching(a).cmp(&reaching(b)).then_with(|| {
                let near = |e: &AiEntitySummary| {
                    ours.iter()
                        .map(|unit| dist2(unit.x, unit.y, e.x, e.y))
                        .fold(f32::INFINITY, f32::min)
                };
                near(b).total_cmp(&near(a))
            })
        });
    let point = parked.map(|enemy| (enemy.x, enemy.y)).or_else(|| {
        if !running {
            return None;
        }
        // Between the rearmost Tank and its nearest chaser.
        let chasers: Vec<&&AiEntitySummary> = enemy_tanks
            .iter()
            .filter(|enemy| {
                ours.iter().any(|unit| {
                    dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(CHASE_TILES * ts)
                })
            })
            .collect();
        let (rear, chaser) = ours
            .iter()
            .flat_map(|unit| chasers.iter().map(move |enemy| (*unit, **enemy)))
            .min_by(|(a, x), (b, y)| dist2(a.x, a.y, x.x, x.y).total_cmp(&dist2(b.x, b.y, y.x, y.y)))?;
        let mid = ((rear.x + chaser.x) * 0.5, (rear.y + chaser.y) * 0.5);
        (in_range(mid) && !covered(mid)).then_some(mid)
    });
    let Some(point) = point else {
        return;
    };
    actions::use_world_ability(actions, car.id, AbilityKind::Smoke, point.0, point.1);
    *last = observation.tick;
}

/// The Scout Car has a smoke charge to use now.
fn ready(observation: &AiObservation, scout: u32) -> bool {
    observation.ability_states.iter().any(|ability| {
        ability.entity_id == scout
            && ability.kind == AbilityKind::Smoke
            && ability.cooldown_left == 0
            && ability.remaining_uses.is_none_or(|uses| uses > 0)
            && ability
                .available_tick
                .is_none_or(|tick| tick <= observation.tick)
            && ability
                .lockout_until_tick
                .is_none_or(|tick| tick <= observation.tick)
    })
}

/// How long the push's Tanks are covered after it falls back.
const RETREAT_TICKS: u32 = config::TICK_HZ * 20;

/// The push falling back: its Tanks and Scout Car until when, and when it last smoked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct SmokeMemory {
    retreat: Option<(Vec<u32>, Option<u32>, u32)>,
    last: u32,
}

/// Notes the push falling back with `tanks` and `scout`.
pub(in crate::ai_core::decision) fn note_retreat(
    memory: &mut AiDecisionMemory,
    tanks: &[u32],
    scout: Option<u32>,
    tick: u32,
) {
    if active(memory) {
        memory.jeff_v1_1.smoke.retreat = Some((tanks.to_vec(), scout, tick + RETREAT_TICKS));
    }
}

/// Smokes for the push's falling-back Tanks while the fall-back lasts.
pub(in crate::ai_core::decision) fn cover_push_retreat(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) {
    let Some((tanks, scout, until)) = memory.jeff_v1_1.smoke.retreat.clone() else {
        return;
    };
    if observation.tick >= until {
        memory.jeff_v1_1.smoke.retreat = None;
        return;
    }
    let mut last = memory.jeff_v1_1.smoke.last;
    cover(actions, observation, memory, scout, &tanks, true, &mut last);
    memory.jeff_v1_1.smoke.last = last;
}
