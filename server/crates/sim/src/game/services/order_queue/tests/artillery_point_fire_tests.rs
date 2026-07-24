use super::*;

#[test]
fn failed_artillery_reposition_drops_its_terminal_fire_intent() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let pos = map.tile_center(10, 10);
    let target = map.tile_center(50, 10);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.replace_active_order(Order::ability(
            AbilityKind::PointFire,
            target.0,
            target.1,
            pos.0,
            pos.1,
        ));
        unit.mark_move_phase(MovePhase::PathFailed);
        unit.append_queued_order(OrderIntent::point_fire(target.0, target.1));
    }

    promote(&map, &mut entities);

    let unit = entities.get(artillery).expect("artillery should exist");
    assert!(matches!(unit.order(), Order::Idle));
    assert!(
        unit.queued_orders().is_empty(),
        "a failed reposition must not recreate the same path request every tick"
    );
}

#[test]
fn close_edge_artillery_target_gets_an_in_range_staging_point() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let target = (8.0, map.world_size_px() * 0.5);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, 4.0, target.1)
        .expect("artillery should spawn");

    let staging = crate::game::services::ability_orders::staging_point(
        &map,
        &entities,
        artillery,
        AbilityKind::PointFire,
        target.0,
        target.1,
    )
    .expect("an in-map firing position should exist");

    let distance = (staging.0 - target.0).hypot(staging.1 - target.1);
    let min_range = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let max_range = config::ARTILLERY_MAX_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    assert!(distance >= min_range && distance <= max_range);
}

#[test]
fn queued_packed_artillery_point_fire_sets_up_on_promotion() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let pos = (320.0, 320.0);
    let target = (pos.0 + config::TILE_SIZE as f32 * 30.0, pos.1);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    entities
        .get_mut(artillery)
        .expect("artillery should exist")
        .append_queued_order(OrderIntent::point_fire(target.0, target.1));

    promote(&map, &mut entities);

    let unit = entities.get(artillery).expect("artillery should exist");
    assert!(matches!(unit.weapon_setup(), WeaponSetup::Packed));
    assert!(matches!(unit.order(), Order::ArtilleryPointFire(_)));
    assert!(
        unit.emplacement_facing().unwrap_or_default().abs() < 0.001,
        "queued packed point fire should set up toward the stored target"
    );
}

#[test]
fn queued_artillery_blanket_fire_outside_arc_redeploys_on_promotion() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let pos = (320.0, 320.0);
    let angle = config::ARTILLERY_FIELD_OF_FIRE_RAD;
    let distance = config::TILE_SIZE as f32 * 30.0;
    let target = (
        pos.0 + angle.cos() * distance,
        pos.1 + angle.sin() * distance,
    );
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    {
        let unit = entities.get_mut(artillery).expect("artillery should exist");
        unit.set_weapon_setup(WeaponSetup::Deployed);
        unit.set_emplacement_facing(Some(0.0));
        unit.set_weapon_facing(0.0);
        unit.append_queued_order(OrderIntent::blanket_fire(
            target.0,
            target.1,
            crate::config::ARTILLERY_BLANKET_RADIUS_TILES,
        ));
    }

    promote(&map, &mut entities);

    let unit = entities.get(artillery).expect("artillery should exist");
    assert!(matches!(
        unit.weapon_setup(),
        WeaponSetup::TearingDownToRedeploy { .. }
    ));
    assert!(matches!(unit.order(), Order::ArtilleryBlanketFire { .. }));
    assert!(
        (unit.pending_redeploy_facing().unwrap_or_default() - angle).abs() < 0.001,
        "queued outside-arc Blanket Fire should redeploy toward the stored center"
    );
    assert!(
        unit.emplacement_facing().unwrap_or_default().abs() < 0.001,
        "queued Blanket Fire must not walk the active field of fire before redeploy"
    );
}
