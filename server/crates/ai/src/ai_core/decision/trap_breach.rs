//! Opening Tank Trap lines that close the live Jeff's way to the enemy. A completed trap is a
//! neutral obstacle whether the map placed it or a player built it, and it blocks vehicles, while
//! the path planner may still route Tanks into a line they cannot pass (diagonal trap lines look
//! porous to it), where they stick. Schone Tage's V is such a line; an opponent can build others.
//!
//! The breach finds the trap lines across Jeff's route to the enemy and takes each one down, all
//! of it, from Jeff's side: Tanks stand at posts within sight of a trap (so it can be targeted),
//! outside the reach of every enemy Tank Jeff knows of, charge, and shoot. The safest trap goes
//! first, so a line opens from the end away from the enemy army. Then the Tanks hold the opening
//! charged for a while and go back to the push, where the Tank micro takes over the open way.
//! Everything is shot down by Tanks; Workers stay home.

use super::geometry::{dist2, squared};
use super::opening_rush::LiveStep;
use super::*;

const START_TANKS: usize = 8;
/// At most this many Tanks join the breach; the rest stay with the regular plans.
const MAX_TANKS: usize = 12;
/// Traps this close to the route block it...
const ROUTE_TILES: f32 = 3.0;
/// ...and every trap linked to one of them this closely belongs to the same line.
const LINE_LINK_TILES: f32 = 3.3;
/// Posts: this far from the target, within a Tank's 9-tile sight so no spotter is needed.
const POST_RINGS: [f32; 3] = [7.0, 8.0, 6.0];
const POST_SPACING_TILES: f32 = 2.2;
const MAX_POSTS: usize = 6;
/// A post is on Jeff's side of the line when the straight way from it back toward Jeff passes no
/// standing trap closer than this; the reference point lies this far back along the route.
const TRAP_CLEARANCE_TILES: f32 = 1.2;
const JEFF_SIDE_BACK_TILES: f32 = 8.0;
/// Posts stay this far from every enemy Tank seen lately: beyond its full range and a margin.
const ENEMY_TANK_CLEARANCE_TILES: f32 = 16.0;
const ENEMY_TANK_MEMORY_TICKS: u32 = config::TICK_HZ * 30;
/// A Tank's range grows from 5 to 14 tiles in its first three seconds standing; 8 tiles takes
/// one second.
const AIM_TICKS: u32 = 45;
const IN_POSITION_TILES: f32 = 1.5;
const ORDER_REFRESH_TICKS: u32 = config::TICK_HZ * 2;
const TANK_SIGHT_TILES: f32 = 8.5;
const SCOUT_SIGHT_TILES: f32 = 14.5;
/// A trap no one can shoot safely is passed over for this long.
const SKIP_TICKS: u32 = config::TICK_HZ * 20;
/// A target not cleared in this long is set aside for another.
const TARGET_TIMEOUT_TICKS: u32 = config::TICK_HZ * 25;
/// With no trap down or safe to shoot for this long, the breach stops and hands the Tanks back.
const STALL_TICKS: u32 = config::TICK_HZ * 90;
/// Once the lines are open the Tanks hold the way charged for at least this long, and until no
/// enemy Tank came near for the quiet time.
const MIN_HOLD_TICKS: u32 = config::TICK_HZ * 45;
const QUIET_TICKS: u32 = config::TICK_HZ * 60;
const NEAR_TILES: f32 = 25.0;
/// After each trap comes down the Tanks step back this far from the gap onto Jeff's side and
/// stand charged a while: whatever comes through the gap meets them at full range.
const REST_TILES: f32 = 12.0;
const REST_TICKS: u32 = config::TICK_HZ * 8;
/// They step back only when an enemy Tank is known this close to the new gap.
const REST_IF_ENEMY_TILES: f32 = 30.0;
/// Enemy Tanks this close to a breach Tank stop all moving: they stand and fight charged.
const DEFEND_TILES: f32 = 20.0;
/// Clearing resumes once no enemy Tank was this close for this long.
const DEFEND_CLEAR_TICKS: u32 = config::TICK_HZ * 10;
/// A target this far from the group costs as much as this much less margin from the enemy.
const TARGET_DISTANCE_WEIGHT: f32 = 0.5;
/// A Tank that gets no closer to its post for this long is stuck and is let go...
const STUCK_TICKS: u32 = config::TICK_HZ * 20;
/// ...and not taken again for this long.
const DROPPED_TICKS: u32 = config::TICK_HZ * 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    /// Trap lines across the route come down, one safe trap at a time.
    Clear,
    /// The opened way is held charged before the push takes over.
    Hold,
    Done,
}

