use super::*;
use crate::game::entity::EntityKind;
use crate::rng::PortableRng;
use crate::rules::combat::{weapon_profile, WeaponKind};
use rand::SeedableRng;

#[test]
fn direct_and_ray_weapons_share_reveal_attribution_alert_and_reload_effects() {
    for (kind, weapon) in [
        (EntityKind::Rifleman, WeaponKind::RiflemanRifle),
        (EntityKind::MachineGunner, WeaponKind::MachineGunnerMg),
        (EntityKind::ScoutCar, WeaponKind::ScoutCarMg),
        (EntityKind::Tank, WeaponKind::TankCoax),
    ] {
        let mut map = Map {
            width: 16,
            height: 16,
            terrain: vec![crate::protocol::terrain::GRASS; 256],
            ..Default::default()
        };
        map.concealment_tiles = vec![(3, 3)];
        let mut entities = EntityStore::new();
        let attacker = entities.spawn_unit(1, kind, 100.0, 100.0).unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 180.0, 100.0)
            .unwrap();
        let hp = entities.get(target).unwrap().hp;
        let teams = TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 2), (4, 4)]);
        let mut fog = Fog::new(16, 16);
        fog.recompute(&[1, 2, 3, 4], &entities, &map);
        let spatial = SpatialIndex::build(&entities, 16, 16);
        let blockers = ShotBlockerIndex::build(&map, &entities);
        let smokes = SmokeCloudStore::new();
        let mut rng = PortableRng::seed_from_u64(1);
        let mut events = HashMap::from([(1, vec![]), (2, vec![]), (3, vec![]), (4, vec![])]);
        let mut reveals = vec![];
        let weapon = weapon_profile(weapon).unwrap();
        assert!(try_fire(
            &map,
            &mut entities,
            &blockers,
            &teams,
            &spatial,
            &LineOfSight::new(&map),
            &fog,
            &smokes,
            &mut crate::game::disciple_shot::DiscipleShotStore::default(),
            &mut rng,
            &mut events,
            &mut reveals,
            attacker,
            target,
            weapon,
            210.0,
            weapon.dmg,
            weapon.cooldown,
            10,
        ));
        let victim = entities.get(target).unwrap();
        assert!(victim.hp < hp, "{kind:?}");
        assert_eq!(victim.last_damage_owner(), Some(1), "{kind:?}");
        assert_eq!(victim.last_damage_entity(), Some(attacker), "{kind:?}");
        assert!(
            events[&2].iter().any(
                |event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack")
            ),
            "{kind:?}"
        );
        let mut viewers: Vec<_> = reveals.iter().map(|source| source.viewer()).collect();
        viewers.sort_unstable();
        assert_eq!(viewers, vec![2, 3], "{kind:?}: reveal only to victim team");
        assert!(reveals.iter().all(|source| source.entity_id() == attacker
            && source.is_active_at(204)
            && !source.is_active_at(205)));
        {
            let actual = entities.get(attacker).unwrap().weapon_cooldown(weapon.id);
            let base: u32 = weapon.cooldown;
            assert!(
                (base.saturating_sub(1)..=base.saturating_add(2)).contains(&actual),
                "reload {actual} outside jitter range around {base}"
            );
        };
        let remaining_hp = victim.hp;
        let counts: Vec<_> = events
            .iter()
            .map(|(pid, output)| (*pid, output.len()))
            .collect();
        assert!(!try_fire(
            &map,
            &mut entities,
            &blockers,
            &teams,
            &spatial,
            &LineOfSight::new(&map),
            &fog,
            &smokes,
            &mut crate::game::disciple_shot::DiscipleShotStore::default(),
            &mut rng,
            &mut events,
            &mut reveals,
            attacker,
            target,
            weapon,
            210.0,
            weapon.dmg,
            weapon.cooldown,
            11,
        ));
        assert_eq!(entities.get(target).unwrap().hp, remaining_hp);
        assert!(counts
            .iter()
            .all(|(pid, count)| events[pid].len() == *count));
    }
}

