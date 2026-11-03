//! Jeff's AI V1.1's Riflemen hunt the enemy's Panzerfausts. A Panzerfaust is infantry (45 HP, small
//! armour): a Meth Rifleman puts 5 damage on it twice a second, so three kill one in under two
//! seconds, while the Panzerfaust's one loaded shot takes a fifth of a Tank. Against Pressure AI,
//! which sends 2-11 Panzerfausts with every Tank wave, only half the Panzerfausts that died had a
//! Jeff Rifleman or gun near them.
//!
//! Every enemy Panzerfaust in sight within `THREAT_TILES` of a Jeff Tank is a quarry.
//! Each, the nearest to Jeff's Tanks first, gets up to `HUNTERS_PER_FAUST` of the nearest Riflemen
//! within `HUNT_TILES` that no other system holds, push escorts included, ordered to attack it and
//! sent again every `REORDER_TICKS`. The hunt runs last in the decision, so its orders stand over
//! the posts and formations; once the quarry is dead or out of sight the hunters go back to them.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

/// Panzerfausts this close to a Jeff Tank or building are hunted.
const THREAT_TILES: f32 = 10.0;
/// Riflemen this close to a Panzerfaust hunt it.
const HUNT_TILES: f32 = 20.0;
const HUNTERS_PER_FAUST: usize = 2;
/// A hunter is sent again this often while its quarry lives, so formation and post orders given in
/// between do not pull it off.
const REORDER_TICKS: u32 = config::TICK_HZ;

/// V1.1's hunters and their quarries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct HuntMemory {
    /// Hunter -> (quarry, when last ordered).
    pub(in crate::ai_core::decision) hunters: BTreeMap<u32, (u32, u32)>,
}

/// Sends Riflemen at the enemy's Panzerfausts (see the module docs). `held`: units other systems
/// must keep this decision (the raid party, the route line). V1.1 only.
pub(in crate::ai_core::decision) fn hunt_panzerfausts(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile_id: &str,
    held: &BTreeSet<u32>,
) {
    if !is_jeff_v1_1(profile_id) || super::off("hunt") {
        return;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let anchors: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.hp > 0 && unit.kind == EntityKind::Tank)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let tanks: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.hp > 0 && unit.kind == EntityKind::Tank)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let near_tank = |at: (f32, f32)| {
        tanks
            .iter()
            .map(|tank| dist2(tank.0, tank.1, at.0, at.1))
            .fold(f32::INFINITY, f32::min)
    };
    let mut quarries: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Panzerfaust && enemy.hp > 0)
        .filter(|enemy| {
            anchors.iter().any(|anchor| {
                dist2(anchor.0, anchor.1, enemy.x, enemy.y) <= squared(THREAT_TILES * ts)
            })
        })
        .collect();
    quarries.sort_by(|a, b| {
        near_tank((a.x, a.y))
            .total_cmp(&near_tank((b.x, b.y)))
            .then(a.id.cmp(&b.id))
    });
    let alive: BTreeSet<u32> = quarries.iter().map(|enemy| enemy.id).collect();
    let state = &mut memory.jeff_v1_1.hunt;
    state
        .hunters
        .retain(|hunter, (quarry, _)| alive.contains(quarry) && !held.contains(hunter));
    if quarries.is_empty() {
        return;
    }
    let reserved = memory.squad_reserved();
    let free: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !held.contains(&unit.id)
                && !reserved.contains(&unit.id)
                && memory.containment.active_scout != Some(unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.rush_reserved(unit.id)
                && !memory.stays_out_of_home_defense(unit.id)
        })
        .collect();
    let state = &mut memory.jeff_v1_1.hunt;
    for quarry in &quarries {
        let mut on_it = state
            .hunters
            .values()
            .filter(|(target, _)| *target == quarry.id)
            .count();
        let mut nearest: Vec<&&AiEntitySummary> = free
            .iter()
            .filter(|unit| !state.hunters.contains_key(&unit.id))
            .filter(|unit| {
                dist2(unit.x, unit.y, quarry.x, quarry.y) <= squared(HUNT_TILES * ts)
            })
            .collect();
        nearest.sort_by(|a, b| {
            dist2(a.x, a.y, quarry.x, quarry.y)
                .total_cmp(&dist2(b.x, b.y, quarry.x, quarry.y))
                .then(a.id.cmp(&b.id))
        });
        for unit in nearest {
            if on_it >= HUNTERS_PER_FAUST {
                break;
            }
            state.hunters.insert(unit.id, (quarry.id, 0));
            on_it += 1;
        }
    }
    let due: Vec<(u32, u32)> = state
        .hunters
        .iter()
        .filter(|(_, (_, ordered))| tick.saturating_sub(*ordered) >= REORDER_TICKS)
        .map(|(hunter, (quarry, _))| (*hunter, *quarry))
        .collect();
    let mut sent = Vec::new();
    for (hunter, quarry) in due {
        if let Some(units) = actions::attack_units(actions, [hunter], quarry) {
            sent.extend(units);
            state.hunters.insert(hunter, (quarry, tick));
        }
    }
    if !sent.is_empty() {
        intents.push(AiIntent::Attack { units: sent });
    }
}