/// The live Jeff's breach of trap lines across its route, kept across decisions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TrapBreach {
    phase: Phase,
    phase_since: u32,
    tanks: BTreeSet<u32>,
    /// Trap ids and positions (world pixels) seen and not yet seen gone.
    traps: BTreeMap<u32, (i32, i32)>,
    /// The trap being shot, since when, and where it stands.
    target: Option<(u32, u32)>,
    target_point: Option<(i32, i32)>,
    /// Traps passed over, since when.
    skipped: BTreeMap<u32, u32>,
    /// Each Tank's post for the current target, world pixels.
    posts: BTreeMap<u32, (i32, i32)>,
    /// When each Tank last stopped moving.
    still_since: BTreeMap<u32, u32>,
    last_order: Option<(u32, u32)>,
    /// When a trap last came down or was last safe to shoot.
    last_progress: u32,
    last_enemy_near: u32,
    /// Per Tank on its way: when it last got closer to its post, and how close (tiles).
    progress: BTreeMap<u32, (u32, u32)>,
    /// Tanks let go as stuck, and when.
    dropped: BTreeMap<u32, u32>,
    /// While Jeff's home is under attack before the shooting starts, the breach lets go.
    paused: bool,
    /// Stepping back after a trap came down, until then.
    rest_until: Option<u32>,
    /// When an enemy Tank last came close to the breach Tanks; standing to fight since.
    last_threat: u32,
    defending: bool,
    /// Tanks told to hold where they stand since they last moved.
    held: BTreeSet<u32>,
    /// This decision's trace notes.
    pub(super) notes: Vec<String>,
}

impl TrapBreach {
    /// Units the breach owns this decision.
    pub(super) fn reserved(&self) -> impl Iterator<Item = u32> + '_ {
        let active = !self.paused && matches!(self.phase, Phase::Clear | Phase::Hold);
        self.tanks.iter().copied().filter(move |_| active)
    }

    pub(super) fn is_reserved(&self, id: u32) -> bool {
        self.reserved().any(|unit| unit == id)
    }
}

pub(super) fn uses_trap_breach(profile_id: &str) -> bool {
    profile_id == JEFFS_AI_ID
}

fn world((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32, y as f32)
}

fn stored((x, y): (f32, f32)) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

/// What the breach knows of the field this decision: the blocking lines, the route from Jeff
/// toward the enemy, the enemy Tanks seen lately, and the reach of parked ones.
type Field<'a> = (
    &'a [Vec<u32>],
    &'a [(f32, f32)],
    &'a [(f32, f32)],
    &'a [super::tank_micro::Zone],
);

/// Runs the breach for the live Jeff.
pub(super) fn plan_live(
    profile_id: &str,
    memory: &mut AiDecisionMemory,
    (actions, observation, map_analysis, intents): LiveStep<'_, '_, '_, '_>,
) {
    if !uses_trap_breach(profile_id) {
        return;
    }
    memory.trap_breach.notes.clear();
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    remember_traps(observation, &mut memory.trap_breach, ts);
    // Jeff's route to the enemy, from Jeff's end; the analysis keeps it from the enemy's.
    let Some(route) = map_analysis
        .and_then(|analysis| analysis.base_route_tiles(observation.player_id))
        .map(|tiles| {
            tiles
                .iter()
                .rev()
                .map(|tile| ((tile.x as f32 + 0.5) * ts, (tile.y as f32 + 0.5) * ts))
                .collect::<Vec<_>>()
        })
        .filter(|route| route.len() >= 2)
    else {
        return;
    };
    let lines = blocking_lines(&memory.trap_breach.traps, &route, ts);
    if lines.is_empty() && matches!(memory.trap_breach.phase, Phase::Idle | Phase::Done) {
        return;
    }
    let enemy_tanks = recent_enemy_tanks(observation, &memory.tank_micro);
    let zones = super::tank_micro::zones(observation, &memory.tank_micro);
    let home_tank = memory.home_defensive_tank;
    let guards: BTreeSet<u32> = memory.later_bases.guards.iter().copied().collect();
    let home_attacked = defense::local_defense_contact(observation).is_some();
    let breach = &mut memory.trap_breach;
    let owned_ok = |id: &u32| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == *id && unit.hp > 0)
    };
    breach.tanks.retain(|id| owned_ok(id));
    breach.posts.retain(|id, _| owned_ok(id));
    if breach.phase == Phase::Done {
        // A new line across the route (an opponent may build one) starts a new breach.
        set_phase(breach, Phase::Idle, tick);
    }
    // Jeff's home under attack takes precedence until the shooting starts.
    breach.paused = breach.phase == Phase::Clear && breach.last_order.is_none() && home_attacked;
    if breach.paused {
        return;
    }
    let spare: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .filter(|unit| Some(unit.id) != home_tank && !guards.contains(&unit.id))
        .filter(|unit| !breach.tanks.contains(&unit.id))
        .filter(|unit| {
            breach
                .dropped
                .get(&unit.id)
                .is_none_or(|at| tick.saturating_sub(*at) >= DROPPED_TICKS)
        })
        .collect();
    if breach.phase == Phase::Idle {
        if spare.len() < START_TANKS || home_attacked {
            return;
        }
        breach.notes.push(format!(
            "start: {} trap lines across the route, {} traps",
            lines.len(),
            lines.iter().map(Vec::len).sum::<usize>()
        ));
        set_phase(breach, Phase::Clear, tick);
        breach.last_progress = tick;
        take_tanks(breach, &spare, &route);
        // The push that would stall at the line gives way to the breach.
        memory.containment = Default::default();
    } else if !breach.defending {
        take_tanks(breach, &spare, &route);
    }
    let breach = &mut memory.trap_breach;
    if breach.tanks.len() < 3 {
        breach.notes.push("too few Tanks left".to_string());
        set_phase(breach, Phase::Done, tick);
        return;
    }
    run(
        actions,
        observation,
        map_analysis,
        breach,
        (&lines, &route, &enemy_tanks, &zones),
        intents,
    );
}

