use super::*;

fn groups(
    movement: bool,
    combat: bool,
    production: bool,
    construction: bool,
    worker: bool,
    resource_node: bool,
    resource_extractor: bool,
    scout_plane: bool,
) -> EntityStateGroups {
    EntityStateGroups {
        movement,
        combat,
        production,
        construction,
        worker,
        resource_node,
        resource_extractor,
        scout_plane,
    }
}

#[test]
fn unit_kinds_have_exact_state_groups() {
    let cases = [
        (
            EntityKind::Worker,
            groups(true, true, false, false, true, false, false, false),
        ),
        (
            EntityKind::Rifleman,
            groups(true, true, false, false, false, false, false, false),
        ),
        (
            EntityKind::MachineGunner,
            groups(true, true, false, false, false, false, false, false),
        ),
        (
            EntityKind::AntiTankGun,
            groups(true, true, false, false, false, false, false, false),
        ),
        (
            EntityKind::Tank,
            groups(true, true, false, false, false, false, false, false),
        ),
        (
            EntityKind::ScoutPlane,
            groups(true, false, false, false, false, false, false, true),
        ),
    ];

    for (kind, expected) in cases {
        let entity = Entity::new_unit(1, kind, 10.0, 20.0).expect("unit kind should spawn");
        assert_eq!(entity.state_groups(), expected, "{kind:?}");
    }
}

#[test]
fn weapon_cooldowns_keep_default_attack_cd_shim_isolated() {
    let mut tank = Entity::new_unit(1, EntityKind::Tank, 10.0, 20.0).expect("tank should spawn");

    assert_eq!(tank.attack_cd(), 0);
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCoax),
        0,
        "missing weapon cooldown entries should read as ready"
    );

    tank.set_weapon_cooldown(crate::rules::combat::WeaponKind::TankCannon, 72);
    tank.set_weapon_cooldown(crate::rules::combat::WeaponKind::TankCoax, 6);
    assert_eq!(
        tank.attack_cd(),
        72,
        "legacy attack_cd shim should read the default tank cannon"
    );

    tank.tick_attack_cd();
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCannon),
        71
    );
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCoax),
        6,
        "legacy tick_attack_cd shim should only tick the default weapon"
    );

    tank.tick_weapon_cooldowns();
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCannon),
        70
    );
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCoax),
        5
    );

    tank.set_attack_cd(0);
    assert_eq!(tank.attack_cd(), 0);
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCoax),
        5,
        "legacy set_attack_cd shim should not clear non-default weapons"
    );
}

#[test]
fn firing_reveal_response_delay_is_tracked_per_weapon() {
    let mut tank = Entity::new_unit(1, EntityKind::Tank, 10.0, 20.0).expect("tank should spawn");

    assert!(tank.start_weapon_firing_reveal_response_delay(
        crate::rules::combat::WeaponKind::TankCannon,
        42,
        30
    ));
    assert!(
        !tank.start_weapon_firing_reveal_response_delay(
            crate::rules::combat::WeaponKind::TankCannon,
            42,
            30
        ),
        "same weapon should not pay the same revealed-target delay twice"
    );
    assert!(
        tank.start_weapon_firing_reveal_response_delay(
            crate::rules::combat::WeaponKind::TankCoax,
            42,
            30
        ),
        "a separate weapon must pay its own revealed-target delay"
    );
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCannon),
        30
    );
    assert_eq!(
        tank.weapon_cooldown(crate::rules::combat::WeaponKind::TankCoax),
        30
    );
}

