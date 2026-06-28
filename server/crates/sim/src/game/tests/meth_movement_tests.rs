use super::*;
use crate::game::upgrade::UpgradeKind;

fn meth_unit_fixture(kind: EntityKind, enqueue_move: bool) -> (Game, u32, (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x5150_0702);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    let start = game.map.tile_center(20, 20);
    let goal = (start.0 + 500.0, start.1);
    let unit = game
        .entities
        .spawn_unit(1, kind, start.0, start.1)
        .expect("unit should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    game.assert_invariants();

    if enqueue_move {
        game.enqueue(
            1,
            Command::Move {
                units: vec![unit],
                x: goal.0,
                y: goal.1,
                queued: false,
            },
        );
    }

    (game, unit, goal)
}

fn meth_movement_fixture(kind: EntityKind) -> (Game, u32, (f32, f32)) {
    meth_unit_fixture(kind, true)
}

fn entity_pos(game: &Game, id: u32) -> (f32, f32) {
    let entity = game.entities.get(id).expect("entity should exist");
    (entity.pos_x, entity.pos_y)
}

fn moved_distance(from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    (dx * dx + dy * dy).sqrt()
}

fn next_moving_step(game: &mut Game, id: u32) -> f32 {
    for _ in 0..3 {
        let before = entity_pos(game, id);
        game.tick();
        let moved = moved_distance(before, entity_pos(game, id));
        if moved > 0.0 {
            return moved;
        }
    }
    0.0
}

#[test]
fn removed_methamphetamines_clears_persistent_rifleman_speed() {
    let (mut game, rifleman, _goal) = meth_movement_fixture(EntityKind::Rifleman);
    let base_speed = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .speed;

    game.players[0].upgrades.insert(UpgradeKind::Methamphetamines);
    let boosted_step = next_moving_step(&mut game, rifleman);
    assert!(
        (boosted_step - base_speed * config::RIFLEMAN_CHARGE_SPEED_MULTIPLIER).abs() < 0.01,
        "researched Methamphetamines should boost rifleman speed, moved {boosted_step:.3}px"
    );
    assert!(
        game.entities
            .get(rifleman)
            .expect("rifleman should exist")
            .charge_ticks()
            > config::RIFLEMAN_CHARGE_TICKS,
        "Methamphetamines should seed the persistent charge state"
    );

    game.players[0]
        .upgrades
        .remove(&UpgradeKind::Methamphetamines);
    let normal_step = next_moving_step(&mut game, rifleman);
    assert!(
        (normal_step - base_speed).abs() < 0.01,
        "removed Methamphetamines should immediately return riflemen to base speed, moved {normal_step:.3}px"
    );
    assert_eq!(
        game.entities
            .get(rifleman)
            .expect("rifleman should exist")
            .charge_ticks(),
        0,
        "removed Methamphetamines should clear the persistent charge state"
    );
}

#[test]
fn methamphetamines_boosts_machine_gunner_to_unupgraded_rifleman_speed() {
    let (mut game, mg, _goal) = meth_movement_fixture(EntityKind::MachineGunner);
    let rifleman_speed = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .speed;

    game.players[0].upgrades.insert(UpgradeKind::Methamphetamines);
    let boosted_step = next_moving_step(&mut game, mg);

    assert!(
        (boosted_step - rifleman_speed).abs() < 0.01,
        "researched Methamphetamines should move machine gunners at unupgraded rifleman speed, moved {boosted_step:.3}px"
    );
}

#[test]
fn methamphetamines_halves_machine_gunner_setup_and_teardown() {
    let (mut game, mg, goal) = meth_unit_fixture(EntityKind::MachineGunner, false);
    game.players[0].upgrades.insert(UpgradeKind::Methamphetamines);

    game.tick();
    assert!(
        matches!(
            game.entities.get(mg).expect("mg should exist").weapon_setup(),
            WeaponSetup::SettingUp { .. }
        ),
        "idle machine gunner should start setting up"
    );

    for _ in 1..config::METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS {
        game.tick();
    }
    assert!(
        !matches!(
            game.entities.get(mg).expect("mg should exist").weapon_setup(),
            WeaponSetup::Deployed
        ),
        "machine gunner should still be setting up one tick before the meth-shortened timer expires"
    );
    game.tick();
    assert_eq!(
        game.entities.get(mg).expect("mg should exist").weapon_setup(),
        WeaponSetup::Deployed
    );

    let start = entity_pos(&game, mg);
    game.enqueue(
        1,
        Command::Move {
            units: vec![mg],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        entity_pos(&game, mg),
        start,
        "machine gunner should not move during teardown"
    );
    assert!(
        matches!(
            game.entities.get(mg).expect("mg should exist").weapon_setup(),
            WeaponSetup::TearingDown { .. }
        ),
        "move command should start teardown from deployed state"
    );

    for _ in 1..config::METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS {
        game.tick();
    }
    assert_eq!(
        game.entities.get(mg).expect("mg should exist").weapon_setup(),
        WeaponSetup::Packed
    );

    let moved_step = next_moving_step(&mut game, mg);
    assert!(
        moved_step > 0.0,
        "machine gunner should move after meth-shortened teardown completes"
    );
}
