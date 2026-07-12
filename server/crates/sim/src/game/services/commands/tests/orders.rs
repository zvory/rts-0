use super::*;

#[test]
fn queued_move_appends_until_cap_and_normal_move_clears_queue() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");

    let queued_moves = (0..10)
        .map(|i| {
            (
                1,
                SimCommand::Move {
                    units: vec![unit],
                    x: 120.0 + i as f32,
                    y: 140.0,
                    queued: true,
                },
            )
        })
        .collect();
    apply(&map, &mut entities, queued_moves);

    let entity = entities.get(unit).expect("unit should exist");
    assert_eq!(
        entity.queued_orders().len(),
        8,
        "unit queue should enforce the phase-0 cap"
    );
    assert!(
        matches!(entity.order(), Order::Idle),
        "queued command should not interrupt the active order in phase 0"
    );

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![unit],
                x: 200.0,
                y: 220.0,
                queued: false,
            },
        )],
    );

    let entity = entities.get(unit).expect("unit should exist");
    assert!(
        entity.queued_orders().is_empty(),
        "replacement move should clear queued intents"
    );
    assert!(
        matches!(entity.order(), Order::Move(_)),
        "replacement move should still issue the active order"
    );
}

#[test]
fn mixed_selection_move_filters_to_owned_movable_units() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let owned_rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("owned rifleman should spawn");
    let owned_worker = entities
        .spawn_unit(1, EntityKind::Worker, 132.0, 100.0)
        .expect("owned worker should spawn");
    let enemy_rifle = entities
        .spawn_unit(2, EntityKind::Rifleman, 164.0, 100.0)
        .expect("enemy rifleman should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, 196.0, 100.0)
        .expect("resource node should spawn");
    mark_units_moving(&mut entities, &[owned_rifle, owned_worker, enemy_rifle]);

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![owned_rifle, enemy_rifle, node, 99_999, owned_worker],
                x: 220.0,
                y: 180.0,
                queued: false,
            },
        )],
    );

    assert!(
        matches!(entities.get(owned_rifle).unwrap().order(), Order::Move(_)),
        "owned movable units in a mixed selection should accept the move"
    );
    assert!(
        matches!(entities.get(owned_worker).unwrap().order(), Order::Move(_)),
        "later owned units should still be processed after invalid selection entries"
    );
    assert_eq!(
        entities.get(enemy_rifle).unwrap().move_intent(),
        Some((10.0, 10.0)),
        "enemy units in the selected id list must be ignored"
    );
    assert!(
        entities.get(node).unwrap().queued_orders().is_empty(),
        "resource node ids in the selected list must not gain queued state"
    );
}

#[test]
fn planner_backed_existing_command_families_preserve_active_and_queued_state() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 16.0, cc_y)
        .expect("worker should spawn");
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, cc_x + 48.0, cc_y)
        .expect("rifleman should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, cc_x + 96.0, cc_y)
        .expect("target should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, cc_x + 64.0, cc_y)
        .expect("node should spawn");

    entities
        .get_mut(rifle)
        .unwrap()
        .append_queued_order(OrderIntent::move_to(700.0, 700.0));
    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![rifle],
                x: 180.0,
                y: 180.0,
                queued: false,
            },
        )],
    );
    assert!(matches!(
        entities.get(rifle).unwrap().order(),
        Order::Move(_)
    ));
    assert!(entities.get(rifle).unwrap().queued_orders().is_empty());

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::AttackMove {
                units: vec![rifle],
                x: 220.0,
                y: 180.0,
                queued: true,
            },
        )],
    );
    assert!(matches!(
        entities.get(rifle).unwrap().order(),
        Order::Move(_)
    ));
    assert!(matches!(
        entities.get(rifle).unwrap().queued_orders().last(),
        Some(OrderIntent::AttackMove(_))
    ));

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Attack {
                units: vec![rifle],
                target,
                queued: false,
            },
        )],
    );
    assert!(matches!(
        entities.get(rifle).unwrap().order(),
        Order::Attack(_)
    ));
    assert!(entities.get(rifle).unwrap().queued_orders().is_empty());

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Gather {
                units: vec![worker],
                node,
                queued: false,
            },
        )],
    );
    assert!(matches!(
        entities.get(worker).unwrap().order(),
        Order::Gather(_)
    ));

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::CityCentre,
                tile_x: 10,
                tile_y: 10,
                queued: true,
            },
        )],
    );
    assert!(matches!(
        entities.get(worker).unwrap().queued_orders().last(),
        Some(OrderIntent::Build(_))
    ));
}

