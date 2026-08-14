use super::*;
use crate::game::services::{occupancy::Occupancy, standability};
use crate::game::tests::fixtures::{empty_flat_game, human_vs_ai_players};

fn allied_spotter_players() -> [PlayerInit; 3] {
    let [spotter, enemy] = human_vs_ai_players();
    [
        spotter,
        PlayerInit {
            id: 3,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Ally".into(),
            color: "#888".into(),
            is_ai: false,
        },
        enemy,
    ]
}

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
    game.state.map.concealment_tiles = vec![(14, 12), (14, 13)];

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

fn close_contact_fixture(extra_gap_px: f32) -> (Game, u32, u32) {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let target_pos = game.state.map.tile_center(20, 20);
    game.state.map.concealment_tiles = vec![(20, 20)];
    let spotter = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("spotter should spawn");
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    let contact_distance = game.state.entities.get(spotter).unwrap().radius()
        + game.state.entities.get(target).unwrap().radius()
        + 2.0 * config::TILE_SIZE as f32
        + extra_gap_px;
    game.state
        .entities
        .get_mut(spotter)
        .unwrap()
        .set_position(target_pos.0 - contact_distance, target_pos.1);
    for id in [spotter, target] {
        game.state
            .entities
            .get_mut(id)
            .unwrap()
            .set_attack_cd(u32::MAX);
    }
    refresh_visibility(&mut game);
    (game, spotter, target)
}

#[test]
fn concealment_close_detection_uses_two_tile_body_edge_range_and_normal_projection() {
    let (inside, _, target) = close_contact_fixture(0.0);
    let view = inside
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|view| view.id == target)
        .expect("exactly two tiles of body-edge separation should detect the target");
    assert!(
        !view.vision_only,
        "close-detected units should render normally"
    );

    let (outside, _, target) = close_contact_fixture(0.25);
    assert!(
        outside
            .snapshot_for(1)
            .entities
            .iter()
            .all(|view| view.id != target),
        "a target just beyond the close-detection boundary should stay concealed"
    );
}

#[test]
fn concealment_close_detection_controls_explicit_attack_legality() {
    let (mut inside, spotter, target) = close_contact_fixture(0.0);
    inside.enqueue(
        1,
        Command::Attack {
            units: vec![spotter],
            target,
            queued: false,
        },
    );
    inside.tick();
    assert_eq!(
        inside.state.entities.get(spotter).unwrap().target_id(),
        Some(target)
    );

    let (mut outside, spotter, target) = close_contact_fixture(0.25);
    outside.enqueue(
        1,
        Command::Attack {
            units: vec![spotter],
            target,
            queued: false,
        },
    );
    outside.tick();
    assert_eq!(
        outside.state.entities.get(spotter).unwrap().target_id(),
        None
    );
}

#[test]
fn concealment_close_detection_persists_for_one_second_after_separation() {
    let (mut game, spotter, target) = close_contact_fixture(0.0);
    assert!(game
        .snapshot_for(1)
        .entities
        .iter()
        .any(|view| view.id == target));
    let far = game.state.map.tile_center(2, 2);
    game.state
        .entities
        .get_mut(spotter)
        .unwrap()
        .set_position(far.0, far.1);

    while game.tick_count() < config::TICK_HZ - 1 {
        game.tick();
    }
    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .any(|view| view.id == target),
        "detection should remain active before its exclusive expiry tick"
    );
    while game.tick_count() < config::TICK_HZ {
        game.tick();
    }
    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .all(|view| view.id != target),
        "detection should expire one second after the final close-contact sample"
    );
}

#[test]
fn concealment_close_detection_is_team_shared_and_checkpointed() {
    let mut game = empty_flat_game(&allied_spotter_players());
    let target_pos = game.state.map.tile_center(20, 20);
    game.state.map.concealment_tiles = vec![(20, 20)];
    let spotter_pos = game.state.map.tile_center(18, 20);
    game.state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, spotter_pos.0, spotter_pos.1)
        .expect("spotter should spawn");
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    refresh_visibility(&mut game);

    assert!(
        game.snapshot_for(3)
            .entities
            .iter()
            .any(|view| view.id == target),
        "a teammate without its own spotter should receive team detection"
    );
    let restored = super::checkpoint_helpers::restore_checkpoint_and_assert_equivalent(
        &game,
        "active concealment close detection",
    );
    assert!(restored
        .snapshot_for(3)
        .entities
        .iter()
        .any(|view| view.id == target));
}

