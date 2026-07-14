use super::*;

#[test]
fn unpaid_manual_unit_waits_then_pays_and_starts() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::Barracks, 10, 10);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, x, y, true)
        .expect("barracks should spawn");
    entities
        .get_mut(barracks)
        .expect("barracks")
        .push_production(ProdItem {
            unit: EntityKind::Rifleman,
            progress: 0,
            total: 10,
            paid: false,
        });
    let mut players = vec![player(1)];
    players[0].set_supply_counts(0, 10);

    tick_production(&map, &mut entities, &mut players);
    let waiting = &entities.get(barracks).expect("barracks").prod_queue()[0];
    assert!(!waiting.paid);
    assert_eq!(waiting.progress, 0);
    assert_eq!(players[0].supply_used, 0);

    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    players[0].set_resources(cost.steel, cost.oil);
    tick_production(&map, &mut entities, &mut players);
    let started = &entities.get(barracks).expect("barracks").prod_queue()[0];
    assert!(started.paid);
    assert_eq!(started.progress, 1);
    assert_eq!(players[0].steel, 0);
    assert_eq!(
        players[0].supply_used,
        rules::economy::supply_cost(EntityKind::Rifleman)
    );
}

#[test]
fn unpaid_manual_research_waits_then_pays_and_starts() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::TrainingCentre, 10, 10);
    let training_centre = entities
        .spawn_building(1, EntityKind::TrainingCentre, x, y, true)
        .expect("training centre should spawn");
    entities
        .get_mut(training_centre)
        .expect("training centre")
        .push_research(ResearchItem {
            upgrade: UpgradeKind::Entrenchment,
            progress: 0,
            total: 10,
            paid: false,
        });
    let mut players = vec![player(1)];

    tick_production(&map, &mut entities, &mut players);
    let waiting = &entities
        .get(training_centre)
        .expect("training centre")
        .research_queue()[0];
    assert!(!waiting.paid);
    assert_eq!(waiting.progress, 0);

    let definition = upgrade::definition(UpgradeKind::Entrenchment);
    players[0].set_resources(definition.cost_steel, definition.cost_oil);
    tick_production(&map, &mut entities, &mut players);
    let started = &entities
        .get(training_centre)
        .expect("training centre")
        .research_queue()[0];
    assert!(started.paid);
    assert_eq!(started.progress, 1);
    assert_eq!((players[0].steel, players[0].oil), (0, 0));
}

#[test]
fn dead_producers_do_not_pay_waiting_unit_or_research_items() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (barracks_x, barracks_y) = footprint_center(&map, EntityKind::Barracks, 4, 4);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, barracks_x, barracks_y, true)
        .expect("barracks should spawn");
    let (research_x, research_y) = footprint_center(&map, EntityKind::TrainingCentre, 14, 14);
    let training_centre = entities
        .spawn_building(1, EntityKind::TrainingCentre, research_x, research_y, true)
        .expect("training centre should spawn");
    entities
        .get_mut(barracks)
        .expect("barracks")
        .push_production(ProdItem {
            unit: EntityKind::Rifleman,
            progress: 0,
            total: 10,
            paid: false,
        });
    entities
        .get_mut(training_centre)
        .expect("training centre")
        .push_research(ResearchItem {
            upgrade: UpgradeKind::Entrenchment,
            progress: 0,
            total: 10,
            paid: false,
        });
    let barracks_hp = entities.get(barracks).expect("barracks").max_hp;
    entities
        .get_mut(barracks)
        .expect("barracks")
        .apply_damage(barracks_hp, None);
    let training_centre_hp = entities
        .get(training_centre)
        .expect("training centre")
        .max_hp;
    entities
        .get_mut(training_centre)
        .expect("training centre")
        .apply_damage(training_centre_hp, None);

    let mut players = vec![player(1)];
    players[0].set_resources(1_000, 1_000);
    players[0].set_supply_counts(0, 10);
    tick_production(&map, &mut entities, &mut players);

    assert!(!entities.get(barracks).expect("barracks").prod_queue()[0].paid);
    assert!(
        !entities
            .get(training_centre)
            .expect("training centre")
            .research_queue()[0]
            .paid
    );
    assert_eq!((players[0].steel, players[0].oil), (1_000, 1_000));
    assert_eq!(players[0].supply_used, 0);
}

#[test]
fn standing_repeat_does_not_create_unpaid_queue_items() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::Barracks, 10, 10);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, x, y, true)
        .expect("barracks should spawn");
    entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::Rifleman), true);
    let mut players = vec![player(1)];
    players[0].set_supply_counts(0, 10);

    tick_production(&map, &mut entities, &mut players);
    assert!(entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());

    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    players[0].set_resources(cost.steel, cost.oil);
    tick_production(&map, &mut entities, &mut players);
    let queue = entities.get(barracks).expect("barracks").prod_queue();
    assert_eq!(queue.len(), 1);
    assert!(queue[0].paid);
}

#[test]
fn produced_mortars_start_with_autocast_after_research() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    spawn_building_training(
        &map,
        &mut entities,
        10,
        10,
        EntityKind::Steelworks,
        EntityKind::MortarTeam,
    );
    let mut player = player(1);
    player.upgrades.insert(UpgradeKind::MortarAutocast);
    let mut players = vec![player];

    tick_production(&map, &mut entities, &mut players);

    let mortar = entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::MortarTeam)
        .expect("produced mortar should exist");
    assert_eq!(mortar.autocast_enabled(AbilityKind::MortarFire), Some(true));
}
