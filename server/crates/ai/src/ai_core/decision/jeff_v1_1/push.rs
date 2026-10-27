//! Jeff's AI V1.1's push against whole armies. V1.0 weighed its push against the most enemy Tanks
//! seen lately, needed a Scout Car to leave, and marched straight for the enemy natural; against
//! Pressure AI, which fields Riflemen, guns and Panzerfausts and few Tanks, it lost all 40 games.
//!
//! - **Whole armies**: every enemy fighter seen is remembered at its cost (`unit_value`) and fades
//!   out over `ARMY_FADE_TICKS`, or as soon as its last position is in sight without it. The push
//!   counts its Tanks, Riflemen and spotter the same way.
//! - **Spotter**: with no Scout Car a Rifleman leads.
//! - **Rolling line**: the push holds anchor after anchor along the enemy's route (`ANCHOR_SHARES`
//!   of the way, then the enemy natural), stepping forward once it holds an anchor and is ahead of
//!   the enemy army, and back one when the enemy in front outweighs it.
//! - **Strike the gathering wave**: Pressure AI gathers its waves about 25 tiles out from Jeff's
//!   natural and waits there for 5, 10, then 20 arrivals. A group within `STRIKE_TILES` of a Jeff
//!   base that the push outweighs `STRIKE_RATIO` times is hit while it gathers.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

/// A remembered enemy fighter counts fully when just seen and fades to nothing over this long.
const ARMY_FADE_TICKS: u32 = config::TICK_HZ * 90;
/// A remembered enemy whose last position is this close to one of Jeff's units or buildings is in
/// sight, so it is gone if it is not seen there.
const SIGHT_TILES: f32 = 8.0;
/// The push leaves, and steps forward, once it outweighs the remembered enemy army this much.
const ADVANCE_RATIO: f32 = 1.2;
/// Enemies this close to a push unit are the fight in front of it.
const FRONT_TILES: f32 = 16.0;
/// Anchors along the enemy's route, as shares of its length from Jeff's start.
const ANCHOR_SHARES: [f32; 3] = [0.3, 0.5, 0.7];
/// The push holds an anchor once its centre is this close to it, for `ANCHOR_HOLD_TICKS`.
const ANCHOR_REACHED_TILES: f32 = 10.0;
const ANCHOR_HOLD_TICKS: u32 = config::TICK_HZ * 8;
/// At most one step, forward or back, this often.
const STEP_TICKS: u32 = config::TICK_HZ * 10;
const STRIKE_TILES: f32 = 30.0;
const STRIKE_RATIO: f32 = 1.5;
/// Enemies this close to one another are one group.
const GROUP_TILES: f32 = 8.0;
/// A strike holds its target point this long after the group was last seen.
const STRIKE_MEMORY_TICKS: u32 = config::TICK_HZ * 10;

/// An enemy fighter as last seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct Sighting {
    tick: u32,
    value: u32,
    at: (i32, i32),
}

/// What V1.1's push remembers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct PushMemory {
    enemies: BTreeMap<u32, Sighting>,
    /// The anchor the push holds or makes for; past the last, the enemy natural.
    anchor: usize,
    last_step: u32,
    /// Since when the push has stood at its anchor.
    holding_since: Option<u32>,
    /// A gathering enemy group being struck, and when it was last seen.
    strike: Option<((i32, i32), u32)>,
}

impl PushMemory {
    /// Whether a gathering enemy group is being struck.
    pub(in crate::ai_core::decision) fn striking(&self) -> bool {
        self.strike.is_some()
    }
}

fn v1_1(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1)
}

fn is_fighter(kind: EntityKind) -> bool {
    kind.is_unit() && !matches!(kind, EntityKind::Worker | EntityKind::Golem)
}

/// A unit's weight in a fight: its cost, Oil counted double.
pub(in crate::ai_core::decision) fn unit_value(kind: EntityKind) -> u32 {
    let (steel, oil) = rts_rules::economy::cost(kind);
    steel + oil * 2
}

/// Notes the enemy fighters in sight and forgets those gone. V1.1 only.
pub(in crate::ai_core::decision) fn note_enemy_army(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) {
    if !v1_1(memory) {
        return;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let enemies = &mut memory.jeff_v1_1.push.enemies;
    let seen: BTreeSet<u32> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy.kind) && enemy.hp > 0)
        .map(|enemy| {
            enemies.insert(
                enemy.id,
                Sighting {
                    tick,
                    value: unit_value(enemy.kind),
                    at: (enemy.x.round() as i32, enemy.y.round() as i32),
                },
            );
            enemy.id
        })
        .collect();
    enemies.retain(|id, sighting| {
        if seen.contains(id) {
            return true;
        }
        let at = (sighting.at.0 as f32, sighting.at.1 as f32);
        let in_sight = observation
            .owned
            .iter()
            .any(|unit| dist2(unit.x, unit.y, at.0, at.1) <= squared(SIGHT_TILES * ts));
        !in_sight && tick.saturating_sub(sighting.tick) < ARMY_FADE_TICKS
    });
}

