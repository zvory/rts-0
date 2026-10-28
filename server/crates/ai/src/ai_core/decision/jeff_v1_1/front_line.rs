//! Jeff's AI V1.1 holds a line across the enemy's way in and walks it forward. V1.0 kept its home
//! Tanks in a ball between its bases (against Pressure AI on Classic, 3-4 Tanks 1.4-2.2 tiles apart
//! from 6:00 to 7:00) and its 35-50 Riflemen spread around the Depots 3-8 tiles forward of them,
//! then sent the Tanks out as one push.
//!
//! - **Line**: `AHEAD_TILES` in front of Jeff's frontier, across the way from its main to the enemy
//!   main: the frontier is the most forward of its Steel Mines and Pump Jacks and of the Steel and
//!   Oil at the sites it is about to take (the natural's, the next later base's), not the Depots,
//!   which stand behind them, so the line moves up as Jeff expands, and its middle sits
//!   across the bases near that front. Tanks `TANK_SPACING_TILES` apart, Riflemen `RIFLE_SPACING_TILES` apart
//!   `RIFLE_AHEAD_TILES` in front of them, each post pulled back onto open ground. Its centre leans
//!   up to `LEAN_TILES` toward the side where more enemy strength was seen lately.
//! - **Units**: every Tank no other system holds, V1.0's home Tank and the raid aside, and the
//!   Riflemen past the `HOME_RIFLES` nearest the main. Each goes to its post and holds there; a unit
//!   in a fight is left to it.
//! - **Give and take**: losing `FALL_LOSS_SHARE` of its strength over `LOSS_TICKS` or facing
//!   `FALL_RATIO` of it in sight, every `STEP_TICKS` it steps `STEP_TILES` back toward the frontier,
//!   up to `MAX_BACK_TILES`; holding with little loss and the enemy near it under `ADVANCE_RATIO`
//!   of it, it steps forward again.

use super::*;
use crate::ai_core::decision::geometry::{building_center, dist2, squared, tile_center};

const TANK_SPACING_TILES: f32 = 8.0;
const RIFLE_SPACING_TILES: f32 = 4.0;
const RIFLE_AHEAD_TILES: f32 = 3.0;
const HOME_RIFLES: usize = 6;
const MAX_RIFLES: usize = 24;
const AHEAD_TILES: f32 = 3.0;
/// Steel and Oil this close to a site being taken are that base's.
const SITE_FIELD_TILES: f32 = 10.0;
/// Depots and sites this far behind the frontier still set where the line's middle sits.
const FRONT_BAND_TILES: f32 = 25.0;
const STEP_TILES: f32 = 4.0;
const STEP_TICKS: u32 = config::TICK_HZ * 20;
const MAX_BACK_TILES: f32 = 12.0;
const LOSS_TICKS: u32 = config::TICK_HZ * 30;
const HOLD_LOSS_SHARE: f32 = 0.1;
const FALL_LOSS_SHARE: f32 = 0.3;
const ADVANCE_RATIO: f32 = 0.6;
const FALL_RATIO: f32 = 1.2;
/// Enemy fighters in sight this close to the line's centre weigh against it.
const NEAR_LINE_TILES: f32 = 18.0;
/// Most units within this of their posts: the line is settled.
const ON_POST_TILES: f32 = 3.0;
const SETTLED_SHARE: f32 = 0.6;
const LEAN_TILES: f32 = 8.0;
/// A unit with an enemy fighter this close fights where it stands.
const FIGHT_TILES: f32 = 7.0;
const REORDER_TICKS: u32 = config::TICK_HZ * 3;

/// V1.1's front line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct FrontLineMemory {
    /// How far the line stands back from `AHEAD_TILES` in front of the frontier, in tenths of a tile.
    back: u32,
    last_step: u32,
    /// Each line unit and its value, last decision.
    members: BTreeMap<u32, u32>,
    /// Value lost from the line, and when.
    losses: Vec<(u32, u32)>,
    /// When each unit was last sent to its post.
    ordered: BTreeMap<u32, u32>,
}

impl FrontLineMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.members.contains_key(&id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.members.keys().copied()
    }
}

/// Whether V1.1 holds a front line.
pub(in crate::ai_core::decision) fn active(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("frontline")
}

