//! When the scouts leave: so that they reach the target's staging point when a Scout Car leaving
//! at 5:30 did, the arrival the raid was tried with. Both walks are timed over the same legs by the
//! sim's movement rules.
//!
//! Over the flank route to each tested map's third the model gives a Scout Car 32-59 s and a
//! Rifleman 47-87 s (Rifleman 1.6 pixels a tick against the Scout Car's 2.35), so the scouts leave
//! 15-28 s before 5:30. Riflemen ordered along the route in the sim arrived within 4% of the model
//! (Classic 50.1 s against 50.0, Crossroads 77.7 against 79.1, The River 72.3 against 75.1, Schone
//! Tage 83.4 against 80.2, Doppelganger 76.2 against 75.0). Walks around enemies on the way are not
//! foreseen.

use super::*;

/// The Scout Car left at 5:30.
const CAR_LAUNCH_TICK: u32 = config::TICK_HZ * 330;
/// Never before 4:00: AI 2.1's third stands by 3:30-3:50.
pub(super) const EARLIEST_SCOUT_TICK: u32 = config::TICK_HZ * 240;
/// Without a map analysis to time the walk, the scouts leave at 5:00.
const UNTIMED_SCOUT_TICK: u32 = config::TICK_HZ * 300;

/// The departure is worked out again this often, as the lead to be may have walked elsewhere.
const RETIME_TICKS: u32 = config::TICK_HZ * 10;

/// When the scouts leave for the base Jeff would raid now, the lead walking from `from`.
pub(super) fn departure(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    goal: (f32, f32),
    from: (f32, f32),
) -> u32 {
    let key = rounded(goal);
    let now = observation.tick;
    if let Some((at, tick, timed)) = memory.third_pressure.departure {
        if at == key && now.saturating_sub(timed) < RETIME_TICKS {
            return tick;
        }
    }
    let tick = match map_analysis {
        Some(analysis) => {
            let route = flank_route(observation, Some(analysis), from, goal);
            let rifle = raid_walk_ticks(observation, analysis, from, &route, EntityKind::Rifleman);
            let car = raid_walk_ticks(observation, analysis, from, &route, EntityKind::ScoutCar);
            let tick = CAR_LAUNCH_TICK
                .saturating_add(car)
                .saturating_sub(rifle)
                .max(EARLIEST_SCOUT_TICK);
            memory.third_pressure.notes.push(format!(
                "scouts leave at tick {tick} for {key:?}: Riflemen walk {rifle} ticks, a Scout Car {car}"
            ));
            tick
        }
        None => UNTIMED_SCOUT_TICK,
    };
    memory.third_pressure.departure = Some((key, tick, now));
    tick
}

/// How many ticks a `kind` takes from `from` to the end of `route` walked as the raid walks it: to
/// each waypoint, on to the next once within `LEG_REACHED_TILES` of it, done within that of the
/// last; straight where the line keeps to open ground, else along the ground path; timed by the
/// sim's movement rules (`AiMapAnalysis::walk_ticks`).
pub(super) fn raid_walk_ticks(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    from: (f32, f32),
    route: &[(f32, f32)],
    kind: EntityKind,
) -> u32 {
    let ts = tile(observation);
    let meth = rts_rules::is_rifle_infantry(kind)
        && observation
            .upgrades
            .contains(&UpgradeKind::Methamphetamines);
    let speed = config::unit_stats(kind).map_or(0.0, |stats| stats.speed)
        * if meth {
            rts_rules::balance::METHAMPHETAMINES_SPEED_MULTIPLIER
        } else {
            1.0
        };
    let mut path = Vec::new();
    let mut here = from;
    for waypoint in route {
        let back = (LEG_REACHED_TILES * ts).min(distance(here, *waypoint));
        let end = normalized_direction(*waypoint, here).map_or(*waypoint, |dir| {
            (waypoint.0 + dir.0 * back, waypoint.1 + dir.1 * back)
        });
        if analysis.line_is_open(here, end) {
            path.push(end);
        } else {
            path.extend(analysis.compact_group_route(here, end, 1));
        }
        here = end;
    }
    analysis.walk_ticks(from, &path, speed)
}
