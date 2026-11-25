use crate::game::ability_runtime::{AbilityObjectPayload, AbilityWorldObjectKind};
use crate::game::fog::Fog;
use crate::game::Game;

pub(in crate::game) fn ability_object_views_for(
    game: &Game,
    player: u32,
    fog: &Fog,
    fogged: bool,
    owner_visible_players: &[u32],
    omniscient: bool,
) -> Vec<crate::protocol::AbilityObjectView> {
    game.state
        .ability_runtime
        .world_objects()
        .filter(|object| {
            !fogged
                || fog.is_visible_world(player, object.x.to_world(), object.y.to_world())
                || (crate::game::services::shop_items::is_shop_item(object.ability)
                    && (object.owner == player || owner_visible_players.contains(&object.owner)))
        })
        .map(|object| {
            let owner_visible = object.owner == player
                || owner_visible_players.contains(&object.owner)
                || omniscient;
            let source_caster_id = if owner_visible
                || game
                    .state
                    .entities
                    .get(object.caster_id)
                    .is_some_and(|caster| {
                        !fogged || fog.is_visible_world(player, caster.pos_x(), caster.pos_y())
                    }) {
                Some(object.caster_id)
            } else {
                None
            };
            crate::protocol::AbilityObjectView {
                vector: match object.payload {
                    AbilityObjectPayload::VectorField { direction } => Some(direction),
                    _ => None,
                },
                id: object.id.get(),
                owner: object.owner,
                ability: object.ability.to_protocol_str().to_string(),
                kind: ability_object_kind_to_protocol(object.kind).to_string(),
                x: object.x.to_world(),
                y: object.y.to_world(),
                expires_in: object.expires_in(game.state.tick),
                source_caster_id,
                owner_state: owner_visible
                    .then(|| ability_object_owner_state_to_protocol(object.payload)),
            }
        })
        .chain(game.state.entities.iter().filter_map(|caster| {
            let shield = caster.shield.as_ref()?;
            if caster.hp == 0
                || shield.remaining == 0
                || (fogged
                    && !fog.is_visible_world(player, shield.x.to_world(), shield.y.to_world()))
            {
                return None;
            }
            let owner_visible = caster.owner == player
                || owner_visible_players.contains(&caster.owner)
                || omniscient;
            Some(crate::protocol::AbilityObjectView {
                vector: None,
                id: shield.id,
                owner: caster.owner,
                ability: crate::game::ability::AbilityKind::DiscipleShield
                    .to_protocol_str()
                    .into(),
                kind: crate::protocol::ability_object_kinds::SHIELD.into(),
                x: shield.x.to_world(),
                y: shield.y.to_world(),
                expires_in: Some(
                    shield
                        .expires_tick
                        .saturating_sub(game.state.tick)
                        .min(u16::MAX as u32) as u16,
                ),
                source_caster_id: (owner_visible
                    || !fogged
                    || fog.is_visible_world(player, caster.pos_x(), caster.pos_y()))
                .then_some(caster.id),
                owner_state: Some(crate::protocol::AbilityObjectOwnerStateView {
                    hp: Some(shield.remaining.min(shield.capacity) as u16),
                    max_hp: Some(shield.capacity as u16),
                    duration_ticks: Some(shield.duration_ticks),
                    radius: Some(
                        crate::config::DISCIPLE_SHIELD_RADIUS_TILES
                            * crate::config::TILE_SIZE as f32,
                    ),
                    ..Default::default()
                }),
            })
        }))
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
        AbilityObjectPayload::None | AbilityObjectPayload::VectorField { .. } => Default::default(),
        AbilityObjectPayload::DashReturn {
            earliest_return_tick,
        } => crate::protocol::AbilityObjectOwnerStateView {
            earliest_return_tick: Some(earliest_return_tick),
            ..Default::default()
        },
        AbilityObjectPayload::MagicAnchor { radius } => {
            crate::protocol::AbilityObjectOwnerStateView {
                radius: Some(radius),
                ..Default::default()
            }
        }
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