/// Picks, walks and posts V1.1's front line (see the module docs). `held`: units other systems keep
/// this decision.
pub(in crate::ai_core::decision) fn hold(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    held: &BTreeSet<u32>,
) {
    if !active(memory) {
        memory.jeff_v1_1.front_line = FrontLineMemory::default();
        return;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let Some(enemy_main) = super::push::enemy_main(observation) else {
        return;
    };
    let by_id: BTreeMap<u32, &AiEntitySummary> =
        observation.owned.iter().map(|unit| (unit.id, unit)).collect();
    // Losses since the last decision.
    {
        let state = &mut memory.jeff_v1_1.front_line;
        let lost: u32 = state
            .members
            .iter()
            .filter(|(id, _)| by_id.get(id).is_none_or(|unit| unit.hp == 0))
            .map(|(_, value)| *value)
            .sum();
        if lost > 0 {
            state.losses.push((tick, lost));
        }
        state.losses.retain(|(at, _)| tick.saturating_sub(*at) <= LOSS_TICKS);
    }
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let reserved = memory.squad_reserved();
    let free = |unit: &AiEntitySummary| {
        unit.is_complete
            && unit.hp > 0
            && !held.contains(&unit.id)
            && !reserved.contains(&unit.id)
            && !memory.containment.active_tanks.contains(&unit.id)
            && !memory.containment.active_riflemen.contains(&unit.id)
            && memory.containment.active_scout != Some(unit.id)
            && memory.home_defensive_tank != Some(unit.id)
            && !memory.later_bases.guards.contains(&unit.id)
            && !memory.expansion_security.riflemen.contains(&unit.id)
            && !memory.jeff_v1_1.natural.holds(unit.id)
            && !memory.jeff_v1_1.tank_raid.holds(unit.id)
            && !memory.jeff_v1_1.base_lines.posted(unit.id)
    };
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && free(unit))
        .collect();
    let mut riflemen: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && free(unit))
        .collect();
    // The Riflemen nearest the main stay home.
    riflemen.sort_by(|a, b| {
        dist2(a.x, a.y, main.0, main.1)
            .total_cmp(&dist2(b.x, b.y, main.0, main.1))
            .then(a.id.cmp(&b.id))
    });
    let riflemen: Vec<&AiEntitySummary> = riflemen
        .into_iter()
        .skip(HOME_RIFLES)
        .take(MAX_RIFLES)
        .collect();
    if tanks.is_empty() {
        // No line without Tanks: the infantry goes back to V1.0's posts.
        memory.jeff_v1_1.front_line.members.clear();
        return;
    }
    let Some(axis) = normalized_direction(main, enemy_main) else {
        return;
    };
    let across = (-axis.1, axis.0);
    // The frontier: Jeff's extractors, and the Steel and Oil at the sites it is about to take.
    let mut anchors: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| matches!(unit.kind, EntityKind::SteelMine | EntityKind::PumpJack))
        .filter(|unit| unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let sites: Vec<(f32, f32)> = [memory.expansion_security.site, memory.later_bases.site()]
        .into_iter()
        .flatten()
        .filter_map(|site| building_center(site, EntityKind::ResourceDepot, observation.map.tile_size))
        .collect();
    anchors.extend(
        observation
            .resources
            .iter()
            .filter(|node| matches!(node.kind, EntityKind::Steel | EntityKind::Oil))
            .filter(|node| {
                sites.iter().any(|site| {
                    dist2(node.x, node.y, site.0, site.1) <= squared(SITE_FIELD_TILES * ts)
                })
            })
            .map(|node| (node.x, node.y)),
    );
    let depth = |at: (f32, f32)| (at.0 - main.0) * axis.0 + (at.1 - main.1) * axis.1;
    let lateral = |at: (f32, f32)| (at.0 - main.0) * across.0 + (at.1 - main.1) * across.1;
    let frontier = anchors.iter().map(|at| depth(*at)).fold(f32::MIN, f32::max);
    if frontier == f32::MIN {
        return;
    }
    let front: Vec<&(f32, f32)> = anchors
        .iter()
        .filter(|at| depth(**at) >= frontier - FRONT_BAND_TILES * ts)
        .collect();
    let middle = front.iter().map(|at| lateral(**at)).sum::<f32>() / front.len().max(1) as f32;
    let strength: f32 = tanks
        .iter()
        .chain(riflemen.iter())
        .map(|unit| super::push::unit_value(unit.kind) as f32)
        .sum();
    // Walk the line.
    {
        let state = &mut memory.jeff_v1_1.front_line;
        let line_depth = frontier + (AHEAD_TILES - state.back as f32 / 10.0) * ts;
        let centre = (
            main.0 + axis.0 * line_depth + across.0 * middle,
            main.1 + axis.1 * line_depth + across.1 * middle,
        );
        let enemy_near: f32 = observation
            .visible_enemies
            .iter()
            .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
            .filter(|enemy| {
                dist2(enemy.x, enemy.y, centre.0, centre.1) <= squared(NEAR_LINE_TILES * ts)
            })
            .map(|enemy| super::push::unit_value(enemy.kind) as f32)
            .sum();
        let lost: f32 = state.losses.iter().map(|(_, value)| *value as f32).sum();
        let on_post = state
            .members
            .keys()
            .filter(|id| by_id.get(id).is_some_and(|unit| unit.state != AiEntityState::Move))
            .count();
        let settled = !state.members.is_empty()
            && on_post as f32 >= state.members.len() as f32 * SETTLED_SHARE;
        if tick.saturating_sub(state.last_step) >= STEP_TICKS {
            if lost >= strength * FALL_LOSS_SHARE || enemy_near >= strength * FALL_RATIO {
                state.back = (state.back as f32 + STEP_TILES * 10.0).min(MAX_BACK_TILES * 10.0) as u32;
                state.last_step = tick;
                state.losses.clear();
            } else if settled
                && lost <= strength * HOLD_LOSS_SHARE
                && enemy_near <= strength * ADVANCE_RATIO
            {
                state.back = (state.back as f32 - STEP_TILES * 10.0).max(0.0) as u32;
                state.last_step = tick;
            }
        }
    }
    let line_depth = frontier + (AHEAD_TILES - memory.jeff_v1_1.front_line.back as f32 / 10.0) * ts;
    let direction = axis;
    let centre = (
        main.0 + axis.0 * line_depth + across.0 * middle,
        main.1 + axis.1 * line_depth + across.1 * middle,
    );
    // Lean toward the side where more enemy strength was seen lately.
    let side_threat = |side: f32| {
        super::push::threat_near(
            observation,
            memory,
            (
                centre.0 + across.0 * side * LEAN_TILES * ts,
                centre.1 + across.1 * side * LEAN_TILES * ts,
            ),
            12.0,
        )
    };
    let (left, right) = (side_threat(-1.0), side_threat(1.0));
    let lean = if left > right * 1.5 {
        -LEAN_TILES
    } else if right > left * 1.5 {
        LEAN_TILES
    } else {
        0.0
    };
    let centre = clamp_to_map(
        (centre.0 + across.0 * lean * ts, centre.1 + across.1 * lean * ts),
        observation.map,
    );
    let rifle_centre = (
        centre.0 + direction.0 * RIFLE_AHEAD_TILES * ts,
        centre.1 + direction.1 * RIFLE_AHEAD_TILES * ts,
    );
    let tank_posts = posts(observation, map_analysis, centre, direction, tanks.len(), TANK_SPACING_TILES);
    let rifle_posts = posts(observation, map_analysis, rifle_centre, direction, riflemen.len(), RIFLE_SPACING_TILES);
    let charging = memory.jeff_v1_1.charge.riflemen.clone();
    let state = &mut memory.jeff_v1_1.front_line;
    state.members = tanks
        .iter()
        .chain(riflemen.iter())
        .map(|unit| (unit.id, super::push::unit_value(unit.kind)))
        .collect();
    state.ordered.retain(|id, _| state.members.contains_key(id));
    let mut sent = Vec::new();
    for (group, slots) in [(&tanks, &tank_posts), (&riflemen, &rifle_posts)] {
        for (unit, post) in seat(group, slots, across) {
            if charging.contains(&unit.id) {
                continue;
            }
            let fighting = observation.visible_enemies.iter().any(|enemy| {
                enemy.hp > 0
                    && enemy.kind.is_unit()
                    && enemy.kind != EntityKind::Worker
                    && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(FIGHT_TILES * ts)
            });
            if fighting {
                continue;
            }
            let near = dist2(unit.x, unit.y, post.0, post.1) <= squared(1.5 * ts);
            if near {
                if unit.state == AiEntityState::Move {
                    actions::hold_position_units(actions, [unit.id]);
                }
                continue;
            }
            if state
                .ordered
                .get(&unit.id)
                .is_some_and(|at| tick.saturating_sub(*at) < REORDER_TICKS)
            {
                continue;
            }
            actions::attack_move_units(actions, [unit.id], post.0, post.1);
            state.ordered.insert(unit.id, tick);
            sent.push(unit.id);
        }
    }
    if !sent.is_empty() {
        intents.push(AiIntent::Assemble { units: sent });
    }
}

