//! The live Jeff's answer to raids on its bases (`JEFFS_AI_ID` only).
//!
//! Human openers come at the base from the side, 42-117 degrees off the line between the bases,
//! and go for the extractors first. Jeff saw them only on arrival, sent its frontal defenders over
//! one or two at a time where they were shot piecemeal, chased raiders out of the base, and let
//! Riflemen stand idle just beyond their own reach. Here:
//! - **Watch**: enemy fighters within `WATCH_TILES` of any Jeff building, extractor, Engineer or
//!   the natural site are a raid before they reach anything, and two sentries posted off the
//!   flanks see them coming.
//! - **Gather, then engage**: the response is sized by the volley estimate to beat the raid. It
//!   gathers beside what the raid threatens, on the side facing home and outside the raid's reach,
//!   and goes in together. A raid nobody can beat is met on the dug-in line instead: the mobile
//!   infantry falls back to it, and nothing is fed to the raid in the open.
//! - **Intercept, don't chase**: the response engages only while the raid is within `LEASH_TILES`
//!   of what it defends. A raid that backs off is not followed; one that leaves is let go and the
//!   defenders go back to their posts.
//! - **Never stand outranged**: a unit shot from beyond its own reach steps in to fire when its
//!   group wins the exchange there, and steps back out of reach when it does not.
//! - **Infantry answers infantry**: enemy infantry seen near the bases in the opening minutes
//!   raises the Rifleman target, ahead of tech, to match it.
//!
//! Tanks stay with the home armour. A raid already in reach of Jeff's dug-in line is left to the
//! line and the older incident response, which crush frontal raids; so is any raid with a Tank.

use super::super::opening_rush::{
    estimate_trade, fighter, Fighter, LiveStep, EARLIEST_ENTRENCHMENT_TICK,
};
use super::envelope::defended_building_sites;
use super::*;

/// Enemy fighters this close to a Jeff building, extractor, Engineer or natural site are a raid.
const WATCH_TILES: f32 = 16.0;
/// Raiders this close to one another are one raid.
const RAID_LINK_TILES: f32 = 8.0;
/// Defenders farther than this from a raid are not called to it.
const RESPONSE_TILES: f32 = 45.0;
/// Units holding a post (sentries, natural and new-base guards) answer only raids this close.
const POSTED_RESPONSE_TILES: f32 = 16.0;
/// The response gathers this far outside the raid's reach.
const GATHER_MARGIN_TILES: f32 = 2.5;
/// A member this close to the gathering point has arrived.
const GATHERED_TILES: f32 = 3.0;
/// The gathering point is re-sent only when it has moved this far.
const GATHER_SHIFT_TILES: f32 = 4.0;
/// The response goes in once this share of its value has gathered, or after
/// `GATHER_TIMEOUT_TICKS` if what has gathered already beats the raid.
const GATHERED_SHARE: (u32, u32) = (3, 4);
const GATHER_TIMEOUT_TICKS: u32 = config::TICK_HZ * 8;
/// Dug-in units and Machine Gunners join a flank response only this early, while they are all the
/// home has. Later the estimate overrates them: a gun moving up must set up before it fires.
const SETTLED_JOIN_UNTIL_TICK: u32 = config::TICK_HZ * 180;
/// A raid within this many degrees of the line to the enemy base comes at the front.
const FRONT_HALF_ANGLE_DEGREES: f32 = 45.0;
/// An outmatched response falls back to the dug-in line within this distance of the main.
const LINE_ANCHOR_TILES: f32 = 20.0;
/// The response engages a raid only while it is this close to what Jeff defends.
const LEASH_TILES: f32 = 8.0;
/// A response ends once no raider has been seen for this long.
const RAID_GONE_TICKS: u32 = config::TICK_HZ * 3;
/// Engaged members are re-sent at most this often, unless the raid moved this far.
const ENGAGE_REORDER_TICKS: u32 = config::TICK_HZ * 2;
const ENGAGE_REORDER_TILES: f32 = 3.0;
/// Units added beyond the estimate's bare win: a raid is often seen a few units at a time.
const RESPONSE_MARGIN_UNITS: usize = 1;
/// Mirrors the simulation's range slack, centre to centre.
const SIM_RANGE_SLACK_PX: f32 = 4.0;
/// Rifleman speed in pixels per tick: how long a unit closing one tile is shot at first.
const RIFLEMAN_SPEED_PX: f32 = 1.6;

/// An outranged unit is re-ordered at most this often.
const OUTRANGED_REORDER_TICKS: u32 = config::TICK_HZ;
/// Enemies this close to an outranged unit are the fight it weighs, and friends this close are
/// its side.
const OUTRANGED_FOES_TILES: f32 = 8.0;
const OUTRANGED_FRIENDS_TILES: f32 = 6.0;
/// A Tank that has stood still for three seconds hits from this far.
const STATIONARY_TANK_REACH_TILES: f32 = 14.0;
/// Within this distance of the main's Depot, enemy Tanks are left to the home Tanks.
const MAIN_CORE_TILES: f32 = 16.0;
/// A unit that stepped back stops this far beyond the shooter's reach.
const STEP_BACK_MARGIN_TILES: f32 = 2.0;
/// A unit that stepped in or back returns to its post once no enemy fighter is this close, and
/// not sooner than `DISPLACED_MIN_TICKS`.
const DISPLACED_CLEAR_TILES: f32 = 12.0;
const DISPLACED_MIN_TICKS: u32 = config::TICK_HZ * 3;

