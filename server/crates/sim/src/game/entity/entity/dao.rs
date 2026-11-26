use super::*;
use crate::game::upgrade::UpgradeKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DiscipleShield {
    pub id: u32,
    pub x: Coord,
    pub y: Coord,
    pub remaining: u32,
    pub capacity: u32,
    pub duration_ticks: u32,
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
        let capacity = if self.kind == EntityKind::MagicAdept && self.dao_master {
            config::MAGIC_MASTER_SHIELD_CAPACITY
        } else {
            config::DISCIPLE_SHIELD_CAPACITY
        };
        self.shield = Some(DiscipleShield {
            id,
            x: Coord::from_world(x),
            y: Coord::from_world(y),
            remaining: capacity,
            capacity,
            duration_ticks: config::disciple_shield_duration_ticks(self.dao_master),
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

    /// Credit actual hostile HP loss with exact fractional carry for building damage.
    pub(in crate::game) fn gain_insight(&mut self, damage: u32, target_kind: EntityKind) {
        if !self.kind.is_disciple() || self.insight >= self.insight_cap() {
            return;
        }
        let (numerator, denominator) = config::disciple_insight_ratio(self.kind, target_kind);
        let total = u64::from(damage)
            .saturating_mul(numerator)
            .saturating_mul(if self.shop_item_active(AbilityKind::DoubleXp) {
                2
            } else {
                1
            })
            .saturating_add(self.insight_fraction);
        let earned = (total / denominator).min(u64::from(self.insight_cap() - self.insight));
        self.insight += earned as u32;
        self.insight_fraction = if self.insight == self.insight_cap() {
            0
        } else {
            total % denominator
        };
    }

    /// Support rewards are whole insight and never credit dead casters or Masters.
    pub(in crate::game) fn gain_magic_insight(&mut self, amount: u32) {
        if self.kind != EntityKind::MagicAdept || self.hp == 0 || self.dao_master {
            return;
        }
        let amount = amount.saturating_mul(if self.shop_item_active(AbilityKind::DoubleXp) {
            2
        } else {
            1
        });
        self.insight = self.insight.saturating_add(amount).min(self.insight_cap());
        if self.insight == self.insight_cap() {
            self.insight_fraction = 0;
        }
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
            AbilityKind::EvolveWind | AbilityKind::EvolveWar | AbilityKind::EvolveMagic => {
                self.dao().is_none() && self.insight >= config::DISCIPLE_INSIGHT_REQUIRED
            }
            AbilityKind::ThrowSword => self.dao() == Some(UpgradeKind::DaoOfHeaviness),
            AbilityKind::WindDash => self.dao() == Some(UpgradeKind::DaoWind),
            AbilityKind::DiscipleShield | AbilityKind::VectorField => {
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
            AbilityKind::EvolveWind => UpgradeKind::DaoWind,
            AbilityKind::EvolveWar => UpgradeKind::DaoOfHeaviness,
            AbilityKind::EvolveMagic => UpgradeKind::DaoOfMagic,
            _ => return false,
        });
        true
    }

    /// Ready novices without player intent pick uniformly from the three paths.
    pub(in crate::game) fn advance_dao(&mut self, rng: &mut impl rand::Rng) {
        if self.kind != EntityKind::Warrior
            || self.hp == 0
            || self.insight < config::DISCIPLE_INSIGHT_REQUIRED
        {
            return;
        }
        if self.chosen_dao.is_none() {
            self.chosen_dao = Some(match rng.gen_range(0..3) {
                0 => UpgradeKind::DaoWind,
                1 => UpgradeKind::DaoOfHeaviness,
                _ => UpgradeKind::DaoOfMagic,
            });
        }
        self.advance_chosen_dao();
    }

    pub(in crate::game) fn advance_chosen_dao(&mut self) {
        let Some(choice) = self.chosen_dao else {
            return;
        };
        if self.hp == 0 {
            return;
        }
        let ability = match choice {
            UpgradeKind::DaoWind => AbilityKind::EvolveWind,
            UpgradeKind::DaoOfHeaviness => AbilityKind::EvolveWar,
            UpgradeKind::DaoOfMagic => AbilityKind::EvolveMagic,
            _ => return,
        };
        self.evolve_dao(ability);
    }

    /// Second-rank advancement preserves orders/reload and grants Wind its extra charge.
    pub(in crate::game) fn advance_dao_master(&mut self) {
        if !self.kind.is_disciple()
            || self.hp == 0
            || self.dao().is_none()
            || self.dao_master
            || self.insight < config::DISCIPLE_EVOLVED_INSIGHT_CAP
        {
            return;
        }
        let dash_uses = self.ability_uses_remaining(AbilityKind::WindDash);
        self.dao_master = true;
        if self.kind == EntityKind::WindAdept {
            self.ability_uses_remaining.insert(
                AbilityKind::WindDash,
                dash_uses.unwrap_or(1).saturating_add(1),
            );
        }
        self.insight = 0;
        self.insight_fraction = 0;
        let vitality = self.vitality_health_bonus();
        self.max_hp = (((self.max_hp.saturating_sub(vitality)) as f32)
            * config::DAO_MASTER_HP_MULTIPLIER)
            .round() as u32;
        self.max_hp = self.max_hp.saturating_add(vitality);
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
            AbilityKind::EvolveWind => EntityKind::WindAdept,
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
        self.max_hp = config::unit_stats(self.kind).map_or(self.max_hp, |stats| {
            stats.hp.saturating_add(self.vitality_health_bonus())
        });
        self.hp = self.max_hp;
        true
    }

    pub(crate) fn dao_damage_after_armor(&self, damage: u32, penetration: f32) -> u32 {
        if self.dao() != Some(UpgradeKind::DaoOfHeaviness) {
            return damage;
        }
        crate::rules::combat::damage_after_armor_reduction(
            damage,
            config::DAO_WAR_ARMOR_REDUCTION,
            penetration,
        )
    }

    pub(in crate::game) fn try_start_novice_charge(
        &mut self,
        target_position: (f32, f32),
        target_movement: (f32, f32),
        hostile_unit: bool,
    ) {
        if !matches!(self.kind, EntityKind::Warrior | EntityKind::WarAdept)
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
        let max_distance = (config::NOVICE_CHARGE_MAX_DISTANCE_TILES * config::TILE_SIZE) as f32;
        let distance_squared = dx * dx + dy * dy;
        if distance_squared > max_distance * max_distance
            || (distance_squared < min_distance * min_distance && dx * mx + dy * my <= 0.0)
        {
            return;
        }
        if let Some(movement) = self.movement.as_mut() {
            movement.novice_charge_ticks = if self.kind == EntityKind::WarAdept {
                config::WAR_CHARGE_DURATION_TICKS
            } else {
                config::NOVICE_CHARGE_DURATION_TICKS
            };
            self.start_ability_cooldown(AbilityKind::Charge, config::NOVICE_CHARGE_COOLDOWN_TICKS);
        }
    }

    pub(in crate::game) fn end_war_charge(&mut self) {
        if self.kind == EntityKind::WarAdept {
            if let Some(movement) = self.movement.as_mut() {
                movement.novice_charge_ticks = 0;
            }
        }
    }

    pub(crate) fn movement_speed(&self) -> f32 {
        let base = config::unit_stats(self.kind).map_or(0.0, |s| s.speed);
        let master = if self.dao_master {
            config::DAO_MASTER_SPEED_MULTIPLIER
        } else {
            1.0
        };
        let charge = if matches!(self.kind, EntityKind::Warrior | EntityKind::WarAdept)
            && self
                .movement
                .as_ref()
                .is_some_and(|m| m.novice_charge_ticks > 0)
        {
            if self.kind == EntityKind::WarAdept {
                config::WAR_CHARGE_SPEED_MULTIPLIER
            } else {
                config::NOVICE_CHARGE_SPEED_MULTIPLIER
            }
        } else {
            1.0
        };
        let haste = if self.shop_item_active(AbilityKind::Haste) {
            config::HASTE_SPEED_MULTIPLIER
        } else {
            1.0
        };
        base * master * charge * haste
    }
}
