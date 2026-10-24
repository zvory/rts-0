//! Jeff's opening rush.
//!
//! The four starting Riflemen march out in one group along the enemy's own attack route (the map
//! analysis base route, walked backwards), so they meet an enemy opening attack head on. Whatever
//! they meet is fought by the rifle squad planner (`squad_micro`): focus fire on the weakest enemy
//! in reach, no overkill, and wounded Riflemen under fire step back.
//!
//! The march does not end at the enemy base but beside its natural, predicted from the map's Steel
//! like the push's objective. There the squad digs in on the far side of the site and denies the
//! expansion: it shoots the builder first, then fighters, extractors and the Depot, and only at
//! targets already in reach so nobody leaves a trench.
//!
//! The group falls back home, stepping off the enemy's line of march first:
//! - On the way out, without a natural to deny: as soon as Entrenchment could be available to
//!   either player (Jeff's own research is known; the enemy's is private, so the earliest tick
//!   anyone could have finished it stands in for it), or an enemy Rifleman or Machine Gunner is seen
//!   moving faster than it can without Methamphetamines.
//! - Always: when the fight in sight is clearly lost (`estimate_trade` says it costs more than
//!   twice what it kills). At the natural the estimate counts the squad's trenches, any enemy
//!   Methamphetamines, and only the enemies close enough to be fighting it; once half the squad
//!   is dug in there it stays unless it would be wiped out without a single kill.
//!
//! - On the way out: when the squad reaches the middle of the map without having met any enemy
//!   fighter, or enemy fighters turn up near Jeff's main first. The enemy's opening attack then
//!   took another way and is heading for an empty base (on Wald des Todes the two groups walk
//!   parallel lanes twelve tiles apart), so the squad goes home, the Barracks trains two Riflemen
//!   at once, and the home pocket holds forest edges where it can. If no enemy fighter has shown
//!   up near the main by the time the squad is home, the enemy kept its Riflemen in its base
//!   instead, and the squad marches out again; then only a raid seen at home turns it back.
//!
//! Survivors that reach home are released to the home pocket. Only the live Jeff runs this; the
//! `jeffs_ai_pre_opening_rush` freeze keeps its starting Riflemen at home.

use super::frontal::enemy_natural_edge;
use super::geometry::dist2;
use super::*;
use crate::ai_core::map_analysis::AiTile;
use crate::ai_core::squad_micro::{
    plan_rifle_squad, RifleSquadParams, SquadMicroMemory, SquadOrder,
};

/// The middle of the map: the squad is as close to the enemy base as to its own. Against AI 2.1 the
/// two opening groups always come into sight before this (they meet at 46-49% of the route).
const MISSED_CONTACT_PROGRESS: f32 = 0.5;
/// Enemy fighters seen this close to Jeff's main before the squad met any have slipped past it.
const HOME_ALERT_TILES: f32 = 28.0;
/// Riflemen the Barracks trains straight away when the enemy's opening attack slips past.
const EMERGENCY_RIFLEMEN: usize = 2;
/// A relaunched squad first gathers at the first march waypoint at least this far from home: back
/// in the base it stands strung out between buildings, where the squad planner would keep
/// switching between regrouping and advancing.
const RELAUNCH_GATHER_TILES: f32 = 16.0;
const RELAUNCH_GATHER_TICKS: u32 = 600;
const GATHER_REACHED_TILES: f32 = 3.0;
/// Squad members still this far from the gathering point when time runs out stay home.
const GATHER_STRAGGLER_TILES: f32 = 8.0;
/// A rush only starts from the opening's first decisions.
const START_TICK_LIMIT: u32 = 90;
/// Distance between march waypoints along the base route.
const WAYPOINT_STRIDE_TILES: usize = 8;
const WAYPOINT_REACHED_TILES: f32 = 4.0;
/// Enemies this close to any squad member are the fight the squad is in.
const ENGAGE_RADIUS_TILES: f32 = 14.0;
/// No player can have Entrenchment sooner: Barracks, then a Training Centre (560 ticks), then 600
/// ticks of research, paid from the starting income. Jeff, which takes that path as fast as it
/// can, finishes at 1,930-2,040 in arena games.
const EARLIEST_ENTRENCHMENT_TICK: u32 = 1_900;
/// Moving faster than this multiple of a unit's unboosted speed, off roads, means
/// Methamphetamines (a 1.25x boost).
const METH_SPEED_MARGIN: f32 = 1.15;
/// Two sightings of the same enemy at most this far apart are compared for speed.
const SPEED_SAMPLE_MAX_TICKS: u32 = 12;
const SIGHTING_MEMORY_TICKS: u32 = 60;
/// A withdrawing squad first steps this far to the side of the enemy's line of march: the enemy
/// only shoots what is in range and keeps walking toward Jeff's base, so it walks past.
const SIDESTEP_TILES: f32 = 8.0;
const WITHDRAW_REORDER_TICKS: u32 = 60;
const HOME_ARRIVAL_TILES: f32 = 12.0;
/// Rifleman speed in pixels per tick, used for the approach under an entrenched enemy's longer
/// range.
const RIFLEMAN_SPEED_PX: f32 = 1.6;
/// The denial posts stand this far beyond the natural's Steel edge, away from the enemy base: the
/// enemy's Depot goes up beside the Steel, within the posts' reach, while its main stays farther
/// off.
const DENY_POST_FORWARD_TILES: f32 = 2.0;
/// Lateral offsets of the denial posts, filled in this order.
const DENY_POST_LATERAL_TILES: [f32; 4] = [0.75, -0.75, 2.25, -2.25];
/// The squad starts denying once its centre is this close to the posts.
const DENY_ENTER_TILES: f32 = 6.0;
const DENY_POST_TOLERANCE_TILES: f32 = 1.5;
/// A unit on its way to a post is re-ordered at most this often.
const DENY_REORDER_TICKS: u32 = 45;
/// Dug in, only enemies this close are the fight the squad is in; its main's garrison farther off
/// is not coming unless something brings it.
const DENY_THREAT_TILES: f32 = 9.0;
/// A building's centre lies about this far inside its nearest edge, which the reach is measured to.
const BUILDING_REACH_SLACK_TILES: f32 = 1.0;
/// Mirrors the simulation combat service's range slack.
const SIM_RANGE_SLACK_PX: f32 = 4.0;

