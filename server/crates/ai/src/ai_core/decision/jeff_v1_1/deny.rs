//! Jeff's AI V1.1 keeps a raided base down. A Depot's extractors decay to nothing within 20 s of it
//! (#1707), but against Pressure AI the raid's dead Depots stood again 29-63 s later, the base
//! then mined again once its extractors were rebuilt. Riflemen do full damage to an Engineer (40
//! HP, eight hits) and to a foundation going up, and cost 35 Steel.
//!
//! `SQUAD` free Riflemen ride out with the raid, `RIDE_BACK_TILES` behind its Tanks (Meth Riflemen
//! move as fast as Tanks): sent out alone after the kill, they met the enemy on the way and turned
//! back. When the raid kills a Depot, they take a post `POST_TILES` short of the site on Jeff's
//! side as the Tanks leave, and wait there until `WATCH_TICKS` after the kill, shooting any enemy
//! Engineer or unfinished building within `WATCH_TILES` of the site. Enemy fighters near them
//! worth `OUTMATCHED` times more than they are (four turned back at two enemy Riflemen), they go
//! home. One site at a time, the latest kill first.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared, tile_center};

const SQUAD: usize = 6;
/// The squad goes home when enemy fighters near it are worth this many times more than it.
const OUTMATCHED: f32 = 1.5;
const POST_TILES: f32 = 5.0;
const WATCH_TILES: f32 = 9.0;
const WATCH_TICKS: u32 = config::TICK_HZ * 120;
/// Enemy fighters this close to the squad weigh against it.
const DANGER_TILES: f32 = 10.0;
/// Riflemen this far from every Jeff Depot are out on the map.
const OUT_TILES: f32 = 20.0;
const REORDER_TICKS: u32 = config::TICK_HZ * 2;
const RIDE_BACK_TILES: f32 = 3.0;
/// Riflemen this close to the main's Depot are the ones sent.
const HOME_TILES: f32 = 30.0;

/// The site being watched and until when, and the squad watching it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct DenyMemory {
    site: Option<((i32, i32), u32)>,
    squad: BTreeSet<u32>,
    ordered: u32,
    /// Why the last watch ended, for the decision log: (tick, danger, strength, over).
    ended: Option<(u32, u32, u32, bool)>,
}

impl DenyMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.squad.contains(&id)
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.squad.iter().copied()
    }

    /// For the decision log.
    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        format!("{:?}/{:?} ended={:?}", self.site, self.squad, self.ended)
    }
}

/// Notes a Depot the raid killed at `at`.
pub(in crate::ai_core::decision) fn note_kill(
    memory: &mut AiDecisionMemory,
    at: (f32, f32),
    tick: u32,
) {
    if memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("deny") {
        let state = &mut memory.jeff_v1_1.deny;
        state.site = Some((
            (at.0.round() as i32, at.1.round() as i32),
            tick + WATCH_TICKS,
        ));
        state.ordered = 0;
    }
}