/// Enemy Tanks seen lately: the visible ones and the Tank micro's memory.
fn recent_enemy_tanks(
    observation: &AiObservation,
    micro: &super::tank_micro::TankMicro,
) -> Vec<(f32, f32)> {
    let tick = observation.tick;
    micro
        .enemy_tanks
        .values()
        .filter(|seen| tick.saturating_sub(seen.last_seen) <= ENEMY_TANK_MEMORY_TICKS)
        .map(|seen| world(seen.pos))
        .chain(
            observation
                .visible_enemies
                .iter()
                .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
                .map(|enemy| (enemy.x, enemy.y)),
        )
        .collect()
}

fn set_phase(breach: &mut TrapBreach, phase: Phase, tick: u32) {
    breach.notes.push(format!(
        "phase {:?} -> {:?} after {} ticks, tanks={}",
        breach.phase,
        phase,
        tick.saturating_sub(breach.phase_since),
        breach.tanks.len()
    ));
    breach.phase = phase;
    breach.phase_since = tick;
    breach.target = None;
    breach.last_order = None;
    if matches!(phase, Phase::Done | Phase::Idle) {
        breach.tanks.clear();
        breach.posts.clear();
    }
}

/// Takes spare Tanks up to the cap, nearest to Jeff's end of the route first.
fn take_tanks(breach: &mut TrapBreach, spare: &[&AiEntitySummary], route: &[(f32, f32)]) {
    let anchor = route.first().copied().unwrap_or((0.0, 0.0));
    let mut spare: Vec<&AiEntitySummary> = spare.to_vec();
    spare.sort_by(|a, b| {
        dist2(a.x, a.y, anchor.0, anchor.1)
            .total_cmp(&dist2(b.x, b.y, anchor.0, anchor.1))
            .then_with(|| a.id.cmp(&b.id))
    });
    for tank in spare {
        if breach.tanks.len() >= MAX_TANKS {
            break;
        }
        breach.tanks.insert(tank.id);
    }
}

/// Keeps every trap seen, and forgets one once an own Tank or Scout Car sees its spot empty.
fn remember_traps(observation: &AiObservation, breach: &mut TrapBreach, ts: f32) {
    for trap in &observation.visible_tank_traps {
        if trap.hp > 0 {
            breach.traps.insert(trap.id, stored((trap.x, trap.y)));
        }
    }
    let visible: BTreeSet<u32> = observation
        .visible_tank_traps
        .iter()
        .filter(|trap| trap.hp > 0)
        .map(|trap| trap.id)
        .collect();
    breach.traps.retain(|id, pos| {
        if visible.contains(id) {
            return true;
        }
        let p = world(*pos);
        let watched = observation.owned.iter().any(|unit| {
            let sight = match unit.kind {
                EntityKind::ScoutCar => SCOUT_SIGHT_TILES,
                EntityKind::Tank => TANK_SIGHT_TILES,
                _ => 0.0,
            };
            sight > 0.0 && dist2(unit.x, unit.y, p.0, p.1) <= squared(sight * ts)
        });
        !watched
    });
}

