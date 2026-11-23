use super::*;

#[test]
fn throw_sword_uses_zero_penetration_and_target_armor_without_changing_line_shot() {
    for (ability, kind, outbound, returning) in [
        (AbilityKind::ThrowSword, EntityKind::Tank, 10, 20),
        (AbilityKind::ThrowSword, EntityKind::WarAdept, 20, 40),
        (AbilityKind::ThrowSword, EntityKind::AntiTankGun, 40, 80),
        (AbilityKind::ThrowSword, EntityKind::MortarTeam, 40, 80),
        (AbilityKind::ThrowSword, EntityKind::ScoutCar, 40, 80),
        (AbilityKind::ThrowSword, EntityKind::CommandCar, 40, 80),
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
        spec.damage = 40;
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
    spec.damage = 40;
    runtime.spawn_projectile(spec).unwrap();
    tick_runtime(&mut runtime, &mut entities, 1);
    tick_runtime(&mut runtime, &mut entities, 2);
    // Enter only on the return leg: 80 raw damage becomes 40, leaving 5 HP.
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
    assert_eq!(entities.get(victim).unwrap().hp, 5);
}
