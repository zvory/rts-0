//! The live Jeff's pressure on the enemy's third base (`JEFFS_AI_ID` only).
//!
//! AI 2.1 orders a third base around 3:15 on almost every map and leaves it alone: on Doppelganger
//! no AI fighter came within 30 tiles of it from 6:00 to 8:00 in any game. Its bases answer only
//! enemies inside their defense zone (6 tiles of a building, extractors included, 5 of a worker, 12
//! of the main), and only with units already inside it. Extractors mine only while a finished
//! Depot anchors them, and a Depot is four Tank shells.
//!
//! So from 5:30, while Jeff has fewer than three bases and its own third-base plan has no site in
//! hand, a party of two Tanks from the army (never the home reserve), five Riflemen and the Scout
//! Car walks a flank route to the enemy's predicted third. AI 2.1 may stand its Depot anywhere that
//! mines a patch (11 tiles), even outside the ring of patches, so the party first finds it: the
//! Scout Car sweeps every spot that could hold it while the rest wait at the staging point. A base
//! with nothing at any such spot is empty and the party goes home. Once the Depot is seen the party
//! parks outside the zone around its exact position, the Riflemen and the Scout Car spotting,
//! shells it from the 14 tiles a parked Tank reaches, then stays to shoot whatever comes to rebuild
//! it. A replacement Scout Car is queued while the party has the
//! old one. The party comes home when it meets a fight it loses, when its last Tank falls, or when
//! enemy Tanks outgun the home Tanks.

use super::geometry::{clamp_to_map, dist2, normalized_direction, tile_center};
use super::opening_rush::{estimate_trade, fighter, LiveStep};
use super::*;
use locate::{begin_locate, field_target, locate, retarget, sight};
use merge::{begin_merge, merge};
use scout::{begin_scout, keep_watch, scout_clear, scout_trip};

mod locate;
mod merge;
mod scout;

/// The party leaves from 5:30, to reach a third 90-150 tiles off by about 7:00.
const LAUNCH_FROM_TICK: u32 = config::TICK_HZ * 330;
const PARTY_TANKS: usize = 2;
const PARTY_RIFLEMEN: usize = 5;
/// A base this close to the enemy main sits inside the main's own defense zone and garrison.
const MIN_TARGET_FROM_ENEMY_MAIN_TILES: f32 = 35.0;
/// The party stays this far from every enemy building, extractors included (AI 2.1's zone reaches
/// 6 tiles past a building), and from every resource patch near the target, which may hold an
/// extractor not yet seen.
const ZONE_CLEARANCE_TILES: f32 = 7.0;
/// AI 2.1's zone reaches 5 tiles past each of its workers.
const WORKER_CLEARANCE_TILES: f32 = 6.5;
/// Resource patches this close to the target belong to its base.
const BASE_RESOURCE_TILES: f32 = 14.0;
/// Posts around the Depot's centre, tried in order. A parked Tank reaches 14 tiles to a building's
/// edge, and the Depot's edge stands 1.5 tiles out, so 14.5 keeps a margin; Riflemen stand inside
/// their 11-tile sight, the Scout Car inside its 15.
const TANK_POST_TILES: [f32; 4] = [14.25, 13.75, 14.75, 13.0];
/// Spotters stand close enough to see a Depot a few tiles off the predicted site.
const SPOTTER_POST_TILES: [f32; 4] = [9.0, 9.5, 8.5, 10.0];
const SCOUT_POST_TILES: [f32; 3] = [12.0, 12.5, 11.5];
/// Posts stay on the party's side of the base: within this angle of the way it came in.
const POST_CONE_DEGREES: f32 = 75.0;
const POST_SEPARATION_TILES: f32 = 1.5;
const POST_REACHED_TILES: f32 = 1.25;
/// A parked Tank reaches 14 tiles after three seconds without moving.
const STATIONARY_READY_TICKS: u32 = config::TICK_HZ * 3 + 9;
/// Parked Tanks shoot units this close and buildings whose edge is this close (measured
/// conservatively from the centre minus `DEPOT_HALF_TILES`).
const TANK_FIRE_TILES: f32 = 13.5;
const DEPOT_HALF_TILES: f32 = 1.5;
const TANK_SHELL_DAMAGE: u32 = 60;
/// A losing party still fires the volley that finishes a Depot, unless enemy Tanks are this close.
const FINISH_CLEAR_OF_TANKS_TILES: f32 = 9.0;
/// A party turned back this close to home disbands on the spot.
const DISBAND_NEAR_HOME_TILES: f32 = 35.0;
/// An attack-move is re-aimed only when the enemy moved this far.
const REAIM_TILES: f32 = 3.0;
/// The party gathers this far from the target before it takes its posts.
const STAGING_TILES: f32 = 18.0;
/// The flank waypoint stands this far off the line between the two mains, which AI 2.1's attacks
/// follow.
const FLANK_OFFSET_TILES: f32 = 24.0;
const ROUTE_STRIDE_TILES: usize = 10;
/// A leg is done once every member is this close to its waypoint, or after the timeout.
const LEG_REACHED_TILES: f32 = 6.0;
const LEG_TIMEOUT_TICKS: u32 = config::TICK_HZ * 40;
const MARCH_TIMEOUT_TICKS: u32 = config::TICK_HZ * 240;
/// Enemy fighters this close to a member are weighed against the party.
const THREAT_TILES: f32 = 13.0;
/// A unit sees a spot this far inside its sight radius.
const SIGHT_MARGIN_TILES: f32 = 1.5;
/// Spots a Depot could stand on are tried on this grid.
const DEPOT_SEARCH_STEP_TILES: f32 = 2.0;
/// The locator looks at a spot from this far inside its sight, from outside the base.
const VIEW_BACKOFF_TILES: f32 = 2.5;
const LOCATE_TIMEOUT_TICKS: u32 = config::TICK_HZ * 90;
/// A remembered Depot this close to the siege target is the same Depot.
const DEPOT_SAME_TILES: f32 = 3.0;
/// After the Depot falls the party stays this long to shoot a rebuild.
const HOLD_AFTER_KILL_TICKS: u32 = config::TICK_HZ * 60;
const SIEGE_TIMEOUT_TICKS: u32 = config::TICK_HZ * 300;
const RETURN_TIMEOUT_TICKS: u32 = config::TICK_HZ * 150;
const HOME_ARRIVED_TILES: f32 = 16.0;
const RELAUNCH_COOLDOWN_TICKS: u32 = config::TICK_HZ * 90;
/// Sites checked and found empty are skipped when this close.
const SITE_SAME_TILES: f32 = 8.0;
/// After this many bases found empty the party goes out only to a Depot actually seen.
const MAX_EMPTY_BASES: u32 = 2;
/// Only a Scout Car with this share of its hit points goes scouting.
const SCOUT_LAUNCH_HP_PERCENT: u32 = 90;
/// A base found defended is left alone this long.
const DEFENDED_SKIP_TICKS: u32 = config::TICK_HZ * 180;
/// A looker watches a found Depot this long at most before the garrison it sees decides.
const SURVEY_TIMEOUT_TICKS: u32 = config::TICK_HZ * 15;
/// Enemy fighters this close to the Depot are its garrison.
const GARRISON_TILES: f32 = 16.0;
/// A search out of time that has seen this share of the spots, and no Depot, found the base empty.
const EMPTY_SEEN_PERCENT: usize = 75;
/// On the march and on the way to the push, enemies this close are fought; farther ones are let
/// pass while the party holds, for up to `AVOID_MAX_TICKS`.
const ENGAGED_TILES: f32 = 8.0;
const AVOID_MAX_TICKS: u32 = config::TICK_HZ * 20;
/// Patches this close to another of a cluster belong to the same base. A base's patches come in
/// groups up to 9.4 tiles apart; different bases' patches stand at least 17.7 apart.
const BASE_MERGE_TILES: f32 = 12.0;

