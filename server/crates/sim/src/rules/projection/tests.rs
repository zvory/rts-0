use super::*;
use crate::game::entity::{
    tank_trap_deconstruction_ticks, DeconstructPhase, EntityKind, EntityStore, Order, OrderIntent,
};
use crate::game::map::Map;
use crate::protocol::terrain;

fn flat_map(size: u32) -> Map {
    Map {
        width: size,
        height: size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4)],
        ..Default::default()
    }
}

fn project_for_test(
    viewer: u32,
    entity: &Entity,
    fog: &Fog,
    fogged: bool,
    entities: &EntityStore,
    target: Option<&Entity>,
    include_debug_path: bool,
) -> Option<EntityView> {
    project_for_test_with_debug_projection(
        viewer,
        entity,
        fog,
        fogged,
        entities,
        target,
        if include_debug_path {
            DebugPathProjection::OwnerOnly
        } else {
            DebugPathProjection::None
        },
    )
}

fn project_for_test_with_debug_projection(
    viewer: u32,
    entity: &Entity,
    fog: &Fog,
    fogged: bool,
    entities: &EntityStore,
    target: Option<&Entity>,
    debug_path_projection: DebugPathProjection,
) -> Option<EntityView> {
    project_for_test_with_active_sites(
        viewer,
        entity,
        fog,
        fogged,
        entities,
        target,
        debug_path_projection,
        None,
    )
}

fn project_for_test_with_active_sites(
    viewer: u32,
    entity: &Entity,
    fog: &Fog,
    fogged: bool,
    entities: &EntityStore,
    target: Option<&Entity>,
    debug_path_projection: DebugPathProjection,
    active_construction_sites: Option<&BTreeSet<u32>>,
) -> Option<EntityView> {
    project_entity(
        viewer,
        entity,
        EntityProjectionContext {
            map: None,
            fog,
            actionable_fog: Some(fog),
            private_detail_fog: Some(fog),
            concealment_fog: None,
            private_detail_projection: PrivateDetailProjection::ExactViewer,
            smokes: None,
            fogged,
            entities,
            target,
            debug_path_projection,
            active_construction_sites,
            extractor_active: None,
            teams: None,
            owner_faction_id: Some(crate::rules::faction::DEFAULT_FACTION_ID),
            ability_runtime: None,
            tick: 0,
        },
    )
}

#[test]
fn pump_jack_extractor_state_projects_for_every_visible_pump() {
    let mut entities = EntityStore::new();
    let pump_id = entities
        .spawn_building(1, EntityKind::PumpJack, 128.0, 128.0, true)
        .expect("pump jack should spawn");
    let pump = entities.get(pump_id).expect("pump jack entity");
    let fog = Fog::new(16, 16);
    let teams = TeamRelations::from_player_teams([(1, 7), (2, 7), (3, 3)]);
    let project = |viewer| {
        project_entity(
            viewer,
            pump,
            EntityProjectionContext {
                map: None,
                fog: &fog,
                actionable_fog: Some(&fog),
                private_detail_fog: Some(&fog),
                concealment_fog: None,
                private_detail_projection: PrivateDetailProjection::ExactViewer,
                smokes: None,
                fogged: false,
                entities: &entities,
                target: None,
                debug_path_projection: DebugPathProjection::None,
                active_construction_sites: None,
                extractor_active: Some(false),
                teams: Some(&teams),
                owner_faction_id: Some(crate::rules::faction::DEFAULT_FACTION_ID),
                ability_runtime: None,
                tick: 0,
            },
        )
        .expect("unfogged pump jack should project")
    };

    assert_eq!(project(1).extractor_active, Some(false));
    assert_eq!(project(2).extractor_active, Some(false));
    assert_eq!(project(3).extractor_active, Some(false));
}

