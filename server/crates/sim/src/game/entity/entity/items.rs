use super::{Entity, EntityKind, Order};

impl Entity {
    pub(in crate::game) fn shop_item_reload(&self, ticks: u32) -> u32 {
        if self.shop_item_active(crate::game::ability::AbilityKind::Haste) {
            (ticks / 2).max(1)
        } else {
            ticks
        }
    }

    pub(in crate::game) fn shop_item_active(
        &self,
        ability: crate::game::ability::AbilityKind,
    ) -> bool {
        self.ability_cooldowns.get(&ability).copied().unwrap_or(0) > 0
    }
    pub(in crate::game) fn shop_item_category_occupied(
        &self,
        ability: crate::game::ability::AbilityKind,
    ) -> bool {
        use crate::game::ability::AbilityKind::*;
        let kinds: &[crate::game::ability::AbilityKind] =
            if matches!(ability, ArmorTreasure | VitalityTreasure) {
                &[ArmorTreasure, VitalityTreasure]
            } else {
                &[Haste, DoubleDamage]
            };
        kinds.iter().any(|kind| {
            self.ability_uses_remaining.get(kind).copied().unwrap_or(0) > 0
                || self.shop_item_active(*kind)
        })
    }
    pub(in crate::game) fn receive_shop_item(
        &mut self,
        ability: crate::game::ability::AbilityKind,
    ) {
        if !self.shop_item_category_occupied(ability) {
            self.ability_uses_remaining.insert(ability, 1);
        }
    }
    pub(in crate::game) fn activate_shop_item(
        &mut self,
        ability: crate::game::ability::AbilityKind,
    ) {
        if self
            .ability_uses_remaining
            .get(&ability)
            .copied()
            .unwrap_or(0)
            == 1
        {
            if ability == crate::game::ability::AbilityKind::VitalityTreasure {
                let Some(max_hp) = self.max_hp.checked_add(150) else {
                    return;
                };
                self.max_hp = max_hp;
                self.hp = self.hp.saturating_add(150).min(max_hp);
            }
            self.ability_uses_remaining.remove(&ability);
            self.ability_cooldowns.insert(ability, 900);
        }
    }

    pub(in crate::game) fn vitality_health_bonus(&self) -> u32 {
        if self.shop_item_active(crate::game::ability::AbilityKind::VitalityTreasure) {
            150
        } else {
            0
        }
    }

    pub(in crate::game) fn expire_shop_vitality(&mut self) {
        if self
            .ability_cooldowns
            .get(&crate::game::ability::AbilityKind::VitalityTreasure)
            == Some(&1)
        {
            self.max_hp = self.max_hp.saturating_sub(150).max(1);
            self.hp = self.hp.min(self.max_hp);
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
