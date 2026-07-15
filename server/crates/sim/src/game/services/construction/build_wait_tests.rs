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
    assert!(
        scaffold.construction_cost_paid(),
        "a scaffold created by the economy-backed build flow should retain its refund receipt"
    );
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
