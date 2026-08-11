//! Compatibility stat records and lookup helpers backed by `defs.rs`.

use crate::defs;
use crate::EntityKind;

use super::TILE_SIZE;

/// A direct attack issued on a completed Tank Trap captures the other visible completed traps
/// inside this radius as one cluster-clearing order.
pub const TANK_TRAP_CLUSTER_ATTACK_RADIUS_TILES: f32 = 4.0;

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