/// The squad planner settings the rush fights with: the planner's `micro` preset with its
/// targeted-wounded pull-back on.
fn squad_params() -> RifleSquadParams {
    RifleSquadParams {
        retreat_hp: 15,
        retreat_tiles: 3.0,
        retreat_ticks: 45,
        ..RifleSquadParams::micro()
    }
}

pub(super) fn uses_opening_rush(profile_id: &str) -> bool {
    profile_id == JEFFS_AI_ID
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum RushPhase {
    #[default]
    NotStarted,
    March,
    /// Dug in beside the enemy's natural.
    Deny,
    Withdraw,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WithdrawReason {
    Entrenchment,
    Methamphetamines,
    BadTrade,
    /// The enemy's opening attack went past the squad toward Jeff's base.
    MissedContact,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct OpeningRush {
    phase: RushPhase,
    squad: BTreeSet<u32>,
    /// The starting Riflemen the rush took, alive or not.
    starters: BTreeSet<u32>,
    /// March waypoints in world pixels, home to the denial posts (or the enemy base).
    waypoints: Vec<(i32, i32)>,
    next_waypoint: usize,
    micro: SquadMicroMemory,
    withdraw_reason: Option<WithdrawReason>,
    withdraw_order_tick: Option<u32>,
    /// Last sighting of each enemy Rifleman and Machine Gunner: tick and position.
    sightings: BTreeMap<u32, (u32, i32, i32)>,
    enemy_meth_seen: bool,
    /// Centre of the denial posts beside the enemy's natural, and the direction away from the
    /// enemy base in thousandths, when a natural was predicted.
    deny_center: Option<(i32, i32)>,
    deny_away: Option<(i32, i32)>,
    deny_since: Option<u32>,
    /// Per denying Rifleman: its current target, whether it was told to hold, and when it was last
    /// sent toward its post.
    deny_targets: BTreeMap<u32, u32>,
    deny_holding: BTreeSet<u32>,
    deny_order_tick: BTreeMap<u32, u32>,
    /// An enemy fighter has come within the squad's fight radius since the march began.
    squad_contact: bool,
    /// The enemy's opening attack slipped past the squad. Sticky for the rest of the game.
    missed_contact: bool,
    /// Enemy fighters have been seen near Jeff's main since the contact was missed.
    home_raid_seen: bool,
    /// The squad came home after a missed contact, found no raid, and marched out again.
    relaunched: bool,
    /// Where a relaunched squad gathers before marching on, until when, and whether it was sent.
    gather_point: Option<(i32, i32)>,
    gather_until: u32,
    gather_ordered: bool,
    /// Riflemen to have, queued ones included, before the emergency recruiting stops.
    emergency_rifle_target: Option<usize>,
}

impl OpeningRush {
    fn active(&self) -> bool {
        matches!(
            self.phase,
            RushPhase::March | RushPhase::Deny | RushPhase::Withdraw
        )
    }

    /// Units the rush owns while it runs. No other system may order them.
    pub(in crate::ai_core::decision) fn reserved(&self) -> impl Iterator<Item = u32> + '_ {
        let active = self.active();
        self.squad.iter().copied().filter(move |_| active)
    }

    pub(in crate::ai_core::decision) fn is_reserved(&self, id: u32) -> bool {
        self.active() && self.squad.contains(&id)
    }

    /// Whether the rush took Jeff's starting Riflemen; they then no longer make up the home pocket
    /// the other systems count on.
    pub(in crate::ai_core::decision) fn started(&self) -> bool {
        !self.starters.is_empty()
    }

    pub(in crate::ai_core::decision) fn is_starter(&self, id: u32) -> bool {
        self.starters.contains(&id)
    }

    /// Whether the enemy's opening attack slipped past the squad toward Jeff's base.
    pub(in crate::ai_core::decision) fn missed_contact(&self) -> bool {
        self.missed_contact
    }

    /// While the enemy's opening attack is loose, Riflemen lead `priorities` until Jeff has the
    /// returned number, queued ones included.
    pub(in crate::ai_core::decision) fn lead_with_emergency_riflemen(
        &self,
        priorities: &mut Vec<EntityKind>,
    ) -> Option<usize> {
        let target = self.emergency_rifle_target?;
        priorities.retain(|unit| *unit != EntityKind::Rifleman);
        priorities.insert(0, EntityKind::Rifleman);
        Some(target)
    }

    /// The emergency Rifleman target while `counts` (queued units included) fall short of it.
    /// Reaching it ends the emergency recruiting for good.
    pub(in crate::ai_core::decision) fn unmet_emergency_target(
        &mut self,
        counts: &[(EntityKind, usize)],
    ) -> Option<usize> {
        let riflemen = counts
            .iter()
            .find_map(|(kind, count)| (*kind == EntityKind::Rifleman).then_some(*count))
            .unwrap_or(0);
        self.note_rifle_count(riflemen);
        self.emergency_rifle_target
    }

    fn note_rifle_count(&mut self, riflemen: usize) {
        if self
            .emergency_rifle_target
            .is_some_and(|target| riflemen >= target)
        {
            self.emergency_rifle_target = None;
        }
    }
}

/// Lets a Barracks train Riflemen up to `target` whatever its cap was.
pub(super) fn raise_rifle_cap(max_counts: &mut Vec<(EntityKind, usize)>, target: usize) {
    match max_counts
        .iter_mut()
        .find(|(kind, _)| *kind == EntityKind::Rifleman)
    {
        Some((_, max)) => *max = (*max).max(target),
        None => max_counts.push((EntityKind::Rifleman, target)),
    }
}

#[derive(Debug, Default)]
pub(super) struct RushDecision {
    pub(super) moved: Vec<u32>,
    pub(super) attacked: Vec<u32>,
    /// Survivors handed back to home defense this decision.
    pub(super) released: Vec<u32>,
}

pub(super) fn plan(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
) -> RushDecision {
    let mut decision = RushDecision::default();
    // Squad members dug in by now, read before the rush state is borrowed.
    let dug_in: BTreeSet<u32> = memory
        .opening_rush
        .squad
        .iter()
        .copied()
        .filter(|id| {
            memory.estimated_entrenchment_ticks(observation, *id)
                >= rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
        })
        .collect();
    let rush = &mut memory.opening_rush;
    if rush.phase == RushPhase::NotStarted {
        start(rush, observation, facts, map_analysis);
    }
    if !rush.active() {
        return decision;
    }
    let squad: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| rush.squad.contains(&unit.id) && unit.hp > 0)
        .collect();
    rush.squad = squad.iter().map(|unit| unit.id).collect();
    if squad.is_empty() {
        rush.phase = RushPhase::Done;
        return decision;
    }
    note_enemy_speeds(rush, observation, map_analysis);
    let tile = observation.map.tile_size.max(1) as f32;
    let center = centroid(&squad);
    let near = enemies_near(observation, &squad, ENGAGE_RADIUS_TILES * tile);
    let home = jeff::rifleman_home_rally(observation, facts)
        .unwrap_or_else(|| tile_center(observation.own_start_tile, observation.map.tile_size));
    let denying = rush.deny_center.is_some();

    let raid_near_home = enemy_fighters_near_home(observation);
    if rush.missed_contact && raid_near_home {
        rush.home_raid_seen = true;
    }
    if rush.phase == RushPhase::March {
        if near.iter().any(|enemy| enemy.kind != EntityKind::Worker) {
            rush.squad_contact = true;
        }
        // After a relaunch the enemy is known to have kept its Riflemen home, so only a raid
        // actually seen near the main turns the squad around again.
        let past_middle = !rush.relaunched && past_the_middle(observation, facts, center);
        if !rush.squad_contact && (raid_near_home || past_middle) {
            if !rush.missed_contact {
                rush.emergency_rifle_target =
                    Some(facts.unit_count(EntityKind::Rifleman) + EMERGENCY_RIFLEMEN);
            }
            rush.missed_contact = true;
            rush.home_raid_seen |= raid_near_home;
            begin_withdraw(rush, WithdrawReason::MissedContact);
        }
    }

    if rush.phase == RushPhase::March {
        let enemy_entrenchment = observation.tick >= EARLIEST_ENTRENCHMENT_TICK;
        let own_entrenchment = observation.upgrades.contains(&UpgradeKind::Entrenchment);
        // A squad on its way to dig in at the enemy natural is the entrenched side there, so the
        // Entrenchment and Methamphetamines warnings are left to the trade estimate.
        let reason = if !denying && (enemy_entrenchment || own_entrenchment) {
            Some(WithdrawReason::Entrenchment)
        } else if !denying && rush.enemy_meth_seen {
            Some(WithdrawReason::Methamphetamines)
        } else if trade(
            observation,
            &squad,
            &near,
            rush.enemy_meth_seen,
            enemy_entrenchment,
            &BTreeSet::new(),
        )
        .is_some_and(TradeEstimate::clearly_losing)
        {
            Some(WithdrawReason::BadTrade)
        } else {
            None
        };
        if let Some(reason) = reason {
            begin_withdraw(rush, reason);
        } else if rush.deny_center.is_some_and(|post| {
            distance(center, (post.0 as f32, post.1 as f32)) <= DENY_ENTER_TILES * tile
        }) {
            rush.phase = RushPhase::Deny;
            rush.deny_since = Some(observation.tick);
        }
    }

    if rush.phase == RushPhase::Deny {
        let threats = enemies_near(observation, &squad, DENY_THREAT_TILES * tile);
        let estimate = trade(
            observation,
            &squad,
            &threats,
            rush.enemy_meth_seen,
            false,
            &dug_in,
        );
        if estimate.is_some_and(|estimate| deny_withdraws(estimate, squad.len(), dug_in.len())) {
            begin_withdraw(rush, WithdrawReason::BadTrade);
        }
    }

    if rush.phase == RushPhase::Withdraw {
        if distance((center.0, center.1), home) <= HOME_ARRIVAL_TILES * tile {
            if rush.withdraw_reason == Some(WithdrawReason::MissedContact)
                && !rush.home_raid_seen
                && !rush.relaunched
            {
                // Nothing followed the squad home: the enemy kept its opening Riflemen in its base
                // rather than taking another lane, so the march goes back out.
                rush.relaunched = true;
                rush.phase = RushPhase::March;
                rush.withdraw_reason = None;
                rush.withdraw_order_tick = None;
                rush.next_waypoint = rush
                    .waypoints
                    .iter()
                    .position(|point| {
                        distance((point.0 as f32, point.1 as f32), home)
                            > RELAUNCH_GATHER_TILES * tile
                    })
                    .unwrap_or(0);
                rush.gather_point = rush.waypoints.get(rush.next_waypoint).copied();
                rush.gather_until = observation.tick + RELAUNCH_GATHER_TICKS;
                rush.gather_ordered = false;
                return decision;
            }
            rush.phase = RushPhase::Done;
            decision.released = rush.squad.iter().copied().collect();
            return decision;
        }
        withdraw(
            actions,
            observation,
            map_analysis,
            rush,
            &squad,
            &near,
            home,
            &mut decision,
        );
        return decision;
    }

    if rush.phase == RushPhase::Deny {
        deny(
            actions,
            observation,
            map_analysis,
            rush,
            &squad,
            &dug_in,
            &mut decision,
        );
        return decision;
    }

    if let Some(point) = rush.gather_point {
        gather(actions, observation, rush, &squad, point, &mut decision);
        if rush.gather_point.is_some() || !decision.released.is_empty() {
            return decision;
        }
    }

    // Marching: move on to the next waypoint once the squad is there, and let the planner either
    // advance on it or fight what is near.
    while rush.next_waypoint + 1 < rush.waypoints.len() {
        let point = rush.waypoints[rush.next_waypoint];
        if distance(center, (point.0 as f32, point.1 as f32)) > WAYPOINT_REACHED_TILES * tile {
            break;
        }
        rush.next_waypoint += 1;
    }
    let Some(&objective) = rush.waypoints.get(rush.next_waypoint) else {
        return decision;
    };
    // The planner fights every enemy it is shown, so it sees only the ones near the squad; enemies
    // spotted by home units elsewhere are not this squad's fight.
    let mut local = observation.clone();
    local
        .visible_enemies
        .retain(|enemy| near.iter().any(|near| near.id == enemy.id));
    let squad_ids: Vec<u32> = rush.squad.iter().copied().collect();
    let orders = plan_rifle_squad(
        &local,
        &squad_ids,
        (objective.0 as f32, objective.1 as f32),
        &squad_params(),
        &mut rush.micro,
    );
    for order in orders {
        match order {
            SquadOrder::Attack { units, target } => {
                if let Some(units) = actions::attack_units(actions, units, target) {
                    decision.attacked.extend(units);
                }
            }
            SquadOrder::AttackMove { units, x, y } => {
                if let Some(units) = actions::attack_move_units(actions, units, x, y) {
                    decision.moved.extend(units);
                }
            }
            SquadOrder::Move { units, x, y } => {
                if let Some(units) = actions::move_units(actions, units, x, y) {
                    decision.moved.extend(units);
                }
            }
            SquadOrder::Hold { units } => {
                if let Some(units) = actions::hold_position_units(actions, units) {
                    decision.moved.extend(units);
                }
            }
        }
    }
    decision
}

/// Brings a relaunched squad together at `point` with one group order before the march resumes.
/// When time runs out, members still far from it stay home and the rest march on.
fn gather(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    rush: &mut OpeningRush,
    squad: &[&AiEntitySummary],
    point: (i32, i32),
    decision: &mut RushDecision,
) {
    let tile = observation.map.tile_size.max(1) as f32;
    let point = (point.0 as f32, point.1 as f32);
    let from_point = |unit: &&AiEntitySummary| distance((unit.x, unit.y), point);
    if observation.tick >= rush.gather_until {
        let stragglers: Vec<u32> = squad
            .iter()
            .filter(|unit| from_point(unit) > GATHER_STRAGGLER_TILES * tile)
            .map(|unit| unit.id)
            .collect();
        for id in &stragglers {
            rush.squad.remove(id);
        }
        decision.released.extend(stragglers);
        rush.gather_point = None;
        return;
    }
    if squad
        .iter()
        .all(|unit| from_point(unit) <= GATHER_REACHED_TILES * tile)
    {
        rush.gather_point = None;
        return;
    }
    let units: Vec<u32> = squad
        .iter()
        .filter(|unit| !rush.gather_ordered || unit.state == AiEntityState::Idle)
        .map(|unit| unit.id)
        .collect();
    if !units.is_empty() {
        if let Some(moved) = actions::attack_move_units(actions, units, point.0, point.1) {
            decision.moved.extend(moved);
        }
    }
    rush.gather_ordered = true;
}

/// Whether enemy fighters are in sight near Jeff's main.
fn enemy_fighters_near_home(observation: &AiObservation) -> bool {
    let tile = observation.map.tile_size.max(1) as f32;
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation.visible_enemies.iter().any(|enemy| {
        enemy.hp > 0
            && enemy.kind.is_unit()
            && enemy.kind != EntityKind::Worker
            && distance(own, (enemy.x, enemy.y)) <= HOME_ALERT_TILES * tile
    })
}

/// Whether the squad has passed the middle of the map.
fn past_the_middle(observation: &AiObservation, facts: &AiFacts, squad_center: (f32, f32)) -> bool {
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    facts.nearest_public_enemy_base.is_some_and(|enemy_base| {
        march_progress(squad_center, own, (enemy_base.x, enemy_base.y)) >= MISSED_CONTACT_PROGRESS
    })
}

/// How far across the map `point` is, from 0 at `own` to 1 at `enemy`: its distance from `own` over
/// the sum of its distances to both.
fn march_progress(point: (f32, f32), own: (f32, f32), enemy: (f32, f32)) -> f32 {
    let from_own = distance(point, own);
    let total = from_own + distance(point, enemy);
    if total <= f32::EPSILON {
        0.0
    } else {
        from_own / total
    }
}

fn begin_withdraw(rush: &mut OpeningRush, reason: WithdrawReason) {
    rush.phase = RushPhase::Withdraw;
    rush.withdraw_reason = Some(reason);
    rush.withdraw_order_tick = None;
}

fn enemies_near<'a>(
    observation: &'a AiObservation,
    squad: &[&AiEntitySummary],
    radius_px: f32,
) -> Vec<&'a AiEntitySummary> {
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit())
        .filter(|enemy| {
            squad
                .iter()
                .any(|unit| dist2(unit.x, unit.y, enemy.x, enemy.y) <= radius_px * radius_px)
        })
        .collect()
}