/// A point to keep clear of and its radius, in world pixels.
type KeepOut = ((f32, f32), f32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    /// The Scout Car out alone, finding the target base's Depot before any Tank leaves.
    Scout,
    March,
    Locate,
    Siege,
    /// Over at the target, on the way to join the push.
    Merge,
    Return,
}

/// An enemy base beyond its main: the centre of its patches, the patches, and where its Depot was
/// seen.
#[derive(Clone, Debug, PartialEq)]
struct EnemyBase {
    center: (f32, f32),
    patches: Vec<(f32, f32)>,
    depot: Option<(f32, f32)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct ThirdPressure {
    phase: Phase,
    tanks: Vec<u32>,
    riflemen: Vec<u32>,
    scout: Option<u32>,
    /// The target base: its Depot's position once seen, its patches' centre before.
    target: Option<(i32, i32)>,
    depot: Option<u32>,
    /// The target base's resource patches.
    area: Vec<(i32, i32)>,
    /// Spots near them that could hold the Depot and nobody has seen yet.
    unseen: Vec<(i32, i32)>,
    /// The members looking for the Depot.
    locator: Vec<u32>,
    /// When the looker began sizing up the found Depot's defenders.
    survey_since: Option<u32>,
    /// The looker waits with the party until this tick after falling back from contact.
    fallback_until: u32,
    /// Bases found defended, and when.
    defended: Vec<((i32, i32), u32)>,
    /// Bases found with no Depot at any spot that could hold one.
    empty_bases: u32,
    /// When the Scout Car began sweeping the base for its Depot.
    search_since: Option<u32>,
    /// Where the Scout Car watches the found Depot from while the party marches.
    watch: Option<(i32, i32)>,
    /// How often the Scout Car turned away from enemies on this trip.
    flees: u32,
    /// Jeff wants a Scout Car for the raid (the push holds the only one).
    wants_scout: bool,
    /// Since when the party has held to let enemies pass.
    avoid_since: Option<u32>,
    /// How many spots the current search began with.
    spots_total: usize,
    route: Vec<(i32, i32)>,
    leg: usize,
    leg_since: u32,
    phase_since: u32,
    /// When each Tank was first found parked on its post.
    parked_since: BTreeMap<u32, u32>,
    /// Where each member was last sent.
    sent: BTreeMap<u32, (i32, i32)>,
    /// Where each member last attack-moved.
    aimed: BTreeMap<u32, (i32, i32)>,
    /// Each member's siege post, kept while it stays clear.
    posts: BTreeMap<u32, (i32, i32)>,
    /// Members told to hold on their post.
    held: BTreeSet<u32>,
    killed_at: Option<u32>,
    /// Sites found empty.
    checked: Vec<(i32, i32)>,
    next_launch: u32,
    /// Enemy Depots seen away from the enemy main.
    seen_depots: Vec<(i32, i32)>,
    /// This decision's trace notes.
    pub(super) notes: Vec<String>,
}

impl ThirdPressure {
    pub(in crate::ai_core::decision) fn reserved(&self) -> impl Iterator<Item = u32> + '_ {
        self.tanks
            .iter()
            .chain(&self.riflemen)
            .copied()
            .chain(self.scout)
    }

    /// The party's Tanks the push counts on: all of them while the party is out and not heading
    /// home.
    pub(in crate::ai_core::decision) fn pledged_tanks(&self) -> usize {
        match self.phase {
            Phase::March | Phase::Locate | Phase::Siege | Phase::Merge => self.tanks.len(),
            Phase::Idle | Phase::Scout | Phase::Return => 0,
        }
    }

    /// The party's Tanks on their way to meet the push.
    pub(in crate::ai_core::decision) fn joining_tanks(&self) -> usize {
        if self.phase == Phase::Merge {
            self.tanks.len()
        } else {
            0
        }
    }

    pub(in crate::ai_core::decision) fn is_reserved(&self, id: u32) -> bool {
        self.tanks.contains(&id) || self.riflemen.contains(&id) || self.scout == Some(id)
    }

    fn members(&self) -> Vec<u32> {
        self.reserved().collect()
    }

    /// The members that march and fight together: all but a Scout Car keeping watch at the base.
    fn fighting_members(&self) -> Vec<u32> {
        if self.watch.is_some() {
            self.tanks.iter().chain(&self.riflemen).copied().collect()
        } else {
            self.members()
        }
    }

    /// Clears what the trip learned about its target, for a new target.
    fn reset_trip(&mut self, tick: u32) {
        self.depot = None;
        self.killed_at = None;
        self.unseen.clear();
        self.spots_total = 0;
        self.search_since = None;
        self.locator.clear();
        self.survey_since = None;
        self.watch = None;
        self.fallback_until = 0;
        self.flees = 0;
        self.avoid_since = None;
        self.leg = 0;
        self.leg_since = tick;
        self.clear_orders();
    }

    fn clear_orders(&mut self) {
        self.parked_since.clear();
        self.sent.clear();
        self.aimed.clear();
        self.posts.clear();
        self.held.clear();
    }

    fn release(&mut self, ids: &[u32]) {
        self.tanks.retain(|id| !ids.contains(id));
        self.riflemen.retain(|id| !ids.contains(id));
        self.scout = self.scout.filter(|id| !ids.contains(id));
    }

