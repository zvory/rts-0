//! Jeff's AI V1.1 hits Pressure AI's waves while they gather. Pressure stages each wave 35 tiles
//! out from the base it means to hit, along the ground route, and waits there for 5, then 10, then
//! 20 arrivals: in front of Jeff's natural, in sight of its line's picket, and long before Jeff's
//! push has the Tanks to leave. A group of enemy fighters standing within `STRIKE_TILES` of a Jeff
//! Depot but clear of its buildings, that the home Riflemen and Tanks outweigh `STRIKE_RATIO`
//! times, is struck by the nearest of them until they outweigh it `PARTY_RATIO` times, Tanks only
//! when the group holds no Panzerfausts. The strike ends when the group is gone, has left, or
//! outweighs the party; the base lines then take the party back.

use super::push::unit_value;
use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

const STRIKE_TILES: f32 = 35.0;
/// Enemies closer than this to a Jeff building are a raid, the raid defense's.
const RAID_TILES: f32 = 12.0;
const GROUP_TILES: f32 = 8.0;
const MIN_GROUP: usize = 3;
const STRIKE_RATIO: f32 = 1.5;
const PARTY_RATIO: f32 = 2.0;
/// The strike holds its target this long after the group was last seen near it.
const TARGET_MEMORY_TICKS: u32 = config::TICK_HZ * 5;
/// The party is sent again this often.
const REORDER_TICKS: u32 = config::TICK_HZ * 3;

/// V1.1's strike on a gathering wave.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct StrikeMemory {
    pub(in crate::ai_core::decision) party: BTreeSet<u32>,
    target: Option<((i32, i32), u32)>,
    ordered: u32,
}

fn is_fighter(kind: EntityKind) -> bool {
    kind.is_unit() && !matches!(kind, EntityKind::Worker | EntityKind::Golem)
}

/// The heaviest group of enemy fighters gathering near a Jeff base: its value, centre, and whether
/// it holds Panzerfausts.
fn gathering_group(observation: &AiObservation) -> Option<(f32, (f32, f32), bool)> {
    let ts = observation.map.tile_size as f32;
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let gathering: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy.kind) && enemy.hp > 0)
        .filter(|enemy| {
            depots.iter().any(|depot| {
                dist2(enemy.x, enemy.y, depot.0, depot.1) <= squared(STRIKE_TILES * ts)
            }) && !observation.owned.iter().any(|unit| {
                unit.kind.is_building()
                    && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(RAID_TILES * ts)
            })
        })
        .collect();
    gathering
        .iter()
        .filter_map(|center| {
            let members: Vec<&&AiEntitySummary> = gathering
                .iter()
                .filter(|other| {
                    dist2(other.x, other.y, center.x, center.y) <= squared(GROUP_TILES * ts)
                })
                .collect();
            (members.len() >= MIN_GROUP).then(|| {
                let n = members.len() as f32;
                let value: u32 = members.iter().map(|member| unit_value(member.kind)).sum();
                let at = (
                    members.iter().map(|member| member.x).sum::<f32>() / n,
                    members.iter().map(|member| member.y).sum::<f32>() / n,
                );
                let fausts = members
                    .iter()
                    .any(|member| member.kind == EntityKind::Panzerfaust);
                (value as f32, at, fausts)
            })
        })
        .max_by(|a, b| a.0.total_cmp(&b.0).then(a.1 .0.total_cmp(&b.1 .0)))
}

/// Strikes a gathering wave (see the module docs). `held`: units other systems ordered or keep this
/// decision. V1.1 only.
pub(in crate::ai_core::decision) fn strike_gathering_wave(
    (actions, observation, _, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    held: &BTreeSet<u32>,
) {
    if !is_jeff_v1_1(profile_id) {
        return;
    }
    let tick = observation.tick;
    let alive: BTreeSet<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.hp > 0)
        .map(|unit| unit.id)
        .collect();
    memory
        .jeff_v1_1
        .strike
        .party
        .retain(|id| alive.contains(id) && !held.contains(id));
    let group = gathering_group(observation);
    // Tanks go only at a group without Panzerfausts.
    let fausts = group.is_some_and(|(_, _, fausts)| fausts);
    let value_of = |ids: &BTreeSet<u32>| -> f32 {
        observation
            .owned
            .iter()
            .filter(|unit| ids.contains(&unit.id))
            .map(|unit| unit_value(unit.kind) as f32)
            .sum()
    };
    let reserved = memory.squad_reserved();
    let free: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| {
            (unit.kind == EntityKind::Rifleman || (unit.kind == EntityKind::Tank && !fausts))
                && unit.is_complete
                && unit.hp > 0
                && !held.contains(&unit.id)
                && !reserved.contains(&unit.id)
                && Some(unit.id) != memory.home_defensive_tank
                && memory.containment.active_scout != Some(unit.id)
                && !memory.containment.active_tanks.contains(&unit.id)
                && !memory.containment.active_riflemen.contains(&unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.later_bases.guards.contains(&unit.id)
                && !memory.opening_rush.is_reserved(unit.id)
        })
        .collect();
    let state = &mut memory.jeff_v1_1.strike;
    match group {
        Some((value, at, _)) => {
            let near_target = state.target.is_none_or(|(target, _)| {
                let ts = observation.map.tile_size as f32;
                dist2(at.0, at.1, target.0 as f32, target.1 as f32)
                    <= squared(2.0 * GROUP_TILES * ts)
            });
            let available: f32 = free.iter().map(|unit| unit_value(unit.kind) as f32).sum();
            if state.party.is_empty() && available < value * STRIKE_RATIO {
                state.target = None;
                return;
            }
            if !near_target && state.party.is_empty() {
                state.target = None;
            }
            state.target = Some(((at.0.round() as i32, at.1.round() as i32), tick));
            // Fill the party from the nearest free units until it outweighs the group.
            let mut nearest: Vec<&&AiEntitySummary> = free
                .iter()
                .filter(|unit| !state.party.contains(&unit.id))
                .collect();
            nearest.sort_by(|a, b| {
                dist2(a.x, a.y, at.0, at.1)
                    .total_cmp(&dist2(b.x, b.y, at.0, at.1))
                    .then(a.id.cmp(&b.id))
            });
            let mut party_value = value_of(&state.party);
            for unit in nearest {
                if party_value >= value * PARTY_RATIO {
                    break;
                }
                state.party.insert(unit.id);
                party_value += unit_value(unit.kind) as f32;
            }
            if party_value < value {
                state.party.clear();
                state.target = None;
                return;
            }
        }
        None => {
            if state
                .target
                .is_none_or(|(_, seen)| tick.saturating_sub(seen) >= TARGET_MEMORY_TICKS)
            {
                state.party.clear();
                state.target = None;
                return;
            }
        }
    }
    let Some((target, _)) = state.target else {
        return;
    };
    if state.party.is_empty() || tick.saturating_sub(state.ordered) < REORDER_TICKS {
        return;
    }
    let party: Vec<u32> = state.party.iter().copied().collect();
    if let Some(units) =
        actions::attack_move_units(actions, party, target.0 as f32, target.1 as f32)
    {
        state.ordered = tick;
        intents.push(AiIntent::Attack { units });
    }
}
