//! Jeff's AI V1.1 clears enemy infantry off the natural it is taking with massed Riflemen
//! (`natclear`). Without it the natural's Riflemen walked onto the site one or two at a time: on
//! Classic against Rush AI, 16 of the 22 Riflemen lost near the natural from 2:51 to 3:57 had at most
//! one friend within 4 tiles, against 2-7 dug-in Machine Gunners.
//!
//! The rules come from a bench of Meth Riflemen against dug-in Meth Machine Gunners on Classic
//! (local `mg-bench`): three Riflemen to a gun arriving together win (1 lost to 1 gun, 1-2 to 2,
//! 2-3 to 3, 4-6 to 4); more trade a little better (3-4 lost to 4 guns at five a gun); the same
//! Riflemen sent one every 2 s lose (all 15 against 3 guns, one gun killed). A group gathered at one
//! point spills forward into the guns' reach (hit from 7.5 tiles; a Rifleman shoots at 5.4) and
//! charges strung out; gathered as a block 5 wide, its front 13 tiles out, it charges as one.
//! Against guns walking at it, falling back into the block lost no more than charging.
//!
//! While the natural is next and not yet standing, and enemy infantry stands within `CLEAR_TILES`
//! of the site or within `ROUTE_TILES` of the way there from the main (past
//! `ROUTE_FROM_MAIN_TILES`), with no enemy Tank near the site. The natural's two guards are not
//! drafted into it (`ncguards=1` drafts them): drafted, they stood in a block short of the 8-10
//! Riflemen it needed while the natural waited for them on their posts, and The River's natural went
//! down at 2:32-3:35 instead of 1:56 on every seed (120s: vs Pressure 5-9-26 drafted, 8-8-24 not;
//! vs Rush 10-6-24 and 12-0-28):
//!
//! - **Size**: at least `rifles_per_gun` (3, `clearper` in `JEFF_V11_LINE`) Riflemen for each
//!   Machine Gunner and `RIFLES_PER_INFANTRY` for each other enemy, and up to `max_rifles_per_gun`
//!   (5, `clearmax`) and `MAX_RIFLES_PER_INFANTRY` when the main can spare them: the free Riflemen at
//!   the main nearest the block, never the last `HOME_KEEP` free there.
//! - **Gather**: each moves (no fighting on the way) to its slot in a block `BLOCK_WIDTH` wide, rows a
//!   tile apart, facing the enemy, its front `BLOCK_FRONT_TILES` from the nearest enemy at the
//!   natural, on the way from the main (or the open point on the way farthest from them). Slots go
//!   left to right so no two paths cross. The block is laid out again every decision: when the
//!   enemy walks at it, it falls back, and its Riflemen with it. Gathered Riflemen hold their slots.
//! - **Charge**: once at least the fewest have gathered, and no other member is within `WAIT_TICKS`
//!   of its slot (or the most have gathered), the gathered ones attack-move onto the enemy together.
//!   Riflemen gathering later join only `JOIN_GROUP` at a time, from the block.
//! - **Fall back**: a charge left with under half the Riflemen the enemy still standing needs moves
//!   back to the block and gathers again.
//!
//! The group picks its Riflemen before home defense does (raid responders included, but not one
//! within `BUSY_TILES` of an enemy away from the natural), and they are kept from home defense, the
//! natural's support (which holds off meanwhile) and the home staging.

use super::*;
use crate::ai_core::decision::geometry::{building_center, dist2, squared, tile_center};

