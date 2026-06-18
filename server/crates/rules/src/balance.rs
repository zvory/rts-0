//! Simulation constants and compatibility stats helpers.

use crate::defs;
use crate::EntityKind;

// --- Timing -----------------------------------------------------------------
pub const TICK_HZ: u32 = 30;
pub const TICK_MS: u64 = 1000 / TICK_HZ as u64;

// --- Map --------------------------------------------------------------------
pub const TILE_SIZE: u32 = 32;

// --- Tolerant arrival -------------------------------------------------------
pub const STUCK_EPS_PX: f32 = 2.0;
pub const STUCK_ARRIVAL_TICKS: u16 = 15;
pub const TOLERANT_ARRIVAL_RADIUS_PX: f32 = 2.0 * TILE_SIZE as f32;

// --- Sidestep (mid-path unstick) --------------------------------------------
pub const SIDESTEP_TRIGGER_TICKS: u16 = 15;
pub const SIDESTEP_DISTANCE_PX: f32 = TILE_SIZE as f32;
pub const SIDESTEP_COOLDOWN_TICKS: u16 = 30;
pub const STATIC_BLOCKED_REPATH_TICKS: u16 = TICK_HZ as u16;

pub const ARRIVE_RADIUS_INTERMEDIATE_PX: f32 = TILE_SIZE as f32 * 0.5;
pub const VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX: f32 = TILE_SIZE as f32 * 0.75;
pub const SCOUT_CAR_FINAL_GOAL_TOLERANCE_PX: f32 = TILE_SIZE as f32 * 0.375;
pub const SCOUT_CAR_STUCK_RECOVERY_TRIGGER_TICKS: u16 = STUCK_ARRIVAL_TICKS;
pub const SCOUT_CAR_REVERSE_RECOVERY_DISTANCE_PX: f32 = TILE_SIZE as f32 * 2.0;
pub const SCOUT_CAR_RECOVERY_COOLDOWN_TICKS: u16 = TICK_HZ as u16;

pub const MACHINE_GUNNER_SETUP_TICKS: u16 = TICK_HZ as u16;
pub const ANTI_TANK_GUN_SETUP_TICKS: u16 = (TICK_HZ as u16) * 3 / 2;
pub const MORTAR_TEAM_SETUP_TICKS: u16 = 0;
pub const MORTAR_SHELL_DELAY_TICKS: u32 = (TICK_HZ * 9 + 2) / 4;
pub const MORTAR_OUTER_RADIUS_TILES: f32 = 1.5;
pub const MORTAR_INNER_RADIUS_TILES: f32 = 0.5;
pub const MORTAR_OUTER_DAMAGE: u32 = 30;
pub const MORTAR_INNER_DAMAGE: u32 = 60;
pub const MORTAR_AUTOFIRE_ERROR_TILES: f32 = 0.35;
pub const ANTI_TANK_GUN_PACKED_RANGE_TILES: u32 = 5;
pub const ANTI_TANK_GUN_DEPLOYED_RANGE_TILES: u32 = 12;
pub const ANTI_TANK_GUN_PACKED_DAMAGE_MULTIPLIER: f32 = 0.75;
pub const ANTI_TANK_GUN_FIELD_OF_FIRE_RAD: f32 = 45.0_f32 * std::f32::consts::PI / 180.0;
pub const ARTILLERY_SETUP_TICKS: u16 = (TICK_HZ as u16) * 3;
pub const ARTILLERY_RELOAD_TICKS: u32 = TICK_HZ * 3;
pub const ARTILLERY_SHELL_DELAY_TICKS: u32 = TICK_HZ * 5;
pub const ARTILLERY_MIN_RANGE_TILES: u32 = 15;
pub const ARTILLERY_MAX_RANGE_TILES: u32 = 60;
pub const ARTILLERY_FIELD_OF_FIRE_RAD: f32 = 20.0_f32 * std::f32::consts::PI / 180.0;
pub const ARTILLERY_AMMO_COST_STEEL: u32 = 10;
pub const ARTILLERY_INNER_RADIUS_TILES: f32 = 1.0;
pub const ARTILLERY_OUTER_RADIUS_TILES: f32 = 3.0;
pub const ARTILLERY_INNER_DAMAGE: u32 = 150;
pub const ARTILLERY_OUTER_MIN_DAMAGE: u32 = 10;
pub const ARTILLERY_INITIAL_ERROR_TILES: f32 = 10.0;
pub const ARTILLERY_MIN_ERROR_TILES: f32 = 2.0;
pub const ARTILLERY_ACCURACY_SHOTS_TO_MIN: u16 = 5;

