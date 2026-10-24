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

mod posts;
use posts::*;

const START_TANKS: usize = 8;
/// At most this many Tanks join the breach; the rest stay with the regular plans.
const MAX_TANKS: usize = 30;
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
/// The breach remembers where it saw enemy Tanks this long, unless it sees the spot empty: a group
/// parked behind a line stays there, out of sight, for minutes.
const ENEMY_TANK_MEMORY_TICKS: u32 = config::TICK_HZ * 120;
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
/// The Scout Car looks at each next trap from this far back (it sees 15), long enough to see.
const PEEK_TILES: f32 = 9.0;
const PEEK_TICKS: u32 = 20;
const LOOK_TIMEOUT_TICKS: u32 = config::TICK_HZ * 20;
/// The concave keeps this far from every enemy Tank seen lately, and the way to each post keeps
/// out of their reach: the whole group once drove past a parked pile to a concave 9 tiles from it
/// and lost 18 Tanks.
const CONCAVE_CLEARANCE_TILES: f32 = 16.0;
const TRAVEL_CLEARANCE_TILES: f32 = 14.5;
/// For a contested trap the group stands in a concave this far back, beyond the reach of Tanks
/// parked behind it (14 tiles and a margin)...
const CONCAVE_TILES: f32 = 18.0;
/// ...and four Tanks fire on it from this far, the most a charged Tank reaches with a margin, while
/// the Scout Car spots from just inside its own sight.
const FIRE_TILES: f32 = 12.3;
const SPOT_TILES: f32 = 13.8;
const VOLLEY_TANKS: usize = 4;
const SHOOTERS_TO_FIRE: usize = 2;
/// A Tank's range reaches 12 tiles after this long standing.
const VOLLEY_AIM_TICKS: u32 = 75;
const VOLLEY_TIMEOUT_TICKS: u32 = config::TICK_HZ * 25;
const VOLLEY_FIRE_TICKS: u32 = config::TICK_HZ * 8;
const FORM_TIMEOUT_TICKS: u32 = config::TICK_HZ * 40;
/// After a volley the concave holds at least this long, and until no enemy Tank came near.
const FALLBACK_MIN_TICKS: u32 = config::TICK_HZ * 10;
const FALLBACK_QUIET_TICKS: u32 = config::TICK_HZ * 15;
/// A Tank standing this long fires at full range.
const CHARGE_TICKS: u32 = config::TICK_HZ * 3 + 15;
/// Volley posts keep at least this far from every enemy Tank seen lately: a charged Tank reaches
/// 14, and a volley fired from inside that reach into a parked group lost Tanks for every trap.
/// More clearance is preferred up to the second figure.
const FIRE_MIN_CLEARANCE_TILES: f32 = 14.5;
/// With Jeff's Tanks ahead, a volley may fire from closer: only Tanks charged and in range answer
/// it, and whatever comes through the gap meets the larger army.
const FIRE_AHEAD_CLEARANCE_TILES: f32 = 11.0;
const FIRE_CLEARANCE_TILES: f32 = 16.0;
/// A line on the enemy's half is opened only while Jeff's Tanks outnumber the enemy Tanks it knows
/// of by this much: whatever stood behind it comes through the gap at the Tanks that opened it.
/// Lines on Jeff's half open as soon as there are Tanks to spare.
const ENEMY_LINE_RATIO: f32 = 1.2;
const ENEMY_LINE_LEAD: f32 = 2.0;
/// After stopping for want of a trap safe to shoot, the breach waits this long before it looks again.
const RESTART_TICKS: u32 = config::TICK_HZ * 60;
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

