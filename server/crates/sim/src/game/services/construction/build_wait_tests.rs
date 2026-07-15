use super::*;
use super::{test_flat_map as flat_map, test_player_state as player_state};

macro_rules! run_construction_tick {
    ($map:expr, $entities:expr, $players:expr, $events:expr) => {{
        let fog = Fog::new($map.size);
        let mut active_sites = BTreeSet::new();
        construction_system($map, $entities, $players, $events, &fog, &mut active_sites);
    }};
}

#[test]
fn arrived_build_waits_without_spawning_when_resources_missing() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, sx, sy)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::Depot, 4, 4));
    let mut players = vec![player_state(1)];
    players[0].set_resources(0, 0);
    let mut events = HashMap::new();

    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    let worker = entities.get(worker).expect("worker should survive");
    assert_eq!(worker.build_phase(), Some(BuildPhase::WaitingAtSite));
    assert!(
        worker.path_is_empty(),
        "worker should stand at the site while waiting for resources"
    );
    assert!(
        entities
            .iter()
            .all(|entity| entity.kind != EntityKind::Depot),
        "resource wait must not spawn a scaffold"
    );
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Not enough steel"
    ));

    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    assert_eq!(
        events.get(&1).map_or(0, Vec::len),
        1,
        "continuing to wait for the same shortage should not spam notices"
    );
}

#[test]
fn waiting_build_starts_when_resources_become_available() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, sx, sy)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::Depot, 4, 4));
    let mut players = vec![player_state(1)];
    players[0].set_resources(0, 0);
    let mut events = HashMap::new();

    run_construction_tick!(&map, &mut entities, &mut players, &mut events);
    let cost = rules::economy::resource_cost(EntityKind::Depot);
    players[0].set_resources(cost.steel, cost.oil);
    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    let scaffold = entities
        .iter()
        .find(|entity| entity.kind == EntityKind::Depot && entity.under_construction())
        .expect("waiting build should spawn a scaffold once resources are available");
    assert_eq!(
        entities
            .get(worker)
            .expect("worker should survive")
            .build_phase(),
        Some(BuildPhase::Constructing { site: scaffold.id })
    );
    assert_eq!(players[0].steel, 0);
    assert_eq!(players[0].oil, 0);
}

#[test]
fn arrived_pump_jack_waits_for_steel_and_charges_on_start() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (sx, sy) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    entities
        .spawn_node(EntityKind::Oil, sx, sy)
        .expect("oil node should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, sx, sy)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::PumpJack, 4, 4));
    let blocker = entities
        .spawn_unit(1, EntityKind::Worker, sx, sy)
        .expect("friendly blocker should spawn");
    let blocker_before = entities
        .get(blocker)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("friendly blocker should exist");
    let mut players = vec![player_state(1)];
    players[0].set_resources(49, 0);
    let mut events = HashMap::new();

    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    assert_eq!(
        entities
            .get(worker)
            .expect("worker should survive")
            .build_phase(),
        Some(BuildPhase::WaitingAtSite)
    );
    assert!(
        entities
            .iter()
            .all(|entity| entity.kind != EntityKind::PumpJack),
        "resource wait must not spawn a Pump Jack scaffold"
    );
    assert_eq!(
        entities
            .get(blocker)
            .map(|entity| (entity.pos_x, entity.pos_y)),
        Some(blocker_before),
        "a Pump Jack that cannot yet be afforded must not displace friendly units"
    );
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Not enough steel"
    ));

    let cost = rules::economy::resource_cost(EntityKind::PumpJack);
    assert_eq!((cost.steel, cost.oil), (50, 0));
    players[0].set_resources(cost.steel, cost.oil);
    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    let scaffold = entities
        .iter()
        .find(|entity| entity.kind == EntityKind::PumpJack && entity.under_construction())
        .expect("Pump Jack should spawn once steel is available");
    assert_eq!(
        entities
            .get(worker)
            .expect("worker should survive")
            .build_phase(),
        Some(BuildPhase::Constructing { site: scaffold.id })
    );
    assert_eq!(players[0].steel, 0);
    assert_eq!(players[0].oil, 0);
    assert_ne!(
        entities
            .get(blocker)
            .map(|entity| (entity.pos_x, entity.pos_y)),
        Some(blocker_before),
        "the friendly blocker should move once construction can actually start"
    );
}

#[test]
fn arrived_pump_jack_ejects_owned_and_allied_units_before_starting() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    entities
        .spawn_node(EntityKind::Oil, site_x, site_y)
        .expect("oil node should spawn");
    let builder = entities
        .spawn_unit(
            1,
            EntityKind::Worker,
            site_x + config::TILE_SIZE as f32,
            site_y,
        )
        .expect("builder should spawn");
    entities
        .get_mut(builder)
        .expect("builder should exist")
        .set_order(Order::build(EntityKind::PumpJack, 4, 4));
    let owned_worker = entities
        .spawn_unit(1, EntityKind::Worker, site_x, site_y)
        .expect("owned blocker should spawn");
    let allied_tank = entities
        .spawn_unit(2, EntityKind::Tank, site_x, site_y)
        .expect("allied blocker should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    players[0].team_id = 7;
    players[1].team_id = 7;
    let mut events = HashMap::new();

    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    assert!(entities
        .iter()
        .any(|entity| entity.kind == EntityKind::PumpJack && entity.under_construction()));
    let site_rect =
        crate::game::services::geometry::building_rect_for_footprint(EntityKind::PumpJack, 4, 4)
            .expect("Pump Jack rect");
    for unit in [owned_worker, allied_tank] {
        let entity = entities.get(unit).expect("friendly unit should survive");
        let body = crate::game::services::geometry::unit_body_for_entity(entity)
            .expect("friendly unit should have a ground body");
        assert!(
            !crate::game::services::geometry::unit_body_intersects_rect(body, site_rect),
            "friendly unit {} should be forced clear of the Pump Jack footprint",
            entity.id,
        );
    }
    assert!(events.get(&1).is_none_or(Vec::is_empty));
}