pub const TANK_OIL_COST_PER_PX: f32 = 20.0 / (96.0 * TILE_SIZE as f32);
pub const SCOUT_CAR_OIL_COST_PER_PX: f32 = 5.0 / (96.0 * TILE_SIZE as f32);
pub const TANK_OIL_STARVED_PAUSE_TICKS: u16 = TICK_HZ as u16;

pub const RIFLEMAN_CHARGE_TICKS: u16 = 64;
pub const RIFLEMAN_CHARGE_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 5;
pub const RIFLEMAN_CHARGE_SPEED_MULTIPLIER: f32 = 1.25;
pub const METHAMPHETAMINES_COST_STEEL: u32 = 100;
pub const METHAMPHETAMINES_COST_OIL: u32 = 100;
pub const METHAMPHETAMINES_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const METHAMPHETAMINES_ATTACK_COOLDOWN_NUMERATOR: u32 = 3;
pub const METHAMPHETAMINES_ATTACK_COOLDOWN_DENOMINATOR: u32 = 4;
pub const ANTI_TANK_GUN_UNLOCK_COST_STEEL: u32 = 200;
pub const ANTI_TANK_GUN_UNLOCK_COST_OIL: u32 = 75;
pub const ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const ARTILLERY_UNLOCK_COST_STEEL: u32 = 300;
pub const ARTILLERY_UNLOCK_COST_OIL: u32 = 200;
pub const ARTILLERY_UNLOCK_RESEARCH_TICKS: u32 = TICK_HZ * 30;
pub const TANK_UNLOCK_COST_STEEL: u32 = 150;
pub const TANK_UNLOCK_COST_OIL: u32 = 100;
pub const TANK_UNLOCK_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const COMMAND_CAR_UNLOCK_COST_STEEL: u32 = 150;
pub const COMMAND_CAR_UNLOCK_COST_OIL: u32 = 150;
pub const COMMAND_CAR_UNLOCK_RESEARCH_TICKS: u32 = TICK_HZ * 30;
pub const MORTAR_AUTOCAST_COST_STEEL: u32 = 150;
pub const MORTAR_AUTOCAST_COST_OIL: u32 = 150;
pub const MORTAR_AUTOCAST_RESEARCH_TICKS: u32 = TICK_HZ * 20;

pub const BREAKTHROUGH_RADIUS_TILES: f32 = 9.0;
pub const BREAKTHROUGH_DURATION_TICKS: u16 = (TICK_HZ as u16) * 6;
pub const BREAKTHROUGH_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 25;
pub const BREAKTHROUGH_BASE_SPEED_MULTIPLIER: f32 = 1.4;
pub const BREAKTHROUGH_SMOKE_SPEED_MULTIPLIER: f32 = 1.8;
pub const BREAKTHROUGH_RECENT_SMOKE_TICKS: u16 = (TICK_HZ as u16) * 2;

pub const SMOKE_ABILITY_RANGE_TILES: u32 = 9;
pub const SMOKE_LAUNCH_MAX_DELAY_TICKS: u32 = TICK_HZ / 10;
pub const SMOKE_CLOUD_RADIUS_TILES: f32 = 2.0;
pub const SMOKE_CLOUD_DURATION_TICKS: u32 = TICK_HZ * 5;
pub const SMOKE_ABILITY_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 20;
pub const SCOUT_CAR_SMOKE_USES: u16 = 2;
pub const SMOKE_ABILITY_COST_STEEL: u32 = 0;
pub const SMOKE_ABILITY_COST_OIL: u32 = 0;
pub const EKAT_REGEN_TICKS: u32 = TICK_HZ;
pub const EKAT_REGEN_HP: u32 = 1;
pub const EKAT_TELEPORT_RANGE_TILES: u32 = 5;
pub const EKAT_TELEPORT_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 8;
pub const EKAT_RETURN_MARKER_DURATION_TICKS: u32 = TICK_HZ * 4;
pub const EKAT_RETURN_MIN_DELAY_TICKS: u32 = 1;
pub const EKAT_LINE_SHOT_RANGE_TILES: u32 = 6;
pub const EKAT_LINE_SHOT_WIDTH_TILES: f32 = 0.6;
pub const EKAT_LINE_SHOT_SPEED_PX_PER_TICK: f32 = 8.0;
pub const EKAT_LINE_SHOT_DAMAGE: u32 = 40;
pub const EKAT_LINE_SHOT_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 10;
pub const EKAT_MAGIC_ANCHOR_RANGE_TILES: u32 = 5;
pub const EKAT_MAGIC_ANCHOR_DURATION_TICKS: u32 = TICK_HZ * 10;
pub const EKAT_MAGIC_ANCHOR_RADIUS_TILES: f32 = 3.0;
pub const EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER: f32 = 0.45;
pub const EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER: f32 = 1.35;