#[test]
fn finished_building_kinds_have_exact_state_groups() {
    let cases = [
        (
            EntityKind::CityCentre,
            groups(false, false, true, false, false, false, false, false),
        ),
        (
            EntityKind::Depot,
            groups(false, false, false, false, false, false, false, false),
        ),
        (
            EntityKind::Barracks,
            groups(false, false, true, false, false, false, false, false),
        ),
        (
            EntityKind::TrainingCentre,
            groups(false, false, true, false, false, false, false, false),
        ),
        (
            EntityKind::ResearchComplex,
            groups(false, false, true, false, false, false, false, false),
        ),
        (
            EntityKind::Factory,
            groups(false, false, true, false, false, false, false, false),
        ),
        (
            EntityKind::Steelworks,
            groups(false, false, true, false, false, false, false, false),
        ),
        (
            EntityKind::PumpJack,
            groups(false, false, false, false, false, false, true, false),
        ),
    ];

    for (kind, expected) in cases {
        let entity =
            Entity::new_building(1, kind, 10.0, 20.0, true).expect("building kind should spawn");
        assert_eq!(entity.state_groups(), expected, "{kind:?}");
    }
}

#[test]
fn unfinished_buildings_add_construction_state_only() {
    let kinds = [
        EntityKind::CityCentre,
        EntityKind::Depot,
        EntityKind::Barracks,
        EntityKind::TrainingCentre,
        EntityKind::ResearchComplex,
        EntityKind::Factory,
        EntityKind::Steelworks,
        EntityKind::PumpJack,
    ];

    for kind in kinds {
        let finished =
            Entity::new_building(1, kind, 10.0, 20.0, true).expect("building kind should spawn");
        let unfinished =
            Entity::new_building(1, kind, 10.0, 20.0, false).expect("building kind should spawn");
        let mut expected = finished.state_groups();
        expected.construction = true;
        assert_eq!(unfinished.state_groups(), expected, "{kind:?}");
    }
}

#[test]
fn unfinished_buildings_start_at_ten_percent_hp() {
    let cases = [
        (EntityKind::Depot, 11),
        (EntityKind::Barracks, 17),
        (EntityKind::CityCentre, 60),
    ];

    for (kind, expected_start_hp) in cases {
        let entity =
            Entity::new_building(1, kind, 10.0, 20.0, false).expect("building kind should spawn");
        assert_eq!(entity.hp, expected_start_hp, "{kind:?}");
        assert!(entity.hp < entity.max_hp, "{kind:?}");
        assert!(entity.under_construction(), "{kind:?}");
    }
}

#[test]
fn construction_hp_scales_linearly_to_full_completion() {
    let mut entity =
        Entity::new_building(1, EntityKind::Depot, 10.0, 20.0, false).expect("depot should spawn");
    let total = entity
        .construction
        .as_ref()
        .expect("depot should be under construction")
        .total;

    assert_eq!(entity.hp, 11);
    assert!(entity.set_construction_progress(total / 2));
    assert_eq!(entity.hp, 60);
    assert!(entity.set_construction_progress(total.saturating_sub(1)));
    assert_eq!(entity.hp, 109);
    assert_eq!(entity.advance_construction(), Some(true));
    assert_eq!(entity.hp, entity.max_hp);
    assert!(!entity.under_construction());
}

#[test]
fn resource_node_kinds_have_exact_state_groups() {
    for kind in [EntityKind::Steel, EntityKind::Oil] {
        let entity = Entity::new_node(kind, 10.0, 20.0).expect("node kind should spawn");
        assert_eq!(
            entity.state_groups(),
            groups(false, false, false, false, false, true, false, false),
            "{kind:?}"
        );
    }
}

#[test]
fn entity_store_keeps_mutable_iteration_guardrails() {
    let source = include_str!("store.rs");

    for disallowed_signature in [
        concat!("pub fn ", "iter_mut"),
        concat!("pub(crate) fn ", "iter_mut"),
        concat!("pub(super) fn ", "iter_mut"),
    ] {
        assert!(
                !source.contains(disallowed_signature),
                "EntityStore must not expose raw mutable iteration; use ids() + get_mut(id) for outcome-affecting mutation"
            );
    }

    let lines: Vec<&str> = source.lines().collect();
    let raw_map_walks: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let trimmed = line.trim();
            let is_raw_mutable_map_walk = trimmed.contains(concat!("self.map.", "values_mut()"))
                || trimmed.contains(concat!("self.map.", "iter_mut()"));
            is_raw_mutable_map_walk.then_some((idx, trimmed))
        })
        .collect();

    assert!(
        !raw_map_walks.is_empty(),
        "guardrail should see at least the documented release_miner raw map walk"
    );

    for (idx, line) in raw_map_walks {
        let context_start = idx.saturating_sub(4);
        let preceding_context = lines[context_start..idx].join("\n");
        assert!(
                preceding_context.contains("Order-independent"),
                "raw mutable EntityStore map walk at line {} must document why unordered visitation cannot affect outcomes: {}",
                idx + 1,
                line
            );
    }
}