    /// While the party has the Scout Car, or wants one the push does not hold, Jeff trains one
    /// more than its usual count, first.
    pub(super) fn queue_scout(
        &self,
        priorities: &mut Vec<EntityKind>,
        max_counts: &mut Vec<(EntityKind, usize)>,
    ) {
        if self.scout.is_none() && !self.wants_scout {
            return;
        }
        match max_counts
            .iter_mut()
            .find(|(kind, _)| *kind == EntityKind::ScoutCar)
        {
            Some((_, count)) => *count += 1,
            None => max_counts.push((EntityKind::ScoutCar, 2)),
        }
        priorities.retain(|kind| *kind != EntityKind::ScoutCar);
        priorities.insert(0, EntityKind::ScoutCar);
    }
}

/// Runs the live Jeff's third-base pressure.
pub(super) fn plan_live(
    profile_id: &str,
    memory: &mut AiDecisionMemory,
    (actions, observation, map_analysis, intents): LiveStep<'_, '_, '_, '_>,
) {
    if profile_id != JEFFS_AI_ID {
        return;
    }
    note_enemy_depots(observation, &mut memory.third_pressure);
    let alive: BTreeSet<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.hp > 0)
        .map(|unit| unit.id)
        .collect();
    let state = &mut memory.third_pressure;
    state.tanks.retain(|id| alive.contains(id));
    state.riflemen.retain(|id| alive.contains(id));
    state.scout = state.scout.filter(|id| alive.contains(id));
    match memory.third_pressure.phase {
        Phase::Idle => match launch(observation, map_analysis, memory) {
            LaunchCheck::Go(launch) => {
                let state = &mut memory.third_pressure;
                state.wants_scout = false;
                begin_scout(
                    observation,
                    state,
                    launch.scout,
                    launch.target,
                    launch.route,
                );
                scout_trip(actions, observation, map_analysis, memory, intents);
            }
            LaunchCheck::NeedScout => memory.third_pressure.wants_scout = true,
            LaunchCheck::No => memory.third_pressure.wants_scout = false,
        },
        Phase::Scout => scout_trip(actions, observation, map_analysis, memory, intents),
        Phase::March => march(actions, observation, map_analysis, memory, intents),
        Phase::Locate => locate(actions, observation, map_analysis, memory, intents),
        Phase::Siege => siege(actions, observation, map_analysis, memory, intents),
        Phase::Merge => merge(actions, observation, map_analysis, memory, intents),
        Phase::Return => return_home(actions, observation, memory, intents),
    }
    let state = &mut memory.third_pressure;
    if state.phase != Phase::Idle {
        let note = format!(
            "{:?} tanks={:?} riflemen={:?} scout={:?} target={:?} depot={:?} leg={}/{} killed={:?} unseen={}",
            state.phase,
            state.tanks,
            state.riflemen,
            state.scout,
            state.target,
            state.depot,
            state.leg,
            state.route.len(),
            state.killed_at,
            state.unseen.len()
        );
        state.notes.push(note);
    }
}

struct Launch {
    scout: u32,
    target: EnemyBase,
    route: Vec<(f32, f32)>,
}

enum LaunchCheck {
    Go(Launch),
    /// Everything but a free Scout Car.
    NeedScout,
    No,
}

/// Whether the Scout Car goes to check a base. The Tanks wait until it has found the Depot.
fn launch(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &AiDecisionMemory,
) -> LaunchCheck {
    match launch_ready(observation, map_analysis, memory) {
        Some(Ok(launch)) => LaunchCheck::Go(launch),
        Some(Err(())) => LaunchCheck::NeedScout,
        None => LaunchCheck::No,
    }
}

fn launch_ready(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &AiDecisionMemory,
) -> Option<Result<Launch, ()>> {
    let tick = observation.tick;
    let state = &memory.third_pressure;
    if tick < LAUNCH_FROM_TICK || tick < state.next_launch {
        return None;
    }
    let bases = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .count();
    if bases >= 3 || memory.later_bases.planning() {
        return None;
    }
    if memory.home_outgunned || defense::local_defense_contact(observation).is_some() {
        return None;
    }
    let target = target_site(observation, state)?;
    // Bases keep turning up empty (AI 2.1 takes no third on some maps): from then on only a Depot
    // actually seen sends the party out.
    if state.empty_bases >= MAX_EMPTY_BASES && target.depot.is_none() {
        return None;
    }
    let home = home_point(observation, memory);
    let Some(scout) = party_scout(observation, memory) else {
        return Some(Err(()));
    };
    let route = flank_route(
        observation,
        map_analysis,
        home,
        target.depot.unwrap_or(target.center),
    );
    Some(Ok(Launch {
        scout,
        target,
        route,
    }))
}

/// Two Tanks from the army: never one pushing, guarding a base, clearing the natural or holding
/// home, and only when the main keeps its whole reserve without them. The natural's forward Tank
/// is restaged every decision, and the live adapter drops any order that shares a unit with a
/// stage, so it stays out too.
fn party_tanks(observation: &AiObservation, memory: &AiDecisionMemory) -> Option<Vec<u32>> {
    let at_main: BTreeSet<u32> = later_bases::main_tank_ids(observation, memory)
        .into_iter()
        .collect();
    let forward = expansion_security::surplus_tank_for_forward_base(observation, memory);
    let mut army: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !memory.containment.active_tanks.contains(&unit.id)
                && !memory.containment.opening_tanks.contains(&unit.id)
                && !memory.trap_breach.is_reserved(unit.id)
                && !memory.later_bases.guards.contains(&unit.id)
                && !memory.expansion_security.clearers.contains(&unit.id)
                && Some(unit.id) != memory.home_defensive_tank
                && Some(unit.id) != forward
        })
        .collect();
    // Tanks away from the main first, then the healthiest.
    army.sort_by(|left, right| {
        at_main
            .contains(&left.id)
            .cmp(&at_main.contains(&right.id))
            .then(right.hp.cmp(&left.hp))
            .then(left.id.cmp(&right.id))
    });
    let party: Vec<u32> = army.iter().take(PARTY_TANKS).map(|unit| unit.id).collect();
    if party.len() < PARTY_TANKS {
        return None;
    }
    let main_left = at_main.iter().filter(|id| !party.contains(id)).count();
    (main_left >= memory.home_tank_reserve()).then_some(party)
}