/// Sentries stand this far from the main's Depot, this many degrees either side of the line to the
/// enemy base.
const SENTRY_TILES: f32 = 14.0;
const SENTRY_ANGLE_DEGREES: f32 = 80.0;
/// A post the map edge pulls this close to the Depot is not worth a Rifleman.
const SENTRY_MIN_TILES: f32 = 10.0;
/// One sentry is posted for every this many free home Riflemen, so the pocket keeps the rest.
const SENTRY_RIFLEMEN_PER_POST: usize = 2;
const SENTRY_REORDER_TICKS: u32 = config::TICK_HZ * 10;

/// Enemy infantry this close to a Jeff building counts toward the Rifleman answer, for this long
/// after it was seen, until this tick.
const INFANTRY_ALERT_TILES: f32 = 30.0;
const INFANTRY_MEMORY_TICKS: u32 = config::TICK_HZ * 60;
const INFANTRY_ANSWER_UNTIL_TICK: u32 = config::TICK_HZ * 360;
/// The answer starts at this many enemy infantry and aims for this many more home infantry.
const INFANTRY_ALERT_MIN: usize = 3;
const INFANTRY_ANSWER_MARGIN: usize = 2;
const INFANTRY_ANSWER_MAX_EXTRA: usize = 6;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct RaidDefense {
    response: Option<Response>,
    /// Units that stepped in or back from an outranging enemy, and when.
    displaced: BTreeMap<u32, u32>,
    /// When each outranged unit was last ordered.
    stepped: BTreeMap<u32, u32>,
    /// Enemy infantry seen near the bases, by id: when last seen.
    seen_infantry: BTreeMap<u32, u32>,
    /// Sentry Rifleman to post index, and when each was last sent to its post.
    sentries: BTreeMap<u32, usize>,
    sentry_orders: BTreeMap<u32, u32>,
    /// Units handed back to their posts this decision.
    released: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Response {
    started: u32,
    last_seen: u32,
    members: BTreeSet<u32>,
    outmatched: bool,
    gather: Option<(i32, i32)>,
    engaged: bool,
    /// When and where engaged members were last sent.
    order_tick: u32,
    order_point: (i32, i32),
    /// Members last sent to the gathering point.
    gathering: BTreeSet<u32>,
}

impl RaidDefense {
    /// Units this system holds: response members, sentries, and units stepped out of reach.
    pub(in crate::ai_core::decision) fn reserved(&self) -> impl Iterator<Item = u32> + '_ {
        self.response
            .iter()
            .flat_map(|response| response.members.iter().copied())
            .chain(self.sentries.keys().copied())
            .chain(self.displaced.keys().copied())
    }

    pub(in crate::ai_core::decision) fn is_sentry(&self, id: u32) -> bool {
        self.sentries.contains_key(&id)
    }

    /// Units handed back to their posts since the last call.
    fn take_released(&mut self) -> Vec<u32> {
        let mut released = std::mem::take(&mut self.released);
        released.sort_unstable();
        released.dedup();
        released
    }
}

pub(in crate::ai_core::decision) enum RaidAnswer {
    /// Not a raid this system answers: the older incident response handles it.
    NotARaid,
    /// The units answering the raid.
    Handled(Vec<u32>),
}

struct Raid<'a> {
    raiders: Vec<&'a AiEntitySummary>,
    center: (f32, f32),
    /// The farthest any raider reaches, centre to centre, in pixels.
    reach_px: f32,
    armored: bool,
}

fn tile(observation: &AiObservation) -> f32 {
    observation.map.tile_size.max(1) as f32
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    dist2(a.0, a.1, b.0, b.1).sqrt()
}

/// Centre-to-centre distance at which `kind` fires, as in the simulation.
fn reach_px(kind: EntityKind, ts: f32, entrenched: bool) -> f32 {
    let range = config::unit_stats(kind).map_or(0.0, |stats| stats.range_tiles);
    let bonus = if entrenched && rts_rules::balance::is_entrenchment_eligible_infantry(kind) {
        rts_rules::balance::ENTRENCHMENT_RANGE_BONUS_TILES as f32
    } else {
        0.0
    };
    let radius = rts_rules::defs::unit_def(kind).map_or(9.0, |def| def.stats.radius);
    (range + bonus) * ts + radius + SIM_RANGE_SLACK_PX
}

/// How far an enemy shoots: a Tank standing still reaches its stationary range, infantry standing
/// still may be dug in.
fn shooter_reach_px(observation: &AiObservation, enemy: &AiEntitySummary, ts: f32) -> f32 {
    if enemy.kind == EntityKind::Tank && enemy.state != AiEntityState::Move {
        let radius = rts_rules::defs::unit_def(enemy.kind).map_or(9.0, |def| def.stats.radius);
        STATIONARY_TANK_REACH_TILES * ts + radius + SIM_RANGE_SLACK_PX
    } else {
        reach_px(enemy.kind, ts, enemy_may_be_entrenched(observation, enemy))
    }
}

/// Whether an enemy may be dug in: Entrenchment could exist by now and it is standing still.
fn enemy_may_be_entrenched(observation: &AiObservation, enemy: &AiEntitySummary) -> bool {
    observation.tick >= EARLIEST_ENTRENCHMENT_TICK && enemy.state != AiEntityState::Move
}

fn is_fighter(entity: &AiEntitySummary) -> bool {
    entity.hp > 0
        && entity.kind.is_unit()
        && entity.kind != EntityKind::Worker
        && rts_rules::combat::default_weapon_profile(entity.kind).is_some()
}

