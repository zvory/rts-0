use super::*;

#[test]
fn packed_anti_tank_gun_cannot_fire() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let tank_id = entities
        .spawn_unit(2, EntityKind::Tank, 220.0, 100.0)
        .expect("enemy tank should spawn");
    entities
        .get_mut(tank_id)
        .expect("tank should exist")
        .set_facing(std::f32::consts::PI);
    let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

    let events = run_combat_tick(&mut entities);

    assert_eq!(
        entities.get(tank_id).expect("enemy should exist").hp,
        enemy_hp,
        "packed anti-tank gun must finish setup before it can fire"
    );
    assert_eq!(
        entities
            .get(at_id)
            .expect("anti-tank gun should exist")
            .attack_cd(),
        0,
        "packed anti-tank gun must not consume its attack cooldown"
    );
    assert!(
        !events
            .values()
            .flatten()
            .any(|event| matches!(event, Event::Attack { from, .. } if *from == at_id)),
        "packed anti-tank gun must not emit an attack event"
    );
}

#[test]
fn anti_tank_gun_auto_acquisition_can_select_infantry() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let mut infantry_ids = Vec::new();
    for (index, kind) in [
        EntityKind::Worker,
        EntityKind::Golem,
        EntityKind::Rifleman,
        EntityKind::Panzerfaust,
        EntityKind::MachineGunner,
    ]
    .into_iter()
    .enumerate()
    {
        let id = entities
            .spawn_unit(2, kind, 132.0 + index as f32 * 20.0, 100.0)
            .expect("infantry target should spawn");
        infantry_ids.push(id);
    }

    assert_eq!(
        resolve_test_target(
            &map,
            &entities,
            &default_team_relations(),
            anti_tank_gun,
            256.0,
        ),
        Some(infantry_ids[2]),
        "anti-tank guns should auto-acquire the nearest non-economy infantry target"
    );

    let mortar = entities
        .spawn_unit(2, EntityKind::MortarTeam, 244.0, 100.0)
        .expect("mortar team should spawn");
    assert_eq!(
        resolve_test_target(
            &map,
            &entities,
            &default_team_relations(),
            anti_tank_gun,
            256.0,
        ),
        Some(infantry_ids[2]),
        "a farther equal-fit support weapon should not displace the nearer infantry target"
    );
    assert_ne!(mortar, infantry_ids[2]);
}

#[test]
fn deployed_anti_tank_gun_fires_reduced_damage_at_infantry() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, 180.0, 100.0)
        .expect("rifleman should spawn");
    if let Some(at) = entities.get_mut(anti_tank_gun) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    let before_hp = entities.get(rifleman).expect("rifleman should exist").hp;
    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities.get(rifleman).expect("rifleman should exist").hp,
        before_hp.saturating_sub(30),
        "a deployed anti-tank gun should reliably deal 30 damage to its intended infantry target"
    );
    assert!(events.values().flatten().any(|event| {
        matches!(
            event,
            Event::Attack { from, to, .. } if *from == anti_tank_gun && *to == rifleman
        )
    }));
}

#[test]
fn deployed_anti_tank_gun_survives_a_head_on_rifleman_duel() {
    let map = open_map(16);
    let players = [player_state(1, false), player_state(2, false)];
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, 180.0, 100.0)
        .expect("rifleman should spawn");
    if let Some(at) = entities.get_mut(anti_tank_gun) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    for _ in 0..=108 {
        run_combat_tick_on_map(&mut entities, &players, &map);
    }

    assert_eq!(
        entities.get(rifleman).expect("rifleman should exist").hp,
        0,
        "two reduced anti-tank-gun hits should kill one full-health rifleman"
    );
    assert!(
        entities
            .get(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .hp
            > 0,
        "a ready, deployed anti-tank gun should survive one head-on rifleman"
    );
}

#[test]
fn deployed_anti_tank_gun_auto_acquisition_skips_out_of_arc_priority_target() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let out_of_arc_anti_tank_gun = entities
        .spawn_unit(2, EntityKind::AntiTankGun, 300.0, 166.0)
        .expect("enemy anti-tank gun should spawn");
    let tank = entities
        .spawn_unit(2, EntityKind::Tank, 310.0, 100.0)
        .expect("enemy tank should spawn");
    if let Some(at) = entities.get_mut(anti_tank_gun) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities
            .get(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .target_id(),
        Some(tank),
        "idle deployed AT should ignore higher-priority targets outside its fixed field"
    );
    assert!(
        events.values().flatten().any(|event| {
            matches!(
                event,
                Event::Attack { from, to, .. } if *from == anti_tank_gun && *to == tank
            )
        }),
        "in-arc fallback target should produce anti-tank attack feedback"
    );
    assert_ne!(
        entities
            .get(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .target_id(),
        Some(out_of_arc_anti_tank_gun)
    );
}

#[test]
fn deployed_anti_tank_gun_does_not_acquire_when_only_target_is_outside_fixed_arc() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    entities
        .spawn_unit(2, EntityKind::Tank, 100.0, 300.0)
        .expect("enemy tank should spawn");
    if let Some(at) = entities.get_mut(anti_tank_gun) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities
            .get(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .target_id(),
        None,
        "a fixed deployed gun must not acquire targets it cannot engage"
    );
}

#[test]
fn deployed_anti_tank_gun_drops_retained_target_that_leaves_fixed_arc() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let outside_target = entities
        .spawn_unit(2, EntityKind::Tank, 275.0, 200.0)
        .expect("outside target should spawn");
    let inside_target = entities
        .spawn_unit(2, EntityKind::Tank, 300.0, 100.0)
        .expect("inside target should spawn");
    if let Some(at) = entities.get_mut(anti_tank_gun) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
        at.set_target_id(Some(outside_target));
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities
            .get(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .target_id(),
        Some(inside_target),
        "automatic retention must release a target that leaves the fixed field"
    );
}

#[test]
fn deployed_anti_tank_gun_drops_commanded_target_that_leaves_fixed_arc() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let outside_target = entities
        .spawn_unit(2, EntityKind::Tank, 275.0, 200.0)
        .expect("outside target should spawn");
    let inside_target = entities
        .spawn_unit(2, EntityKind::Tank, 300.0, 100.0)
        .expect("inside target should spawn");
    if let Some(at) = entities.get_mut(anti_tank_gun) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
        at.set_order(Order::attack(outside_target));
        at.set_target_id(Some(outside_target));
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities
            .get(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .target_id(),
        Some(inside_target),
        "a commanded target that leaves the fixed field must not lock the gun forever"
    );
}