#[test]
fn order_state_machines_keep_intent_separate_from_execution() {
    let mut worker =
        Entity::new_unit(1, EntityKind::Worker, 10.0, 20.0).expect("worker should spawn");

    worker.set_order(Order::gather(42));
    assert_eq!(worker.order().gather_node(), Some(42));
    assert_eq!(worker.gather_phase(), Some(GatherPhase::ToNode));

    worker.mark_gather_phase(GatherPhase::Harvesting);
    assert_eq!(worker.order().gather_node(), Some(42));
    assert_eq!(worker.gather_phase(), Some(GatherPhase::Harvesting));
    assert_eq!(worker.tick_gather_harvest(), Some(1));
    assert_eq!(worker.tick_gather_harvest(), Some(2));

    worker.mark_gather_phase(GatherPhase::ToNode);
    assert_eq!(worker.order().gather_node(), Some(42));
    assert_eq!(worker.gather_phase(), Some(GatherPhase::ToNode));
    assert_eq!(worker.tick_gather_harvest(), None);

    worker.clear_orders();
    assert_eq!(worker.order(), Order::Idle);
    assert_eq!(worker.gather_phase(), None);
}

#[test]
fn node_slot_holder_requires_live_worker_harvesting_same_node() {
    let mut store = EntityStore::new();
    let worker = store.spawn_unit(1, EntityKind::Worker, 10.0, 20.0).unwrap();
    let other_worker = store.spawn_unit(1, EntityKind::Worker, 20.0, 20.0).unwrap();
    let node = store.spawn_node(EntityKind::Steel, 30.0, 20.0).unwrap();

    assert!(!store.claim_miner(node, worker));

    store
        .get_mut(worker)
        .unwrap()
        .set_order(Order::gather(node));
    assert!(!store.claim_miner(node, worker));

    store
        .get_mut(worker)
        .unwrap()
        .mark_gather_phase(GatherPhase::Harvesting);
    assert!(store.claim_miner(node, worker));
    assert_eq!(store.node_slot_holder(node), Some(worker));

    store
        .get_mut(other_worker)
        .unwrap()
        .set_order(Order::gather(node));
    store
        .get_mut(other_worker)
        .unwrap()
        .mark_gather_phase(GatherPhase::Harvesting);
    assert!(!store.claim_miner(node, other_worker));
    assert_eq!(store.node_slot_holder(node), Some(worker));

    store.get_mut(worker).unwrap().clear_orders();
    assert_eq!(store.node_slot_holder(node), None);
    store.clear_stale_miner_slots();
    assert_eq!(store.get(node).unwrap().miner(), None);
}

#[test]
fn release_miner_clears_slot_after_worker_order_changes() {
    let mut store = EntityStore::new();
    let worker = store.spawn_unit(1, EntityKind::Worker, 10.0, 20.0).unwrap();
    let node = store.spawn_node(EntityKind::Oil, 30.0, 20.0).unwrap();

    store
        .get_mut(worker)
        .unwrap()
        .set_order(Order::gather(node));
    store
        .get_mut(worker)
        .unwrap()
        .mark_gather_phase(GatherPhase::Harvesting);
    assert!(store.claim_miner(node, worker));

    store.get_mut(worker).unwrap().clear_orders();
    store.release_miner(worker);
    assert_eq!(store.get(node).unwrap().miner(), None);
}

