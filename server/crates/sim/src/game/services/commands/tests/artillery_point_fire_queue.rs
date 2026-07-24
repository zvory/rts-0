use super::*;

#[test]
fn queued_artillery_point_fire_preserves_clicked_center_after_future_move() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let pos = (320.0, 320.0);
    let move_dest = (640.0, 320.0);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![artillery],
                x: move_dest.0,
                y: move_dest.1,
                queued: false,
            },
        )],
    );
    let future = entities
        .get(artillery)
        .expect("artillery should exist")
        .move_intent()
        .expect("move command should store an authoritative future destination");
    let raw_click = (future.0 + config::TILE_SIZE as f32 * 5.0, future.1);
    queue_point_fire(&map, &mut entities, artillery, raw_click);

    let point = queued_point_fire(&entities, artillery);
    assert!((point.x - raw_click.0).abs() < 0.001);
    assert!((point.y - raw_click.1).abs() < 0.001);
}

#[test]
fn queued_artillery_blanket_fire_preserves_center_after_future_move() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let pos = (320.0, 320.0);
    let move_dest = (640.0, 320.0);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![artillery],
                x: move_dest.0,
                y: move_dest.1,
                queued: false,
            },
        )],
    );
    let future = entities
        .get(artillery)
        .expect("artillery should exist")
        .move_intent()
        .expect("move command should store an authoritative future destination");
    let raw_click = (future.0 + config::TILE_SIZE as f32 * 5.0, future.1);
    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::BlanketFire,
                units: vec![artillery],
                x: Some(raw_click.0),
                y: Some(raw_click.1),
                queued: true,
            },
        )],
    );

    let unit = entities.get(artillery).expect("artillery should exist");
    let [OrderIntent::BlanketFire { point, .. }] = unit.queued_orders() else {
        panic!("queued Blanket Fire should be stored behind the move");
    };
    assert!((point.x - raw_click.0).abs() < 0.001);
    assert!((point.y - raw_click.1).abs() < 0.001);
}

#[test]
fn queued_artillery_point_fire_accepts_deployed_move_teardown() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let pos = (320.0, 320.0);
    let move_dest = (640.0, 320.0);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.set_weapon_setup(WeaponSetup::Deployed);
        unit.set_emplacement_facing(Some(0.0));
        unit.set_weapon_facing(0.0);
    }

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![artillery],
                x: move_dest.0,
                y: move_dest.1,
                queued: false,
            },
        )],
    );
    let unit = entities.get(artillery).expect("artillery should exist");
    assert!(matches!(
        unit.weapon_setup(),
        WeaponSetup::TearingDown { .. }
    ));
    let future = unit
        .move_intent()
        .expect("move command should store an authoritative future destination");
    let raw_click = (future.0 + config::TILE_SIZE as f32 * 5.0, future.1);
    queue_point_fire(&map, &mut entities, artillery, raw_click);

    let point = queued_point_fire(&entities, artillery);
    assert!((point.x - raw_click.0).abs() < 0.001);
    assert!((point.y - raw_click.1).abs() < 0.001);
}

#[test]
fn queued_artillery_point_fire_at_future_origin_preserves_clicked_center() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let pos = (320.0, 320.0);
    let move_dest = (640.0, 320.0);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![artillery],
                x: move_dest.0,
                y: move_dest.1,
                queued: false,
            },
        )],
    );
    let future = entities
        .get(artillery)
        .expect("artillery should exist")
        .move_intent()
        .expect("move command should store an authoritative future destination");
    let setup_face = (future.0, future.1 + config::TILE_SIZE as f32 * 10.0);
    apply(
        &map,
        &mut entities,
        vec![
            (
                1,
                SimCommand::SetupAntiTankGuns {
                    units: vec![artillery],
                    x: setup_face.0,
                    y: setup_face.1,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::PointFire,
                    units: vec![artillery],
                    x: Some(future.0),
                    y: Some(future.1),
                    queued: true,
                },
            ),
        ],
    );

    let unit = entities.get(artillery).expect("artillery should exist");
    let [OrderIntent::SetupAntiTankGuns(_), OrderIntent::PointFire(point)] = unit.queued_orders()
    else {
        panic!(
            "queued setup and point fire should be stored behind the move, got {:?}",
            unit.queued_orders()
        );
    };
    assert!((point.x - future.0).abs() < 0.001);
    assert!((point.y - future.1).abs() < 0.001);
}

fn queue_point_fire(map: &Map, entities: &mut EntityStore, artillery: u32, raw_click: (f32, f32)) {
    apply(
        map,
        entities,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::PointFire,
                units: vec![artillery],
                x: Some(raw_click.0),
                y: Some(raw_click.1),
                queued: true,
            },
        )],
    );
}

fn queued_point_fire(entities: &EntityStore, artillery: u32) -> crate::game::entity::PointIntent {
    let unit = entities.get(artillery).expect("artillery should exist");
    let [OrderIntent::PointFire(point)] = unit.queued_orders() else {
        panic!("queued point fire should be stored behind the move");
    };
    *point
}
