use super::*;
use crate::rng::PortableRng;

#[test]
fn direct_weapons_only_damage_the_resolved_victim() {
    for weapon in combat_rules::WEAPON_PROFILES.iter().filter(|weapon| {
        !matches!(
            weapon.id,
            combat_rules::WeaponKind::TankCannon
                | combat_rules::WeaponKind::MortarTeamMortar
                | combat_rules::WeaponKind::ArtilleryGun
                | combat_rules::WeaponKind::PanzerfaustLoadedShot
        )
    }) {
        for primary_hp in [1, 45] {
            for miss_chance in [0.0, 1.0] {
                let map = open_map(16);
                let mut entities = EntityStore::new();
                let attacker = entities
                    .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
                    .unwrap();
                let primary = entities
                    .spawn_unit(2, EntityKind::Rifleman, 180.0, 100.0)
                    .unwrap();
                let primary_entity = entities.get_mut(primary).unwrap();
                primary_entity.apply_damage(primary_entity.hp.saturating_sub(primary_hp), None);
                let behind = entities
                    .spawn_unit(2, EntityKind::Worker, 205.0, 100.0)
                    .unwrap();
                let tank = entities
                    .spawn_unit(2, EntityKind::Tank, 240.0, 100.0)
                    .unwrap();
                let farther = entities
                    .spawn_unit(2, EntityKind::Rifleman, 280.0, 100.0)
                    .unwrap();
                let untouched =
                    [attacker, behind, tank, farther].map(|id| (id, entities.get(id).unwrap().hp));
                let teams = default_team_relations();
                let fog = visible_fog(&map, &entities);
                let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
                let mut rng = PortableRng::seed_from_u64(0);
                let blockers = ShotBlockerIndex::build(&map, &entities);
                apply_damage(
                    &map,
                    &mut entities,
                    &blockers,
                    &teams,
                    &mut events,
                    &fog,
                    &mut rng,
                    attacker,
                    primary,
                    weapon,
                    weapon.dmg,
                    1,
                    100.0,
                    100.0,
                    180.0,
                    100.0,
                    miss_chance,
                    10,
                );
                if miss_chance == 0.0 {
                    assert!(
                        entities.get(primary).unwrap().hp < primary_hp,
                        "{:?} must hit its primary",
                        weapon.id
                    );
                } else {
                    assert_eq!(entities.get(primary).unwrap().hp, primary_hp);
                    assert!(events[&1]
                        .iter()
                        .any(|event| matches!(event, Event::Miss { to } if *to == primary)));
                }
                for (id, hp) in untouched {
                    assert_eq!(
                        entities.get(id).unwrap().hp,
                        hp,
                        "{:?} damaged bystander {id}",
                        weapon.id
                    );
                    assert!(events[&1]
                        .iter()
                        .all(|event| !matches!(event, Event::Attack { to, .. } if *to == id)));
                }
                assert_eq!(events[&1].iter().filter(|event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == primary)).count(), 1);
            }
        }
    }
}
