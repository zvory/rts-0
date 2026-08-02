//! Canonical constants and deterministic geometry for the fixed-roster Hellhole workload.

use rts_rules::balance::{TANK_BODY_CLEARANCE_PX, TANK_BODY_LENGTH_PX, TANK_BODY_WIDTH_PX};
use rts_sim::game::entity::EntityKind;

pub const SCENARIO_ID: &str = "fixed-roster-hellhole";
pub const SEED: u32 = 0x5a00_0300;
pub const TILE: f32 = 32.0;
pub const CENTER_TILE: i32 = 63;
pub const CENTER: (f32, f32) = (CENTER_TILE as f32 * TILE, CENTER_TILE as f32 * TILE);
pub const LEG_TICKS: u32 = 900;
pub const COMMAND_INTERVAL_TICKS: u32 = 30;
pub const SHUTTLE_UNIT_COUNT: usize = 77;
pub const SHUTTLE_SELECTION_COUNT: usize = 43;
pub const SHUTTLE_OFFSET_TILES: i32 = 18;
pub const SCATTERED_TANK_TRAP_COUNT: usize = 120;
pub const INITIAL_ENTITY_COUNT: usize = 468;
pub const RESPAWN_CANDIDATE_COLUMNS: usize = 28;
pub const RESPAWN_CANDIDATE_ROWS: usize = 18;
pub const RESPAWN_CANDIDATE_LIMIT: usize = RESPAWN_CANDIDATE_COLUMNS * RESPAWN_CANDIDATE_ROWS;

const RESPAWN_CANDIDATE_GAP_PX: f32 = 2.0;

/// Stable per-army unit mix for performance comparisons.
///
/// These authored counts, rather than current balance supply costs, own the benchmark load. A
/// normal supply rebalance must therefore leave entity count, terrain, checkpoint, and stream
/// inputs unchanged.
pub const FIXED_ROSTER_COUNTS: &[(EntityKind, usize)] = &[
    (EntityKind::Worker, 1),
    (EntityKind::Golem, 1),
    (EntityKind::Rifleman, 8),
    (EntityKind::MachineGunner, 8),
    (EntityKind::Panzerfaust, 8),
    (EntityKind::AntiTankGun, 8),
    (EntityKind::MortarTeam, 8),
    (EntityKind::Artillery, 1),
    (EntityKind::ScoutCar, 9),
    (EntityKind::Tank, 16),
    (EntityKind::CommandCar, 9),
];

pub fn fixed_roster_composition() -> Vec<EntityKind> {
    let required = [
        EntityKind::Worker,
        EntityKind::Golem,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::Panzerfaust,
        EntityKind::AntiTankGun,
        EntityKind::MortarTeam,
        EntityKind::Artillery,
        EntityKind::ScoutCar,
        EntityKind::Tank,
        EntityKind::CommandCar,
    ];
    let repeated_mix = [
        EntityKind::Tank,
        EntityKind::Tank,
        EntityKind::ScoutCar,
        EntityKind::CommandCar,
        EntityKind::MachineGunner,
        EntityKind::MortarTeam,
        EntityKind::AntiTankGun,
        EntityKind::Rifleman,
        EntityKind::Panzerfaust,
    ];
    let tail = [
        EntityKind::Tank,
        EntityKind::ScoutCar,
        EntityKind::CommandCar,
    ];
    required
        .into_iter()
        .chain(
            repeated_mix
                .into_iter()
                .cycle()
                .take(repeated_mix.len() * 7),
        )
        .chain(tail)
        .collect()
}

pub fn shuttle_endpoint(player_id: u32, phase: u32) -> (i32, i32) {
    let endpoint_a = match player_id {
        3 => (
            CENTER_TILE + SHUTTLE_OFFSET_TILES,
            CENTER_TILE - SHUTTLE_OFFSET_TILES,
        ),
        4 => (
            CENTER_TILE - SHUTTLE_OFFSET_TILES,
            CENTER_TILE - SHUTTLE_OFFSET_TILES,
        ),
        _ => (CENTER_TILE, CENTER_TILE),
    };
    let endpoint_b = match player_id {
        3 => (
            CENTER_TILE - SHUTTLE_OFFSET_TILES,
            CENTER_TILE + SHUTTLE_OFFSET_TILES,
        ),
        4 => (
            CENTER_TILE + SHUTTLE_OFFSET_TILES,
            CENTER_TILE + SHUTTLE_OFFSET_TILES,
        ),
        _ => (CENTER_TILE, CENTER_TILE),
    };
    if phase.is_multiple_of(2) {
        endpoint_b
    } else {
        endpoint_a
    }
}

pub fn hash_words(words: &[u32]) -> u32 {
    words
        .iter()
        .copied()
        .fold(SEED ^ 0x9e37_79b9, |hash, word| {
            let mixed = word.wrapping_mul(0x85eb_ca6b).rotate_left(13);
            (hash ^ mixed).wrapping_mul(0xc2b2_ae35).rotate_left(15)
        })
}

pub fn respawn_candidates() -> Vec<(f32, f32)> {
    let spacing_x = TANK_BODY_LENGTH_PX + TANK_BODY_CLEARANCE_PX * 2.0 + RESPAWN_CANDIDATE_GAP_PX;
    let spacing_y = TANK_BODY_WIDTH_PX + TANK_BODY_CLEARANCE_PX * 2.0 + RESPAWN_CANDIDATE_GAP_PX;
    let width = (RESPAWN_CANDIDATE_COLUMNS - 1) as f32 * spacing_x;
    let height = (RESPAWN_CANDIDATE_ROWS - 1) as f32 * spacing_y;
    let mut candidates = Vec::with_capacity(RESPAWN_CANDIDATE_LIMIT);
    for row in 0..RESPAWN_CANDIDATE_ROWS {
        for column in 0..RESPAWN_CANDIDATE_COLUMNS {
            let x = CENTER.0 - width * 0.5 + column as f32 * spacing_x;
            let y = CENTER.1 - height * 0.5 + row as f32 * spacing_y;
            candidates.push((x, y));
        }
    }
    candidates.sort_by(|a, b| {
        let a_distance = (a.0 - CENTER.0).powi(2) + (a.1 - CENTER.1).powi(2);
        let b_distance = (b.0 - CENTER.0).powi(2) + (b.1 - CENTER.1).powi(2);
        a_distance
            .total_cmp(&b_distance)
            .then_with(|| a.1.total_cmp(&b.1))
            .then_with(|| a.0.total_cmp(&b.0))
    });
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_composition_keeps_authored_unit_count_contract() {
        let composition = fixed_roster_composition();
        assert_eq!(composition.len(), SHUTTLE_UNIT_COUNT);
        assert!(composition.iter().all(|kind| kind.is_unit()));
        assert_eq!(
            FIXED_ROSTER_COUNTS
                .iter()
                .map(|(_, count)| count)
                .sum::<usize>(),
            77
        );
        for &(kind, expected) in FIXED_ROSTER_COUNTS {
            assert_eq!(
                composition
                    .iter()
                    .filter(|candidate| **candidate == kind)
                    .count(),
                expected,
                "fixed count for {kind}"
            );
        }
    }
}