fn start(
    rush: &mut OpeningRush,
    observation: &AiObservation,
    facts: &AiFacts,
    map_analysis: Option<&AiMapAnalysis>,
) {
    rush.phase = RushPhase::Done;
    // Only from the opening itself: no Barracks yet, so the Riflemen are the starting ones.
    if observation.tick > START_TICK_LIMIT || facts.building_count(EntityKind::Barracks) > 0 {
        return;
    }
    let Some(enemy_base) = facts.nearest_public_enemy_base else {
        return;
    };
    let squad: BTreeSet<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .map(|unit| unit.id)
        .collect();
    if squad.is_empty() {
        return;
    }
    let tile_size = observation.map.tile_size;
    let tile = tile_size.max(1) as f32;
    let mut waypoints: Vec<(i32, i32)> = map_analysis
        .and_then(|analysis| analysis.base_route_tiles(observation.player_id))
        .map(|route| {
            // The route runs from the enemy start to Jeff's; the rush walks it the other way.
            let reversed: Vec<AiTile> = route.iter().rev().copied().collect();
            reversed
                .iter()
                .step_by(WAYPOINT_STRIDE_TILES)
                .chain(reversed.last())
                .map(|tile| {
                    let (x, y) = tile_center((tile.x, tile.y), tile_size);
                    (x as i32, y as i32)
                })
                .collect()
        })
        .unwrap_or_default();
    let enemy_point = (enemy_base.x, enemy_base.y);
    match enemy_natural_edge(observation, enemy_base) {
        Some(natural) => {
            // Walk the route as far as its point nearest the natural, then on to the posts.
            let away = normalized_direction(enemy_point, natural).unwrap_or((1.0, 0.0));
            let post = clamp_to_map(
                (
                    natural.0 + away.0 * DENY_POST_FORWARD_TILES * tile,
                    natural.1 + away.1 * DENY_POST_FORWARD_TILES * tile,
                ),
                observation.map,
            );
            if let Some(cut) = waypoints
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let to = |p: &(i32, i32)| distance((p.0 as f32, p.1 as f32), natural);
                    to(a).total_cmp(&to(b))
                })
                .map(|(index, _)| index)
            {
                waypoints.truncate(cut + 1);
            }
            waypoints.push((post.0 as i32, post.1 as i32));
            rush.deny_center = Some((post.0 as i32, post.1 as i32));
            rush.deny_away = Some(((away.0 * 1000.0) as i32, (away.1 * 1000.0) as i32));
        }
        None => waypoints.push((enemy_point.0 as i32, enemy_point.1 as i32)),
    }
    waypoints.dedup();
    rush.starters = squad.clone();
    rush.squad = squad;
    rush.waypoints = waypoints;
    rush.next_waypoint = 0;
    rush.phase = RushPhase::March;
}

