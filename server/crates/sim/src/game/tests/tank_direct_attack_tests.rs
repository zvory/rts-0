use super::fixtures::*;
use super::*;

fn fixture(kind: EntityKind, distance: f32) -> (Game, u32, u32) {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let attacker = game
        .state
        .entities
        .spawn_unit(1, kind, 500.0, 500.0)
        .unwrap();
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, 500.0 + distance, 500.0)
        .unwrap();
    game.state
        .entities
        .spawn_unit(1, EntityKind::CommandCar, 500.0 + distance, 650.0)
        .unwrap();
    // Keep the target still and prevent return fire from affecting the observation.
    let enemy = game.state.entities.get_mut(target).unwrap();
    enemy.set_order(Order::HoldPosition);
    enemy.set_attack_cd(1000);
    enemy.set_weapon_cooldown(combat::WeaponKind::TankCoax, 1000);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    (game, attacker, target)
}

fn attack(game: &mut Game, attacker: u32, target: u32) {
    game.enqueue(
        1,
        Command::Attack {
            units: vec![attacker],
            target,
            queued: false,
        },
    );
    game.tick();
}

#[test]
fn direct_attack_switches_target_during_reload_for_tanks_and_riflemen() {
    for kind in [EntityKind::Tank, EntityKind::Rifleman] {
        for cooldown in [0, 1, 15, 90] {
            let (mut game, attacker, old_target) = fixture(kind, 100.0);
            let new_target = game
                .state
                .entities
                .spawn_unit(2, EntityKind::Tank, 580.0, 540.0)
                .unwrap();
            let enemy = game.state.entities.get_mut(new_target).unwrap();
            enemy.set_attack_cd(1000);
            enemy.set_weapon_cooldown(combat::WeaponKind::TankCoax, 1000);
            game.state
                .fog
                .recompute(&[1, 2], &game.state.entities, &game.state.map);
            let unit = game.state.entities.get_mut(attacker).unwrap();
            unit.set_order(Order::attack(old_target));
            unit.set_target_id(Some(old_target));
            unit.set_attack_cd(cooldown);
            attack(&mut game, attacker, new_target);
            let unit = game.state.entities.get(attacker).unwrap();
            assert_eq!(
                unit.order().attack_target(),
                Some(new_target),
                "{kind:?}, cooldown {cooldown}"
            );
            assert_eq!(
                unit.target_id(),
                Some(new_target),
                "{kind:?}, cooldown {cooldown}"
            );
            assert!(unit.path_is_empty());
        }
    }
}

#[test]
fn direct_tank_attack_waits_for_range_then_fires_without_moving() {
    // At zero charge this is outside current range, but inside the full stationary range.
    let (mut game, tank, target) = fixture(EntityKind::Tank, 350.0);
    let hp = game.state.entities.get(target).unwrap().hp;
    attack(&mut game, tank, target);
    assert_eq!(
        game.state.entities.get(target).unwrap().hp,
        hp,
        "must not fire using future range"
    );
    for _ in 0..100 {
        let unit = game.state.entities.get(tank).unwrap();
        assert_eq!((unit.pos_x, unit.pos_y), (500.0, 500.0));
        assert!(unit.path_is_empty());
        assert_eq!(unit.order().attack_target(), Some(target));
        game.tick();
    }
    assert!(
        game.state.entities.get(target).unwrap().hp < hp,
        "tank must fire once range reaches the target"
    );
}

#[test]
fn direct_tank_attack_pursues_when_target_leaves_stationary_range() {
    let (mut game, tank, target) = fixture(EntityKind::Tank, 350.0);
    attack(&mut game, tank, target);
    assert!(game.state.entities.get(tank).unwrap().path_is_empty());
    game.state
        .entities
        .get_mut(target)
        .unwrap()
        .set_position(1030.0, 500.0);
    // A friendly spotter preserves authoritative visibility of the retreating target.
    game.state
        .entities
        .spawn_unit(1, EntityKind::CommandCar, 1030.0, 650.0)
        .unwrap();
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
    for _ in 0..15 {
        game.tick();
    }
    let unit = game.state.entities.get(tank).unwrap();
    assert_eq!(unit.order().attack_target(), Some(target));
    assert!(
        unit.pos_x > 500.0,
        "target beyond maximum range must still be pursued"
    );
}