/// The remembered trap lines across the route, nearest to Jeff first: every trap within
/// `ROUTE_TILES` of it, with all the traps linked to those.
fn blocking_lines(
    traps: &BTreeMap<u32, (i32, i32)>,
    route: &[(f32, f32)],
    ts: f32,
) -> Vec<Vec<u32>> {
    let near_route = |p: (f32, f32)| {
        route
            .iter()
            .position(|r| dist2(r.0, r.1, p.0, p.1) <= squared(ROUTE_TILES * ts))
    };
    let mut assigned: BTreeSet<u32> = BTreeSet::new();
    let mut lines: Vec<(usize, Vec<u32>)> = Vec::new();
    for (id, pos) in traps {
        if assigned.contains(id) || near_route(world(*pos)).is_none() {
            continue;
        }
        let mut line = vec![*id];
        assigned.insert(*id);
        let mut next = 0;
        while next < line.len() {
            let here = world(traps[&line[next]]);
            for (other, other_pos) in traps {
                let p = world(*other_pos);
                if !assigned.contains(other)
                    && dist2(here.0, here.1, p.0, p.1) <= squared(LINE_LINK_TILES * ts)
                {
                    assigned.insert(*other);
                    line.push(*other);
                }
            }
            next += 1;
        }
        let first = line
            .iter()
            .filter_map(|id| near_route(world(traps[id])))
            .min()
            .unwrap_or(usize::MAX);
        lines.push((first, line));
    }
    lines.sort_by_key(|(first, line)| (*first, line.first().copied()));
    lines.into_iter().map(|(_, line)| line).collect()
}

/// The route's heading toward the enemy near `point`.
fn route_heading(route: &[(f32, f32)], point: (f32, f32)) -> Option<(f32, f32)> {
    let index = (0..route.len()).min_by(|a, b| {
        dist2(route[*a].0, route[*a].1, point.0, point.1).total_cmp(&dist2(
            route[*b].0,
            route[*b].1,
            point.0,
            point.1,
        ))
    })?;
    let from = route[index.saturating_sub(3)];
    let to = route[(index + 3).min(route.len() - 1)];
    super::geometry::normalized_direction(from, to)
}

