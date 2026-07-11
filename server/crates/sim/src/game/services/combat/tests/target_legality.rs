use super::super::acquisition::{
    direct_fire_target_legal, DirectFireLegality, DirectFireVisibility,
};
use super::super::activation::{
    secondary_weapon_target_passes_activation, SecondaryWeaponActivationConstraints,
};
use super::*;

fn direct_fire_legal(
    map: &Map,
    entities: &EntityStore,
    smokes: &SmokeCloudStore,
    attacker: u32,
    target: u32,
    legality: DirectFireLegality,
) -> bool {
    let teams = default_team_relations();
    let fog = visible_fog(map, entities);
    let los = LineOfSight::with_smoke(map, smokes);
    let attacker_entity = entities.get(attacker).expect("attacker should exist");
    direct_fire_target_legal(
        map,
        entities,
        &teams,
        &los,
        &fog,
        smokes,
        attacker,
        attacker_entity.owner,
        (attacker_entity.pos_x, attacker_entity.pos_y),
        target,
        legality,
    )
}

fn secondary_weapon_activation_legal(
    map: &Map,
    entities: &EntityStore,
    smokes: &SmokeCloudStore,
    attacker: u32,
    target: u32,
    constraints: SecondaryWeaponActivationConstraints,
) -> bool {
    let teams = default_team_relations();
    let fog = visible_fog(map, entities);
    let los = LineOfSight::with_smoke(map, smokes);
    let attacker_entity = entities.get(attacker).expect("attacker should exist");
    secondary_weapon_target_passes_activation(
        map,
        entities,
        &teams,
        &los,
        &fog,
        smokes,
        attacker,
        attacker_entity.owner,
        (attacker_entity.pos_x, attacker_entity.pos_y),
        target,
        constraints,
    )
}

#[test]
fn ordered_attack_can_damage_owned_targets() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let target_pos = map.tile_center(4, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let target = entities
        .spawn_building(1, EntityKind::Barracks, target_pos.0, target_pos.1, true)
        .expect("owned target should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(target));
    let target_hp = entities.get(target).expect("target should exist").hp;

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert!(
        entities.get(target).expect("target should exist").hp < target_hp,
        "ordered self-attack should damage the owned target"
    );
    assert!(
        events
            .get(&1)
            .expect("owner events should exist")
            .iter()
            .any(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == target)),
        "owner should receive attack feedback for explicit self-attacks"
    );
}

#[test]
fn direct_fire_legality_rejects_resource_nodes() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let target_pos = map.tile_center(4, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let resource = entities
        .spawn_node(EntityKind::Steel, target_pos.0, target_pos.1)
        .expect("resource should spawn");
    let smokes = SmokeCloudStore::new();

    assert!(!direct_fire_legal(
        &map,
        &entities,
        &smokes,
        attacker,
        resource,
        DirectFireLegality::auto_acquire(),
    ));
}

#[test]
fn direct_fire_legality_rejects_smoke_at_attacker_or_target() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let target_pos = map.tile_center(4, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");

    let mut attacker_smoke = SmokeCloudStore::new();
    attacker_smoke
        .spawn(attacker_pos.0, attacker_pos.1, 1.0, 100, 0)
        .expect("attacker smoke should spawn");
    assert!(!direct_fire_legal(
        &map,
        &entities,
        &attacker_smoke,
        attacker,
        target,
        DirectFireLegality::auto_acquire(),
    ));

    let mut target_smoke = SmokeCloudStore::new();
    target_smoke
        .spawn(target_pos.0, target_pos.1, 1.0, 100, 0)
        .expect("target smoke should spawn");
    assert!(!direct_fire_legal(
        &map,
        &entities,
        &target_smoke,
        attacker,
        target,
        DirectFireLegality::auto_acquire(),
    ));
}