/// Five healthy Riflemen nobody else holds, nearest home first.
fn party_riflemen(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    home: (f32, f32),
) -> Option<Vec<u32>> {
    let held: BTreeSet<u32> = memory
        .raid_defense
        .reserved()
        .chain(memory.route_line.reserved())
        .chain(memory.opening_rush.reserved())
        .chain(memory.trap_breach.reserved())
        .collect();
    let mut free: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !held.contains(&unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.containment.active_riflemen.contains(&unit.id)
        })
        .collect();
    free.sort_by(|left, right| {
        right
            .hp
            .cmp(&left.hp)
            .then(distance((left.x, left.y), home).total_cmp(&distance((right.x, right.y), home)))
            .then(left.id.cmp(&right.id))
    });
    let party: Vec<u32> = free
        .iter()
        .take(PARTY_RIFLEMEN)
        .map(|unit| unit.id)
        .collect();
    (party.len() == PARTY_RIFLEMEN).then_some(party)
}

/// A sound Scout Car the push does not hold: taking the one a forming push counted on kept it
/// home, and a damaged one turned back as soon as it left.
fn party_scout(observation: &AiObservation, memory: &AiDecisionMemory) -> Option<u32> {
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ScoutCar && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            let max = config::unit_stats(unit.kind).map_or(unit.hp, |stats| stats.hp);
            unit.hp.saturating_mul(100) >= max.saturating_mul(SCOUT_LAUNCH_HP_PERCENT)
        })
        .filter(|unit| {
            memory.containment.active_scout != Some(unit.id)
                && !memory.trap_breach.is_reserved(unit.id)
        })
        .map(|unit| unit.id)
        .min()
}

/// The enemy base to press: an enemy base besides its natural (the base nearest its main), on the
/// enemy's side of the map, at least `MIN_TARGET_FROM_ENEMY_MAIN_TILES` from the enemy main, not
/// found empty (unless a Depot has been seen there since) and not found defended lately. A base
/// where a Depot has been seen comes first, then the best-ranked guess.
fn target_site(observation: &AiObservation, state: &ThirdPressure) -> Option<EnemyBase> {
    let ts = tile(observation);
    let tick = observation.tick;
    let enemy = enemy_start(observation)?;
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let bases = enemy_bases(observation, state);
    let natural = bases
        .iter()
        .map(|base| base.center)
        .min_by(|left, right| distance(*left, enemy).total_cmp(&distance(*right, enemy)));
    let same = |site: (i32, i32), base: &EnemyBase| {
        distance(unrounded(site), base.center) < SITE_SAME_TILES * ts
    };
    let mut candidates: Vec<EnemyBase> = bases
        .into_iter()
        .filter(|base| Some(base.center) != natural)
        .filter(|base| distance(base.center, enemy) < distance(base.center, own))
        .filter(|base| distance(base.center, enemy) >= MIN_TARGET_FROM_ENEMY_MAIN_TILES * ts)
        .filter(|base| base.depot.is_some() || !state.checked.iter().any(|site| same(*site, base)))
        .filter(|base| {
            !state.defended.iter().any(|(site, since)| {
                tick.saturating_sub(*since) < DEFENDED_SKIP_TICKS && same(*site, base)
            })
        })
        .collect();
    let first = candidates
        .iter()
        .position(|base| base.depot.is_some())
        .unwrap_or(0);
    (first < candidates.len()).then(|| candidates.swap_remove(first))
}

/// The enemy's bases beyond its main, in the order AI 2.1 takes them: it ranks sites by how much
/// nearer they are to itself than to its enemy, so its natural comes first and its third next.
/// Each is a cluster of Steel patches away from both starts, with the Oil patches beside them, and
/// the Depot seen within mining reach of them, if any.
fn enemy_bases(observation: &AiObservation, state: &ThirdPressure) -> Vec<EnemyBase> {
    let ts = tile(observation);
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let Some(enemy) = enemy_start(observation) else {
        return Vec::new();
    };
    let start_radius = (config::START_RESOURCE_MAX_DIST_TILES + 1.5) * ts;
    let starts: Vec<(f32, f32)> = observation
        .players
        .iter()
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))
        .collect();
    let away_from_starts = |resource: &&AiResourceSummary| {
        resource.remaining > 0
            && starts
                .iter()
                .all(|start| distance((resource.x, resource.y), *start) > start_radius)
    };
    let mut clusters: Vec<Vec<(f32, f32)>> = Vec::new();
    for resource in observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel)
        .filter(away_from_starts)
    {
        let point = (resource.x, resource.y);
        let (joined, rest): (Vec<_>, Vec<_>) = clusters.into_iter().partition(|cluster| {
            cluster
                .iter()
                .any(|other| distance(*other, point) <= BASE_MERGE_TILES * ts)
        });
        let mut merged: Vec<(f32, f32)> = joined.into_iter().flatten().collect();
        merged.push(point);
        clusters = rest;
        clusters.push(merged);
    }
    let mut bases: Vec<EnemyBase> = clusters
        .into_iter()
        .map(|patches| {
            let n = patches.len() as f32;
            let center = patches
                .iter()
                .fold((0.0, 0.0), |sum, p| (sum.0 + p.0 / n, sum.1 + p.1 / n));
            EnemyBase {
                center,
                patches,
                depot: None,
            }
        })
        .collect();
    for oil in observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Oil)
        .filter(away_from_starts)
    {
        let point = (oil.x, oil.y);
        if let Some(base) = bases.iter_mut().find(|base| {
            base.patches
                .iter()
                .any(|patch| distance(*patch, point) <= BASE_MERGE_TILES * ts)
        }) {
            base.patches.push(point);
        }
    }
    for depot in &state.seen_depots {
        let depot = unrounded(*depot);
        if let Some(base) = bases
            .iter_mut()
            .find(|base| mines_from(observation, depot, &base.patches))
        {
            base.depot = Some(depot);
        }
    }
    let ratio = |point: (f32, f32)| distance(point, enemy) / distance(point, own).max(1.0);
    bases.sort_by(|left, right| ratio(left.center).total_cmp(&ratio(right.center)));
    bases
}

/// Whether a Depot at `at` mines one of `patches`: AI 2.1's Depots anchor every patch within
/// `MINING_ANCHOR_RANGE_TILES` of their centre.
fn mines_from(observation: &AiObservation, at: (f32, f32), patches: &[(f32, f32)]) -> bool {
    let reach = (config::MINING_ANCHOR_RANGE_TILES + 0.5) * tile(observation);
    patches.iter().any(|patch| distance(*patch, at) <= reach)
}