#[test]
fn smoke_suppresses_concealment_close_detection() {
    let (mut game, _, target) = close_contact_fixture(0.0);
    let target_entity = game.state.entities.get(target).unwrap();
    game.state
        .smokes
        .spawn(
            target_entity.pos_x,
            target_entity.pos_y,
            1.0,
            config::TICK_HZ,
            game.tick_count(),
        )
        .expect("smoke should spawn");
    refresh_visibility(&mut game);

    assert!(game
        .snapshot_for(1)
        .entities
        .iter()
        .all(|view| view.id != target));
}

#[test]
fn concealment_hides_enemies_on_visible_ground_but_not_their_own_team() {
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
            "an enemy unit on a concealment tile should not be projected"
        );
        assert!(
            game.snapshot_for(2)
                .entities
                .iter()
                .any(|view| view.id == rifle),
            "owners should always see their own units in concealment"
        );
    }
}

#[test]
fn concealment_does_not_leak_deployed_anti_tank_guns_through_remembered_intel() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let scout_pos = game.state.map.tile_center(10, 12);
    let gun_pos = game.state.map.tile_center(14, 12);
    game.state.map.concealment_tiles = vec![(14, 12)];
    game.state
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_pos.0, scout_pos.1)
        .expect("scout car should spawn");
    let gun = game
        .state
        .entities
        .spawn_unit(2, EntityKind::AntiTankGun, gun_pos.0, gun_pos.1)
        .expect("anti-tank gun should spawn");
    let gun_entity = game
        .state
        .entities
        .get_mut(gun)
        .expect("anti-tank gun should exist");
    gun_entity.set_weapon_setup(crate::game::entity::WeaponSetup::Deployed);
    gun_entity.set_emplacement_facing(Some(0.0));
    gun_entity.set_weapon_facing(0.0);
    refresh_visibility(&mut game);
    let player_ids = game.state.player_ids();
    game.refresh_fog_memories(&player_ids);

    let snapshot = game.snapshot_for(1);
    assert!(snapshot.entities.iter().all(|entity| entity.id != gun));
    assert!(
        snapshot
            .remembered_anti_tank_guns
            .iter()
            .all(|memory| memory.id != gun),
        "ordinary sight of a concealment tile must not refresh exact Anti-Tank Gun intel",
    );
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
    let scout_snapshot = game.snapshot_for(1);
    let revealed = scout_snapshot
        .entities
        .into_iter()
        .find(|view| view.id == rifles[0])
        .expect("revealed rifleman should enter the scout snapshot");
    assert!(
        revealed.vision_only,
        "a firing unit in concealment should use the transient reveal presentation"
    );
    let rifle_tile = game.state.map.tile_of(revealed.x, revealed.y);
    let rifle_tile_index = (rifle_tile.1 * game.state.map.width + rifle_tile.0) as usize;
    assert_eq!(
        scout_snapshot.visible_tiles[rifle_tile_index], 1,
        "revealing a concealed unit must not darken terrain the scout already sees"
    );
    assert!(
        game.snapshot_for_observer(&ObserverView::Players(vec![1]))
            .entities
            .iter()
            .any(|view| view.id == rifles[0]),
        "a player-perspective observer should receive that player's firing reveal"
    );

    while game.tick_count() < reveal_started + config::TICK_HZ / 2 - 1 {
        game.tick();
        assert_eq!(
            game.state
                .entities
                .get(rifles[0])
                .expect("first rifleman should survive the reaction window")
                .hp,
            rifle_hp_before,
            "counterfire should wait for the half-second reveal reaction delay"
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
