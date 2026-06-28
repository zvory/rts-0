use super::*;
use crate::game::tests::fixtures::{empty_flat_game, human_vs_ai_players};

fn deploy_anti_tank_gun_toward(game: &mut Game, id: u32, target: (f32, f32)) {
    let gun = game
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
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.recompute_live_fog(&ids);
    game.refresh_building_memory(&ids);
}

fn hidden_enemy_at_gun_fixture() -> (Game, u32, u32) {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let target_pos = game.map.tile_center(10, 10);
    let enemy_pos = (
        target_pos.0 + config::TILE_SIZE as f32 * 5.0,
        target_pos.1,
    );
    let tank_pos = game.map.tile_center(3, 3);

    game.entities
        .spawn_building(1, EntityKind::CityCentre, target_pos.0, target_pos.1, true)
        .expect("city centre should spawn");
    let tank = game
        .entities
        .spawn_unit(1, EntityKind::Tank, tank_pos.0, tank_pos.1)
        .expect("tank should spawn");
    let enemy_at = game
        .entities
        .spawn_unit(2, EntityKind::AntiTankGun, enemy_pos.0, enemy_pos.1)
        .expect("anti-tank gun should spawn");
    deploy_anti_tank_gun_toward(&mut game, enemy_at, target_pos);
    refresh_visibility_for_test(&mut game);

    assert!(
        !game.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
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
        "firing reveal must be actionable live fog, not lingering vision-only intel"
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
        game.entities
            .get(tank)
            .expect("tank should exist")
            .order()
            .attack_target(),
        Some(enemy_at),
        "a unit should accept a direct attack command against the firing-revealed AT gun"
    );
}

#[test]
fn anti_tank_gun_firing_reveal_lasts_for_firing_cycle_plus_half_second() {
    let (mut game, enemy_at, _tank) = hidden_enemy_at_gun_fixture();
    game.tick();
    let fired_at_tick = game.tick_count();
    let reveal_ticks = config::unit_stats(EntityKind::AntiTankGun)
        .expect("anti-tank gun stats should exist")
        .cooldown
        + config::TICK_HZ / 2;

    game.entities
        .get_mut(enemy_at)
        .expect("anti-tank gun should exist")
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
}
