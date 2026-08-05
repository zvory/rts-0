use super::*;

#[test]
fn repeat_production_skips_unit_with_missing_building_requirement() {
    let (map, mut entities, barracks) =
        repeat_barracks([EntityKind::MachineGunner, EntityKind::Rifleman]);
    let mut players = vec![player(1)];
    players[0].set_resources(10_000, 10_000);

    tick_production(&map, &mut entities, &mut players);

    let producer = entities.get(barracks).expect("barracks");
    assert_eq!(producer.prod_queue()[0].unit, EntityKind::Rifleman);
    assert_eq!(
        producer
            .production
            .as_ref()
            .expect("production")
            .repeat_units,
        [EntityKind::MachineGunner, EntityKind::Rifleman],
        "the locked choice should stay enabled for when its requirement completes"
    );

    entities
        .get_mut(barracks)
        .expect("barracks")
        .remove_front_production();
    spawn_complete_building(&map, &mut entities, EntityKind::TrainingCentre, 16);
    tick_production(&map, &mut entities, &mut players);
    assert_eq!(
        entities.get(barracks).expect("barracks").prod_queue()[0].unit,
        EntityKind::MachineGunner
    );
}

#[test]
fn repeat_production_skips_unit_with_missing_upgrade_requirement() {
    let (map, mut entities, barracks) =
        repeat_barracks([EntityKind::Panzerfaust, EntityKind::Rifleman]);
    spawn_complete_building(&map, &mut entities, EntityKind::TrainingCentre, 16);
    let mut players = vec![player(1)];
    players[0].set_resources(10_000, 10_000);

    tick_production(&map, &mut entities, &mut players);
    assert_eq!(
        entities.get(barracks).expect("barracks").prod_queue()[0].unit,
        EntityKind::Rifleman
    );

    entities
        .get_mut(barracks)
        .expect("barracks")
        .remove_front_production();
    players[0].upgrades.insert(UpgradeKind::Panzerfausts);
    tick_production(&map, &mut entities, &mut players);
    assert_eq!(
        entities.get(barracks).expect("barracks").prod_queue()[0].unit,
        EntityKind::Panzerfaust
    );
}

#[test]
fn repeat_production_skips_stale_unit_incompatible_with_producer() {
    let (map, mut entities, barracks) = repeat_barracks([EntityKind::Tank, EntityKind::Rifleman]);
    spawn_complete_building(&map, &mut entities, EntityKind::Factory, 16);
    let mut players = vec![player(1)];
    players[0].upgrades.insert(UpgradeKind::TankUnlock);
    players[0].set_resources(10_000, 10_000);

    tick_production(&map, &mut entities, &mut players);

    let producer = entities.get(barracks).expect("barracks");
    assert_eq!(producer.prod_queue()[0].unit, EntityKind::Rifleman);
    assert_eq!(
        producer
            .production
            .as_ref()
            .expect("production")
            .repeat_units,
        [EntityKind::Tank, EntityKind::Rifleman],
        "stale intent should remain visible so it can be removed explicitly"
    );
}

fn repeat_barracks<const N: usize>(units: [EntityKind; N]) -> (Map, EntityStore, u32) {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let barracks = spawn_complete_building(&map, &mut entities, EntityKind::Barracks, 8);
    let producer = entities.get_mut(barracks).expect("barracks");
    for unit in units {
        producer.set_repeat_production(Some(unit), true);
    }
    (map, entities, barracks)
}

fn spawn_complete_building(
    map: &Map,
    entities: &mut EntityStore,
    kind: EntityKind,
    tile_x: u32,
) -> u32 {
    let (x, y) = footprint_center(map, kind, tile_x, 8);
    entities
        .spawn_building(1, kind, x, y, true)
        .expect("building should spawn")
}
