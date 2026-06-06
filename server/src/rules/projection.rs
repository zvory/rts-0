//! Projection rules for fog-gated entity views and event delivery.
//!
//! This module owns what a player is allowed to see. It does not mutate the world; future
//! last-known-position or partial-reveal rules should grow here.

use crate::game::entity::{
    fires_while_moving, Entity, EntityKind, GatherPhase, Order, OrderIntent,
};
use crate::game::fog::Fog;
#[cfg(debug_assertions)]
use crate::protocol::{DebugPathPoint, DebugPathView};
use crate::protocol::{EntityView, QueuedOrderMarker};

#[cfg(debug_assertions)]
const MAX_DEBUG_PATH_WAYPOINTS: usize = 128;

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
    actionable_fog: Option<&Fog>,
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
    let actionable_fog = actionable_fog.unwrap_or(fog);
    let vision_only = fogged
        && entity.owner != viewer
        && !entity.is_node()
        && !entity_visible_to(viewer, entity, actionable_fog);
    view.vision_only = vision_only;

    if entity.is_unit() {
        view.facing = Some(entity.facing());
    }
    if let Some(oil_used) = entity.lifetime_oil_used() {
        view.oil_used = Some(oil_used);
    }
    let active_combat_target = matches!(entity.order(), Order::Attack(_) | Order::AttackMove(_))
        || (fires_while_moving(entity.kind) && entity.target_id().is_some())
        || (entity.is_building() && entity.can_attack());
    let target_visible = if let Some(target_id) = entity.target_id() {
        target
            .filter(|target| target.id == target_id)
            .map(|target| {
                entity.owner == viewer
                    || !fogged
                    || (!vision_only
                        && actionable_fog.is_visible_world(viewer, target.pos_x, target.pos_y))
            })
            .unwrap_or(false)
    } else {
        false
    };
    let weapon_facing_useful = fires_while_moving(entity.kind) || active_combat_target;
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
    if matches!(entity.kind, EntityKind::MachineGunner | EntityKind::AtTeam) {
        view.setup_state = Some(entity.weapon_setup().to_protocol_str().to_string());
    }
    if entity.kind == EntityKind::AtTeam && entity.owner == viewer {
        view.setup_facing = entity.emplacement_facing();
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
        view.active_marker = active_order_marker(entity);
        view.queued_markers = queued_order_markers(entity);
        #[cfg(debug_assertions)]
        {
            view.debug_path = debug_path_view(entity);
        }
        if entity.kind == EntityKind::Rifleman {
            let charge_cooldown_left = entity.charge_cooldown_ticks();
            if charge_cooldown_left > 0 {
                view.charge_cooldown_left = Some(charge_cooldown_left);
            }
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

fn active_order_marker(entity: &Entity) -> Option<QueuedOrderMarker> {
    let attack_move = match entity.order() {
        Order::Move(_) => false,
        Order::AttackMove(_) => true,
        Order::Idle | Order::Attack(_) | Order::Gather(_) | Order::Build(_) => return None,
    };
    let (x, y) = entity.path_goal().or_else(|| entity.move_intent())?;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(QueuedOrderMarker { x, y, attack_move })
}

fn queued_order_markers(entity: &Entity) -> Vec<QueuedOrderMarker> {
    entity
        .queued_orders()
        .iter()
        .filter_map(|intent| match intent {
            OrderIntent::Move(point) if point.x.is_finite() && point.y.is_finite() => {
                Some(QueuedOrderMarker {
                    x: point.x,
                    y: point.y,
                    attack_move: false,
                })
            }
            OrderIntent::AttackMove(point) if point.x.is_finite() && point.y.is_finite() => {
                Some(QueuedOrderMarker {
                    x: point.x,
                    y: point.y,
                    attack_move: true,
                })
            }
            OrderIntent::Move(_)
            | OrderIntent::AttackMove(_)
            | OrderIntent::Attack(_)
            | OrderIntent::Gather(_)
            | OrderIntent::Build(_) => None,
        })
        .collect()
}

#[cfg(debug_assertions)]
fn debug_path_view(entity: &Entity) -> Option<DebugPathView> {
    let movement = entity.movement.as_ref()?;
    if movement.path.is_empty() {
        return None;
    }

    let waypoints = movement
        .path
        .iter()
        .rev()
        .take(MAX_DEBUG_PATH_WAYPOINTS)
        .filter_map(|&(x, y)| debug_path_point(x, y))
        .collect::<Vec<_>>();
    if waypoints.is_empty() {
        return None;
    }

    let goal = movement.path_goal.and_then(|(x, y)| debug_path_point(x, y));
    Some(DebugPathView {
        waypoints,
        goal,
        last_repath_tick: movement.last_repath_tick,
        stuck_ticks: movement.stuck_ticks,
        static_blocked_ticks: movement.static_blocked_ticks,
        total_waypoints: movement.path.len().min(u16::MAX as usize) as u16,
    })
}

#[cfg(debug_assertions)]
fn debug_path_point(x: f32, y: f32) -> Option<DebugPathPoint> {
    (x.is_finite() && y.is_finite()).then_some(DebugPathPoint { x, y })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, Order, OrderIntent};
    use crate::game::map::Map;
    use crate::protocol::terrain;

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
        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1)],
            expansion_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], &entities, &map);
        let tank = entities.get(tank_id).expect("tank should exist");
        let hidden_target = entities
            .get(hidden_target_id)
            .expect("hidden target should exist");

        let enemy_view = project_entity(1, tank, &fog, Some(&fog), true, Some(hidden_target))
            .expect("viewer should see nearby tank");
        assert_eq!(enemy_view.target_id, None);
        assert_eq!(enemy_view.weapon_facing, None);

        let owner_view = project_entity(2, tank, &fog, Some(&fog), true, Some(hidden_target))
            .expect("owner should see own tank");
        assert_eq!(owner_view.target_id, Some(hidden_target_id));
        assert_eq!(owner_view.weapon_facing, Some(1.2));
    }

    #[test]
    fn moving_tank_projects_visible_turret_target() {
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("viewer spotter should spawn");
        let tank_id = entities
            .spawn_unit(2, EntityKind::Tank, 120.0, 100.0)
            .expect("tank should spawn");
        let target_id = entities
            .spawn_unit(3, EntityKind::Rifleman, 140.0, 100.0)
            .expect("target should spawn");
        {
            let tank = entities.get_mut(tank_id).expect("tank should exist");
            tank.set_order(Order::move_to(300.0, 100.0));
            tank.set_target_id(Some(target_id));
            tank.set_weapon_facing(0.0);
        }
        let map = Map {
            size: 16,
            terrain: vec![terrain::GRASS; 16 * 16],
            starts: vec![(1, 1)],
            expansion_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], &entities, &map);
        let tank = entities.get(tank_id).expect("tank should exist");
        let target = entities.get(target_id).expect("target should exist");

        let viewer_view = project_entity(1, tank, &fog, Some(&fog), true, Some(target))
            .expect("viewer should see tank");

        assert_eq!(viewer_view.target_id, Some(target_id));
        assert_eq!(viewer_view.weapon_facing, Some(0.0));
    }

    #[test]
    fn tank_projects_lifetime_oil_used() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 120.0, 100.0)
            .expect("tank should spawn");
        {
            let tank = entities.get_mut(tank_id).expect("tank should exist");
            if let Some(movement) = tank.movement.as_mut() {
                movement.lifetime_oil_used = 3.25;
            }
        }
        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1)],
            expansion_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let tank = entities.get(tank_id).expect("tank should exist");

        let view =
            project_entity(1, tank, &fog, Some(&fog), true, None).expect("tank should be visible");
        assert_eq!(view.oil_used, Some(3.25));
    }

    #[test]
    fn queued_markers_are_owner_only_points() {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let hidden_enemy = entities
            .spawn_unit(2, EntityKind::Rifleman, 700.0, 700.0)
            .expect("enemy should spawn");
        let hidden_node = entities
            .spawn_node(EntityKind::Steel, 720.0, 720.0)
            .expect("node should spawn");
        {
            let unit = entities.get_mut(unit_id).expect("unit should exist");
            unit.set_order(Order::attack_move_to(120.0, 130.0));
            unit.append_queued_order(OrderIntent::move_to(140.0, 160.0));
            unit.append_queued_order(OrderIntent::attack(hidden_enemy));
            unit.append_queued_order(OrderIntent::gather(hidden_node));
            unit.append_queued_order(OrderIntent::attack_move_to(180.0, 200.0));
        }

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            expansion_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let unit = entities.get(unit_id).expect("unit should exist");

        let owner_view = project_entity(1, unit, &fog, Some(&fog), true, None)
            .expect("owner should see own unit");
        assert_eq!(
            owner_view.active_marker,
            Some(QueuedOrderMarker {
                x: 120.0,
                y: 130.0,
                attack_move: true,
            })
        );
        assert_eq!(owner_view.queued_markers.len(), 2);
        assert_eq!(
            owner_view.queued_markers[0],
            QueuedOrderMarker {
                x: 140.0,
                y: 160.0,
                attack_move: false,
            }
        );
        assert_eq!(
            owner_view.queued_markers[1],
            QueuedOrderMarker {
                x: 180.0,
                y: 200.0,
                attack_move: true,
            }
        );

        let enemy_view = project_entity(2, unit, &fog, Some(&fog), false, None)
            .expect("full view should include unit");
        assert_eq!(enemy_view.active_marker, None);
        assert!(enemy_view.queued_markers.is_empty());
    }

    #[test]
    fn debug_path_is_owner_only_and_in_movement_order() {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let unit = entities.get_mut(unit_id).expect("unit should exist");
            unit.set_order(Order::move_to(300.0, 300.0));
            unit.set_path(vec![(300.0, 300.0), (200.0, 200.0), (120.0, 120.0)]);
            unit.set_path_goal(Some((300.0, 300.0)));
            unit.set_last_repath_tick(7);
            if let Some(movement) = unit.movement.as_mut() {
                movement.stuck_ticks = 2;
                movement.static_blocked_ticks = 3;
            }
        }

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            expansion_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let unit = entities.get(unit_id).expect("unit should exist");

        let owner_view = project_entity(1, unit, &fog, Some(&fog), true, None)
            .expect("owner should see own unit");
        let debug_path = owner_view
            .debug_path
            .expect("moving own unit should expose debug path in debug builds");
        assert_eq!(
            debug_path.waypoints,
            vec![
                DebugPathPoint { x: 120.0, y: 120.0 },
                DebugPathPoint { x: 200.0, y: 200.0 },
                DebugPathPoint { x: 300.0, y: 300.0 },
            ]
        );
        assert_eq!(debug_path.goal, Some(DebugPathPoint { x: 300.0, y: 300.0 }));
        assert_eq!(debug_path.last_repath_tick, 7);
        assert_eq!(debug_path.stuck_ticks, 2);
        assert_eq!(debug_path.static_blocked_ticks, 3);
        assert_eq!(debug_path.total_waypoints, 3);

        let enemy_view = project_entity(2, unit, &fog, Some(&fog), false, None)
            .expect("full view should include unit");
        assert_eq!(enemy_view.debug_path, None);
    }

    #[test]
    fn charge_cooldown_is_owner_only() {
        let mut entities = EntityStore::new();
        let rifle_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        entities
            .get_mut(rifle_id)
            .expect("rifleman should exist")
            .start_charge_cooldown(42);

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            expansion_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let rifle = entities.get(rifle_id).expect("rifleman should exist");

        let owner_view = project_entity(1, rifle, &fog, Some(&fog), true, None)
            .expect("owner should see own rifleman");
        assert_eq!(owner_view.charge_cooldown_left, Some(42));

        let enemy_view = project_entity(2, rifle, &fog, Some(&fog), false, None)
            .expect("full view should include rifleman");
        assert_eq!(enemy_view.charge_cooldown_left, None);
    }
}
