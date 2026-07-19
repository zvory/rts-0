use super::*;

#[test]
fn setup_anti_tank_gun_group_fans_authoritatively_from_one_command() {
    let map = flat_map(64);
    let tile = config::TILE_SIZE as f32;
    let mut entities = EntityStore::new();
    let guns: Vec<u32> = [-10.0_f32, -5.0, 0.0, 5.0, 10.0]
        .into_iter()
        .map(|y_tiles| {
            entities
                .spawn_unit(
                    1,
                    EntityKind::AntiTankGun,
                    1_000.0,
                    1_000.0 + y_tiles * tile,
                )
                .expect("anti-tank gun should spawn")
        })
        .collect();

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::SetupAntiTankGuns {
                units: guns.clone(),
                x: 1_000.0 + 25.0 * tile,
                y: 1_000.0,
                queued: false,
            },
        )],
    );

    let expected = [
        -std::f32::consts::FRAC_PI_4,
        -std::f32::consts::FRAC_PI_8,
        0.0,
        std::f32::consts::FRAC_PI_8,
        std::f32::consts::FRAC_PI_4,
    ];
    for (id, expected_facing) in guns.into_iter().zip(expected) {
        let facing = entities
            .get(id)
            .and_then(|entity| entity.emplacement_facing())
            .expect("group setup should stage every gun's facing");
        assert!(
            (facing - expected_facing).abs() < 0.0001,
            "expected {expected_facing:.4}, got {facing:.4}"
        );
    }
}
