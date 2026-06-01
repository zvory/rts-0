//! Pure combat rules: classification predicates and damage formula.

use crate::config;
use crate::game::entity::EntityKind;

/// Attack profile for a combat-capable unit or building.
pub struct AttackProfile {
    pub range_tiles: u32,
    pub dmg: u32,
    pub cooldown: u32,
}

/// Returns the attack profile for the given kind, or zeroes if non-combatant.
pub fn attack_profile(kind: EntityKind) -> AttackProfile {
    if let Some(s) = config::unit_stats(kind) {
        AttackProfile { range_tiles: s.range_tiles, dmg: s.dmg, cooldown: s.cooldown }
    } else if let Some(s) = config::building_stats(kind) {
        AttackProfile { range_tiles: s.range_tiles, dmg: s.dmg, cooldown: s.cooldown }
    } else {
        AttackProfile { range_tiles: 0, dmg: 0, cooldown: 0 }
    }
}

/// Armored targets take 75% damage reduction from non-AP weapons.
pub fn is_armored(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Tank) || kind.is_building()
}

/// AP weapons deal full damage to armored targets.
pub fn is_ap(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::AtTeam | EntityKind::Tank)
}

/// AT teams prefer armored targets over all others.
pub fn prefers_armored_targets(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::AtTeam)
}

/// Applies the AP/armor damage formula.
pub fn effective_damage(attacker_kind: EntityKind, victim_kind: EntityKind, base_dmg: u32) -> u32 {
    if is_armored(victim_kind) && !is_ap(attacker_kind) {
        base_dmg / 4
    } else {
        base_dmg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ap_vs_armored_full_damage() {
        assert_eq!(effective_damage(EntityKind::AtTeam, EntityKind::Tank, 40), 40);
    }

    #[test]
    fn non_ap_vs_armored_reduced() {
        assert_eq!(effective_damage(EntityKind::Rifleman, EntityKind::Tank, 40), 10);
    }

    #[test]
    fn ap_vs_unarmored_full_damage() {
        assert_eq!(effective_damage(EntityKind::AtTeam, EntityKind::Rifleman, 20), 20);
    }

    #[test]
    fn non_ap_vs_unarmored_full_damage() {
        assert_eq!(effective_damage(EntityKind::Rifleman, EntityKind::Rifleman, 20), 20);
    }

    #[test]
    fn tank_ap_vs_building_full_damage() {
        assert_eq!(effective_damage(EntityKind::Tank, EntityKind::Barracks, 50), 50);
    }

    #[test]
    fn infantry_vs_building_reduced() {
        assert_eq!(effective_damage(EntityKind::MachineGunner, EntityKind::Depot, 40), 10);
    }
}