#[test]
fn weapon_facing_is_omitted_when_target_direction_is_hidden() {
    let mut entities = EntityStore::new();
    let spotter_id = entities
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
        let combat = tank.combat.as_mut().expect("combat");
        combat.commander_facing = -0.8;
        combat.commander_target_id = Some(hidden_target_id);
    }
    let map = Map {
        width: 64,
        height: 64,
        terrain: vec![terrain::GRASS; 64 * 64],
        starts: vec![(1, 1)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2, 3], &entities, &map);
    let tank = entities.get(tank_id).expect("tank should exist");
    let hidden_target = entities
        .get(hidden_target_id)
        .expect("hidden target should exist");

    let enemy_view = project_for_test(1, tank, &fog, true, &entities, Some(hidden_target), false)
        .expect("viewer should see nearby tank");
    assert_eq!(enemy_view.target_id, None);
    assert_eq!(enemy_view.weapon_facing, None);
    assert_eq!(enemy_view.commander_facing, None);

    let owner_view = project_for_test(2, tank, &fog, true, &entities, Some(hidden_target), false)
        .expect("owner should see own tank");
    assert_eq!(owner_view.target_id, Some(hidden_target_id));
    assert_eq!(owner_view.weapon_facing, Some(1.2));
    assert_eq!(owner_view.commander_facing, Some(-0.8));

    for (cannon_target, commander_target, expected_facing) in [
        (Some(hidden_target_id), None, None),
        (Some(hidden_target_id), Some(spotter_id), Some(-0.8)),
        (None, None, Some(-0.8)),
    ] {
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        tank.set_target_id(cannon_target);
        tank.combat.as_mut().expect("combat").commander_target_id = commander_target;
        let tank = entities.get(tank_id).expect("tank should exist");
        let target = cannon_target.and_then(|id| entities.get(id));
        let view = project_for_test(1, tank, &fog, true, &entities, target, false)
            .expect("viewer should see nearby tank");
        assert_eq!(
            view.commander_facing, expected_facing,
            "idle MG aim must not reveal a hidden cannon target, but independent visible aim is safe"
        );
    }
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
        width: 16,
        height: 16,
        terrain: vec![terrain::GRASS; 16 * 16],
        starts: vec![(1, 1)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2, 3], &entities, &map);
    let tank = entities.get(tank_id).expect("tank should exist");
    let target = entities.get(target_id).expect("target should exist");

    let viewer_view = project_for_test(1, tank, &fog, true, &entities, Some(target), false)
        .expect("viewer should see tank");

    assert_eq!(viewer_view.target_id, Some(target_id));
    assert_eq!(viewer_view.weapon_facing, Some(0.0));
}

#[test]
fn idle_combat_unit_projects_visible_acquired_target() {
    let mut entities = EntityStore::new();
    entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("viewer spotter should spawn");
    let gunner_id = entities
        .spawn_unit(2, EntityKind::MachineGunner, 120.0, 100.0)
        .expect("machine gunner should spawn");
    let target_id = entities
        .spawn_unit(3, EntityKind::Rifleman, 140.0, 100.0)
        .expect("target should spawn");
    {
        let gunner = entities.get_mut(gunner_id).expect("gunner should exist");
        gunner.set_target_id(Some(target_id));
        gunner.set_weapon_facing(0.0);
    }
    let map = Map {
        width: 16,
        height: 16,
        terrain: vec![terrain::GRASS; 16 * 16],
        starts: vec![(1, 1)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2, 3], &entities, &map);
    let gunner = entities.get(gunner_id).expect("gunner should exist");
    let target = entities.get(target_id).expect("target should exist");

    let viewer_view = project_for_test(1, gunner, &fog, true, &entities, Some(target), false)
        .expect("viewer should see gunner");

    assert_eq!(viewer_view.state, "idle");
    assert_eq!(viewer_view.target_id, Some(target_id));
    assert_eq!(viewer_view.weapon_facing, Some(0.0));
}

#[test]
fn idle_combat_unit_omits_hidden_acquired_target() {
    let mut entities = EntityStore::new();
    entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("viewer spotter should spawn");
    let gunner_id = entities
        .spawn_unit(2, EntityKind::MachineGunner, 120.0, 100.0)
        .expect("machine gunner should spawn");
    let hidden_target_id = entities
        .spawn_unit(3, EntityKind::Rifleman, 700.0, 700.0)
        .expect("hidden target should spawn");
    {
        let gunner = entities.get_mut(gunner_id).expect("gunner should exist");
        gunner.set_target_id(Some(hidden_target_id));
        gunner.set_weapon_facing(1.2);
    }
    let map = Map {
        width: 64,
        height: 64,
        terrain: vec![terrain::GRASS; 64 * 64],
        starts: vec![(1, 1)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2, 3], &entities, &map);
    let gunner = entities.get(gunner_id).expect("gunner should exist");
    let hidden_target = entities
        .get(hidden_target_id)
        .expect("hidden target should exist");

    let viewer_view =
        project_for_test(1, gunner, &fog, true, &entities, Some(hidden_target), false)
            .expect("viewer should see nearby gunner");

    assert_eq!(viewer_view.state, "idle");
    assert_eq!(viewer_view.target_id, None);
    assert_eq!(viewer_view.weapon_facing, None);
}