/// Dug in beside the enemy's natural: each Rifleman takes a post, shoots the best target already
/// in its reach, and otherwise holds so it digs in (or keeps its trench).
fn deny(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    rush: &mut OpeningRush,
    squad: &[&AiEntitySummary],
    dug_in: &BTreeSet<u32>,
    decision: &mut RushDecision,
) {
    let Some(center) = rush.deny_center else {
        return;
    };
    let tick = observation.tick;
    let tile = observation.map.tile_size.max(1) as f32;
    let away = rush
        .deny_away
        .map_or((1.0, 0.0), |(x, y)| (x as f32 / 1000.0, y as f32 / 1000.0));
    let side = (-away.1, away.0);
    let base_reach = rifle_reach_px(tile);
    let dug_in_reach =
        base_reach + rts_rules::balance::ENTRENCHMENT_RANGE_BONUS_TILES as f32 * tile;
    for (index, unit) in squad.iter().enumerate() {
        let lateral = DENY_POST_LATERAL_TILES[index % DENY_POST_LATERAL_TILES.len()];
        let wanted = (
            center.0 as f32 + side.0 * lateral * tile,
            center.1 as f32 + side.1 * lateral * tile,
        );
        let post = map_analysis
            .and_then(|analysis| analysis.open_ground_near((unit.x, unit.y), wanted, 1, 3))
            .unwrap_or(wanted);
        let reach = if dug_in.contains(&unit.id) {
            dug_in_reach
        } else {
            base_reach
        };
        let target = pick_deny_target(&observation.visible_enemies, (unit.x, unit.y), reach, tile);
        if let Some(target) = target {
            if rush.deny_targets.get(&unit.id) != Some(&target) || unit.state == AiEntityState::Idle
            {
                if let Some(units) = actions::attack_units(actions, [unit.id], target) {
                    decision.attacked.extend(units);
                }
                rush.deny_targets.insert(unit.id, target);
                rush.deny_holding.remove(&unit.id);
            }
            continue;
        }
        rush.deny_targets.remove(&unit.id);
        let at_post = distance((unit.x, unit.y), post) <= DENY_POST_TOLERANCE_TILES * tile;
        if !at_post {
            let due = rush
                .deny_order_tick
                .get(&unit.id)
                .is_none_or(|ordered| tick.saturating_sub(*ordered) >= DENY_REORDER_TICKS);
            if due || unit.state == AiEntityState::Idle {
                if let Some(units) = actions::attack_move_units(actions, [unit.id], post.0, post.1)
                {
                    decision.moved.extend(units);
                }
                rush.deny_order_tick.insert(unit.id, tick);
                rush.deny_holding.remove(&unit.id);
            }
        } else if rush.deny_holding.insert(unit.id) {
            if let Some(units) = actions::hold_position_units(actions, [unit.id]) {
                decision.moved.extend(units);
            }
        }
    }
}

