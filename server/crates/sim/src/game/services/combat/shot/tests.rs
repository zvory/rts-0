use super::*;
use crate::game::entity::EntityKind;
use crate::rules::combat::{weapon_profile, WeaponKind};
use rand::{rngs::SmallRng, SeedableRng};

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
        let mut rng = SmallRng::seed_from_u64(1);
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
        assert_eq!(
            entities.get(attacker).unwrap().weapon_cooldown(weapon.id),
            weapon.cooldown
        );
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
