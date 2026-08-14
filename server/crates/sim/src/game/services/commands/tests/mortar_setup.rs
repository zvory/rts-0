use super::*;

#[test]
fn mortar_setup_and_teardown_commands_are_ignored() {
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
    assert_eq!(mortar_entity.emplacement_facing(), None);
    assert!(matches!(mortar_entity.order(), Order::Idle));

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
        WeaponSetup::Deployed,
        "legacy mortar setup state must not make teardown a valid player command"
    );
}

#[test]
fn queued_mortar_setup_does_not_enter_the_order_queue() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let mortar = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");

    apply(
        &map,
        &mut entities,
        vec![
            (
                1,
                SimCommand::Move {
                    units: vec![mortar],
                    x: 180.0,
                    y: 100.0,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::SetupAntiTankGuns {
                    units: vec![mortar],
                    x: 0.0,
                    y: 0.0,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::AttackMove {
                    units: vec![mortar],
                    x: 220.0,
                    y: 100.0,
                    queued: true,
                },
            ),
        ],
    );

    let mortar = entities.get(mortar).expect("mortar should exist");
    assert_eq!(mortar.queued_orders().len(), 2);
    assert!(matches!(mortar.queued_orders()[0], OrderIntent::Move(_)));
    assert!(matches!(
        mortar.queued_orders()[1],
        OrderIntent::AttackMove(_)
    ));
}
