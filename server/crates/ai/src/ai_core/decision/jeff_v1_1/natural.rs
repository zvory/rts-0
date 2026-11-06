//! Jeff's AI V1.1 holds its natural forward and with more than two guards. V1.0's two guards stood
//! 3.5 tiles out from the Depot, in among the natural's own extractors, and the rest of its army
//! stood at the main: when the natural went down against Pressure AI on Classic, 4-6 guns and 3-4
//! Riflemen stood at the main and two Riflemen at the natural.
//!
//! - **Forward guards** (`natforward`): the two guards stand `GUARD_FORWARD_TILES` out toward the
//!   enemy, past the extractors.
//! - **Support** (`natsupport`): from the moment the natural is next, up to `SUPPORT` more units
//!   from the main hold a line `LINE_FORWARD_TILES` out in front of it, 2.5 tiles apart: the home
//!   guns past the `HOME_GUNS_KEPT` nearest the main first, then free Riflemen at the main. They
//!   stay while the natural stands, and the home pocket and the push leave them be.

use super::*;
use crate::ai_core::decision::geometry::{building_center, dist2, squared, tile_center};

pub(in crate::ai_core::decision) const GUARD_FORWARD_TILES: f32 = 7.0;
const SUPPORT: usize = 4;
const HOME_GUNS_KEPT: usize = 2;
const LINE_FORWARD_TILES: f32 = 6.0;
const LINE_SPACING_TILES: f32 = 2.5;
/// Supporters to a rank of the line, later ranks this far behind: the Rifleman opening's holders
/// stand two deep instead of 30 tiles wide.
const RANK_WIDTH: usize = 6;
const RANK_BACK_TILES: f32 = 2.0;

/// How many units support the natural: `SUPPORT`, or under the Rifleman opening (`riflenat`) all of
/// its holders but the two guards.
pub(in crate::ai_core::decision) fn support_size(profile_id: &str) -> usize {
    if super::rifle_opening(profile_id) {
        super::natural_rifles().saturating_sub(2)
    } else {
        SUPPORT
    }
}
/// How many Riflemen the natural's line wants now: its size while the line stands, else none.
pub(in crate::ai_core::decision) fn support_wanted(
    memory: &AiDecisionMemory,
    profile_id: &str,
) -> usize {
    let active = is_jeff_v1_1(profile_id)
        && !super::off("natsupport")
        && memory.expansion_security.site.is_some()
        && !memory.expansion_security.riflemen.is_empty()
        && !(memory.jeff_v1_1.natclear.active() && !super::off("natclear"));
    if active {
        support_size(profile_id)
    } else {
        0
    }
}
/// A unit this close to its post holds there.
const ON_POST_TILES: f32 = 1.0;
/// A unit with an enemy this close fights where it stands.
const FIGHT_TILES: f32 = 7.0;
/// Riflemen this close to the main's Depot are the main's to give.
const HOME_TILES: f32 = 16.0;
/// A unit off its post is sent back this often.
const REORDER_TICKS: u32 = config::TICK_HZ * 3;
/// A Rifleman not shooting with an enemy close is sent at it this often (`natfight`).
const FIGHT_REORDER_TICKS: u32 = config::TICK_HZ * 2;
/// A supporter home defense took keeps its post this long (`postreturn`).
const AWAY_TICKS: u32 = config::TICK_HZ * 60;

/// V1.1's natural support and when each was last sent to its post.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct NaturalMemory {
    pub(in crate::ai_core::decision) support: BTreeMap<u32, u32>,
    /// Supporters home defense holds, and since when (`postreturn`).
    away: BTreeMap<u32, u32>,
    /// Each supporter's post (`natseat`).
    seats: BTreeMap<u32, (i32, i32)>,
}

impl NaturalMemory {
    pub(in crate::ai_core::decision) fn holds(&self, id: u32) -> bool {
        self.support.contains_key(&id)
    }
}

/// Whether the natural's guards stand forward.
pub(in crate::ai_core::decision) fn forward(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("natforward")
}

