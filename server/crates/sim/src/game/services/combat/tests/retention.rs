use super::*;
use crate::game::entity::Entity;
use crate::game::fog::LingeringSightSource;
use crate::game::services::combat::acquisition::resolve_target as resolve_target_with_obstruction;

fn tank_with_retained_target(
    map: &Map,
    tank_pos: (f32, f32),
    retained_owner: u32,
    retained_pos: (f32, f32),
    fallback_pos: (f32, f32),
) -> (EntityStore, u32, u32, u32) {
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, tank_pos.0, tank_pos.1)
        .expect("tank should spawn");
    let retained = entities
        .spawn_unit(
            retained_owner,
            EntityKind::Worker,
            retained_pos.0,
            retained_pos.1,
        )
        .expect("retained target should spawn");
    let fallback = entities
        .spawn_unit(2, EntityKind::Worker, fallback_pos.0, fallback_pos.1)
        .expect("fallback target should spawn");
    entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_order(Order::move_to(
            map.tile_center(8, 4).0,
            map.tile_center(4, 4).1,
        ));
    entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_target_id(Some(retained));
    (entities, tank, retained, fallback)
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_friendly() {
    let map = open_map(8);
    let (entities, tank, _retained, fallback) =
        tank_with_retained_target(&map, (100.0, 100.0), 1, (150.0, 100.0), (120.0, 130.0));

    assert_eq!(
        resolve_test_target(&map, &entities, &default_team_relations(), tank, 192.0),
        Some(fallback)
    );
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_hidden() {
    let map = open_map(24);
    let tank_sight = config::unit_stats(EntityKind::Tank)
        .expect("tank should have stats")
        .sight_tiles;
    let (entities, tank, retained, fallback) = tank_with_retained_target(
        &map,
        (100.0, 100.0),
        2,
        (
            100.0 + (tank_sight + 1) as f32 * config::TILE_SIZE as f32,
            100.0,
        ),
        (130.0, 100.0),
    );
    let los = LineOfSight::new(&map);
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], &entities, &map);
    let retained_entity = entities
        .get(retained)
        .expect("retained target should exist");
    assert!(
        !fog.is_visible_world(1, retained_entity.pos_x, retained_entity.pos_y),
        "test setup requires the retained target to be hidden"
    );
    let smokes = SmokeCloudStore::new();
    let tank_entity = entities.get(tank).expect("tank should exist");

    let target = resolve_target(
        &map,
        &entities,
        &default_team_relations(),
        &spatial,
        &los,
        &fog,
        &smokes,
        tank,
        tank_entity.owner,
        tank_entity.pos_x,
        tank_entity.pos_y,
        512.0,
        combat_mode(tank_entity),
    );

    assert_eq!(target, Some(fallback));
}

#[test]
fn lingering_death_vision_feeds_auto_acquisition() {
    let map = open_map(32);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(4, 4);
    let target_pos = map.tile_center(7, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    let teams = default_team_relations();
    let spatial = SpatialIndex::build(&entities, map.size);
    let los = LineOfSight::new(&map);
    let smokes = SmokeCloudStore::new();
    let mut live_fog = Fog::new(map.size);
    live_fog.recompute(&[1, 2], &EntityStore::new(), &map);
    let source = LingeringSightSource::new(1, target_pos.0, target_pos.1, 2, 99)
        .expect("death vision source should be valid");
    live_fog.stamp_lingering_sources(&[source], &map, &entities);
    let blockers = ShotBlockerIndex::build(&map, &entities);

    assert!(live_fog.is_visible_world(1, target_pos.0, target_pos.1));
    assert_eq!(
        resolve_target_with_obstruction(
            &map,
            &entities,
            &blockers,
            &teams,
            &spatial,
            &los,
            &live_fog,
            &smokes,
            &|_, _| false,
            attacker,
            1,
            attacker_pos.0,
            attacker_pos.1,
            1_000.0,
            CombatMode::Aggressive,
            false,
            &|candidate| candidate == target,
        ),
        Some(target),
        "death vision is normal fog and should feed auto-acquisition"
    );
}

#[test]
fn cooling_down_unit_keeps_committed_target_instead_of_rescanning() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let committed = entities
        .spawn_unit(2, EntityKind::Worker, 180.0, 100.0)
        .expect("committed target should spawn");
    entities
        .spawn_unit(2, EntityKind::AntiTankGun, 140.0, 100.0)
        .expect("higher-priority target should spawn");
    if let Some(tank) = entities.get_mut(attacker) {
        tank.set_order(Order::HoldPosition);
        tank.set_target_id(Some(committed));
        tank.set_weapon_cooldown(combat_rules::WeaponKind::TankCannon, 10);
        tank.set_weapon_cooldown(combat_rules::WeaponKind::TankCoax, 10);
    }

    run_combat_tick(&mut entities);

    assert_eq!(
        entities.get(attacker).and_then(Entity::target_id),
        Some(committed),
        "reload ticks should revalidate the committed target without ranking alternatives"
    );
}