/// The remembered enemy army's weight, each fighter fading with the time since it was seen.
pub(in crate::ai_core::decision) fn enemy_army(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
) -> f32 {
    memory
        .jeff_v1_1
        .push
        .enemies
        .values()
        .map(|sighting| {
            let age = observation.tick.saturating_sub(sighting.tick) as f32;
            sighting.value as f32 * (1.0 - age / ARMY_FADE_TICKS as f32).max(0.0)
        })
        .sum()
}

/// The push's weight: its units alive, and `joining` Tanks more.
pub(in crate::ai_core::decision) fn push_army(
    observation: &AiObservation,
    units: &[u32],
    joining: usize,
) -> f32 {
    let own: u32 = observation
        .owned
        .iter()
        .filter(|unit| units.contains(&unit.id) && unit.hp > 0)
        .map(|unit| unit_value(unit.kind))
        .sum();
    (own + joining as u32 * unit_value(EntityKind::Tank)) as f32
}

/// The push units: Tanks, spotter and Riflemen.
fn push_units(memory: &AiDecisionMemory) -> Vec<u32> {
    let push = &memory.containment;
    push.active_tanks
        .iter()
        .copied()
        .chain(push.active_scout)
        .chain(push.active_riflemen.iter().copied())
        .collect()
}

/// V1.1's launch gate: the push outweighs the remembered enemy army `ADVANCE_RATIO` times, or a
/// gathering group is there to strike. `None` for V1.0, which keeps its Tank-count gates.
pub(in crate::ai_core::decision) fn launch_ready(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    joining: usize,
) -> Option<bool> {
    if !v1_1(memory) {
        return None;
    }
    let units = push_units(memory);
    let ours = push_army(observation, &units, joining);
    memory.jeff_v1_1.push.anchor = 0;
    memory.jeff_v1_1.push.holding_since = None;
    note_strike(observation, memory, ours);
    Some(
        memory.jeff_v1_1.push.strike.is_some()
            || ours >= enemy_army(observation, memory) * ADVANCE_RATIO,
    )
}

/// The enemy fighters near a Jeff base forming a group the push outweighs `STRIKE_RATIO` times:
/// its centre is struck. Groups seen earlier are held for `STRIKE_MEMORY_TICKS`.
fn note_strike(observation: &AiObservation, memory: &mut AiDecisionMemory, ours: f32) {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let near: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy.kind) && enemy.hp > 0)
        .filter(|enemy| {
            depots.iter().any(|depot| {
                dist2(enemy.x, enemy.y, depot.0, depot.1) <= squared(STRIKE_TILES * ts)
            })
        })
        .collect();
    // The largest group: the enemies within `GROUP_TILES` of its heaviest member.
    let group = near
        .iter()
        .map(|center| {
            let members: Vec<&&AiEntitySummary> = near
                .iter()
                .filter(|other| {
                    dist2(other.x, other.y, center.x, center.y) <= squared(GROUP_TILES * ts)
                })
                .collect();
            let value: u32 = members.iter().map(|member| unit_value(member.kind)).sum();
            let n = members.len() as f32;
            let at = (
                members.iter().map(|member| member.x).sum::<f32>() / n,
                members.iter().map(|member| member.y).sum::<f32>() / n,
            );
            (value, at)
        })
        .max_by(|a, b| a.0.cmp(&b.0).then(a.1 .0.total_cmp(&b.1 .0)));
    let push = &mut memory.jeff_v1_1.push;
    match group {
        Some((value, at)) if ours >= value as f32 * STRIKE_RATIO => {
            push.strike = Some(((at.0.round() as i32, at.1.round() as i32), tick));
        }
        Some(_) => push.strike = None,
        None => {
            push.strike = push
                .strike
                .filter(|(_, seen)| tick.saturating_sub(*seen) < STRIKE_MEMORY_TICKS);
        }
    }
}

