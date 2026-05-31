//! Balance & simulation constants. See `DESIGN.md` §5.
//!
//! Authoritative source of game balance. `client/src/config.js` mirrors the subset the
//! UI / rendering / fog overlay needs (costs, supply, sight, sizes, colors). Keep both
//! in sync; when you change a number here that the UI shows, change it there too.

use crate::game::entity::EntityKind;

// --- Timing -----------------------------------------------------------------
pub const TICK_HZ: u32 = 30;
pub const TICK_MS: u64 = 1000 / TICK_HZ as u64;

// --- Map --------------------------------------------------------------------
pub const TILE_SIZE: u32 = 32;

// --- Tolerant arrival -------------------------------------------------------
/// Movement below this many world pixels per tick counts as "no progress" for stuck detection.
pub const STUCK_EPS_PX: f32 = 2.0;
/// Consecutive ticks of no progress before a near-goal unit is considered arrived (~0.5 s at 30 Hz).
pub const STUCK_ARRIVAL_TICKS: u16 = 15;
/// A stuck unit within this radius of its `path_goal` is forcibly marked Arrived.
pub const TOLERANT_ARRIVAL_RADIUS_PX: f32 = 2.0 * TILE_SIZE as f32;

// --- Sidestep (mid-path unstick) --------------------------------------------
/// Consecutive stuck ticks before a mid-path unit injects a sidestep detour (~0.5 s at 30 Hz).
pub const SIDESTEP_TRIGGER_TICKS: u16 = 15;
/// Perpendicular detour distance in world pixels (~1 tile).
pub const SIDESTEP_DISTANCE_PX: f32 = TILE_SIZE as f32;
/// Ticks after a sidestep during which another sidestep is suppressed (~1 s at 30 Hz).
pub const SIDESTEP_COOLDOWN_TICKS: u16 = 30;
/// Consecutive ticks blocked by a static obstacle before queuing a repath (~1 s at 30 Hz).
pub const STATIC_BLOCKED_REPATH_TICKS: u16 = TICK_HZ as u16;

/// Radius within which an *intermediate* waypoint is considered reached. Tile centers are routing
/// hints; brushing within half a tile satisfies the route. Must be ≥ largest unit radius so two
/// units contesting the same tile center cannot both lock onto it simultaneously.
pub const ARRIVE_RADIUS_INTERMEDIATE_PX: f32 = TILE_SIZE as f32 * 0.5; // 16 px

/// Machine-gunner setup/teardown time. One second at the simulation tick rate.
pub const MACHINE_GUNNER_SETUP_TICKS: u16 = TICK_HZ as u16;

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
pub const OIL_LOAD: u32 = 2;
pub const HARVEST_TICKS: u32 = 40;
pub const STEEL_PATCH_AMOUNT: u32 = 1500;
pub const OIL_GEYSER_AMOUNT: u32 = 5000;
pub const STEEL_PATCHES_PER_BASE: u32 = 16;

/// Minimum distance (in tiles) from an Industrial Center center to any starting resource node.
/// Prevents resources from spawning inside or too close to the building footprint.
pub const IC_RESOURCE_MIN_DIST_TILES: f32 = 3.5;

/// Maximum distance (in tiles) from an Industrial Center center to any starting resource node.
/// Ensures no player is advantaged by resources being too far away.
pub const IC_RESOURCE_MAX_DIST_TILES: f32 = 7.0;

/// Distance (in tiles) from the Industrial Center to the center of the steel patch block.
pub const STEEL_BLOCK_DIST_TILES: f32 = 5.5;

/// Distance (in tiles) from the Industrial Center to the starting oil geyser.
pub const OIL_DIST_TILES: f32 = 6.0;

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

impl UnitStats {
    /// Tile clearance radius for pathfinding: how many tiles around the center must be open.
    /// A tank (radius ~26 px) needs 1 tile of clearance; infantry (~9 px) is point-sized.
    pub fn radius_tiles(&self) -> u32 {
        (self.radius / TILE_SIZE as f32).round() as u32
    }
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
    // dmg == 0 means the building does not attack.
    pub dmg: u32,
    pub range_tiles: u32,
    pub cooldown: u32,
}