#[test]
fn arrived_pump_jack_does_not_eject_any_units_when_enemy_blocks_site() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::PumpJack, 4, 4);
    entities
        .spawn_node(EntityKind::Oil, site_x, site_y)
        .expect("oil node should spawn");
    let builder = entities
        .spawn_unit(
            1,
            EntityKind::Worker,
            site_x + config::TILE_SIZE as f32,
            site_y,
        )
        .expect("builder should spawn");
    entities
        .get_mut(builder)
        .expect("builder should exist")
        .set_order(Order::build(EntityKind::PumpJack, 4, 4));
    let enemy = entities
        .spawn_unit(2, EntityKind::Tank, site_x, site_y)
        .expect("enemy blocker should spawn");
    let friendly = entities
        .spawn_unit(1, EntityKind::Worker, site_x, site_y)
        .expect("friendly blocker should spawn");
    let before: Vec<_> = [enemy, friendly]
        .into_iter()
        .map(|id| {
            entities
                .get(id)
                .map(|entity| (entity.pos_x, entity.pos_y))
                .expect("blocker should exist")
        })
        .collect();
    let mut players = vec![player_state(1), player_state(2)];
    let mut events = HashMap::new();

    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    assert!(entities
        .iter()
        .all(|entity| entity.kind != EntityKind::PumpJack));
    for (index, id) in [enemy, friendly].into_iter().enumerate() {
        let entity = entities.get(id).expect("blocker should survive");
        assert_eq!(
            (entity.pos_x, entity.pos_y),
            before[index],
            "no unit should be displaced while an enemy still blocks construction"
        );
    }
    assert_eq!(
        entities
            .get(builder)
            .expect("builder should survive")
            .build_phase(),
        Some(BuildPhase::WaitingAtSite),
    );
}

#[test]
fn waiting_build_cancels_when_building_claims_footprint() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, sx, sy)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::Depot, 4, 4));
    let mut players = vec![player_state(1), player_state(2)];
    players[0].set_resources(0, 0);
    let mut events = HashMap::new();

    run_construction_tick!(&map, &mut entities, &mut players, &mut events);
    entities
        .spawn_building(2, EntityKind::Depot, sx, sy, true)
        .expect("competing building should spawn");
    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    assert!(matches!(
        entities.get(worker).expect("worker should survive").order(),
        Order::Idle
    ));
    assert!(events.get(&1).is_some_and(|events| events
        .iter()
        .any(|event| matches!(event, Event::Notice { msg, .. } if msg == "Cannot build there"))));
}

#[test]
fn unit_blocked_build_starts_when_blocker_clears_before_timeout() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, sx, sy)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::Depot, 4, 4));
    let blocker = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("blocker should spawn");
    let mut players = vec![player_state(1)];
    let mut events = HashMap::new();

    for _ in 0..10 {
        run_construction_tick!(&map, &mut entities, &mut players, &mut events);
    }
    assert_eq!(
        entities
            .get(worker)
            .expect("worker should survive")
            .build_phase(),
        Some(BuildPhase::WaitingAtSite)
    );
    entities.remove(blocker);
    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    let scaffold = entities
        .iter()
        .find(|entity| entity.kind == EntityKind::Depot && entity.under_construction())
        .expect("cleared blocker should allow construction to start");
    assert_eq!(
        entities
            .get(worker)
            .expect("worker should survive")
            .build_phase(),
        Some(BuildPhase::Constructing { site: scaffold.id })
    );
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "clearing before timeout should not emit a blocker failure notice"
    );
}

#[test]
fn unit_blocked_build_clears_after_three_second_timeout() {
    let map = flat_map(16);
    let mut entities = EntityStore::new();
    let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, sx, sy)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::Depot, 4, 4));
    entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("blocker should spawn");
    let mut players = vec![player_state(1)];
    let mut events = HashMap::new();
    let grace_ticks = config::TICK_HZ * 3;

    for _ in 0..grace_ticks.saturating_sub(1) {
        run_construction_tick!(&map, &mut entities, &mut players, &mut events);
        assert!(
            matches!(
                entities.get(worker).expect("worker should survive").order(),
                Order::Build(_)
            ),
            "unit-blocked build should remain active before the grace timeout"
        );
    }
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "unit-blocked build should stay quiet before timeout"
    );

    run_construction_tick!(&map, &mut entities, &mut players, &mut events);

    assert!(matches!(
        entities.get(worker).expect("worker should survive").order(),
        Order::Idle
    ));
    assert!(
        entities
            .iter()
            .all(|entity| entity.kind != EntityKind::Depot),
        "timed-out blocker should not spawn a scaffold"
    );
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Cannot build there"
    ));
}
