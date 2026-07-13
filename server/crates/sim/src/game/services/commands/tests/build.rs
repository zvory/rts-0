use super::*;

#[test]
fn build_order_can_start_when_worker_inside_intent_but_stages_outside() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (wx, wy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, wx, wy)
        .expect("worker should spawn");
    let spatial = SpatialIndex::build(&entities, map.size);
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = vec![player_state(1)];
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1], &entities, &map);
    let mut smokes = SmokeCloudStore::new();
    let mut ability_runtime = AbilityRuntime::new();
    let mut mortar_shells = MortarShellStore::default();
    let mut artillery_shells = ArtilleryShellStore::default();
    let mut firing_reveals = Vec::new();
    let mut events: HashMap<u32, Vec<Event>> = players
        .iter()
        .map(|player| (player.id, Vec::new()))
        .collect();

    apply_commands(
        &map,
        &mut entities,
        &mut players,
        &spatial,
        &mut coordinator,
        &fog,
        &mut smokes,
        &mut ability_runtime,
        &mut mortar_shells,
        &mut artillery_shells,
        &mut firing_reveals,
        normal_pending(vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::CityCentre,
                tile_x: 4,
                tile_y: 4,
                queued: false,
            },
        )]),
        &mut events,
        1,
    );

    let worker = entities.get(worker).expect("worker should remain alive");
    assert!(
        matches!(worker.order(), Order::Build(_)),
        "worker should keep the accepted build order"
    );
    let goal = worker
        .path_goal()
        .expect("build order should set a staging goal");
    let goal_tile = map.tile_of(goal.0, goal.1);
    assert!(
        !footprint_tiles(EntityKind::CityCentre, 4, 4).contains(&goal_tile),
        "build-over-self order should stage outside the requested footprint"
    );
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "valid build-over-self intent should not emit a failure notice"
    );
}

#[test]
fn build_order_does_not_pull_worker_off_active_construction() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, site_x, site_y)
        .expect("worker should spawn");
    let site = entities
        .spawn_building(1, EntityKind::CityCentre, site_x, site_y, false)
        .expect("scaffold should spawn");
    let worker_entity = entities.get_mut(worker).expect("worker should exist");
    worker_entity.set_order(Order::build(EntityKind::CityCentre, 4, 4));
    worker_entity.mark_build_phase(BuildPhase::Constructing { site });
    worker_entity.set_target_id(Some(site));

    let spatial = SpatialIndex::build(&entities, map.size);
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = vec![player_state(1)];
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1], &entities, &map);
    let mut smokes = SmokeCloudStore::new();
    let mut ability_runtime = AbilityRuntime::new();
    let mut mortar_shells = MortarShellStore::default();
    let mut artillery_shells = ArtilleryShellStore::default();
    let mut firing_reveals = Vec::new();
    let mut events: HashMap<u32, Vec<Event>> = players
        .iter()
        .map(|player| (player.id, Vec::new()))
        .collect();

    apply_commands(
        &map,
        &mut entities,
        &mut players,
        &spatial,
        &mut coordinator,
        &fog,
        &mut smokes,
        &mut ability_runtime,
        &mut mortar_shells,
        &mut artillery_shells,
        &mut firing_reveals,
        normal_pending(vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::Barracks,
                tile_x: 8,
                tile_y: 8,
                queued: false,
            },
        )]),
        &mut events,
        1,
    );

    let worker = entities.get(worker).expect("worker should remain alive");
    assert_eq!(
        worker.build_phase(),
        Some(BuildPhase::Constructing { site }),
        "active build command should keep constructing the original scaffold"
    );
    assert_eq!(
        worker.order().build_intent_tile(),
        Some((EntityKind::CityCentre, 4, 4)),
        "second build order must not replace the active construction intent"
    );
    assert_eq!(
        worker.target_id(),
        Some(site),
        "worker should stay latched to the scaffold it is building"
    );
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "ignored build command should not emit a failure notice"
    );
}