/// What a raid threatens: building footprints (planned ones too), Engineers and the natural site.
struct Defended {
    sites: Vec<super::envelope::DefendedBuildingSite>,
    points: Vec<(f32, f32)>,
}

impl Defended {
    fn new(observation: &AiObservation, memory: &AiDecisionMemory) -> Self {
        let mut points: Vec<(f32, f32)> = observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Worker && unit.hp > 0)
            .map(|unit| (unit.x, unit.y))
            .collect();
        if let Some(center) = memory.expansion_security.site.and_then(|site| {
            building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
        }) {
            points.push(center);
        }
        Self {
            sites: defended_building_sites(observation, true),
            points,
        }
    }

    /// The defended building centre or point nearest to `point`.
    fn nearest(&self, point: (f32, f32)) -> Option<(f32, f32)> {
        self.sites
            .iter()
            .map(|site| ((site.x, site.y), site.distance2_to_footprint(point)))
            .chain(
                self.points
                    .iter()
                    .map(|p| (*p, dist2(p.0, p.1, point.0, point.1))),
            )
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(nearest, _)| nearest)
    }

    fn distance_px(&self, point: (f32, f32)) -> f32 {
        let sites = self
            .sites
            .iter()
            .map(|site| site.distance2_to_footprint(point));
        let points = self
            .points
            .iter()
            .map(|p| dist2(p.0, p.1, point.0, point.1));
        sites.chain(points).fold(f32::INFINITY, f32::min).sqrt()
    }
}

/// The most valuable group of enemy fighters within `WATCH_TILES` of what Jeff defends.
fn find_raid<'a>(observation: &'a AiObservation, defended: &Defended) -> Option<Raid<'a>> {
    let ts = tile(observation);
    let near: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy))
        .filter(|enemy| defended.distance_px((enemy.x, enemy.y)) <= WATCH_TILES * ts)
        .collect();
    if near.is_empty() {
        return None;
    }
    // Group by chains of raiders within `RAID_LINK_TILES` of each other.
    let link2 = squared(RAID_LINK_TILES * ts);
    let mut group = vec![usize::MAX; near.len()];
    let mut groups = 0;
    for start in 0..near.len() {
        if group[start] != usize::MAX {
            continue;
        }
        group[start] = groups;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            for j in 0..near.len() {
                if group[j] == usize::MAX
                    && dist2(near[i].x, near[i].y, near[j].x, near[j].y) <= link2
                {
                    group[j] = groups;
                    stack.push(j);
                }
            }
        }
        groups += 1;
    }
    let value = |kind: EntityKind| {
        let (steel, oil) = rts_rules::economy::cost(kind);
        (steel + oil).max(1)
    };
    let best = (0..groups).max_by_key(|g| {
        let members = near.iter().zip(&group).filter(|(_, gg)| **gg == *g);
        let total: u32 = members.clone().map(|(enemy, _)| value(enemy.kind)).sum();
        let min_id = members.map(|(enemy, _)| enemy.id).min().unwrap_or(u32::MAX);
        (total, std::cmp::Reverse(min_id))
    })?;
    let raiders: Vec<&AiEntitySummary> = near
        .iter()
        .zip(&group)
        .filter(|(_, g)| **g == best)
        .map(|(enemy, _)| *enemy)
        .collect();
    let n = raiders.len() as f32;
    let center = (
        raiders.iter().map(|enemy| enemy.x).sum::<f32>() / n,
        raiders.iter().map(|enemy| enemy.y).sum::<f32>() / n,
    );
    let reach = raiders
        .iter()
        .map(|enemy| reach_px(enemy.kind, ts, enemy_may_be_entrenched(observation, enemy)))
        .fold(0.0, f32::max);
    Some(Raid {
        armored: raiders.iter().any(|enemy| enemy.kind == EntityKind::Tank),
        raiders,
        center,
        reach_px: reach,
    })
}

fn enemy_fighters(observation: &AiObservation, raiders: &[&AiEntitySummary]) -> Vec<Fighter> {
    let meth = false;
    raiders
        .iter()
        .filter_map(|enemy| fighter(enemy, meth, enemy_may_be_entrenched(observation, enemy)))
        .collect()
}

fn own_fighters(
    observation: &AiObservation,
    dug: &BTreeSet<u32>,
    units: &[&AiEntitySummary],
) -> Vec<Fighter> {
    let meth = observation
        .upgrades
        .contains(&UpgradeKind::Methamphetamines);
    units
        .iter()
        .filter_map(|unit| fighter(unit, meth, dug.contains(&unit.id)))
        .collect()
}

/// Jeff's units dug in by now.
fn dug_in_units(observation: &AiObservation, memory: &AiDecisionMemory) -> BTreeSet<u32> {
    observation
        .owned
        .iter()
        .filter(|unit| {
            memory.estimated_entrenchment_ticks(observation, unit.id)
                >= rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
        })
        .map(|unit| unit.id)
        .collect()
}

/// Whether `ours` kill every one of `theirs` without losing more than they kill.
fn beats(ours: &[Fighter], theirs: &[Fighter], head_start: u32) -> bool {
    if ours.is_empty() {
        return false;
    }
    let estimate = estimate_trade(ours, theirs, head_start);
    estimate.units_killed >= theirs.len() && estimate.value_lost <= estimate.value_killed
}

fn unit_value(kind: EntityKind) -> u32 {
    let (steel, oil) = rts_rules::economy::cost(kind);
    steel + oil
}

