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
    pub(crate) fn dao(&self) -> Option<UpgradeKind> {
        self.kind.dao()
    }

    // Isolated fixtures may switch identity without simulating insight or healing.
    #[cfg(test)]
    pub(in crate::game) fn set_disciple_kind_for_test(&mut self, kind: EntityKind) {
        assert!(self.kind.is_disciple() && kind.is_disciple());
        self.kind = kind;
    }

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

    pub(crate) fn insight_cap(&self) -> u32 {
        if self.dao_master {
            0
        } else if self.dao().is_some() {
            config::DISCIPLE_EVOLVED_INSIGHT_CAP
        } else {
            config::DISCIPLE_INSIGHT_REQUIRED
        }
    }

    /// Credit actual hostile HP loss using exact cost-weighted fractions.
    pub(in crate::game) fn gain_insight(&mut self, damage: u32, target_kind: EntityKind) {
        if !self.kind.is_disciple() || self.insight >= self.insight_cap() {
            return;
        }
        let (numerator, denominator) = config::disciple_insight_ratio(self.kind, target_kind);
        let total = u64::from(damage)
            .saturating_mul(numerator)
            .saturating_add(self.insight_fraction);
        let earned = (total / denominator).min(u64::from(self.insight_cap() - self.insight));
        self.insight += earned as u32;
        self.insight_fraction = if self.insight == self.insight_cap() {
            0
        } else {
            total % denominator
        };
    }

    pub(crate) fn units_killed(&self) -> u32 {
        self.units_killed
    }

    pub(in crate::game) fn record_kill(&mut self) {
        self.units_killed = self.units_killed.saturating_add(1);
    }

    pub(crate) fn dao_ability_available(&self, ability: AbilityKind) -> bool {
        if !self.kind.is_disciple() {
            return true;
        }
        match ability {
            AbilityKind::EvolveLightness | AbilityKind::EvolveWar | AbilityKind::EvolveMagic => {
                self.dao().is_none() && self.insight >= config::DISCIPLE_INSIGHT_REQUIRED
            }
            AbilityKind::ThrowSword => {
                self.dao() == Some(UpgradeKind::DaoOfHeaviness) && self.dao_master
            }
            AbilityKind::LightnessDash => self.dao() == Some(UpgradeKind::DaoOfLightness),
            AbilityKind::DiscipleShield | AbilityKind::EkatMagicAnchor => {
                self.dao() == Some(UpgradeKind::DaoOfMagic)
            }
            _ => true,
        }
    }

    /// Persistent intent is independent of combat/movement orders and can be replaced.
    pub(in crate::game) fn choose_dao(&mut self, ability: AbilityKind) -> bool {
        if self.kind != EntityKind::Warrior || self.hp == 0 {
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

    pub(in crate::game) fn advance_chosen_dao(&mut self) {
        let Some(choice) = self.chosen_dao else {
            return;
        };
        if self.hp == 0 {
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

    /// Second-rank advancement preserves orders/reload and grants Lightness its extra charge.
    pub(in crate::game) fn advance_dao_master(&mut self) {
        if !self.kind.is_disciple()
            || self.hp == 0
            || self.dao().is_none()
            || self.dao_master
            || self.insight < config::DISCIPLE_EVOLVED_INSIGHT_CAP
        {
            return;
        }
        let dash_uses = self.ability_uses_remaining(AbilityKind::LightnessDash);
        self.dao_master = true;
        if self.kind == EntityKind::LightnessAdept {
            self.ability_uses_remaining.insert(
                AbilityKind::LightnessDash,
                dash_uses.unwrap_or(1).saturating_add(1),
            );
        }
        self.insight = 0;
        self.insight_fraction = 0;
        self.max_hp = ((self.max_hp as f32) * config::DAO_MASTER_HP_MULTIPLIER).round() as u32;
        self.hp = self.max_hp;
        self.dao_advance_ticks = 15;
    }

    pub(crate) fn evolve_dao(&mut self, ability: AbilityKind) -> bool {
        if self.kind != EntityKind::Warrior || self.hp == 0 || !self.dao_ability_available(ability)
        {
            return false;
        }
        let attack_cd = self.attack_cd();
        self.kind = match ability {
            AbilityKind::EvolveLightness => EntityKind::LightnessAdept,
            AbilityKind::EvolveWar => EntityKind::WarAdept,
            AbilityKind::EvolveMagic => EntityKind::MagicAdept,
            _ => return false,
        };
        // Evolution changes the primary weapon without granting a free extra shot.
        self.set_attack_cd(attack_cd);
        self.chosen_dao = None;
        self.insight = 0;
        self.insight_fraction = 0;
        // Specialists cannot enter tournaments. Keep their movement order, but drop admission
        // intent immediately so evolution late in the tick leaves checkpoint-valid state.
        if let Some(movement) = self.movement.as_mut() {
            movement.tournament_target = None;
        }
        self.dao_advance_ticks = 15;
        self.max_hp = config::unit_stats(self.kind).map_or(self.max_hp, |stats| stats.hp);
        self.hp = self.max_hp;
        true
    }

    pub(crate) fn dao_damage_after_armor(&self, damage: u32, penetration: f32) -> u32 {
        if self.dao() != Some(UpgradeKind::DaoOfHeaviness) {
            return damage;
        }
        ((damage as f32)
            * (1.0 - config::DAO_WAR_ARMOR_REDUCTION * (1.0 - penetration.clamp(0.0, 1.0))))
        .round() as u32
    }

    pub(in crate::game) fn try_start_novice_charge(
        &mut self,
        target_position: (f32, f32),
        target_movement: (f32, f32),
        hostile_unit: bool,
    ) {
        if self.kind != EntityKind::Warrior
            || self.hp == 0
            || !hostile_unit
            || self.ability_cooldown_ticks(AbilityKind::Charge) > 0
        {
            return;
        }
        let dx = target_position.0 - self.pos_x();
        let dy = target_position.1 - self.pos_y();
        let (mx, my) = target_movement;
        let min_distance = (config::NOVICE_CHARGE_MIN_DISTANCE_TILES * config::TILE_SIZE) as f32;
        if dx * dx + dy * dy < min_distance * min_distance && dx * mx + dy * my <= 0.0 {
            return;
        }
        if let Some(movement) = self.movement.as_mut() {
            movement.novice_charge_ticks = config::NOVICE_CHARGE_DURATION_TICKS;
            self.start_ability_cooldown(AbilityKind::Charge, config::NOVICE_CHARGE_COOLDOWN_TICKS);
        }
    }

    pub(crate) fn movement_speed(&self) -> f32 {
        let base = config::unit_stats(self.kind).map_or(0.0, |s| s.speed);
        if self.kind == EntityKind::Warrior
            && self
                .movement
                .as_ref()
                .is_some_and(|m| m.novice_charge_ticks > 0)
        {
            base * config::NOVICE_CHARGE_SPEED_MULTIPLIER
        } else {
            base * if self.dao_master {
                config::DAO_MASTER_SPEED_MULTIPLIER
            } else {
                1.0
            }
        }
    }
}
