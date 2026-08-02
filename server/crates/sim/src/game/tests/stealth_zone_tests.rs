use super::*;
use crate::game::services::{occupancy::Occupancy, standability};
use crate::game::tests::fixtures::{empty_flat_game, human_vs_ai_players};

fn refresh_visibility(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let player_ids = game.state.player_ids();
    game.recompute_live_fog(&player_ids);
}

fn scout_and_hidden_rifles() -> (Game, u32, [u32; 2]) {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let scout_pos = game.state.map.tile_center(10, 12);
    let rifle_a_pos = game.state.map.tile_center(14, 12);
    let rifle_b_pos = game.state.map.tile_center(14, 13);
    game.state.map.stealth_tiles = vec![(14, 12), (14, 13)];

    let scout = game
        .state
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_pos.0, scout_pos.1)
        .expect("scout car should spawn");
    let rifle_a = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, rifle_a_pos.0, rifle_a_pos.1)
        .expect("first rifleman should spawn");
    let rifle_b = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, rifle_b_pos.0, rifle_b_pos.1)
        .expect("second rifleman should spawn");
    refresh_visibility(&mut game);
    (game, scout, [rifle_a, rifle_b])
}

#[test]
fn stealth_hides_enemies_on_visible_ground_but_not_their_own_team() {
    let (game, _scout, rifles) = scout_and_hidden_rifles();

    for rifle in rifles {
        let entity = game.state.entities.get(rifle).expect("rifleman");
        assert!(
            game.state
                .fog
                .is_visible_world(1, entity.pos_x, entity.pos_y),
            "the ground tile should be ordinarily visible to the scout team"
        );
        assert!(
            !game
                .snapshot_for(1)
                .entities
                .iter()
                .any(|view| view.id == rifle),
            "an enemy unit on a stealth tile should not be projected"
        );
        assert!(
            game.snapshot_for(2)
                .entities
                .iter()
                .any(|view| view.id == rifle),
            "owners should always see their own units in stealth"
        );
    }
}

#[test]
fn scout_car_waits_for_hidden_rifle_fire_and_the_reveal_reaction_delay() {
    let (mut game, scout, rifles) = scout_and_hidden_rifles();
    let rifle_hp_before = game
        .state
        .entities
        .get(rifles[0])
        .expect("first rifleman")
        .hp;
    let scout_hp_before = game.state.entities.get(scout).expect("scout car").hp;

    game.tick();

    assert!(
        game.state.entities.get(scout).expect("scout car").hp < scout_hp_before,
        "the riflemen should be able to fire at the visible scout car"
    );
    assert_eq!(
        game.state
            .entities
            .get(rifles[0])
            .expect("first rifleman")
            .hp,
        rifle_hp_before,
        "the scout car must not fire before a hidden rifleman exposes itself"
    );

    while !game.tick_count().is_multiple_of(FOG_UPDATE_INTERVAL_TICKS) {
        game.tick();
    }
    let reveal_started = game
        .state
        .fog
        .active_firing_reveal_episode(1, rifles[0])
        .expect("rifle fire should create an actionable reveal for the scout team");
    let revealed = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|view| view.id == rifles[0])
        .expect("revealed rifleman should enter the scout snapshot");
    assert!(
        revealed.vision_only,
        "a firing unit in stealth should use the transient reveal presentation"
    );

    while game.tick_count() < reveal_started + config::TICK_HZ - 1 {
        game.tick();
        assert_eq!(
            game.state
                .entities
                .get(rifles[0])
                .expect("first rifleman should survive the reaction window")
                .hp,
            rifle_hp_before,
            "counterfire should wait for the full one-second reveal reaction delay"
        );
    }

    for _ in 0..=2 {
        game.tick();
        if game
            .state
            .entities
            .get(rifles[0])
            .is_none_or(|rifle| rifle.hp < rifle_hp_before)
        {
            return;
        }
    }
    panic!("the scout car should counterfire once the revealed-target reaction delay elapses");
}

#[test]
fn vehicle_moves_normally_outside_no_vehicle_tiles() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    game.state.players[0].set_resources(0, 100);
    game.state.map.no_vehicle_tiles = (21..30)
        .flat_map(|x| (31..40).map(move |y| (x, y)))
        .collect();
    let start = game.state.map.tile_center(16, 35);
    let goal = game.state.map.tile_center(18, 35);
    let scout = game
        .state
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, start.0, start.1)
        .expect("scout car should spawn");
    let occupancy = Occupancy::build(&game.state.map, &game.state.entities);
    assert!(
        standability::unit_static_standable(
            &game.state.map,
            &occupancy,
            EntityKind::ScoutCar,
            start.0,
            start.1,
        ),
        "test start must itself be statically standable",
    );
    refresh_visibility(&mut game);
    game.enqueue(
        1,
        Command::Move {
            units: vec![scout],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    for _ in 0..20 {
        game.tick();
    }
    let scout_entity = game.state.entities.get(scout).expect("scout car");
    assert!(
        scout_entity.pos_x > start.0,
        "a no-vehicle region must not freeze vehicles elsewhere on the map: pos=({}, {}), facing={}, order={:?}, path_empty={}, next_waypoint={:?}, path_goal={:?}, last_repath_tick={}",
        scout_entity.pos_x,
        scout_entity.pos_y,
        scout_entity.facing(),
        scout_entity.order(),
        scout_entity.path_is_empty(),
        scout_entity.next_waypoint(),
        scout_entity.path_goal(),
        scout_entity.last_repath_tick(),
    );
}
