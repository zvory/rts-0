//! Jeff's AI V1.1 keeps its free Riflemen as one roaming force (`rovers`). Meth Riflemen move at
//! Tank speed and fire on the move, so they can run to a base under attack, while the guns, which
//! fire only set up, hold the bases' trenches (`basemgs`). Posted 3 tiles in front of the Tank line
//! instead, V1.1 lost 60 Riflemen and guns a game against Pressure AI (V1.0 46), half of them with
//! an enemy Tank within 8 tiles, while its outer bases fell to squads of 1-8 Panzerfausts with no
//! defender near.
//!
//! - **Force**: the Riflemen past the `HOME_RIFLES` nearest the main that no other system holds, up
//!   to `MAX_ROVERS`.
//! - **Station**: between Jeff's bases, at the middle of its Depots pulled `STATION_BACK_TILES`
//!   toward the main (`STATION_OUT_TILES` out from the main toward the enemy while it has no other),
//!   `SPACING_TILES` apart.
//! - **Response**: enemy fighters in sight within `ALARM_TILES` of a Jeff Depot or extractor are a
//!   raid on that base. The force runs at the biggest raid it can take, enemy Tanks there no more
//!   than Jeff's Tanks near it or one to `RIFLES_PER_TANK` rovers, attack-moving at the raid's
//!   centre, sent again every `REORDER_TICKS`, until no raid has been in sight for `CLEAR_TICKS`.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared, tile_center};

const HOME_RIFLES: usize = 6;
const MAX_ROVERS: usize = 24;
const STATION_BACK_TILES: f32 = 4.0;
const STATION_OUT_TILES: f32 = 8.0;
const SPACING_TILES: f32 = 1.75;
const PER_ROW: usize = 5;
const ALARM_TILES: f32 = 14.0;
/// Jeff Tanks this close to a raid fight it with the force.
const SUPPORT_TILES: f32 = 14.0;
const RIFLES_PER_TANK: usize = 6;
const REORDER_TICKS: u32 = config::TICK_HZ * 2;
const STATION_REORDER_TICKS: u32 = config::TICK_HZ * 4;
const CLEAR_TICKS: u32 = config::TICK_HZ * 5;
/// A rover this close to its station point stands.
const ON_STATION_TILES: f32 = 2.5;
/// A rover with an enemy fighter this close fights where it stands.
const FIGHT_TILES: f32 = 7.0;

/// V1.1's roaming Riflemen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct RoverMemory {
    /// Each rover and when it was last ordered.
    rovers: BTreeMap<u32, u32>,
    /// The raid the force is running at, and when it was last in sight.
    raid: Option<((i32, i32), u32)>,
}

impl RoverMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.rovers.contains_key(&id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.rovers.keys().copied()
    }
}

/// Whether V1.1 keeps a roaming force.
pub(in crate::ai_core::decision) fn active(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("rovers")
}

