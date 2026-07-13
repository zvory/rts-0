use super::*;

#[test]
fn deployed_anti_tank_gun_does_not_auto_acquire_targets_hidden_by_fog() {
    let map = open_map(24);
    let mut entities = EntityStore::new();
    let anti_tank_sight = config::unit_stats(EntityKind::AntiTankGun)
        .expect("anti-tank gun should have stats")
        .sight_tiles;
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let tank_id = entities
        .spawn_unit(
            2,
            EntityKind::Tank,
            100.0 + (anti_tank_sight + 1) as f32 * config::TILE_SIZE as f32,
            100.0,
        )
        .expect("enemy tank should spawn");
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }
    entities
        .get_mut(tank_id)
        .expect("tank should exist")
        .set_facing(std::f32::consts::PI);

    let mut fog = Fog::new(map.size);
    fog.recompute(&[1], &entities, &map);
    assert!(
        !fog.is_visible_world(
            1,
            entities.get(tank_id).expect("tank should exist").pos_x,
            entities.get(tank_id).expect("tank should exist").pos_y,
        ),
        "test setup requires the tank to be outside the Anti-Tank Gun owner's sight"
    );
    let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities.get(tank_id).expect("enemy should exist").hp,
        enemy_hp,
        "deployed anti-tank guns must not fire at targets hidden by fog"
    );
    assert_eq!(
        entities
            .get(at_id)
            .expect("anti-tank gun should exist")
            .target_id(),
        None,
        "hidden targets should not be retained as combat targets"
    );
    assert!(
        events
            .values()
            .flatten()
            .all(|event| !matches!(event, Event::Attack { .. })),
        "hidden-target suppression should not emit attack tracers"
    );
}
