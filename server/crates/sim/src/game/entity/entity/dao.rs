use super::*;
use crate::game::upgrade::UpgradeKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DiscipleShield {
    pub id: u32,
    pub x: Coord,
    pub y: Coord,
    pub remaining: u32,
}

impl Entity {
    pub(in crate::game) fn place_shield(
        &mut self,
        id: u32,
        x: f32,
        y: f32,
        cooldown: u16,
        preserve_order: bool,
    ) {
        self.shield = Some(DiscipleShield {
            id,
            x: Coord::from_world(x),
            y: Coord::from_world(y),
            remaining: config::DISCIPLE_SHIELD_CAPACITY,
        });
        self.start_ability_cooldown(AbilityKind::DiscipleShield, cooldown);
        if !preserve_order {
            self.clear_active_order();
        }
    }

    pub(crate) fn units_killed(&self) -> u32 {
        self.units_killed
    }

    pub(in crate::game) fn record_kill(&mut self) {
        self.units_killed = self.units_killed.saturating_add(1);
    }

    pub(crate) fn dao_ability_available(&self, ability: AbilityKind) -> bool {
        if self.kind != EntityKind::Warrior {
            return true;
        }
        match ability {
            AbilityKind::EvolveLightness | AbilityKind::EvolveWar | AbilityKind::EvolveReality => {
                self.dao.is_none() && self.insight >= config::DISCIPLE_INSIGHT_REQUIRED
            }
            AbilityKind::EkatTeleport => self.dao == Some(UpgradeKind::DaoOfLightness),
            AbilityKind::DiscipleShield | AbilityKind::EkatMagicAnchor => {
                self.dao == Some(UpgradeKind::DaoOfBarriers)
            }
            _ => true,
        }
    }

    pub(crate) fn evolve_dao(&mut self, ability: AbilityKind) -> bool {
        if self.kind != EntityKind::Warrior || !self.dao_ability_available(ability) {
            return false;
        }
        self.dao = Some(match ability {
            AbilityKind::EvolveLightness => UpgradeKind::DaoOfLightness,
            AbilityKind::EvolveWar => UpgradeKind::DaoOfHeaviness,
            AbilityKind::EvolveReality => UpgradeKind::DaoOfBarriers,
            _ => return false,
        });
        self.dao_advance_ticks = 15;
        if self.dao == Some(UpgradeKind::DaoOfHeaviness) {
            self.max_hp = self.max_hp.saturating_add(config::DAO_WAR_HP_BONUS);
            self.hp = self
                .hp
                .saturating_add(config::DAO_WAR_HP_BONUS)
                .min(self.max_hp);
        }
        true
    }

    pub(crate) fn dao_damage_after_armor(&self, damage: u32, penetration: f32) -> u32 {
        if self.dao != Some(UpgradeKind::DaoOfHeaviness) {
            return damage;
        }
        ((damage as f32)
            * (1.0 - config::DAO_WAR_ARMOR_REDUCTION * (1.0 - penetration.clamp(0.0, 1.0))))
        .round() as u32
    }

    pub(crate) fn movement_speed(&self) -> f32 {
        let base = config::unit_stats(self.kind).map_or(0.0, |s| s.speed);
        match self.dao {
            Some(UpgradeKind::DaoOfLightness) => config::dao_lightness_speed(),
            Some(UpgradeKind::DaoOfBarriers) => base * config::DAO_REALITY_SPEED_MULTIPLIER,
            _ => base,
        }
    }
}
