use crate::game::entity::Entity;

use super::CheckpointPayloadError;

pub(super) fn validate(entity: &Entity) -> Result<(), CheckpointPayloadError> {
    for (&ability, &remaining) in &entity.ability_uses_remaining {
        let definition = crate::game::ability::definition(ability);
        if !definition.carriers.contains(&entity.kind)
            || definition
                .charges
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
            || definition
                .charges
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
