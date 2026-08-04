use super::*;
use crate::game::entity::Order;
use crate::rules::combat::WeaponKind;

#[test]
fn reloading_plain_move_acquires_and_tracks_an_in_range_gun_without_stopping() {
    let setup = Game::new_move_reload_acquisition_scenario(EntityKind::Tank, 1, 0xA7_263)
        .expect("move reload acquisition scenario setup should succeed");
    assert_eq!(setup.issue_after_ticks, config::TICK_HZ * 10);
    assert!(matches!(setup.command(), SimCommand::Move { .. }));

    let attacker_id = setup.units[0];
    let target_id = setup
        .game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 2 && entity.kind == EntityKind::AntiTankGun)
        .expect("scenario Anti-Tank Gun should exist")
        .id;
    let cannon = crate::rules::combat::weapon_profile(WeaponKind::TankCannon)
        .expect("Tank cannon profile should exist");
    let command = setup.command();
    let mut game = setup.game;
    for _ in 0..setup.issue_after_ticks {
        game.tick();
    }
    game.enqueue(setup.player_id, command);
    game.tick();

    // After one second the Tank is plainly inside its moving cannon range. It should already have
    // committed the gun and begun aiming without interrupting its plain Move order.
    for _ in 0..config::TICK_HZ {
        game.tick();
    }
    let attacker = game
        .state
        .entities
        .get(attacker_id)
        .expect("scenario Tank should survive");
    let target = game
        .state
        .entities
        .get(target_id)
        .expect("scenario Anti-Tank Gun should survive");
    let distance = (target.pos_x - attacker.pos_x).hypot(target.pos_y - attacker.pos_y);
    let moving_range = cannon.range_tiles as f32 * config::TILE_SIZE as f32 + attacker.radius();
    assert!(
        distance < moving_range,
        "gun should already be inside cannon range"
    );
    assert!(matches!(attacker.order(), Order::Move(_)));
    assert_eq!(attacker.target_id(), Some(target_id));
    assert!(attacker.weapon_cooldown(WeaponKind::TankCannon) > 0);
    assert!(
        (attacker.weapon_facing().unwrap_or_default() - attacker.facing()).abs() > 0.1,
        "turret should track the gun independently of the moving hull"
    );
    assert!(!attacker.path_is_empty(), "plain Move must keep travelling");

    let before_reissue = (attacker.pos_x, attacker.pos_y);
    game.enqueue(
        setup.player_id,
        SimCommand::Move {
            units: vec![attacker_id],
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );
    game.tick();
    let attacker = game
        .state
        .entities
        .get(attacker_id)
        .expect("scenario Tank should survive the repeated Move");
    assert_eq!(
        attacker.target_id(),
        Some(target_id),
        "a repeated Move should reacquire during the same reload cycle"
    );
    assert!(attacker.weapon_cooldown(WeaponKind::TankCannon) > 0);
    assert_ne!(
        (attacker.pos_x, attacker.pos_y),
        before_reissue,
        "reacquisition must not stop an ordinary Move"
    );
}
