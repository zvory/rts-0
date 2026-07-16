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
fn firing_reveal_reaction_gates_are_episode_target_and_weapon_scoped() {
    let mut tank = Entity::new_unit(1, EntityKind::Tank, 10.0, 20.0).expect("tank should spawn");
    let cannon = crate::rules::combat::WeaponKind::TankCannon;
    let coax = crate::rules::combat::WeaponKind::TankCoax;
    tank.set_weapon_cooldown(cannon, 5);
    let episode = |source_entity, started_at_tick| super::FiringRevealEpisode {
        viewer: 1,
        source_entity,
        started_at_tick,
    };

    assert!(
        !tank.weapon_firing_reveal_reaction_ready(cannon, 42, episode(42, 7), 10, 30),
        "a new reveal episode should start a reaction gate"
    );
    assert_eq!(
        tank.weapon_cooldown(cannon),
        5,
        "reaction time must not be mixed into the real weapon reload"
    );

    tank.set_target_id(Some(42));
    tank.set_target_id(None);
    tank.set_target_id(Some(42));
    assert!(
        !tank.weapon_firing_reveal_reaction_ready(cannon, 42, episode(42, 7), 44, 30),
        "transient target clears must retain the original gate deadline"
    );
    assert_eq!(
        tank.combat
            .as_ref()
            .and_then(|combat| combat.firing_reveal_reaction_gates.get(&cannon))
            .and_then(|gates| gates.get(&42))
            .map(|gate| gate.ready_at_tick),
        Some(45)
    );

    assert!(
        !tank.weapon_firing_reveal_reaction_ready(cannon, 43, episode(43, 8), 20, 30),
        "switching to another target should start that target's own gate"
    );
    assert!(
        tank.weapon_firing_reveal_reaction_ready(cannon, 42, episode(42, 7), 45, 30),
        "switching back in the same episode must reuse the original deadline"
    );
    assert!(
        !tank.weapon_firing_reveal_reaction_ready(cannon, 42, episode(42, 50), 50, 30),
        "a later reveal episode should charge a new reaction gate"
    );
    assert!(
        !tank.weapon_firing_reveal_reaction_ready(coax, 42, episode(42, 50), 50, 30),
        "cannon and coax reaction gates must remain independent"
    );
}

#[test]
fn firing_reveal_reaction_gates_evict_oldest_at_the_runtime_bound() {
    let mut tank = Entity::new_unit(1, EntityKind::Tank, 10.0, 20.0).expect("tank should spawn");
    let cannon = crate::rules::combat::WeaponKind::TankCannon;
    for target in 1..=65 {
        assert!(!tank.weapon_firing_reveal_reaction_ready(
            cannon,
            target,
            super::FiringRevealEpisode {
                viewer: 1,
                source_entity: target,
                started_at_tick: target,
            },
            target,
            30,
        ));
    }
    let gates = tank
        .combat
        .as_ref()
        .and_then(|combat| combat.firing_reveal_reaction_gates.get(&cannon))
        .expect("cannon gates should exist");
    assert_eq!(
        gates.len(),
        super::MAX_FIRING_REVEAL_REACTION_GATES_PER_WEAPON
    );
    assert!(!gates.contains_key(&1));
    assert!(gates.contains_key(&65));
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
fn entity_store_core_operations_keep_stale_ids_fallible() {
    let mut store = EntityStore::new();
    let worker = store
        .spawn_unit(1, EntityKind::Worker, 10.0, 20.0)
        .expect("worker should spawn");
    let rifleman = store
        .spawn_unit(2, EntityKind::Rifleman, 30.0, 40.0)
        .expect("rifleman should spawn");

    assert_eq!((worker, rifleman), (1, 2));
    assert!(store.contains(worker));
    assert_eq!(store.get(rifleman).map(|entity| entity.owner), Some(2));
    let rifleman_hp = store
        .get(rifleman)
        .map(|entity| entity.hp)
        .expect("rifleman should still exist");
    if let Some(entity) = store.get_mut(rifleman) {
        entity.hp = entity.hp.saturating_sub(1);
    }
    assert_eq!(
        store.get(rifleman).map(|entity| entity.hp),
        Some(rifleman_hp.saturating_sub(1))
    );

    assert_eq!(store.remove(worker).map(|entity| entity.id), Some(worker));
    assert!(!store.contains(worker));
    assert!(store.get(worker).is_none());
    assert!(store.get_mut(worker).is_none());
    assert!(store.remove(worker).is_none());
    assert!(store.get(u32::MAX).is_none());
}

#[test]
fn entity_store_iteration_and_checkpoint_restore_stay_ordered() {
    let mut store = EntityStore::new();
    let first = store
        .spawn_unit(1, EntityKind::Worker, 10.0, 20.0)
        .expect("worker should spawn");
    let removed = store
        .spawn_unit(1, EntityKind::Rifleman, 20.0, 20.0)
        .expect("rifleman should spawn");
    let last = store
        .spawn_node(EntityKind::Steel, 30.0, 20.0)
        .expect("steel should spawn");
    assert!(store.remove(removed).is_some());

    assert_eq!(store.ids(), vec![first, last]);
    assert_eq!(
        store.iter().map(|entity| entity.id).collect::<Vec<_>>(),
        vec![first, last]
    );

    let restored = EntityStore::from_checkpoint_entities(
        store.checkpoint_next_id(),
        store.checkpoint_entities(),
    );
    assert_eq!(restored.next_id_for_test(), 4);
    assert_eq!(restored.ids(), vec![first, last]);
    assert!(restored.get(removed).is_none());
}

#[test]
fn entity_store_serde_round_trip_and_default_semantics_stay_unchanged() {
    let mut store = EntityStore::new();
    store
        .spawn_unit(3, EntityKind::Tank, 50.0, 60.0)
        .expect("tank should spawn");
    store
        .spawn_building(3, EntityKind::Depot, 70.0, 80.0, true)
        .expect("depot should spawn");

    let encoded = serde_json::to_value(&store).expect("store should serialize");
    assert_eq!(encoded["next_id"], 3);
    assert!(encoded["map"].get("1").is_some());
    assert!(encoded["map"].get("2").is_some());
    let restored: EntityStore =
        serde_json::from_value(encoded.clone()).expect("store should deserialize");
    assert_eq!(
        serde_json::to_value(restored).expect("restored store should serialize"),
        encoded
    );

    let mut default_store = EntityStore::default();
    let default_id = default_store
        .spawn_node(EntityKind::Oil, 5.0, 6.0)
        .expect("oil should spawn");
    let mut new_store = EntityStore::new();
    let new_id = new_store
        .spawn_node(EntityKind::Oil, 5.0, 6.0)
        .expect("oil should spawn");
    assert_eq!(default_id, 0);
    assert_eq!(new_id, 1);
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
