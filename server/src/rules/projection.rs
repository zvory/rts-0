//! Projection rules for fog-gated entity views and event delivery.
//!
//! This module owns what a player is allowed to see. It does not mutate the world; future
//! last-known-position or partial-reveal rules should grow here.

use crate::config;
use crate::game::ability;
use crate::game::entity::{
    fires_while_moving, Entity, EntityKind, EntityStore, GatherPhase, Order, OrderIntent,
};
use crate::game::fog::Fog;
use crate::protocol::{AbilityCooldownView, DebugPathPoint, DebugPathView};
use crate::protocol::{EntityView, OrderPlanMarker};

const MAX_DEBUG_PATH_WAYPOINTS: usize = 128;

pub struct EntityProjectionContext<'a> {
    pub fog: &'a Fog,
    pub actionable_fog: Option<&'a Fog>,
    pub fogged: bool,
    pub entities: &'a EntityStore,
    pub target: Option<&'a Entity>,
    pub include_debug_path: bool,
}

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
    context: EntityProjectionContext<'_>,
) -> Option<EntityView> {
    if context.fogged && !entity_visible_to(viewer, entity, context.fog) {
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
    let actionable_fog = context.actionable_fog.unwrap_or(context.fog);
    let vision_only = context.fogged
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
        context
            .target
            .filter(|target| target.id == target_id)
            .map(|target| {
                entity.owner == viewer
                    || !context.fogged
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
                || !context.fogged
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
        view.order_plan = order_plan(entity, context.entities, viewer, actionable_fog);
        if context.include_debug_path {
            view.debug_path = debug_path_view(entity);
        }
        if entity.kind == EntityKind::Rifleman {
            let charge_cooldown_left = entity.charge_cooldown_ticks();
            if charge_cooldown_left > 0 {
                view.charge_cooldown_left = Some(charge_cooldown_left);
            }
        }
        view.abilities = entity
            .ability_cooldowns
            .iter()
            .filter(|(_, cooldown_left)| **cooldown_left > 0)
            .map(|(kind, cooldown_left)| AbilityCooldownView {
                ability: kind.to_protocol_str().to_string(),
                cooldown_left: *cooldown_left,
            })
            .collect();
        for kind in [ability::AbilityKind::Charge, ability::AbilityKind::Smoke] {
            if ability::carried_by(kind, entity.kind)
                && !view
                    .abilities
                    .iter()
                    .any(|cooldown| cooldown.ability == kind.to_protocol_str())
            {
                view.abilities.push(AbilityCooldownView {
                    ability: kind.to_protocol_str().to_string(),
                    cooldown_left: 0,
                });
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
            if let Some(target) = context.target {
                if target.id == target_id && target_visible {
                    view.target_id = Some(target_id);
                }
            }
        }
    }

    Some(view)
}

fn order_plan(
    entity: &Entity,
    entities: &EntityStore,
    viewer: u32,
    fog: &Fog,
) -> Vec<OrderPlanMarker> {
    let mut plan = Vec::new();
    if let Some(marker) = active_order_plan_marker(entity, entities, viewer, fog) {
        plan.push(marker);
    }
    plan.extend(
        entity
            .queued_orders()
            .iter()
            .filter_map(|intent| intent_plan_marker(intent, entities, viewer, fog)),
    );
    plan
}

fn active_order_plan_marker(
    entity: &Entity,
    entities: &EntityStore,
    viewer: u32,
    fog: &Fog,
) -> Option<OrderPlanMarker> {
    match entity.order() {
        Order::Move(_) => {
            let (x, y) = entity.path_goal().or_else(|| entity.move_intent())?;
            point_marker("move", x, y)
        }
        Order::AttackMove(_) => {
            let (x, y) = entity.path_goal().or_else(|| entity.move_intent())?;
            point_marker("attackMove", x, y)
        }
        Order::Attack(order) => target_marker("attack", order.intent.target, entities, viewer, fog),
        Order::Gather(order) => entity_point_marker("gather", order.intent.node, entities),
        Order::Build(order) => {
            build_marker(order.intent.kind, order.intent.tile_x, order.intent.tile_y)
        }
        Order::Idle => None,
    }
}

fn intent_plan_marker(
    intent: &OrderIntent,
    entities: &EntityStore,
    viewer: u32,
    fog: &Fog,
) -> Option<OrderPlanMarker> {
    match intent {
        OrderIntent::Move(point) => point_marker("move", point.x, point.y),
        OrderIntent::AttackMove(point) => point_marker("attackMove", point.x, point.y),
        OrderIntent::Attack(attack) => {
            target_marker("attack", attack.target, entities, viewer, fog)
        }
        OrderIntent::Gather(gather) => entity_point_marker("gather", gather.node, entities),
        OrderIntent::Build(build) => build_marker(build.kind, build.tile_x, build.tile_y),
    }
}

fn target_marker(
    kind: &str,
    target: u32,
    entities: &EntityStore,
    viewer: u32,
    fog: &Fog,
) -> Option<OrderPlanMarker> {
    let target = entities.get(target)?;
    fog.is_visible_world(viewer, target.pos_x, target.pos_y)
        .then(|| point_marker(kind, target.pos_x, target.pos_y))
        .flatten()
}

fn entity_point_marker(kind: &str, id: u32, entities: &EntityStore) -> Option<OrderPlanMarker> {
    let entity = entities.get(id)?;
    point_marker(kind, entity.pos_x, entity.pos_y)
}

fn build_marker(kind: EntityKind, tile_x: u32, tile_y: u32) -> Option<OrderPlanMarker> {
    let stats = config::building_stats(kind)?;
    let tile_size = config::TILE_SIZE as f32;
    let x = tile_x as f32 * tile_size + stats.foot_w as f32 * tile_size * 0.5;
    let y = tile_y as f32 * tile_size + stats.foot_h as f32 * tile_size * 0.5;
    point_marker("build", x, y)
}

fn point_marker(kind: &str, x: f32, y: f32) -> Option<OrderPlanMarker> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(OrderPlanMarker {
        kind: kind.to_string(),
        x,
        y,
    })
}

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

fn debug_path_point(x: f32, y: f32) -> Option<DebugPathPoint> {
    (x.is_finite() && y.is_finite()).then_some(DebugPathPoint { x, y })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, Order, OrderIntent};
    use crate::game::map::Map;
    use crate::protocol::terrain;

    fn project_for_test(
        viewer: u32,
        entity: &Entity,
        fog: &Fog,
        fogged: bool,
        entities: &EntityStore,
        target: Option<&Entity>,
        include_debug_path: bool,
    ) -> Option<EntityView> {
        project_entity(
            viewer,
            entity,
            EntityProjectionContext {
                fog,
                actionable_fog: Some(fog),
                fogged,
                entities,
                target,
                include_debug_path,
            },
        )
    }

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

        let enemy_view =
            project_for_test(1, tank, &fog, true, &entities, Some(hidden_target), false)
                .expect("viewer should see nearby tank");
        assert_eq!(enemy_view.target_id, None);
        assert_eq!(enemy_view.weapon_facing, None);

        let owner_view =
            project_for_test(2, tank, &fog, true, &entities, Some(hidden_target), false)
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

        let viewer_view = project_for_test(1, tank, &fog, true, &entities, Some(target), false)
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

        let view = project_for_test(1, tank, &fog, true, &entities, None, false)
            .expect("tank should be visible");
        assert_eq!(view.oil_used, Some(3.25));
    }

    #[test]
    fn order_plan_is_owner_only_and_projects_safe_stages() {
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

        let owner_view = project_for_test(1, unit, &fog, true, &entities, None, false)
            .expect("owner should see own unit");
        assert_eq!(
            owner_view.order_plan,
            vec![
                OrderPlanMarker {
                    kind: "attackMove".to_string(),
                    x: 120.0,
                    y: 130.0,
                },
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 140.0,
                    y: 160.0,
                },
                OrderPlanMarker {
                    kind: "gather".to_string(),
                    x: 720.0,
                    y: 720.0,
                },
                OrderPlanMarker {
                    kind: "attackMove".to_string(),
                    x: 180.0,
                    y: 200.0,
                },
            ]
        );

        let enemy_view = project_for_test(2, unit, &fog, false, &entities, None, false)
            .expect("full view should include unit");
        assert!(enemy_view.order_plan.is_empty());
    }

    #[test]
    fn active_build_marker_uses_building_footprint_center() {
        let mut entities = EntityStore::new();
        let worker_id = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("worker should spawn");
        {
            let worker = entities.get_mut(worker_id).expect("worker should exist");
            worker.set_order(Order::build(EntityKind::Depot, 4, 5));
            worker.append_queued_order(OrderIntent::move_to(320.0, 352.0));
        }

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            expansion_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let worker = entities.get(worker_id).expect("worker should exist");

        let owner_view = project_for_test(1, worker, &fog, true, &entities, None, false)
            .expect("owner should see own worker");
        assert_eq!(
            owner_view.order_plan,
            vec![
                OrderPlanMarker {
                    kind: "build".to_string(),
                    x: 160.0,
                    y: 192.0,
                },
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 320.0,
                    y: 352.0,
                },
            ]
        );

        let enemy_view = project_for_test(2, worker, &fog, false, &entities, None, false)
            .expect("full view should include worker");
        assert!(enemy_view.order_plan.is_empty());
    }

    #[test]
    fn debug_path_is_runtime_debug_mode_owner_only_and_in_movement_order() {
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

        let standard_owner_view = project_for_test(1, unit, &fog, true, &entities, None, false)
            .expect("owner should see own unit");
        assert_eq!(standard_owner_view.debug_path, None);

        let owner_view = project_for_test(1, unit, &fog, true, &entities, None, true)
            .expect("owner should see own unit");
        let debug_path = owner_view
            .debug_path
            .expect("moving own unit should expose debug path when runtime debug mode is enabled");
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

        let enemy_view = project_for_test(2, unit, &fog, false, &entities, None, true)
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
            .start_ability_cooldown(ability::AbilityKind::Charge, 42);

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            expansion_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let rifle = entities.get(rifle_id).expect("rifleman should exist");

        let owner_view = project_for_test(1, rifle, &fog, true, &entities, None, false)
            .expect("owner should see own rifleman");
        assert_eq!(owner_view.charge_cooldown_left, Some(42));

        let enemy_view = project_for_test(2, rifle, &fog, false, &entities, None, false)
            .expect("full view should include rifleman");
        assert_eq!(enemy_view.charge_cooldown_left, None);
    }
}
