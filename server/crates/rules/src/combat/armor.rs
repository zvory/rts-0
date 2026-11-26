//! Shared native and additive runtime armor resolution.
use super::{
    apply_facing_damage_multiplier, armor_class, damage_after_target_type_modifier, ArmorClass,
    WeaponProfile,
};
use crate::terrain::{self, TerrainKind};
use crate::EntityKind;

/// Apply a percentage armor layer using the weapon's ordinary armor penetration.
pub fn damage_after_armor_reduction(damage: u32, reduction: f32, penetration: f32) -> u32 {
    ((damage as f32) * (1.0 - reduction * (1.0 - penetration.clamp(0.0, 1.0)))).round() as u32
}

/// Resolve armor and terrain protection for a hit with explicit penetration (0 = none).
pub fn effective_damage_for_armor_penetration(
    armor_penetration: f32,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
) -> u32 {
    effective_damage_for_armor_penetration_with_bonus(
        armor_penetration,
        victim_kind,
        base_dmg,
        victim_terrain,
        0.0,
    )
}

/// Runtime armor adds percentage points to native armor, capped before penetration.
pub fn effective_damage_for_armor_penetration_with_bonus(
    armor_penetration: f32,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
    armor_bonus: f32,
) -> u32 {
    let armor_class = armor_class(victim_kind);
    let penetration = armor_penetration.clamp(0.0, 1.0);
    let armor_adjusted = if armor_bonus > 0.0 {
        let native = if armor_class == Some(ArmorClass::Armored) {
            0.75
        } else {
            0.0
        };
        damage_after_armor_reduction(
            base_dmg,
            (native + armor_bonus).clamp(0.0, 1.0),
            penetration,
        )
    } else {
        match armor_class {
            Some(ArmorClass::Armored) if penetration <= 0.0 => base_dmg / 4,
            Some(ArmorClass::Armored) => damage_after_armor_reduction(base_dmg, 0.75, penetration),
            _ => base_dmg,
        }
    };
    let terrain = victim_terrain.unwrap_or(TerrainKind::Open);
    (armor_adjusted as f32 * terrain::cover_modifier(victim_kind, terrain)).round() as u32
}

#[allow(clippy::too_many_arguments)]
pub fn effective_damage_with_facing_for_weapon(
    profile: &WeaponProfile,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
    victim_facing: Option<f32>,
    victim_pos: (f32, f32),
    attacker_pos: (f32, f32),
) -> u32 {
    effective_damage_with_facing_for_weapon_and_armor_bonus(
        profile,
        victim_kind,
        base_dmg,
        victim_terrain,
        victim_facing,
        victim_pos,
        attacker_pos,
        0.0,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn effective_damage_with_facing_for_weapon_and_armor_bonus(
    profile: &WeaponProfile,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
    victim_facing: Option<f32>,
    victim_pos: (f32, f32),
    attacker_pos: (f32, f32),
    armor_bonus: f32,
) -> u32 {
    let damage = effective_damage_for_armor_penetration_with_bonus(
        profile.armor_penetration,
        victim_kind,
        base_dmg,
        victim_terrain,
        armor_bonus,
    );
    let damage = damage_after_target_type_modifier(profile, victim_kind, damage);
    apply_facing_damage_multiplier(
        damage,
        Some(profile),
        victim_kind,
        victim_facing,
        victim_pos,
        attacker_pos,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additive_armor_is_capped_before_penetration_and_rounded_once() {
        for (kind, bonus, penetration, expected) in [
            (EntityKind::WindAdept, 0.5, 0.0, 50),
            (EntityKind::WarAdept, 1.0, 0.0, 0),
            (EntityKind::WarAdept, 1.0, 0.5, 50),
            (EntityKind::Tank, 0.5, 0.0, 0),
            (EntityKind::Tank, 0.5, 0.5, 50),
            (EntityKind::Tank, 0.5, 1.0, 100),
        ] {
            assert_eq!(
                effective_damage_for_armor_penetration_with_bonus(
                    penetration,
                    kind,
                    100,
                    None,
                    bonus
                ),
                expected
            );
        }
        assert_eq!(
            effective_damage_for_armor_penetration_with_bonus(
                0.0,
                EntityKind::WindAdept,
                3,
                None,
                0.5
            ),
            2
        );
        // Preserve the existing integer-quarter native-armor path when no bonus exists.
        assert_eq!(
            effective_damage_for_armor_penetration(0.0, EntityKind::Tank, 3, None),
            0
        );
    }
}
