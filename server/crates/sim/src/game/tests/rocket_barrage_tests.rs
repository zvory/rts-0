use super::fixtures::*;
use super::*;
use crate::game::mortar::HALF_TURN_TICKS;
use crate::game::services::dist2;

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
        .spawn_unit(
            1,
            EntityKind::RocketLauncher,
            launcher_pos.0,
            launcher_pos.1,
        )
        .expect("rocket launcher should spawn");
    if let Some(entity) = game.state.entities.get_mut(launcher) {
        entity.set_facing(0.0);
        entity.set_weapon_facing(0.0);
    }
    let player = game
        .state
        .players
        .iter_mut()
        .find(|player| player.id == 1)
        .unwrap();
    player.set_resources(0, oil);
    player.upgrades.insert(upgrade::UpgradeKind::Rockets);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
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
    assert_eq!(
        config::ROCKET_BARRAGE_RELOAD_TICKS,
        config::TICK_HZ as u16 * 30
    );
    let (mut game, launcher, target) = fixture(0);
    assert!(
        !game.state.fog.is_visible_world(1, target.0, target.1),
        "the regression target must begin outside the Rocket Truck's sight"
    );
    order_barrage(&mut game, launcher, target);

    let mut launches = Vec::new();
    for _ in 0..=config::ROCKET_BARRAGE_UNLOAD_TICKS + 2 {
        for (player, events) in game.tick() {
            if player != 1 {
                continue;
            }
            launches.extend(events.into_iter().filter(|event| {
                matches!(
                    event,
                    Event::MortarLaunch { from, rocket: true, .. } if *from == launcher
                )
            }));
        }
    }

    assert_eq!(launches.len(), config::ROCKET_BARRAGE_ROCKETS as usize);
    let scatter_radius = config::ROCKET_BARRAGE_SCATTER_RADIUS_TILES * config::TILE_SIZE as f32;
    let launch_distances: Vec<f32> = launches
        .iter()
        .filter_map(|event| match event {
            Event::MortarLaunch { to_x, to_y, .. } => {
                Some(dist2(*to_x, *to_y, target.0, target.1).sqrt())
            }
            _ => None,
        })
        .collect();
    assert!(launch_distances
        .iter()
        .all(|distance| *distance <= scatter_radius + 0.01));
    assert!(
        launch_distances
            .iter()
            .any(|distance| *distance > 4.0 * config::TILE_SIZE as f32),
        "the deterministic barrage should exercise the expanded area beyond four tiles"
    );
    assert_eq!(game.state.players[0].oil, 0);
    assert_eq!(
        game.state
            .entities
            .get(launcher)
            .unwrap()
            .ability_uses_remaining(ability::AbilityKind::Barrage),
        Some(0),
    );
}

#[test]
fn barrage_click_waits_for_a_truck_facing_away_then_fires_once() {
    let (mut game, launcher, target) = fixture(0);
    let entity = game.state.entities.get_mut(launcher).unwrap();
    entity.set_facing(std::f32::consts::PI);
    entity.set_weapon_facing(std::f32::consts::PI);
    order_barrage(&mut game, launcher, target);

    let mut launches = 0;
    for _ in 0..=config::ROCKET_BARRAGE_UNLOAD_TICKS + HALF_TURN_TICKS + 4 {
        for (player, events) in game.tick() {
            if player == 1 {
                launches += events
                    .iter()
                    .filter(|event| matches!(event, Event::MortarLaunch { from, rocket: true, .. } if *from == launcher))
                    .count();
            }
        }
    }

    assert_eq!(launches, config::ROCKET_BARRAGE_ROCKETS as usize);
}

#[test]
fn later_barrage_costs_one_hundred_oil() {
    let (mut game, launcher, target) = fixture(100);
    order_barrage(&mut game, launcher, target);
    game.tick();
    assert_eq!(game.state.players[0].oil, 100, "first barrage remains free");

    for _ in 0..config::ROCKET_BARRAGE_RELOAD_TICKS as u32 + 2 {
        game.tick();
    }
    order_barrage(&mut game, launcher, target);
    game.tick();
    assert_eq!(game.state.players[0].oil, 0);
}

#[test]
fn one_manual_barrage_stops_after_sixteen_rockets_and_leaves_mortar_decals() {
    let (mut game, launcher, target) = fixture(0);
    order_barrage(&mut game, launcher, target);

    let mut rocket_launches = 0;
    let observation_ticks = config::ROCKET_BARRAGE_RELOAD_TICKS as u32 * 2;
    for _ in 0..observation_ticks {
        for (player, events) in game.tick() {
            if player != 1 {
                continue;
            }
            rocket_launches += events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Event::MortarLaunch {
                            from,
                            rocket: true,
                            ..
                        } if *from == launcher
                    )
                })
                .count();
        }
    }

    assert_eq!(rocket_launches, config::ROCKET_BARRAGE_ROCKETS as usize);
    let (_, decals, trails) = game.ground_decals_for_observer(&ObserverView::Omniscient, 0);
    assert_eq!(decals.len(), config::ROCKET_BARRAGE_ROCKETS as usize);
    assert!(decals
        .iter()
        .all(|decal| decal.decal_class == "mortarBlast"));
    assert!(trails.is_empty());
}
