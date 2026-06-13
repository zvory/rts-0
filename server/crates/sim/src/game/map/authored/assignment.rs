use std::collections::{HashMap, HashSet};

use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use super::{parse_layout_pairs, AuthoredLayout, AuthoredSite};
use crate::game::map::{BaseSlot, StartAssignmentPlayer, Tile};

pub(super) fn assign_layout_slots(
    matching_layouts: &[&AuthoredLayout],
    sites: &HashMap<String, AuthoredSite>,
    players: &[StartAssignmentPlayer],
    seed: u32,
) -> Result<Vec<BaseSlot>, String> {
    if all_singleton_teams(players) {
        let layout = matching_layouts[(seed as usize) % matching_layouts.len()];
        let mut slots = parse_layout_pairs(layout, sites)?;
        let mut rng = SmallRng::seed_from_u64(seed as u64);
        slots.shuffle(&mut rng);
        return Ok(slots);
    }

    let mut best: Option<(AssignmentScore, Vec<BaseSlot>)> = None;
    for (layout_index, layout) in matching_layouts.iter().enumerate() {
        let slots = parse_layout_pairs(layout, sites)?;
        let mut slot_order: Vec<usize> = (0..slots.len()).collect();
        evaluate_slot_orders(
            &mut slot_order,
            0,
            &slots,
            players,
            seed,
            layout_index,
            &mut best,
        );
    }

    best.map(|(_, slots)| slots)
        .ok_or_else(|| "map has no assignable spawn layout".to_string())
}

fn all_singleton_teams(players: &[StartAssignmentPlayer]) -> bool {
    let mut seen = HashSet::with_capacity(players.len());
    players
        .iter()
        .all(|player| player.team_id != 0 && seen.insert(player.team_id))
}

fn evaluate_slot_orders(
    slot_order: &mut [usize],
    fixed: usize,
    slots: &[BaseSlot],
    players: &[StartAssignmentPlayer],
    seed: u32,
    layout_index: usize,
    best: &mut Option<(AssignmentScore, Vec<BaseSlot>)>,
) {
    if fixed == slot_order.len() {
        let score = score_assignment(slot_order, slots, players, seed, layout_index);
        let should_replace = best
            .as_ref()
            .map(|(current, _)| score < *current)
            .unwrap_or(true);
        if should_replace {
            let assigned = slot_order
                .iter()
                .map(|&index| slots[index].clone())
                .collect();
            *best = Some((score, assigned));
        }
        return;
    }

    for index in fixed..slot_order.len() {
        slot_order.swap(fixed, index);
        evaluate_slot_orders(
            slot_order,
            fixed + 1,
            slots,
            players,
            seed,
            layout_index,
            best,
        );
        slot_order.swap(fixed, index);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct AssignmentScore {
    teammate_spread: u64,
    nearest_enemy_distance: i64,
    exposure_imbalance: u64,
    tie_break: u64,
}

fn score_assignment(
    slot_order: &[usize],
    slots: &[BaseSlot],
    players: &[StartAssignmentPlayer],
    seed: u32,
    layout_index: usize,
) -> AssignmentScore {
    let mut teammate_spread = 0u64;
    let mut nearest_enemy_distance_sq = u64::MAX;
    let mut player_nearest_enemies = Vec::with_capacity(players.len());

    for i in 0..players.len() {
        let mut nearest_for_player = u64::MAX;
        for j in 0..players.len() {
            if i == j {
                continue;
            }
            let dist = distance_sq(slots[slot_order[i]].0, slots[slot_order[j]].0);
            if same_team(players[i], players[j]) {
                teammate_spread = teammate_spread.saturating_add(dist);
            } else {
                nearest_enemy_distance_sq = nearest_enemy_distance_sq.min(dist);
                nearest_for_player = nearest_for_player.min(dist);
            }
        }
        if nearest_for_player != u64::MAX {
            player_nearest_enemies.push(nearest_for_player);
        }
    }

    AssignmentScore {
        teammate_spread,
        nearest_enemy_distance: -(nearest_enemy_distance_sq.min(i64::MAX as u64) as i64),
        exposure_imbalance: exposure_imbalance(&player_nearest_enemies),
        tie_break: assignment_tie_break(slot_order, players, seed, layout_index),
    }
}

fn same_team(a: StartAssignmentPlayer, b: StartAssignmentPlayer) -> bool {
    a.team_id != 0 && a.team_id == b.team_id
}

fn distance_sq(a: Tile, b: Tile) -> u64 {
    let dx = i64::from(a.0) - i64::from(b.0);
    let dy = i64::from(a.1) - i64::from(b.1);
    (dx * dx + dy * dy) as u64
}

fn exposure_imbalance(distances: &[u64]) -> u64 {
    let Some(min) = distances.iter().min().copied() else {
        return 0;
    };
    let Some(max) = distances.iter().max().copied() else {
        return 0;
    };
    max.saturating_sub(min)
}

fn assignment_tie_break(
    slot_order: &[usize],
    players: &[StartAssignmentPlayer],
    seed: u32,
    layout_index: usize,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in seed.to_le_bytes() {
        hash = fnv_step(hash, byte);
    }
    for byte in (layout_index as u64).to_le_bytes() {
        hash = fnv_step(hash, byte);
    }
    for (player, slot) in players.iter().zip(slot_order.iter()) {
        for byte in player.id.to_le_bytes() {
            hash = fnv_step(hash, byte);
        }
        for byte in player.team_id.to_le_bytes() {
            hash = fnv_step(hash, byte);
        }
        for byte in (*slot as u64).to_le_bytes() {
            hash = fnv_step(hash, byte);
        }
    }
    hash
}

fn fnv_step(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
}
