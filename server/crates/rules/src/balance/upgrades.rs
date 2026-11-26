//! Upgrade costs and research durations.

use super::TICK_HZ;

pub const METHAMPHETAMINES_COST_STEEL: u32 = 100;
pub const METHAMPHETAMINES_COST_OIL: u32 = 200;
pub const METHAMPHETAMINES_RESEARCH_TICKS: u32 = TICK_HZ * 40;
pub const METHAMPHETAMINES_SPEED_MULTIPLIER: f32 = 1.25;
pub const METHAMPHETAMINES_RIFLE_ATTACK_COOLDOWN_TICKS: u32 = 24;
/// 150 ms rounded up to the next authoritative simulation tick.
pub const METHAMPHETAMINES_RIFLE_POST_SHOT_LOCK_TICKS: u32 = (TICK_HZ * 150).div_ceil(1000);
pub const PANZERFAUSTS_COST_STEEL: u32 = 100;
pub const PANZERFAUSTS_COST_OIL: u32 = 100;
pub const PANZERFAUSTS_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const ENTRENCHMENT_COST_STEEL: u32 = 100;
pub const ENTRENCHMENT_COST_OIL: u32 = 0;
pub const ENTRENCHMENT_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const ANTI_TANK_GUN_UNLOCK_COST_STEEL: u32 = 100;
pub const ANTI_TANK_GUN_UNLOCK_COST_OIL: u32 = 50;
pub const ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS: u32 = TICK_HZ * 10;
pub const ARTILLERY_UNLOCK_COST_STEEL: u32 = 200;
pub const ARTILLERY_UNLOCK_COST_OIL: u32 = 100;
pub const ARTILLERY_UNLOCK_RESEARCH_TICKS: u32 = TICK_HZ * 25;
pub const BALLISTIC_TABLES_COST_STEEL: u32 = 50;
pub const BALLISTIC_TABLES_COST_OIL: u32 = 100;
pub const BALLISTIC_TABLES_RESEARCH_TICKS: u32 = TICK_HZ * 15;
pub const TANK_UNLOCK_COST_STEEL: u32 = 150;
pub const TANK_UNLOCK_COST_OIL: u32 = 100;
pub const TANK_UNLOCK_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const TANK_PLUS_COST_STEEL: u32 = 150;
pub const TANK_PLUS_COST_OIL: u32 = 150;
pub const TANK_PLUS_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const TANK_PLUS_TURN_MULTIPLIER: f32 = 1.5;
pub const TANK_PLUS_SPEED_MULTIPLIER: f32 = 1.1;
pub const TANK_PLUS_SIGHT_BONUS: u32 = 2;
pub const SMOKE_PLUS_COST_STEEL: u32 = 150;
pub const SMOKE_PLUS_COST_OIL: u32 = 150;
pub const SMOKE_PLUS_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const SCOUT_PLANE_UNLOCK_COST_STEEL: u32 = 100;
pub const SCOUT_PLANE_UNLOCK_COST_OIL: u32 = 200;
pub const SCOUT_PLANE_UNLOCK_RESEARCH_TICKS: u32 = TICK_HZ * 20;
pub const ROCKETS_COST_STEEL: u32 = 75;
pub const ROCKETS_COST_OIL: u32 = 125;
pub const ROCKETS_RESEARCH_TICKS: u32 = TICK_HZ * 25;

// Research unlocks paid Dao training; combat-earned evolution requires insight only.
pub const DAO_TRAINING_COST_OIL: u32 = 100;
pub const DAO_TRAINING_TICKS: u32 = TICK_HZ * 10;
pub const MAX_DAO_TRAINING_QUEUE: usize = 64;
pub const DAO_COST_STEEL: u32 = 100;
pub const DAO_COST_OIL: u32 = 200;
pub const DAO_RESEARCH_TICKS: u32 = TICK_HZ * 40;

/// Wind retains its Scout Car baseline independently of base Disciple speed.
pub const DAO_WIND_SPEED_MULTIPLIER: f32 = 1.15;
/// Ordinary Disciple attacks run at 70% of their former rate, rounded to whole ticks.
pub const DISCIPLE_ATTACK_COOLDOWN_TICKS: u32 = (64 * 10 + 3) / 7;