/// Whether a raider is already in reach of one of Jeff's dug-in or set-up defenders: the line
/// fights it where it stands.
fn raid_at_the_line(
    observation: &AiObservation,
    dug: &BTreeSet<u32>,
    raid: &Raid<'_>,
    ts: f32,
) -> bool {
    observation.owned.iter().any(|unit| {
        is_fighter(unit)
            && unit.kind != EntityKind::Tank
            && (dug.contains(&unit.id) || unit.kind == EntityKind::MachineGunner)
            && raid.raiders.iter().any(|raider| {
                distance((unit.x, unit.y), (raider.x, raider.y))
                    <= reach_px(unit.kind, ts, dug.contains(&unit.id)) + ts
            })
    })
}

/// The live Jeff's answer to a raid (see the module docs). `local_defenders` are the units home
/// defense may use this decision.
pub(in crate::ai_core::decision) fn respond(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    local_defenders: &[u32],
    map_analysis: Option<&AiMapAnalysis>,
) -> RaidAnswer {
    let ts = tile(observation);
    let tick = observation.tick;
    let defended = Defended::new(observation, memory);
    let dug = dug_in_units(observation, memory);
    let raid = find_raid(observation, &defended)
        .filter(|raid| !raid.armored && !raid_at_the_line(observation, &dug, raid, ts));
    let Some(raid) = raid else {
        // Nothing to answer here. A response keeps its units for a moment, since raiders flicker
        // in and out of sight; then it lets them go.
        if let Some(response) = memory.raid_defense.response.as_ref() {
            if tick.saturating_sub(response.last_seen) < RAID_GONE_TICKS
                && find_raid(observation, &defended).is_none()
            {
                let members: Vec<u32> = response.members.iter().copied().collect();
                return RaidAnswer::Handled(members);
            }
            end_response(memory);
        }
        return RaidAnswer::NotARaid;
    };

    let owned = |id: u32| observation.owned.iter().find(|unit| unit.id == id);
    let defenders: BTreeSet<u32> = local_defenders.iter().copied().collect();
    let posted = |id: u32| {
        memory.raid_defense.is_sentry(id)
            || memory.expansion_security.riflemen.contains(&id)
            || memory.later_bases.guards.contains(&id)
    };
    let candidates: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        // Tanks stay with the home armour, which parks them for stationary range.
        .filter(|unit| defenders.contains(&unit.id) && unit.is_complete && is_fighter(unit))
        .filter(|unit| unit.kind != EntityKind::Tank)
        .filter(|unit| {
            let reach = if posted(unit.id) {
                POSTED_RESPONSE_TILES
            } else {
                RESPONSE_TILES
            };
            distance((unit.x, unit.y), raid.center) <= reach * ts
        })
        .collect();
    let in_reach = |unit: &AiEntitySummary| {
        let reach = reach_px(unit.kind, ts, dug.contains(&unit.id));
        raid.raiders
            .iter()
            .any(|raider| distance((unit.x, unit.y), (raider.x, raider.y)) <= reach)
    };

    let mut response = memory.raid_defense.response.take().unwrap_or(Response {
        started: tick,
        last_seen: tick,
        members: BTreeSet::new(),
        outmatched: false,
        gather: None,
        engaged: false,
        order_tick: 0,
        order_point: (0, 0),
        gathering: BTreeSet::new(),
    });
    response.last_seen = tick;
    response
        .members
        .retain(|id| candidates.iter().any(|unit| unit.id == *id));
    // Units already in reach are in the fight whatever the plan.
    for unit in &candidates {
        if in_reach(unit) {
            response.members.insert(unit.id);
        }
    }
    // Then the nearest, mobile units first, until the estimate beats the raid.
    let theirs = enemy_fighters(observation, &raid.raiders);
    let head_start = if theirs.iter().any(|enemy| enemy.entrenched) {
        (ts / RIFLEMAN_SPEED_PX).ceil() as u32
    } else {
        0
    };
    // The response is made of the mobile units, nearest first, until the estimate beats the raid.
    // Dug-in units and Machine Gunners hold their posts and fight what comes into reach; against a
    // raid on a flank in the opening minutes they join last, when the mobile units alone cannot
    // beat it: the starting Riflemen are out on the rush and the guns are all the home has.
    let settled =
        |unit: &AiEntitySummary| dug.contains(&unit.id) || unit.kind == EntityKind::MachineGunner;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let flank = observation.tick < SETTLED_JOIN_UNTIL_TICK
        && enemy_start(observation).is_none_or(|enemy| {
            let (Some(to_raid), Some(to_enemy)) = (
                normalized_direction(home, raid.center),
                normalized_direction(home, enemy),
            ) else {
                return false;
            };
            to_raid.0 * to_enemy.0 + to_raid.1 * to_enemy.1
                < FRONT_HALF_ANGLE_DEGREES.to_radians().cos()
        });
    let mut queue: Vec<&AiEntitySummary> = candidates
        .iter()
        .copied()
        .filter(|unit| !response.members.contains(&unit.id) && (flank || !settled(unit)))
        .collect();
    queue.sort_by(|left, right| {
        settled(left)
            .cmp(&settled(right))
            .then_with(|| {
                distance((left.x, left.y), raid.center)
                    .total_cmp(&distance((right.x, right.y), raid.center))
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    let members_now = |members: &BTreeSet<u32>| -> Vec<&AiEntitySummary> {
        members.iter().filter_map(|id| owned(*id)).collect()
    };
    let mut won = beats(
        &own_fighters(observation, &dug, &members_now(&response.members)),
        &theirs,
        head_start,
    );
    let mut picked = response.members.clone();
    let mut extra = 0;
    for unit in &queue {
        if won {
            if extra >= RESPONSE_MARGIN_UNITS {
                break;
            }
            extra += 1;
        }
        picked.insert(unit.id);
        if !won {
            won = beats(
                &own_fighters(observation, &dug, &members_now(&picked)),
                &theirs,
                head_start,
            );
        }
    }
    // Once engaged a response sees its fight through; otherwise a raid nobody can beat is met on
    // the dug-in line rather than in the open.
    response.outmatched = !won && !response.engaged;
    if response.outmatched {
        response.members.extend(
            queue
                .iter()
                .filter(|unit| !settled(unit))
                .map(|unit| unit.id),
        );
    } else {
        response.members = picked;
    }

    let near_defended = defended.distance_px(raid.center) <= LEASH_TILES * ts;
    let mut assigned = Vec::new();
    let infantry: Vec<u32> = response.members.iter().copied().collect();

    // Where the infantry gathers: beside what the raid threatens, on the side facing home, or
    // just outside the raid's reach when the raid is already there. Outmatched: on the dug-in line.
    let gather_distance = raid.reach_px + GATHER_MARGIN_TILES * ts;
    let desired = if response.outmatched {
        line_anchor(observation, &dug, home, ts)
    } else {
        let threatened = defended.nearest(raid.center).unwrap_or(home);
        let inward = normalized_direction(threatened, home).unwrap_or((0.0, 0.0));
        let beside = (
            threatened.0 + inward.0 * GATHER_MARGIN_TILES * ts,
            threatened.1 + inward.1 * GATHER_MARGIN_TILES * ts,
        );
        if distance(beside, raid.center) >= gather_distance {
            beside
        } else {
            let back = normalized_direction(raid.center, home)
                .or_else(|| normalized_direction(raid.center, beside))
                .unwrap_or((0.0, 1.0));
            (
                raid.center.0 + back.0 * gather_distance,
                raid.center.1 + back.1 * gather_distance,
            )
        }
    };
    let desired = clamp_to_map(desired, observation.map);
    let shifted = response
        .gather
        .is_none_or(|(x, y)| distance(desired, (x as f32, y as f32)) > GATHER_SHIFT_TILES * ts);
    if shifted {
        let point =
            clear_mobile_defensive_position(observation, map_analysis, desired).unwrap_or(desired);
        response.gather = Some((point.0 as i32, point.1 as i32));
        response.gathering.clear();
    }
    let gather = response
        .gather
        .map(|(x, y)| (x as f32, y as f32))
        .unwrap_or(desired);
    let travelling = infantry
        .iter()
        .filter_map(|id| owned(*id))
        .any(|unit| !in_reach(unit));

    if !response.engaged && near_defended && !response.outmatched {
        let value = |unit: &AiEntitySummary| unit_value(unit.kind);
        let total: u32 = infantry.iter().filter_map(|id| owned(*id)).map(value).sum();
        let gathered: Vec<&AiEntitySummary> = infantry
            .iter()
            .filter_map(|id| owned(*id))
            .filter(|unit| {
                in_reach(unit) || distance((unit.x, unit.y), gather) <= GATHERED_TILES * ts
            })
            .collect();
        let gathered_value: u32 = gathered.iter().copied().map(value).sum();
        let share_in = gathered_value * GATHERED_SHARE.1 >= total * GATHERED_SHARE.0;
        let timed_out = tick.saturating_sub(response.started) >= GATHER_TIMEOUT_TICKS
            && beats(
                &own_fighters(observation, &dug, &gathered),
                &theirs,
                head_start,
            );
        response.engaged = share_in || timed_out || !travelling;
    }
    if response.engaged && !near_defended {
        // The raid backed off: hold the gathering point instead of following it.
        response.engaged = false;
        response.gathering.clear();
    }

    if response.engaged {
        let point = raid.center;
        let reorder = tick.saturating_sub(response.order_tick) >= ENGAGE_REORDER_TICKS
            || distance(
                point,
                (response.order_point.0 as f32, response.order_point.1 as f32),
            ) > ENGAGE_REORDER_TILES * ts;
        if reorder {
            if let Some(units) =
                actions::attack_move_units(actions, infantry.iter().copied(), point.0, point.1)
            {
                assigned.extend(units);
            }
            response.order_tick = tick;
            response.order_point = (point.0 as i32, point.1 as i32);
        }
    } else {
        let mut hold = Vec::new();
        let mut go = Vec::new();
        let mut fight = Vec::new();
        for id in &infantry {
            let Some(unit) = owned(*id) else {
                continue;
            };
            let shot_at = raid.raiders.iter().any(|raider| {
                distance((unit.x, unit.y), (raider.x, raider.y))
                    <= reach_px(
                        raider.kind,
                        ts,
                        enemy_may_be_entrenched(observation, raider),
                    )
            });
            if in_reach(unit) {
                // Already in the fight: answer it from here.
                if unit.state == AiEntityState::Move {
                    hold.push(*id);
                }
            } else if shot_at && !response.outmatched {
                // Shot from beyond its own reach by a raid the response beats: close in.
                if unit.state != AiEntityState::Attack {
                    fight.push(*id);
                }
            } else if distance((unit.x, unit.y), gather) > GATHERED_TILES * ts * 0.5 {
                if !response.gathering.contains(id) || unit.state == AiEntityState::Idle {
                    go.push(*id);
                }
            } else if unit.state == AiEntityState::Move {
                hold.push(*id);
            }
        }
        if let Some(units) = actions::hold_position_units(actions, hold) {
            assigned.extend(units);
        }
        if let Some(units) =
            actions::attack_move_units(actions, fight, raid.center.0, raid.center.1)
        {
            assigned.extend(units);
        }
        if let Some(units) = actions::move_units(actions, go, gather.0, gather.1) {
            response.gathering.extend(units.iter().copied());
            assigned.extend(units);
        }
    }

    assigned.extend(response.members.iter().copied());
    assigned.sort_unstable();
    assigned.dedup();
    memory.raid_defense.response = Some(response);
    RaidAnswer::Handled(assigned)
}

/// Where outmatched infantry falls back: the middle of the dug-in Riflemen and Machine Gunners near
/// the main, or the main's Depot.
fn line_anchor(
    observation: &AiObservation,
    dug: &BTreeSet<u32>,
    home: (f32, f32),
    ts: f32,
) -> (f32, f32) {
    let line: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| is_fighter(unit) && unit.kind != EntityKind::Tank)
        .filter(|unit| dug.contains(&unit.id) || unit.kind == EntityKind::MachineGunner)
        .filter(|unit| distance((unit.x, unit.y), home) <= LINE_ANCHOR_TILES * ts)
        .map(|unit| (unit.x, unit.y))
        .collect();
    if line.is_empty() {
        return home;
    }
    let n = line.len() as f32;
    (
        line.iter().map(|p| p.0).sum::<f32>() / n,
        line.iter().map(|p| p.1).sum::<f32>() / n,
    )
}

/// The nearest living enemy's start, in world pixels.
fn enemy_start(observation: &AiObservation) -> Option<(f32, f32)> {
    let own_team = observation
        .players
        .iter()
        .find(|player| player.id == observation.player_id)
        .map(|player| player.team_id);
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation
        .players
        .iter()
        .filter(|player| player.id != observation.player_id && player.is_alive)
        .filter(|player| own_team.is_none_or(|team| player.team_id != team))
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))
        .min_by(|left, right| distance(*left, home).total_cmp(&distance(*right, home)))
}

fn end_response(memory: &mut AiDecisionMemory) {
    if let Some(response) = memory.raid_defense.response.take() {
        memory.raid_defense.released.extend(response.members);
    }
}

/// Units in `units` standing still while an enemy shoots them from beyond their own reach: each
/// steps in to fire when the fight around it is won, and otherwise steps back out of reach. Returns
/// the units this system holds this decision (ordered now or recently).
fn answer_outranged(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    units: &[u32],
) -> Vec<u32> {
    let ts = tile(observation);
    let tick = observation.tick;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let dug = dug_in_units(observation, memory);
    let enemies: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy))
        .collect();
    // Units that stepped in or back go home once nothing is near.
    let alive: BTreeSet<u32> = observation.owned.iter().map(|unit| unit.id).collect();
    memory
        .raid_defense
        .displaced
        .retain(|id, _| alive.contains(id));
    memory
        .raid_defense
        .stepped
        .retain(|id, _| alive.contains(id));
    let clear: Vec<u32> = memory
        .raid_defense
        .displaced
        .iter()
        .filter(|(id, since)| {
            tick.saturating_sub(**since) >= DISPLACED_MIN_TICKS
                && observation
                    .owned
                    .iter()
                    .find(|unit| unit.id == **id)
                    .is_none_or(|unit| {
                        enemies.iter().all(|enemy| {
                            distance((unit.x, unit.y), (enemy.x, enemy.y))
                                > DISPLACED_CLEAR_TILES * ts
                        })
                    })
        })
        .map(|(id, _)| *id)
        .collect();
    for id in clear {
        memory.raid_defense.displaced.remove(&id);
        memory.raid_defense.released.push(id);
    }

    let mut held = Vec::new();
    for id in units {
        let Some(unit) = observation.owned.iter().find(|unit| unit.id == *id) else {
            continue;
        };
        if !unit.is_complete
            || unit.hp == 0
            || !matches!(unit.kind, EntityKind::Rifleman | EntityKind::Panzerfaust)
        {
            continue;
        }
        if memory
            .raid_defense
            .stepped
            .get(id)
            .is_some_and(|last| tick.saturating_sub(*last) < OUTRANGED_REORDER_TICKS)
        {
            held.push(*id);
            continue;
        }
        if unit.state != AiEntityState::Idle {
            continue;
        }
        let own_dug_in = dug.contains(id);
        let own_reach = reach_px(unit.kind, ts, own_dug_in);
        let at = |enemy: &&AiEntitySummary| distance((unit.x, unit.y), (enemy.x, enemy.y));
        if enemies.iter().any(|enemy| at(enemy) <= own_reach) {
            continue;
        }
        // Tanks shelling the main are the home Tanks' fight; elsewhere infantry clears out.
        let in_main = distance((unit.x, unit.y), home) <= MAIN_CORE_TILES * ts;
        let shooter = enemies
            .iter()
            .filter(|enemy| !(in_main && enemy.kind == EntityKind::Tank))
            .filter(|enemy| at(enemy) <= shooter_reach_px(observation, enemy, ts) + 0.5 * ts)
            .min_by(|left, right| at(left).total_cmp(&at(right)).then(left.id.cmp(&right.id)));
        let Some(shooter) = shooter else {
            continue;
        };
        let friends: Vec<&AiEntitySummary> = observation
            .owned
            .iter()
            .filter(|other| is_fighter(other) && other.is_complete)
            .filter(|other| {
                distance((unit.x, unit.y), (other.x, other.y)) <= OUTRANGED_FRIENDS_TILES * ts
            })
            .collect();
        let foes: Vec<&AiEntitySummary> = enemies
            .iter()
            .copied()
            .filter(|enemy| at(enemy) <= OUTRANGED_FOES_TILES * ts)
            .collect();
        let theirs = enemy_fighters(observation, &foes);
        // Closing the gap means being shot for about a tile first.
        let head_start = (ts / RIFLEMAN_SPEED_PX).ceil() as u32;
        // Riflemen do not take on a Tank.
        let step_in = !(shooter.kind == EntityKind::Tank && unit.kind == EntityKind::Rifleman)
            && beats(
                &own_fighters(observation, &dug, &friends),
                &theirs,
                head_start,
            );
        let ordered = if step_in {
            actions::attack_move_units(actions, [*id], shooter.x, shooter.y)
        } else {
            let away = normalized_direction((shooter.x, shooter.y), (unit.x, unit.y))
                .unwrap_or((0.0, 1.0));
            let homeward = normalized_direction((unit.x, unit.y), home).unwrap_or(away);
            let direction = normalized_direction(
                (0.0, 0.0),
                (away.0 + 0.5 * homeward.0, away.1 + 0.5 * homeward.1),
            )
            .unwrap_or(away);
            let back = shooter_reach_px(observation, shooter, ts) + STEP_BACK_MARGIN_TILES * ts;
            let desired = clamp_to_map(
                (
                    shooter.x + direction.0 * back,
                    shooter.y + direction.1 * back,
                ),
                observation.map,
            );
            let point = clear_mobile_defensive_position(observation, map_analysis, desired)
                .unwrap_or(desired);
            actions::move_units(actions, [*id], point.0, point.1)
        };
        if ordered.is_some() {
            memory.raid_defense.stepped.insert(*id, tick);
            memory.raid_defense.displaced.insert(*id, tick);
            held.push(*id);
        }
    }
    held.extend(memory.raid_defense.displaced.keys().copied());
    held.sort_unstable();
    held.dedup();
    held
}

