use super::fixtures::*;
use super::*;

fn fixture(oil: u32) -> (Game, u32, (f32, f32)) {
    let players = [
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
    ];
    let mut game = empty_flat_game(&players);
    let launcher_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(20, 8);
    let launcher = game
        .state
        .entities
        .spawn_unit(1, EntityKind::RocketLauncher, launcher_pos.0, launcher_pos.1)
        .expect("rocket launcher should spawn");
    if let Some(entity) = game.state.entities.get_mut(launcher) {
        entity.set_facing(0.0);
        entity.set_weapon_facing(0.0);
    }
    let player = game.state.players.iter_mut().find(|player| player.id == 1).unwrap();
    player.set_resources(0, oil);
    player.upgrades.insert(upgrade::UpgradeKind::Rockets);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state.fog.recompute(&ids, &game.state.entities, &game.state.map);
    (game, launcher, target_pos)
}

fn order_barrage(game: &mut Game, launcher: u32, target: (f32, f32)) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Barrage,
            units: vec![launcher],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
}

#[test]
fn first_barrage_is_free_and_unloads_sixteen_rockets() {
    let (mut game, launcher, target) = fixture(0);
    order_barrage(&mut game, launcher, target);

    let mut launches = Vec::new();
    for _ in 0..=config::ROCKET_BARRAGE_UNLOAD_TICKS + 2 {
        for (player, events) in game.tick() {
            if player != 1 {
                continue;
            }
            launches.extend(events.into_iter().filter(|event| matches!(
                event,
                Event::MortarLaunch { from, rocket: true, .. } if *from == launcher
            )));
        }
    }

    assert_eq!(launches.len(), config::ROCKET_BARRAGE_ROCKETS as usize);
    assert_eq!(game.state.players[0].oil, 0);
    assert_eq!(
        game.state.entities.get(launcher).unwrap().ability_uses_remaining(ability::AbilityKind::Barrage),
        Some(0),
    );
}

#[test]
fn later_barrage_costs_seventy_five_oil() {
    let (mut game, launcher, target) = fixture(75);
    order_barrage(&mut game, launcher, target);
    game.tick();
    assert_eq!(game.state.players[0].oil, 75, "first barrage remains free");

    for _ in 0..config::ROCKET_BARRAGE_RELOAD_TICKS as u32 + 2 {
        game.tick();
    }
    order_barrage(&mut game, launcher, target);
    game.tick();
    assert_eq!(game.state.players[0].oil, 0);
}