/// Centre-to-centre distance at which a Rifleman fires, as in the simulation.
fn rifle_reach_px(tile: f32) -> f32 {
    let range = rts_rules::combat::default_weapon_profile(EntityKind::Rifleman)
        .map_or(5.0, |weapon| weapon.range_tiles);
    let radius =
        rts_rules::defs::unit_def(EntityKind::Rifleman).map_or(9.0, |def| def.stats.radius);
    range * tile + radius + SIM_RANGE_SLACK_PX
}

/// The denial target already in reach of a Rifleman at `from`: the builder first, then fighters
/// (weakest first), extractors, the Depot, and other buildings.
fn pick_deny_target(
    enemies: &[AiEntitySummary],
    from: (f32, f32),
    reach_px: f32,
    tile: f32,
) -> Option<u32> {
    let rank = |enemy: &AiEntitySummary| match enemy.kind {
        EntityKind::Worker => 0,
        kind if kind.is_unit() => 1,
        EntityKind::SteelMine | EntityKind::PumpJack => 2,
        EntityKind::ResourceDepot => 3,
        _ => 4,
    };
    enemies
        .iter()
        .filter(|enemy| enemy.hp > 0)
        .filter(|enemy| {
            let slack = if enemy.kind.is_unit() {
                0.0
            } else {
                BUILDING_REACH_SLACK_TILES * tile
            };
            distance(from, (enemy.x, enemy.y)) <= reach_px + slack
        })
        .min_by_key(|enemy| {
            (
                rank(enemy),
                enemy.hp,
                distance(from, (enemy.x, enemy.y)) as u32,
                enemy.id,
            )
        })
        .map(|enemy| enemy.id)
}

/// Records enemy Rifleman and Machine Gunner positions and flags Methamphetamines when one is
/// seen covering more ground than its unboosted speed allows, with both sightings off roads.
fn note_enemy_speeds(
    rush: &mut OpeningRush,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
) {
    let tick = observation.tick;
    let tile = observation.map.tile_size.max(1) as f32;
    let off_road = |x: f32, y: f32| {
        map_analysis.is_some_and(|analysis| {
            x >= 0.0 && y >= 0.0 && !analysis.tile_is_road((x / tile) as u32, (y / tile) as u32)
        })
    };
    for enemy in &observation.visible_enemies {
        if !matches!(enemy.kind, EntityKind::Rifleman | EntityKind::MachineGunner) || enemy.hp == 0
        {
            continue;
        }
        if let Some(&(seen, x, y)) = rush.sightings.get(&enemy.id) {
            let elapsed = tick.saturating_sub(seen);
            if (1..=SPEED_SAMPLE_MAX_TICKS).contains(&elapsed)
                && off_road(x as f32, y as f32)
                && off_road(enemy.x, enemy.y)
            {
                let speed = distance((x as f32, y as f32), (enemy.x, enemy.y)) / elapsed as f32;
                let base = rts_rules::defs::unit_def(enemy.kind)
                    .map(|def| def.stats.speed)
                    .unwrap_or(RIFLEMAN_SPEED_PX);
                if speed > base * METH_SPEED_MARGIN {
                    rush.enemy_meth_seen = true;
                }
            }
        }
        rush.sightings
            .insert(enemy.id, (tick, enemy.x as i32, enemy.y as i32));
    }
    rush.sightings
        .retain(|_, (seen, _, _)| tick.saturating_sub(*seen) <= SIGHTING_MEMORY_TICKS);
}

