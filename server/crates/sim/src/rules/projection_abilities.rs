use crate::game::ability::{self, AbilityKind};
use crate::game::ability_runtime::AbilityObjectPayload;
use crate::game::entity::Entity;

use super::projection::EntityProjectionContext;

pub(super) fn active_return_object_id(
    context: &EntityProjectionContext<'_>,
    entity: &Entity,
    ability: AbilityKind,
) -> Option<u32> {
    if ability == ability::AbilityKind::EkatMagicAnchor {
        return context
            .ability_runtime?
            .active_anchor(entity.owner, entity.id, ability, context.tick)
            .map(|object| object.id.get());
    }
    context
        .ability_runtime?
        .active_return_marker(entity.owner, entity.id, ability, None, context.tick)
        .map(|object| object.id.get())
}

pub(super) fn return_available_tick(
    context: &EntityProjectionContext<'_>,
    entity: &Entity,
    ability: AbilityKind,
) -> Option<u32> {
    match context
        .ability_runtime?
        .active_return_marker(entity.owner, entity.id, ability, None, context.tick)?
        .payload
    {
        AbilityObjectPayload::DashReturn {
            earliest_return_tick,
        } => Some(earliest_return_tick),
        _ => None,
    }
}

pub(super) fn active_ability_object_expires_in(
    context: &EntityProjectionContext<'_>,
    entity: &Entity,
    ability: AbilityKind,
) -> Option<u16> {
    if ability::definition(ability).effect_hook == ability::AbilityEffectHook::OwnedAreaStatus {
        return (entity.breakthrough_aura_ticks() > 0).then_some(entity.breakthrough_aura_ticks());
    }
    if ability == ability::AbilityKind::EkatMagicAnchor {
        return context
            .ability_runtime?
            .active_anchor(entity.owner, entity.id, ability, context.tick)
            .and_then(|object| object.expires_in(context.tick));
    }
    context
        .ability_runtime?
        .active_return_marker(entity.owner, entity.id, ability, None, context.tick)
        .and_then(|object| object.expires_in(context.tick))
}

/// Timer-only team affordance; the projection caller owns ability-specific admission.
pub(super) fn team_cooldown(
    entity: &Entity,
    kind: AbilityKind,
) -> crate::protocol::AbilityCooldownView {
    crate::protocol::AbilityCooldownView {
        ability: kind.to_protocol_str().to_string(),
        cooldown_left: entity.ability_cooldown_ticks(kind),
        remaining_uses: None,
        autocast_enabled: None,
        active_object_id: None,
        available_tick: None,
        lockout_until_tick: None,
        expires_in: None,
        charge_recharge_left: None,
    }
}
