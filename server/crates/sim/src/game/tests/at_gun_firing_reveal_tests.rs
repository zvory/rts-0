use super::*;
use crate::game::tests::fixtures::{empty_flat_game, human_vs_ai_players};
use crate::protocol::DEFAULT_FACTION_ID;

mod visibility_reaction;

fn deploy_anti_tank_gun_toward(game: &mut Game, id: u32, target: (f32, f32)) {
    let gun = game
        .state
        .entities
        .get_mut(id)
        .expect("anti-tank gun should exist");
    let facing = (target.1 - gun.pos_y).atan2(target.0 - gun.pos_x);
    gun.set_weapon_setup(WeaponSetup::Deployed);
    gun.set_emplacement_facing(Some(facing));
    gun.set_facing(facing);
    gun.set_weapon_facing(facing);
}

fn refresh_visibility_for_test(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.recompute_live_fog(&ids);
    game.refresh_building_memory(&ids);
}

fn hidden_enemy_at_gun_fixture() -> (Game, u32, u32) {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let target_pos = game.state.map.tile_center(10, 10);
    let enemy_pos = (target_pos.0 + config::TILE_SIZE as f32 * 5.0, target_pos.1);
    let tank_pos = game.state.map.tile_center(3, 3);

    game.state
        .entities
        .spawn_building(1, EntityKind::CityCentre, target_pos.0, target_pos.1, true)
        .expect("city centre should spawn");
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, tank_pos.0, tank_pos.1)
        .expect("tank should spawn");
    let enemy_at = game
        .state
        .entities
        .spawn_unit(2, EntityKind::AntiTankGun, enemy_pos.0, enemy_pos.1)
        .expect("anti-tank gun should spawn");
    deploy_anti_tank_gun_toward(&mut game, enemy_at, target_pos);
    refresh_visibility_for_test(&mut game);

    assert!(
        !game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
        "fixture requires the AT gun to start outside player 1 live fog"
    );
    assert!(
        !game
            .snapshot_for(1)
            .entities
            .iter()
            .any(|entity| entity.id == enemy_at),
        "fixture requires the AT gun to start hidden from player 1 snapshots"
    );

    (game, enemy_at, tank)
}

fn hidden_enemy_tank_fixture() -> (Game, u32, u32) {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let target_pos = game.state.map.tile_center(10, 10);
    let enemy_pos = (target_pos.0 + config::TILE_SIZE as f32 * 5.0, target_pos.1);
    let tank_pos = game.state.map.tile_center(3, 3);

    game.state
        .entities
        .spawn_building(1, EntityKind::CityCentre, target_pos.0, target_pos.1, true)
        .expect("city centre should spawn");
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, tank_pos.0, tank_pos.1)
        .expect("tank should spawn");
    let enemy_tank = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, enemy_pos.0, enemy_pos.1)
        .expect("enemy tank should spawn");
    if let Some(enemy) = game.state.entities.get_mut(enemy_tank) {
        let facing = (target_pos.1 - enemy.pos_y).atan2(target_pos.0 - enemy.pos_x);
        enemy.set_facing(facing);
        enemy.set_weapon_facing(facing);
    }
    refresh_visibility_for_test(&mut game);

    assert!(
        !game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
        "fixture requires the tank to start outside player 1 live fog"
    );

    (game, enemy_tank, tank)
}

fn hidden_enemy_at_gun_with_counter_fixture() -> (Game, u32, u32) {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let target_pos = game.state.map.tile_center(10, 10);
    let enemy_pos = (target_pos.0 + config::TILE_SIZE as f32 * 5.0, target_pos.1);
    let counter_pos = (target_pos.0, target_pos.1 + config::TILE_SIZE as f32 * 10.0);

    game.state
        .entities
        .spawn_building(1, EntityKind::CityCentre, target_pos.0, target_pos.1, true)
        .expect("city centre should spawn");
    let counter_at = game
        .state
        .entities
        .spawn_unit(1, EntityKind::AntiTankGun, counter_pos.0, counter_pos.1)
        .expect("counter anti-tank gun should spawn");
    let enemy_at = game
        .state
        .entities
        .spawn_unit(2, EntityKind::AntiTankGun, enemy_pos.0, enemy_pos.1)
        .expect("enemy anti-tank gun should spawn");
    deploy_anti_tank_gun_toward(&mut game, counter_at, enemy_pos);
    deploy_anti_tank_gun_toward(&mut game, enemy_at, target_pos);
    refresh_visibility_for_test(&mut game);

    assert!(
        !game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
        "fixture requires the enemy AT gun to start outside player 1 live fog"
    );

    (game, enemy_at, counter_at)
}