/// One side of an estimated fight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Fighter {
    pub(super) kind: EntityKind,
    pub(super) hp: u32,
    pub(super) cooldown: u32,
    /// Resource value, for weighing losses against kills.
    pub(super) value: u32,
    pub(super) entrenched: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TradeEstimate {
    pub(super) value_lost: u32,
    pub(super) value_killed: u32,
    pub(super) units_lost: usize,
    pub(super) units_killed: usize,
}

impl TradeEstimate {
    /// Costs more than twice what it kills. The volley estimate credits none of the squad
    /// planner's pull-backs, and turning away mid-fight hands the enemy free shots, so a fight
    /// that only looks somewhat worse is still taken.
    pub(super) fn clearly_losing(self) -> bool {
        self.value_lost > self.value_killed.saturating_mul(2)
    }

    /// All `side` units die without a single kill.
    pub(super) fn hopeless(self, side: usize) -> bool {
        self.units_killed == 0 && self.units_lost >= side
    }
}

/// Whether a denying squad of `squad` Riflemen, `dug_in` of them in trenches, gives the natural
/// up. Once at least half is dug in it holds unless it would be wiped out without a kill: walking
/// out of the trenches under fire costs about as much as staying, and the natural is the point of
/// the rush. Before that it leaves only a clearly lost fight.
fn deny_withdraws(estimate: TradeEstimate, squad: usize, dug_in: usize) -> bool {
    if dug_in.saturating_mul(2) >= squad {
        estimate.hopeless(squad)
    } else {
        estimate.clearly_losing()
    }
}

/// The estimated fight against `near`, or `None` when no enemy fighter is near. `ours_dug_in`
/// squad members fight from trenches.
fn trade(
    observation: &AiObservation,
    squad: &[&AiEntitySummary],
    near: &[&AiEntitySummary],
    enemy_meth: bool,
    enemy_entrenchment: bool,
    ours_dug_in: &BTreeSet<u32>,
) -> Option<TradeEstimate> {
    let own_meth = observation
        .upgrades
        .contains(&UpgradeKind::Methamphetamines);
    let ours: Vec<Fighter> = squad
        .iter()
        .filter_map(|unit| fighter(unit, own_meth, ours_dug_in.contains(&unit.id)))
        .collect();
    let theirs: Vec<Fighter> = near
        .iter()
        .filter(|enemy| enemy.kind != EntityKind::Worker)
        .filter_map(|enemy| {
            // An enemy standing still once Entrenchment is possible may be dug in.
            let entrenched = enemy_entrenchment && enemy.state != AiEntityState::Move;
            fighter(enemy, enemy_meth, entrenched)
        })
        .collect();
    if theirs.is_empty() {
        return None;
    }
    // Dug-in infantry outranges a Rifleman by a tile; the squad takes fire while it closes.
    let head_start = if theirs.iter().any(|enemy| enemy.entrenched) {
        (observation.map.tile_size as f32 / RIFLEMAN_SPEED_PX).ceil() as u32
    } else {
        0
    };
    Some(estimate_trade(&ours, &theirs, head_start))
}

fn fighter(unit: &AiEntitySummary, meth: bool, entrenched: bool) -> Option<Fighter> {
    let weapon = rts_rules::combat::default_weapon_profile(unit.kind)?;
    let cooldown = if meth && matches!(unit.kind, EntityKind::Rifleman) {
        rts_rules::balance::METHAMPHETAMINES_RIFLEMAN_ATTACK_COOLDOWN_TICKS
    } else {
        weapon.cooldown
    };
    let (steel, oil) = rts_rules::economy::cost(unit.kind);
    Some(Fighter {
        kind: unit.kind,
        hp: unit.hp,
        cooldown: cooldown.max(1),
        value: steel + oil,
        entrenched,
    })
}

/// Damage one shot from `attacker` does to `victim`. A Machine Gunner burst counts as two hits.
fn shot_damage(attacker: &Fighter, victim: &Fighter) -> u32 {
    let base = rts_rules::combat::default_weapon_profile(attacker.kind)
        .map(|weapon| weapon.dmg)
        .unwrap_or(0);
    let hits = if attacker.kind == EntityKind::MachineGunner {
        2
    } else {
        1
    };
    let damage = rts_rules::combat::effective_damage(attacker.kind, victim.kind, base, None);
    rts_rules::combat::direct_damage_after_entrenchment(victim.kind, damage, victim.entrenched)
        * hits
}

/// Fights `ours` against `theirs` to the end in whole volleys. Every shooter fires on cooldown at
/// the weakest enemy still standing, moving on once the shots already aimed at it cover its HP.
/// `their_head_start` is how many ticks they fire before the squad can shoot back.
pub(super) fn estimate_trade(
    ours: &[Fighter],
    theirs: &[Fighter],
    their_head_start: u32,
) -> TradeEstimate {
    const MAX_TICKS: u32 = 1_800;
    let mut ours: Vec<(Fighter, u32)> = ours.iter().map(|f| (*f, their_head_start)).collect();
    let mut theirs: Vec<(Fighter, u32)> = theirs.iter().map(|f| (*f, 0)).collect();
    let mut tick = 0;
    while tick <= MAX_TICKS {
        let alive = |side: &[(Fighter, u32)]| side.iter().any(|(f, _)| f.hp > 0);
        if !alive(&ours) || !alive(&theirs) {
            break;
        }
        let our_hits = volley(&ours, &theirs, tick);
        let their_hits = volley(&theirs, &ours, tick);
        apply(&mut theirs, &our_hits);
        apply(&mut ours, &their_hits);
        for side in [&mut ours, &mut theirs] {
            for (fighter, next) in side.iter_mut() {
                if fighter.hp > 0 && *next <= tick {
                    *next = tick + fighter.cooldown;
                }
            }
        }
        let next_tick = ours
            .iter()
            .chain(theirs.iter())
            .filter(|(f, _)| f.hp > 0)
            .map(|(_, next)| *next)
            .min()
            .unwrap_or(MAX_TICKS + 1);
        tick = next_tick.max(tick + 1);
    }
    let lost: Vec<&Fighter> = ours.iter().map(|(f, _)| f).filter(|f| f.hp == 0).collect();
    let killed: Vec<&Fighter> = theirs
        .iter()
        .map(|(f, _)| f)
        .filter(|f| f.hp == 0)
        .collect();
    TradeEstimate {
        value_lost: lost.iter().map(|f| f.value).sum(),
        value_killed: killed.iter().map(|f| f.value).sum(),
        units_lost: lost.len(),
        units_killed: killed.len(),
    }
}

