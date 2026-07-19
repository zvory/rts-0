use super::*;
use crate::game::entity::Order;
use crate::rules::combat::WeaponKind;

#[test]
fn scenario_starts_on_the_failure_boundary() {
    let setup = Game::new_attack_move_reload_acquisition_scenario(EntityKind::Tank, 1, 0x5150_0719)
        .expect("attack-move reload acquisition scenario setup should succeed");
    assert_eq!(setup.issue_after_ticks, config::TICK_HZ * 10);
    assert!(setup.attack_move);
    assert_eq!(setup.units.len(), 2);

    let attacker_id = setup.units[0];
    let target_id = setup.units[1];
    let attacker = setup
        .game
        .state
        .entities
        .get(attacker_id)
        .expect("scenario attacker should exist");
    let target = setup
        .game
        .state
        .entities
        .get(target_id)
        .expect("scenario target should exist");
    let cannon = crate::rules::combat::weapon_profile(WeaponKind::TankCannon)
        .expect("Tank cannon profile should exist");
    let dx = target.pos_x - attacker.pos_x;
    let dy = target.pos_y - attacker.pos_y;
    let current_range = (cannon.range_tiles + 1) as f32 * config::TILE_SIZE as f32;
    assert!(
        dx * dx + dy * dy < current_range * current_range,
        "target must begin inside the moving Tank's current cannon range"
    );
    assert_eq!(attacker.target_id(), None);
    assert_eq!(
        attacker.weapon_cooldown(WeaponKind::TankCannon),
        cannon.cooldown
    );
    assert!(matches!(target.order(), Order::HoldPosition));
    assert!(
        target.invulnerable(),
        "inspection target should survive the demonstration"
    );

    let mut game = setup.game;
    game.enqueue(
        setup.player_id,
        SimCommand::AttackMove {
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
        .expect("scenario attacker should survive its first tick");
    assert!(matches!(attacker.order(), Order::AttackMove(_)));
    assert_eq!(
        attacker.weapon_cooldown(WeaponKind::TankCannon),
        cannon.cooldown - 1,
        "the real attack-move command should begin while the cannon remains reloading"
    );
}
