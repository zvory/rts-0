use super::fixtures::empty_flat_game;
use super::*;
use crate::game::services::occupancy::footprint_center;

fn players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ]
}

fn refresh_fog(game: &mut Game) {
    let ids = game.state.player_ids();
    game.recompute_live_fog(&ids);
}

#[test]
fn ordinary_unit_sight_updates_on_the_next_15_hz_tick() {
    let mut game = empty_flat_game(&players());
    let far = game.state.map.tile_center(30, 30);
    let observer = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, far.0, far.1)
        .expect("observer should spawn");
    let enemy_pos = game.state.map.tile_center(5, 2);
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    game.state
        .entities
        .get_mut(enemy)
        .expect("enemy should exist")
        .set_order(Order::gather(u32::MAX));
    refresh_fog(&mut game);
    assert!(game
        .snapshot_for(1)
        .entities
        .iter()
        .all(|view| view.id != enemy));

    let near = game.state.map.tile_center(1, 2);
    game.state
        .entities
        .get_mut(observer)
        .expect("observer should exist")
        .set_position(near.0, near.1);
    game.state
        .entities
        .get_mut(observer)
        .expect("observer should exist")
        .set_order(Order::gather(u32::MAX));

    game.tick();
    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .all(|view| view.id != enemy),
        "ordinary movement sight should retain the prior grid on the unscheduled tick"
    );

    game.tick();
    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .any(|view| view.id == enemy),
        "ordinary movement sight should refresh on the next 15 Hz tick"
    );
}

#[test]
fn smoke_expiration_wakes_fog_on_an_unscheduled_tick() {
    let mut game = empty_flat_game(&players());
    let observer_pos = game.state.map.tile_center(1, 2);
    game.state
        .entities
        .spawn_unit(1, EntityKind::Worker, observer_pos.0, observer_pos.1)
        .expect("observer should spawn");
    let enemy_pos = game.state.map.tile_center(5, 2);
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    let smoke_pos = game.state.map.tile_center(3, 2);
    game.state
        .smokes
        .spawn(smoke_pos.0, smoke_pos.1, 1.0, 1, game.tick_count())
        .expect("smoke should spawn");
    refresh_fog(&mut game);
    assert!(game
        .snapshot_for(1)
        .entities
        .iter()
        .all(|view| view.id != enemy));

    game.tick();

    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .any(|view| view.id == enemy),
        "an expiring smoke cloud must wake fog without waiting for the scheduled tick"
    );
}

#[test]
fn destroyed_los_blocker_wakes_fog_on_an_unscheduled_tick() {
    let mut game = empty_flat_game(&players());
    let observer_pos = game.state.map.tile_center(6, 3);
    let attacker = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, observer_pos.0, observer_pos.1)
        .expect("attacker should spawn");
    let hidden_pos = game.state.map.tile_center(2, 3);
    let hidden = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, hidden_pos.0, hidden_pos.1)
        .expect("hidden unit should spawn");
    let depot_pos = footprint_center(&game.state.map, EntityKind::Depot, 3, 2);
    let depot = game
        .state
        .entities
        .spawn_building(1, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("depot should spawn");
    refresh_fog(&mut game);
    assert!(game
        .snapshot_for(2)
        .entities
        .iter()
        .all(|view| view.id != hidden));

    let depot_entity = game
        .state
        .entities
        .get_mut(depot)
        .expect("depot should exist");
    let damage = depot_entity.hp.saturating_sub(1);
    assert!(depot_entity.apply_damage(damage, None));
    game.state
        .entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(depot));
    game.tick();
    assert!(
        game.state.entities.get(depot).is_none(),
        "fixture requires the blocker to be destroyed during the tick"
    );

    assert!(
        game.snapshot_for(2)
            .entities
            .iter()
            .any(|view| view.id == hidden),
        "removing a line-of-sight blocker must wake fog without waiting for the scheduled tick"
    );
}