#[test]
fn attack_and_build_orders_have_explicit_execution_phases() {
    let mut unit =
        Entity::new_unit(1, EntityKind::Rifleman, 10.0, 20.0).expect("unit should spawn");

    unit.set_order(Order::attack(99));
    assert_eq!(unit.order().attack_target(), Some(99));
    assert!(matches!(
        unit.order(),
        Order::Attack(AttackOrder {
            execution: AttackExecution {
                phase: AttackPhase::Chasing,
                ..
            },
            ..
        })
    ));
    unit.mark_attack_phase(AttackPhase::Firing);
    assert_eq!(unit.order().attack_target(), Some(99));
    assert!(matches!(
        unit.order(),
        Order::Attack(AttackOrder {
            execution: AttackExecution {
                phase: AttackPhase::Firing,
                ..
            },
            ..
        })
    ));

    let mut worker =
        Entity::new_unit(1, EntityKind::Worker, 10.0, 20.0).expect("worker should spawn");
    worker.set_order(Order::build(EntityKind::Depot, 4, 5));
    assert_eq!(worker.order().build_site(), None);
    assert_eq!(
        worker.order().build_intent_tile(),
        Some((EntityKind::Depot, 4, 5))
    );
    assert!(matches!(
        worker.order(),
        Order::Build(BuildOrder {
            execution: BuildExecution {
                phase: BuildPhase::ToSite,
                unit_blocked_ticks: 0,
                routing: FootprintRouting {
                    attempt: 0,
                    static_fingerprint: None,
                    start_tile: None,
                },
            },
            ..
        })
    ));
    worker.mark_build_phase(BuildPhase::Constructing { site: 7 });
    assert_eq!(worker.order().build_site(), Some(7));
    assert!(matches!(
        worker.order(),
        Order::Build(BuildOrder {
            execution: BuildExecution {
                phase: BuildPhase::Constructing { site: 7 },
                unit_blocked_ticks: 0,
                routing: FootprintRouting {
                    attempt: 0,
                    static_fingerprint: None,
                    start_tile: None,
                },
            },
            ..
        })
    ));
}

#[test]
fn build_wait_state_tracks_unit_block_ticks_and_resets() {
    let mut worker =
        Entity::new_unit(1, EntityKind::Worker, 10.0, 20.0).expect("worker should spawn");
    worker.set_order(Order::build(EntityKind::Depot, 4, 5));

    assert_eq!(worker.build_phase(), Some(BuildPhase::ToSite));
    assert_eq!(
        worker.update_build_unit_blocked(true),
        None,
        "unit-block ticks are only meaningful while waiting at the site"
    );

    worker.mark_build_phase(BuildPhase::WaitingAtSite);
    assert_eq!(worker.update_build_unit_blocked(true), Some(false));
    assert_eq!(worker.update_build_unit_blocked(true), Some(false));
    match worker.order() {
        Order::Build(order) => assert_eq!(order.execution.unit_blocked_ticks, 2),
        other => panic!("expected build order, got {other:?}"),
    }

    assert_eq!(worker.update_build_unit_blocked(false), Some(false));
    match worker.order() {
        Order::Build(order) => assert_eq!(order.execution.unit_blocked_ticks, 0),
        other => panic!("expected build order, got {other:?}"),
    }

    let mut grace_reached = false;
    for _ in 0..crate::config::TICK_HZ * 3 {
        grace_reached = worker
            .update_build_unit_blocked(true)
            .expect("waiting build order should count unit-blocked ticks");
    }
    assert!(grace_reached);
    match worker.order() {
        Order::Build(order) => assert_eq!(
            order.execution.unit_blocked_ticks,
            crate::config::TICK_HZ * 3
        ),
        other => panic!("expected build order, got {other:?}"),
    }

    worker.mark_build_phase(BuildPhase::Constructing { site: 7 });
    match worker.order() {
        Order::Build(order) => assert_eq!(order.execution.unit_blocked_ticks, 0),
        other => panic!("expected build order, got {other:?}"),
    }
    assert_eq!(worker.update_build_unit_blocked(true), None);
}
