use super::*;

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