// Waypoints from home to a staging point beside the target: out via a point off the line between
/// the two mains, on the target's side, and into the target from away from the enemy main.
fn flank_route(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    home: (f32, f32),
    target: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = tile(observation);
    let enemy = enemy_start(observation).unwrap_or(target);
    let axis = normalized_direction(home, enemy).unwrap_or((1.0, 0.0));
    let to_target = (target.0 - home.0, target.1 - home.1);
    let side = if axis.0 * to_target.1 - axis.1 * to_target.0 >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let perpendicular = (-axis.1 * side, axis.0 * side);
    let flank = clamp_to_map(
        (
            (home.0 + target.0) * 0.5 + perpendicular.0 * FLANK_OFFSET_TILES * ts,
            (home.1 + target.1) * 0.5 + perpendicular.1 * FLANK_OFFSET_TILES * ts,
        ),
        observation.map,
    );
    let away = normalized_direction(enemy, target).unwrap_or((0.0, 0.0));
    let back = normalized_direction(target, flank).unwrap_or(away);
    let approach =
        normalized_direction((0.0, 0.0), (away.0 + back.0, away.1 + back.1)).unwrap_or(back);
    let staging = clamp_to_map(
        (
            target.0 + approach.0 * STAGING_TILES * ts,
            target.1 + approach.1 * STAGING_TILES * ts,
        ),
        observation.map,
    );
    let Some(analysis) = map_analysis else {
        return vec![flank, staging];
    };
    let mut route = analysis.compact_group_route(home, flank, ROUTE_STRIDE_TILES);
    route.extend(analysis.compact_group_route(flank, staging, ROUTE_STRIDE_TILES));
    route
}

fn march(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let tick = observation.tick;
    let state = &memory.third_pressure;
    if state.tanks.is_empty()
        || memory.home_outgunned
        || tick.saturating_sub(state.phase_since) > MARCH_TIMEOUT_TICKS
    {
        begin_return(observation, memory, intents, "march over");
        return;
    }
    if memory.third_pressure.watch.is_some() {
        keep_watch(actions, observation, map_analysis, memory, intents);
    }
    if !answer_threat(actions, observation, memory, intents, None) {
        return;
    }
    // A Depot seen on the way, near the party, beats a guess at the target.
    if let Some(base) = field_target(observation, &memory.third_pressure, true) {
        retarget(observation, map_analysis, &mut memory.third_pressure, base);
    }
    let state = &mut memory.third_pressure;
    let Some(point) = state.route.get(state.leg).map(|p| unrounded(*p)) else {
        begin_locate(observation, map_analysis, memory, actions, intents);
        return;
    };
    let members = units(observation, &state.fighting_members());
    let ts = tile(observation);
    let reached = members
        .iter()
        .all(|unit| distance((unit.x, unit.y), point) <= LEG_REACHED_TILES * ts);
    if reached || tick.saturating_sub(state.leg_since) > LEG_TIMEOUT_TICKS {
        state.leg += 1;
        state.leg_since = tick;
        if state.leg >= state.route.len() {
            begin_locate(observation, map_analysis, memory, actions, intents);
            return;
        }
    }
    let point = unrounded(memory.third_pressure.route[memory.third_pressure.leg]);
    let members = memory.third_pressure.fighting_members();
    send(
        actions,
        observation,
        &mut memory.third_pressure,
        &members,
        point,
        intents,
    );
}

/// Weighs enemy fighters near the party. One it loses, or one against at least as many Tanks,
/// sends it home, unless the siege is `finishing` the Depot with no enemy Tank close. On the march
/// it takes a fight it wins, attack-moving at the enemy; at the siege its parked Tanks and holding
/// spotters fight from their posts. Returns whether the party carries on with its plan.
fn answer_threat(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    finishing: Option<bool>,
) -> bool {
    let ts = tile(observation);
    let tick = observation.tick;
    let members = units(observation, &memory.third_pressure.fighting_members());
    let near: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| {
            members
                .iter()
                .any(|unit| distance((unit.x, unit.y), (enemy.x, enemy.y)) <= THREAT_TILES * ts)
        })
        .collect();
    if near.is_empty() {
        memory.third_pressure.avoid_since = None;
        return true;
    }
    let meth = observation
        .upgrades
        .contains(&UpgradeKind::Methamphetamines);
    let ours: Vec<_> = members
        .iter()
        .filter_map(|unit| fighter(unit, meth, false))
        .collect();
    let theirs: Vec<_> = near
        .iter()
        .filter_map(|enemy| fighter(enemy, false, enemy.state != AiEntityState::Move))
        .collect();
    let their_tanks = near
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank)
        .count();
    let estimate = estimate_trade(&ours, &theirs, 0);
    if estimate.clearly_losing()
        || (their_tanks > 0 && their_tanks >= memory.third_pressure.tanks.len())
    {
        let tanks_close = near.iter().any(|enemy| {
            enemy.kind == EntityKind::Tank
                && members.iter().any(|unit| {
                    unit.kind == EntityKind::Tank
                        && distance((unit.x, unit.y), (enemy.x, enemy.y))
                            <= FINISH_CLEAR_OF_TANKS_TILES * ts
                })
        });
        if finishing == Some(true) && !tanks_close {
            return true;
        }
        begin_return(observation, memory, intents, "outmatched");
        return false;
    }
    if finishing.is_some() {
        return true;
    }
    // Enemies passing at a distance are let pass: the party holds (its Tanks still fire at what
    // comes into reach) and fights only once they close in, or when they stay in the way.
    let engaged = near.iter().any(|enemy| {
        members
            .iter()
            .any(|unit| distance((unit.x, unit.y), (enemy.x, enemy.y)) <= ENGAGED_TILES * ts)
    });
    let state = &mut memory.third_pressure;
    let since = *state.avoid_since.get_or_insert(tick);
    if !engaged && tick.saturating_sub(since) < AVOID_MAX_TICKS {
        let hold: Vec<u32> = members
            .iter()
            .map(|unit| unit.id)
            .filter(|id| state.held.insert(*id))
            .collect();
        actions::hold_position_units(actions, hold);
        return false;
    }
    let n = near.len() as f32;
    let center = near.iter().fold((0.0, 0.0), |sum, enemy| {
        (sum.0 + enemy.x / n, sum.1 + enemy.y / n)
    });
    let state = &mut memory.third_pressure;
    let aim = rounded(center);
    let fight: Vec<u32> = members
        .iter()
        .filter(|unit| {
            unit.state == AiEntityState::Idle
                || state
                    .aimed
                    .get(&unit.id)
                    .is_none_or(|at| distance(unrounded(*at), center) > REAIM_TILES * ts)
        })
        .map(|unit| unit.id)
        .collect();
    if let Some(units) = actions::attack_move_units(actions, fight, center.0, center.1) {
        for id in &units {
            state.sent.remove(id);
            state.held.remove(id);
            state.aimed.insert(*id, aim);
        }
        intents.push(AiIntent::Attack { units });
    }
    false
}