#[test]
fn ready_tank_keeps_acquired_target_while_turret_rotates() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let committed = entities
        .spawn_unit(2, EntityKind::Worker, 100.0, 180.0)
        .expect("initial target should spawn");
    if let Some(tank) = entities.get_mut(attacker) {
        tank.set_order(Order::HoldPosition);
        tank.set_facing(0.0);
        tank.set_weapon_facing(0.0);
        tank.set_weapon_cooldown(combat_rules::WeaponKind::TankCoax, 99);
    }

    run_combat_tick(&mut entities);
    assert_eq!(
        entities.get(attacker).and_then(Entity::target_id),
        Some(committed),
        "tank should acquire the only initial target"
    );
    assert_eq!(entities.get(attacker).map(Entity::attack_cd), Some(0));

    entities
        .spawn_unit(2, EntityKind::AntiTankGun, 150.0, 100.0)
        .expect("higher-priority target should spawn");
    run_combat_tick(&mut entities);

    assert_eq!(
        entities.get(attacker).and_then(Entity::target_id),
        Some(committed),
        "a ready tank should finish rotating toward its committed target before rescanning"
    );
}

#[test]
fn firing_immediately_acquires_the_next_target_for_reload_tracking() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let first = entities
        .spawn_unit(2, EntityKind::Worker, 130.0, 100.0)
        .expect("first target should spawn");
    let next = entities
        .spawn_unit(2, EntityKind::Worker, 100.0, 140.0)
        .expect("next target should spawn");
    if let Some(target) = entities.get_mut(first) {
        target.apply_damage(target.hp.saturating_sub(1), None);
    }

    run_combat_tick(&mut entities);

    let attacker = entities.get(attacker).expect("attacker should exist");
    assert!(
        attacker.attack_cd() > 0,
        "the first shot should begin reload"
    );
    assert_eq!(
        attacker.target_id(),
        Some(next),
        "the post-fire acquisition pass should commit the next target during reload"
    );
}

#[test]
fn explicit_attack_kill_keeps_fallback_target_through_reload() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let commanded = entities
        .spawn_unit(2, EntityKind::Worker, 130.0, 100.0)
        .expect("commanded target should spawn");
    let fallback = entities
        .spawn_unit(2, EntityKind::Worker, 100.0, 140.0)
        .expect("fallback target should spawn");
    if let Some(attacker) = entities.get_mut(attacker) {
        attacker.set_order(Order::attack(commanded));
    }
    if let Some(target) = entities.get_mut(commanded) {
        target.apply_damage(target.hp.saturating_sub(1), None);
    }

    run_combat_tick(&mut entities);
    assert_eq!(
        entities.get(attacker).and_then(Entity::target_id),
        Some(fallback),
        "killing the commanded target should prepare an automatic fallback"
    );

    run_combat_tick(&mut entities);
    assert_eq!(
        entities.get(attacker).and_then(Entity::target_id),
        Some(fallback),
        "the dead explicit order must not discard its prepared fallback during reload"
    );
}

#[test]
fn ready_weapon_fires_at_valid_reload_target_before_post_fire_retargeting() {
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let retained = entities
        .spawn_unit(2, EntityKind::Worker, 150.0, 100.0)
        .expect("reload target should spawn");
    let higher_priority = entities
        .spawn_unit(2, EntityKind::AntiTankGun, 100.0, 150.0)
        .expect("higher-priority target should spawn");
    if let Some(tank) = entities.get_mut(tank) {
        tank.set_order(Order::HoldPosition);
        tank.set_target_id(Some(retained));
        tank.set_facing(0.0);
        tank.set_weapon_facing(0.0);
        tank.set_weapon_cooldown(combat_rules::WeaponKind::TankCannon, 1);
        tank.set_weapon_cooldown(combat_rules::WeaponKind::TankCoax, 99);
    }
    let events = run_combat_tick(&mut entities);

    assert!(
        events.values().flatten().any(
            |event| matches!(event, Event::Attack { from, to, .. } if *from == tank && *to == retained)
        ),
        "the valid target prepared during reload should receive the ready shot"
    );
    assert!(
        events.values().flatten().all(
            |event| !matches!(event, Event::Attack { from, to, .. } if *from == tank && *to == higher_priority)
        ),
        "the ready tick must not switch to a newly preferred target before firing"
    );
}

#[test]
fn ready_weapon_retargets_when_reload_target_is_gone() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let stale = entities
        .spawn_unit(2, EntityKind::Worker, 130.0, 100.0)
        .expect("stale target should spawn");
    let fallback = entities
        .spawn_unit(2, EntityKind::Worker, 140.0, 100.0)
        .expect("fallback target should spawn");
    if let Some(attacker) = entities.get_mut(attacker) {
        attacker.set_target_id(Some(stale));
        attacker.set_attack_cd(1);
    }
    entities.remove(stale);
    let fallback_hp = entities.get(fallback).expect("fallback should exist").hp;

    run_combat_tick(&mut entities);

    assert!(
        entities.get(fallback).expect("fallback should survive").hp < fallback_hp,
        "an invalid reload target should trigger acquisition and fire on the ready tick"
    );
}