/// Two posts off the main's flanks, `SENTRY_ANGLE_DEGREES` either side of the line to the enemy
/// base, where a Rifleman sees a raid coming before it reaches the extractors.
fn sentry_posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    enemy_base: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = tile(observation);
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let Some(axis) = normalized_direction(home, enemy_base) else {
        return Vec::new();
    };
    let base_angle = axis.1.atan2(axis.0);
    [1.0_f32, -1.0]
        .into_iter()
        .filter_map(|side| {
            let angle = base_angle + side * SENTRY_ANGLE_DEGREES.to_radians();
            let desired = clamp_to_map(
                (
                    home.0 + angle.cos() * SENTRY_TILES * ts,
                    home.1 + angle.sin() * SENTRY_TILES * ts,
                ),
                observation.map,
            );
            if distance(desired, home) < SENTRY_MIN_TILES * ts {
                return None;
            }
            clear_mobile_defensive_position(observation, map_analysis, desired)
        })
        .collect()
}

/// Posts up to two sentries from `free_riflemen` (home Riflemen no other system holds) and keeps
/// them there. Returns the sentries.
fn post_sentries(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    enemy_base: Option<(f32, f32)>,
    free_riflemen: &[u32],
) -> Vec<u32> {
    let ts = tile(observation);
    let tick = observation.tick;
    let posts = enemy_base
        .map(|base| sentry_posts(observation, map_analysis, base))
        .unwrap_or_default();
    let alive = |id: u32| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == id && unit.hp > 0)
    };
    let sentries = &mut memory.raid_defense.sentries;
    sentries.retain(|id, post| alive(*id) && *post < posts.len());
    memory
        .raid_defense
        .sentry_orders
        .retain(|id, _| sentries.contains_key(id));
    let free: Vec<u32> = free_riflemen
        .iter()
        .copied()
        .filter(|id| !sentries.contains_key(id))
        .collect();
    // The post nearer the main's Steel first: raids go for the extractors.
    let wanted = ((free.len() + sentries.len()) / SENTRY_RIFLEMEN_PER_POST).min(posts.len());
    let mut order: Vec<usize> = (0..posts.len()).collect();
    if let Some(steel) = main_steel_cluster_center(observation) {
        order.sort_by(|left, right| {
            distance(posts[*left], steel).total_cmp(&distance(posts[*right], steel))
        });
    }
    {
        let mut free = free;
        for index in order {
            if sentries.len() >= wanted {
                break;
            }
            if sentries.values().any(|taken| *taken == index) {
                continue;
            }
            let post = &posts[index];
            let nearest = free.iter().copied().min_by(|left, right| {
                let at = |id: u32| {
                    observation
                        .owned
                        .iter()
                        .find(|unit| unit.id == id)
                        .map_or(f32::INFINITY, |unit| distance((unit.x, unit.y), *post))
                };
                at(*left).total_cmp(&at(*right)).then(left.cmp(right))
            });
            if let Some(id) = nearest {
                free.retain(|other| *other != id);
                sentries.insert(id, index);
            }
        }
    }
    let assignments: Vec<(u32, usize)> = sentries.iter().map(|(id, post)| (*id, *post)).collect();
    for (id, post) in assignments {
        let Some(unit) = observation.owned.iter().find(|unit| unit.id == id) else {
            continue;
        };
        let point = posts[post];
        if distance((unit.x, unit.y), point) > ts {
            let due = memory
                .raid_defense
                .sentry_orders
                .get(&id)
                .is_none_or(|last| tick.saturating_sub(*last) >= SENTRY_REORDER_TICKS);
            if due || unit.state == AiEntityState::Idle {
                actions::move_units(actions, [id], point.0, point.1);
                memory.raid_defense.sentry_orders.insert(id, tick);
            }
        } else if unit.state == AiEntityState::Move {
            actions::hold_position_units(actions, [id]);
        }
    }
    memory.raid_defense.sentries.keys().copied().collect()
}