/// Sends, posts and brings home the squad watching a raided site (see the module docs). `held`:
/// units other systems keep this decision.
pub(in crate::ai_core::decision) fn watch(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    held: &BTreeSet<u32>,
) {
    if !memory.profile_id().is_some_and(is_jeff_v1_1) || super::off("deny") {
        memory.jeff_v1_1.deny = DenyMemory::default();
        return;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let Some((site, until)) = memory.jeff_v1_1.deny.site else {
        ride_with_raid(actions, observation, memory, intents, held);
        return;
    };
    let site = (site.0 as f32, site.1 as f32);
    let alive = |id: &u32| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == *id && unit.hp > 0)
    };
    memory.jeff_v1_1.deny.squad.retain(|id| alive(id));
    let squad: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| memory.jeff_v1_1.deny.holds(unit.id))
        .collect();
    let strength: f32 = squad
        .iter()
        .map(|unit| super::push::unit_value(unit.kind) as f32)
        .sum();
    // Only the Riflemen out of the bases weigh the enemy: those still at home are the bases'
    // defence's to answer, and an enemy Tank near the main turned every squad back at once.
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let out: Vec<&&AiEntitySummary> = squad
        .iter()
        .filter(|unit| {
            depots
                .iter()
                .all(|depot| dist2(unit.x, unit.y, depot.0, depot.1) > squared(OUT_TILES * ts))
        })
        .collect();
    let danger: f32 = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| {
            out.iter()
                .any(|unit| dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(DANGER_TILES * ts))
        })
        .map(|enemy| super::push::unit_value(enemy.kind) as f32)
        .sum();
    // Over, or outmatched: home, and the Riflemen go back to the bases.
    if tick >= until || (!squad.is_empty() && danger > strength * OUTMATCHED) {
        let home = tile_center(observation.own_start_tile, observation.map.tile_size);
        let ids: Vec<u32> = squad.iter().map(|unit| unit.id).collect();
        if !ids.is_empty() {
            actions::move_units(actions, ids.iter().copied(), home.0, home.1);
            intents.push(AiIntent::Assemble { units: ids });
        }
        memory.jeff_v1_1.deny = DenyMemory {
            ended: Some((tick, danger as u32, strength as u32, tick >= until)),
            ..DenyMemory::default()
        };
        return;
    }
    // Make up the squad from the free Riflemen at the main.
    if memory.jeff_v1_1.deny.squad.len() < SQUAD {
        let main = tile_center(observation.own_start_tile, observation.map.tile_size);
        let reserved = memory.squad_reserved();
        let mut free: Vec<&AiEntitySummary> = observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
            .filter(|unit| {
                !held.contains(&unit.id)
                    && !reserved.contains(&unit.id)
                    && !excluded(memory, unit.id)
                    && dist2(unit.x, unit.y, main.0, main.1) <= squared(HOME_TILES * ts)
            })
            .collect();
        free.sort_by(|a, b| {
            dist2(a.x, a.y, site.0, site.1)
                .total_cmp(&dist2(b.x, b.y, site.0, site.1))
                .then(a.id.cmp(&b.id))
        });
        let need = SQUAD - memory.jeff_v1_1.deny.squad.len();
        let picked: Vec<u32> = free.iter().take(need).map(|unit| unit.id).collect();
        memory.jeff_v1_1.deny.squad.extend(picked);
    }
    let state = &mut memory.jeff_v1_1.deny;
    if tick.saturating_sub(state.ordered) < REORDER_TICKS || state.squad.is_empty() {
        return;
    }
    state.ordered = tick;
    let ids: Vec<u32> = state.squad.iter().copied().collect();
    // An Engineer or a foundation at the site: shoot it.
    let quarry = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0)
        .filter(|enemy| {
            enemy.kind == EntityKind::Worker || (enemy.kind.is_building() && !enemy.is_complete)
        })
        .filter(|enemy| dist2(enemy.x, enemy.y, site.0, site.1) <= squared(WATCH_TILES * ts))
        .min_by(|a, b| {
            let rank = |e: &AiEntitySummary| u8::from(e.kind != EntityKind::Worker);
            rank(a)
                .cmp(&rank(b))
                .then(dist2(a.x, a.y, site.0, site.1).total_cmp(&dist2(b.x, b.y, site.0, site.1)))
                .then(a.id.cmp(&b.id))
        });
    if let Some(quarry) = quarry {
        if let Some(units) = actions::attack_units(actions, ids.iter().copied(), quarry.id) {
            intents.push(AiIntent::Attack { units });
        }
        return;
    }
    // Otherwise wait at the post, short of the site on Jeff's side.
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let back = normalized_direction(site, main).unwrap_or((0.0, 0.0));
    let post = clamp_to_map(
        (
            site.0 + back.0 * POST_TILES * ts,
            site.1 + back.1 * POST_TILES * ts,
        ),
        observation.map,
    );
    let away: Vec<u32> = ids
        .iter()
        .copied()
        .filter(|id| {
            observation.owned.iter().any(|unit| {
                unit.id == *id && dist2(unit.x, unit.y, post.0, post.1) > squared(2.0 * ts)
            })
        })
        .collect();
    if !away.is_empty() {
        actions::attack_move_units(actions, away.iter().copied(), post.0, post.1);
        intents.push(AiIntent::Assemble { units: away });
    }
}

