use super::{default_weapon_profile, EntityKind, WeaponProfile, FULL_ARMOR_PENETRATION};

/// Resolve primary penetration by specialization and rank without changing ability weapons.
pub fn ranked_primary_weapon_profile(kind: EntityKind, master: bool) -> Option<WeaponProfile> {
    let mut profile = *default_weapon_profile(kind)?;
    profile.armor_penetration = match kind {
        EntityKind::WarAdept => FULL_ARMOR_PENETRATION,
        EntityKind::WindAdept | EntityKind::MagicAdept => {
            if master {
                0.5
            } else {
                0.25
            }
        }
        _ => profile.armor_penetration,
    };
    Some(profile)
}

/// Rank damage is rounded before the War Master bonus and carried-item multipliers.
pub fn ranked_primary_damage(kind: EntityKind, master: bool, damage: u32) -> u32 {
    if !master {
        return damage;
    }
    let damage = (damage as f32 * crate::balance::DAO_MASTER_DAMAGE_MULTIPLIER).round() as u32;
    if kind == EntityKind::WarAdept {
        damage.saturating_mul(crate::balance::DAO_WAR_MASTER_DAMAGE_MULTIPLIER)
    } else {
        damage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranked_primary_penetration_keeps_novice_and_non_disciple_weapons_unchanged() {
        for (kind, master, penetration) in [
            (EntityKind::Warrior, false, 0.5),
            (EntityKind::WindAdept, false, 0.25),
            (EntityKind::WindAdept, true, 0.5),
            (EntityKind::MagicAdept, false, 0.25),
            (EntityKind::MagicAdept, true, 0.5),
            (EntityKind::WarAdept, false, 1.0),
            (EntityKind::WarAdept, true, 1.0),
            (EntityKind::Tank, false, 1.0),
        ] {
            assert_eq!(
                ranked_primary_weapon_profile(kind, master)
                    .unwrap()
                    .armor_penetration,
                penetration
            );
        }
    }
}
