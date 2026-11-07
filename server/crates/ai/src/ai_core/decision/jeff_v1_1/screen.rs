//! Jeff's AI V1.1 spreads its push's rifle screen and stands it further out. A Tank's shell on
//! infantry splashes everyone within 1.4 tiles: V1.0's screen, two tiles apart with its ranks 1.5
//! tiles behind each other, stood within 1.4 tiles of another of Jeff's infantry in 49% of samples
//! near enemy Tanks (median gap 1.4 tiles), and two tiles in front of the lead Tank it saw only
//! 13 tiles out, inside a parked Tank's 14. Spread `SPACING_TILES` apart, its later ranks
//! `RANK_BACK_TILES` behind, one shell takes one Rifleman; `FORWARD_TILES` ahead, it spots
//! enemy Tanks 15 tiles out and the push halts and parks before they close.

use super::*;

const FORWARD_TILES: f32 = 4.0;
const SPACING_TILES: f32 = 3.0;
const RANK_BACK_TILES: f32 = 3.0;
const FIRST_RANK: usize = 4;
const LATER_RANK: usize = 5;

/// `column`: the push marches and holds Riflemen first, the Scout Car just behind them and the
/// Tanks at the rear. The Riflemen's rearmost rank stands `COLUMN_REAR_TILES` ahead of the front
/// Tanks and each further rank `COLUMN_RANK_TILES` beyond it, `COLUMN_WIDTH` abreast and
/// `SPACING_TILES` apart, so no shell splashes two; the Scout Car keeps `COLUMN_SCOUT_TILES`
/// ahead of the Tanks. With Methamphetamines the Riflemen walk as fast as the Tanks.
const COLUMN_REAR_TILES: f32 = 4.5;
const COLUMN_RANK_TILES: f32 = 2.5;
const COLUMN_WIDTH: usize = 6;
const COLUMN_SCOUT_TILES: f32 = 2.0;
/// The column's Riflemen stay with the push within this many tiles of its Tanks' centre: its
/// deepest rank, a Tank rank's depth and room to fall behind.
pub(in crate::ai_core::decision) const COLUMN_KEEP_TILES: f32 = 20.0;

/// Whether this profile spreads its rifle screen.
pub(in crate::ai_core::decision) fn spread(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("spread")
}

/// Whether the push marches Riflemen first, then the Scout Car, then the Tanks.
pub(in crate::ai_core::decision) fn column(memory: &AiDecisionMemory) -> bool {
    memory.profile_id().is_some_and(is_jeff_v1_1) && !super::off("column")
}

/// The column's Rifle posts ahead of the front Tanks at `tank_anchor`, rearmost rank first.
pub(in crate::ai_core::decision) fn column_points(
    tank_anchor: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    count: usize,
) -> Vec<(f32, f32)> {
    let Some(direction) = normalized_direction(tank_anchor, objective) else {
        return Vec::new();
    };
    let ts = map.tile_size as f32;
    let across = (-direction.1, direction.0);
    (0..count)
        .map(|index| {
            let rank = index / COLUMN_WIDTH;
            let width = (count - rank * COLUMN_WIDTH).min(COLUMN_WIDTH);
            let slot = index % COLUMN_WIDTH;
            let side = (slot as f32 - (width as f32 - 1.0) * 0.5) * SPACING_TILES * ts;
            let ahead = (COLUMN_REAR_TILES + COLUMN_RANK_TILES * rank as f32) * ts;
            clamp_to_map(
                (
                    tank_anchor.0 + direction.0 * ahead + across.0 * side,
                    tank_anchor.1 + direction.1 * ahead + across.1 * side,
                ),
                map,
            )
        })
        .collect()
}

/// The column's Scout Car post: between the front Tanks and the Riflemen's rearmost rank.
pub(in crate::ai_core::decision) fn column_scout(
    tank_anchor: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
) -> Option<(f32, f32)> {
    let direction = normalized_direction(tank_anchor, objective)?;
    let ahead = COLUMN_SCOUT_TILES * map.tile_size as f32;
    Some(clamp_to_map(
        (
            tank_anchor.0 + direction.0 * ahead,
            tank_anchor.1 + direction.1 * ahead,
        ),
        map,
    ))
}

/// The screen's posts: a first rank of `FIRST_RANK` `FORWARD_TILES` ahead of `tank_anchor` toward
/// `objective`, later ranks of `LATER_RANK` each `RANK_BACK_TILES` further back, so no post stands
/// within `SPACING_TILES` of another.
pub(in crate::ai_core::decision) fn points(
    tank_anchor: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    count: usize,
) -> Vec<(f32, f32)> {
    let Some(direction) = normalized_direction(tank_anchor, objective) else {
        return Vec::new();
    };
    let ts = map.tile_size as f32;
    let across = (-direction.1, direction.0);
    let center = (
        tank_anchor.0 + direction.0 * FORWARD_TILES * ts,
        tank_anchor.1 + direction.1 * FORWARD_TILES * ts,
    );
    (0..count)
        .map(|index| {
            let (slot, width, rank) = if index < FIRST_RANK {
                (index, count.min(FIRST_RANK), 0)
            } else {
                let behind = index - FIRST_RANK;
                let start = behind / LATER_RANK * LATER_RANK;
                (
                    behind - start,
                    (count - FIRST_RANK - start).min(LATER_RANK),
                    behind / LATER_RANK + 1,
                )
            };
            let side = (slot as f32 - (width as f32 - 1.0) * 0.5) * SPACING_TILES * ts;
            let back = RANK_BACK_TILES * rank as f32 * ts;
            clamp_to_map(
                (
                    center.0 + across.0 * side - direction.0 * back,
                    center.1 + across.1 * side - direction.1 * back,
                ),
                map,
            )
        })
        .collect()
}
