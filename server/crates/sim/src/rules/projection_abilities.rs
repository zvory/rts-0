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