fn begin_siege(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    actions: &mut AiActionContext<'_>,
    intents: &mut Vec<AiIntent>,
) {
    let state = &mut memory.third_pressure;
    state.phase = Phase::Siege;
    state.phase_since = observation.tick;
    state.clear_orders();
    siege(actions, observation, map_analysis, memory, intents);
}

fn siege(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let tick = observation.tick;
    let ts = tile(observation);
    let state = &memory.third_pressure;
    if state.tanks.is_empty() || memory.home_outgunned {
        begin_return(observation, memory, intents, "siege over");
        return;
    }
    if tick.saturating_sub(state.phase_since) > SIEGE_TIMEOUT_TICKS {
        begin_merge(observation, memory, intents, "siege out of time");
        return;
    }
    let Some(site) = state.target.map(unrounded) else {
        begin_merge(observation, memory, intents, "no target");
        return;
    };
    // The Depot the party found, or one standing where a remembered Depot was.
    let depot = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::ResourceDepot && enemy.hp > 0)
        .find(|enemy| match state.depot {
            Some(id) => enemy.id == id,
            None => distance((enemy.x, enemy.y), site) <= DEPOT_SAME_TILES * ts,
        });
    // The Tanks parked long enough to reach the Depot finish it this volley.
    let finishing = depot.is_some_and(|depot| {
        let ready = units(observation, &state.tanks)
            .into_iter()
            .filter(|tank| {
                state
                    .parked_since
                    .get(&tank.id)
                    .is_some_and(|since| tick.saturating_sub(*since) >= STATIONARY_READY_TICKS)
                    && in_reach(observation, tank, depot)
            })
            .count() as u32;
        depot.hp <= TANK_SHELL_DAMAGE * ready
    });
    if !answer_threat(actions, observation, memory, intents, Some(finishing)) {
        return;
    }
    let state = &mut memory.third_pressure;
    let members = units(observation, &state.members());
    let watched = members
        .iter()
        .any(|unit| distance((unit.x, unit.y), site) <= (sight(unit) - SIGHT_MARGIN_TILES) * ts);
    match depot {
        Some(depot) => {
            state.depot = Some(depot.id);
            state.target = Some(rounded((depot.x, depot.y)));
        }
        None if watched && state.depot.is_some() => {
            state.killed_at.get_or_insert(tick);
        }
        None if watched => {
            // A remembered Depot is no longer there: look for the base's Depot again.
            state
                .seen_depots
                .retain(|seen| distance(unrounded(*seen), site) >= DEPOT_SAME_TILES * ts);
            begin_locate(observation, map_analysis, memory, actions, intents);
            return;
        }
        None => {}
    }
    let site = state.target.map(unrounded).unwrap_or(site);
    let Some(posts) = assign_posts(observation, map_analysis, state, site) else {
        begin_merge(observation, memory, intents, "no Tank post");
        return;
    };
    let mut fire_targets_left = false;
    for (id, post) in posts {
        let Some(unit) = observation.owned.iter().find(|unit| unit.id == id) else {
            continue;
        };
        if distance((unit.x, unit.y), post) > POST_REACHED_TILES * ts {
            state.parked_since.remove(&id);
            send(actions, observation, state, &[id], post, intents);
            continue;
        }
        // A fighter in reach is the holding Tank's to answer; explicit orders would chase it.
        let threatened = observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && enemy.kind != EntityKind::Worker
                && in_reach(observation, unit, enemy)
        });
        let fire = (unit.kind == EntityKind::Tank && !threatened)
            .then(|| fire_target(observation, unit, site))
            .flatten();
        fire_targets_left |= fire.is_some();
        let parked = *state.parked_since.entry(id).or_insert(tick);
        match fire {
            Some(target) if tick.saturating_sub(parked) >= STATIONARY_READY_TICKS => {
                if unit.target_id != Some(target) {
                    if let Some(units) = actions::attack_units(actions, [id], target) {
                        state.held.remove(&id);
                        intents.push(AiIntent::Attack { units });
                    }
                }
            }
            // On its post a member holds, firing at what comes into reach without chasing it
            // into the base's defense zone.
            _ => {
                if state.held.insert(id) {
                    actions::hold_position_units(actions, [id]);
                }
            }
        }
    }
    if state
        .killed_at
        .is_some_and(|killed| tick.saturating_sub(killed) >= HOLD_AFTER_KILL_TICKS)
        && !fire_targets_left
    {
        begin_merge(observation, memory, intents, "base cleared");
    }
}

/// Each member's post around `site`: the Tanks', then the Scout Car's, then the Riflemen's. A post
/// still clear is kept, so a parked Tank keeps its reach; a spotter without one stands behind the
/// first Tank. `None` when a Tank has no post.
fn assign_posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    state: &mut ThirdPressure,
    site: (f32, f32),
) -> Option<Vec<(u32, (f32, f32))>> {
    let ts = tile(observation);
    // Tanks keep clear of every patch at the base, which may hold an extractor not yet seen;
    // spotters walk up to see it, clear of the buildings they do see.
    let tank_fixed = keep_out(observation, site, true);
    let spotter_fixed = keep_out(observation, site, false);
    let workers = worker_keep_out(observation);
    let facing = facing(observation, state, site);
    let roles: Vec<(u32, &[f32], &[KeepOut])> = state
        .tanks
        .iter()
        .map(|id| (*id, &TANK_POST_TILES[..], &tank_fixed[..]))
        .chain(
            state
                .scout
                .map(|id| (id, &SCOUT_POST_TILES[..], &spotter_fixed[..])),
        )
        .chain(
            state
                .riflemen
                .iter()
                .map(|id| (*id, &SPOTTER_POST_TILES[..], &spotter_fixed[..])),
        )
        .collect();
    let mut posts: Vec<(u32, (f32, f32))> = Vec::new();
    for (id, radii, fixed) in &roles {
        let Some(post) = state.posts.get(id).map(|post| unrounded(*post)) else {
            continue;
        };
        let taken: Vec<(f32, f32)> = posts.iter().map(|(_, post)| *post).collect();
        if post_still_clear(post, site, radii, fixed, &taken, ts) {
            posts.push((*id, post));
        }
    }
    for (id, radii, fixed) in &roles {
        if posts.iter().any(|(other, _)| other == id) {
            continue;
        }
        let taken: Vec<(f32, f32)> = posts.iter().map(|(_, post)| *post).collect();
        let mut avoid = fixed.to_vec();
        avoid.extend(workers.iter().copied());
        let post = post_near(
            observation,
            map_analysis,
            site,
            radii,
            facing,
            &avoid,
            &taken,
        );
        if let Some(post) = post {
            posts.push((*id, post));
        }
    }
    let tank_posts: Vec<(f32, f32)> = posts
        .iter()
        .filter(|(id, _)| state.tanks.contains(id))
        .map(|(_, post)| *post)
        .collect();
    if tank_posts.len() < state.tanks.len() {
        return None;
    }
    if let Some(first) = tank_posts.first() {
        let out = normalized_direction(site, *first).unwrap_or(facing);
        let behind = clamp_to_map(
            (first.0 + out.0 * 2.0 * ts, first.1 + out.1 * 2.0 * ts),
            observation.map,
        );
        for (id, _, _) in &roles {
            if !posts.iter().any(|(other, _)| other == id) {
                posts.push((*id, behind));
            }
        }
    }
    state.posts = posts
        .iter()
        .map(|(id, post)| (*id, rounded(*post)))
        .collect();
    Some(posts)
}

