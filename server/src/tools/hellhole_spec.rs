//! Canonical constants and deterministic geometry for the Supply 300 Hellhole workload.

use rts_protocol::DEFAULT_FACTION_ID;
use rts_rules::balance::{TANK_BODY_CLEARANCE_PX, TANK_BODY_LENGTH_PX, TANK_BODY_WIDTH_PX};
use rts_rules::economy::supply_cost;
use rts_rules::faction::catalog_for;
use rts_sim::game::entity::EntityKind;

pub const SCENARIO_ID: &str = "supply-300-hellhole";
pub const SEED: u32 = 0x5a00_0300;
pub const TILE: f32 = 32.0;
pub const CENTER_TILE: i32 = 63;
pub const CENTER: (f32, f32) = (CENTER_TILE as f32 * TILE, CENTER_TILE as f32 * TILE);
pub const LEG_TICKS: u32 = 900;
pub const COMMAND_INTERVAL_TICKS: u32 = 30;
pub const SHUTTLE_UNIT_COUNT: usize = 85;
pub const SHUTTLE_SELECTION_COUNT: usize = 43;
pub const SHUTTLE_OFFSET_TILES: i32 = 18;
pub const RESPAWN_CANDIDATE_COLUMNS: usize = 28;
pub const RESPAWN_CANDIDATE_ROWS: usize = 18;
pub const RESPAWN_CANDIDATE_LIMIT: usize = RESPAWN_CANDIDATE_COLUMNS * RESPAWN_CANDIDATE_ROWS;

const TARGET_SUPPLY: u32 = 300;
const RESPAWN_CANDIDATE_GAP_PX: f32 = 2.0;

pub fn composition_300_supply() -> Result<Vec<EntityKind>, String> {
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
    let filler = [
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
    let catalog = catalog_for(DEFAULT_FACTION_ID)
        .ok_or_else(|| format!("missing faction catalog {DEFAULT_FACTION_ID}"))?;
    let supply_of = |kind: EntityKind| -> Result<u32, String> {
        if !kind.is_unit() {
            return Err(format!(
                "Hellhole composition contains non-unit kind {kind}"
            ));
        }
        if catalog.allows_unit(kind) {
            Ok(supply_cost(kind))
        } else {
            Ok(0)
        }
    };
    let mut out = required.to_vec();
    let mut supply = out
        .iter()
        .copied()
        .map(&supply_of)
        .sum::<Result<u32, _>>()?;
    if supply > TARGET_SUPPLY {
        return Err(format!(
            "Hellhole required composition uses {supply} supply, above target {TARGET_SUPPLY}"
        ));
    }
    let mut index = 0;
    let mut attempts_without_progress = 0;
    while supply < TARGET_SUPPLY {
        let kind = filler[index % filler.len()];
        index += 1;
        let cost = supply_of(kind)?;
        if cost == 0 || supply + cost > TARGET_SUPPLY {
            attempts_without_progress += 1;
            if attempts_without_progress == filler.len() {
                return Err(format!(
                    "Hellhole composition cannot reach {TARGET_SUPPLY} supply from {supply} with the configured filler units"
                ));
            }
            continue;
        }
        out.push(kind);
        supply += cost;
        attempts_without_progress = 0;
    }
    if out.len() != SHUTTLE_UNIT_COUNT {
        return Err(format!(
            "Hellhole 300-supply composition has {} units, expected {SHUTTLE_UNIT_COUNT}",
            out.len()
        ));
    }
    Ok(out)
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
    fn canonical_composition_keeps_supply_and_unit_count_contracts() {
        let composition = composition_300_supply().expect("canonical composition");
        let catalog = catalog_for(DEFAULT_FACTION_ID).expect("default faction catalog");
        let supply: u32 = composition
            .iter()
            .copied()
            .filter(|kind| catalog.allows_unit(*kind))
            .map(supply_cost)
            .sum();
        assert_eq!(supply, TARGET_SUPPLY);
        assert_eq!(composition.len(), SHUTTLE_UNIT_COUNT);
    }
}
