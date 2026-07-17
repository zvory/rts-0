use super::*;

#[test]
fn mortar_setup_and_teardown_commands_use_mortar_timings() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let mortar = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::SetupAntiTankGuns {
                units: vec![mortar],
                x: 220.0,
                y: 100.0,
                queued: false,
            },
        )],
    );

    let mortar_entity = entities.get(mortar).expect("mortar should exist");
    assert_eq!(mortar_entity.weapon_setup(), WeaponSetup::Packed);
    assert_eq!(mortar_entity.emplacement_facing(), Some(0.0));

    entities
        .get_mut(mortar)
        .expect("mortar should exist")
        .set_weapon_setup(WeaponSetup::Deployed);
    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::TearDownAntiTankGuns {
                units: vec![mortar],
            },
        )],
    );

    assert_eq!(
        entities
            .get(mortar)
            .expect("mortar should exist")
            .weapon_setup(),
        WeaponSetup::TearingDown {
            ticks: config::MORTAR_TEAM_TEARDOWN_TICKS,
        }
    );
}