#[test]
fn magic_orbs_land_after_release_and_distance_based_flight_for_both_ranks() {
    for master in [false, true] {
        for distance in [20.0, 130.0, 260.0] {
            for double_damage in [false, true] {
                let map = crate::game::map::Map {
                    width: 32,
                    height: 32,
                    terrain: vec![crate::protocol::terrain::GRASS; 1024],
                    ..Default::default()
                };
                let mut entities = EntityStore::new();
                let attacker = entities
                    .spawn_unit(1, EntityKind::MagicAdept, 100.0, 100.0)
                    .unwrap();
                let target = entities
                    .spawn_unit(2, EntityKind::Tank, 100.0 + distance, 100.0)
                    .unwrap();
                let caster = entities.get_mut(attacker).unwrap();
                if master {
                    caster.insight = crate::config::DISCIPLE_EVOLVED_INSIGHT_CAP;
                    caster.advance_dao_master();
                }
                if double_damage {
                    caster.receive_shop_item(crate::game::ability::AbilityKind::DoubleDamage);
                }
                let profile = weapons::effective_attack_profile(entities.get(attacker).unwrap());
                let weapon = profile.weapon.unwrap();
                let hp = entities.get(target).unwrap().hp;
                let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
                let mut fog = Fog::new(32, 32);
                fog.recompute(&[1, 2], &entities, &map);
                let spatial = SpatialIndex::build(&entities, 32, 32);
                let blockers = ShotBlockerIndex::build(&map, &entities);
                let smokes = SmokeCloudStore::new();
                let mut rng = PortableRng::seed_from_u64(1);
                let mut events = HashMap::from([(1, vec![]), (2, vec![])]);
                let mut reveals = vec![];
                let mut shots = crate::game::disciple_shot::DiscipleShotStore::default();
                assert!(try_fire(
                    &map,
                    &mut entities,
                    &blockers,
                    &teams,
                    &spatial,
                    &LineOfSight::new(&map),
                    &fog,
                    &smokes,
                    &mut shots,
                    &mut rng,
                    &mut events,
                    &mut reveals,
                    attacker,
                    target,
                    &weapon,
                    300.0,
                    profile.dmg,
                    profile.cooldown,
                    10
                ));
                assert_eq!(entities.get(target).unwrap().hp, hp, "no damage at launch");
                assert_eq!(shots.shots.len(), 1);
                let due = shots.shots[0].impact_tick;
                let expected_delay = match distance as u32 {
                    20 => 5,
                    130 => 9,
                    260 => 15,
                    _ => unreachable!(),
                };
                assert_eq!(due, 10 + expected_delay);
                assert_eq!(
                    events[&1]
                        .iter()
                        .filter(|event| matches!(event, Event::Attack { .. }))
                        .count(),
                    1
                );
                assert!((profile.cooldown - 1..=profile.cooldown + 2)
                    .contains(&entities.get(attacker).unwrap().weapon_cooldown(weapon.id)));
                assert!(events[&2]
                    .iter()
                    .all(|event| !matches!(event, Event::Notice { .. })));
                // Detached shots survive their caster dying; preserve the launch rank and item damage.
                entities
                    .get_mut(attacker)
                    .unwrap()
                    .apply_damage(u32::MAX, None);
                resolve_orbs(
                    &map,
                    &mut entities,
                    &teams,
                    &fog,
                    &mut events,
                    &mut shots,
                    due - 1,
                );
                assert_eq!(entities.get(target).unwrap().hp, hp);
                resolve_orbs(
                    &map,
                    &mut entities,
                    &teams,
                    &fog,
                    &mut events,
                    &mut shots,
                    due,
                );
                let expected = match (master, double_damage) {
                    (false, false) => 10,
                    (false, true) => 20,
                    (true, false) => 18,
                    (true, true) => 35,
                };
                assert_eq!(hp - entities.get(target).unwrap().hp, expected);
                assert!(shots.shots.is_empty());
                assert_eq!(
                    entities.get(target).unwrap().last_damage_entity(),
                    Some(attacker)
                );
                // Impact must not launch a second visual orb or apply damage twice.
                assert_eq!(
                    events[&1]
                        .iter()
                        .filter(|event| matches!(event, Event::Attack { .. }))
                        .count(),
                    1
                );
                resolve_orbs(
                    &map,
                    &mut entities,
                    &teams,
                    &fog,
                    &mut events,
                    &mut shots,
                    due + 1,
                );
                assert_eq!(hp - entities.get(target).unwrap().hp, expected);
            }
        }
    }
}

#[test]
fn magic_orbs_discard_dead_and_missing_victims() {
    let map = crate::game::map::Map {
        width: 16,
        height: 16,
        terrain: vec![crate::protocol::terrain::GRASS; 256],
        ..Default::default()
    };
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::MagicAdept, 100.0, 100.0)
        .unwrap();
    let dead = entities
        .spawn_unit(2, EntityKind::Rifleman, 200.0, 100.0)
        .unwrap();
    entities.get_mut(dead).unwrap().apply_damage(u32::MAX, None);
    let missing = 9999;
    let mut shots = crate::game::disciple_shot::DiscipleShotStore::default();
    for target in [dead, missing] {
        shots.schedule(
            1,
            attacker,
            target,
            (100.0, 100.0),
            (200.0, 100.0),
            23,
            0.25,
            10,
        );
    }
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let fog = Fog::new(16, 16);
    let mut events = HashMap::from([(1, vec![]), (2, vec![])]);
    resolve_orbs(
        &map,
        &mut entities,
        &teams,
        &fog,
        &mut events,
        &mut shots,
        40,
    );
    assert!(shots.shots.is_empty());
    assert!(events.values().all(Vec::is_empty));
    assert_eq!(entities.get(dead).unwrap().hp, 0);
}