// --- Economy ----------------------------------------------------------------
pub const STARTING_STEEL: u32 = 75;
pub const STARTING_OIL: u32 = 0;
pub const STARTING_WORKERS: u32 = 4;
pub const QUICKSTART_STEEL: u32 = 99_999;
pub const QUICKSTART_OIL: u32 = 99_999;

pub const STEEL_LOAD: u32 = 2;
pub const OIL_LOAD: u32 = 2;
pub const HARVEST_TICKS: u32 = 40;
pub const STEEL_PATCH_AMOUNT: u32 = 1000;
pub const OIL_GEYSER_AMOUNT: u32 = 3333;
pub const STEEL_PATCHES_PER_BASE: u32 = 18;
pub const OIL_PATCHES_PER_BASE: u32 = 3;

pub const CC_RESOURCE_MIN_DIST_TILES: f32 = 3.5;
pub const CC_RESOURCE_MAX_DIST_TILES: f32 = 7.0;
// Two-tile mining buffer beyond authored/base resource cluster placement.
pub const MINING_CC_RANGE_TILES: f32 = 9.0;
pub const STEEL_BLOCK_DIST_TILES: f32 = 5.0;
pub const OIL_DIST_TILES: f32 = 6.0;

// --- Supply -----------------------------------------------------------------
pub const CITY_CENTRE_SUPPLY: u32 = 10;
pub const DEPOT_SUPPLY: u32 = 8;
pub const SUPPLY_CAP_MAX: u32 = 200;

// --- Vehicle bodies ----------------------------------------------------------
pub const TANK_BODY_LENGTH_PX: f32 = 50.4;
pub const TANK_BODY_WIDTH_PX: f32 = 28.8;
pub const TANK_BODY_CLEARANCE_PX: f32 = 1.5;
pub const ANTI_TANK_GUN_BODY_LENGTH_PX: f32 = 42.0;
pub const ANTI_TANK_GUN_BODY_WIDTH_PX: f32 = 24.0;
pub const ANTI_TANK_GUN_BODY_CLEARANCE_PX: f32 = 1.0;
pub const ARTILLERY_BODY_LENGTH_PX: f32 = TANK_BODY_LENGTH_PX;
pub const ARTILLERY_BODY_WIDTH_PX: f32 = TANK_BODY_WIDTH_PX;
pub const ARTILLERY_BODY_CLEARANCE_PX: f32 = TANK_BODY_CLEARANCE_PX;
pub const SCOUT_CAR_BODY_LENGTH_PX: f32 = 40.8;
pub const SCOUT_CAR_BODY_WIDTH_PX: f32 = 21.6;
pub const SCOUT_CAR_BODY_CLEARANCE_PX: f32 = 1.0;
pub const COMMAND_CAR_BODY_LENGTH_PX: f32 = 34.8;
pub const COMMAND_CAR_BODY_WIDTH_PX: f32 = 18.4;
pub const COMMAND_CAR_BODY_CLEARANCE_PX: f32 = 1.0;

// --- Stats ------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct UnitStats {
    pub hp: u32,
    pub dmg: u32,
    pub range_tiles: u32,
    pub cooldown: u32,
    pub speed: f32,
    pub sight_tiles: u32,
    pub cost_steel: u32,
    pub cost_oil: u32,
    pub supply: u32,
    pub build_ticks: u32,
    pub radius: f32,
}

impl UnitStats {
    pub fn radius_tiles(&self) -> u32 {
        (self.radius / TILE_SIZE as f32).round() as u32
    }
}

pub fn unit_radius_tiles(kind: EntityKind) -> u32 {
    if matches!(
        kind,
        EntityKind::AntiTankGun
            | EntityKind::MortarTeam
            | EntityKind::Artillery
            | EntityKind::ScoutCar
            | EntityKind::Tank
            | EntityKind::CommandCar
    ) {
        return 0;
    }
    unit_stats(kind)
        .map(|stats| stats.radius_tiles())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy)]
pub struct BuildingStats {
    pub hp: u32,
    pub sight_tiles: u32,
    pub cost_steel: u32,
    pub cost_oil: u32,
    pub foot_w: u32,
    pub foot_h: u32,
    pub build_ticks: u32,
    pub provides_supply: u32,
    pub dmg: u32,
    pub range_tiles: u32,
    pub cooldown: u32,
}

pub fn unit_stats(kind: EntityKind) -> Option<UnitStats> {
    defs::unit_def(kind).map(|d| d.stats)
}

pub fn building_stats(kind: EntityKind) -> Option<BuildingStats> {
    defs::building_def(kind).map(|d| d.stats)
}
