use super::*;

#[test]
fn disciple_shield_armors_projectiles_crossing_barrier_and_pays_full_damage_on_both_legs() {
    for (ability, damage, total) in [
        (
            AbilityKind::ThrowSword,
            crate::config::THROW_SWORD_DAMAGE,
            60,
        ),
        (
            AbilityKind::EkatLineShot,
            crate::config::EKAT_LINE_SHOT_DAMAGE,
            80,
        ),
    ] {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let victim = entities
            .spawn_unit(2, EntityKind::Rifleman, 192.0, 128.0)
            .unwrap();
        let shield_caster = entities
            .spawn_unit(2, EntityKind::MagicAdept, 400.0, 400.0)
            .unwrap();
        entities
            .get_mut(shield_caster)
            .unwrap()
            .place_shield(1, 192.0, 128.0, 0, 150, true);
        let hp = entities.get(victim).unwrap().hp;
        let mut runtime = AbilityRuntime::new();
        let mut spec = projectile_spec(caster, (96.0, 128.0), (288.0, 128.0));
        spec.ability = ability;
        spec.damage = damage;
        runtime.spawn_projectile(spec).unwrap();
        for tick in 1..=6 {
            tick_runtime(&mut runtime, &mut entities, tick);
        }
        assert_eq!(entities.get(victim).unwrap().hp, hp, "{ability:?}");
        assert_eq!(
            entities
                .get(shield_caster)
                .unwrap()
                .shield
                .as_ref()
                .unwrap()
                .remaining,
            crate::config::DISCIPLE_SHIELD_CAPACITY - total,
            "{ability:?}"
        );
    }
}

#[test]
fn throw_sword_uses_zero_penetration_and_target_armor_without_changing_line_shot() {
    for (ability, kind, outbound, returning) in [
        (AbilityKind::ThrowSword, EntityKind::Tank, 5, 10),
        (AbilityKind::ThrowSword, EntityKind::WarAdept, 10, 20),
        (AbilityKind::ThrowSword, EntityKind::AntiTankGun, 20, 40),
        (AbilityKind::ThrowSword, EntityKind::MortarTeam, 20, 40),
        (AbilityKind::ThrowSword, EntityKind::ScoutCar, 20, 40),
        (AbilityKind::ThrowSword, EntityKind::CommandCar, 20, 40),
        (AbilityKind::EkatLineShot, EntityKind::Tank, 40, 40),
        (AbilityKind::EkatLineShot, EntityKind::WarAdept, 40, 40),
    ] {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let victim = entities.spawn_unit(2, kind, 192.0, 128.0).unwrap();
        let initial_hp = entities.get(victim).unwrap().hp;
        let mut runtime = AbilityRuntime::new();
        let mut spec = projectile_spec(caster, (128.0, 128.0), (256.0, 128.0));
        spec.ability = ability;
        spec.damage = if ability == AbilityKind::ThrowSword {
            crate::config::THROW_SWORD_DAMAGE
        } else {
            crate::config::EKAT_LINE_SHOT_DAMAGE
        };
        runtime.spawn_projectile(spec).unwrap();
        tick_runtime(&mut runtime, &mut entities, 1);
        assert_eq!(
            entities.get(victim).unwrap().hp,
            initial_hp.saturating_sub(outbound),
            "{ability:?} outbound against {kind:?}"
        );
        tick_runtime(&mut runtime, &mut entities, 2);
        tick_runtime(&mut runtime, &mut entities, 3);
        assert_eq!(
            entities.get(victim).unwrap().hp,
            initial_hp
                .saturating_sub(outbound)
                .saturating_sub(returning),
            "{ability:?} return against {kind:?}"
        );
    }
}

#[test]
fn throw_sword_return_respects_active_entrenchment() {
    let mut entities = EntityStore::default();
    let caster = spawn_caster(&mut entities, 1);
    let mut runtime = AbilityRuntime::new();
    let mut spec = projectile_spec(caster, (128.0, 128.0), (256.0, 128.0));
    spec.ability = AbilityKind::ThrowSword;
    spec.damage = crate::config::THROW_SWORD_DAMAGE;
    runtime.spawn_projectile(spec).unwrap();
    tick_runtime(&mut runtime, &mut entities, 1);
    tick_runtime(&mut runtime, &mut entities, 2);
    // Enter only on the return leg: 40 raw damage becomes 20, leaving 25 HP.
    let victim = entities
        .spawn_unit(2, EntityKind::Rifleman, 192.0, 128.0)
        .unwrap();
    entities
        .get_mut(victim)
        .unwrap()
        .movement
        .as_mut()
        .unwrap()
        .occupied_trench_id = Some(1);
    tick_runtime(&mut runtime, &mut entities, 3);
    assert_eq!(entities.get(victim).unwrap().hp, 25);
}

#[test]
fn throw_sword_round_trip_leaves_entrenched_rifleman_alive() {
    let mut entities = EntityStore::default();
    let caster = spawn_caster(&mut entities, 1);
    let victim = entities
        .spawn_unit(2, EntityKind::Rifleman, 192.0, 128.0)
        .unwrap();
    entities
        .get_mut(victim)
        .unwrap()
        .movement
        .as_mut()
        .unwrap()
        .occupied_trench_id = Some(1);
    let mut runtime = AbilityRuntime::new();
    let mut spec = projectile_spec(caster, (128.0, 128.0), (256.0, 128.0));
    spec.ability = AbilityKind::ThrowSword;
    spec.damage = crate::config::THROW_SWORD_DAMAGE;
    runtime.spawn_projectile(spec).unwrap();
    tick_runtime(&mut runtime, &mut entities, 1);
    assert_eq!(entities.get(victim).unwrap().hp, 35);
    tick_runtime(&mut runtime, &mut entities, 2);
    tick_runtime(&mut runtime, &mut entities, 3);
    assert_eq!(entities.get(victim).unwrap().hp, 15);
}

#[test]
fn sword_and_line_shot_grant_insight_from_actual_damage_on_both_legs() {
    for master in [false, true] {
        let kind = EntityKind::WarAdept;
        for ability in [AbilityKind::ThrowSword, AbilityKind::EkatLineShot] {
            let mut entities = EntityStore::default();
            let caster = entities.spawn_unit(1, kind, 128.0, 128.0).unwrap();
            let initial_insight = if master { 0 } else { 17 };
            entities.get_mut(caster).unwrap().insight = initial_insight;
            entities.get_mut(caster).unwrap().dao_master = master;
            let victim = entities
                .spawn_unit(2, EntityKind::Tank, 192.0, 128.0)
                .unwrap();
            let initial_hp = entities.get(victim).unwrap().hp;
            let mut runtime = AbilityRuntime::new();
            let mut spec = projectile_spec(caster, (128.0, 128.0), (256.0, 128.0));
            spec.ability = ability;
            runtime.spawn_projectile(spec).unwrap();
            for tick in 1..=3 {
                tick_runtime(&mut runtime, &mut entities, tick);
                let damage = initial_hp - entities.get(victim).unwrap().hp;
                assert_eq!(
                    entities.get(caster).unwrap().insight,
                    initial_insight + if master { 0 } else { damage * 2 },
                    "{kind:?} {ability:?} tick {tick}"
                );
            }
            assert!(entities.get(victim).unwrap().hp < initial_hp);
        }
    }
}