fn run(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    breach: &mut TrapBreach,
    field: Field<'_>,
    intents: &mut Vec<AiIntent>,
) {
    let (lines, route, enemy_tanks, _) = field;
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let elapsed = tick.saturating_sub(breach.phase_since);
    // An enemy Tank near the traps the breach works on, or near where they stood.
    let work: Vec<(f32, f32)> = breach
        .traps
        .values()
        .map(|t| world(*t))
        .chain(breach.posts.values().map(|p| world(*p)))
        .collect();
    if enemy_tanks.iter().any(|e| {
        work.iter()
            .any(|w| dist2(w.0, w.1, e.0, e.1) <= squared(NEAR_TILES * ts))
    }) {
        breach.last_enemy_near = tick;
    }
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| breach.tanks.contains(&unit.id))
        .collect();
    for tank in &tanks {
        if tank.state == AiEntityState::Move {
            breach.still_since.remove(&tank.id);
        } else {
            breach.still_since.entry(tank.id).or_insert(tick);
        }
    }
    drop_stuck_tanks(observation, breach);
    let mut moved: Vec<u32> = Vec::new();
    let mut attacked: Vec<u32> = Vec::new();

    // Enemy Tanks close by: nobody moves. Charged and standing, the breach Tanks win the trade
    // against Tanks coming at them, which moving they would lose.
    // Only enemy Tanks near the work with an open way to it count: one behind a standing line
    // cannot drive at the breach.
    let site = work_site(breach, &tanks);
    let standing: Vec<(f32, f32)> = breach.traps.values().map(|t| world(*t)).collect();
    let threat = site.is_some_and(|site| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.kind == EntityKind::Tank
                && enemy.hp > 0
                && dist2(enemy.x, enemy.y, site.0, site.1) <= squared(DEFEND_TILES * ts)
                && !crosses_traps((enemy.x, enemy.y), site, &standing, ts)
        })
    });
    if threat {
        breach.last_threat = tick;
        if !breach.defending {
            breach
                .notes
                .push("enemy Tanks close: standing to fight".to_string());
            breach.defending = true;
            breach.held.clear();
        }
    } else if breach.defending && tick.saturating_sub(breach.last_threat) >= DEFEND_CLEAR_TICKS {
        breach.notes.push("quiet again: back to work".to_string());
        breach.defending = false;
        // Fresh posts for wherever the work stands now.
        if breach.phase == Phase::Clear {
            breach.target = None;
            breach.last_order = None;
        }
        breach.last_progress = breach
            .last_progress
            .max(tick.saturating_sub(STALL_TICKS / 2));
    }
    if breach.defending {
        let hold: Vec<u32> = tanks
            .iter()
            .filter(|tank| breach.held.insert(tank.id))
            .map(|tank| tank.id)
            .collect();
        if !hold.is_empty() {
            actions::hold_position_units(actions, hold.iter().copied());
        }
        breach.last_progress = breach
            .last_progress
            .max(tick.saturating_sub(STALL_TICKS / 2));
        return;
    }
    breach.held.clear();

    match breach.phase {
        Phase::Idle | Phase::Done => {}
        Phase::Clear => {
            if lines.is_empty() {
                breach.notes.push("the route is open".to_string());
                breach.last_enemy_near = breach.last_enemy_near.max(tick);
                let posts = std::mem::take(&mut breach.posts);
                set_phase(breach, Phase::Hold, tick);
                // They hold where they shot the last traps.
                breach.posts = posts;
            } else {
                clear_step(
                    observation,
                    map_analysis,
                    actions,
                    breach,
                    field,
                    &tanks,
                    (&mut moved, &mut attacked),
                );
                if tick.saturating_sub(breach.last_progress) >= STALL_TICKS {
                    breach
                        .notes
                        .push("no trap safe to shoot for long: hands the Tanks back".to_string());
                    set_phase(breach, Phase::Done, tick);
                }
            }
        }
        Phase::Hold => {
            // Charged where they shot the last traps; anything coming through meets them there.
            for tank in &tanks {
                let Some(post) = breach.posts.get(&tank.id).copied().map(world) else {
                    continue;
                };
                let engaged = observation.visible_enemies.iter().any(|enemy| {
                    enemy.hp > 0
                        && enemy.kind.is_unit()
                        && dist2(enemy.x, enemy.y, tank.x, tank.y) <= squared(14.0 * ts)
                });
                if !engaged
                    && dist2(tank.x, tank.y, post.0, post.1) > squared(2.5 * ts)
                    && actions::move_units(actions, [tank.id], post.0, post.1).is_some()
                {
                    moved.push(tank.id);
                }
            }
            let quiet = tick.saturating_sub(breach.last_enemy_near) >= QUIET_TICKS;
            if elapsed >= MIN_HOLD_TICKS && quiet {
                let units: Vec<u32> = tanks.iter().map(|tank| tank.id).collect();
                set_phase(breach, Phase::Done, tick);
                if !units.is_empty() {
                    // Released to the push; assembling clears any cached stage orders.
                    intents.push(AiIntent::Assemble { units });
                }
            }
        }
    }
    if tick % 300 < 9 {
        let positions: Vec<String> = tanks
            .iter()
            .map(|tank| {
                format!(
                    "{}@{},{}{}",
                    tank.id,
                    (tank.x / ts) as i32,
                    (tank.y / ts) as i32,
                    &format!("{:?}", tank.state)[..1]
                )
            })
            .collect();
        breach.notes.push(format!(
            "status {:?} target={:?} lines={} traps_left={} enemy_tanks_known={} route_len={} {}",
            breach.phase,
            breach.target.map(|(id, _)| id),
            lines.len(),
            lines.iter().map(Vec::len).sum::<usize>(),
            enemy_tanks.len(),
            route.len(),
            positions.join(" ")
        ));
    }
    if !moved.is_empty() {
        intents.push(AiIntent::Move { units: moved });
    }
    if !attacked.is_empty() {
        intents.push(AiIntent::Attack { units: attacked });
    }
}

