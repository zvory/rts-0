//! Balance & simulation constants. See `DESIGN.md` §5.
//!
//! Authoritative source of game balance. `client/src/config.js` mirrors the subset the
//! UI / rendering / fog overlay needs (costs, supply, sight, sizes, colors). Keep both
//! in sync; when you change a number here that the UI shows, change it there too.

use crate::protocol::kinds;

// --- Timing -----------------------------------------------------------------
pub const TICK_HZ: u32 = 30;
pub const TICK_MS: u64 = 1000 / TICK_HZ as u64;
pub const SNAPSHOT_EVERY_N_TICKS: u32 = 1;

// --- Map --------------------------------------------------------------------
pub const TILE_SIZE: u32 = 32;

/// Map size (in tiles) for a given player count. Square, symmetric.
pub const fn map_size_for(players: usize) -> u32 {
    if players <= 2 {
        64
    } else {
        96
    }
}

// --- Economy ----------------------------------------------------------------
pub const STARTING_MINERALS: u32 = 50;
pub const STARTING_GAS: u32 = 0;
pub const STARTING_WORKERS: u32 = 4;

pub const MINERAL_LOAD: u32 = 5;
pub const GAS_LOAD: u32 = 4;
pub const HARVEST_TICKS: u32 = 20;
pub const MINERAL_PATCH_AMOUNT: u32 = 1500;
pub const GAS_GEYSER_AMOUNT: u32 = 5000;
pub const MINERAL_PATCHES_PER_BASE: u32 = 8;

// --- Supply -----------------------------------------------------------------
pub const HQ_SUPPLY: u32 = 10;
pub const DEPOT_SUPPLY: u32 = 8;
pub const SUPPLY_CAP_MAX: u32 = 200;

// --- Stats ------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct UnitStats {
    pub hp: u32,
    pub dmg: u32,
    pub range_tiles: u32,
    pub cooldown: u32, // ticks between attacks
    pub speed: f32,    // world px per tick
    pub sight_tiles: u32,
    pub cost_min: u32,
    pub cost_gas: u32,
    pub supply: u32,
    pub build_ticks: u32,
    pub radius: f32, // collision / render radius in world px
}

#[derive(Debug, Clone, Copy)]
pub struct BuildingStats {
    pub hp: u32,
    pub sight_tiles: u32,
    pub cost_min: u32,
    pub cost_gas: u32,
    pub foot_w: u32, // footprint in tiles
    pub foot_h: u32,
    pub build_ticks: u32,
    pub provides_supply: u32,
    // Defensive attack (turret). dmg == 0 means the building does not attack.
    pub dmg: u32,
    pub range_tiles: u32,
    pub cooldown: u32,
}

/// Stats for a unit kind, or `None` if `kind` is not a unit.
pub fn unit_stats(kind: &str) -> Option<UnitStats> {
    let s = match kind {
        kinds::WORKER => UnitStats {
            hp: 40,
            dmg: 4,
            range_tiles: 1,
            cooldown: 12,
            speed: 3.0,
            sight_tiles: 7,
            cost_min: 50,
            cost_gas: 0,
            supply: 1,
            build_ticks: 120,
            radius: 9.0,
        },
        kinds::SOLDIER => UnitStats {
            hp: 45,
            dmg: 5,
            range_tiles: 4,
            cooldown: 8,
            speed: 3.2,
            sight_tiles: 8,
            cost_min: 50,
            cost_gas: 0,
            supply: 1,
            build_ticks: 150,
            radius: 9.0,
        },
        kinds::HEAVY => UnitStats {
            hp: 130,
            dmg: 20,
            range_tiles: 3,
            cooldown: 18,
            speed: 2.0,
            sight_tiles: 7,
            cost_min: 100,
            cost_gas: 50,
            supply: 2,
            build_ticks: 250,
            radius: 13.0,
        },
        _ => return None,
    };
    Some(s)
}

/// Stats for a building kind, or `None` if `kind` is not a building.
pub fn building_stats(kind: &str) -> Option<BuildingStats> {
    let s = match kind {
        kinds::HQ => BuildingStats {
            hp: 600,
            sight_tiles: 9,
            cost_min: 400,
            cost_gas: 0,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 400,
            provides_supply: HQ_SUPPLY,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        kinds::DEPOT => BuildingStats {
            hp: 220,
            sight_tiles: 4,
            cost_min: 50,
            cost_gas: 0,
            foot_w: 2,
            foot_h: 2,
            build_ticks: 120,
            provides_supply: DEPOT_SUPPLY,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        kinds::BARRACKS => BuildingStats {
            hp: 320,
            sight_tiles: 6,
            cost_min: 100,
            cost_gas: 0,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 200,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        kinds::TURRET => BuildingStats {
            hp: 200,
            sight_tiles: 6,
            cost_min: 75,
            cost_gas: 0,
            foot_w: 1,
            foot_h: 1,
            build_ticks: 120,
            provides_supply: 0,
            dmg: 10,
            range_tiles: 7,
            cooldown: 10,
        },
        _ => return None,
    };
    Some(s)
}

/// Which units a given building can train.
pub fn trainable_units(building_kind: &str) -> &'static [&'static str] {
    match building_kind {
        kinds::HQ => &[kinds::WORKER],
        kinds::BARRACKS => &[kinds::SOLDIER, kinds::HEAVY],
        _ => &[],
    }
}

/// Whether `building_kind` is allowed to be placed given the set of building kinds the
/// player already owns (tech requirements). Barracks requires an existing HQ.
pub fn build_requirement_met(building_kind: &str, owned_building_kinds: &[&str]) -> bool {
    match building_kind {
        kinds::BARRACKS => owned_building_kinds.contains(&kinds::HQ),
        _ => true,
    }
}

/// Resource node starting amount for a node kind (`minerals` | `gas`).
pub fn node_amount(kind: &str) -> u32 {
    match kind {
        kinds::MINERALS => MINERAL_PATCH_AMOUNT,
        kinds::GAS => GAS_GEYSER_AMOUNT,
        _ => 0,
    }
}