/// The steps of clearing one trap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Stage {
    /// The Scout Car looks at the trap.
    #[default]
    Look,
    /// No enemy about: Tanks shoot from posts in sight of it.
    Shoot,
    /// Contested: the group charges in a concave beyond reach.
    Form,
    /// Four Tanks drive up and fire one volley.
    Volley,
    /// Back in the concave until things are quiet.
    FallBack,
}

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
    /// The Scout Car that looks and spots for the breach.
    scout: Option<u32>,
    stage: Stage,
    stage_since: u32,
    /// The current trap has enemy Tanks about.
    contested: bool,
    /// Firing posts for a volley, and the Tanks firing it with their posts.
    fire_posts: Vec<(i32, i32)>,
    volley: BTreeMap<u32, (i32, i32)>,
    /// Since when the Scout Car stands at its look-out.
    peeked_since: Option<u32>,
    /// Enemy Tanks seen and where, and when, until their spot is seen empty or they are forgotten.
    enemy_seen: BTreeMap<u32, ((i32, i32), u32)>,
    /// The breach stopped then with no trap safe to shoot.
    stalled_at: Option<u32>,
    /// Jeff's Tanks outnumber the known enemy Tanks enough to open lines on the enemy's half.
    superior: bool,
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
        self.tanks
            .iter()
            .copied()
            .chain(self.scout)
            .filter(move |_| active)
    }

    pub(super) fn is_reserved(&self, id: u32) -> bool {
        self.reserved().any(|unit| unit == id)
    }
}

