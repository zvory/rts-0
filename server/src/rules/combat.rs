//! Pure combat rules: classification predicates and damage formula.

use crate::game::entity::EntityKind;
use crate::rules::defs::{self, ArmorClass, TargetPriority, WeaponClass};
use crate::rules::terrain::{self, TerrainKind};

/// Attack profile for a combat-capable unit or building.
pub struct AttackProfile {
    pub range_tiles: u32,
    pub dmg: u32,
    pub cooldown: u32,
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

/// Miss probability [0.0, 1.0) for an attack. AP weapons have a high miss rate against small
/// targets (dispersed infantry) — the shell flies straight through without finding anyone.
/// Hits that do connect deal full damage.
pub fn miss_chance(attacker_kind: EntityKind, victim_kind: EntityKind) -> f32 {
    if attacker_kind == EntityKind::AtTeam && !is_armored(victim_kind) {
        0.65
    } else {
        0.0
    }
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

fn weapon(kind: EntityKind) -> WeaponClass {
    defs::unit_def(kind)
        .map(|d| d.weapon)
        .or_else(|| defs::building_def(kind).map(|d| d.weapon))
        .unwrap_or(WeaponClass::None)
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
            (EntityKind::Tank, true, true, false),
            (EntityKind::IndustrialCenter, true, false, false),
            (EntityKind::Depot, true, false, false),
            (EntityKind::Barracks, true, false, false),
            (EntityKind::TrainingCentre, true, false, false),
            (EntityKind::TankFactory, true, false, false),
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
}
