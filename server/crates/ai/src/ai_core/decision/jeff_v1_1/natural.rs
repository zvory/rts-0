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
/// Supporters to a rank of the line, later ranks this far behind: the Rifleman opening's twelve
/// stand two deep instead of 30 tiles wide.
const RANK_WIDTH: usize = 6;
const RANK_BACK_TILES: f32 = 2.0;

/// How many units support the natural: `SUPPORT`, or under the Rifleman opening (`riflenat`) all of
/// its holders but the two guards.
fn support_size(profile_id: &str) -> usize {
    if super::rifle_opening(profile_id) {
        super::natural_rifles().saturating_sub(2)
    } else {
        SUPPORT
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

/// V1.1's natural support and when each was last sent to its post.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct NaturalMemory {
    pub(in crate::ai_core::decision) support: BTreeMap<u32, u32>,
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

/// Picks and posts the natural's support (see the module docs). `home_guns`: the main's defensive
/// guns; `held`: units other systems keep this decision.
pub(in crate::ai_core::decision) fn support(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    home_guns: &[u32],
    held: &BTreeSet<u32>,
) {
    let ts = observation.map.tile_size as f32;
    let site = memory.expansion_security.site;
    // While enemy infantry holds the natural, its clearing group (`natclear`) fights for it: the
    // support walked to posts in among the enemy, one or two at a time.
    let active = is_jeff_v1_1(profile_id)
        && !super::off("natsupport")
        && !memory.expansion_security.riflemen.is_empty()
        && !(memory.jeff_v1_1.natclear.active() && !super::off("natclear"));
    let Some(center) = site
        .filter(|_| active)
        .and_then(|site| building_center(site, EntityKind::ResourceDepot, observation.map.tile_size))
    else {
        memory.jeff_v1_1.natural.support.clear();
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
    };
    let by_id: BTreeMap<u32, &AiEntitySummary> =
        observation.owned.iter().map(|unit| (unit.id, unit)).collect();
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
        free(unit) && (guns_go || unit.kind != EntityKind::MachineGunner)
    };
    // Keep the support it has while each is still free.
    let mut members: Vec<u32> = memory
        .jeff_v1_1
        .natural
        .support
        .keys()
        .copied()
        .filter(|id| by_id.get(id).is_some_and(|unit| free(unit)))
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
        .filter(|unit| dist2(unit.x, unit.y, main.0, main.1) <= squared(HOME_TILES * ts))
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
                    || defense::defensive_position_is_open(observation, map_analysis, point.0, point.1)
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
    let tick = observation.tick;
    let charging = memory.jeff_v1_1.charge.riflemen.clone();
    let mut sent = Vec::new();
    for (id, post) in members.iter().zip(posts) {
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
        if fighting || charging.contains(id) {
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
