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
    let active_combat_target = matches!(entity.order(), Order::Attack(_) | Order::AttackMove(_))
        || (entity.is_building() && entity.can_attack());
    let target_visible = if let Some(target_id) = entity.target_id() {
        target
            .filter(|target| target.id == target_id)
            .map(|target| {
                entity.owner == viewer
                    || !fogged
                    || fog.is_visible_world(viewer, target.pos_x, target.pos_y)
            })
            .unwrap_or(false)
    } else {
        false
    };
    let weapon_facing_useful =
        entity.kind == crate::game::entity::EntityKind::Tank || active_combat_target;
    if weapon_facing_useful {
        if let Some(weapon_facing) = entity.weapon_facing() {
            let weapon_facing_is_safe = entity.owner == viewer
                || !fogged
                || entity.target_id().is_none()
                || !active_combat_target
                || target_visible;
            if weapon_facing_is_safe {
                view.weapon_facing = Some(weapon_facing);
            }
        }
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
        if entity.owner == viewer {
            view.prod_queue = Some(entity.prod_queue().len() as u32);
        }
    }

    // Rally point is a private planning aid: only ever revealed to the owner.
    if entity.owner == viewer {
        if let Some((rx, ry)) = entity.rally_point() {
            view.rally = Some([rx, ry]);
        }
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
        if active_combat_target {
            if let Some(target) = target {
                if target.id == target_id && target_visible {
                    view.target_id = Some(target_id);
                }
            }
        }
    }

    Some(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, Order};

    #[test]
    fn weapon_facing_is_omitted_when_target_direction_is_hidden() {
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("viewer spotter should spawn");
        let tank_id = entities
            .spawn_unit(2, EntityKind::Tank, 120.0, 100.0)
            .expect("tank should spawn");
        let hidden_target_id = entities
            .spawn_unit(3, EntityKind::Rifleman, 700.0, 700.0)
            .expect("hidden target should spawn");
        {
            let tank = entities.get_mut(tank_id).expect("tank should exist");
            tank.set_order(Order::attack(hidden_target_id));
            tank.set_target_id(Some(hidden_target_id));
            tank.set_weapon_facing(1.2);
        }
        let mut fog = Fog::new(64);
        fog.recompute(&[1, 2, 3], &entities);
        let tank = entities.get(tank_id).expect("tank should exist");
        let hidden_target = entities
            .get(hidden_target_id)
            .expect("hidden target should exist");

        let enemy_view = project_entity(1, tank, &fog, true, Some(hidden_target))
            .expect("viewer should see nearby tank");
        assert_eq!(enemy_view.target_id, None);
        assert_eq!(enemy_view.weapon_facing, None);

        let owner_view = project_entity(2, tank, &fog, true, Some(hidden_target))
            .expect("owner should see own tank");
        assert_eq!(owner_view.target_id, Some(hidden_target_id));
        assert_eq!(owner_view.weapon_facing, Some(1.2));
    }
}
