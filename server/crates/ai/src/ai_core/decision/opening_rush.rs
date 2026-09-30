//! Jeff's opening rush.
//!
//! The four starting Riflemen march on the enemy base in one group along the enemy's own attack
//! route (the map analysis base route, walked backwards), so they meet an enemy opening attack
//! head on. Whatever they meet is fought by the rifle squad planner (`squad_micro`): focus fire on
//! the weakest enemy in reach, no overkill, and wounded Riflemen under fire step back.
//!
//! The group falls back home, stepping off the enemy's line of march first, as soon as:
//! - Entrenchment could be available to either player. Jeff's own research is known; the enemy's
//!   is private, so the earliest tick anyone could have finished it stands in for it.
//! - An enemy Rifleman or Machine Gunner is seen moving faster than it can without
//!   Methamphetamines, which also doubles the Rifleman's rate of fire.
//! - The fight in sight would cost more than it kills (`estimate_trade`).
//!
//! Survivors that reach home are released to the home pocket. Only the live Jeff runs this; the
//! `jeffs_ai_pre_opening_rush` freeze keeps its starting Riflemen at home.

use super::geometry::dist2;
use super::*;
use crate::ai_core::map_analysis::AiTile;
use crate::ai_core::squad_micro::{
    plan_rifle_squad, RifleSquadParams, SquadMicroMemory, SquadOrder,
};

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
    Withdraw,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WithdrawReason {
    Entrenchment,
    Methamphetamines,
    BadTrade,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct OpeningRush {
    phase: RushPhase,
    squad: BTreeSet<u32>,
    /// March waypoints in world pixels, home to enemy base.
    waypoints: Vec<(i32, i32)>,
    next_waypoint: usize,
    micro: SquadMicroMemory,
    withdraw_reason: Option<WithdrawReason>,
    withdraw_order_tick: Option<u32>,
    /// Last sighting of each enemy Rifleman and Machine Gunner: tick and position.
    sightings: BTreeMap<u32, (u32, i32, i32)>,
    enemy_meth_seen: bool,
}

impl OpeningRush {
    /// Units the rush owns while it runs. No other system may order them.
    pub(in crate::ai_core::decision) fn reserved(&self) -> impl Iterator<Item = u32> + '_ {
        let active = matches!(self.phase, RushPhase::March | RushPhase::Withdraw);
        self.squad.iter().copied().filter(move |_| active)
    }

    pub(in crate::ai_core::decision) fn is_reserved(&self, id: u32) -> bool {
        matches!(self.phase, RushPhase::March | RushPhase::Withdraw) && self.squad.contains(&id)
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
    let rush = &mut memory.opening_rush;
    if rush.phase == RushPhase::NotStarted {
        start(rush, observation, facts, map_analysis);
    }
    if !matches!(rush.phase, RushPhase::March | RushPhase::Withdraw) {
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
    let engage_px = ENGAGE_RADIUS_TILES * tile;
    let near: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit())
        .filter(|enemy| {
            squad
                .iter()
                .any(|unit| dist2(unit.x, unit.y, enemy.x, enemy.y) <= engage_px * engage_px)
        })
        .collect();
    let home = jeff::rifleman_home_rally(observation, facts)
        .unwrap_or_else(|| tile_center(observation.own_start_tile, observation.map.tile_size));

    if rush.phase == RushPhase::March {
        let enemy_entrenchment = observation.tick >= EARLIEST_ENTRENCHMENT_TICK;
        let reason =
            if enemy_entrenchment || observation.upgrades.contains(&UpgradeKind::Entrenchment) {
                Some(WithdrawReason::Entrenchment)
            } else if rush.enemy_meth_seen {
                Some(WithdrawReason::Methamphetamines)
            } else if bad_trade(
                observation,
                &squad,
                &near,
                rush.enemy_meth_seen,
                enemy_entrenchment,
            ) {
                Some(WithdrawReason::BadTrade)
            } else {
                None
            };
        if let Some(reason) = reason {
            rush.phase = RushPhase::Withdraw;
            rush.withdraw_reason = Some(reason);
            rush.withdraw_order_tick = None;
        }
    }

    if rush.phase == RushPhase::Withdraw {
        if distance((center.0, center.1), home) <= HOME_ARRIVAL_TILES * tile {
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

fn start(
    rush: &mut OpeningRush,
    observation: &AiObservation,
    facts: &AiFacts,
    map_analysis: Option<&AiMapAnalysis>,
) {
    rush.phase = RushPhase::Done;
    if observation.tick > START_TICK_LIMIT {
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
    waypoints.push((enemy_base.x as i32, enemy_base.y as i32));
    waypoints.dedup();
    rush.squad = squad;
    rush.waypoints = waypoints;
    rush.next_waypoint = 0;
    rush.phase = RushPhase::March;
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
    pub(super) fn unfavorable(self) -> bool {
        self.value_lost > self.value_killed
    }
}

fn bad_trade(
    observation: &AiObservation,
    squad: &[&AiEntitySummary],
    near: &[&AiEntitySummary],
    enemy_meth: bool,
    enemy_entrenchment: bool,
) -> bool {
    let own_meth = observation
        .upgrades
        .contains(&UpgradeKind::Methamphetamines);
    let ours: Vec<Fighter> = squad
        .iter()
        .filter_map(|unit| fighter(unit, own_meth, false))
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
        return false;
    }
    // Dug-in infantry outranges a Rifleman by a tile; the squad takes fire while it closes.
    let head_start = if theirs.iter().any(|enemy| enemy.entrenched) {
        (observation.map.tile_size as f32 / RIFLEMAN_SPEED_PX).ceil() as u32
    } else {
        0
    };
    estimate_trade(&ours, &theirs, head_start).unfavorable()
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

    #[test]
    fn three_riflemen_take_a_good_trade_against_one_dug_in_rifleman() {
        let ours = [rifle(45, false); 3];
        let theirs = [rifle(45, true)];
        let estimate = estimate_trade(&ours, &theirs, 20);
        assert_eq!(estimate.units_killed, 1);
        assert_eq!(estimate.units_lost, 0);
        assert!(!estimate.unfavorable());
    }

    #[test]
    fn a_lone_rifleman_loses_to_a_dug_in_rifleman() {
        let estimate = estimate_trade(&[rifle(45, false)], &[rifle(45, true)], 20);
        assert_eq!(estimate.units_lost, 1);
        assert_eq!(estimate.units_killed, 0);
        assert!(estimate.unfavorable());
    }

    #[test]
    fn an_even_open_fight_is_not_a_bad_trade_but_being_outnumbered_is() {
        let four = [rifle(45, false); 4];
        assert!(!estimate_trade(&four, &four, 0).unfavorable());
        let two = [rifle(45, false); 2];
        assert!(estimate_trade(&two, &four, 0).unfavorable());
        assert!(!estimate_trade(&four, &two, 0).unfavorable());
    }

    #[test]
    fn wounded_squad_against_fresh_enemies_is_a_bad_trade() {
        let wounded = [rifle(10, false); 4];
        let fresh = [rifle(45, false); 3];
        assert!(estimate_trade(&wounded, &fresh, 0).unfavorable());
    }

    #[test]
    fn only_the_live_jeff_rushes() {
        assert!(uses_opening_rush(JEFFS_AI_ID));
        assert!(!uses_opening_rush(JEFFS_AI_PRE_OPENING_RUSH_ID));
        assert!(!uses_opening_rush(crate::ai_core::profiles::AI_2_1_ID));
    }
}
