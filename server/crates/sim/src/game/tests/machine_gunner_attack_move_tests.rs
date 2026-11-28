use super::fixtures::*;
use super::*;

fn fixture(setup: WeaponSetup, enemy_x: f32) -> (Game, u32, u32) {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let mg = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MachineGunner, 300., 300.)
        .unwrap();
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_x, 300.)
        .unwrap();
    game.state
        .entities
        .get_mut(mg)
        .unwrap()
        .set_weapon_setup(setup);
    // Keep the target stationary and harmless while observing uninterrupted setup/fire.
    game.state
        .entities
        .get_mut(enemy)
        .unwrap()
        .set_order(Order::HoldPosition);
    game.state
        .entities
        .get_mut(enemy)
        .unwrap()
        .set_weapon_cooldown(combat::WeaponKind::RiflemanRifle, 1000);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    (game, mg, enemy)
}
fn attack_move(game: &mut Game, mg: u32) {
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![mg],
            x: 600.,
            y: 300.,
            queued: false,
        },
    );
}

#[test]
fn repeated_ground_attack_moves_finish_mg_setup_and_fire() {
    let (mut game, mg, enemy) = fixture(WeaponSetup::SettingUp { ticks: 30 }, 420.);
    let initial_hp = game.state.entities.get(enemy).unwrap().hp;
    for tick in 0..60 {
        if tick % 9 == 0 {
            attack_move(&mut game, mg);
        }
        game.tick();
        let unit = game.state.entities.get(mg).unwrap();
        assert_eq!((unit.pos_x(), unit.pos_y()), (300., 300.));
        assert!(!matches!(
            unit.weapon_setup(),
            WeaponSetup::Packed | WeaponSetup::TearingDown { .. }
        ));
        if tick < 29 {
            assert_eq!(
                unit.weapon_setup(),
                WeaponSetup::SettingUp { ticks: 29 - tick }
            );
        } else {
            assert_eq!(unit.weapon_setup(), WeaponSetup::Deployed);
        }
    }
    assert!(
        game.state
            .entities
            .get(enemy)
            .is_none_or(|e| e.hp < initial_hp),
        "MG must actually fire despite order refreshes"
    );
    game.state.entities.remove(enemy);
    for _ in 0..100 {
        game.tick();
    }
    assert!(
        game.state.entities.get(mg).unwrap().pos_x() > 300.,
        "new destination must resume after target loss and teardown grace"
    );
}

#[test]
fn deployed_mg_preserves_trench_and_bonus_range_on_ground_attack_move() {
    // Base range is six tiles; the occupied trench adds one.
    let (mut game, mg, enemy) = fixture(WeaponSetup::Deployed, 515.);
    let trench = game
        .state
        .trenches
        .create(&game.state.map, 300., 300.)
        .unwrap();
    game.state
        .entities
        .get_mut(mg)
        .unwrap()
        .movement
        .as_mut()
        .unwrap()
        .occupied_trench_id = Some(trench);
    for _ in 0..3 {
        attack_move(&mut game, mg);
        game.tick();
        let unit = game.state.entities.get(mg).unwrap();
        assert_eq!(unit.weapon_setup(), WeaponSetup::Deployed);
        assert_eq!(unit.target_id(), Some(enemy));
        assert_eq!(
            unit.movement.as_ref().unwrap().occupied_trench_id,
            Some(trench)
        );
        assert_eq!((unit.pos_x(), unit.pos_y()), (300., 300.));
    }
}

#[test]
fn ground_attack_move_without_shootable_enemy_and_plain_move_still_pack() {
    for (enemy_x, plain_move, hidden) in [
        (900., false, false),
        (420., true, false),
        (420., false, true),
    ] {
        let (mut game, mg, _) = fixture(WeaponSetup::Deployed, enemy_x);
        if hidden {
            game.state.fog = Fog::new(game.state.map.width, game.state.map.height);
            game.state
                .fog
                .recompute(&[1, 2], &EntityStore::new(), &game.state.map);
        }
        if plain_move {
            game.enqueue(
                1,
                Command::Move {
                    units: vec![mg],
                    x: 600.,
                    y: 300.,
                    queued: false,
                },
            );
        } else {
            attack_move(&mut game, mg);
        }
        game.tick();
        assert!(matches!(
            game.state.entities.get(mg).unwrap().weapon_setup(),
            WeaponSetup::TearingDown { .. }
        ));
    }
}

#[test]
fn smoke_blocked_enemy_does_not_pin_a_ground_attack_move_mg() {
    let (mut game, mg, _) = fixture(WeaponSetup::Deployed, 420.);
    game.state
        .smokes
        .spawn(420., 300., 1., 300, game.tick_count())
        .unwrap();
    attack_move(&mut game, mg);
    game.tick();
    assert!(matches!(
        game.state.entities.get(mg).unwrap().weapon_setup(),
        WeaponSetup::TearingDown { .. }
    ));
}
