use super::*;
use crate::rng::PortableRng;

#[test]
fn tank_he_preserves_primary_and_splashes_once_with_direct_trench_reduction() {
    for entrenched in [false, true] {
        for vehicle_target in [false, true] {
            for miss in [false, true] {
                let map = open_map(32);
                let mut entities = EntityStore::new();
                let attacker = entities
                    .spawn_unit(1, EntityKind::Tank, 100.0, 200.0)
                    .unwrap();
                let kind = if vehicle_target {
                    EntityKind::Tank
                } else {
                    EntityKind::Rifleman
                };
                let primary = entities.spawn_unit(2, kind, 300.0, 200.0).unwrap();
                let neighbor = entities
                    .spawn_unit(2, EntityKind::Rifleman, 300.0, 224.0)
                    .unwrap();
                let friendly = entities
                    .spawn_unit(1, EntityKind::Rifleman, 300.0, 176.0)
                    .unwrap();
                let edge = entities
                    .spawn_unit(2, EntityKind::Rifleman, 344.75, 200.0)
                    .unwrap();
                let outside = entities
                    .spawn_unit(2, EntityKind::Rifleman, 345.0, 200.0)
                    .unwrap();
                if entrenched {
                    for id in [primary, neighbor, friendly, edge] {
                        entities
                            .get_mut(id)
                            .unwrap()
                            .movement
                            .as_mut()
                            .unwrap()
                            .occupied_trench_id = Some(1);
                    }
                }
                let teams = default_team_relations();
                let fog = visible_fog(&map, &entities);
                let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new()), (3, Vec::new())]);
                let mut rng = PortableRng::seed_from_u64(0);
                let blockers = ShotBlockerIndex::build(&map, &entities);
                let weapon =
                    combat_rules::weapon_profile(combat_rules::WeaponKind::TankCannon).unwrap();
                let primary_before = entities.get(primary).unwrap().hp;
                let expected_primary = if miss {
                    0
                } else {
                    let target = entities.get(primary).unwrap();
                    let damage = combat_rules::effective_damage_with_facing_for_weapon(
                        weapon,
                        kind,
                        60,
                        None,
                        Some(target.facing()),
                        (300.0, 200.0),
                        (100.0, 200.0),
                    );
                    crate::game::entrenchment_combat::reduce_direct_damage(target, damage)
                };
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
                    60,
                    1,
                    100.0,
                    200.0,
                    300.0,
                    200.0,
                    if miss { 1.0 } else { 0.0 },
                    10,
                );
                assert_eq!(
                    entities.get(primary).unwrap().hp,
                    primary_before.saturating_sub(expected_primary)
                );
                let splash = if miss || vehicle_target {
                    0
                } else if entrenched {
                    15
                } else {
                    30
                };
                for id in [neighbor, friendly, edge] {
                    assert_eq!(entities.get(id).unwrap().hp, 45 - splash, "bystander {id}");
                }
                assert_eq!(entities.get(outside).unwrap().hp, 45);
                assert_eq!(events[&1].iter().filter(|event| matches!(event, Event::MortarImpact { radius_tiles, .. } if *radius_tiles == 1.4)).count(), usize::from(!miss && !vehicle_target));
                assert!(!events[&3]
                    .iter()
                    .any(|event| matches!(event, Event::MortarImpact { .. })));
            }
        }
    }
}
