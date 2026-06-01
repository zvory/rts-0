//! Projection rules for fog-gated entity views and event delivery.
//!
//! This module owns what a player is allowed to see. It does not mutate the world; future
//! last-known-position or partial-reveal rules should grow here.

use crate::game::entity::{Entity, GatherPhase, Order};
use crate::game::fog::Fog;
use crate::protocol::EntityView;

pub fn entity_visible_to(viewer: u32, entity: &Entity, fog: &Fog) -> bool {
    entity.owner == viewer
        || entity.is_node()
        || fog.is_visible_world(viewer, entity.pos_x, entity.pos_y)
}

pub fn event_visible_to(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    attacker_owner: u32,
    fog: &Fog,
) -> bool {
    viewer == attacker_owner || fog.is_visible_world(viewer, event_origin_x, event_origin_y)
}

pub fn attack_event_visible_to(
    viewer: u32,
    attacker_x: f32,
    attacker_y: f32,
    target_x: f32,
    target_y: f32,
    attacker_owner: u32,
    fog: &Fog,
) -> bool {
    event_visible_to(viewer, attacker_x, attacker_y, attacker_owner, fog)
        || fog.is_visible_world(viewer, target_x, target_y)
}

pub fn project_entity(
    viewer: u32,
    entity: &Entity,
    fog: &Fog,
    fogged: bool,
    target: Option<&Entity>,
) -> Option<EntityView> {
    if fogged && !entity_visible_to(viewer, entity, fog) {
        return None;
    }

    let mut view = EntityView::new(
        entity.id,
        entity.owner,
        entity.kind.to_protocol_str(),
        entity.pos_x,
        entity.pos_y,
        entity.hp,
        entity.max_hp,
        entity.state_str(),
    );

    if entity.is_unit() {
        view.facing = Some(entity.facing());
    }
    if entity.kind == crate::game::entity::EntityKind::MachineGunner {
        view.setup_state = Some(entity.weapon_setup().to_protocol_str().to_string());
    }

    if entity.is_building() && !entity.prod_queue().is_empty() {
        if let Some(front) = entity.prod_queue().first() {
            view.prod_kind = Some(front.unit.to_protocol_str().to_string());
            view.prod_progress = Some(if front.total == 0 {
                0.0
            } else {
                front.progress as f32 / front.total as f32
            });
        }
        view.prod_queue = Some(entity.prod_queue().len() as u32);
    }

    if let Some(progress) = entity.build_progress_fraction() {
        view.build_progress = Some(progress);
    }

    // Current behavior exposes static resource amount even through fog.
    if entity.is_node() {
        view.remaining = entity.remaining();
    }

    if entity.kind == crate::game::entity::EntityKind::Worker
        && entity.gather_phase() == Some(GatherPhase::Harvesting)
    {
        if let Some(node) = entity.order().gather_node() {
            view.latched_node = Some(node);
        }
    }

    if let Some(target_id) = entity.target_id() {
        let active_combat_target =
            matches!(entity.order(), Order::Attack(_) | Order::AttackMove(_))
                || (entity.is_building() && entity.can_attack());
        if active_combat_target {
            if let Some(target) = target {
                let target_visible = entity.owner == viewer
                    || !fogged
                    || fog.is_visible_world(viewer, target.pos_x, target.pos_y);
                if target.id == target_id && target_visible {
                    view.target_id = Some(target_id);
                }
            }
        }
    }

    Some(view)
}