#[test]
fn tank_weapon_range_tiles_are_owner_only() {
    let mut entities = EntityStore::new();
    let tank_id = entities
        .spawn_unit(1, EntityKind::Tank, 120.0, 100.0)
        .expect("tank should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
        .expect("enemy spotter should spawn");
    {
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        if let Some(combat) = tank.combat.as_mut() {
            combat.tank_stationary_range_ticks = TANK_STATIONARY_RANGE_RAMP_TICKS / 2;
        }
    }
    let map = Map {
        width: 64,
        height: 64,
        terrain: vec![terrain::GRASS; 64 * 64],
        starts: vec![(1, 1)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2], &entities, &map);
    let tank = entities.get(tank_id).expect("tank should exist");

    let owner_view = project_for_test(1, tank, &fog, true, &entities, None, false)
        .expect("owner should see tank");
    assert_eq!(owner_view.weapon_range_tiles, Some(9.5));

    let enemy_view = project_for_test(2, tank, &fog, true, &entities, None, false)
        .expect("enemy should see nearby tank");
    assert_eq!(enemy_view.weapon_range_tiles, None);
}

#[test]
fn breakthrough_caster_status_is_visible_without_owner_ability_details() {
    let mut entities = EntityStore::new();
    let command_car_id = entities
        .spawn_unit(1, EntityKind::CommandCar, 120.0, 100.0)
        .expect("Command Car should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
        .expect("enemy spotter should spawn");
    entities
        .get_mut(command_car_id)
        .expect("Command Car should exist")
        .start_breakthrough_aura(90);

    let map = flat_map(64);
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2], &entities, &map);
    let command_car = entities
        .get(command_car_id)
        .expect("Command Car should exist");

    let owner_view = project_for_test(1, command_car, &fog, true, &entities, None, false)
        .expect("owner should see Command Car");
    assert_eq!(owner_view.breakthrough_aura_ticks, Some(90));
    assert!(owner_view.abilities.iter().any(|ability| {
        ability.ability == protocol::abilities::BREAKTHROUGH && ability.expires_in == Some(90)
    }));

    let enemy_view = project_for_test(2, command_car, &fog, true, &entities, None, false)
        .expect("enemy spotter should reveal Command Car");
    assert_eq!(enemy_view.breakthrough_aura_ticks, Some(90));
    assert!(
        enemy_view.abilities.is_empty(),
        "caster status must not expose owner-only ability details"
    );
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
        width: 64,
        height: 64,
        terrain: vec![terrain::GRASS; 64 * 64],
        starts: vec![(1, 1), (40, 40)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
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
                building_kind: None,
            },
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 140.0,
                y: 160.0,
                building_kind: None,
            },
            OrderPlanMarker {
                kind: "gather".to_string(),
                x: 720.0,
                y: 720.0,
                building_kind: None,
            },
            OrderPlanMarker {
                kind: "attackMove".to_string(),
                x: 180.0,
                y: 200.0,
                building_kind: None,
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
        width: 64,
        height: 64,
        terrain: vec![terrain::GRASS; 64 * 64],
        starts: vec![(1, 1), (40, 40)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
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
                building_kind: Some("depot".to_string()),
            },
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 320.0,
                y: 352.0,
                building_kind: None,
            },
        ]
    );

    let enemy_view = project_for_test(2, worker, &fog, false, &entities, None, false)
        .expect("full view should include worker");
    assert!(enemy_view.order_plan.is_empty());
}

#[test]
fn queued_hold_position_projects_at_the_preceding_stage_destination() {
    let mut entities = EntityStore::new();
    let unit_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    {
        let unit = entities.get_mut(unit_id).expect("unit should exist");
        unit.set_order(Order::move_to(120.0, 130.0));
        unit.append_queued_order(OrderIntent::move_to(180.0, 200.0));
        unit.append_queued_order(OrderIntent::hold_position());
    }

    let map = Map {
        width: 64,
        height: 64,
        terrain: vec![terrain::GRASS; 64 * 64],
        starts: vec![(1, 1)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1], &entities, &map);
    let unit = entities.get(unit_id).expect("unit should exist");

    let owner_view = project_for_test(1, unit, &fog, true, &entities, None, false)
        .expect("owner should see own unit");
    assert_eq!(
        owner_view.order_plan,
        vec![
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 120.0,
                y: 130.0,
                building_kind: None,
            },
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 180.0,
                y: 200.0,
                building_kind: None,
            },
            OrderPlanMarker {
                kind: "holdPosition".to_string(),
                x: 180.0,
                y: 200.0,
                building_kind: None,
            },
        ]
    );
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
        width: 64,
        height: 64,
        terrain: vec![terrain::GRASS; 64 * 64],
        starts: vec![(1, 1), (40, 40)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
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

    let full_world_view = project_for_test_with_debug_projection(
        2,
        unit,
        &fog,
        false,
        &entities,
        None,
        DebugPathProjection::AllProjected,
    )
    .expect("full-world diagnostics should include projected units");
    assert!(
        full_world_view.debug_path.is_some(),
        "full-world diagnostic policy may expose movement paths for every projected entity"
    );
}

