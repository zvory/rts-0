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
pub const STARTING_STEEL: u32 = 50;
pub const STARTING_OIL: u32 = 0;
pub const STARTING_WORKERS: u32 = 4;

pub const STEEL_LOAD: u32 = 5;
pub const OIL_LOAD: u32 = 4;
pub const HARVEST_TICKS: u32 = 20;
pub const STEEL_PATCH_AMOUNT: u32 = 1500;
pub const OIL_GEYSER_AMOUNT: u32 = 5000;
pub const STEEL_PATCHES_PER_BASE: u32 = 8;

// --- Supply -----------------------------------------------------------------
pub const INDUSTRIAL_CENTER_SUPPLY: u32 = 10;
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
    pub cost_steel: u32,
    pub cost_oil: u32,
    pub supply: u32,
    pub build_ticks: u32,
    pub radius: f32, // collision / render radius in world px
}

#[derive(Debug, Clone, Copy)]
pub struct BuildingStats {
    pub hp: u32,
    pub sight_tiles: u32,
    pub cost_steel: u32,
    pub cost_oil: u32,
    pub foot_w: u32, // footprint in tiles
    pub foot_h: u32,
    pub build_ticks: u32,
    pub provides_supply: u32,
    // Defensive attack (bunker). dmg == 0 means the building does not attack.
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
            speed: 1.5,
            sight_tiles: 7,
            cost_steel: 50,
            cost_oil: 0,
            supply: 1,
            build_ticks: 120,
            radius: 9.0,
        },
        kinds::RIFLEMAN => UnitStats {
            hp: 45,
            dmg: 5,
            range_tiles: 4,
            cooldown: 8,
            speed: 1.6,
            sight_tiles: 8,
            cost_steel: 50,
            cost_oil: 0,
            supply: 1,
            build_ticks: 150,
            radius: 9.0,
        },
        kinds::MACHINE_GUNNER => UnitStats {
            hp: 55,
            dmg: 4,
            range_tiles: 5,
            cooldown: 3,
            speed: 1.2,
            sight_tiles: 8,
            cost_steel: 75,
            cost_oil: 25,
            supply: 2,
            build_ticks: 200,
            radius: 10.0,
        },
        kinds::AT_TEAM => UnitStats {
            hp: 45,
            dmg: 24,
            range_tiles: 4,
            cooldown: 24,
            speed: 0.65,
            sight_tiles: 8,
            cost_steel: 75,
            cost_oil: 25,
            supply: 2,
            build_ticks: 220,
            radius: 10.0,
        },
        kinds::TANK => UnitStats {
            hp: 130,
            dmg: 20,
            range_tiles: 3,
            cooldown: 18,
            speed: 2.0,
            sight_tiles: 7,
            cost_steel: 100,
            cost_oil: 50,
            supply: 2,
            build_ticks: 250,
            radius: 26.0,
        },
        _ => return None,
    };
    Some(s)
}

/// Stats for a building kind, or `None` if `kind` is not a building.
pub fn building_stats(kind: &str) -> Option<BuildingStats> {
    let s = match kind {
        kinds::INDUSTRIAL_CENTER => BuildingStats {
            hp: 600,
            sight_tiles: 9,
            cost_steel: 400,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 400,
            provides_supply: INDUSTRIAL_CENTER_SUPPLY,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        kinds::DEPOT => BuildingStats {
            hp: 220,
            sight_tiles: 4,
            cost_steel: 50,
            cost_oil: 0,
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
            cost_steel: 100,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 200,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        kinds::ADVANCED_TRAINING_CENTRE => BuildingStats {
            hp: 300,
            sight_tiles: 6,
            cost_steel: 125,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 220,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        kinds::TANK_FACTORY => BuildingStats {
            hp: 360,
            sight_tiles: 6,
            cost_steel: 150,
            cost_oil: 0,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 240,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        kinds::BUNKER => BuildingStats {
            hp: 200,
            sight_tiles: 6,
            cost_steel: 150,
            cost_oil: 0,
            foot_w: 2,
            foot_h: 2,
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
        kinds::INDUSTRIAL_CENTER => &[kinds::WORKER],
        kinds::BARRACKS => &[kinds::RIFLEMAN, kinds::MACHINE_GUNNER, kinds::AT_TEAM],
        kinds::TANK_FACTORY => &[kinds::TANK],
        _ => &[],
    }
}

/// Whether `building_kind` is allowed to be placed given the set of building kinds the
/// player already owns (tech requirements). Most combat structures require an Industrial Center.
pub fn build_requirement_met(building_kind: &str, owned_building_kinds: &[&str]) -> bool {
    match building_kind {
        kinds::BARRACKS | kinds::ADVANCED_TRAINING_CENTRE | kinds::TANK_FACTORY | kinds::BUNKER => {
            owned_building_kinds.contains(&kinds::INDUSTRIAL_CENTER)
        }
        _ => true,
    }
}

/// Whether a unit's training tech has been unlocked by completed buildings.
pub fn train_requirement_met(unit_kind: &str, owned_complete_building_kinds: &[&str]) -> bool {
    match unit_kind {
        kinds::MACHINE_GUNNER | kinds::AT_TEAM => {
            owned_complete_building_kinds.contains(&kinds::ADVANCED_TRAINING_CENTRE)
        }
        _ => true,
    }
}

/// Resource node starting amount for a node kind (`steel` | `oil`).
pub fn node_amount(kind: &str) -> u32 {
    match kind {
        kinds::STEEL => STEEL_PATCH_AMOUNT,
        kinds::OIL => OIL_GEYSER_AMOUNT,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ww2_production_chain_matches_design() {
        assert_eq!(trainable_units(kinds::INDUSTRIAL_CENTER), &[kinds::WORKER]);
        assert_eq!(
            trainable_units(kinds::BARRACKS),
            &[kinds::RIFLEMAN, kinds::MACHINE_GUNNER, kinds::AT_TEAM]
        );
        assert_eq!(trainable_units(kinds::TANK_FACTORY), &[kinds::TANK]);

        assert!(train_requirement_met(kinds::RIFLEMAN, &[]));
        assert!(!train_requirement_met(kinds::MACHINE_GUNNER, &[]));
        assert!(!train_requirement_met(kinds::AT_TEAM, &[]));
        assert!(train_requirement_met(
            kinds::MACHINE_GUNNER,
            &[kinds::ADVANCED_TRAINING_CENTRE]
        ));
        assert!(train_requirement_met(
            kinds::AT_TEAM,
            &[kinds::ADVANCED_TRAINING_CENTRE]
        ));

        let bunker = building_stats(kinds::BUNKER).expect("bunker stats");
        assert_eq!(bunker.cost_steel, 150);
        assert_eq!((bunker.foot_w, bunker.foot_h), (2, 2));
    }
}
