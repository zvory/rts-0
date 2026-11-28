//! Ability effect and cooldown constants exported through the balance surface.

use super::TICK_HZ;

pub const BREAKTHROUGH_RADIUS_TILES: f32 = 9.0;
pub const BREAKTHROUGH_DURATION_TICKS: u16 = (TICK_HZ as u16) * 6;
pub const BREAKTHROUGH_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 25;
/// Permanent nearby-owned-unit movement multiplier from each Command Car.
pub const COMMAND_CAR_AURA_SPEED_MULTIPLIER: f32 = 1.3;
pub const BREAKTHROUGH_BASE_SPEED_MULTIPLIER: f32 = 1.6;
pub const BREAKTHROUGH_SMOKE_SPEED_MULTIPLIER: f32 = 1.9;
pub const BREAKTHROUGH_MOVING_ATTACK_COOLDOWN_NUMERATOR: u32 = 1;
pub const BREAKTHROUGH_MOVING_ATTACK_COOLDOWN_DENOMINATOR: u32 = 2;
pub const BREAKTHROUGH_RECENT_SMOKE_TICKS: u16 = (TICK_HZ as u16) * 2;

pub const SMOKE_ABILITY_RANGE_TILES: u32 = 14;
pub const SMOKE_LAUNCH_MAX_DELAY_TICKS: u32 = TICK_HZ / 10;
pub const SMOKE_CLOUD_RADIUS_TILES: f32 = 2.25;
pub const SMOKE_CLOUD_DURATION_TICKS: u32 = TICK_HZ * 5;
/// Body-edge distance at which two units inside smoke gain mutual close-quarters visibility.
pub const SMOKE_MELEE_VISIBILITY_RANGE_TILES: f32 = 1.0;
pub const SMOKE_PLUS_CLOUD_RADIUS_TILES: f32 = 4.0;
pub const SMOKE_PLUS_CLOUD_DURATION_TICKS: u32 = SMOKE_CLOUD_DURATION_TICKS * 2;
pub const SMOKE_ABILITY_COOLDOWN_TICKS: u16 = 0;
pub const SCOUT_CAR_SMOKE_CHARGES: u16 = 2;
pub const SCOUT_CAR_SMOKE_CHARGE_RECHARGE_TICKS: u16 = (TICK_HZ as u16) * 15;
pub const SMOKE_ABILITY_COST_STEEL: u32 = 0;
pub const SMOKE_ABILITY_COST_OIL: u32 = 0;

pub const ARTILLERY_BLANKET_RADIUS_TILES: f32 = 15.0;
pub const ARTILLERY_MIN_FIRE_RADIUS_TILES: f32 = 4.0;
pub const ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES: f32 = 1.0;

pub const ROCKET_BARRAGE_ROCKETS: u32 = 16;
pub const ROCKET_BARRAGE_UNLOAD_TICKS: u32 = TICK_HZ * 4;
// Cooldown starts when the manual barrage begins and lasts thirty seconds.
pub const ROCKET_BARRAGE_RELOAD_TICKS: u16 = (TICK_HZ as u16) * 30;
pub const ROCKET_BARRAGE_COST_OIL: u32 = 150;
pub const ROCKET_BARRAGE_MIN_RANGE_TILES: u32 = 10;
pub const ROCKET_BARRAGE_MAX_RANGE_TILES: u32 = 44;
pub const ROCKET_BARRAGE_SCATTER_RADIUS_TILES: f32 = 6.0;
pub const ROCKET_BARRAGE_AP_RADIUS_TILES: f32 = 0.5;
pub const ROCKET_BARRAGE_INNER_RADIUS_TILES: f32 = 1.0;
pub const ROCKET_BARRAGE_OUTER_RADIUS_TILES: f32 = 2.0;
pub const ROCKET_BARRAGE_DIRECT_DAMAGE: u32 = 70;
pub const ROCKET_BARRAGE_INNER_DAMAGE: u32 = 53;
pub const ROCKET_BARRAGE_OUTER_DAMAGE: u32 = 21;

/// Physical rack slots, in launch order, in world pixels relative to the truck's centre.
/// Mirrored by client rocket_truck_visuals.js. The rear rack has six rails and three
/// short ranks; its back rank leaves the two outer rails empty (16 rounds total).
pub fn rocket_rack_slot(index: u32) -> (f32, f32) {
    let index = index.min(ROCKET_BARRAGE_ROCKETS - 1);
    let column = index / 6;
    let row = index % 6 + u32::from(column == 2);
    (-3.5 - column as f32 * 9.8, -7.9 + row as f32 * 2.8)
}

/// Derive visible ammunition from the unload clock; randomized reloads must not shift
/// the rack's launch cadence. No private timer is exposed to opponents.
pub fn rocket_rack_count(cooldown_left: u16, unload_ticks_left: u16) -> u8 {
    if cooldown_left == 0 {
        return ROCKET_BARRAGE_ROCKETS as u8;
    }
    // The unload lock ticks down during the activation tick, after its first launch.
    let elapsed = ROCKET_BARRAGE_UNLOAD_TICKS.saturating_sub(u32::from(unload_ticks_left));
    (0..ROCKET_BARRAGE_ROCKETS)
        .filter(|index| {
            index * ROCKET_BARRAGE_UNLOAD_TICKS / (ROCKET_BARRAGE_ROCKETS - 1) > elapsed
        })
        .count() as u8
}
/// Rocket barrages are anti-unit saturation weapons, not efficient demolition weapons.
/// Apply this after the existing armor policy whenever a rocket damages a building.
pub const ROCKET_BARRAGE_BUILDING_DAMAGE_NUMERATOR: u32 = 1;
pub const ROCKET_BARRAGE_BUILDING_DAMAGE_DENOMINATOR: u32 = 4;