fn three_player_combat_fixture() -> [PlayerInit; 3] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Observer".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Shooter".into(),
            color: "#000".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Victim".into(),
            color: "#f00".into(),
            is_ai: false,
        },
    ]
}

#[test]
fn anti_tank_gun_firing_from_fog_projects_as_actionable_snapshot_entity() {
    let (mut game, enemy_at, tank) = hidden_enemy_at_gun_fixture();

    let events = game.tick();

    assert!(
        events.iter().any(|(player, events)| {
            *player == 1
                && events.iter().any(|event| {
                    matches!(
                        event,
                        Event::Attack {
                            from,
                            reveal: Some(reveal),
                            ..
                        } if *from == enemy_at && reveal.kind == kinds::ANTI_TANK_GUN
                    )
                })
        }),
        "the hidden AT gun shot should still deliver the normal attack reveal event"
    );
    let snapshot = game.snapshot_for(1);
    let view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == enemy_at)
        .expect("firing AT gun should be a normal visible snapshot entity");
    assert!(
        !view.vision_only,
        "firing reveal must use normal live fog, not render-only intel"
    );

    game.enqueue(
        1,
        Command::Attack {
            units: vec![tank],
            target: enemy_at,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(
        game.state
            .entities
            .get(tank)
            .expect("tank should exist")
            .order()
            .attack_target(),
        Some(enemy_at),
        "a unit should accept a direct attack command against the firing-revealed AT gun"
    );
}

#[test]
fn tank_firing_from_fog_projects_as_actionable_snapshot_entity() {
    let (mut game, enemy_tank, tank) = hidden_enemy_tank_fixture();

    let events = game.tick();

    assert!(
        events.iter().any(|(player, events)| {
            *player == 1
                && events.iter().any(|event| {
                    matches!(
                        event,
                        Event::Attack {
                            from,
                            reveal: Some(reveal),
                            ..
                        } if *from == enemy_tank && reveal.kind == kinds::TANK
                    )
                })
        }),
        "the hidden tank shot should still deliver the normal attack reveal event"
    );
    let snapshot = game.snapshot_for(1);
    let view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == enemy_tank)
        .expect("firing tank should be a normal visible snapshot entity");
    assert!(
        !view.vision_only,
        "firing reveal must use normal live fog, not render-only intel"
    );

    game.enqueue(
        1,
        Command::Attack {
            units: vec![tank],
            target: enemy_tank,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(
        game.state
            .entities
            .get(tank)
            .expect("tank should exist")
            .order()
            .attack_target(),
        Some(enemy_tank),
        "a unit should accept a direct attack command against any firing-revealed unit"
    );
}

#[test]
fn third_party_combat_does_not_make_hidden_shooter_actionable() {
    let players = three_player_combat_fixture();
    let mut game = empty_flat_game(&players);
    let target_pos = game.state.map.tile_center(10, 10);
    let shooter_pos = (target_pos.0 + config::TILE_SIZE as f32 * 5.0, target_pos.1);
    let observer_pos = (target_pos.0 - config::TILE_SIZE as f32 * 7.0, target_pos.1);

    let observer = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, observer_pos.0, observer_pos.1)
        .expect("observer worker should spawn");
    let victim_cc = game
        .state
        .entities
        .spawn_building(3, EntityKind::CityCentre, target_pos.0, target_pos.1, true)
        .expect("victim city centre should spawn");
    let shooter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::AntiTankGun, shooter_pos.0, shooter_pos.1)
        .expect("shooter anti-tank gun should spawn");
    deploy_anti_tank_gun_toward(&mut game, shooter, target_pos);
    if let Some(shooter_entity) = game.state.entities.get_mut(shooter) {
        shooter_entity.set_order(Order::attack(victim_cc));
        shooter_entity.set_target_id(Some(victim_cc));
    }
    refresh_visibility_for_test(&mut game);

    assert!(
        game.state
            .fog
            .is_visible_world(1, target_pos.0, target_pos.1),
        "observer should see the third-party target"
    );
    assert!(
        !game
            .state
            .fog
            .is_visible_world(1, shooter_pos.0, shooter_pos.1),
        "observer should not see the third-party shooter"
    );

    game.tick();

    assert!(
        game.snapshot_for(3)
            .entities
            .iter()
            .any(|entity| entity.id == shooter),
        "the victim should receive actionable firing reveal"
    );
    assert!(
        !game
            .snapshot_for(1)
            .entities
            .iter()
            .any(|entity| entity.id == shooter),
        "a third-party observer should not get an actionable firing reveal source"
    );

    game.enqueue(
        1,
        Command::Attack {
            units: vec![observer],
            target: shooter,
            queued: false,
        },
    );
    game.tick();

    assert_ne!(
        game.state
            .entities
            .get(observer)
            .expect("observer should exist")
            .order()
            .attack_target(),
        Some(shooter),
        "third-party combat should not validate attack commands against the hidden shooter"
    );
}