/// Whether one of Jeff's own units stands on the natural's Depot footprint, which then reads as
/// unplaceable (`sitewalk`). Natural security dropped such a site with its guards and their timer:
/// on The River against Pressure AI the support, let go once the clearing group was done, walked
/// across the site just as the guards had held it, twice, and the Depot went down at 2:59 instead
/// of 1:56. Units move; the site is kept while they stand on it.
pub(in crate::ai_core::decision) fn own_units_on_site(
    profile_id: &str,
    observation: &AiObservation,
    site: (u32, u32),
) -> bool {
    if !is_jeff_v1_1(profile_id) || super::off("sitewalk") {
        return false;
    }
    let Some(stats) = config::building_stats(EntityKind::ResourceDepot) else {
        return false;
    };
    let ts = observation.map.tile_size as f32;
    let rect = (
        site.0 as f32 * ts,
        site.1 as f32 * ts,
        site.0.saturating_add(stats.foot_w) as f32 * ts,
        site.1.saturating_add(stats.foot_h) as f32 * ts,
    );
    observation.owned.iter().any(|unit| {
        unit.kind.is_unit()
            && unit.hp > 0
            && crate::sdk::unit_circle_touches_rect(
                (unit.x, unit.y),
                rts_rules::balance::unit_placement_radius(unit.kind),
                rect,
            )
    })
}