#[test]
fn legacy_charge_cooldown_is_not_projected() {
    let mut entities = EntityStore::new();
    let rifle_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    entities
        .get_mut(rifle_id)
        .expect("rifleman should exist")
        .start_ability_cooldown(ability::AbilityKind::Charge, 42);

    let map = Map {
        width: 64,
        height: 64,
        terrain: vec![terrain::GRASS; 64 * 64],
        starts: vec![(1, 1), (40, 40)],
        ..Default::default()
    };
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2], &entities, &map);
    let rifle = entities.get(rifle_id).expect("rifleman should exist");

    let owner_view = project_for_test(1, rifle, &fog, true, &entities, None, false)
        .expect("owner should see own rifleman");
    assert_eq!(owner_view.charge_cooldown_left, None);
    assert!(owner_view
        .abilities
        .iter()
        .all(|entry| entry.ability != "charge"));

    let enemy_view = project_for_test(2, rifle, &fog, false, &entities, None, false)
        .expect("full view should include rifleman");
    assert_eq!(enemy_view.charge_cooldown_left, None);
    assert!(enemy_view.abilities.is_empty());
}

#[test]
fn construction_active_signal_is_owner_only() {
    let mut entities = EntityStore::new();
    let scaffold_id = entities
        .spawn_building(1, EntityKind::Barracks, 160.0, 160.0, false)
        .expect("scaffold should spawn");
    let scaffold = entities.get(scaffold_id).expect("scaffold should exist");
    let mut fog = Fog::new(16, 16);
    fog.recompute(&[1, 2], &entities, &flat_map(16));
    let mut active_sites = BTreeSet::new();
    active_sites.insert(scaffold_id);

    let owner_view = project_for_test_with_active_sites(
        1,
        scaffold,
        &fog,
        true,
        &entities,
        None,
        DebugPathProjection::None,
        Some(&active_sites),
    )
    .expect("owner should see scaffold");
    assert_eq!(owner_view.build_progress, Some(0.0));
    assert!(owner_view.build_active);

    let enemy_view = project_for_test_with_active_sites(
        2,
        scaffold,
        &fog,
        false,
        &entities,
        None,
        DebugPathProjection::None,
        Some(&active_sites),
    )
    .expect("non-owner visible scaffold should project");
    assert_eq!(enemy_view.build_progress, Some(0.0));
    assert!(!enemy_view.build_active);
}

#[test]
fn tank_trap_deconstruction_projects_reverse_progress() {
    let required_ticks = tank_trap_deconstruction_ticks();
    let elapsed_ticks = required_ticks / 2;
    let mut entities = EntityStore::new();
    let trap_id = entities
        .spawn_building(2, EntityKind::TankTrap, 160.0, 160.0, true)
        .expect("tank trap should spawn");
    let worker_id = entities
        .spawn_unit(1, EntityKind::Worker, 192.0, 160.0)
        .expect("worker should spawn");
    {
        let worker = entities.get_mut(worker_id).expect("worker should exist");
        worker.set_order(Order::deconstruct(trap_id));
        worker.mark_deconstruct_phase(DeconstructPhase::Deconstructing);
        for _ in 0..elapsed_ticks {
            worker.tick_deconstruction();
        }
    }

    let fog = Fog::new(16, 16);
    let trap = entities.get(trap_id).expect("trap should exist");
    let view = project_for_test(1, trap, &fog, false, &entities, None, false)
        .expect("viewer should see trap");

    assert_eq!(view.build_progress, None);
    let expected_remaining = 1.0 - elapsed_ticks as f32 / required_ticks as f32;
    assert!(
        (view
            .deconstruct_progress
            .expect("deconstruct progress should project")
            - expected_remaining)
            .abs()
            < 0.001,
        "deconstruction progress should be the remaining reverse fraction"
    );
}