#[test]
fn immediate_build_skips_active_constructor_for_busy_worker() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    let constructing = entities
        .spawn_unit(1, EntityKind::Worker, 520.0, 448.0)
        .expect("constructing worker should spawn");
    let site = entities
        .spawn_building(1, EntityKind::CityCentre, site_x, site_y, false)
        .expect("scaffold should spawn");
    let miner = entities
        .spawn_unit(1, EntityKind::Worker, 96.0, 96.0)
        .expect("mining worker should spawn");
    let steel = entities
        .spawn_node(EntityKind::Steel, 64.0, 64.0)
        .expect("steel node should spawn");
    {
        let worker = entities
            .get_mut(constructing)
            .expect("constructing worker should exist");
        worker.set_order(Order::build(EntityKind::CityCentre, 4, 4));
        worker.mark_build_phase(BuildPhase::Constructing { site });
        worker.set_target_id(Some(site));
    }
    entities
        .get_mut(miner)
        .expect("mining worker should exist")
        .set_order(Order::gather(steel));

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Build {
                units: vec![constructing, miner],
                building: EntityKind::CityCentre,
                tile_x: 12,
                tile_y: 12,
                queued: false,
            },
        )],
    );

    assert_eq!(
        entities
            .get(constructing)
            .expect("constructing worker should remain")
            .order()
            .build_intent_tile(),
        Some((EntityKind::CityCentre, 4, 4)),
        "active construction must remain latched"
    );
    assert_eq!(
        entities
            .get(miner)
            .expect("mining worker should remain")
            .order()
            .build_intent_tile(),
        Some((EntityKind::CityCentre, 12, 12)),
        "the interruptible busy worker should receive the new build"
    );
}

#[test]
fn build_order_accepts_resuming_owned_scaffold() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
        .expect("worker should spawn");
    let scaffold = entities
        .spawn_building(1, EntityKind::CityCentre, site_x, site_y, false)
        .expect("scaffold should spawn");
    let spatial = SpatialIndex::build(&entities, map.size);
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = vec![player_state(1)];
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1], &entities, &map);
    let mut smokes = SmokeCloudStore::new();
    let mut ability_runtime = AbilityRuntime::new();
    let mut mortar_shells = MortarShellStore::default();
    let mut artillery_shells = ArtilleryShellStore::default();
    let mut firing_reveals = Vec::new();
    let mut events = HashMap::new();

    apply_commands(
        &map,
        &mut entities,
        &mut players,
        &spatial,
        &mut coordinator,
        &fog,
        &mut smokes,
        &mut ability_runtime,
        &mut mortar_shells,
        &mut artillery_shells,
        &mut firing_reveals,
        normal_pending(vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::CityCentre,
                tile_x: 4,
                tile_y: 4,
                queued: false,
            },
        )]),
        &mut events,
        1,
    );

    let worker = entities.get(worker).expect("worker should remain alive");
    assert!(
        matches!(worker.order(), Order::Build(_)),
        "worker should accept the resume order"
    );
    assert_eq!(
        worker.order().build_intent_tile(),
        Some((EntityKind::CityCentre, 4, 4)),
        "resume order should keep the scaffold footprint intent"
    );
    assert_ne!(
        worker.path_goal(),
        None,
        "resume order should still path the worker to the scaffold"
    );
    assert!(
        entities
            .get(scaffold)
            .expect("scaffold should survive")
            .under_construction(),
        "existing scaffold should remain available for resume"
    );
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "resume order should not emit a placement failure notice"
    );
}

#[test]
fn build_order_accepts_resuming_owned_scaffold_without_resources() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
        .expect("worker should spawn");
    entities
        .spawn_building(1, EntityKind::CityCentre, site_x, site_y, false)
        .expect("scaffold should spawn");
    let spatial = SpatialIndex::build(&entities, map.size);
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    let mut players = vec![player_state(1)];
    assert!(players[0].spend_cost(rules::economy::ResourceCost::new(1_000, 1_000)));
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1], &entities, &map);
    let mut smokes = SmokeCloudStore::new();
    let mut ability_runtime = AbilityRuntime::new();
    let mut mortar_shells = MortarShellStore::default();
    let mut artillery_shells = ArtilleryShellStore::default();
    let mut firing_reveals = Vec::new();
    let mut events = HashMap::new();

    apply_commands(
        &map,
        &mut entities,
        &mut players,
        &spatial,
        &mut coordinator,
        &fog,
        &mut smokes,
        &mut ability_runtime,
        &mut mortar_shells,
        &mut artillery_shells,
        &mut firing_reveals,
        normal_pending(vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::CityCentre,
                tile_x: 4,
                tile_y: 4,
                queued: false,
            },
        )]),
        &mut events,
        1,
    );

    let worker = entities.get(worker).expect("worker should remain alive");
    assert!(
        matches!(worker.order(), Order::Build(_)),
        "worker should accept resume orders even when the original cost is no longer affordable"
    );
    assert_eq!(
        worker.order().build_intent_tile(),
        Some((EntityKind::CityCentre, 4, 4))
    );
    assert_eq!(players[0].steel, 0, "resume order should not charge steel");
    assert_eq!(players[0].oil, 0, "resume order should not charge oil");
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "resume order should not emit a resource shortage notice"
    );
}