/// Where V1.1's launched push makes for: a gathering group it strikes, else the anchor it holds
/// along the enemy's route, else `natural` (see the module docs). `None` for V1.0.
pub(in crate::ai_core::decision) fn objective(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    own_base: (f32, f32),
    natural: (f32, f32),
) -> Option<(f32, f32)> {
    if !v1_1(memory) {
        return None;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let units = push_units(memory);
    let ours = push_army(observation, &units, 0);
    note_strike(observation, memory, ours);
    if let Some((at, _)) = memory.jeff_v1_1.push.strike {
        return Some((at.0 as f32, at.1 as f32));
    }
    let anchors = anchors(observation, map_analysis, own_base, natural);
    let tanks: Vec<u32> = memory.containment.active_tanks.iter().copied().collect();
    let center = super::super::frontal::group_center(observation, &tanks)?;
    let front: f32 = observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy.kind) && enemy.hp > 0)
        .filter(|enemy| {
            observation.owned.iter().any(|unit| {
                units.contains(&unit.id)
                    && dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(FRONT_TILES * ts)
            })
        })
        .map(|enemy| unit_value(enemy.kind) as f32)
        .sum();
    let ahead = ours >= enemy_army(observation, memory) * ADVANCE_RATIO && ours >= front;
    let push = &mut memory.jeff_v1_1.push;
    let target = |anchor: usize| anchors.get(anchor).copied().unwrap_or(natural);
    let reached = dist2(
        center.0,
        center.1,
        target(push.anchor).0,
        target(push.anchor).1,
    ) <= squared(ANCHOR_REACHED_TILES * ts);
    if reached {
        push.holding_since.get_or_insert(tick);
    } else {
        push.holding_since = None;
    }
    let step_due = tick.saturating_sub(push.last_step) >= STEP_TICKS;
    let held = push
        .holding_since
        .is_some_and(|since| tick.saturating_sub(since) >= ANCHOR_HOLD_TICKS);
    if step_due && front > ours && push.anchor > 0 {
        push.anchor -= 1;
        push.last_step = tick;
        push.holding_since = None;
    } else if step_due && ahead && held && push.anchor < anchors.len() {
        push.anchor += 1;
        push.last_step = tick;
        push.holding_since = None;
    }
    Some(target(push.anchor))
}

/// Whether V1.1's push holds short of the enemy natural, at an anchor or a strike, where the
/// march watchdog must not drive it on.
pub(in crate::ai_core::decision) fn holding_short(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &AiDecisionMemory,
    own_base: (f32, f32),
    natural: (f32, f32),
) -> bool {
    v1_1(memory)
        && (memory.jeff_v1_1.push.strike.is_some()
            || memory.jeff_v1_1.push.anchor
                < anchors(observation, map_analysis, own_base, natural).len())
}

/// The anchors along the enemy's route toward `natural`, nearest Jeff first.
fn anchors(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    own_base: (f32, f32),
    natural: (f32, f32),
) -> Vec<(f32, f32)> {
    let enemy = observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .min_by(|a, b| {
            let at = |tile: (u32, u32)| tile_center(tile, observation.map.tile_size);
            let (a, b) = (at(a.start_tile), at(b.start_tile));
            dist2(a.0, a.1, own_base.0, own_base.1).total_cmp(&dist2(b.0, b.1, own_base.0, own_base.1))
        });
    let route = enemy.and_then(|enemy| {
        map_analysis.and_then(|analysis| analysis.base_route_tiles(enemy.id))
    });
    let limit = dist2(natural.0, natural.1, own_base.0, own_base.1);
    ANCHOR_SHARES
        .iter()
        .map(|share| match route.filter(|route| !route.is_empty()) {
            Some(route) => {
                let index = ((route.len() - 1) as f32 * share).round() as usize;
                tile_center((route[index].x, route[index].y), observation.map.tile_size)
            }
            None => (
                own_base.0 + (natural.0 - own_base.0) * share,
                own_base.1 + (natural.1 - own_base.1) * share,
            ),
        })
        .filter(|point| dist2(point.0, point.1, own_base.0, own_base.1) < limit)
        .collect()
}

/// V1.1's spotter when no Scout Car is ready: the free Rifleman nearest `rally`.
pub(in crate::ai_core::decision) fn spotter(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    rally: (f32, f32),
) -> Option<u32> {
    if !v1_1(memory) {
        return None;
    }
    let reserved = memory.squad_reserved();
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !memory.containment.active_riflemen.contains(&unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.later_bases.guards.contains(&unit.id)
                && !reserved.contains(&unit.id)
        })
        .min_by(|a, b| {
            dist2(a.x, a.y, rally.0, rally.1)
                .total_cmp(&dist2(b.x, b.y, rally.0, rally.1))
                .then(a.id.cmp(&b.id))
        })
        .map(|unit| unit.id)
}

/// V1.1's field check in place of the Tank count: the enemy in front of the push outweighs it.
/// `None` for V1.0.
pub(in crate::ai_core::decision) fn outweighed(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
) -> Option<bool> {
    if !v1_1(memory) {
        return None;
    }
    let ts = observation.map.tile_size as f32;
    let units = push_units(memory);
    let ours = push_army(observation, &units, 0);
    let front: f32 = observation
        .visible_enemies
        .iter()
        .filter(|enemy| is_fighter(enemy.kind) && enemy.hp > 0)
        .filter(|enemy| {
            observation.owned.iter().any(|unit| {
                units.contains(&unit.id)
                    && dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(FRONT_TILES * ts)
            })
        })
        .map(|enemy| unit_value(enemy.kind) as f32)
        .sum();
    // Outweighed short of the first anchor it falls back to form again; further out the rolling
    // line steps back an anchor instead (`objective`).
    Some(front > ours && memory.jeff_v1_1.push.anchor == 0)
}