#[test]
fn gather_command_accepts_occupied_but_mineable_resource_node() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, cc_x + 64.0, cc_y)
        .expect("node should spawn");
    let holder = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 64.0, cc_y)
        .expect("holder should spawn");
    {
        let h = entities.get_mut(holder).expect("holder should exist");
        h.set_order(Order::gather(node));
        h.mark_gather_phase(GatherPhase::Harvesting);
    }
    assert!(entities.claim_miner(node, holder));
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 32.0, cc_y)
        .expect("worker should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Gather {
                units: vec![worker],
                node,
                queued: false,
            },
        )],
    );

    assert_eq!(
        entities
            .get(worker)
            .expect("worker should exist")
            .order()
            .gather_node(),
        Some(node),
        "occupied mineable resources should remain valid gather targets; scatter happens on arrival"
    );
}

#[test]
fn gather_command_rejects_oil_resource_nodes() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let node = entities
        .spawn_node(EntityKind::Oil, cc_x + 64.0, cc_y)
        .expect("oil node should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 32.0, cc_y)
        .expect("worker should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Gather {
                units: vec![worker],
                node,
                queued: false,
            },
        )],
    );

    assert!(
        !matches!(
            entities.get(worker).expect("worker should exist").order(),
            Order::Gather(_)
        ),
        "workers must build Pump Jacks on oil instead of direct gather orders"
    );
}

#[test]
fn planner_backed_valid_queued_commands_emit_queue_full_notices() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let mover = entities
        .spawn_unit(1, EntityKind::Tank, cc_x + 16.0, cc_y)
        .expect("tank should spawn");
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, cc_x + 48.0, cc_y)
        .expect("rifleman should spawn");
    let gatherer = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 16.0, cc_y + 32.0)
        .expect("gather worker should spawn");
    let builder = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 48.0, cc_y + 32.0)
        .expect("build worker should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, cc_x + 96.0, cc_y)
        .expect("target should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, cc_x + 64.0, cc_y + 32.0)
        .expect("node should spawn");

    for id in [mover, attacker, gatherer, builder] {
        fill_queue(&mut entities, id);
    }

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut [player_state(1), player_state(2)],
        vec![
            (
                1,
                SimCommand::Move {
                    units: vec![mover],
                    x: 160.0,
                    y: 160.0,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::Attack {
                    units: vec![attacker],
                    target,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::Gather {
                    units: vec![gatherer],
                    node,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::Build {
                    units: vec![builder],
                    building: EntityKind::CityCentre,
                    tile_x: 10,
                    tile_y: 10,
                    queued: true,
                },
            ),
        ],
    );

    let notices = events.get(&1).map(Vec::as_slice).unwrap_or(&[]);
    assert_eq!(
        notices
            .iter()
            .filter(|event| matches!(
                event,
                Event::Notice { msg, .. } if msg == "Command queue full"
            ))
            .count(),
        4,
        "each valid queued command that only fails the queue cap should notify"
    );
    for id in [mover, attacker, gatherer, builder] {
        assert_eq!(entities.get(id).unwrap().queued_orders().len(), 8);
    }
}

#[test]
fn deconstruct_command_assigns_workers_only_to_tank_traps() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
        .expect("worker should spawn");
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 104.0, 100.0)
        .expect("rifleman should spawn");
    let trap = entities
        .spawn_building(2, EntityKind::TankTrap, 132.0, 100.0, true)
        .expect("tank trap should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Deconstruct {
                units: vec![rifle, worker],
                target: trap,
                queued: false,
            },
        )],
    );

    assert!(matches!(
        entities.get(worker).expect("worker should exist").order(),
        Order::Deconstruct(_)
    ));
    assert_eq!(
        entities
            .get(worker)
            .expect("worker should exist")
            .order()
            .deconstruct_target(),
        Some(trap)
    );
    assert!(matches!(
        entities.get(rifle).expect("rifleman should exist").order(),
        Order::Idle
    ));
}

#[test]
fn queued_deconstructs_distribute_across_selected_workers_by_build_queue_load() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let first = entities
        .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
        .expect("first worker should spawn");
    let second = entities
        .spawn_unit(1, EntityKind::Worker, 132.0, 100.0)
        .expect("second worker should spawn");
    let traps: Vec<u32> = (0..4)
        .map(|i| {
            entities
                .spawn_building(
                    2,
                    EntityKind::TankTrap,
                    160.0 + i as f32 * 32.0,
                    100.0,
                    true,
                )
                .expect("tank trap should spawn")
        })
        .collect();

    apply(
        &map,
        &mut entities,
        traps
            .iter()
            .map(|target| {
                (
                    1,
                    SimCommand::Deconstruct {
                        units: vec![first, second],
                        target: *target,
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
fn stop_clears_orders_and_hold_position_enters_hold_stance() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    {
        let entity = entities.get_mut(unit).expect("unit should exist");
        entity.set_order(Order::move_to(300.0, 300.0));
        entity.append_queued_order(OrderIntent::move_to(400.0, 400.0));
    }

    apply(
        &map,
        &mut entities,
        vec![(1, SimCommand::Stop { units: vec![unit] })],
    );

    let entity = entities.get(unit).expect("unit should exist");
    assert!(matches!(entity.order(), Order::Idle));
    assert!(entity.queued_orders().is_empty());

    {
        let entity = entities.get_mut(unit).expect("unit should exist");
        entity.set_order(Order::move_to(300.0, 300.0));
        entity.append_queued_order(OrderIntent::move_to(400.0, 400.0));
        entity.set_target_id(Some(99));
    }

    apply(
        &map,
        &mut entities,
        vec![(1, SimCommand::HoldPosition { units: vec![unit] })],
    );

    let entity = entities.get(unit).expect("unit should exist");
    assert!(matches!(entity.order(), Order::HoldPosition));
    assert!(entity.queued_orders().is_empty());
    assert_eq!(entity.target_id(), None);
    assert!(entity.path_is_empty());
}

#[test]
fn queued_move_ignores_stale_ids_and_invalid_coordinates() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![unit, 99_999],
                x: f32::NAN,
                y: 140.0,
                queued: true,
            },
        )],
    );

    assert!(
        entities
            .get(unit)
            .expect("unit should exist")
            .queued_orders()
            .is_empty(),
        "invalid queued point should be ignored without appending or panicking"
    );
}

