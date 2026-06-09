//! Pure combat rules: classification predicates and damage formula.

use crate::defs::{self, ArmorClass, TargetPriority, WeaponClass};
use crate::terrain::{self, TerrainKind};
use crate::EntityKind;

const FRONT_ARC_RAD: f32 = std::f32::consts::FRAC_PI_4;
const SIDE_ARC_RAD: f32 = std::f32::consts::PI * 3.0 / 4.0;
const FRONT_ARMOR_DAMAGE_MULTIPLIER: f32 = 1.0;
const SIDE_ARMOR_DAMAGE_MULTIPLIER: f32 = 1.25;
const REAR_ARMOR_DAMAGE_MULTIPLIER: f32 = 1.75;
/// Attack profile for a combat-capable unit or building.
pub struct AttackProfile {
    pub range_tiles: u32,
    pub dmg: u32,
    pub cooldown: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorFacing {
    Front,
    Side,
    Rear,
}

/// Returns the attack profile for the given kind, or zeroes if non-combatant.
pub fn attack_profile(kind: EntityKind) -> AttackProfile {
    if let Some(s) = defs::unit_def(kind).map(|d| d.stats) {
        AttackProfile {
            range_tiles: s.range_tiles,
            dmg: s.dmg,
            cooldown: s.cooldown,
        }
    } else if let Some(s) = defs::building_def(kind).map(|d| d.stats) {
        AttackProfile {
            range_tiles: s.range_tiles,
            dmg: s.dmg,
            cooldown: s.cooldown,
        }
    } else {
        AttackProfile {
            range_tiles: 0,
            dmg: 0,
            cooldown: 0,
        }
    }
}

/// Armored targets take 75% damage reduction from non-AP weapons.
pub fn is_armored(kind: EntityKind) -> bool {
    let armor_class = defs::unit_def(kind)
        .map(|d| d.armor_class)
        .or_else(|| defs::building_def(kind).map(|d| d.armor_class));
    armor_class == Some(ArmorClass::Armored)
}

/// AP weapons deal full damage to armored targets.
pub fn is_ap(kind: EntityKind) -> bool {
    weapon(kind) == WeaponClass::AntiTank
}

/// AT teams prefer armored targets over all others.
pub fn prefers_armored_targets(kind: EntityKind) -> bool {
    defs::unit_def(kind)
        .map(|d| d.target_priority == TargetPriority::PrefersArmored)
        .unwrap_or(false)
}

/// Miss probability [0.0, 1.0) for an attack. AT guns have a high miss rate against
/// infantry-sized targets — the shell flies straight through without finding anyone.
/// Hits that do connect deal full damage.
pub fn miss_chance(attacker_kind: EntityKind, victim_kind: EntityKind) -> f32 {
    if attacker_kind == EntityKind::AtTeam && at_team_miss_target(victim_kind) {
        0.65
    } else {
        0.0
    }
}

fn at_team_miss_target(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Worker | EntityKind::Rifleman | EntityKind::MachineGunner
    )
}

/// Applies the AP/armor damage formula. The miss_chance roll is handled at the call site.
pub fn effective_damage(
    attacker_kind: EntityKind,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
) -> u32 {
    let armor_adjusted = if !is_ap(attacker_kind) && is_armored(victim_kind) {
        base_dmg / 4
    } else {
        base_dmg
    };
    let terrain = victim_terrain.unwrap_or(TerrainKind::Open);
    (armor_adjusted as f32 * terrain::cover_modifier(victim_kind, terrain)).round() as u32
}

pub fn classify_armor_facing(
    victim_facing: f32,
    victim_pos: (f32, f32),
    attacker_pos: (f32, f32),
) -> ArmorFacing {
    let attacker_angle = (attacker_pos.1 - victim_pos.1).atan2(attacker_pos.0 - victim_pos.0);
    let angle_error = normalized_angle_delta(attacker_angle, victim_facing).abs();
    if angle_error <= FRONT_ARC_RAD {
        ArmorFacing::Front
    } else if angle_error <= SIDE_ARC_RAD {
        ArmorFacing::Side
    } else {
        ArmorFacing::Rear
    }
}