/// The side of the base the party works from: away from the enemy main and toward the staging
/// point it came in by.
fn facing(observation: &AiObservation, state: &ThirdPressure, site: (f32, f32)) -> (f32, f32) {
    let enemy = enemy_start(observation).unwrap_or(site);
    let approach = state
        .route
        .last()
        .map(|point| unrounded(*point))
        .unwrap_or(site);
    let away = normalized_direction(enemy, site).unwrap_or((0.0, 0.0));
    let back = normalized_direction(site, approach).unwrap_or(away);
    normalized_direction((0.0, 0.0), (away.0 + back.0, away.1 + back.1)).unwrap_or((0.0, 1.0))
}

/// What a parked Tank shoots within reach: a worker first (the Depot's builder), then the Depot,
/// then anything else standing at the base.
fn fire_target(
    observation: &AiObservation,
    tank: &AiEntitySummary,
    site: (f32, f32),
) -> Option<u32> {
    let ts = tile(observation);
    let rank = |enemy: &AiEntitySummary| match enemy.kind {
        EntityKind::Worker => 0,
        EntityKind::ResourceDepot => 1,
        _ => 2,
    };
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0)
        .filter(|enemy| enemy.kind == EntityKind::Worker || enemy.kind.is_building())
        .filter(|enemy| distance((enemy.x, enemy.y), site) <= BASE_RESOURCE_TILES * ts)
        .filter(|enemy| in_reach(observation, tank, enemy))
        .min_by_key(|enemy| (rank(enemy), enemy.hp, enemy.id))
        .map(|enemy| enemy.id)
}

/// Whether a parked Tank reaches `target`: a unit within `TANK_FIRE_TILES` of it, a building whose
/// edge is (half its narrower side nearer than its centre).
fn in_reach(observation: &AiObservation, tank: &AiEntitySummary, target: &AiEntitySummary) -> bool {
    let ts = tile(observation);
    let half = if target.kind.is_building() {
        config::building_stats(target.kind)
            .map_or(0.0, |stats| stats.foot_w.min(stats.foot_h) as f32 * 0.5)
            .min(DEPOT_HALF_TILES)
    } else {
        0.0
    };
    distance((tank.x, tank.y), (target.x, target.y)) <= (TANK_FIRE_TILES + half) * ts
}

/// Points the party keeps clear of, with their radius: every enemy building, the target itself and,
/// with `patches`, the resource patches at the target that may hold an extractor.
fn keep_out(
    observation: &AiObservation,
    site: (f32, f32),
    patches: bool,
) -> Vec<((f32, f32), f32)> {
    let ts = tile(observation);
    let zone = ZONE_CLEARANCE_TILES * ts;
    let mut points: Vec<((f32, f32), f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_building())
        .map(|enemy| ((enemy.x, enemy.y), zone))
        .collect();
    if patches {
        points.extend(
            observation
                .resources
                .iter()
                .filter(|resource| {
                    distance((resource.x, resource.y), site) <= BASE_RESOURCE_TILES * ts
                })
                .map(|resource| ((resource.x, resource.y), zone)),
        );
    }
    points.push((site, zone + 1.5 * ts));
    points
}

/// Every enemy worker, which a new post keeps clear of; a post already held stays.
fn worker_keep_out(observation: &AiObservation) -> Vec<((f32, f32), f32)> {
    let ts = tile(observation);
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind == EntityKind::Worker)
        .map(|enemy| ((enemy.x, enemy.y), WORKER_CLEARANCE_TILES * ts))
        .collect()
}

fn post_still_clear(
    post: (f32, f32),
    site: (f32, f32),
    radii: &[f32],
    keep_out: &[((f32, f32), f32)],
    taken: &[(f32, f32)],
    ts: f32,
) -> bool {
    let low = radii.iter().copied().fold(f32::MAX, f32::min);
    let high = radii.iter().copied().fold(f32::MIN, f32::max);
    let from_site = distance(post, site) / ts;
    (low - 0.75..=high).contains(&from_site)
        && keep_out.iter().all(|(at, r)| distance(post, *at) >= *r)
        && taken
            .iter()
            .all(|other| distance(post, *other) >= POST_SEPARATION_TILES * ts)
}