#[test]
fn build_order_accepts_new_build_without_current_resources() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
        .expect("worker should spawn");
    let mut players = vec![player_state(1)];
    players[0].set_resources(0, 0);

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::CityCentre,
                tile_x: 4,
                tile_y: 4,
                queued: false,
            },
        )],
    );

    let worker = entities.get(worker).expect("worker should remain alive");
    assert!(
        matches!(worker.order(), Order::Build(_)),
        "worker should keep an otherwise valid build order while broke"
    );
    assert!(
        worker.path_goal().is_some(),
        "accepted broke build order should send the worker toward the build site"
    );
    assert!(
        entities
            .iter()
            .all(|entity| entity.kind != EntityKind::CityCentre),
        "build command admission must not spawn a scaffold"
    );
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "resource shortage should be reported on arrival, not at command issue"
    );
}

#[test]
fn build_order_rejects_disabled_supply_depot() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
        .expect("worker should spawn");
    let mut players = vec![player_state(1)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::Depot,
                tile_x: 4,
                tile_y: 4,
                queued: false,
            },
        )],
    );

    assert!(
        !matches!(
            entities
                .get(worker)
                .expect("worker should remain alive")
                .order(),
            Order::Build(_)
        ),
        "disabled Supply Depot build commands must not issue an order"
    );
    assert_notice(&events, 1, "Building unavailable");
}

#[test]
fn build_order_accepts_contextual_pump_jack_on_oil() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    entities
        .spawn_node(EntityKind::Oil, site_x, site_y)
        .expect("oil node should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
        .expect("worker should spawn");
    let mut players = vec![player_state(1)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::PumpJack,
                tile_x: 4,
                tile_y: 4,
                queued: false,
            },
        )],
    );

    let worker = entities.get(worker).expect("worker should remain alive");
    assert_eq!(
        worker.order().build_intent_tile(),
        Some((EntityKind::PumpJack, 4, 4)),
        "Pump Jacks should be valid contextual worker builds on live oil patches"
    );
    assert!(
        worker.path_goal().is_some(),
        "accepted Pump Jack build order should send the worker toward the site"
    );
    assert!(
        entities
            .iter()
            .all(|entity| entity.kind != EntityKind::PumpJack),
        "build command admission must not spawn a Pump Jack scaffold"
    );
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "valid Pump Jack build admission should not emit a placement notice"
    );
}

#[test]
fn repeated_immediate_pump_jacks_distribute_across_selected_miners() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let oil_sites = [(12, 12), (14, 12), (16, 12)];
    for (tile_x, tile_y) in oil_sites {
        let (x, y) = footprint_center(&map, EntityKind::PumpJack, tile_x, tile_y);
        entities
            .spawn_node(EntityKind::Oil, x, y)
            .expect("oil node should spawn");
    }

    let worker_positions = [(360.0, 400.0), (96.0, 96.0), (128.0, 96.0)];
    let mut workers = Vec::new();
    for (index, (x, y)) in worker_positions.into_iter().enumerate() {
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, x, y)
            .expect("worker should spawn");
        let steel = entities
            .spawn_node(EntityKind::Steel, 64.0 + index as f32 * 32.0, 64.0)
            .expect("steel node should spawn");
        let worker_entity = entities.get_mut(worker).expect("worker should exist");
        worker_entity.set_order(Order::gather(steel));
        worker_entity.mark_gather_phase(GatherPhase::Harvesting);
        workers.push(worker);
    }

    apply(
        &map,
        &mut entities,
        oil_sites
            .into_iter()
            .map(|(tile_x, tile_y)| {
                (
                    1,
                    SimCommand::Build {
                        units: workers.clone(),
                        building: EntityKind::PumpJack,
                        tile_x,
                        tile_y,
                        queued: false,
                    },
                )
            })
            .collect(),
    );

    let mut assigned_sites: Vec<(u32, u32)> = workers
        .iter()
        .filter_map(|worker| {
            entities
                .get(*worker)
                .and_then(|entity| entity.order().build_intent_tile())
                .map(|(_, tile_x, tile_y)| (tile_x, tile_y))
        })
        .collect();
    assigned_sites.sort_unstable();

    assert_eq!(assigned_sites, oil_sites);
}

