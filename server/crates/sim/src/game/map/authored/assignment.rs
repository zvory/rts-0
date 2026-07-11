use std::collections::HashSet;

use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::game::map::{StartAssignmentPlayer, Tile};

pub(super) fn assign_start_locations(
    locations: &[Tile],
    players: &[StartAssignmentPlayer],
    seed: u32,
) -> Result<Vec<Tile>, String> {
    if locations.len() < players.len() {
        return Err("map has no assignable start locations".to_string());
    }
    if all_singleton_teams(players) {
        let mut shuffled = locations.to_vec();
        let mut rng = SmallRng::seed_from_u64(seed as u64);
        shuffled.shuffle(&mut rng);
        shuffled.truncate(players.len());
        return Ok(shuffled);
    }

    let mut best: Option<(AssignmentScore, Vec<Tile>)> = None;
    let mut selected = Vec::with_capacity(players.len());
    let mut used = vec![false; locations.len()];
    evaluate_start_orders(
        locations,
        players,
        seed,
        &mut selected,
        &mut used,
        &mut best,
    );
    best.map(|(_, starts)| starts)
        .ok_or_else(|| "map has no assignable start locations".to_string())
}

fn all_singleton_teams(players: &[StartAssignmentPlayer]) -> bool {
    let mut seen = HashSet::with_capacity(players.len());
    players
        .iter()
        .all(|player| player.team_id != 0 && seen.insert(player.team_id))
}

fn evaluate_start_orders(
    locations: &[Tile],
    players: &[StartAssignmentPlayer],
    seed: u32,
    selected: &mut Vec<usize>,
    used: &mut [bool],
    best: &mut Option<(AssignmentScore, Vec<Tile>)>,
) {
    if selected.len() == players.len() {
        let score = score_assignment(selected, locations, players, seed);
        if best
            .as_ref()
            .map(|(current, _)| score < *current)
            .unwrap_or(true)
        {
            *best = Some((
                score,
                selected.iter().map(|&index| locations[index]).collect(),
            ));
        }
        return;
    }
    for index in 0..locations.len() {
        if used[index] {
            continue;
        }
        used[index] = true;
        selected.push(index);
        evaluate_start_orders(locations, players, seed, selected, used, best);
        selected.pop();
        used[index] = false;
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
    start_order: &[usize],
    locations: &[Tile],
    players: &[StartAssignmentPlayer],
    seed: u32,
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
            let dist = distance_sq(locations[start_order[i]], locations[start_order[j]]);
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
        tie_break: assignment_tie_break(start_order, players, seed),
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
    start_order: &[usize],
    players: &[StartAssignmentPlayer],
    seed: u32,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in seed.to_le_bytes() {
        hash = fnv_step(hash, byte);
    }
    for (player, start) in players.iter().zip(start_order.iter()) {
        for byte in player.id.to_le_bytes() {
            hash = fnv_step(hash, byte);
        }
        for byte in player.team_id.to_le_bytes() {
            hash = fnv_step(hash, byte);
        }
        for byte in (*start as u64).to_le_bytes() {
            hash = fnv_step(hash, byte);
        }
    }
    hash
}

fn fnv_step(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
}