pub(super) fn uses_trap_breach(profile_id: &str) -> bool {
    runs_live_jeff(profile_id)
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
    let enemy_tanks = remember_enemy_tanks(observation, &mut memory.trap_breach, ts);
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
        // A line still across the route (one an opponent built, or one left standing) starts a
        // new breach, a while after one stopped for want of a safe trap.
        if breach
            .stalled_at
            .is_some_and(|at| tick.saturating_sub(at) < RESTART_TICKS)
        {
            return;
        }
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
    let own_tanks = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .count();
    breach.superior =
        own_tanks as f32 >= ENEMY_LINE_RATIO * enemy_tanks.len() as f32 + ENEMY_LINE_LEAD;
    if breach.phase == Phase::Idle {
        // Nothing to start on: every line left is on the enemy's half and Jeff is not ahead.
        let workable = breach.superior
            || lines.iter().flatten().any(|id| {
                breach
                    .traps
                    .get(id)
                    .is_some_and(|pos| !on_enemy_half(&route, world(*pos)))
            });
        if spare.len() < START_TANKS || home_attacked || !workable {
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
        take_scout(breach, observation, &route);
        // The push that would stall at the line gives way to the breach.
        memory.containment = Default::default();
    } else if !breach.defending {
        take_tanks(breach, &spare, &route);
        take_scout(breach, observation, &route);
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

/// Enemy Tanks seen lately: every one in sight now, and the rest where they were last seen until
/// that spot is seen empty or `ENEMY_TANK_MEMORY_TICKS` pass.
fn remember_enemy_tanks(
    observation: &AiObservation,
    breach: &mut TrapBreach,
    ts: f32,
) -> Vec<(f32, f32)> {
    let tick = observation.tick;
    let visible: BTreeMap<u32, (f32, f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .map(|enemy| (enemy.id, (enemy.x, enemy.y)))
        .collect();
    for (id, pos) in &visible {
        breach.enemy_seen.insert(*id, (stored(*pos), tick));
    }
    breach.enemy_seen.retain(|id, (pos, seen)| {
        if visible.contains_key(id) {
            return true;
        }
        if tick.saturating_sub(*seen) > ENEMY_TANK_MEMORY_TICKS {
            return false;
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
    breach
        .enemy_seen
        .values()
        .map(|(pos, _)| world(*pos))
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
    breach.stage = Stage::Look;
    breach.stage_since = tick;
    breach.volley.clear();
    if matches!(phase, Phase::Done | Phase::Idle) {
        breach.tanks.clear();
        breach.posts.clear();
        breach.scout = None;
    }
}

/// Takes spare Tanks up to the cap, nearest to Jeff's end of the route first.
/// Takes a Scout Car to look and spot, the one nearest Jeff's end of the route.
fn take_scout(breach: &mut TrapBreach, observation: &AiObservation, route: &[(f32, f32)]) {
    if breach.scout.is_some_and(|id| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == id && unit.hp > 0)
    }) {
        return;
    }
    let anchor = route.first().copied().unwrap_or((0.0, 0.0));
    breach.scout = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ScoutCar && unit.is_complete && unit.hp > 0)
        .min_by(|a, b| {
            dist2(a.x, a.y, anchor.0, anchor.1)
                .total_cmp(&dist2(b.x, b.y, anchor.0, anchor.1))
                .then_with(|| a.id.cmp(&b.id))
        })
        .map(|unit| unit.id);
}

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
                    breach.stalled_at = Some(tick);
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

/// One decision of clearing. The Scout Car looks at the next trap first. A trap with no enemy
/// Tank about is shot from posts in sight of it. A contested one (enemy Tanks about, usually
/// parked behind it) is shot in one volley: the group charges in a concave beyond their reach,
/// four Tanks drive up to fire from as far as the trap can be shot, then fall back, and whatever
/// comes through the gap meets the concave.
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
    let standing: Vec<(f32, f32)> = breach.traps.values().map(|t| world(*t)).collect();
    let scout = breach
        .scout
        .and_then(|id| observation.owned.iter().find(|unit| unit.id == id));

    // A target that is gone, or took too long, makes way for the next once the group has fallen
    // back and things are quiet.
    if let Some((target, since)) = breach.target {
        let gone = !breach.traps.contains_key(&target);
        // Only shooting from posts in sight has a deadline here; a volley keeps its own.
        let timed_out =
            breach.stage == Stage::Shoot && tick.saturating_sub(since) >= TARGET_TIMEOUT_TICKS;
        if (gone || timed_out) && breach.stage != Stage::FallBack {
            if gone {
                breach.notes.push(format!("trap {target} down"));
                breach.last_progress = tick;
            } else {
                breach.notes.push(format!("trap {target} passed over"));
                breach.skipped.insert(target, tick);
            }
            breach.last_order = None;
            if breach.contested {
                breach.stage = Stage::FallBack;
                breach.stage_since = tick;
            } else {
                breach.target = None;
            }
        }
    }
    if breach.target.is_none() {
        let Some(pick) = pick_target(
            observation,
            map_analysis,
            breach,
            (lines, route, enemy_tanks, zones),
            tanks,
            &standing,
        ) else {
            return;
        };
        breach.notes.push(format!("target trap {pick}"));
        breach.target = Some((pick, tick));
        breach.target_point = breach.traps.get(&pick).copied();
        breach.last_progress = tick;
        breach.last_order = None;
        breach.posts.clear();
        breach.fire_posts.clear();
        breach.stage = Stage::Look;
        breach.stage_since = tick;
    }
    let Some((target, _)) = breach.target else {
        return;
    };
    let Some(trap) = breach.target_point.map(world) else {
        breach.target = None;
        return;
    };
    let back = back_direction(route, trap, ts);
    let rest = (
        trap.0 + back.0 * (CONCAVE_TILES + 3.0) * ts,
        trap.1 + back.1 * (CONCAVE_TILES + 3.0) * ts,
    );

    match breach.stage {
        Stage::Look => {
            // The Scout Car looks from just inside its sight, so a parked army behind the trap is
            // known before any Tank stands within its reach.
            let peek = (
                trap.0 + back.0 * PEEK_TILES * ts,
                trap.1 + back.1 * PEEK_TILES * ts,
            );
            let looked = match scout {
                None => true,
                Some(scout) => {
                    let there =
                        dist2(scout.x, scout.y, peek.0, peek.1) <= squared(IN_POSITION_TILES * ts);
                    if there {
                        breach.peeked_since.get_or_insert(tick);
                    } else {
                        breach.peeked_since = None;
                        send(actions, scout, peek, tick, moved);
                    }
                    breach
                        .peeked_since
                        .is_some_and(|since| tick.saturating_sub(since) >= PEEK_TICKS)
                }
            };
            // The Tanks wait behind meanwhile.
            for tank in tanks {
                if dist2(tank.x, tank.y, rest.0, rest.1) > squared(6.0 * ts) {
                    send(actions, tank, rest, tick, moved);
                }
            }
            if !looked && tick.saturating_sub(breach.stage_since) < LOOK_TIMEOUT_TICKS {
                return;
            }
            breach.peeked_since = None;
            // On Jeff's half, posts in sight of the trap and out of every known enemy Tank's reach
            // are used whenever there are any. A trap on the enemy's half always gets the volley:
            // a line there is likely covered by Tanks parked out of sight behind it, which a look
            // from Jeff's side does not reach (one breach walked its posts into such a group and
            // lost 10 of 12 Tanks).
            let posts = if on_enemy_half(route, trap) {
                Vec::new()
            } else {
                safe_posts(
                    observation,
                    map_analysis,
                    route,
                    enemy_tanks,
                    zones,
                    &standing,
                    trap,
                )
            };
            breach.contested = posts.is_empty();
            if posts.is_empty() {
                let concave = concave_posts(
                    observation,
                    map_analysis,
                    trap,
                    back,
                    &standing,
                    enemy_tanks,
                    group_center(tanks),
                );
                let fire = fire_posts(
                    observation,
                    map_analysis,
                    trap,
                    back,
                    &standing,
                    (enemy_tanks, volley_clearance(breach)),
                    ts,
                );
                if concave.is_empty() || fire.is_empty() {
                    breach
                        .notes
                        .push(format!("trap {target}: no room for a volley, passed over"));
                    breach.skipped.insert(target, tick);
                    breach.target = None;
                    return;
                }
                assign_posts(breach, tanks, &concave);
                breach.fire_posts = fire.iter().map(|p| stored(*p)).collect();
                breach
                    .notes
                    .push(format!("trap {target} contested: forming the concave"));
                breach.stage = Stage::Form;
            } else {
                assign_posts(breach, tanks, &posts);
                breach.notes.push(format!(
                    "trap {target} clear of enemies: {} posts",
                    posts.len()
                ));
                breach.stage = Stage::Shoot;
            }
            breach.stage_since = tick;
        }
        Stage::Shoot => {
            if let Some(scout) = scout {
                send_if_far(actions, scout, rest, tick, moved);
            }
            let aimed = man_posts(breach, tanks, actions, tick, ts, moved, AIM_TICKS);
            fire(breach, actions, attacked, target, aimed, tick);
        }
        Stage::Form => {
            if let Some(scout) = scout {
                send_if_far(actions, scout, rest, tick, moved);
            }
            let charged = man_posts(breach, tanks, actions, tick, ts, moved, CHARGE_TICKS);
            let need = (tanks.len() * 2).div_ceil(3).max(1);
            if charged.len() >= need
                || tick.saturating_sub(breach.stage_since) >= FORM_TIMEOUT_TICKS
            {
                // The Tanks nearest the firing posts go.
                breach.volley.clear();
                let mut free: Vec<&AiEntitySummary> = tanks.to_vec();
                for post in breach.fire_posts.clone() {
                    let p = world(post);
                    let Some(index) = (0..free.len()).min_by(|a, b| {
                        dist2(free[*a].x, free[*a].y, p.0, p.1)
                            .total_cmp(&dist2(free[*b].x, free[*b].y, p.0, p.1))
                            .then_with(|| free[*a].id.cmp(&free[*b].id))
                    }) else {
                        break;
                    };
                    breach.volley.insert(free.remove(index).id, post);
                }
                breach.notes.push(format!(
                    "volley on trap {target} with {}",
                    breach.volley.len()
                ));
                breach.stage = Stage::Volley;
                breach.stage_since = tick;
            }
        }
        Stage::Volley => {
            // The Scout Car spots the trap from as far back as its sight allows.
            if let Some(scout) = scout {
                let spot = (
                    trap.0 + back.0 * SPOT_TILES * ts,
                    trap.1 + back.1 * SPOT_TILES * ts,
                );
                send_if_far(actions, scout, spot, tick, moved);
            }
            let mut aimed = Vec::new();
            for tank in tanks {
                let post = breach
                    .volley
                    .get(&tank.id)
                    .or_else(|| breach.posts.get(&tank.id))
                    .copied()
                    .map(world);
                let Some(post) = post else {
                    continue;
                };
                let firing = breach.volley.contains_key(&tank.id) && breach.last_order.is_some();
                if firing {
                    continue;
                }
                if dist2(tank.x, tank.y, post.0, post.1) > squared(IN_POSITION_TILES * ts) {
                    send(actions, tank, post, tick, moved);
                    continue;
                }
                let still = breach
                    .still_since
                    .get(&tank.id)
                    .is_some_and(|since| tick.saturating_sub(*since) >= VOLLEY_AIM_TICKS);
                if breach.volley.contains_key(&tank.id)
                    && still
                    && dist2(tank.x, tank.y, trap.0, trap.1) <= squared(12.8 * ts)
                {
                    aimed.push(tank.id);
                }
            }
            let volley_alive = tanks
                .iter()
                .filter(|tank| breach.volley.contains_key(&tank.id))
                .count();
            if breach.last_order.is_none() && aimed.len() < SHOOTERS_TO_FIRE.min(volley_alive) {
                if tick.saturating_sub(breach.stage_since) >= VOLLEY_TIMEOUT_TICKS
                    || volley_alive == 0
                {
                    breach.stage = Stage::FallBack;
                    breach.stage_since = tick;
                }
                return;
            }
            let shooters: Vec<u32> = tanks
                .iter()
                .filter(|tank| breach.volley.contains_key(&tank.id))
                .map(|tank| tank.id)
                .collect();
            fire(breach, actions, attacked, target, shooters, tick);
            if breach
                .last_order
                .is_some_and(|(_, at)| tick.saturating_sub(at) >= VOLLEY_FIRE_TICKS)
            {
                breach.stage = Stage::FallBack;
                breach.stage_since = tick;
            }
        }
        Stage::FallBack => {
            // Everyone back in the concave, charged; whatever comes through meets it.
            breach.volley.clear();
            breach.last_order = None;
            if let Some(scout) = scout {
                send_if_far(actions, scout, rest, tick, moved);
            }
            man_posts(breach, tanks, actions, tick, ts, moved, CHARGE_TICKS);
            // Quiet means nothing came at the concave lately; Tanks parked behind a line are not
            // coming.
            let quiet = tick.saturating_sub(breach.last_threat) >= FALLBACK_QUIET_TICKS;
            if tick.saturating_sub(breach.stage_since) >= FALLBACK_MIN_TICKS && quiet {
                breach.target = None;
                breach.stage = Stage::Look;
                breach.stage_since = tick;
            }
        }
    }
}

/// The next trap: in the first line still standing, the one with the best margin from the enemy
/// army, not too far from where the Tanks stand. A trap on the enemy's half is only taken when a
/// volley can be fired at it from out of every known enemy Tank's reach.
fn pick_target(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    breach: &TrapBreach,
    (lines, route, enemy_tanks, _): Field<'_>,
    tanks: &[&AiEntitySummary],
    standing: &[(f32, f32)],
) -> Option<u32> {
    let ts = observation.map.tile_size as f32;
    let clearance = volley_clearance(breach);
    let volleyable = |p: (f32, f32)| {
        let back = back_direction(route, p, ts);
        fire_posts(
            observation,
            map_analysis,
            p,
            back,
            standing,
            (enemy_tanks, clearance),
            ts,
        )
        .len()
            >= SHOOTERS_TO_FIRE
    };
    let n = tanks.len().max(1) as f32;
    let group = (
        tanks.iter().map(|tank| tank.x).sum::<f32>() / n,
        tanks.iter().map(|tank| tank.y).sum::<f32>() / n,
    );
    lines.iter().find_map(|line| {
        line.iter()
            .copied()
            .filter(|id| !breach.skipped.contains_key(id))
            .filter_map(|id| {
                let p = world(*breach.traps.get(&id)?);
                if on_enemy_half(route, p) && !(breach.superior && volleyable(p)) {
                    return None;
                }
                let margin = enemy_tanks
                    .iter()
                    .map(|e| dist2(e.0, e.1, p.0, p.1).sqrt() / ts)
                    .fold(40.0f32, f32::min);
                let away = dist2(group.0, group.1, p.0, p.1).sqrt() / ts;
                Some((id, margin - TARGET_DISTANCE_WEIGHT * away))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
            .map(|(id, _)| id)
    })
}

/// How far volley posts keep from known enemy Tanks: out of every charged Tank's reach, unless
/// Jeff is ahead.
fn volley_clearance(breach: &TrapBreach) -> f32 {
    if breach.superior {
        FIRE_AHEAD_CLEARANCE_TILES
    } else {
        FIRE_MIN_CLEARANCE_TILES
    }
}

/// Sends each Tank to its post; returns the posted Tanks standing there at least `ready_ticks`.
fn man_posts(
    breach: &mut TrapBreach,
    tanks: &[&AiEntitySummary],
    actions: &mut AiActionContext<'_>,
    tick: u32,
    ts: f32,
    moved: &mut Vec<u32>,
    ready_ticks: u32,
) -> Vec<u32> {
    let rally = breach.posts.values().next().copied().map(world);
    let mut ready = Vec::new();
    for tank in tanks {
        let Some(post) = breach.posts.get(&tank.id).copied().map(world).or(rally) else {
            continue;
        };
        let posted = breach.posts.contains_key(&tank.id);
        let tolerance = if posted { IN_POSITION_TILES } else { 4.0 };
        if dist2(tank.x, tank.y, post.0, post.1) > squared(tolerance * ts) {
            if !(posted && breach.last_order.is_some()) {
                send(actions, tank, post, tick, moved);
            }
            continue;
        }
        let still = breach
            .still_since
            .get(&tank.id)
            .is_some_and(|since| tick.saturating_sub(*since) >= ready_ticks);
        if posted && still {
            ready.push(tank.id);
        }
    }
    ready
}

fn fire(
    breach: &mut TrapBreach,
    actions: &mut AiActionContext<'_>,
    attacked: &mut Vec<u32>,
    target: u32,
    shooters: Vec<u32>,
    tick: u32,
) {
    let ready = !shooters.is_empty() && (shooters.len() >= 2 || breach.last_order.is_some());
    let due = breach
        .last_order
        .is_none_or(|(id, at)| id != target || tick.saturating_sub(at) >= ORDER_REFRESH_TICKS);
    if ready && due {
        if let Some(sent) = actions::clear_obstacle_area(actions, shooters, target) {
            breach
                .notes
                .push(format!("shoot trap {target} with {}", sent.len()));
            attacked.extend(sent);
            breach.last_order = Some((target, tick));
        }
    }
}

/// Orders a unit to `point` unless it is already driving there.
fn send(
    actions: &mut AiActionContext<'_>,
    unit: &AiEntitySummary,
    point: (f32, f32),
    tick: u32,
    moved: &mut Vec<u32>,
) {
    let reissue = unit.state != AiEntityState::Move || tick % ORDER_REFRESH_TICKS < 9;
    if reissue && actions::move_units(actions, [unit.id], point.0, point.1).is_some() {
        moved.push(unit.id);
    }
}

fn send_if_far(
    actions: &mut AiActionContext<'_>,
    unit: &AiEntitySummary,
    point: (f32, f32),
    tick: u32,
    moved: &mut Vec<u32>,
) {
    if dist2(unit.x, unit.y, point.0, point.1) > squared(2.0 * 32.0) {
        send(actions, unit, point, tick, moved);
    }
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