/// One decision of clearing: pick the safest blocking trap, post Tanks around it within sight and
/// out of enemy reach, and shoot it once they are aimed.
fn clear_step(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    actions: &mut AiActionContext<'_>,
    breach: &mut TrapBreach,
    (lines, route, enemy_tanks, zones): Field<'_>,
    tanks: &[&AiEntitySummary],
    (moved, attacked): (&mut Vec<u32>, &mut Vec<u32>),
) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    breach
        .skipped
        .retain(|_, since| tick.saturating_sub(*since) < SKIP_TICKS);
    // A target that is gone, or took too long, makes way for the next.
    if let Some((target, since)) = breach.target {
        let gone = !breach.traps.contains_key(&target);
        if gone || tick.saturating_sub(since) >= TARGET_TIMEOUT_TICKS {
            if gone {
                breach.notes.push(format!("trap {target} down"));
                breach.last_progress = tick;
                // With enemy Tanks about, step back from the gap and stand charged a while.
                let gap = breach.target_point.map(world);
                let enemy_about = gap.is_some_and(|gap| {
                    enemy_tanks
                        .iter()
                        .any(|e| dist2(e.0, e.1, gap.0, gap.1) <= squared(REST_IF_ENEMY_TILES * ts))
                });
                if let Some(point) = breach.target_point.filter(|_| enemy_about) {
                    let rest =
                        rest_posts(observation, map_analysis, route, enemy_tanks, world(point));
                    assign_posts(breach, tanks, &rest);
                    breach.rest_until = Some(tick + REST_TICKS);
                }
            } else {
                breach.notes.push(format!("trap {target} passed over"));
                breach.skipped.insert(target, tick);
            }
            breach.target = None;
            breach.last_order = None;
        }
    }
    if breach.rest_until.is_some_and(|until| tick < until) {
        // Resting: back at the rest posts, charged.
        for tank in tanks {
            let Some(post) = breach.posts.get(&tank.id).copied().map(world) else {
                continue;
            };
            let far = dist2(tank.x, tank.y, post.0, post.1) > squared(IN_POSITION_TILES * ts);
            let reissue = tank.state != AiEntityState::Move || tick % ORDER_REFRESH_TICKS < 9;
            if far && reissue && actions::move_units(actions, [tank.id], post.0, post.1).is_some() {
                moved.push(tank.id);
            }
        }
        return;
    }
    breach.rest_until = None;
    let standing: Vec<(f32, f32)> = breach.traps.values().map(|t| world(*t)).collect();
    if breach.target.is_none() {
        // The first line still standing, and in it the trap with the best margin from the enemy
        // army that has safe posts, not too far from where the Tanks stand.
        let n = tanks.len().max(1) as f32;
        let group = (
            tanks.iter().map(|tank| tank.x).sum::<f32>() / n,
            tanks.iter().map(|tank| tank.y).sum::<f32>() / n,
        );
        let pick = lines.iter().find_map(|line| {
            line.iter()
                .copied()
                .filter(|id| !breach.skipped.contains_key(id))
                .filter_map(|id| {
                    let p = world(*breach.traps.get(&id)?);
                    let posts = safe_posts(
                        observation,
                        map_analysis,
                        route,
                        enemy_tanks,
                        zones,
                        &standing,
                        p,
                    );
                    (!posts.is_empty()).then(|| {
                        let margin = enemy_tanks
                            .iter()
                            .map(|e| dist2(e.0, e.1, p.0, p.1).sqrt() / ts)
                            .fold(40.0f32, f32::min);
                        let away = dist2(group.0, group.1, p.0, p.1).sqrt() / ts;
                        (id, posts, margin - TARGET_DISTANCE_WEIGHT * away)
                    })
                })
                .max_by(|a, b| a.2.total_cmp(&b.2).then_with(|| b.0.cmp(&a.0)))
        });
        let Some((id, posts, _)) = pick else {
            return;
        };
        breach
            .notes
            .push(format!("target trap {id} with {} posts", posts.len()));
        breach.target = Some((id, tick));
        breach.target_point = breach.traps.get(&id).copied();
        breach.last_progress = tick;
        assign_posts(breach, tanks, &posts);
    }
    let Some((target, _)) = breach.target else {
        return;
    };
    let Some(target_point) = breach.traps.get(&target).copied().map(world) else {
        return;
    };
    // Posts that came within reach of an enemy Tank are given up: pick again.
    let threatened = breach.posts.values().any(|post| {
        let p = world(*post);
        enemy_tanks
            .iter()
            .any(|e| dist2(e.0, e.1, p.0, p.1) <= squared((ENEMY_TANK_CLEARANCE_TILES - 1.0) * ts))
    });
    if threatened && breach.last_order.is_none() {
        breach
            .notes
            .push(format!("posts around trap {target} came under threat"));
        breach.skipped.insert(target, tick);
        breach.target = None;
        return;
    }
    // Tanks without a post (late joiners, spares) stand behind the posted ones.
    let rally = rally_point(breach, target_point, route, ts);
    let mut aimed: Vec<u32> = Vec::new();
    for tank in tanks {
        let posted = breach.posts.contains_key(&tank.id);
        let post = breach
            .posts
            .get(&tank.id)
            .copied()
            .map(world)
            .unwrap_or(rally);
        let at_post = dist2(tank.x, tank.y, post.0, post.1) <= squared(IN_POSITION_TILES * ts);
        let shooting = posted && breach.last_order.is_some();
        if !at_post && !shooting {
            let tolerance = if posted { IN_POSITION_TILES } else { 3.0 };
            let far = dist2(tank.x, tank.y, post.0, post.1) > squared(tolerance * ts);
            let reissue = tank.state != AiEntityState::Move || tick % ORDER_REFRESH_TICKS < 9;
            if far && reissue && actions::move_units(actions, [tank.id], post.0, post.1).is_some() {
                moved.push(tank.id);
            }
            continue;
        }
        let still = breach
            .still_since
            .get(&tank.id)
            .is_some_and(|since| tick.saturating_sub(*since) >= AIM_TICKS);
        if posted && still {
            aimed.push(tank.id);
        }
    }
    let posted = breach.posts.len();
    let ready = !aimed.is_empty() && (aimed.len() >= 2 || aimed.len() == posted);
    let refresh_due = breach
        .last_order
        .is_none_or(|(id, at)| id != target || tick.saturating_sub(at) >= ORDER_REFRESH_TICKS);
    if (ready || breach.last_order.is_some()) && refresh_due && !aimed.is_empty() {
        if let Some(sent) = actions::clear_obstacle_area(actions, aimed, target) {
            breach
                .notes
                .push(format!("shoot trap {target} with {}", sent.len()));
            attacked.extend(sent);
            breach.last_order = Some((target, tick));
        }
    }
}

