//! Jeff's AI V1.1 keeps Tanks parked in front of its main (`homeguard`). The main's Depot has 180
//! HP, three Tank shells: against Pressure AI, 4 of the 10 games lost with the Tank line (7:17-8:18)
//! ended with 3-4 enemy Tanks reaching the main and shooting its Depot down in 10-20 s, while 3-4 of
//! Jeff's Tanks stood on the line at the forward bases and 0-2 at home. V1.0's home Tank comes only
//! once its push has gone out.
//!
//! From `MIN_TANKS` Tanks on, `GUARDS` of them (one below `TWO_GUARDS_FROM`, and one fewer while
//! V1.0 has its home Tank) stand on the main's base-line Tank posts, behind the line in front of its
//! mines toward where the enemy comes, parked on hold position: a parked Tank's 14 tiles reach the
//! Depot and whatever comes at it. No other system takes them. A guard goes back once more than
//! `OFF_POST_TILES` off its post, but with an enemy Tank within `ENGAGED_TILES` only past
//! `LEASH_TILES`: moving drops its reach to 5 tiles.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared, tile_center};

const MIN_TANKS: usize = 3;
const TWO_GUARDS_FROM: usize = 4;
const GUARDS: usize = 2;
const OFF_POST_TILES: f32 = 2.5;
const ENGAGED_TILES: f32 = 15.0;
const LEASH_TILES: f32 = 8.0;
/// Without a base line at the main: this far from its Depot toward the enemy main.
const OUT_TILES: f32 = 5.0;
const SPACING_TILES: f32 = 2.5;
const REORDER_TICKS: u32 = config::TICK_HZ * 3;

/// V1.1's home guard: each guard and when it was last sent to its post.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct HomeGuardMemory {
    guards: BTreeMap<u32, u32>,
}

impl HomeGuardMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.guards.contains_key(&id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.guards.keys().copied()
    }
}

/// Picks and posts V1.1's home guard (see the module docs). `held`: units other systems keep this
/// decision.
pub(in crate::ai_core::decision) fn guard(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    held: &BTreeSet<u32>,
) {
    if !memory.profile_id().is_some_and(is_jeff_v1_1) || super::off("homeguard") {
        memory.jeff_v1_1.home_guard = HomeGuardMemory::default();
        return;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .collect();
    let wanted = match tanks.len() {
        n if n < MIN_TANKS => 0,
        n if n < TWO_GUARDS_FROM => 1,
        _ => GUARDS,
    }
    .saturating_sub(usize::from(memory.home_defensive_tank.is_some()));
    let eligible = |unit: &AiEntitySummary| {
        !held.contains(&unit.id)
            && !memory.containment.active_tanks.contains(&unit.id)
            && !memory.containment.stuck_tanks.contains_key(&unit.id)
            && memory.home_defensive_tank != Some(unit.id)
            && !memory.trap_breach.is_reserved(unit.id)
            && !memory.third_pressure.is_reserved(unit.id)
            && !memory.jeff_v1_1.tank_raid.holds(unit.id)
            && !memory.jeff_v1_1.front_line.yielded(unit.id)
            // A later base's guard keeps its post (`tug2`): taken by both, it drove between them.
            && (super::off("tug2") || !memory.later_bases.guards.contains(&unit.id))
    };
    // The guards keep their place; new ones are the Tanks nearest the main.
    let mut picked: Vec<&AiEntitySummary> = tanks
        .iter()
        .copied()
        .filter(|unit| memory.jeff_v1_1.home_guard.holds(unit.id) && eligible(unit))
        .collect();
    picked.sort_by_key(|unit| unit.id);
    picked.truncate(wanted);
    let mut rest: Vec<&AiEntitySummary> = tanks
        .iter()
        .copied()
        .filter(|unit| !picked.iter().any(|guard| guard.id == unit.id) && eligible(unit))
        .collect();
    rest.sort_by(|a, b| {
        dist2(a.x, a.y, main.0, main.1)
            .total_cmp(&dist2(b.x, b.y, main.0, main.1))
            .then(a.id.cmp(&b.id))
    });
    picked.extend(rest.into_iter().take(wanted.saturating_sub(picked.len())));
    if picked.is_empty() {
        memory.jeff_v1_1.home_guard.guards.clear();
        return;
    }
    // The main's base-line Tank posts, or posts straight out toward the enemy main.
    let line = super::base_line::base_lines(observation, map_analysis, &memory.jeff_v1_1.base_lines.contacts)
        .into_iter()
        .find(|line| line.main);
    let wanted_points = match &line {
        Some(line) => line.tank_points(picked.len(), ts),
        None => {
            let Some(axis) = super::push::enemy_main(observation)
                .and_then(|enemy| normalized_direction(main, enemy))
            else {
                return;
            };
            let across = (-axis.1, axis.0);
            (0..picked.len())
                .map(|index| {
                    let side = (index as f32 - (picked.len() as f32 - 1.0) * 0.5) * SPACING_TILES;
                    (
                        main.0 + (axis.0 * OUT_TILES + across.0 * side) * ts,
                        main.1 + (axis.1 * OUT_TILES + across.1 * side) * ts,
                    )
                })
                .collect()
        }
    };
    let points: Vec<(f32, f32)> = wanted_points
        .into_iter()
        .map(|point| {
            let point = clamp_to_map(point, observation.map);
            defense::clear_mobile_defensive_position(observation, map_analysis, point).unwrap_or(point)
        })
        .collect();
    // Guards to posts, nearest first.
    let mut seated: Vec<(&AiEntitySummary, (f32, f32))> = Vec::new();
    let mut open: Vec<&AiEntitySummary> = picked.clone();
    for point in points {
        let Some(index) = (0..open.len()).min_by(|a, b| {
            dist2(open[*a].x, open[*a].y, point.0, point.1)
                .total_cmp(&dist2(open[*b].x, open[*b].y, point.0, point.1))
        }) else {
            break;
        };
        seated.push((open.remove(index), point));
    }
    let engaged = |unit: &AiEntitySummary| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.kind == EntityKind::Tank
                && enemy.hp > 0
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(ENGAGED_TILES * ts)
        })
    };
    let previous = std::mem::take(&mut memory.jeff_v1_1.home_guard.guards);
    let mut sent = Vec::new();
    for (unit, post) in seated {
        let mut ordered = previous.get(&unit.id).copied().unwrap_or(0);
        let off = dist2(unit.x, unit.y, post.0, post.1);
        if off <= squared(OFF_POST_TILES * ts) {
            if unit.state == AiEntityState::Move {
                actions::hold_position_units(actions, [unit.id]);
            }
        } else if (!engaged(unit) || off > squared(LEASH_TILES * ts))
            && tick.saturating_sub(ordered) >= REORDER_TICKS
        {
            actions::move_units(actions, [unit.id], post.0, post.1);
            ordered = tick;
            sent.push(unit.id);
        }
        memory.jeff_v1_1.home_guard.guards.insert(unit.id, ordered);
    }
    if !sent.is_empty() {
        intents.push(AiIntent::Assemble { units: sent });
    }
}