/// Damage per target index from every `shooters` member ready at `tick`.
fn volley(shooters: &[(Fighter, u32)], targets: &[(Fighter, u32)], tick: u32) -> Vec<u32> {
    let mut pending = vec![0_u32; targets.len()];
    for (shooter, next) in shooters {
        if shooter.hp == 0 || *next > tick {
            continue;
        }
        let pick = targets
            .iter()
            .enumerate()
            .filter(|(index, (target, _))| target.hp > pending[*index])
            .min_by_key(|(index, (target, _))| (target.hp - pending[*index], *index))
            .map(|(index, _)| index);
        if let Some(index) = pick {
            pending[index] += shot_damage(shooter, &targets[index].0);
        }
    }
    pending
}

fn apply(side: &mut [(Fighter, u32)], hits: &[u32]) {
    for ((fighter, _), damage) in side.iter_mut().zip(hits) {
        fighter.hp = fighter.hp.saturating_sub(*damage);
    }
}

#[allow(clippy::too_many_arguments)]
fn withdraw(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    rush: &mut OpeningRush,
    squad: &[&AiEntitySummary],
    near: &[&AiEntitySummary],
    home: (f32, f32),
    decision: &mut RushDecision,
) {
    let tick = observation.tick;
    if rush.withdraw_reason == Some(WithdrawReason::MissedContact) {
        // Home is where the enemy's opening attack went: walk straight back and fight whatever is
        // there, instead of stepping around it.
        let idle = squad.iter().any(|unit| unit.state == AiEntityState::Idle);
        if rush.withdraw_order_tick.is_none() || idle {
            let units = squad.iter().map(|unit| unit.id);
            if let Some(moved) = actions::attack_move_units(actions, units, home.0, home.1) {
                decision.moved.extend(moved);
            }
            rush.withdraw_order_tick = Some(tick);
        }
        return;
    }
    let threats: Vec<&AiEntitySummary> = near
        .iter()
        .copied()
        .filter(|enemy| enemy.kind != EntityKind::Worker)
        .collect();
    let due = rush
        .withdraw_order_tick
        .is_none_or(|ordered| tick.saturating_sub(ordered) >= WITHDRAW_REORDER_TICKS);
    let idle = squad.iter().any(|unit| unit.state == AiEntityState::Idle);
    if !(due && (!threats.is_empty() || rush.withdraw_order_tick.is_none()) || idle) {
        return;
    }
    let units: Vec<u32> = squad.iter().map(|unit| unit.id).collect();
    let center = centroid(squad);
    let tile = observation.map.tile_size.max(1) as f32;
    if !threats.is_empty() {
        // Step off the enemy's line of march toward Jeff's base, on the side the squad is already
        // on, then go home.
        let threat_center = centroid(&threats);
        let march = normalized_direction(threat_center, home).unwrap_or((1.0, 0.0));
        let side = (-march.1, march.0);
        let offset = (center.0 - threat_center.0, center.1 - threat_center.1);
        let sign = if offset.0 * side.0 + offset.1 * side.1 >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let wanted = clamp_to_map(
            (
                center.0 + side.0 * sign * SIDESTEP_TILES * tile,
                center.1 + side.1 * sign * SIDESTEP_TILES * tile,
            ),
            observation.map,
        );
        let step = map_analysis
            .and_then(|analysis| analysis.open_ground_near(center, wanted, 1, 6))
            .unwrap_or(wanted);
        if let Some(moved) = actions::move_units(actions, units.iter().copied(), step.0, step.1) {
            decision.moved.extend(moved);
        }
        actions::move_units_with_queue(actions, units, home.0, home.1, true);
    } else if let Some(moved) = actions::move_units(actions, units, home.0, home.1) {
        decision.moved.extend(moved);
    }
    rush.withdraw_order_tick = Some(tick);
}

