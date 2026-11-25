use crate::game::entity::Entity;

use super::CheckpointPayloadError;

pub(super) fn validate(entity: &Entity) -> Result<(), CheckpointPayloadError> {
    use crate::game::ability::AbilityKind::{ArmorTreasure, DoubleDamage, Haste, VitalityTreasure};
    for ability in [Haste, DoubleDamage, ArmorTreasure, VitalityTreasure] {
        let ticks = entity.ability_cooldowns.get(&ability).copied().unwrap_or(0);
        if ticks > 900
            || (ticks > 0
                && (!entity.is_unit()
                    || (ability == Haste && entity.shop_item_active(DoubleDamage))
                    || (ability == VitalityTreasure
                        && (entity.max_hp <= 150 || entity.shop_item_active(ArmorTreasure)))))
        {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "entities.abilityCooldowns",
            });
        }
    }
    for (&ability, &remaining) in &entity.ability_uses_remaining {
        if matches!(
            ability,
            Haste | DoubleDamage | ArmorTreasure | VitalityTreasure
        ) {
            let conflicting = if matches!(ability, ArmorTreasure | VitalityTreasure) {
                &[ArmorTreasure, VitalityTreasure][..]
            } else {
                &[Haste, DoubleDamage][..]
            };
            if !entity.is_unit()
                || remaining != 1
                || conflicting.iter().any(|kind| {
                    entity.shop_item_active(*kind)
                        || (*kind != ability
                            && entity
                                .ability_uses_remaining
                                .get(kind)
                                .copied()
                                .unwrap_or(0)
                                > 0)
                })
            {
                return Err(CheckpointPayloadError::InvalidValue {
                    field: "entities.abilityUsesRemaining",
                });
            }
            continue;
        }
        let definition = crate::game::ability::definition(ability);
        if !definition.carriers.contains(&entity.kind)
            || entity
                .ability_max_charges(ability)
                .is_none_or(|max_charges| remaining > max_charges)
        {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "entities.abilityUsesRemaining",
            });
        }
    }
    for (&ability, &ticks) in &entity.ability_charge_recharge_ticks {
        let definition = crate::game::ability::definition(ability);
        let remaining = entity.ability_uses_remaining(ability);
        if !definition.carriers.contains(&entity.kind)
            || ticks == 0
            || definition
                .charge_recharge_ticks
                .is_none_or(|recharge_ticks| ticks > recharge_ticks.saturating_add(1))
            || entity
                .ability_max_charges(ability)
                .zip(remaining)
                .is_none_or(|(max_charges, remaining)| remaining >= max_charges)
        {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "entities.abilityChargeRechargeTicks",
            });
        }
    }
    Ok(())
}