#[test]
fn counterfire_against_firing_revealed_target_waits_one_second() {
    let (mut game, enemy_at, counter_at) = hidden_enemy_at_gun_with_counter_fixture();

    game.tick();
    let hp_after_reveal = game
        .state
        .entities
        .get(enemy_at)
        .expect("enemy AT gun should exist")
        .hp;
    game.state
        .entities
        .get_mut(enemy_at)
        .expect("enemy AT gun should exist")
        .set_attack_cd(u32::MAX);

    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(enemy_at)
            .expect("enemy AT gun should still exist")
            .hp,
        hp_after_reveal,
        "counter AT gun should not fire on the first revealed-target acquisition tick"
    );
    assert_eq!(
        game.state
            .entities
            .get(counter_at)
            .expect("counter AT gun should exist")
            .target_id(),
        Some(enemy_at),
        "counter AT gun should still acquire the firing-revealed target"
    );

    for _ in 1..config::TICK_HZ {
        game.tick();
        assert_eq!(
            game.state
                .entities
                .get(enemy_at)
                .expect("enemy AT gun should still exist")
                .hp,
            hp_after_reveal,
            "counterfire should wait the full one-second response delay"
        );
    }

    game.tick();
    assert!(
        game.state.entities.get(enemy_at).is_none_or(|entity| entity.hp < hp_after_reveal),
        "counter AT gun should fire after the one-second response delay while reveal remains active"
    );
}

#[test]
fn anti_tank_gun_firing_reveal_lasts_for_firing_cycle_plus_half_second() {
    let (mut game, enemy_at, tank) = hidden_enemy_at_gun_fixture();
    game.tick();
    let fired_at_tick = game.tick_count();
    let reveal_ticks = config::unit_stats(EntityKind::AntiTankGun)
        .expect("anti-tank gun stats should exist")
        .cooldown
        + config::TICK_HZ / 2;

    game.state
        .entities
        .get_mut(enemy_at)
        .expect("anti-tank gun should exist")
        .set_attack_cd(u32::MAX);
    game.state
        .entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_attack_cd(u32::MAX);

    for _ in 1..reveal_ticks {
        game.tick();
    }
    assert_eq!(
        game.tick_count(),
        fired_at_tick + reveal_ticks - 1,
        "test should stop on the final active reveal tick"
    );
    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .any(|entity| entity.id == enemy_at),
        "AT gun should remain visible through the full firing-cycle-plus-half-second window"
    );

    game.enqueue(
        1,
        Command::Attack {
            units: vec![tank],
            target: enemy_at,
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.tick_count(),
        fired_at_tick + reveal_ticks,
        "test should advance to the first expired reveal tick"
    );
    assert!(
        !game
            .snapshot_for(1)
            .entities
            .iter()
            .any(|entity| entity.id == enemy_at),
        "AT gun should disappear from snapshots once the firing reveal expires"
    );
    assert_ne!(
        game.state
            .entities
            .get(tank)
            .expect("tank should exist")
            .order()
            .attack_target(),
        Some(enemy_at),
        "commands on the first expired reveal tick must consume refreshed event visibility"
    );
}