#[test]
fn ready_weapon_retargets_when_reload_target_is_out_of_range() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let out_of_range = entities
        .spawn_unit(2, EntityKind::Worker, 500.0, 100.0)
        .expect("prepared target should spawn");
    let fireable = entities
        .spawn_unit(2, EntityKind::Worker, 140.0, 100.0)
        .expect("fireable fallback should spawn");
    if let Some(attacker) = entities.get_mut(attacker) {
        attacker.set_order(Order::attack_move_to(600.0, 100.0));
        attacker.set_target_id(Some(out_of_range));
        attacker.set_attack_cd(1);
    }

    let events = run_combat_tick(&mut entities);

    assert!(
        events.values().flatten().any(
            |event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == fireable)
        ),
        "a ready weapon should replace an out-of-range reload target with a fireable target"
    );
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_smoke_covered() {
    let map = open_map(12);
    let (entities, tank, retained, fallback) =
        tank_with_retained_target(&map, (100.0, 100.0), 2, (150.0, 100.0), (120.0, 130.0));
    let mut smokes = SmokeCloudStore::new();
    let retained_entity = entities
        .get(retained)
        .expect("retained target should exist");
    smokes
        .spawn(retained_entity.pos_x, retained_entity.pos_y, 1.0, 100, 0)
        .expect("smoke should spawn");
    let los = LineOfSight::with_smoke(&map, &smokes);
    let spatial = SpatialIndex::build(&entities, map.size);
    let mut fog = Fog::new(map.size);
    fog.recompute_with_smoke(&[1, 2], &entities, &map, &smokes);
    let tank_entity = entities.get(tank).expect("tank should exist");

    let target = resolve_target(
        &map,
        &entities,
        &default_team_relations(),
        &spatial,
        &los,
        &fog,
        &smokes,
        tank,
        tank_entity.owner,
        tank_entity.pos_x,
        tank_entity.pos_y,
        192.0,
        combat_mode(tank_entity),
    );

    assert_eq!(target, Some(fallback));
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_not_fireable() {
    let map = map_with_rock_at((3, 4));
    let attacker_pos = map.tile_center(2, 4);
    let blocked_pos = map.tile_center(6, 4);
    let fallback_pos = map.tile_center(2, 5);
    let (entities, tank, retained, fallback) =
        tank_with_retained_target(&map, attacker_pos, 2, blocked_pos, fallback_pos);
    assert_eq!(entities.get(retained).map(|target| target.owner), Some(2));

    assert_eq!(
        resolve_test_target(&map, &entities, &default_team_relations(), tank, 512.0),
        Some(fallback)
    );
}

#[test]
fn shoot_while_moving_units_reacquire_when_retained_target_is_out_of_range() {
    for kind in [EntityKind::Tank, EntityKind::ScoutCar] {
        let map = open_map(20);
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        let stats = config::unit_stats(kind).expect("moving-fire unit should have stats");
        let profile = combat_rules::attack_profile(kind);
        let range_px =
            profile.range_tiles as f32 * config::TILE_SIZE as f32 + stats.radius + RANGE_SLACK;
        let retained_distance =
            (range_px + 4.0).min(stats.sight_tiles as f32 * config::TILE_SIZE as f32 - 1.0);
        assert!(
            retained_distance > range_px,
            "{kind:?} test setup needs a visible target just outside weapon range"
        );
        let retained = entities
            .spawn_unit(2, EntityKind::Worker, 100.0 + retained_distance, 100.0)
            .expect("retained target should spawn");
        let fallback = entities
            .spawn_unit(2, EntityKind::Worker, 130.0, 100.0)
            .expect("fallback target should spawn");
        if let Some(attacker) = entities.get_mut(attacker_id) {
            attacker.set_order(Order::move_to(500.0, 100.0));
            attacker.set_target_id(Some(retained));
        }

        let los = LineOfSight::new(&map);
        let spatial = SpatialIndex::build(&entities, map.size);
        let fog = visible_fog(&map, &entities);
        let retained_entity = entities
            .get(retained)
            .expect("retained target should exist");
        assert!(
            fog.is_visible_world(1, retained_entity.pos_x, retained_entity.pos_y),
            "{kind:?} retained target should be visible so range is the failing fireability gate"
        );
        let smokes = SmokeCloudStore::new();
        let attacker = entities
            .get(attacker_id)
            .expect("attacker should still exist");

        let target = resolve_target(
            &map,
            &entities,
            &default_team_relations(),
            &spatial,
            &los,
            &fog,
            &smokes,
            attacker_id,
            attacker.owner,
            attacker.pos_x,
            attacker.pos_y,
            512.0,
            combat_mode(attacker),
        );

        assert_eq!(target, Some(fallback), "{kind:?} should reacquire");
        assert_ne!(target, Some(retained));
    }
}
