//! `pfsafe`: how a resting or spent Panzerfaust walks past enemy Tanks (see the parent module
//! docs).

use super::*;

/// `pfsafe`: a loaded Panzerfaust this close to an enemy Tank fires instead of backing off.
const POINT_BLANK_TILES: f32 = 5.5;

/// `pfsafe`: what a resting or spent Panzerfaust on its way to `place` does instead of walking on.
enum Step {
    Go,
    Hold,
    Flee((f32, f32)),
    Fire(u32),
}

fn safe_step(
    observation: &AiObservation,
    unit: &AiEntitySummary,
    place: (f32, f32),
    tanks: &[&AiEntitySummary],
    loaded: bool,
) -> Step {
    if !safe() {
        return Step::Go;
    }
    let ts = observation.map.tile_size as f32;
    let nearest = tanks
        .iter()
        .min_by(|a, b| {
            dist2(unit.x, unit.y, a.x, a.y)
                .total_cmp(&dist2(unit.x, unit.y, b.x, b.y))
                .then(a.id.cmp(&b.id))
        })
        .copied();
    if let Some(tank) = nearest {
        let d2 = dist2(unit.x, unit.y, tank.x, tank.y);
        if loaded && d2 <= squared(POINT_BLANK_TILES * ts) {
            return Step::Fire(tank.id);
        }
        if d2 <= squared(SAFE_TILES * ts) {
            let away = unit_vector((tank.x, tank.y), (unit.x, unit.y)).unwrap_or((0.0, 1.0));
            return Step::Flee(clamp_to_map(
                (
                    unit.x + away.0 * FLEE_TILES * ts,
                    unit.y + away.1 * FLEE_TILES * ts,
                ),
                observation.map,
            ));
        }
    }
    let clear = tanks.iter().all(|tank| {
        segment_dist2((unit.x, unit.y), place, (tank.x, tank.y)) > squared(PATH_CLEAR_TILES * ts)
    });
    if clear {
        Step::Go
    } else {
        Step::Hold
    }
}

/// `pfsafe`: sends `unit` to `place` the safe way (see `safe_step`).
#[allow(clippy::too_many_arguments)]
pub(super) fn safe_send(
    commands: &mut Vec<Command>,
    pack: &mut FaustPackMemory,
    observation: &AiObservation,
    unit: &AiEntitySummary,
    place: (f32, f32),
    tanks: &[&AiEntitySummary],
    loaded: bool,
) {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    match safe_step(observation, unit, place, tanks, loaded) {
        Step::Go => send(commands, pack, unit, place, tick, ts),
        Step::Flee(at) => send(commands, pack, unit, at, tick, ts),
        Step::Hold => {
            if unit.state == AiEntityState::Move {
                commands.push(Command::HoldPosition {
                    units: vec![unit.id],
                    queued: false,
                });
            }
            pack.sent.remove(&unit.id);
        }
        Step::Fire(target) => {
            if unit.target_id != Some(target) {
                commands.push(Command::Attack {
                    units: vec![unit.id],
                    target,
                    queued: false,
                });
            }
            pack.sent.remove(&unit.id);
        }
    }
}
