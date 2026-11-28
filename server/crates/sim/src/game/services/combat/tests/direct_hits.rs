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

#[test]
fn disciple_shield_armors_direct_hits_and_tank_splash_and_charges_full_damage() {
    for kind in [
        combat_rules::WeaponKind::RiflemanRifle,
        combat_rules::WeaponKind::TankCannon,
        combat_rules::WeaponKind::AntiTankGun,
        combat_rules::WeaponKind::WarriorSword,
        combat_rules::WeaponKind::DiscipleOrb,
    ] {
        let map = open_map(16);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 200.0)
            .unwrap();
        let primary = entities
            .spawn_unit(2, EntityKind::Rifleman, 300.0, 200.0)
            .unwrap();
        let neighbor = entities
            .spawn_unit(2, EntityKind::Rifleman, 300.0, 224.0)
            .unwrap();
        let caster = entities
            .spawn_unit(2, EntityKind::MagicAdept, 350.0, 200.0)
            .unwrap();
        entities
            .get_mut(caster)
            .unwrap()
            .place_shield(1, 300.0, 200.0, 0, 450, true);
        let teams = default_team_relations();
        let fog = visible_fog(&map, &entities);
        let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
        let mut rng = PortableRng::seed_from_u64(0);
        let blockers = ShotBlockerIndex::build(&map, &entities);
        let weapon = combat_rules::weapon_profile(kind).unwrap();
        let hp = entities.get(primary).unwrap().hp;
        let neighbor_hp = entities.get(neighbor).unwrap().hp;
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
            200.0,
            300.0,
            200.0,
            0.0,
            10,
        )
        .unwrap();
        let remaining = entities
            .get(caster)
            .unwrap()
            .shield
            .as_ref()
            .map_or(0, |s| s.remaining);
        assert_eq!(
            hp - entities.get(primary).unwrap().hp,
            ((combat_rules::effective_damage_for_weapon(
                weapon,
                EntityKind::Rifleman,
                weapon.dmg,
                Some(crate::rules::terrain::TerrainKind::Open),
            ) as f32
                * weapon.armor_penetration)
                .round() as u32)
                .min(hp),
            "{kind:?}"
        );
        let splash = if kind == combat_rules::WeaponKind::TankCannon {
            config::TANK_HE_SPLASH_DAMAGE
        } else {
            0
        };
        assert_eq!(
            remaining,
            config::DISCIPLE_SHIELD_CAPACITY.saturating_sub(weapon.dmg + splash),
            "{kind:?}"
        );
        if kind == combat_rules::WeaponKind::TankCannon {
            assert_eq!(neighbor_hp - entities.get(neighbor).unwrap().hp, 0);
        }
    }
}

#[test]
fn shop_armor_adds_to_unit_armor_before_penetration_and_health_mutation() {
    use crate::game::ability::AbilityKind;
    for (kind, item, penetration, expected) in [
        (EntityKind::WindAdept, false, 0.0, 100),
        (EntityKind::WindAdept, true, 0.0, 50),
        (EntityKind::WindAdept, true, 0.5, 75),
        (EntityKind::WindAdept, true, 1.0, 100),
        (EntityKind::WarAdept, false, 0.0, 50),
        (EntityKind::WarAdept, true, 0.0, 0),
        (EntityKind::WarAdept, true, 0.5, 50),
        (EntityKind::WarAdept, true, 1.0, 100),
        (EntityKind::Tank, true, 0.0, 0),
        (EntityKind::Tank, true, 0.5, 50),
        (EntityKind::Tank, true, 1.0, 100),
    ] {
        let map = open_map(16);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 200.0)
            .unwrap();
        let target = entities.spawn_unit(2, kind, 300.0, 200.0).unwrap();
        let unit = entities.get_mut(target).unwrap();
        unit.set_spawn_health(1000);
        if item {
            unit.receive_shop_item(AbilityKind::ArmorTreasure);
        }
        let mut weapon =
            *combat_rules::weapon_profile(combat_rules::WeaponKind::RiflemanRifle).unwrap();
        weapon.armor_penetration = penetration;
        let teams = default_team_relations();
        let fog = visible_fog(&map, &entities);
        let mut events = HashMap::new();
        super::super::damage::apply_direct_hit(
            &map,
            &mut entities,
            &teams,
            &mut events,
            &fog,
            attacker,
            target,
            &weapon,
            100,
            1,
            (100.0, 200.0),
            0,
            1,
        );
        assert_eq!(
            1000 - entities.get(target).unwrap().hp,
            expected,
            "{kind:?}, item={item}, penetration={penetration}"
        );
    }
}

#[test]
fn resolved_and_armor_bypassing_damage_is_not_halved_again() {
    let mut unit =
        crate::game::entity::Entity::new_unit(1, EntityKind::WindAdept, 100.0, 100.0).unwrap();
    unit.receive_shop_item(crate::game::ability::AbilityKind::ArmorTreasure);
    let hp = unit.hp;
    unit.apply_damage(20, None);
    assert_eq!(
        unit.hp,
        hp - 20,
        "raw HP mutation receives already-resolved damage"
    );
}