/// Enemies this close to the natural site are cleared...
const CLEAR_TILES: f32 = 15.0;
/// ... and those this close to the way there from the main, past this far from the main.
const ROUTE_TILES: f32 = 8.0;
const ROUTE_FROM_MAIN_TILES: f32 = 10.0;
/// An enemy Tank this close to the site leaves it to the Tanks: Riflemen cannot hurt one.
const TANK_ABORT_TILES: f32 = 16.0;
/// The fewest Riflemen that go in against each enemy...
const RIFLES_PER_GUN: f32 = 3.0;
const RIFLES_PER_INFANTRY: f32 = 2.0;
const RIFLES_PER_VEHICLE: f32 = 4.0;
/// ... and the most sent when the main can spare them.
const MAX_RIFLES_PER_GUN: f32 = 5.0;
const MAX_RIFLES_PER_INFANTRY: f32 = 3.0;
const MAX_RIFLES_PER_VEHICLE: f32 = 5.0;
const MIN_GROUP: usize = 3;
/// Once the group has the fewest it needs, the charge waits only for members this close in walking.
const WAIT_TICKS: u32 = config::TICK_HZ * 6;
/// Methamphetamines' speed for Riflemen.
const METH_SPEED: f32 = 1.25;
/// Riflemen gathering after a charge has gone in join it this many at a time.
const JOIN_GROUP: usize = 3;
/// The block: this many to a row, rows a tile apart, its front this far from the nearest enemy.
const BLOCK_WIDTH: usize = 5;
const BLOCK_FRONT_TILES: f32 = 13.0;
/// A Rifleman this close to its slot has gathered.
const GATHERED_TILES: f32 = 2.5;
/// The block stays put while no enemy comes this close to it and the ideal spot stays this near.
const BLOCK_KEEP_TILES: f32 = 10.0;
const BLOCK_DRIFT_TILES: f32 = 6.0;
/// Once the fewest have gathered, the charge waits at most this long for more.
const READY_WAIT_TICKS: u32 = config::TICK_HZ * 3;
/// Riflemen this close to the main can be taken.
const HOME_TILES: f32 = 40.0;
/// A Rifleman this close to an enemy fighter away from the natural is left to home defense.
const BUSY_TILES: f32 = 8.0;
/// Free Riflemen the group always leaves at the main.
const HOME_KEEP: usize = 4;
/// A gatherer is sent again when its slot moves this far.
const REGATHER_TILES: f32 = 1.5;
/// A charge is re-aimed when the enemy's centre moves this far.
const RETARGET_TILES: f32 = 3.0;
/// An idle member short of where it was sent is sent again this often.
const REORDER_TICKS: u32 = config::TICK_HZ * 2;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct NatClearMemory {
    members: BTreeSet<u32>,
    charge: BTreeSet<u32>,
    /// Where each member was last sent, when, and whether to charge.
    sent: BTreeMap<u32, ((i32, i32), u32, bool)>,
    need: usize,
    gathered: usize,
    targets: usize,
    block: Option<(i32, i32)>,
    /// Since when the fewest the enemy needs have stood gathered.
    enough_since: Option<u32>,
    /// Why the group stands down (trace), or "active".
    why: &'static str,
}

impl NatClearMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.members.contains(&id)
    }

    /// Whether enemy infantry holds the natural being taken and the group is at work on it.
    pub(in crate::ai_core::decision) fn active(&self) -> bool {
        self.why == "active"
    }

    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.members.iter().copied()
    }

    /// Whether the group stands short of the fewest it needs, not charging: `breakout` may take it
    /// in front of the station Tanks (`ncbreak`).
    pub(in crate::ai_core::decision) fn stuck(&self) -> bool {
        self.why == "active" && self.charge.is_empty() && self.members.len() < self.need
    }

    pub(in crate::ai_core::decision) fn trace(&self) -> String {
        format!(
            "v1_1_natclear why={} members={} charge={} gathered={} need={} targets={} block={:?}",
            self.why,
            self.members.len(),
            self.charge.len(),
            self.gathered,
            self.need,
            self.targets,
            self.block
        )
    }
}

/// A local testing switch in `JEFF_V11_LINE` (`name=0` or `name=1`).
pub(in crate::ai_core::decision) fn flag(name: &str, default: bool) -> bool {
    crate::ai_core::profiles::jeff_v1_1_param(name).map_or(default, |v| v != 0.0)
}

fn rifles_per_gun() -> f32 {
    crate::ai_core::profiles::jeff_v1_1_param("clearper").map_or(RIFLES_PER_GUN, |v| v.max(1.0))
}

fn max_rifles_per_gun() -> f32 {
    crate::ai_core::profiles::jeff_v1_1_param("clearmax")
        .map_or(MAX_RIFLES_PER_GUN, |v| v.max(rifles_per_gun()))
}

/// The fewest Riflemen that go in against `kind`.
fn need_for(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::MachineGunner => rifles_per_gun(),
        EntityKind::ScoutCar => RIFLES_PER_VEHICLE,
        _ => RIFLES_PER_INFANTRY,
    }
}

/// The most Riflemen sent against `kind` when the main can spare them: more trade better.
fn ideal_for(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::MachineGunner => max_rifles_per_gun(),
        EntityKind::ScoutCar => MAX_RIFLES_PER_VEHICLE,
        _ => MAX_RIFLES_PER_INFANTRY,
    }
}

