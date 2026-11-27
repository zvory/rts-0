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
/// `chargeesc`: no charge at a Tank with this much infantry near it, and a charge under way is
/// called back when its escort grows to it. Bench vs Pressure AI (80 games, 4:00-10:00), Riflemen
/// lost per Tank killed: escort 4+ from 6-10 tiles 20-29, escort grown during the charge 33 for 1.
const MAX_ESCORT: usize = 4;
/// `chargeesc`: with this much infantry near the Tank, only the Riflemen this close start a charge:
/// from farther the Tank shells them all the way in. Escort 2-3: 7 lost per Tank from under 8
/// tiles, 30 for 3 Tanks from farther; escort 0-1: 1.2-2.6 per Tank out to 10 tiles.
const CLOSE_ESCORT: usize = 2;
const CLOSE_TILES: f32 = 8.0;
/// `chargeesc`: escort infantry seen this recently counts though out of sight; the Tank fires from
/// out of the Riflemen's sight and its escort stays hidden until they are in among it.
const ESCORT_MEMORY_TICKS: u32 = config::TICK_HZ * 20;

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
    // `chargeesc`: a breakout sortie fights its own fight; charging its Riflemen at one Tank swapped
    // their orders every few ticks while Panzerfausts shot them.
    let escort_rule = !super::off("chargeesc");
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
                && !(escort_rule && memory.jeff_v1_1.breakout.holds(unit.id))
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
            && defenders
                .iter()
                .any(|unit| dist2(unit.x, unit.y, tank.x, tank.y) <= squared(SHELL_TILES * ts))
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
    let mut chargers: Vec<&&AiEntitySummary> = free
        .iter()
        .filter(|unit| dist2(unit.x, unit.y, target.x, target.y) <= squared(SHELL_TILES * ts))
        .collect();
    let group = enemy_tanks
        .iter()
        .filter(|tank| dist2(tank.x, tank.y, target.x, target.y) <= squared(GROUP_TILES * ts))
        .count();
    let infantry = |kind: EntityKind| {
        matches!(
            kind,
            EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::Panzerfaust
        )
    };
    let near_target = |x: f32, y: f32| dist2(x, y, target.x, target.y) <= squared(GROUP_TILES * ts);
    let seen_escort: BTreeSet<u32> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && infantry(enemy.kind) && near_target(enemy.x, enemy.y))
        .map(|enemy| enemy.id)
        .collect();
    let escort = seen_escort.len();
    // `chargeesc`: with the infantry remembered near it too.
    let full_escort = if escort_rule {
        let tick = observation.tick;
        escort
            + memory
                .jeff_v1_1
                .push
                .remembered()
                .filter(|(id, kind, _, at, seen)| {
                    !seen_escort.contains(id)
                        && infantry(*kind)
                        && tick.saturating_sub(*seen) <= ESCORT_MEMORY_TICKS
                        && near_target(at.0, at.1)
                })
                .count()
    } else {
        escort
    };
    let crowded = escort_rule && full_escort >= MAX_ESCORT;
    // `chargeesc`: an escorted Tank is charged only by the Riflemen already close to it.
    if escort_rule && previous != Some(target.id) && full_escort >= CLOSE_ESCORT {
        chargers
            .retain(|unit| dist2(unit.x, unit.y, target.x, target.y) <= squared(CLOSE_TILES * ts));
    }
    let enough = chargers.len() >= RIFLES_PER_TANK * group.max(1)
        && escort as f32 <= chargers.len() as f32 * ESCORT_SHARE
        && !crowded;
    // A charge under way keeps going while it still outnumbers the Tanks.
    let going = previous == Some(target.id)
        && chargers.len() >= RIFLES_PER_TANK.div_ceil(2) * group
        && !crowded;
    if !enough && !going {
        if crowded && previous == Some(target.id) {
            // `chargeesc`: call the charge back to the nearest Depot, and let the stations and
            // the home staging place its Riflemen again (their attack order otherwise keeps them
            // out of staging for a while).
            let back: Vec<u32> = memory
                .jeff_v1_1
                .charge
                .riflemen
                .iter()
                .copied()
                .filter(|id| {
                    observation
                        .owned
                        .iter()
                        .any(|unit| unit.id == *id && unit.hp > 0)
                })
                .collect();
            for id in &back {
                let Some(unit) = observation.owned.iter().find(|unit| unit.id == *id) else {
                    continue;
                };
                let home = depots
                    .iter()
                    .min_by(|a, b| {
                        dist2(a.0, a.1, unit.x, unit.y).total_cmp(&dist2(b.0, b.1, unit.x, unit.y))
                    })
                    .copied();
                if let Some(home) = home {
                    let _ = actions::move_units(actions, [*id], home.0, home.1);
                }
            }
            if !back.is_empty() {
                intents.push(AiIntent::Assemble { units: back });
            }
        }
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