/// Picks and posts the natural's support (see the module docs). `home_guns`: the main's defensive
/// guns; `held`: units other systems keep this decision. `defending`: the units home defense holds this
/// decision: supporters among them keep their posts while away (`postreturn`).
pub(in crate::ai_core::decision) fn support(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    home_guns: &[u32],
    held: &BTreeSet<u32>,
    defending: &BTreeSet<u32>,
) {
    let ts = observation.map.tile_size as f32;
    let site = memory.expansion_security.site;
    // While enemy infantry holds the natural, its clearing group (`natclear`) fights for it: the
    // support walked to posts in among the enemy, one or two at a time.
    let active = is_jeff_v1_1(profile_id)
        && !super::off("natsupport")
        && !memory.expansion_security.riflemen.is_empty()
        && !(memory.jeff_v1_1.natclear.active() && !super::off("natclear"));
    let Some(center) = site.filter(|_| active).and_then(|site| {
        building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
    }) else {
        memory.jeff_v1_1.natural.support.clear();
        memory.jeff_v1_1.natural.away.clear();
        memory.jeff_v1_1.natural.seats.clear();
        return;
    };
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy = observation
        .players
        .iter()
        .filter(|p| p.is_alive && observation.is_enemy_player(p.id))
        .min_by_key(|p| p.id)
        .map(|p| tile_center(p.start_tile, observation.map.tile_size))
        .unwrap_or(center);
    let direction = normalized_direction(center, enemy)
        .or_else(|| normalized_direction(main, center))
        .unwrap_or((1.0, 0.0));
    let reserved = memory.squad_reserved();
    let free = |unit: &AiEntitySummary| {
        unit.is_complete
            && unit.hp > 0
            && !held.contains(&unit.id)
            && !reserved.contains(&unit.id)
            && !memory.expansion_security.riflemen.contains(&unit.id)
            && !memory.containment.active_riflemen.contains(&unit.id)
            && !memory.rush_reserved(unit.id)
            && !memory.later_bases.guards.contains(&unit.id)
            && !memory.jeff_v1_1.base_lines.posted(unit.id)
            && !memory.jeff_v1_1.stations.posted_out(unit.id)
    };
    // Under `riflestations` the Rifle split (`stations::place_riflemen`) names the natural's
    // Riflemen: its own, wherever they stand, and no others.
    let split = super::stations::rifles_on(profile_id);
    let named = |unit: &AiEntitySummary| {
        !split
            || unit.kind != EntityKind::Rifleman
            || memory.jeff_v1_1.stations.rifle_post(unit.id)
                == Some(super::stations::RiflePost::Natural)
    };
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    // Under `mghome` the guns go only once the natural's Depot stands: sent to a line in front of
    // an empty site, they walked out ahead of it and died there or on the way.
    let guns_go = super::off("mghome")
        || observation.owned.iter().any(|unit| {
            unit.kind == EntityKind::ResourceDepot
                && unit.is_complete
                && unit.hp > 0
                && dist2(unit.x, unit.y, center.0, center.1) <= squared(ts)
        });
    // Under `mgline` the guns hold the spread lines in front of the bases (`base_line`) instead.
    let guns_go = guns_go && super::off("mgline");
    let free = |unit: &AiEntitySummary| {
        free(unit) && named(unit) && (guns_go || unit.kind != EntityKind::MachineGunner)
    };
    // Keep the support it has while each is still free.
    // Under `postreturn` a supporter home defense took keeps its post for up to `AWAY_TICKS`, and
    // walks back to it once free: let go, it joined the home pocket at the main while a Rifleman
    // from the main took its post.
    let tick = observation.tick;
    let returning = !super::off("postreturn");
    let away_before = memory.jeff_v1_1.natural.away.clone();
    let away_ok = |id: &u32| {
        returning
            && defending.contains(id)
            && by_id.contains_key(id)
            && away_before
                .get(id)
                .is_none_or(|since| tick.saturating_sub(*since) < AWAY_TICKS)
    };
    let mut members: Vec<u32> = memory
        .jeff_v1_1
        .natural
        .support
        .keys()
        .copied()
        .filter(|id| by_id.get(id).is_some_and(|unit| free(unit)) || away_ok(id))
        .collect();
    let away: BTreeMap<u32, u32> = members
        .iter()
        .filter(|id| defending.contains(id))
        .map(|id| (*id, away_before.get(id).copied().unwrap_or(tick)))
        .collect();
    let to_natural = |id: &u32| {
        by_id
            .get(id)
            .map_or(f32::MAX, |unit| dist2(unit.x, unit.y, center.0, center.1))
    };
    // The home guns nearest the main stay; the rest may go.
    let mut guns: Vec<u32> = home_guns
        .iter()
        .copied()
        .filter(|id| by_id.get(id).is_some_and(|unit| free(unit)))
        .collect();
    guns.sort_by(|a, b| {
        let to_main = |id: &u32| {
            by_id
                .get(id)
                .map_or(f32::MAX, |unit| dist2(unit.x, unit.y, main.0, main.1))
        };
        to_main(a).total_cmp(&to_main(b)).then(a.cmp(b))
    });
    let spare_guns: Vec<u32> = guns.into_iter().skip(HOME_GUNS_KEPT).collect();
    let mut riflemen: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && free(unit))
        .filter(|unit| split || dist2(unit.x, unit.y, main.0, main.1) <= squared(HOME_TILES * ts))
        .map(|unit| unit.id)
        .collect();
    riflemen.sort_by(|a, b| to_natural(a).total_cmp(&to_natural(b)).then(a.cmp(b)));
    let support = support_size(profile_id);
    for id in spare_guns.into_iter().chain(riflemen) {
        if members.len() >= support {
            break;
        }
        if !members.contains(&id) {
            members.push(id);
        }
    }
    let previous = std::mem::take(&mut memory.jeff_v1_1.natural.support);
    let previous_seats = std::mem::take(&mut memory.jeff_v1_1.natural.seats);
    memory.jeff_v1_1.natural.away = away;
    if members.is_empty() {
        return;
    }
    // Posts across the way in, assigned left to right so no two cross.
    let across = (-direction.1, direction.0);
    let count = members.len();
    let posts: Vec<(f32, f32)> = (0..count)
        .map(|index| {
            let (rank, slot) = (index / RANK_WIDTH, index % RANK_WIDTH);
            let width = (count - rank * RANK_WIDTH).min(RANK_WIDTH);
            let side = (slot as f32 - (width as f32 - 1.0) * 0.5) * LINE_SPACING_TILES;
            let mut ahead = LINE_FORWARD_TILES - rank as f32 * RANK_BACK_TILES;
            loop {
                let point = clamp_to_map(
                    (
                        center.0 + (direction.0 * ahead + across.0 * side) * ts,
                        center.1 + (direction.1 * ahead + across.1 * side) * ts,
                    ),
                    observation.map,
                );
                if ahead <= 2.0
                    || defense::defensive_position_is_open(
                        observation,
                        map_analysis,
                        point.0,
                        point.1,
                    )
                {
                    break point;
                }
                ahead -= 1.0;
            }
        })
        .collect();
    let lateral = |id: &u32| {
        by_id.get(id).map_or(0.0, |unit| {
            (unit.x - center.0) * across.0 + (unit.y - center.1) * across.1
        })
    };
    members.sort_by(|a, b| lateral(a).total_cmp(&lateral(b)).then(a.cmp(b)));
    let seated: Vec<(u32, (f32, f32))> = if super::off("natseat") {
        members.iter().copied().zip(posts).collect()
    } else {
        stable_seats(&members, &posts, &by_id, &previous_seats, ts)
    };
    for (id, post) in &seated {
        memory
            .jeff_v1_1
            .natural
            .seats
            .insert(*id, (post.0.round() as i32, post.1.round() as i32));
    }
    let charging = memory.jeff_v1_1.charge.riflemen.clone();
    let mut sent = Vec::new();
    for (id, post) in seated.iter().map(|(id, post)| (id, *post)) {
        let last = previous.get(id).copied().unwrap_or(0);
        memory.jeff_v1_1.natural.support.insert(*id, last);
        let Some(unit) = by_id.get(id) else {
            continue;
        };
        let fighting = observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && enemy.kind.is_unit()
                && dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(FIGHT_TILES * ts)
        });
        // Away with home defense: its post waits for it.
        if charging.contains(id) || defending.contains(id) {
            continue;
        }
        if fighting {
            // A Rifleman not shooting closes on the nearest enemy (`natfight`). Held on its post
            // it stood at the edge of its reach while Panzerfausts, with the same 5 tiles, and a
            // Tank shot the line apart: The River against Pressure AI, 19 Riflemen to 6 in 20 s.
            let idle_under_fire = !super::off("natfight")
                && unit.kind == EntityKind::Rifleman
                && unit.state != AiEntityState::Attack
                && tick.saturating_sub(last) >= FIGHT_REORDER_TICKS;
            let nearest = observation
                .visible_enemies
                .iter()
                .filter(|enemy| {
                    enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker
                })
                .min_by(|a, b| {
                    dist2(a.x, a.y, unit.x, unit.y)
                        .total_cmp(&dist2(b.x, b.y, unit.x, unit.y))
                        .then(a.id.cmp(&b.id))
                })
                .filter(|enemy| {
                    dist2(enemy.x, enemy.y, unit.x, unit.y) <= squared(FIGHT_TILES * ts)
                });
            if let Some(enemy) = nearest.filter(|_| idle_under_fire) {
                if actions::attack_move_units(actions, [*id], enemy.x, enemy.y).is_some() {
                    memory.jeff_v1_1.natural.support.insert(*id, tick);
                    sent.push(*id);
                }
            }
            continue;
        }
        let on_post = dist2(unit.x, unit.y, post.0, post.1) <= squared(ON_POST_TILES * ts);
        if on_post {
            if unit.kind == EntityKind::Rifleman && unit.state != AiEntityState::Idle {
                actions::hold_position_units(actions, [*id]);
            }
            continue;
        }
        if previous.contains_key(id) && tick.saturating_sub(last) < REORDER_TICKS {
            continue;
        }
        actions::attack_move_units(actions, [*id], post.0, post.1);
        memory.jeff_v1_1.natural.support.insert(*id, tick);
        sent.push(*id);
    }
    if !sent.is_empty() {
        intents.push(AiIntent::Assemble { units: sent });
    }
}