/// Gathers and charges the natural's clearing group (see the module docs). `held`: units other
/// systems keep this decision. Returns the group's Riflemen.
pub(in crate::ai_core::decision) fn plan(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    held: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    let previous = std::mem::take(&mut memory.jeff_v1_1.natclear);
    let previous = NatClearMemory {
        enough_since: previous.enough_since,
        ..previous
    };
    if !is_jeff_v1_1(profile_id) || super::off("natclear") {
        return BTreeSet::new();
    }
    let ts = observation.map.tile_size as f32;
    let depots = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .count();
    let due = !memory.expansion_security.riflemen.is_empty() && depots < 2;
    let Some(center) = memory
        .expansion_security
        .site
        .filter(|_| due)
        .and_then(|site| building_center(site, EntityKind::ResourceDepot, observation.map.tile_size))
    else {
        memory.jeff_v1_1.natclear.why = "not_due";
        return BTreeSet::new();
    };
    let near = |x: f32, y: f32, tiles: f32| dist2(x, y, center.0, center.1) <= squared(tiles * ts);
    if observation.visible_enemies.iter().any(|enemy| {
        enemy.hp > 0 && enemy.kind == EntityKind::Tank && near(enemy.x, enemy.y, TANK_ABORT_TILES)
    }) {
        memory.jeff_v1_1.natclear.why = "tank";
        return BTreeSet::new();
    }
    // The enemies at the natural, and those on the way to it from the main past `ROUTE_FROM_MAIN_TILES`
    // (nearer the main they are home defense's).
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let on_route = |x: f32, y: f32| {
        let (vx, vy) = (center.0 - main.0, center.1 - main.1);
        let len2 = (vx * vx + vy * vy).max(1.0);
        let along = (((x - main.0) * vx + (y - main.1) * vy) / len2).clamp(0.0, 1.0);
        let (px, py) = (main.0 + vx * along, main.1 + vy * along);
        dist2(x, y, px, py) <= squared(ROUTE_TILES * ts)
            && dist2(x, y, main.0, main.1) >= squared(ROUTE_FROM_MAIN_TILES * ts)
    };
    let in_play = |x: f32, y: f32| near(x, y, CLEAR_TILES) || (flag("ncroute", true) && on_route(x, y));
    let targets: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| in_play(enemy.x, enemy.y))
        .collect();
    if targets.is_empty() {
        memory.jeff_v1_1.natclear.why = "clear";
        return BTreeSet::new();
    }
    let need = (targets.iter().map(|enemy| need_for(enemy.kind)).sum::<f32>().ceil() as usize)
        .max(MIN_GROUP);
    let ideal = (targets.iter().map(|enemy| ideal_for(enemy.kind)).sum::<f32>().ceil() as usize)
        .max(need);
    let n = targets.len() as f32;
    let aim = (
        targets.iter().map(|enemy| enemy.x).sum::<f32>() / n,
        targets.iter().map(|enemy| enemy.y).sum::<f32>() / n,
    );
    let nearest_target = |x: f32, y: f32| {
        targets
            .iter()
            .map(|enemy| dist2(enemy.x, enemy.y, x, y).sqrt())
            .fold(f32::MAX, f32::min)
    };
    // The block's front centre: the nearest open point to the enemy's centre, on the way from the
    // main, `BLOCK_FRONT_TILES` from every enemy at the natural; with none short of the main, the
    // open point on the way farthest from them. `facing`: from the block towards the enemy.
    let toward_main = normalized_direction(aim, main).unwrap_or((0.0, 0.0));
    let ideal_front = {
        let span = dist2(aim.0, aim.1, main.0, main.1).sqrt();
        let mut out = 0.0;
        let mut safest: Option<((f32, f32), f32)> = None;
        loop {
            if out > span {
                break safest.map_or(main, |(point, _)| point);
            }
            let point = clamp_to_map(
                (aim.0 + toward_main.0 * out, aim.1 + toward_main.1 * out),
                observation.map,
            );
            if defense::defensive_position_is_open(observation, map_analysis, point.0, point.1) {
                let room = nearest_target(point.0, point.1);
                if room >= BLOCK_FRONT_TILES * ts {
                    break point;
                }
                if safest.is_none_or(|(_, best)| room > best) {
                    safest = Some((point, room));
                }
            }
            out += ts;
        }
    };
    // The block stays where it was while no enemy comes within `BLOCK_KEEP_TILES` of it and the
    // ideal spot has not drifted `BLOCK_DRIFT_TILES`: re-laid every decision under Rush AI's moving
    // guns, its Riflemen kept walking to new slots and charged two minutes late.
    let front = previous
        .block
        .map(|(x, y)| (x as f32, y as f32))
        .filter(|block| {
            nearest_target(block.0, block.1) >= BLOCK_KEEP_TILES * ts
                && dist2(block.0, block.1, ideal_front.0, ideal_front.1)
                    <= squared(BLOCK_DRIFT_TILES * ts)
        })
        .unwrap_or(ideal_front);
    let facing = normalized_direction(front, aim).unwrap_or((-toward_main.0, -toward_main.1));
    // A Rifleman this close to an enemy fighter not at the natural is busy defending elsewhere.
    let busy = |unit: &AiEntitySummary| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && enemy.kind != EntityKind::Worker
                && !in_play(enemy.x, enemy.y)
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(BUSY_TILES * ts)
        })
    };
    let reserved = memory.squad_reserved();
    let charging_riflemen = memory.jeff_v1_1.charge.riflemen.clone();
    // Without `ncguards` the natural's guards are never taken (`ncskipguards`): taken as free
    // Riflemen at the main, one charged with the group while natural security sent it back to its
    // post the same decision, and the natural went unsecured until the group was done.
    let skip_guards = !flag("ncguards", false) && !super::off("ncskipguards");
    let free = |unit: &AiEntitySummary| {
        unit.kind == EntityKind::Rifleman
            && unit.is_complete
            && unit.hp > 0
            && !held.contains(&unit.id)
            && !(skip_guards && memory.expansion_security.riflemen.contains(&unit.id))
            && !reserved.contains(&unit.id)
            && !busy(unit)
            && !charging_riflemen.contains(&unit.id)
            && !memory.containment.active_riflemen.contains(&unit.id)
            && !memory.rush_reserved(unit.id)
            && !memory.stays_out_of_home_defense(unit.id)
            && !memory.later_bases.guards.contains(&unit.id)
            && !memory.jeff_v1_1.base_lines.posted(unit.id)
            && !memory.jeff_v1_1.stations.posted_out(unit.id)
    };
    let by_id: BTreeMap<u32, &AiEntitySummary> =
        observation.owned.iter().map(|unit| (unit.id, unit)).collect();
    // The members still free (with `ncguards=1` the natural's guards first), then the free Riflemen
    // at the main nearest the block, up to the most the enemy is worth, always leaving `HOME_KEEP`
    // free at the main.
    let mut members: BTreeSet<u32> = previous
        .members
        .iter()
        .chain(memory.expansion_security.riflemen.iter().filter(|_| flag("ncguards", false)))
        .copied()
        .filter(|id| by_id.get(id).is_some_and(|unit| free(unit)))
        .collect();
    let mut spare: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| free(unit) && !members.contains(&unit.id))
        .filter(|unit| dist2(unit.x, unit.y, main.0, main.1) <= squared(HOME_TILES * ts))
        .collect();
    spare.sort_by(|a, b| {
        dist2(a.x, a.y, front.0, front.1)
            .total_cmp(&dist2(b.x, b.y, front.0, front.1))
            .then(a.id.cmp(&b.id))
    });
    let can_take = spare.len().saturating_sub(HOME_KEEP);
    for unit in spare.into_iter().take(can_take) {
        if members.len() >= ideal {
            break;
        }
        members.insert(unit.id);
    }
    let members = members;
    let mut charge: BTreeSet<u32> =
        previous.charge.iter().copied().filter(|id| members.contains(id)).collect();
    // A charge left with under half of what the enemy still needs falls back to gather.
    if charge.len() * 2 < need {
        charge.clear();
    }
    // The block's slots for the members not charging, left to right across the approach.
    let across = (-facing.1, facing.0);
    let lateral = |x: f32, y: f32| (x - front.0) * across.0 + (y - front.1) * across.1;
    let mut gatherers: Vec<u32> = members.iter().copied().filter(|id| !charge.contains(id)).collect();
    gatherers.sort_by(|a, b| {
        let at = |id: &u32| by_id.get(id).map_or(0.0, |unit| lateral(unit.x, unit.y));
        at(a).total_cmp(&at(b)).then(a.cmp(b))
    });
    let count = gatherers.len();
    let mut slots: Vec<(f32, f32)> = (0..count)
        .map(|index| {
            let (row, col) = (index / BLOCK_WIDTH, index % BLOCK_WIDTH);
            let width = (count - row * BLOCK_WIDTH).min(BLOCK_WIDTH);
            let side = (col as f32 - (width as f32 - 1.0) * 0.5) * ts;
            let mut back = row as f32 * ts;
            loop {
                let point = clamp_to_map(
                    (
                        front.0 + across.0 * side - facing.0 * back,
                        front.1 + across.1 * side - facing.1 * back,
                    ),
                    observation.map,
                );
                if back >= row as f32 * ts + 4.0 * ts
                    || defense::defensive_position_is_open(observation, map_analysis, point.0, point.1)
                {
                    break point;
                }
                back += 0.5 * ts;
            }
        })
        .collect();
    slots.sort_by(|a, b| lateral(a.0, a.1).total_cmp(&lateral(b.0, b.1)));
    // Rows were laid out front to back; give each Rifleman, left to right, the slot in its place.
    let slot_of: BTreeMap<u32, (f32, f32)> = gatherers.iter().copied().zip(slots).collect();
    let gathered: BTreeSet<u32> = gatherers
        .iter()
        .copied()
        .filter(|id| {
            by_id.get(id).zip(slot_of.get(id)).is_some_and(|(unit, slot)| {
                dist2(unit.x, unit.y, slot.0, slot.1) <= squared(GATHERED_TILES * ts)
            })
        })
        .collect();
    // Members still walking that will be there within `WAIT_TICKS`: the charge waits for them once it
    // has the fewest, and no longer for any.
    let speed = config::unit_stats(EntityKind::Rifleman).map_or(1.6, |stats| stats.speed)
        * if observation.upgrades.contains(&UpgradeKind::Methamphetamines) { METH_SPEED } else { 1.0 };
    let coming = gatherers
        .iter()
        .filter(|id| !gathered.contains(id))
        .filter(|id| {
            by_id.get(id).zip(slot_of.get(id)).is_some_and(|(unit, slot)| {
                dist2(unit.x, unit.y, slot.0, slot.1).sqrt() / speed.max(0.1) <= WAIT_TICKS as f32
            })
        })
        .count();
    let tick = observation.tick;
    // Since when the fewest have stood gathered: the charge waits for no one past `READY_WAIT_TICKS`.
    let enough_since = (gathered.len() >= need)
        .then(|| previous.enough_since.unwrap_or(tick))
        .filter(|_| charge.is_empty());
    let ready = gathered.len() >= need
        && (gathered.len() >= ideal
            || coming == 0
            || enough_since.is_some_and(|since| tick.saturating_sub(since) >= READY_WAIT_TICKS));
    if (charge.is_empty() && ready) || (!charge.is_empty() && gathered.len() >= JOIN_GROUP) {
        charge.extend(gathered.iter().copied());
    }
    let key = |point: (f32, f32)| (point.0.round() as i32, point.1.round() as i32);
    let mut sent = BTreeMap::new();
    let (mut charged, mut holding) = (Vec::new(), Vec::new());
    for id in &members {
        let Some(unit) = by_id.get(id) else {
            continue;
        };
        let last = previous.sent.get(id).copied();
        if charge.contains(id) {
            let fresh = last.is_none_or(|(at, when, charging)| {
                !charging
                    || dist2(at.0 as f32, at.1 as f32, aim.0, aim.1) > squared(RETARGET_TILES * ts)
                    || (unit.state == AiEntityState::Idle
                        && tick.saturating_sub(when) >= REORDER_TICKS)
            });
            if fresh {
                charged.push(*id);
                sent.insert(*id, (key(aim), tick, true));
            } else if let Some(last) = last {
                sent.insert(*id, last);
            }
            continue;
        }
        let Some(slot) = slot_of.get(id).copied() else {
            continue;
        };
        if gathered.contains(id) {
            if unit.state != AiEntityState::Idle {
                holding.push(*id);
            }
            sent.insert(*id, (key(slot), tick, false));
            continue;
        }
        // To its slot: sent again when the slot moves, a charge falls back, or it stands idle short.
        let fresh = last.is_none_or(|(at, when, charging)| {
            charging
                || dist2(at.0 as f32, at.1 as f32, slot.0, slot.1) > squared(REGATHER_TILES * ts)
                || (unit.state == AiEntityState::Idle && tick.saturating_sub(when) >= REORDER_TICKS)
        });
        if fresh {
            let order = if flag("ncattack", false) {
                actions::attack_move_units(actions, [*id], slot.0, slot.1)
            } else {
                actions::move_units(actions, [*id], slot.0, slot.1)
            };
            if let Some(units) = order {
                intents.push(AiIntent::Assemble { units });
            }
            sent.insert(*id, (key(slot), tick, false));
        } else if let Some(last) = last {
            sent.insert(*id, last);
        }
    }
    if let Some(units) = actions::attack_move_units(actions, charged, aim.0, aim.1) {
        intents.push(AiIntent::Attack { units });
    }
    actions::hold_position_units(actions, holding);
    memory.jeff_v1_1.natclear = NatClearMemory {
        members: members.clone(),
        charge,
        sent,
        need,
        gathered: gathered.len(),
        targets: targets.len(),
        block: Some(key(front)),
        enough_since,
        why: "active",
    };
    members
}