/// Riflemen another V1.1 or V1.0 job holds.
fn excluded(memory: &AiDecisionMemory, id: u32) -> bool {
    memory.expansion_security.riflemen.contains(&id)
        || memory.containment.active_riflemen.contains(&id)
        || memory.jeff_v1_1.natural.holds(id)
        || memory.jeff_v1_1.front_line.holds(id)
        || memory.jeff_v1_1.base_lines.posted(id)
        || memory.jeff_v1_1.stations.posted_out(id)
        || memory.jeff_v1_1.charge.riflemen.contains(&id)
        || memory.rush_reserved(id)
}

/// With no site to watch: the squad rides behind the raid while it goes out, and comes home when
/// it turns back.
fn ride_with_raid(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    held: &BTreeSet<u32>,
) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let alive = |id: &u32| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == *id && unit.hp > 0)
    };
    memory.jeff_v1_1.deny.squad.retain(|id| alive(id));
    let tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| {
            memory
                .jeff_v1_1
                .tank_raid
                .tank_ids()
                .any(|id| id == unit.id)
        })
        .collect();
    if !memory.jeff_v1_1.tank_raid.going() || tanks.is_empty() {
        let ids: Vec<u32> = memory.jeff_v1_1.deny.squad.iter().copied().collect();
        if !ids.is_empty() {
            let home = tile_center(observation.own_start_tile, observation.map.tile_size);
            actions::move_units(actions, ids.iter().copied(), home.0, home.1);
            intents.push(AiIntent::Assemble { units: ids });
        }
        memory.jeff_v1_1.deny.squad.clear();
        return;
    }
    let n = tanks.len() as f32;
    let centre = (
        tanks.iter().map(|unit| unit.x).sum::<f32>() / n,
        tanks.iter().map(|unit| unit.y).sum::<f32>() / n,
    );
    if memory.jeff_v1_1.deny.squad.len() < SQUAD {
        let reserved = memory.squad_reserved();
        let mut free: Vec<&AiEntitySummary> = observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
            .filter(|unit| {
                !held.contains(&unit.id)
                    && !reserved.contains(&unit.id)
                    && !excluded(memory, unit.id)
            })
            .filter(|unit| dist2(unit.x, unit.y, centre.0, centre.1) <= squared(HOME_TILES * ts))
            .collect();
        free.sort_by(|a, b| {
            dist2(a.x, a.y, centre.0, centre.1)
                .total_cmp(&dist2(b.x, b.y, centre.0, centre.1))
                .then(a.id.cmp(&b.id))
        });
        let need = SQUAD - memory.jeff_v1_1.deny.squad.len();
        let picked: Vec<u32> = free.iter().take(need).map(|unit| unit.id).collect();
        memory.jeff_v1_1.deny.squad.extend(picked);
    }
    let state = &mut memory.jeff_v1_1.deny;
    if tick.saturating_sub(state.ordered) < REORDER_TICKS || state.squad.is_empty() {
        return;
    }
    state.ordered = tick;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let back = normalized_direction(centre, home).unwrap_or((0.0, 0.0));
    let spot = clamp_to_map(
        (
            centre.0 + back.0 * RIDE_BACK_TILES * ts,
            centre.1 + back.1 * RIDE_BACK_TILES * ts,
        ),
        observation.map,
    );
    let ids: Vec<u32> = state.squad.iter().copied().collect();
    actions::move_units(actions, ids.iter().copied(), spot.0, spot.1);
    intents.push(AiIntent::Assemble { units: ids });
}