/// Stations V1.1's roaming Riflemen and sends them at raids on its bases (see the module docs).
/// `held`: units other systems keep this decision.
pub(in crate::ai_core::decision) fn roam(
    (actions, observation, _map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    held: &BTreeSet<u32>,
) {
    if !active(memory) {
        memory.jeff_v1_1.rovers = RoverMemory::default();
        return;
    }
    // The opening's Riflemen are V1.0's until the first Tank: roaming from the start, the force left
    // the main to Rush AI's and Pressure AI's first waves (6 games lost at 3:08-4:44, none without).
    if !observation
        .owned
        .iter()
        .any(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
    {
        memory.jeff_v1_1.rovers = RoverMemory::default();
        return;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let reserved = memory.squad_reserved();
    let mut riflemen: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !held.contains(&unit.id)
                && !reserved.contains(&unit.id)
                && !memory.containment.active_riflemen.contains(&unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.later_bases.guards.contains(&unit.id)
                && !memory.rush_reserved(unit.id)
                && memory.route_line.picket() != Some(unit.id)
                && !memory.jeff_v1_1.natural.holds(unit.id)
                && !memory.jeff_v1_1.deny.holds(unit.id)
                && !memory.jeff_v1_1.front_line.holds(unit.id)
                && !memory.jeff_v1_1.base_lines.posted(unit.id)
                && !memory.jeff_v1_1.strike.party.contains(&unit.id)
                && !memory.jeff_v1_1.hunt.hunters.contains_key(&unit.id)
        })
        .collect();
    riflemen.sort_by(|a, b| {
        dist2(a.x, a.y, main.0, main.1)
            .total_cmp(&dist2(b.x, b.y, main.0, main.1))
            .then(a.id.cmp(&b.id))
    });
    let rovers: Vec<&AiEntitySummary> = riflemen
        .into_iter()
        .skip(HOME_RIFLES)
        .take(MAX_ROVERS)
        .collect();
    let previous = std::mem::take(&mut memory.jeff_v1_1.rovers.rovers);
    if rovers.is_empty() {
        memory.jeff_v1_1.rovers.raid = None;
        return;
    }
    // Raids: enemy fighters in sight near a Depot or extractor, by the nearest Depot.
    let depots: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .collect();
    let sites: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| {
            unit.hp > 0
                && matches!(
                    unit.kind,
                    EntityKind::ResourceDepot | EntityKind::SteelMine | EntityKind::PumpJack
                )
        })
        .map(|unit| (unit.x, unit.y))
        .collect();
    let mut raids: BTreeMap<u32, Vec<&AiEntitySummary>> = BTreeMap::new();
    for enemy in observation.visible_enemies.iter().filter(|enemy| {
        enemy.hp > 0
            && enemy.kind.is_unit()
            && !matches!(enemy.kind, EntityKind::Worker | EntityKind::Golem)
    }) {
        if !sites
            .iter()
            .any(|site| dist2(enemy.x, enemy.y, site.0, site.1) <= squared(ALARM_TILES * ts))
        {
            continue;
        }
        let Some(depot) = depots.iter().min_by(|a, b| {
            dist2(a.x, a.y, enemy.x, enemy.y).total_cmp(&dist2(b.x, b.y, enemy.x, enemy.y))
        }) else {
            continue;
        };
        raids.entry(depot.id).or_default().push(enemy);
    }
    let jeff_tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .collect();
    let target = raids
        .values()
        .filter_map(|raiders| {
            let n = raiders.len() as f32;
            let centre = (
                raiders.iter().map(|enemy| enemy.x).sum::<f32>() / n,
                raiders.iter().map(|enemy| enemy.y).sum::<f32>() / n,
            );
            let enemy_tanks = raiders.iter().filter(|enemy| enemy.kind == EntityKind::Tank).count();
            let support = jeff_tanks
                .iter()
                .filter(|tank| {
                    dist2(tank.x, tank.y, centre.0, centre.1) <= squared(SUPPORT_TILES * ts)
                })
                .count();
            let takes = enemy_tanks <= support || enemy_tanks * RIFLES_PER_TANK <= rovers.len();
            let value: u32 = raiders.iter().map(|enemy| super::push::unit_value(enemy.kind)).sum();
            takes.then_some((value, centre))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, centre)| centre);
    let state = &mut memory.jeff_v1_1.rovers;
    if let Some(at) = target {
        state.raid = Some(((at.0.round() as i32, at.1.round() as i32), tick));
    }
    let raid = state
        .raid
        .filter(|(_, seen)| tick.saturating_sub(*seen) <= CLEAR_TICKS)
        .map(|((x, y), _)| (x as f32, y as f32));
    if raid.is_none() {
        state.raid = None;
    }
    let fighting = |unit: &AiEntitySummary| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && enemy.kind != EntityKind::Worker
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(FIGHT_TILES * ts)
        })
    };
    let mut sent = Vec::new();
    if let Some(at) = raid {
        for unit in &rovers {
            let mut ordered = previous.get(&unit.id).copied().unwrap_or(0);
            if !fighting(unit) && tick.saturating_sub(ordered) >= REORDER_TICKS {
                actions::attack_move_units(actions, [unit.id], at.0, at.1);
                ordered = tick;
                sent.push(unit.id);
            }
            memory.jeff_v1_1.rovers.rovers.insert(unit.id, ordered);
        }
    } else {
        // The station: the middle of the bases past the main, pulled back toward it.
        let outer: Vec<&&AiEntitySummary> = depots
            .iter()
            .filter(|depot| dist2(depot.x, depot.y, main.0, main.1) > squared(8.0 * ts))
            .collect();
        let station = if outer.is_empty() {
            let Some(axis) = super::push::enemy_main(observation)
                .and_then(|enemy| normalized_direction(main, enemy))
            else {
                return;
            };
            (main.0 + axis.0 * STATION_OUT_TILES * ts, main.1 + axis.1 * STATION_OUT_TILES * ts)
        } else {
            let n = outer.len() as f32 + 1.0;
            let middle = (
                (outer.iter().map(|depot| depot.x).sum::<f32>() + main.0) / n,
                (outer.iter().map(|depot| depot.y).sum::<f32>() + main.1) / n,
            );
            match normalized_direction(middle, main) {
                Some(back) => (
                    middle.0 + back.0 * STATION_BACK_TILES * ts,
                    middle.1 + back.1 * STATION_BACK_TILES * ts,
                ),
                None => middle,
            }
        };
        let station = clamp_to_map(station, observation.map);
        let mut ordered_rovers: Vec<&AiEntitySummary> = rovers.clone();
        ordered_rovers.sort_by_key(|unit| unit.id);
        for (index, unit) in ordered_rovers.iter().enumerate() {
            let (row, slot) = (index / PER_ROW, index % PER_ROW);
            let point = clamp_to_map(
                (
                    station.0 + (slot as f32 - (PER_ROW as f32 - 1.0) * 0.5) * SPACING_TILES * ts,
                    station.1 + row as f32 * SPACING_TILES * ts,
                ),
                observation.map,
            );
            let mut ordered = previous.get(&unit.id).copied().unwrap_or(0);
            let on = dist2(unit.x, unit.y, point.0, point.1) <= squared(ON_STATION_TILES * ts);
            if !on && !fighting(unit) && tick.saturating_sub(ordered) >= STATION_REORDER_TICKS {
                actions::attack_move_units(actions, [unit.id], point.0, point.1);
                ordered = tick;
                sent.push(unit.id);
            }
            memory.jeff_v1_1.rovers.rovers.insert(unit.id, ordered);
        }
    }
    if !sent.is_empty() {
        intents.push(AiIntent::Assemble { units: sent });
    }
}