/// Posts around `trap`: within sight of it, on Jeff's side of its line, open ground, outside the
/// reach of every enemy Tank seen lately and of parked groups, spaced apart.
#[allow(clippy::too_many_arguments)]
fn safe_posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    route: &[(f32, f32)],
    enemy_tanks: &[(f32, f32)],
    zones: &[super::tank_micro::Zone],
    standing: &[(f32, f32)],
    trap: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let Some(heading) = route_heading(route, trap) else {
        return Vec::new();
    };
    let along = |p: &(f32, f32)| (p.0 - trap.0) * heading.0 + (p.1 - trap.1) * heading.1;
    let jeff_side = jeff_side_point(route, trap, ts);
    let mut posts: Vec<(f32, f32)> = Vec::new();
    for radius in POST_RINGS {
        for step in 0..18 {
            let angle = step as f32 * std::f32::consts::TAU / 18.0;
            let p = (
                trap.0 + angle.cos() * radius * ts,
                trap.1 + angle.sin() * radius * ts,
            );
            if crosses_traps(p, jeff_side, standing, ts)
                || standing
                    .iter()
                    .any(|t| dist2(t.0, t.1, p.0, p.1) <= squared(1.5 * ts))
            {
                continue;
            }
            let exposed = enemy_tanks
                .iter()
                .any(|e| dist2(e.0, e.1, p.0, p.1) <= squared(ENEMY_TANK_CLEARANCE_TILES * ts))
                || zones.iter().any(|zone| {
                    dist2(zone.center.0, zone.center.1, p.0, p.1) <= squared(zone.radius)
                });
            let crowded = posts
                .iter()
                .any(|q| dist2(q.0, q.1, p.0, p.1) < squared(POST_SPACING_TILES * ts));
            if exposed
                || crowded
                || !defense::defensive_position_is_open(observation, map_analysis, p.0, p.1)
            {
                continue;
            }
            posts.push(p);
        }
    }
    // The posts deepest on Jeff's side first.
    posts.sort_by(|a, b| along(a).total_cmp(&along(b)));
    posts.truncate(MAX_POSTS);
    posts
}

/// Posts to step back to from a gap: `REST_TILES` behind it toward Jeff along the route, in a
/// line across it, open ground and out of enemy reach.
fn rest_posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    route: &[(f32, f32)],
    enemy_tanks: &[(f32, f32)],
    gap: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let Some(heading) = route_heading(route, gap) else {
        return Vec::new();
    };
    let across = (-heading.1, heading.0);
    let mut posts = Vec::new();
    for depth in [REST_TILES, REST_TILES + 2.5] {
        for k in [0.0f32, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0] {
            let p = (
                gap.0 - heading.0 * depth * ts + across.0 * k * POST_SPACING_TILES * ts,
                gap.1 - heading.1 * depth * ts + across.1 * k * POST_SPACING_TILES * ts,
            );
            let exposed = enemy_tanks
                .iter()
                .any(|e| dist2(e.0, e.1, p.0, p.1) <= squared(ENEMY_TANK_CLEARANCE_TILES * ts));
            if !exposed && defense::defensive_position_is_open(observation, map_analysis, p.0, p.1)
            {
                posts.push(p);
            }
        }
    }
    posts.truncate(MAX_TANKS);
    posts
}