/// One passive insight for every five seconds of a living Disciple’s lifetime.
pub const DISCIPLE_PASSIVE_INSIGHT_TICKS: u16 = (TICK_HZ * 5) as u16;
pub const DISCIPLE_INSIGHT_REQUIRED: u32 = 60;
pub const DISCIPLE_EVOLVED_INSIGHT_CAP: u32 = 200;
pub const DAO_MASTER_BODY_SCALE: f32 = 1.2;
pub const DAO_MASTER_SPEED_MULTIPLIER: f32 = 1.1;
pub const DAO_MASTER_DAMAGE_MULTIPLIER: f32 = 1.2;
/// Additional primary damage multiplier after ordinary War Master rank scaling.
pub const DAO_WAR_MASTER_DAMAGE_MULTIPLIER: u32 = 2;
pub const DAO_MASTER_HP_MULTIPLIER: f32 = 1.4;
pub const DAO_WAR_SPEED_MULTIPLIER: f32 = 1.05;
// Adept HP increased by 30%, rounded to whole HP; Novices keep their base HP.
pub const DAO_ADEPT_HP: u32 = 150;
pub const DAO_WAR_HP_BONUS: u32 = 26;
pub const DAO_WAR_DAMAGE_MULTIPLIER: f32 = 1.5;
pub const DAO_WAR_ARMOR_REDUCTION: f32 = 0.5;
/// Additive armor percentage points from the permanent Shop item.
pub const SHOP_ARMOR_BONUS_REDUCTION: f32 = 0.5;
/// Yari ready-tip reach, with the usual attacker radius and combat slack.
pub const DAO_WAR_RANGE_TILES: f32 = 1.75;
/// War artwork and collision body shrink together to 70% of the former size.
pub const DAO_WAR_BODY_SCALE: f32 = 0.7;
pub const DAO_RANGED_RANGE_TILES: f32 = 5.0;
pub const DAO_WIND_ATTACK_RATE: f32 = 1.5;
pub const DAO_MAGIC_SPEED_MULTIPLIER: f32 = 1.05;

pub const DISCIPLE_SHIELD_ARMOR_REDUCTION: f32 = 0.75;
pub const DISCIPLE_SHIELD_CAPACITY: u32 = 150;
pub const MAGIC_MASTER_SHIELD_CAPACITY: u32 = DISCIPLE_SHIELD_CAPACITY * 2;
pub const MAGIC_MASTER_SHIELD_DURATION_TICKS: u32 = DISCIPLE_SHIELD_DURATION_TICKS * 2;
pub const MAGIC_ANCHOR_DEATH_INSIGHT: u32 = 20;
pub const DISCIPLE_SHIELD_RADIUS_TILES: f32 = 2.1;
pub const DISCIPLE_SHIELD_DURATION_TICKS: u32 = TICK_HZ * 5;
pub const DISCIPLE_SHIELD_COOLDOWN_TICKS: u16 = (TICK_HZ * 10) as u16;

/// Completed and paid active Dao researches reserve player-wide price tiers.
pub fn dao_research_tier(reserved: usize) -> (u32, u32, u32) {
    let multiplier = 1u32 << reserved.min(2);
    (
        DAO_COST_STEEL / 2 * multiplier,
        DAO_COST_OIL / 2 * multiplier,
        DAO_RESEARCH_TICKS / 2 * multiplier,
    )
}

/// Exact combat insight ratio, with a shared denominator to carry building fractions.
pub fn disciple_insight_ratio(
    attacker: crate::EntityKind,
    target: crate::EntityKind,
) -> (u64, u64) {
    if super::building_stats(target).is_some()
        && !(attacker == crate::EntityKind::WindAdept && target.is_resource_extractor())
    {
        (1, 4)
    } else if attacker == crate::EntityKind::WarAdept && target == crate::EntityKind::Tank {
        (8, 4)
    } else {
        (4, 4)
    }
}

/// Shield strength is selected at cast time and retained through later promotion.
pub const fn disciple_shield_duration_ticks(master: bool) -> u32 {
    if master {
        MAGIC_MASTER_SHIELD_DURATION_TICKS
    } else {
        DISCIPLE_SHIELD_DURATION_TICKS
    }
}
