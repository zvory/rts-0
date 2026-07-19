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
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let start = game.state.map.tile_center(20, 20);
    let goal = (start.0 + 500.0, start.1);
    let unit = game
        .state
        .entities
        .spawn_unit(1, kind, start.0, start.1)
        .expect("unit should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
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
    let entity = game.state.entities.get(id).expect("entity should exist");
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
fn removed_methamphetamines_immediately_clears_rifle_infantry_speed_boost() {
    for kind in [EntityKind::Rifleman, EntityKind::Panzerfaust] {
        let (mut game, unit, _goal) = meth_movement_fixture(kind);
        let base_speed = config::unit_stats(kind)
            .expect("rifle infantry stats")
            .speed;

        game.state.players[0]
            .upgrades
            .insert(UpgradeKind::Methamphetamines);
        let boosted_step = next_moving_step(&mut game, unit);
        assert!(
            (boosted_step - base_speed * config::METHAMPHETAMINES_SPEED_MULTIPLIER).abs() < 0.01,
            "researched Methamphetamines should boost {kind:?} speed, moved {boosted_step:.3}px"
        );

        game.state.players[0]
            .upgrades
            .remove(&UpgradeKind::Methamphetamines);
        let normal_step = next_moving_step(&mut game, unit);
        assert!(
            (normal_step - base_speed).abs() < 0.01,
            "removed Methamphetamines should return {kind:?} to base speed, moved {normal_step:.3}px"
        );
    }
}

#[test]
fn methamphetamines_boosts_machine_gunner_to_unupgraded_rifleman_speed() {
    let (mut game, mg, _goal) = meth_movement_fixture(EntityKind::MachineGunner);
    let rifleman_speed = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .speed;

    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::Methamphetamines);
    let boosted_step = next_moving_step(&mut game, mg);

    assert!(
        (boosted_step - rifleman_speed).abs() < 0.01,
        "researched Methamphetamines should move machine gunners at unupgraded rifleman speed, moved {boosted_step:.3}px"
    );
}

#[test]
fn methamphetamines_halves_machine_gunner_setup_and_teardown() {
    let (mut game, mg, goal) = meth_unit_fixture(EntityKind::MachineGunner, false);
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::Methamphetamines);

    game.tick();
    assert!(
        matches!(
            game.state
                .entities
                .get(mg)
                .expect("mg should exist")
                .weapon_setup(),
            WeaponSetup::SettingUp { .. }
        ),
        "idle machine gunner should start setting up"
    );

    for _ in 1..config::METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS {
        game.tick();
    }
    assert!(
        !matches!(
            game.state.entities.get(mg).expect("mg should exist").weapon_setup(),
            WeaponSetup::Deployed
        ),
        "machine gunner should still be setting up one tick before the meth-shortened timer expires"
    );
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(mg)
            .expect("mg should exist")
            .weapon_setup(),
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
            game.state
                .entities
                .get(mg)
                .expect("mg should exist")
                .weapon_setup(),
            WeaponSetup::TearingDown { .. }
        ),
        "move command should start teardown from deployed state"
    );

    for _ in 1..config::METHAMPHETAMINES_MACHINE_GUNNER_SETUP_TICKS {
        game.tick();
    }
    assert_eq!(
        game.state
            .entities
            .get(mg)
            .expect("mg should exist")
            .weapon_setup(),
        WeaponSetup::Packed
    );

    let moved_step = next_moving_step(&mut game, mg);
    assert!(
        moved_step > 0.0,
        "machine gunner should move after meth-shortened teardown completes"
    );
}