/// `count` posts across `direction` through `centre`, `spacing` tiles apart, each pulled back
/// toward the line's own side until the ground is open.
fn posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    centre: (f32, f32),
    direction: (f32, f32),
    count: usize,
    spacing: f32,
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let across = (-direction.1, direction.0);
    (0..count)
        .map(|index| {
            let side = (index as f32 - (count as f32 - 1.0) * 0.5) * spacing;
            let mut back = 0.0;
            loop {
                let point = clamp_to_map(
                    (
                        centre.0 + (across.0 * side - direction.0 * back) * ts,
                        centre.1 + (across.1 * side - direction.1 * back) * ts,
                    ),
                    observation.map,
                );
                if back >= 6.0
                    || defense::defensive_position_is_open(observation, map_analysis, point.0, point.1)
                {
                    break point;
                }
                back += 1.0;
            }
        })
        .collect()
}

/// Units to posts, both ordered across the line so none crosses another.
fn seat<'u>(
    units: &[&'u AiEntitySummary],
    posts: &[(f32, f32)],
    across: (f32, f32),
) -> Vec<(&'u AiEntitySummary, (f32, f32))> {
    let lateral = |x: f32, y: f32| x * across.0 + y * across.1;
    let mut units = units.to_vec();
    units.sort_by(|a, b| lateral(a.x, a.y).total_cmp(&lateral(b.x, b.y)).then(a.id.cmp(&b.id)));
    let mut posts = posts.to_vec();
    posts.sort_by(|a, b| lateral(a.0, a.1).total_cmp(&lateral(b.0, b.1)));
    units.into_iter().zip(posts).collect()
}