pub fn rocket_barrage_building_damage(damage: u32) -> u32 {
    damage
        .saturating_mul(ROCKET_BARRAGE_BUILDING_DAMAGE_NUMERATOR)
        .div_ceil(ROCKET_BARRAGE_BUILDING_DAMAGE_DENOMINATOR)
}

pub const EKAT_CONSUME_GOLEM_RANGE_TILES: u32 = 2;
pub const NOVICE_CHARGE_DURATION_TICKS: u16 = TICK_HZ as u16;
pub const WAR_CHARGE_DURATION_TICKS: u16 = (TICK_HZ as u16) * 4;
pub const WAR_CHARGE_SPEED_MULTIPLIER: f32 = 2.0;
pub const NOVICE_CHARGE_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 15;
pub const NOVICE_CHARGE_SPEED_MULTIPLIER: f32 = 1.5;
pub const NOVICE_CHARGE_MIN_DISTANCE_TILES: u32 = 2;
pub const NOVICE_CHARGE_MAX_DISTANCE_TILES: u32 = 5;
pub const HASTE_SPEED_MULTIPLIER: f32 = 1.15;
pub const DAO_WIND_DASH_RANGE_TILES: u32 = 12;
pub const DAO_WIND_DASH_RECHARGE_TICKS: u16 = (TICK_HZ as u16) * 18;
pub const EKAT_TELEPORT_RANGE_TILES: u32 = 5;
pub const EKAT_TELEPORT_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 8;
pub const EKAT_RETURN_MARKER_DURATION_TICKS: u32 = TICK_HZ * 4;
pub const EKAT_RETURN_MIN_DELAY_TICKS: u32 = 1;
pub const EKAT_LINE_SHOT_RANGE_TILES: u32 = 6;
pub const EKAT_LINE_SHOT_WIDTH_TILES: f32 = 0.6;
pub const EKAT_LINE_SHOT_SPEED_PX_PER_TICK: f32 = 8.0;
pub const THROW_SWORD_DAMAGE: u32 = 20;
pub const THROW_SWORD_ARMOR_PENETRATION: f32 = 0.0;
pub const THROW_SWORD_RANGE_TILES: f32 = 7.5;
pub const THROW_SWORD_WIDTH_TILES: f32 = 0.9;
pub const THROW_SWORD_SPEED_PX_PER_TICK: f32 = 100.0 / 11.0;
pub const EKAT_LINE_SHOT_DAMAGE: u32 = 40;
pub const EKAT_LINE_SHOT_COOLDOWN_TICKS: u16 = (TICK_HZ as u16) * 20;
pub const EKAT_MAGIC_ANCHOR_RANGE_TILES: f32 = 7.5;
pub const EKAT_MAGIC_ANCHOR_DURATION_TICKS: u32 = TICK_HZ * 10;
pub const EKAT_MAGIC_ANCHOR_RADIUS_TILES: f32 = 3.0;
pub const EKAT_MAGIC_ANCHOR_PULL_SPEED_MULTIPLIER: f32 = 2.4;

// Shop ground field: HP stores consumable healing capacity, never combat durability.
pub const REGEN_PULSE_TICKS: u32 = super::TICK_HZ / 3;

pub const HEALING_SEAL_CAPACITY: u32 = 400;
pub const HEALING_SEAL_RADIUS_TILES: f32 = 1.5;
pub const HEALING_SEAL_PULSE_TICKS: u32 = super::TICK_HZ / 3;

/// Total carried-item slots by Cultivator rank. Consumable goods do not occupy slots.
pub const CULTIVATOR_NOVICE_ITEM_CAPACITY: u8 = 1;
pub const CULTIVATOR_ITEM_CAPACITY: u8 = 3;
pub const CULTIVATOR_MASTER_ITEM_CAPACITY: u8 = 6;

/// Fixed presentation speed for field particles; gameplay push is unit-relative.
pub const VECTOR_FIELD_SPEED_PX_PER_TICK: f32 = 1.0;
/// Fraction of the affected unit's effective movement speed added by each field.
pub const VECTOR_FIELD_UNIT_SPEED_MULTIPLIER: f32 = 0.5;
pub const VECTOR_FIELD_ADEPT_SIZE_TILES: f32 = 4.0;
pub const VECTOR_FIELD_MASTER_SIZE_TILES: f32 = 5.0;

pub const VECTOR_FIELD_RANGE_TILES: f32 = 10.5;
pub const VECTOR_FIELD_DURATION_TICKS: u32 = TICK_HZ * 10;
pub const VECTOR_FIELD_ADEPT_COOLDOWN_TICKS: u16 = (TICK_HZ * 30) as u16;
pub const VECTOR_FIELD_MASTER_COOLDOWN_TICKS: u16 = (TICK_HZ * 20) as u16;