fn centroid(units: &[&AiEntitySummary]) -> (f32, f32) {
    let n = units.len().max(1) as f32;
    let (x, y) = units
        .iter()
        .fold((0.0, 0.0), |(x, y), unit| (x + unit.x, y + unit.y));
    (x / n, y / n)
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rifle(hp: u32, entrenched: bool) -> Fighter {
        Fighter {
            kind: EntityKind::Rifleman,
            hp,
            cooldown: 32,
            value: 35,
            entrenched,
        }
    }

    fn enemy(id: u32, kind: EntityKind, tiles_away: f32, hp: u32) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 2,
            kind,
            x: tiles_away * 32.0,
            y: 0.0,
            hp,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: true,
        }
    }

    #[test]
    fn three_riflemen_take_a_good_trade_against_one_dug_in_rifleman() {
        let ours = [rifle(45, false); 3];
        let theirs = [rifle(45, true)];
        let estimate = estimate_trade(&ours, &theirs, 20);
        assert_eq!(estimate.units_killed, 1);
        assert_eq!(estimate.units_lost, 0);
        assert!(!estimate.clearly_losing());
    }

    #[test]
    fn a_lone_rifleman_loses_to_a_dug_in_rifleman() {
        let estimate = estimate_trade(&[rifle(45, false)], &[rifle(45, true)], 20);
        assert_eq!(estimate.units_lost, 1);
        assert_eq!(estimate.units_killed, 0);
        assert!(estimate.clearly_losing());
    }

    #[test]
    fn an_even_open_fight_is_taken_but_being_outnumbered_two_to_one_is_not() {
        let four = [rifle(45, false); 4];
        assert!(!estimate_trade(&four, &four, 0).clearly_losing());
        let two = [rifle(45, false); 2];
        assert!(estimate_trade(&two, &four, 0).clearly_losing());
        assert!(!estimate_trade(&four, &two, 0).clearly_losing());
    }

    #[test]
    fn wounded_squad_against_fresh_enemies_is_a_bad_trade() {
        let wounded = [rifle(10, false); 4];
        let fresh = [rifle(45, false); 3];
        assert!(estimate_trade(&wounded, &fresh, 0).clearly_losing());
    }

    #[test]
    fn a_dug_in_denial_holds_unless_it_would_die_without_a_kill() {
        // Four in trenches against six fresh Riflemen take some with them, so they stay.
        let outnumbered = estimate_trade(&[rifle(45, true); 4], &[rifle(45, false); 6], 0);
        assert!(outnumbered.units_killed > 0);
        assert!(!deny_withdraws(outnumbered, 4, 4));
        assert!(!deny_withdraws(outnumbered, 4, 2));
        // A badly wounded Rifleman in its trench against four dies without a kill, so it goes.
        let hopeless = estimate_trade(&[rifle(5, true)], &[rifle(45, false); 4], 0);
        assert!(hopeless.hopeless(1));
        assert!(deny_withdraws(hopeless, 1, 1));
    }

    #[test]
    fn a_denial_not_yet_dug_in_leaves_only_a_clearly_lost_fight() {
        let trade = |lost: usize, killed: usize| TradeEstimate {
            value_lost: lost as u32 * 35,
            value_killed: killed as u32 * 35,
            units_lost: lost,
            units_killed: killed,
        };
        // One of four in a trench is not dug in: three for two is taken, three for one is not.
        assert!(!deny_withdraws(trade(3, 2), 4, 1));
        assert!(deny_withdraws(trade(3, 1), 4, 1));
        // Two of four is half the squad dug in: three for one is held.
        assert!(!deny_withdraws(trade(3, 1), 4, 2));
        assert!(deny_withdraws(trade(4, 0), 4, 2));
    }

    #[test]
    fn a_dug_in_squad_holds_against_as_many_attackers() {
        let dug_in = [rifle(45, true); 4];
        let attackers = [rifle(45, false); 4];
        let estimate = estimate_trade(&dug_in, &attackers, 0);
        assert_eq!(estimate.units_killed, 4);
        assert!(!estimate.clearly_losing());
    }

    #[test]
    fn denial_shoots_the_builder_first_then_fighters_then_extractors_then_the_depot() {
        let tile = 32.0;
        let reach = rifle_reach_px(tile);
        let mut enemies = vec![
            enemy(1, EntityKind::ResourceDepot, 4.0, 100),
            enemy(2, EntityKind::SteelMine, 4.0, 37),
            enemy(3, EntityKind::Rifleman, 4.0, 45),
            enemy(4, EntityKind::Worker, 4.5, 40),
        ];
        assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), Some(4));
        enemies.retain(|e| e.id != 4);
        assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), Some(3));
        enemies.retain(|e| e.id != 3);
        assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), Some(2));
        enemies.retain(|e| e.id != 2);
        assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), Some(1));
    }

    #[test]
    fn denial_never_targets_what_is_out_of_reach() {
        let tile = 32.0;
        let reach = rifle_reach_px(tile);
        let enemies = vec![enemy(4, EntityKind::Worker, 7.0, 40)];
        assert_eq!(pick_deny_target(&enemies, (0.0, 0.0), reach, tile), None);
        let dug_in = reach + tile;
        assert_eq!(
            pick_deny_target(&enemies, (0.0, 0.0), dug_in, tile),
            None,
            "seven tiles is beyond even a dug-in Rifleman"
        );
        let close = vec![enemy(4, EntityKind::Worker, 6.0, 40)];
        assert_eq!(pick_deny_target(&close, (0.0, 0.0), dug_in, tile), Some(4));
    }

    #[test]
    fn the_middle_of_the_map_is_equally_far_from_both_bases() {
        let own = (0.0, 0.0);
        let enemy = (1000.0, 0.0);
        assert!((march_progress((500.0, 0.0), own, enemy) - 0.5).abs() < 1e-6);
        assert!(march_progress((400.0, 0.0), own, enemy) < MISSED_CONTACT_PROGRESS);
        // A lane off the straight line still reaches the middle where both distances match.
        assert!((march_progress((500.0, 300.0), own, enemy) - 0.5).abs() < 1e-6);
        assert!(march_progress((600.0, 300.0), own, enemy) > MISSED_CONTACT_PROGRESS);
    }

    #[test]
    fn emergency_recruiting_stops_once_its_riflemen_are_trained_or_queued() {
        let mut rush = OpeningRush {
            emergency_rifle_target: Some(6),
            ..OpeningRush::default()
        };
        let mut priorities = vec![EntityKind::Tank, EntityKind::Rifleman];
        assert_eq!(rush.lead_with_emergency_riflemen(&mut priorities), Some(6));
        assert_eq!(priorities, vec![EntityKind::Rifleman, EntityKind::Tank]);
        let counts = |riflemen| vec![(EntityKind::Rifleman, riflemen)];
        assert_eq!(rush.unmet_emergency_target(&counts(5)), Some(6));
        assert_eq!(rush.unmet_emergency_target(&counts(6)), None);
        // Later losses do not restart it.
        assert_eq!(rush.unmet_emergency_target(&counts(2)), None);
        let mut untouched = vec![EntityKind::Tank];
        assert_eq!(rush.lead_with_emergency_riflemen(&mut untouched), None);
        assert_eq!(untouched, vec![EntityKind::Tank]);
        let mut caps = vec![(EntityKind::MachineGunner, 4), (EntityKind::Rifleman, 3)];
        raise_rifle_cap(&mut caps, 6);
        assert_eq!(caps[1], (EntityKind::Rifleman, 6));
    }

    #[test]
    fn only_the_live_jeff_rushes() {
        assert!(uses_opening_rush(JEFFS_AI_ID));
        assert!(!uses_opening_rush(JEFFS_AI_PRE_OPENING_RUSH_ID));
        assert!(!uses_opening_rush(crate::ai_core::profiles::AI_2_1_ID));
    }
}