/// A post `radii` tiles from `site` (tried in order) on passable ground, clear of `keep_out` and of
/// the posts already `taken`, within `POST_CONE_DEGREES` of `facing` and nearest it.
fn post_near(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    site: (f32, f32),
    radii: &[f32],
    facing: (f32, f32),
    keep_out: &[((f32, f32), f32)],
    taken: &[(f32, f32)],
) -> Option<(f32, f32)> {
    let ts = tile(observation);
    let wanted_angle = facing.1.atan2(facing.0);
    for radius in radii {
        let best = (0..48)
            .map(|step| step as f32 * std::f32::consts::TAU / 48.0)
            .map(|angle| {
                let point = clamp_to_map(
                    (
                        site.0 + angle.cos() * radius * ts,
                        site.1 + angle.sin() * radius * ts,
                    ),
                    observation.map,
                );
                let off = (angle - wanted_angle + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                (point, off.abs())
            })
            .filter(|(_, off)| *off <= POST_CONE_DEGREES.to_radians())
            .filter(|(point, _)| (distance(*point, site) - radius * ts).abs() < 0.5 * ts)
            .filter(|(point, _)| keep_out.iter().all(|(at, r)| distance(*point, *at) >= *r))
            .filter(|(point, _)| {
                taken
                    .iter()
                    .all(|other| distance(*point, *other) >= POST_SEPARATION_TILES * ts)
            })
            .filter(|(point, _)| {
                map_analysis.is_none_or(|analysis| {
                    analysis.tile_is_passable((point.0 / ts) as u32, (point.1 / ts) as u32)
                })
            })
            .min_by(|left, right| left.1.total_cmp(&right.1));
        if let Some((point, _)) = best {
            return Some(point);
        }
    }
    None
}

/// Turns the party home the way it came. Still near home, it disbands on the spot.
fn begin_return(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    why: &str,
) {
    let ts = tile(observation);
    let home = home_point(observation, memory);
    let state = &mut memory.third_pressure;
    state.notes.push(format!("return: {why}"));
    let members = units(observation, &state.members());
    if centroid(&members).is_none_or(|at| distance(at, home) <= DISBAND_NEAR_HOME_TILES * ts) {
        disband(observation.tick, state, intents);
        return;
    }
    state.phase = Phase::Return;
    state.phase_since = observation.tick;
    state.route.reverse();
    let leg = state
        .route
        .len()
        .saturating_sub(state.leg.min(state.route.len()));
    state.leg = leg.min(state.route.len());
    state.leg_since = observation.tick;
    state.clear_orders();
}

/// Hands every member back to the rest of Jeff.
fn disband(tick: u32, state: &mut ThirdPressure, intents: &mut Vec<AiIntent>) {
    let released = state.members();
    state.release(&released);
    state.route.clear();
    state.clear_orders();
    state.phase = Phase::Idle;
    state.next_launch = tick.saturating_add(RELAUNCH_COOLDOWN_TICKS);
    if !released.is_empty() {
        // Clears the live adapter's cached staging so home places them again.
        intents.push(AiIntent::Assemble { units: released });
    }
}

/// Walks the route back; each member is handed back on reaching home.
fn return_home(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
) {
    let tick = observation.tick;
    let ts = tile(observation);
    let home = home_point(observation, memory);
    let alone = memory.third_pressure.tanks.is_empty() && memory.third_pressure.riflemen.is_empty();
    if alone
        && memory.third_pressure.scout.is_some()
        && !scout_clear(actions, observation, None, memory, intents, false)
    {
        return;
    }
    let state = &mut memory.third_pressure;
    let arrived: Vec<u32> = units(observation, &state.members())
        .iter()
        .filter(|unit| distance((unit.x, unit.y), home) <= HOME_ARRIVED_TILES * ts)
        .map(|unit| unit.id)
        .collect();
    if !arrived.is_empty() {
        state.release(&arrived);
        intents.push(AiIntent::Assemble { units: arrived });
    }
    if state.members().is_empty() || tick.saturating_sub(state.phase_since) > RETURN_TIMEOUT_TICKS {
        disband(tick, state, intents);
        return;
    }
    let members = units(observation, &state.members());
    let mut point = home;
    while let Some(waypoint) = state.route.get(state.leg).map(|p| unrounded(*p)) {
        let reached = members
            .iter()
            .all(|unit| distance((unit.x, unit.y), waypoint) <= LEG_REACHED_TILES * ts);
        if reached || tick.saturating_sub(state.leg_since) > LEG_TIMEOUT_TICKS {
            state.leg += 1;
            state.leg_since = tick;
            continue;
        }
        point = waypoint;
        break;
    }
    let ids = state.members();
    send(actions, observation, state, &ids, point, intents);
}

/// Moves `ids` to `point`, ordering each only when it was sent elsewhere or has stopped short.
fn send(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    state: &mut ThirdPressure,
    ids: &[u32],
    point: (f32, f32),
    intents: &mut Vec<AiIntent>,
) {
    let ts = tile(observation);
    let goal = rounded(point);
    let go: Vec<u32> = ids
        .iter()
        .copied()
        .filter(|id| {
            let Some(unit) = observation.owned.iter().find(|unit| unit.id == *id) else {
                return false;
            };
            state.sent.get(id) != Some(&goal)
                || (unit.state == AiEntityState::Idle
                    && distance((unit.x, unit.y), point) > LEG_REACHED_TILES * ts)
        })
        .collect();
    if let Some(units) = actions::move_units(actions, go, point.0, point.1) {
        for id in &units {
            state.sent.insert(*id, goal);
            state.aimed.remove(id);
            state.held.remove(id);
        }
        intents.push(AiIntent::Move { units });
    }
}

/// Remembers enemy Depots seen away from the enemy main.
fn note_enemy_depots(observation: &AiObservation, state: &mut ThirdPressure) {
    let ts = tile(observation);
    let Some(enemy) = enemy_start(observation) else {
        return;
    };
    for depot in observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::ResourceDepot && enemy.hp > 0)
    {
        let at = (depot.x, depot.y);
        if distance(at, enemy) < 20.0 * ts {
            continue;
        }
        if !state
            .seen_depots
            .iter()
            .any(|seen| distance(unrounded(*seen), at) < 4.0 * ts)
        {
            state.seen_depots.push(rounded(at));
        }
    }
}

fn home_point(observation: &AiObservation, memory: &AiDecisionMemory) -> (f32, f32) {
    memory.home_post.map_or_else(
        || tile_center(observation.own_start_tile, observation.map.tile_size),
        |post| post.center(),
    )
}

/// The nearest living enemy's start, in world pixels.
fn enemy_start(observation: &AiObservation) -> Option<(f32, f32)> {
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))
        .min_by(|left, right| distance(*left, own).total_cmp(&distance(*right, own)))
}

fn units<'o>(observation: &'o AiObservation, ids: &[u32]) -> Vec<&'o AiEntitySummary> {
    observation
        .owned
        .iter()
        .filter(|unit| ids.contains(&unit.id) && unit.hp > 0)
        .collect()
}

fn centroid(units: &[&AiEntitySummary]) -> Option<(f32, f32)> {
    let n = units.len() as f32;
    (n > 0.0).then(|| {
        units.iter().fold((0.0, 0.0), |sum, unit| {
            (sum.0 + unit.x / n, sum.1 + unit.y / n)
        })
    })
}

fn tile(observation: &AiObservation) -> f32 {
    observation.map.tile_size.max(1) as f32
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    dist2(a.0, a.1, b.0, b.1).sqrt()
}

fn rounded(point: (f32, f32)) -> (i32, i32) {
    (point.0.round() as i32, point.1.round() as i32)
}

fn unrounded(point: (i32, i32)) -> (f32, f32) {
    (point.0 as f32, point.1 as f32)
}

#[cfg(test)]
#[path = "third_pressure_tests.rs"]
mod tests;