/// Stats for a unit kind, or `None` if `kind` is not a unit.
pub fn unit_stats(kind: EntityKind) -> Option<UnitStats> {
    let s = match kind {
        EntityKind::Worker => UnitStats {
            hp: 40,
            dmg: 4,
            range_tiles: 1,
            cooldown: 12,
            speed: 1.6,
            sight_tiles: 7,
            cost_steel: 50,
            cost_oil: 0,
            supply: 1,
            build_ticks: 120,
            radius: 9.0,
        },
        EntityKind::Rifleman => UnitStats {
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
        EntityKind::MachineGunner => UnitStats {
            hp: 55,
            dmg: 4,
            range_tiles: 5,
            cooldown: 3,
            speed: 1.44,
            sight_tiles: 8,
            cost_steel: 75,
            cost_oil: 25,
            supply: 2,
            build_ticks: 200,
            radius: 10.0,
        },
        EntityKind::AtTeam => UnitStats {
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
        EntityKind::Tank => UnitStats {
            hp: 390,
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
pub fn building_stats(kind: EntityKind) -> Option<BuildingStats> {
    let s = match kind {
        EntityKind::IndustrialCenter => BuildingStats {
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
        EntityKind::Depot => BuildingStats {
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
        EntityKind::Barracks => BuildingStats {
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
        EntityKind::TrainingCentre => BuildingStats {
            hp: 300,
            sight_tiles: 6,
            cost_steel: 100,
            cost_oil: 50,
            foot_w: 3,
            foot_h: 2,
            build_ticks: 220,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        EntityKind::TankFactory => BuildingStats {
            hp: 360,
            sight_tiles: 6,
            cost_steel: 200,
            cost_oil: 100,
            foot_w: 3,
            foot_h: 3,
            build_ticks: 240,
            provides_supply: 0,
            dmg: 0,
            range_tiles: 0,
            cooldown: 0,
        },
        _ => return None,
    };
    Some(s)
}

/// Which units a given building can train.
pub fn trainable_units(building_kind: EntityKind) -> &'static [EntityKind] {
    match building_kind {
        EntityKind::IndustrialCenter => &[EntityKind::Worker],
        EntityKind::Barracks => &[
            EntityKind::Rifleman,
            EntityKind::MachineGunner,
            EntityKind::AtTeam,
        ],
        EntityKind::TankFactory => &[EntityKind::Tank],
        _ => &[],
    }
}

/// Whether `building_kind` is allowed to be placed given the set of building kinds the
/// player already owns (tech requirements). Most combat structures require an Industrial Center.
pub fn build_requirement_met(
    building_kind: EntityKind,
    owned_building_kinds: &[EntityKind],
) -> bool {
    match building_kind {
        EntityKind::Barracks | EntityKind::TrainingCentre | EntityKind::TankFactory => {
            owned_building_kinds.contains(&EntityKind::IndustrialCenter)
        }
        _ => true,
    }
}

/// Whether a unit's training tech has been unlocked by completed buildings.
pub fn train_requirement_met(
    unit_kind: EntityKind,
    owned_complete_building_kinds: &[EntityKind],
) -> bool {
    match unit_kind {
        EntityKind::MachineGunner | EntityKind::AtTeam => {
            owned_complete_building_kinds.contains(&EntityKind::TrainingCentre)
        }
        _ => true,
    }
}

/// Resource node starting amount for a node kind (`steel` | `oil`).
pub fn node_amount(kind: EntityKind) -> u32 {
    match kind {
        EntityKind::Steel => STEEL_PATCH_AMOUNT,
        EntityKind::Oil => OIL_GEYSER_AMOUNT,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ww2_production_chain_matches_design() {
        assert_eq!(
            trainable_units(EntityKind::IndustrialCenter),
            &[EntityKind::Worker]
        );
        assert_eq!(
            trainable_units(EntityKind::Barracks),
            &[
                EntityKind::Rifleman,
                EntityKind::MachineGunner,
                EntityKind::AtTeam
            ]
        );
        assert_eq!(
            trainable_units(EntityKind::TankFactory),
            &[EntityKind::Tank]
        );

        assert!(train_requirement_met(EntityKind::Rifleman, &[]));
        assert!(!train_requirement_met(EntityKind::MachineGunner, &[]));
        assert!(!train_requirement_met(EntityKind::AtTeam, &[]));
        assert!(train_requirement_met(
            EntityKind::MachineGunner,
            &[EntityKind::TrainingCentre]
        ));
        assert!(train_requirement_met(
            EntityKind::AtTeam,
            &[EntityKind::TrainingCentre]
        ));
    }
}
