use super::{Entity, EntityKind, Order};

impl Entity {
    /// Public visual strength only; inventory identities and timers stay private.
    pub(crate) fn item_radiance_tier(&self) -> u8 {
        if !self.kind.is_disciple() {
            return 0;
        }
        self.carried_item_count()
            .min(crate::config::CULTIVATOR_MASTER_ITEM_CAPACITY)
    }

    pub(in crate::game) fn carried_item_capacity(&self) -> Option<u8> {
        self.kind.is_disciple().then_some(if self.dao_master {
            crate::config::CULTIVATOR_MASTER_ITEM_CAPACITY
        } else if self.kind == EntityKind::Warrior {
            crate::config::CULTIVATOR_NOVICE_ITEM_CAPACITY
        } else {
            crate::config::CULTIVATOR_ITEM_CAPACITY
        })
    }

    pub(in crate::game) fn carried_item_count(&self) -> u8 {
        use crate::game::ability::AbilityKind::{ArmorTreasure, Haste};
        u8::from(self.escape_treasure.is_some())
            + [
                crate::game::ability::AbilityKind::Regen,
                ArmorTreasure,
                Haste,
                crate::game::ability::AbilityKind::BreakthroughTreasure,
                crate::game::ability::AbilityKind::VitalityTreasure,
                crate::game::ability::AbilityKind::DoubleDamage,
            ]
            .iter()
            .filter(|&&kind| self.shop_item_active(kind))
            .count() as u8
    }

    pub(in crate::game) fn shop_item_reload(&self, ticks: u32) -> u32 {
        if self.shop_item_active(crate::game::ability::AbilityKind::Haste) {
            // +25% attack rate means 80% reload time, rounded up to a whole tick.
            ((u64::from(ticks) * 4).div_ceil(5) as u32).max(1)
        } else {
            ticks
        }
    }

    pub(in crate::game) fn shop_item_active(
        &self,
        ability: crate::game::ability::AbilityKind,
    ) -> bool {
        self.ability_uses_remaining
            .get(&ability)
            .copied()
            .unwrap_or(0)
            > 0
    }

    pub(in crate::game) fn receive_shop_item(
        &mut self,
        ability: crate::game::ability::AbilityKind,
    ) {
        if self.shop_item_active(ability) {
            return;
        }
        if ability == crate::game::ability::AbilityKind::VitalityTreasure {
            let Some(max_hp) = self.max_hp.checked_add(150) else {
                return;
            };
            self.max_hp = max_hp;
            self.hp = self.hp.saturating_add(150).min(max_hp);
        }
        self.ability_uses_remaining.insert(ability, 1);
    }
    pub(in crate::game) fn vitality_health_bonus(&self) -> u32 {
        if self.shop_item_active(crate::game::ability::AbilityKind::VitalityTreasure) {
            150
        } else {
            0
        }
    }

    /// Healing fields store capacity in the ordinary health pair, immune to combat and repair.
    pub(in crate::game) fn spend_seal_healing(&mut self, amount: u32) {
        if self.kind == EntityKind::HealingSeal {
            self.hp = self.hp.saturating_sub(amount);
        }
    }

    /// Consume a delivered escape charge and reset all movement at the validated Shop exit.
    pub(in crate::game) fn rescue_shop_escape(&mut self, x: f32, y: f32, hp: u32) {
        self.escape_treasure = None;
        self.escape_autocast_disabled = false;
        self.hp = hp.max(1);
        self.clear_orders();
        self.replace_active_order(Order::Idle);
        self.reset_stuck(x, y);
        self.set_position(x, y);
    }
}