#[test]
fn direct_fire_legality_rejects_terrain_los_blocking() {
    let map = map_with_rock_at((4, 4));
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let target_pos = map.tile_center(6, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    let smokes = SmokeCloudStore::new();

    assert!(!direct_fire_legal(
        &map,
        &entities,
        &smokes,
        attacker,
        target,
        DirectFireLegality::auto_acquire(),
    ));
}

#[test]
fn direct_fire_legality_rejects_friendly_hard_blockers() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let blocker_pos = map.tile_center(4, 4);
    let target_pos = map.tile_center(6, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    entities
        .spawn_unit(1, EntityKind::Tank, blocker_pos.0, blocker_pos.1)
        .expect("friendly blocker should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    let smokes = SmokeCloudStore::new();

    assert!(!direct_fire_legal(
        &map,
        &entities,
        &smokes,
        attacker,
        target,
        DirectFireLegality::auto_acquire(),
    ));
}

#[test]
fn intended_target_mode_rejects_enemy_hard_blockers_before_target() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let blocker_pos = map.tile_center(4, 4);
    let target_pos = map.tile_center(6, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    entities
        .spawn_unit(2, EntityKind::Tank, blocker_pos.0, blocker_pos.1)
        .expect("enemy blocker should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Worker, target_pos.0, target_pos.1)
        .expect("target should spawn");
    let smokes = SmokeCloudStore::new();

    assert!(
        direct_fire_legal(
            &map,
            &entities,
            &smokes,
            attacker,
            target,
            DirectFireLegality::auto_acquire(),
        ),
        "current auto-acquisition legality should still allow a shot that resolves to the blocker",
    );
    assert!(!direct_fire_legal(
        &map,
        &entities,
        &smokes,
        attacker,
        target,
        DirectFireLegality::intended_target(DirectFireVisibility::Owner),
    ));
}

#[test]
fn intended_target_mode_keeps_tank_traps_and_pump_jacks_non_blocking() {
    for blocker_kind in [EntityKind::TankTrap, EntityKind::PumpJack] {
        let map = open_map(12);
        let mut entities = EntityStore::new();
        let attacker_pos = map.tile_center(2, 4);
        let blocker_pos = map.tile_center(4, 4);
        let target_pos = map.tile_center(6, 4);
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, attacker_pos.0, attacker_pos.1)
            .expect("attacker should spawn");
        entities
            .spawn_building(2, blocker_kind, blocker_pos.0, blocker_pos.1, true)
            .expect("non-blocking building should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Worker, target_pos.0, target_pos.1)
            .expect("target should spawn");
        let smokes = SmokeCloudStore::new();

        assert!(
            direct_fire_legal(
                &map,
                &entities,
                &smokes,
                attacker,
                target,
                DirectFireLegality::intended_target(DirectFireVisibility::Owner),
            ),
            "{blocker_kind:?} should not block intended direct fire",
        );
    }
}

#[test]
fn secondary_weapon_activation_requires_range_and_turret_arc() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let in_arc_pos = map.tile_center(4, 4);
    let out_of_arc_pos = map.tile_center(2, 6);
    let out_of_range_pos = map.tile_center(8, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let in_arc = entities
        .spawn_unit(2, EntityKind::Worker, in_arc_pos.0, in_arc_pos.1)
        .expect("in-arc target should spawn");
    let out_of_arc = entities
        .spawn_unit(2, EntityKind::Worker, out_of_arc_pos.0, out_of_arc_pos.1)
        .expect("out-of-arc target should spawn");
    let out_of_range = entities
        .spawn_unit(
            2,
            EntityKind::Worker,
            out_of_range_pos.0,
            out_of_range_pos.1,
        )
        .expect("out-of-range target should spawn");
    let smokes = SmokeCloudStore::new();
    let constraints = SecondaryWeaponActivationConstraints {
        facing_rad: 0.0,
        half_arc_rad: std::f32::consts::FRAC_PI_4,
        range_px: config::TILE_SIZE as f32 * 3.0,
        direct_fire_legality: DirectFireLegality::intended_target(DirectFireVisibility::Owner),
    };

    assert!(secondary_weapon_activation_legal(
        &map,
        &entities,
        &smokes,
        attacker,
        in_arc,
        constraints,
    ));
    assert!(!secondary_weapon_activation_legal(
        &map,
        &entities,
        &smokes,
        attacker,
        out_of_arc,
        constraints,
    ));
    assert!(!secondary_weapon_activation_legal(
        &map,
        &entities,
        &smokes,
        attacker,
        out_of_range,
        constraints,
    ));
}

#[test]
fn secondary_weapon_activation_requires_intended_direct_fire_hit() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let attacker_pos = map.tile_center(2, 4);
    let blocker_pos = map.tile_center(4, 4);
    let target_pos = map.tile_center(6, 4);
    let attacker = entities
        .spawn_unit(1, EntityKind::Tank, attacker_pos.0, attacker_pos.1)
        .expect("attacker should spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::Tank, blocker_pos.0, blocker_pos.1)
        .expect("blocker should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Worker, target_pos.0, target_pos.1)
        .expect("target should spawn");
    let smokes = SmokeCloudStore::new();
    let constraints = SecondaryWeaponActivationConstraints {
        facing_rad: 0.0,
        half_arc_rad: std::f32::consts::FRAC_PI_4,
        range_px: config::TILE_SIZE as f32 * 6.0,
        direct_fire_legality: DirectFireLegality::intended_target(DirectFireVisibility::Owner),
    };

    assert!(
        secondary_weapon_activation_legal(&map, &entities, &smokes, attacker, blocker, constraints,),
        "the first enemy hard blocker is a legal intended target"
    );
    assert!(
        !secondary_weapon_activation_legal(&map, &entities, &smokes, attacker, target, constraints,),
        "secondary weapons must reject an intended target when the shot would hit a blocker first"
    );
}
