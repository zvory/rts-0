use crate::game::ability_runtime::{AbilityObjectPayload, AbilityWorldObjectKind};
use crate::game::fog::Fog;
use crate::game::Game;

pub(in crate::game) fn ability_object_views_for(
    game: &Game,
    player: u32,
    fog: &Fog,
    fogged: bool,
    include_player_resources: bool,
) -> Vec<crate::protocol::AbilityObjectView> {
    game.state.ability_runtime
        .world_objects()
        .filter(|object| !fogged || fog.is_visible_world(player, object.x, object.y))
        .map(|object| {
            let owner_visible = object.owner == player || include_player_resources || !fogged;
            let source_caster_id = if owner_visible
                || game.state.entities.get(object.caster_id).is_some_and(|caster| {
                    !fogged || fog.is_visible_world(player, caster.pos_x, caster.pos_y)
                }) {
                Some(object.caster_id)
            } else {
                None
            };
            crate::protocol::AbilityObjectView {
                id: object.id.get(),
                owner: object.owner,
                ability: object.ability.to_protocol_str().to_string(),
                kind: ability_object_kind_to_protocol(object.kind).to_string(),
                x: object.x,
                y: object.y,
                expires_in: object.expires_in(game.state.tick),
                source_caster_id,
                owner_state: owner_visible
                    .then(|| ability_object_owner_state_to_protocol(object.payload)),
            }
        })
        .collect()
}

fn ability_object_kind_to_protocol(kind: AbilityWorldObjectKind) -> &'static str {
    match kind {
        AbilityWorldObjectKind::ReturnMarker => {
            crate::protocol::ability_object_kinds::RETURN_MARKER
        }
        AbilityWorldObjectKind::MagicAnchor => crate::protocol::ability_object_kinds::MAGIC_ANCHOR,
        AbilityWorldObjectKind::LineProjectile => {
            crate::protocol::ability_object_kinds::LINE_PROJECTILE
        }
    }
}

fn ability_object_owner_state_to_protocol(
    payload: AbilityObjectPayload,
) -> crate::protocol::AbilityObjectOwnerStateView {
    match payload {
        AbilityObjectPayload::None => Default::default(),
        AbilityObjectPayload::DashReturn {
            earliest_return_tick,
        } => crate::protocol::AbilityObjectOwnerStateView {
            earliest_return_tick: Some(earliest_return_tick),
            ..Default::default()
        },
        AbilityObjectPayload::MagicAnchor { radius } => crate::protocol::AbilityObjectOwnerStateView {
            radius: Some(radius),
            ..Default::default()
        },
        AbilityObjectPayload::LineProjectile {
            distance_traveled,
            ticks_out,
        } => crate::protocol::AbilityObjectOwnerStateView {
            distance_traveled: Some(distance_traveled),
            ticks_out: Some(ticks_out),
            ..Default::default()
        },
    }
}