/// A supporter whose last post moved no further than this keeps it (`natseat`).
const KEEP_SEAT_TILES: f32 = 1.5;

/// Each supporter's post (`natseat`): its last one where a post still stands near it, else the post
/// it stands on, and the rest left to right. Dealt left to right afresh every decision, the posts
/// went round the line as the Riflemen walked: in eight traces against Rush AI 60-70% of the
/// support's orders sent a Rifleman to another post than its last, and the line never dug in.
fn stable_seats(
    members: &[u32],
    posts: &[(f32, f32)],
    by_id: &BTreeMap<u32, &AiEntitySummary>,
    previous: &BTreeMap<u32, (i32, i32)>,
    ts: f32,
) -> Vec<(u32, (f32, f32))> {
    let mut open: Vec<Option<(f32, f32)>> = posts.iter().copied().map(Some).collect();
    let mut seated: Vec<(u32, (f32, f32))> = Vec::new();
    let claim = |open: &mut Vec<Option<(f32, f32)>>, from: (f32, f32), within: f32| {
        let best = open
            .iter()
            .enumerate()
            .filter_map(|(index, post)| post.map(|post| (index, post)))
            .filter(|(_, post)| dist2(post.0, post.1, from.0, from.1) <= squared(within * ts))
            .min_by(|a, b| {
                dist2(a.1 .0, a.1 .1, from.0, from.1)
                    .total_cmp(&dist2(b.1 .0, b.1 .1, from.0, from.1))
                    .then(a.0.cmp(&b.0))
            })?;
        open[best.0] = None;
        Some(best.1)
    };
    let mut left: Vec<u32> = Vec::new();
    for id in members {
        let kept = previous
            .get(id)
            .and_then(|at| claim(&mut open, (at.0 as f32, at.1 as f32), KEEP_SEAT_TILES));
        match kept {
            Some(post) => seated.push((*id, post)),
            None => left.push(*id),
        }
    }
    let mut rest: Vec<u32> = Vec::new();
    for id in left {
        let on = by_id
            .get(&id)
            .and_then(|unit| claim(&mut open, (unit.x, unit.y), ON_POST_TILES));
        match on {
            Some(post) => seated.push((id, post)),
            None => rest.push(id),
        }
    }
    // `members` is already left to right, and so are the posts within each rank.
    for (id, post) in rest.into_iter().zip(open.into_iter().flatten()) {
        seated.push((id, post));
    }
    seated
}

