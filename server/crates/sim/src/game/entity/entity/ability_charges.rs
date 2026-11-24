use super::*;

impl Entity {
    pub(crate) fn ability_max_charges(&self, ability: AbilityKind) -> Option<u16> {
        if ability == AbilityKind::WindDash && self.dao_master {
            Some(2)
        } else {
            crate::game::ability::definition(ability).charges
        }
    }

    pub fn ability_uses_remaining(&self, ability: AbilityKind) -> Option<u16> {
        let max_charges = self.ability_max_charges(ability)?;
        Some(
            self.ability_uses_remaining
                .get(&ability)
                .copied()
                .unwrap_or(max_charges),
        )
    }

    pub(crate) fn has_initial_free_barrage(&self) -> bool {
        self.ability_uses_remaining
            .get(&AbilityKind::Barrage)
            .copied()
            .unwrap_or(1)
            > 0
    }

    pub(crate) fn consume_initial_free_barrage(&mut self) -> bool {
        if !self.has_initial_free_barrage() {
            return false;
        }
        self.ability_uses_remaining.insert(AbilityKind::Barrage, 0);
        true
    }

    pub fn consume_ability_use(&mut self, ability: AbilityKind) -> bool {
        let definition = crate::game::ability::definition(ability);
        let max_charges = self.ability_max_charges(ability);
        match self.ability_uses_remaining(ability) {
            Some(0) => false,
            Some(_) => {
                let uses = self
                    .ability_uses_remaining
                    .entry(ability)
                    .or_insert(max_charges.unwrap_or(0));
                *uses = uses.saturating_sub(1);
                if let Some(recharge_ticks) = definition.charge_recharge_ticks {
                    self.ability_charge_recharge_ticks
                        .entry(ability)
                        .or_insert_with(|| recharge_ticks.saturating_add(1));
                }
                true
            }
            None => true,
        }
    }

    pub(crate) fn tick_ability_charge_recharges(&mut self) {
        for (&ability, &remaining) in &self.ability_uses_remaining {
            let definition = crate::game::ability::definition(ability);
            let (Some(max_charges), Some(recharge_ticks)) = (
                self.ability_max_charges(ability),
                definition.charge_recharge_ticks,
            ) else {
                continue;
            };
            if remaining < max_charges {
                self.ability_charge_recharge_ticks
                    .entry(ability)
                    .or_insert_with(|| recharge_ticks.saturating_add(1));
            }
        }
        let dao_master = self.dao_master;
        let uses = &mut self.ability_uses_remaining;
        self.ability_charge_recharge_ticks.retain(|ability, ticks| {
            *ticks = ticks.saturating_sub(1);
            if *ticks > 0 {
                return true;
            }
            let definition = crate::game::ability::definition(*ability);
            let (Some(max_charges), Some(recharge_ticks), Some(remaining)) = (
                if *ability == AbilityKind::WindDash && dao_master {
                    Some(2)
                } else {
                    definition.charges
                },
                definition.charge_recharge_ticks,
                uses.get_mut(ability),
            ) else {
                return false;
            };
            *remaining = remaining.saturating_add(1).min(max_charges);
            if *remaining < max_charges {
                *ticks = recharge_ticks;
                true
            } else {
                false
            }
        });
    }

    pub(crate) fn ability_charge_recharge_ticks(&self, ability: AbilityKind) -> Option<u16> {
        self.ability_charge_recharge_ticks.get(&ability).copied()
    }
}