#[test]
fn build_order_rejects_pump_jack_off_oil() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
        .expect("worker should spawn");
    let mut players = vec![player_state(1)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::PumpJack,
                tile_x: 4,
                tile_y: 4,
                queued: false,
            },
        )],
    );

    assert!(
        !matches!(
            entities
                .get(worker)
                .expect("worker should remain alive")
                .order(),
            Order::Build(_)
        ),
        "Pump Jack build orders must be rejected away from live oil patches"
    );
    assert_notice(&events, 1, "Cannot build there");
}

#[test]
fn build_with_multiple_selected_workers_uses_idle_closest_worker() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let busy_close = entities
        .spawn_unit(1, EntityKind::Worker, 555.0, 512.0)
        .expect("busy worker should spawn");
    let idle_far = entities
        .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
        .expect("far worker should spawn");
    let idle_close = entities
        .spawn_unit(1, EntityKind::Worker, 570.0, 512.0)
        .expect("close worker should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, 560.0, 560.0)
        .expect("node should spawn");
    entities
        .get_mut(busy_close)
        .expect("busy worker should exist")
        .set_order(Order::gather(node));

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Build {
                units: vec![busy_close, idle_far, idle_close],
                building: EntityKind::CityCentre,
                tile_x: 12,
                tile_y: 12,
                queued: false,
            },
        )],
    );

    assert!(matches!(
        entities.get(idle_close).expect("close worker").order(),
        Order::Build(_)
    ));
    assert!(matches!(
        entities.get(idle_far).expect("far worker").order(),
        Order::Idle
    ));
    assert!(matches!(
        entities.get(busy_close).expect("busy worker").order(),
        Order::Gather(_)
    ));
}

#[test]
fn queued_builds_distribute_across_selected_workers_by_queue_length() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let first = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 64.0, cc_y)
        .expect("first worker should spawn");
    let second = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
        .expect("second worker should spawn");

    apply(
        &map,
        &mut entities,
        (0..4)
            .map(|i| {
                (
                    1,
                    SimCommand::Build {
                        units: vec![first, second],
                        building: EntityKind::CityCentre,
                        tile_x: 10 + i,
                        tile_y: 10,
                        queued: true,
                    },
                )
            })
            .collect(),
    );

    assert_eq!(entities.get(first).unwrap().queued_orders().len(), 2);
    assert_eq!(entities.get(second).unwrap().queued_orders().len(), 2);
}

#[test]
fn queued_build_prefers_idle_worker_over_closer_active_builder() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let west = entities
        .spawn_unit(1, EntityKind::Worker, 320.0, 512.0)
        .expect("west worker should spawn");
    let east = entities
        .spawn_unit(1, EntityKind::Worker, 640.0, 512.0)
        .expect("east worker should spawn");
    entities
        .get_mut(west)
        .expect("west worker should exist")
        .set_order(Order::build(EntityKind::CityCentre, 8, 16));

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Build {
                units: vec![west, east],
                building: EntityKind::CityCentre,
                tile_x: 9,
                tile_y: 16,
                queued: true,
            },
        )],
    );

    assert!(
        entities.get(west).unwrap().queued_orders().is_empty(),
        "closer worker already walking to build should not receive the queued build"
    );
    assert_eq!(
        entities.get(east).unwrap().queued_orders(),
        &[OrderIntent::build(EntityKind::CityCentre, 9, 16)],
        "idle worker should receive the next queued build"
    );
}

#[test]
fn repeated_invalid_queued_builds_stay_bounded() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
        .expect("worker should spawn");
    let pending = (0..32)
        .map(|_| {
            (
                1,
                SimCommand::Build {
                    units: vec![worker],
                    building: EntityKind::CityCentre,
                    tile_x: u32::MAX,
                    tile_y: u32::MAX,
                    queued: true,
                },
            )
        })
        .collect();

    apply(&map, &mut entities, pending);

    assert_eq!(
        entities
            .get(worker)
            .expect("worker should exist")
            .queued_orders()
            .len(),
        8,
        "queued build intents should enforce the per-unit queue cap even when invalid"
    );
}
