//! Jeff's AI V1.1's Riflemen charge enemy Tanks shelling its bases. A parked Tank reaches 14 tiles
//! and a Rifleman 5, so Riflemen holding their posts under a Tank's fire only die: one Tank shell
//! kills a Rifleman about every 5 s. Small arms do a Tank 1 damage a hit, but a Meth Rifleman hits
//! twice a second: ten of them kill a 292-HP Tank in about 16 s, for the 3-4 Riflemen it shells on
//! the way in, 140 Steel against 425 Steel and 175 Oil.
//!
//! An enemy Tank in sight within `SHELL_TILES` of a Jeff Rifleman standing within `BASE_TILES` of a
//! Jeff Depot is charged by the free Riflemen within `SHELL_TILES` of it, when there are at least
//! `RIFLES_PER_TANK` of them for each enemy Tank near it and its infantry escort is no more than
//! `ESCORT_SHARE` of them. They all go for the Tank nearest the base, re-ordered each decision until
//! it is dead, out of sight, or `LEASH_TILES` from every Jeff Depot.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

const SHELL_TILES: f32 = 14.0;
const BASE_TILES: f32 = 20.0;
const LEASH_TILES: f32 = 26.0;
const RIFLES_PER_TANK: usize = 6;
/// Enemy Tanks this close to the target count against the charge.
const GROUP_TILES: f32 = 6.0;
/// The charge goes only while the enemy infantry near the target is at most this share of it.
const ESCORT_SHARE: f32 = 0.5;

/// V1.1's charge: the Tank charged and the Riflemen charging it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct ChargeMemory {
    pub(in crate::ai_core::decision) target: Option<u32>,
    pub(in crate::ai_core::decision) riflemen: BTreeSet<u32>,
}

/// Sends the base's Riflemen at an enemy Tank shelling it (see the module docs). `held`: units
/// other systems keep this decision. V1.1 only.
pub(in crate::ai_core::decision) fn charge_tanks(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile_id: &str,
    held: &BTreeSet<u32>,
) {
    if !is_jeff_v1_1(profile_id) || super::off("tankcharge") {
        memory.jeff_v1_1.charge = ChargeMemory::default();
        return;
    }
    let ts = observation.map.tile_size as f32;
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let near_depot = |at: (f32, f32), tiles: f32| {
        depots
            .iter()
            .any(|depot| dist2(depot.0, depot.1, at.0, at.1) <= squared(tiles * ts))
    };
    let reserved = memory.squad_reserved();
    let free: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !held.contains(&unit.id)
                && !reserved.contains(&unit.id)
                && memory.containment.active_scout != Some(unit.id)
                && !memory.containment.active_riflemen.contains(&unit.id)
                && !memory.rush_reserved(unit.id)
        })
        .collect();
    let defenders: Vec<&&AiEntitySummary> = free
        .iter()
        .filter(|unit| near_depot((unit.x, unit.y), BASE_TILES))
        .collect();
    let enemy_tanks: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .collect();
    let shelling = |tank: &AiEntitySummary| {
        near_depot((tank.x, tank.y), LEASH_TILES)
            && defenders.iter().any(|unit| {
                dist2(unit.x, unit.y, tank.x, tank.y) <= squared(SHELL_TILES * ts)
            })
    };
    // Keep the current target while it is in sight and in reach of the base, else the shelling Tank
    // nearest a Depot.
    let to_base = |tank: &AiEntitySummary| {
        depots
            .iter()
            .map(|depot| dist2(depot.0, depot.1, tank.x, tank.y))
            .fold(f32::INFINITY, f32::min)
    };
    let previous = memory.jeff_v1_1.charge.target;
    let target = previous
        .and_then(|id| enemy_tanks.iter().find(|tank| tank.id == id))
        .filter(|tank| near_depot((tank.x, tank.y), LEASH_TILES))
        .or_else(|| {
            enemy_tanks
                .iter()
                .filter(|tank| shelling(tank))
                .min_by(|a, b| to_base(a).total_cmp(&to_base(b)).then(a.id.cmp(&b.id)))
        })
        .copied();
    let Some(target) = target else {
        memory.jeff_v1_1.charge = ChargeMemory::default();
        return;
    };
    let chargers: Vec<&&AiEntitySummary> = free
        .iter()
        .filter(|unit| dist2(unit.x, unit.y, target.x, target.y) <= squared(SHELL_TILES * ts))
        .collect();
    let group = enemy_tanks
        .iter()
        .filter(|tank| dist2(tank.x, tank.y, target.x, target.y) <= squared(GROUP_TILES * ts))
        .count();
    let escort = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0)
        .filter(|enemy| {
            matches!(
                enemy.kind,
                EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::Panzerfaust
            )
        })
        .filter(|enemy| dist2(enemy.x, enemy.y, target.x, target.y) <= squared(GROUP_TILES * ts))
        .count();
    let enough = chargers.len() >= RIFLES_PER_TANK * group.max(1)
        && escort as f32 <= chargers.len() as f32 * ESCORT_SHARE;
    // A charge under way keeps going while it still outnumbers the Tanks.
    let going = previous == Some(target.id) && chargers.len() >= RIFLES_PER_TANK.div_ceil(2) * group;
    if !enough && !going {
        memory.jeff_v1_1.charge = ChargeMemory::default();
        return;
    }
    let mut sent = Vec::new();
    for unit in &chargers {
        if unit.target_id == Some(target.id) {
            continue;
        }
        if let Some(units) = actions::attack_units(actions, [unit.id], target.id) {
            sent.extend(units);
        }
    }
    memory.jeff_v1_1.charge = ChargeMemory {
        target: Some(target.id),
        riflemen: chargers.iter().map(|unit| unit.id).collect(),
    };
    if !sent.is_empty() {
        intents.push(AiIntent::Attack { units: sent });
    }
}