/// Where the breach works: the centre of its posts, else of its Tanks.
fn work_site(breach: &TrapBreach, tanks: &[&AiEntitySummary]) -> Option<(f32, f32)> {
    let points: Vec<(f32, f32)> = if breach.posts.is_empty() {
        tanks.iter().map(|tank| (tank.x, tank.y)).collect()
    } else {
        breach.posts.values().map(|p| world(*p)).collect()
    };
    let n = points.len();
    (n > 0).then(|| {
        let (sx, sy) = points
            .iter()
            .fold((0.0, 0.0), |acc, p| (acc.0 + p.0, acc.1 + p.1));
        (sx / n as f32, sy / n as f32)
    })
}

/// A point on Jeff's side of the line at `trap`: back along the route from where it passes nearest.
fn jeff_side_point(route: &[(f32, f32)], trap: (f32, f32), ts: f32) -> (f32, f32) {
    let index = (0..route.len())
        .min_by(|a, b| {
            dist2(route[*a].0, route[*a].1, trap.0, trap.1).total_cmp(&dist2(
                route[*b].0,
                route[*b].1,
                trap.0,
                trap.1,
            ))
        })
        .unwrap_or(0);
    // Route points are a tile apart; step back toward Jeff.
    let back = index.saturating_sub((JEFF_SIDE_BACK_TILES + ROUTE_TILES) as usize);
    let _ = ts;
    route[back]
}

/// Whether the straight way from `a` to `b` passes a standing trap closer than
/// `TRAP_CLEARANCE_TILES`.
fn crosses_traps(a: (f32, f32), b: (f32, f32), standing: &[(f32, f32)], ts: f32) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    standing.iter().any(|t| {
        let along = if len2 > 0.0 {
            (((t.0 - a.0) * dx + (t.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (px, py) = (a.0 + dx * along, a.1 + dy * along);
        dist2(px, py, t.0, t.1) <= squared(TRAP_CLEARANCE_TILES * ts)
    })
}

/// Gives the posts to the nearest Tanks.
fn assign_posts(breach: &mut TrapBreach, tanks: &[&AiEntitySummary], posts: &[(f32, f32)]) {
    breach.posts.clear();
    let mut free: Vec<&AiEntitySummary> = tanks.to_vec();
    for post in posts {
        let Some(index) = (0..free.len()).min_by(|a, b| {
            dist2(free[*a].x, free[*a].y, post.0, post.1)
                .total_cmp(&dist2(free[*b].x, free[*b].y, post.0, post.1))
                .then_with(|| free[*a].id.cmp(&free[*b].id))
        }) else {
            return;
        };
        let tank = free.remove(index);
        breach.posts.insert(tank.id, stored(*post));
    }
}

/// Where Tanks without a post wait: behind the posts, toward Jeff along the route.
fn rally_point(
    breach: &TrapBreach,
    target: (f32, f32),
    route: &[(f32, f32)],
    ts: f32,
) -> (f32, f32) {
    let heading = route_heading(route, target).unwrap_or((0.0, 0.0));
    let n = breach.posts.len().max(1) as f32;
    let (sx, sy) = breach
        .posts
        .values()
        .map(|p| world(*p))
        .fold((0.0, 0.0), |acc, p| (acc.0 + p.0, acc.1 + p.1));
    let center = if breach.posts.is_empty() {
        target
    } else {
        (sx / n, sy / n)
    };
    (
        center.0 - heading.0 * 4.0 * ts,
        center.1 - heading.1 * 4.0 * ts,
    )
}

/// Lets go of Tanks that got no closer to their post for `STUCK_TICKS`.
fn drop_stuck_tanks(observation: &AiObservation, breach: &mut TrapBreach) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let mut stuck = Vec::new();
    for tank in observation
        .owned
        .iter()
        .filter(|unit| breach.tanks.contains(&unit.id))
    {
        let Some(post) = breach.posts.get(&tank.id).copied().map(world) else {
            breach.progress.remove(&tank.id);
            continue;
        };
        let tiles = (dist2(tank.x, tank.y, post.0, post.1).sqrt() / ts) as u32;
        if tiles as f32 <= IN_POSITION_TILES + 1.0 {
            breach.progress.remove(&tank.id);
            continue;
        }
        match breach.progress.get(&tank.id).copied() {
            Some((since, best)) if tiles + 1 > best => {
                if tick.saturating_sub(since) >= STUCK_TICKS {
                    stuck.push(tank.id);
                }
            }
            _ => {
                breach.progress.insert(tank.id, (tick, tiles));
            }
        }
    }
    for id in stuck {
        breach.notes.push(format!("let go of stuck tank {id}"));
        breach.tanks.remove(&id);
        breach.posts.remove(&id);
        breach.progress.remove(&id);
        breach.dropped.insert(id, tick);
    }
}

#[cfg(test)]
#[path = "trap_breach_tests.rs"]
mod tests;