/// Extra room kept between Jeff's units and the natural's Depot footprint (`siteclear`).
const SITE_CLEAR_TILES: f32 = 1.0;

/// Moves Jeff's own units off the natural's Depot footprint while an Engineer is on its way to build
/// it (`siteclear`): the order fails with a unit standing there. On Classic against Rush AI the third
/// scouts' rear Rifleman waited on a waypoint in the middle of the site and home defense rallied 12
/// Riflemen onto its edge; the 6:56 order failed and the natural never stood.
pub(in crate::ai_core::decision) fn clear_site(
    (actions, observation, _map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &AiDecisionMemory,
    profile_id: &str,
) {
    if !is_jeff_v1_1(profile_id) || super::off("siteclear") {
        return;
    }
    let Some(site) = memory.expansion_security.site else {
        return;
    };
    if !memory.expansion_security.attempting() {
        return;
    }
    let Some(stats) = config::building_stats(EntityKind::ResourceDepot) else {
        return;
    };
    let ts = observation.map.tile_size as f32;
    let rect = (
        site.0 as f32 * ts,
        site.1 as f32 * ts,
        site.0.saturating_add(stats.foot_w) as f32 * ts,
        site.1.saturating_add(stats.foot_h) as f32 * ts,
    );
    let inside = |x: f32, y: f32| x >= rect.0 && x <= rect.2 && y >= rect.1 && y <= rect.3;
    if observation
        .owned
        .iter()
        .any(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0 && inside(unit.x, unit.y))
    {
        return;
    }
    let mut moved = Vec::new();
    for unit in observation
        .owned
        .iter()
        .filter(|unit| unit.kind.is_unit() && unit.kind != EntityKind::Worker && unit.hp > 0)
    {
        let room = rts_rules::balance::unit_placement_radius(unit.kind) + SITE_CLEAR_TILES * ts;
        let grown = (rect.0 - room, rect.1 - room, rect.2 + room, rect.3 + room);
        if unit.x <= grown.0 || unit.x >= grown.2 || unit.y <= grown.1 || unit.y >= grown.3 {
            continue;
        }
        // Out over the nearest edge, a quarter tile past the room kept.
        let step = 0.25 * ts;
        let exits = [
            (unit.x - grown.0, (grown.0 - step, unit.y)),
            (grown.2 - unit.x, (grown.2 + step, unit.y)),
            (unit.y - grown.1, (unit.x, grown.1 - step)),
            (grown.3 - unit.y, (unit.x, grown.3 + step)),
        ];
        let Some((_, out)) = exits
            .iter()
            .map(|(gap, point)| (*gap, clamp_to_map(*point, observation.map)))
            .filter(|(_, point)| {
                point.0 <= grown.0 || point.0 >= grown.2 || point.1 <= grown.1 || point.1 >= grown.3
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
        else {
            continue;
        };
        if let Some(units) = actions::move_units(actions, [unit.id], out.0, out.1) {
            moved.extend(units);
        }
    }
    if !moved.is_empty() {
        intents.push(AiIntent::Move { units: moved });
    }
}
