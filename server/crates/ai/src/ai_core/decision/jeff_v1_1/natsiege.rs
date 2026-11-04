//! Jeff's AI V1.1's natural siege (`natsiege`).
//!
//! The Tanks sent to clear enemy infantry off the natural attack-moved onto it. A moving Tank
//! reaches 5 tiles, so it fought inside the reach of the Machine Gunners (6.1) and Panzerfausts
//! there: against Rush AI on The River seven guns held the natural from before 3:48, the first Tank
//! drove in at 4:48 and was dead by 5:12, and the natural went down at 6:27 after the second
//! cleared it. A Tank standing still reaches 8 tiles after a second and 14 after three, and sees 9.
//!
//! Under `natsiege` a clearing Tank stops `SIEGE_TILES` from the nearest enemy fighter it sees
//! there, in sight of it and out of every infantry reach, and holds there firing; it backs off
//! when one comes within `BACK_OFF_TILES`, and with none in sight it closes on the spot it was sent
//! to until one is.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

fn is_fighter(kind: EntityKind) -> bool {
    kind.is_unit() && !matches!(kind, EntityKind::Worker | EntityKind::Golem)
}

/// The clearing Tank stands this far from the nearest enemy fighter it sees.
const SIEGE_TILES: f32 = 7.5;
/// It holds while the nearest is between `BACK_OFF_TILES` and this far.
const HOLD_MAX_TILES: f32 = 8.3;
/// An enemy fighter this close sends it back `BACK_STEP_TILES`.
const BACK_OFF_TILES: f32 = 6.4;
const BACK_STEP_TILES: f32 = 3.5;
/// Enemy fighters this close to the spot the Tank was sent to are the ones it sieges.
const AREA_TILES: f32 = 16.0;
/// A move order is sent again once its point has shifted this far.
const REISSUE_TILES: f32 = 1.5;

/// The order a clearing Tank last got: held, or moving to a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai_core::decision) enum SiegeOrder {
    Hold,
    Move((i32, i32)),
}

/// Whether this profile sieges its natural (`natsiege`).
pub(in crate::ai_core::decision) fn active(profile_id: &str) -> bool {
    is_jeff_v1_1(profile_id) && !super::off("natsiege")
}

/// The clearing Tanks keep at it this long after enemies were last seen on the natural: the
/// natural waits for its guards to take their posts, and out of sight the guns still hold them.
const CONTEST_MEMORY_TICKS: u32 = config::TICK_HZ * 30;

/// Whether the natural counts as held by the enemy for its clearing Tanks: seen held this decision,
/// or under `natsiege` within `CONTEST_MEMORY_TICKS`.
pub(in crate::ai_core::decision) fn contest_current(
    profile_id: Option<&str>,
    contested_tick: Option<u32>,
    tick: u32,
) -> bool {
    match contested_tick {
        Some(seen) if profile_id.is_some_and(active) => tick.saturating_sub(seen) <= CONTEST_MEMORY_TICKS,
        Some(seen) => seen == tick,
        None => false,
    }
}

/// The order for clearing Tank `tank` sent at `spot`, given the one it last got; `None` keeps it.
pub(in crate::ai_core::decision) fn order(
    observation: &AiObservation,
    tank: &AiEntitySummary,
    spot: (f32, f32),
    last: Option<SiegeOrder>,
) -> Option<(SiegeOrder, Command)> {
    let ts = observation.map.tile_size as f32;
    let nearest = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && is_fighter(enemy.kind) && enemy.kind != EntityKind::Tank)
        .filter(|enemy| dist2(enemy.x, enemy.y, spot.0, spot.1) <= squared(AREA_TILES * ts))
        .min_by(|a, b| {
            dist2(a.x, a.y, tank.x, tank.y).total_cmp(&dist2(b.x, b.y, tank.x, tank.y))
        });
    let toward = |from: (f32, f32), to: (f32, f32), tiles: f32| {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        (from.0 + dx / len * tiles * ts, from.1 + dy / len * tiles * ts)
    };
    let point = match nearest {
        Some(enemy) => {
            let distance = dist2(enemy.x, enemy.y, tank.x, tank.y).sqrt() / ts;
            if distance < BACK_OFF_TILES {
                // Straight away from it.
                toward((tank.x, tank.y), (2.0 * tank.x - enemy.x, 2.0 * tank.y - enemy.y), BACK_STEP_TILES)
            } else if distance <= HOLD_MAX_TILES {
                if last == Some(SiegeOrder::Hold) {
                    return None;
                }
                return Some((
                    SiegeOrder::Hold,
                    Command::HoldPosition {
                        units: vec![tank.id],
                        queued: false,
                    },
                ));
            } else {
                // Stand `SIEGE_TILES` short of it, on the Tank's side.
                toward((enemy.x, enemy.y), (tank.x, tank.y), SIEGE_TILES)
            }
        }
        // None in sight: drive on toward the spot. The Tank sees 9 tiles against a gun's 6.1, so it
        // stops to fire on the first it sees before that one can reach it. Stopping short of the spot
        // left it looking at nothing while seven guns held the site 16 tiles off.
        None => spot,
    };
    let key = (point.0.round() as i32, point.1.round() as i32);
    if let Some(SiegeOrder::Move(prev)) = last {
        let shifted = dist2(prev.0 as f32, prev.1 as f32, point.0, point.1) > squared(REISSUE_TILES * ts);
        if !shifted && tank.state != AiEntityState::Idle {
            return None;
        }
    }
    Some((
        SiegeOrder::Move(key),
        Command::Move {
            units: vec![tank.id],
            x: point.0,
            y: point.1,
            queued: false,
        },
    ))
}