#[test]
fn oversized_queued_unit_lists_are_deduped_and_capped_before_appending() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let owned = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("owned unit should spawn");
    let enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 130.0, 100.0)
        .expect("enemy unit should spawn");
    let mut units = vec![owned; 20_000];
    units.extend([99_999, enemy, owned]);

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units,
                x: 180.0,
                y: 180.0,
                queued: true,
            },
        )],
    );

    assert_eq!(
        entities
            .get(owned)
            .expect("owned unit should exist")
            .queued_orders()
            .len(),
        1,
        "repeated ids, stale ids, and enemy ids should not multiply queued state"
    );
    assert!(
        entities
            .get(enemy)
            .expect("enemy unit should exist")
            .queued_orders()
            .is_empty(),
        "enemy ids in a hostile queued command must be ignored"
    );
}

#[test]
fn queued_attack_and_gather_reject_dead_or_depleted_targets_before_appending() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
    entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 16.0, cc_y)
        .expect("worker should spawn");
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, cc_x + 32.0, cc_y)
        .expect("rifleman should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, cc_x + 96.0, cc_y)
        .expect("target should spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, cc_x + 64.0, cc_y)
        .expect("node should spawn");
    assert!(entities
        .get_mut(target)
        .expect("target should exist")
        .apply_damage(u32::MAX, None));
    if let Some(resource) = entities
        .get_mut(node)
        .expect("node should exist")
        .resource_node
        .as_mut()
    {
        resource.remaining = 0;
    }

    apply(
        &map,
        &mut entities,
        vec![
            (
                1,
                SimCommand::Attack {
                    units: vec![rifle],
                    target,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::Gather {
                    units: vec![worker],
                    node,
                    queued: true,
                },
            ),
        ],
    );

    assert!(
        entities
            .get(rifle)
            .expect("rifleman should exist")
            .queued_orders()
            .is_empty(),
        "dead attack targets should not create queued attack intents"
    );
    assert!(
        entities
            .get(worker)
            .expect("worker should exist")
            .queued_orders()
            .is_empty(),
        "depleted resources should not create queued gather intents"
    );
}

#[test]
fn attack_command_rejects_hidden_targets() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let hidden_target = entities
        .spawn_unit(2, EntityKind::Tank, 420.0, 100.0)
        .expect("hidden target should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Attack {
                units: vec![rifle],
                target: hidden_target,
                queued: false,
            },
        )],
    );

    let rifle = entities.get(rifle).expect("rifleman should exist");
    assert!(
        !matches!(rifle.order(), Order::Attack(_)),
        "hidden target ids should not become attack orders"
    );
    assert_eq!(rifle.target_id(), None);
    assert_eq!(rifle.path_goal(), None);
}

#[test]
fn attack_command_rejects_allied_targets() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let ally = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .expect("ally should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    players[0].team_id = 7;
    players[1].team_id = 7;

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Attack {
                units: vec![rifle],
                target: ally,
                queued: false,
            },
        )],
    );

    let rifle = entities.get(rifle).expect("rifleman should exist");
    assert!(
        !matches!(rifle.order(), Order::Attack(_)),
        "allied target ids should not become attack orders"
    );
    assert_eq!(rifle.target_id(), None);
    assert_eq!(rifle.path_goal(), None);
}

#[test]
fn attack_command_accepts_owned_targets() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let own_building = entities
        .spawn_building(1, EntityKind::Barracks, 132.0, 100.0, true)
        .expect("owned building should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Attack {
                units: vec![rifle],
                target: own_building,
                queued: false,
            },
        )],
    );

    let rifle = entities.get(rifle).expect("rifleman should exist");
    assert_eq!(
        rifle.order().attack_target(),
        Some(own_building),
        "explicit attack commands should allow owned unit/building targets"
    );
}
