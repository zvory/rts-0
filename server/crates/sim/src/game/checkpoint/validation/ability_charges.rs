use crate::game::entity::Entity;

use super::CheckpointPayloadError;

pub(super) fn validate(entity: &Entity) -> Result<(), CheckpointPayloadError> {
    use crate::game::ability::AbilityKind::{
        ArmorTreasure, DoubleDamage, DoubleXp, Haste, Regen, VitalityTreasure,
    };
    for ability in [
        Regen,
        Haste,
        DoubleXp,
        DoubleDamage,
        ArmorTreasure,
        VitalityTreasure,
    ] {
        let ticks = entity.ability_cooldowns.get(&ability).copied().unwrap_or(0);
        if ticks > 0 {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "entities.abilityCooldowns",
            });
        }
    }
    for (&ability, &remaining) in &entity.ability_uses_remaining {
        if matches!(
            ability,
            Regen | Haste | DoubleXp | DoubleDamage | ArmorTreasure | VitalityTreasure
        ) {
            if !entity.is_unit()
                || remaining != 1
                || (ability == VitalityTreasure && entity.max_hp <= 150)
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