/// Notes enemy infantry seen near Jeff's buildings.
fn note_enemy_infantry(observation: &AiObservation, memory: &mut AiDecisionMemory) {
    let ts = tile(observation);
    let tick = observation.tick;
    let sites = defended_building_sites(observation, false);
    for enemy in &observation.visible_enemies {
        if enemy.hp == 0 || !rts_rules::balance::is_entrenchment_eligible_infantry(enemy.kind) {
            continue;
        }
        let near = sites.iter().any(|site| {
            site.distance2_to_footprint((enemy.x, enemy.y)) <= squared(INFANTRY_ALERT_TILES * ts)
        });
        if near {
            memory.raid_defense.seen_infantry.insert(enemy.id, tick);
        }
    }
    memory
        .raid_defense
        .seen_infantry
        .retain(|_, seen| tick.saturating_sub(*seen) <= INFANTRY_MEMORY_TICKS);
}

/// The live Jeff only (`profile_id`): notes enemy infantry seen near the bases, and while it
/// outnumbers Jeff's home infantry (queued Riflemen included) in the opening minutes, Riflemen lead
/// `priorities` and the returned count (queued included) is what to train up to, ahead of tech.
pub(in crate::ai_core::decision) fn rifle_answer(
    profile_id: &str,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    priorities: &mut Vec<EntityKind>,
) -> Option<usize> {
    if profile_id != JEFFS_AI_ID {
        return None;
    }
    note_enemy_infantry(observation, memory);
    if observation.tick >= INFANTRY_ANSWER_UNTIL_TICK {
        return None;
    }
    let riflemen = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman)
        .count();
    let seen = memory.raid_defense.seen_infantry.len();
    if seen < INFANTRY_ALERT_MIN {
        return None;
    }
    let queued: usize = observation
        .owned
        .iter()
        .filter(|building| {
            building.is_complete && building.production_kind == Some(EntityKind::Rifleman)
        })
        .map(|building| building.production_queue_len.unwrap_or(0))
        .sum();
    let home_infantry = observation
        .owned
        .iter()
        .filter(|unit| {
            matches!(unit.kind, EntityKind::Rifleman | EntityKind::MachineGunner)
                && unit.is_complete
                && unit.hp > 0
                && !memory.opening_rush.is_reserved(unit.id)
                && !memory.containment.active_riflemen.contains(&unit.id)
        })
        .count();
    let wanted = seen + INFANTRY_ANSWER_MARGIN;
    let short = wanted
        .saturating_sub(home_infantry + queued)
        .min(INFANTRY_ANSWER_MAX_EXTRA);
    if short == 0 {
        return None;
    }
    priorities.retain(|unit| *unit != EntityKind::Rifleman);
    priorities.insert(0, EntityKind::Rifleman);
    Some(riflemen + queued + short)
}