pub fn facing_damage_multiplier(
    attacker_kind: EntityKind,
    victim_kind: EntityKind,
    facing: ArmorFacing,
) -> f32 {
    if victim_kind != EntityKind::Tank {
        return 1.0;
    }
    if !matches!(attacker_kind, EntityKind::Tank | EntityKind::AtTeam) {
        return 1.0;
    }
    match facing {
        ArmorFacing::Front => FRONT_ARMOR_DAMAGE_MULTIPLIER,
        ArmorFacing::Side => SIDE_ARMOR_DAMAGE_MULTIPLIER,
        ArmorFacing::Rear => REAR_ARMOR_DAMAGE_MULTIPLIER,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn effective_damage_with_facing(
    attacker_kind: EntityKind,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
    victim_facing: Option<f32>,
    victim_pos: (f32, f32),
    attacker_pos: (f32, f32),
) -> u32 {
    let damage = effective_damage(attacker_kind, victim_kind, base_dmg, victim_terrain);
    let Some(victim_facing) = victim_facing.filter(|facing| facing.is_finite()) else {
        return damage;
    };
    let facing = classify_armor_facing(victim_facing, victim_pos, attacker_pos);
    let multiplier = facing_damage_multiplier(attacker_kind, victim_kind, facing);
    ((damage as f32) * multiplier).round().max(0.0) as u32
}

fn weapon(kind: EntityKind) -> WeaponClass {
    defs::unit_def(kind)
        .map(|d| d.weapon)
        .or_else(|| defs::building_def(kind).map(|d| d.weapon))
        .unwrap_or(WeaponClass::None)
}

fn normalized_angle_delta(from: f32, to: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    (from - to + std::f32::consts::PI).rem_euclid(two_pi) - std::f32::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ap_vs_armored_full_damage() {
        assert_eq!(
            effective_damage(EntityKind::AtTeam, EntityKind::Tank, 40, None),
            40
        );
    }

    #[test]
    fn non_ap_vs_armored_reduced() {
        assert_eq!(
            effective_damage(EntityKind::Rifleman, EntityKind::Tank, 40, None),
            10
        );
    }

    #[test]
    fn ap_vs_small_full_damage_on_hit() {
        assert_eq!(
            effective_damage(EntityKind::AtTeam, EntityKind::Rifleman, 20, None),
            20
        );
    }

    #[test]
    fn non_ap_vs_unarmored_full_damage() {
        assert_eq!(
            effective_damage(EntityKind::Rifleman, EntityKind::Rifleman, 20, None),
            20
        );
    }

    #[test]
    fn tank_ap_vs_building_full_damage() {
        assert_eq!(
            effective_damage(EntityKind::Tank, EntityKind::Barracks, 50, None),
            50
        );
    }

    #[test]
    fn infantry_vs_building_reduced() {
        assert_eq!(
            effective_damage(EntityKind::MachineGunner, EntityKind::Depot, 40, None),
            10
        );
    }

    #[test]
    fn open_terrain_keeps_current_damage_values() {
        assert_eq!(
            effective_damage(
                EntityKind::Rifleman,
                EntityKind::Rifleman,
                20,
                Some(TerrainKind::Open)
            ),
            20
        );
        assert_eq!(
            effective_damage(
                EntityKind::Rifleman,
                EntityKind::Tank,
                40,
                Some(TerrainKind::Open)
            ),
            10
        );
    }

    #[test]
    fn combat_classification_matches_phase_1_table() {
        let expected = [
            (EntityKind::Worker, false, false, false),
            (EntityKind::Rifleman, false, false, false),
            (EntityKind::MachineGunner, false, false, false),
            (EntityKind::AtTeam, false, true, true),
            (EntityKind::ScoutCar, false, false, false),
            (EntityKind::Tank, true, true, false),
            (EntityKind::CityCentre, true, false, false),
            (EntityKind::Depot, true, false, false),
            (EntityKind::Barracks, true, false, false),
            (EntityKind::TrainingCentre, true, false, false),
            (EntityKind::Factory, true, false, false),
            (EntityKind::Steelworks, true, false, false),
            (EntityKind::Steel, false, false, false),
            (EntityKind::Oil, false, false, false),
        ];

        for (kind, armored, ap, prefers_armored) in expected {
            assert_eq!(is_armored(kind), armored, "{kind} armor classification");
            assert_eq!(is_ap(kind), ap, "{kind} AP classification");
            assert_eq!(
                prefers_armored_targets(kind),
                prefers_armored,
                "{kind} target priority"
            );
        }
    }

    #[test]
    fn at_team_miss_chance_applies_only_to_infantry_sized_targets() {
        assert_eq!(miss_chance(EntityKind::AtTeam, EntityKind::Worker), 0.65);
        assert_eq!(miss_chance(EntityKind::AtTeam, EntityKind::Rifleman), 0.65);
        assert_eq!(
            miss_chance(EntityKind::AtTeam, EntityKind::MachineGunner),
            0.65
        );

        assert_eq!(miss_chance(EntityKind::AtTeam, EntityKind::ScoutCar), 0.0);
        assert_eq!(miss_chance(EntityKind::AtTeam, EntityKind::AtTeam), 0.0);
        assert_eq!(miss_chance(EntityKind::AtTeam, EntityKind::Tank), 0.0);
    }

    #[test]
    fn tank_front_hit_uses_normal_at_damage() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::AtTeam,
                EntityKind::Tank,
                48,
                None,
                Some(0.0),
                (100.0, 100.0),
                (140.0, 100.0),
            ),
            48
        );
    }

    #[test]
    fn tank_side_hit_boosts_at_damage() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::AtTeam,
                EntityKind::Tank,
                48,
                None,
                Some(0.0),
                (100.0, 100.0),
                (100.0, 140.0),
            ),
            60
        );
    }

    #[test]
    fn tank_rear_hit_boosts_at_damage() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::AtTeam,
                EntityKind::Tank,
                48,
                None,
                Some(0.0),
                (100.0, 100.0),
                (60.0, 100.0),
            ),
            84
        );
    }

    #[test]
    fn tank_shell_uses_same_facing_modifiers_against_tank() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Tank,
                EntityKind::Tank,
                60,
                None,
                Some(0.0),
                (100.0, 100.0),
                (140.0, 100.0),
            ),
            60
        );
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Tank,
                EntityKind::Tank,
                60,
                None,
                Some(0.0),
                (100.0, 100.0),
                (100.0, 140.0),
            ),
            75
        );
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Tank,
                EntityKind::Tank,
                60,
                None,
                Some(0.0),
                (100.0, 100.0),
                (60.0, 100.0),
            ),
            105
        );
    }

    #[test]
    fn rifleman_vs_rifleman_ignores_facing() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Rifleman,
                EntityKind::Rifleman,
                5,
                None,
                Some(0.0),
                (100.0, 100.0),
                (60.0, 100.0),
            ),
            5
        );
    }

    #[test]
    fn tank_vs_building_ignores_facing() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Tank,
                EntityKind::Barracks,
                60,
                None,
                Some(0.0),
                (100.0, 100.0),
                (60.0, 100.0),
            ),
            60
        );
    }

    #[test]
    fn facing_classification_wraps_around_pi() {
        assert_eq!(
            classify_armor_facing(std::f32::consts::PI - 0.05, (100.0, 100.0), (60.0, 98.0),),
            ArmorFacing::Front
        );
        assert_eq!(
            classify_armor_facing(-std::f32::consts::PI + 0.05, (100.0, 100.0), (60.0, 102.0),),
            ArmorFacing::Front
        );
    }
}
