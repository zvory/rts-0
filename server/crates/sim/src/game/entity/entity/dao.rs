use super::*;
use crate::game::upgrade::UpgradeKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DiscipleShield {
    pub id: u32,
    pub x: Coord,
    pub y: Coord,
    pub remaining: u32,
    pub expires_tick: u32,
}

impl Entity {
    pub(in crate::game) fn expire_shield(&mut self, tick: u32) {
        if self
            .shield
            .as_ref()
            .is_some_and(|shield| shield.expires_tick <= tick)
        {
            self.shield = None;
        }
    }

    pub(in crate::game) fn place_shield(
        &mut self,
        id: u32,
        x: f32,
        y: f32,
        cooldown: u16,
        expires_tick: u32,
        preserve_order: bool,
    ) {
        self.shield = Some(DiscipleShield {
            id,
            x: Coord::from_world(x),
            y: Coord::from_world(y),
            remaining: config::DISCIPLE_SHIELD_CAPACITY,
            expires_tick,
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
            AbilityKind::EvolveLightness | AbilityKind::EvolveWar | AbilityKind::EvolveMagic => {
                self.dao.is_none() && self.insight >= config::DISCIPLE_INSIGHT_REQUIRED
            }
            AbilityKind::ThrowSword => self.dao == Some(UpgradeKind::DaoOfHeaviness),
            AbilityKind::EkatTeleport => self.dao == Some(UpgradeKind::DaoOfLightness),
            AbilityKind::DiscipleShield | AbilityKind::EkatMagicAnchor => {
                self.dao == Some(UpgradeKind::DaoOfMagic)
            }
            _ => true,
        }
    }

    /// Persistent intent is independent of combat/movement orders and can be replaced.
    pub(in crate::game) fn choose_dao(&mut self, ability: AbilityKind) -> bool {
        if self.kind != EntityKind::Warrior || self.hp == 0 || self.dao.is_some() {
            return false;
        }
        self.chosen_dao = Some(match ability {
            AbilityKind::EvolveLightness => UpgradeKind::DaoOfLightness,
            AbilityKind::EvolveWar => UpgradeKind::DaoOfHeaviness,
            AbilityKind::EvolveMagic => UpgradeKind::DaoOfMagic,
            _ => return false,
        });
        true
    }

    pub(in crate::game) fn advance_chosen_dao(&mut self, researched: impl Fn(UpgradeKind) -> bool) {
        let Some(choice) = self.chosen_dao else {
            return;
        };
        if self.hp == 0 || !researched(choice) {
            return;
        }
        let ability = match choice {
            UpgradeKind::DaoOfLightness => AbilityKind::EvolveLightness,
            UpgradeKind::DaoOfHeaviness => AbilityKind::EvolveWar,
            UpgradeKind::DaoOfMagic => AbilityKind::EvolveMagic,
            _ => return,
        };
        self.evolve_dao(ability);
    }

    pub(crate) fn evolve_dao(&mut self, ability: AbilityKind) -> bool {
        if self.kind != EntityKind::Warrior || self.hp == 0 || !self.dao_ability_available(ability)
        {
            return false;
        }
        let attack_cd = self.attack_cd();
        self.dao = Some(match ability {
            AbilityKind::EvolveLightness => UpgradeKind::DaoOfLightness,
            AbilityKind::EvolveWar => UpgradeKind::DaoOfHeaviness,
            AbilityKind::EvolveMagic => UpgradeKind::DaoOfMagic,
            _ => return false,
        });
        // Evolution changes the primary weapon without granting a free extra shot.
        self.set_attack_cd(attack_cd);
        self.chosen_dao = None;
        // Specialists cannot enter tournaments. Keep their movement order, but drop admission
        // intent immediately so evolution late in the tick leaves checkpoint-valid state.
        if let Some(movement) = self.movement.as_mut() {
            movement.tournament_target = None;
        }
        self.dao_advance_ticks = 15;
        if self.dao == Some(UpgradeKind::DaoOfHeaviness) {
            self.max_hp = self.max_hp.saturating_add(config::DAO_WAR_HP_BONUS);
        }
        self.hp = self.max_hp;
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
            Some(UpgradeKind::DaoOfHeaviness) => base * config::DAO_WAR_SPEED_MULTIPLIER,
            Some(UpgradeKind::DaoOfMagic) => base * config::DAO_MAGIC_SPEED_MULTIPLIER,
            _ => base,
        }
    }
}