/// The live Jeff's home infantry once the raid response has taken its units: infantry shot from
/// beyond its reach steps in or back, and sentries watch the main's flanks. Neither touches units
/// in `held` (other systems') or `assigned` (home defense's this decision); both add theirs to
/// `assigned`. Units handed back to their posts are released to the pocket.
pub(in crate::ai_core::decision) fn plan_home_infantry(
    (actions, observation, map_analysis, intents): LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    enemy_base: Option<(f32, f32)>,
    held: &BTreeSet<u32>,
    assigned: &mut BTreeSet<u32>,
) {
    let held: BTreeSet<u32> = held
        .iter()
        .copied()
        .chain(memory.containment.active_riflemen.iter().copied())
        .chain(memory.route_line.picket())
        .collect();
    let infantry: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| matches!(unit.kind, EntityKind::Rifleman | EntityKind::Panzerfaust))
        .map(|unit| unit.id)
        .filter(|id| !assigned.contains(id) && !held.contains(id))
        .collect();
    let stepping = answer_outranged(actions, observation, memory, map_analysis, &infantry);
    if !stepping.is_empty() {
        assigned.extend(stepping.iter().copied());
        intents.push(AiIntent::Attack { units: stepping });
    }
    let free_riflemen: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .map(|unit| unit.id)
        .filter(|id| {
            !assigned.contains(id)
                && !held.contains(id)
                && !memory.expansion_security.riflemen.contains(id)
                && !memory.later_bases.guards.contains(id)
        })
        .collect();
    let sentries = post_sentries(
        actions,
        observation,
        memory,
        map_analysis,
        enemy_base,
        &free_riflemen,
    );
    assigned.extend(sentries);
    let released = memory.raid_defense.take_released();
    if !released.is_empty() {
        // Clears the live adapter's cached staging so the pocket places them again.
        intents.push(AiIntent::Assemble { units: released });
    }
}

#[cfg(test)]
#[path = "raid_tests.rs"]
mod tests;
