//! Jeff's AI V1.1's Tanks know they are seen and keep out of reach of more enemy Tanks than they
//! have. A shot reveals the shooter to its target's side for 6.5 s, and a parked Tank reaches 14
//! tiles: a Tank that has fired, or has an enemy within a Tank's 9 tiles of sight, is in sight of
//! the enemy whether or not it sees them. Against Pressure AI Jeff's Tanks stood and traded at the
//! enemy's range while outnumbered.
//!
//! A Jeff Tank outside the push that is seen, with more enemy Tanks within `REACH_TILES` of it than
//! Jeff Tanks within `SUPPORT_TILES` of it, backs off to `CLEAR_TILES` from the enemy Tanks' centre,
//! straight away from it, re-ordered every `REORDER_TICKS` while it stays outmatched.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

/// The firing reveal (`FIRING_REVEAL_DURATION_TICKS`).
const REVEAL_TICKS: u32 = config::TICK_HZ * 13 / 2;
/// A Tank's sight: an enemy this close sees it anyway.
const SEEN_TILES: f32 = 9.0;
/// A parked Tank's reach, and a little more.
const REACH_TILES: f32 = 14.5;
const SUPPORT_TILES: f32 = 8.0;
const CLEAR_TILES: f32 = 16.0;
const REORDER_TICKS: u32 = config::TICK_HZ;

/// When each Jeff Tank last fired, and when each backing-off Tank was last sent back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct RevealMemory {
    fired: BTreeMap<u32, u32>,
    backing: BTreeMap<u32, u32>,
}

/// Backs seen, outmatched Tanks out of the enemy's reach (see the module docs). `held`: units other
/// systems keep this decision. V1.1 only.
pub(in crate::ai_core::decision) fn keep_out_of_reach(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile_id: &str,
    held: &BTreeSet<u32>,
) {
    if !is_jeff_v1_1(profile_id) || super::off("reveal") {
        return;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .collect();
    let alive: BTreeSet<u32> = tanks.iter().map(|tank| tank.id).collect();
    let state = &mut memory.jeff_v1_1.reveal;
    state.fired.retain(|id, _| alive.contains(id));
    state.backing.retain(|id, _| alive.contains(id));
    for tank in &tanks {
        if tank.state == AiEntityState::Attack || tank.target_id.is_some() {
            state.fired.insert(tank.id, tick);
        }
    }
    let enemy_tanks: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .collect();
    if enemy_tanks.is_empty() {
        memory.jeff_v1_1.reveal.backing.clear();
        return;
    }
    let mut sent = Vec::new();
    for tank in &tanks {
        // The grouped line's Tanks and the home guard hold parked: backing off, a Tank was sent
        // back to its post by the line 3 s later, and moved under fire at 5 tiles' reach both ways.
        if held.contains(&tank.id)
            || memory.containment.active_tanks.contains(&tank.id)
            || memory.jeff_v1_1.home_guard.holds(tank.id)
            || (!super::off("linegroups") && memory.jeff_v1_1.front_line.holds(tank.id))
        {
            continue;
        }
        let threats: Vec<&&AiEntitySummary> = enemy_tanks
            .iter()
            .filter(|enemy| dist2(enemy.x, enemy.y, tank.x, tank.y) <= squared(REACH_TILES * ts))
            .collect();
        let support = tanks
            .iter()
            .filter(|other| dist2(other.x, other.y, tank.x, tank.y) <= squared(SUPPORT_TILES * ts))
            .count();
        let fired = memory
            .jeff_v1_1
            .reveal
            .fired
            .get(&tank.id)
            .is_some_and(|at| tick.saturating_sub(*at) <= REVEAL_TICKS);
        let spotted = observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && dist2(enemy.x, enemy.y, tank.x, tank.y) <= squared(SEEN_TILES * ts)
        });
        if threats.len() <= support || !(fired || spotted) {
            memory.jeff_v1_1.reveal.backing.remove(&tank.id);
            continue;
        }
        let state = &mut memory.jeff_v1_1.reveal;
        if state
            .backing
            .get(&tank.id)
            .is_some_and(|at| tick.saturating_sub(*at) < REORDER_TICKS)
        {
            continue;
        }
        let n = threats.len() as f32;
        let centre = (
            threats.iter().map(|enemy| enemy.x).sum::<f32>() / n,
            threats.iter().map(|enemy| enemy.y).sum::<f32>() / n,
        );
        let Some(away) = normalized_direction(centre, (tank.x, tank.y)) else {
            continue;
        };
        let point = clamp_to_map(
            (
                centre.0 + away.0 * CLEAR_TILES * ts,
                centre.1 + away.1 * CLEAR_TILES * ts,
            ),
            observation.map,
        );
        actions::move_units(actions, [tank.id], point.0, point.1);
        state.backing.insert(tank.id, tick);
        sent.push(tank.id);
    }
    if !sent.is_empty() {
        intents.push(AiIntent::Assemble { units: sent });
    }
}
