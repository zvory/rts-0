use super::*;
use crate::game::{Game, ObserverView, PlayerInit};
use crate::rules::death_ground_decal_class;

fn one_player_game() -> Game {
    Game::new(
        &[PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        }],
        7,
    )
}

fn fog_with_visible_tile(player: u32, visible_index: Option<usize>) -> Fog {
    let mut grid = vec![false; 16];
    if let Some(index) = visible_index {
        grid[index] = true;
    }
    Fog::from_checkpoint_grids(
        4,
        4,
        BTreeMap::from([(player, grid)]),
        BTreeMap::new(),
        BTreeMap::new(),
    )
}

#[test]
fn store_keeps_marks_beyond_the_old_beta_cap() {
    let mut store = GroundDecalStore::new();
    let map = one_player_game().state.map;
    for _ in 0..4_097 {
        assert!(store.create_mortar_impact(&map, 32.0, 32.0).is_some());
    }
    assert_eq!(store.decals.first().map(|decal| decal.id), Some(1));
    assert_eq!(store.decals.last().map(|decal| decal.id), Some(4_097));
}

#[test]
fn stored_mark_has_no_heap_owned_vocabulary() {
    assert!(std::mem::size_of::<GroundDecal>() <= 64);
}

#[test]
fn hidden_mark_is_sent_only_after_first_physical_discovery() {
    let mut store = GroundDecalStore::new();
    let map = one_player_game().state.map;
    store.create_mortar_impact(&map, 48.0, 48.0).unwrap();

    store.refresh_memory_for_player(2, &fog_with_visible_tile(2, None), &map);
    assert_eq!(
        store.views_for_players_after(&[2], 0),
        (0, Vec::new(), Vec::new())
    );
    assert_eq!(
        store.recent_views_for_players(&[2], 64),
        (0, 0, Vec::new(), Vec::new())
    );

    store.refresh_memory_for_player(2, &fog_with_visible_tile(2, Some(5)), &map);
    let (revision, decals, trails) = store.views_for_players_after(&[2], 0);
    assert!(revision > 0);
    assert_eq!(decals.len(), 1);
    assert!(trails.is_empty());
    assert_eq!(decals[0].decal_class, "mortarBlast");
    let (fast_revision, fast_after, fast_decals, fast_trails) =
        store.recent_views_for_players(&[2], 64);
    assert_eq!(fast_revision, revision);
    assert_eq!(fast_after, 0);
    assert_eq!(fast_decals, decals);
    assert!(fast_trails.is_empty());
    assert_eq!(
        decals[0].owner, 0,
        "impact decals must not leak firing owner"
    );
    assert_eq!(
        store.views_for_players_after(&[2], revision),
        (revision, Vec::new(), Vec::new())
    );

    store.refresh_memory_for_player(2, &fog_with_visible_tile(2, Some(5)), &map);
    assert_eq!(store.revision_for_players(&[2]), revision);
    store.begin_tick(1);
    assert_eq!(
        store.recent_views_for_players(&[2], 64),
        (fast_revision, fast_revision, Vec::new(), Vec::new()),
        "a later tick advertises the cursor without repeating old rows"
    );
}

#[test]
fn current_tick_delta_cursor_skips_hidden_global_revision_gaps() {
    assert_eq!(
        revision_log::current_delta_after_for_test(50, &[10, 50], &[50], 64),
        10,
        "the delta must begin at the perspective cursor, not global revision 49"
    );
    assert_eq!(
        revision_log::current_delta_after_for_test(50, &[10, 50], &[], 64),
        50,
        "ticks with no newly entitled rows should carry no delta"
    );
}

#[test]
fn in_place_pivot_finalizes_as_one_sparse_trail_chunk() {
    let mut game = one_player_game();
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, 96.0, 96.0)
        .unwrap();

    game.state.ground_decals.begin_tick(1);
    game.state
        .ground_decals
        .update_tank_trails(&game.state.entities, &game.state.map, 1);
    game.state
        .entities
        .get_mut(tank)
        .unwrap()
        .set_facing(std::f32::consts::FRAC_PI_2);
    for tick in 2..=4 {
        game.state.ground_decals.begin_tick(tick);
        game.state
            .ground_decals
            .update_tank_trails(&game.state.entities, &game.state.map, tick);
    }

    assert_eq!(game.state.ground_decals.tank_trails.finalized_len(), 1);
    let (_, decals, trails) = game.state.ground_decals.full_world_views_after(0);
    assert!(decals.is_empty());
    assert_eq!(trails.len(), 1);
    assert_eq!(trails[0].poses.len(), 2);
    assert_eq!(trails[0].poses[0][..2], trails[0].poses[1][..2]);
    assert_ne!(trails[0].poses[0][2], trails[0].poses[1][2]);
}

#[test]
fn diagonal_motion_does_not_settle_at_the_checkpoint_quantum() {
    let mut game = one_player_game();
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, 96.0, 96.0)
        .unwrap();

    game.state.ground_decals.begin_tick(1);
    game.state
        .ground_decals
        .update_tank_trails(&game.state.entities, &game.state.map, 1);
    let diagonal_step = 2.0_f32 / 2.0_f32.sqrt();
    for tick in 2..=10 {
        let entity = game.state.entities.get_mut(tank).unwrap();
        entity.set_position(entity.pos_x + diagonal_step, entity.pos_y + diagonal_step);
        entity.set_movement_delta(diagonal_step, diagonal_step);
        game.state.ground_decals.begin_tick(tick);
        game.state
            .ground_decals
            .update_tank_trails(&game.state.entities, &game.state.map, tick);
    }

    assert_eq!(
        game.state.ground_decals.tank_trails.finalized_len(),
        0,
        "continuous sub-quantum diagonal motion must remain one active trail"
    );
    game.state
        .entities
        .get_mut(tank)
        .unwrap()
        .set_movement_delta(0.0, 0.0);
    for tick in 11..=12 {
        game.state.ground_decals.begin_tick(tick);
        game.state
            .ground_decals
            .update_tank_trails(&game.state.entities, &game.state.map, tick);
    }
    assert_eq!(game.state.ground_decals.tank_trails.finalized_len(), 1);
}

#[test]
fn player_delta_is_fog_safe_while_full_world_gets_created_marks() {
    let mut store = GroundDecalStore::new();
    let map = one_player_game().state.map;
    store.create_artillery_impact(&map, 48.0, 48.0).unwrap();
    store.refresh_memory_for_player(1, &fog_with_visible_tile(1, Some(5)), &map);
    store.refresh_memory_for_player(2, &fog_with_visible_tile(2, None), &map);

    assert_eq!(
        store.views_for_players_after(&[2], 0),
        (0, Vec::new(), Vec::new())
    );
    assert!(store.recent_views_for_players(&[2], 64).2.is_empty());
    assert_eq!(store.views_for_players_after(&[1], 0).1.len(), 1);
    assert_eq!(store.recent_views_for_players(&[1], 64).2.len(), 1);
    assert_eq!(store.full_world_views_after(0).1.len(), 1);
    assert_eq!(store.recent_full_world_views(64).2.len(), 1);
}

#[test]
fn recent_player_delta_is_revision_bounded_and_complete_after_its_cursor() {
    let mut store = GroundDecalStore::new();
    let map = one_player_game().state.map;
    for offset in 0..70 {
        store
            .create_mortar_impact(&map, 48.0 + offset as f32 * 0.01, 48.0)
            .unwrap();
    }
    store.refresh_memory_for_player(1, &fog_with_visible_tile(1, Some(5)), &map);

    let (revision, after_revision, decals, trails) = store.recent_views_for_players(&[1], 64);
    assert_eq!(after_revision, revision - 64);
    assert_eq!(decals.len(), 64);
    assert!(trails.is_empty());
    assert_eq!(
        decals,
        store.views_for_players_after(&[1], after_revision).1,
        "the advertised range must contain every entitled row after its cursor"
    );
    assert!(decals.iter().all(|decal| decal.owner == 0));
}

#[test]
fn game_checkpoint_round_trip_preserves_marks_and_discovery_revisions() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".to_string(),
            color: "#000".to_string(),
            is_ai: false,
        },
    ];
    let mut game = Game::new(&players, 0xdec0_1234);
    let source = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && death_ground_decal_class(entity.kind).is_some())
        .expect("player one worker")
        .clone();
    game.state
        .ground_decals
        .create_death(
            source.kind,
            source.pos_x,
            source.pos_y,
            source.owner,
            Some(source.facing()),
            source.weapon_facing(),
        )
        .unwrap();
    game.refresh_ground_decal_memory(&[1, 2]);
    let before_player = game.ground_decals_for_player(1, 0);
    let before_full = game.ground_decals_for_observer(&ObserverView::Omniscient, 0);
    assert_eq!(before_player.1.len(), 1);

    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.map_metadata().clone(),
    )
    .unwrap();
    assert_eq!(restored.ground_decals_for_player(1, 0), before_player);
    assert_eq!(
        restored.ground_decals_for_observer(&ObserverView::Omniscient, 0),
        before_full
    );
}

#[test]
fn checkpoint_restore_rebuilds_spatial_discovery_index() {
    let mut game = one_player_game();
    game.state
        .ground_decals
        .create_mortar_impact(&game.state.map, 48.0, 48.0)
        .unwrap();
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.map_metadata().clone(),
    )
    .unwrap();

    restored.state.ground_decals.refresh_memory_for_player(
        1,
        &fog_with_visible_tile(1, Some(5)),
        &restored.state.map,
    );

    assert_eq!(restored.ground_decals_for_player(1, 0).1.len(), 1);
}

#[test]
fn checkpoint_round_trip_accepts_a_finalized_tank_trail() {
    let mut game = one_player_game();
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, 96.0, 96.0)
        .unwrap();
    game.state.ground_decals.begin_tick(1);
    game.state
        .ground_decals
        .update_tank_trails(&game.state.entities, &game.state.map, 1);
    game.state
        .entities
        .get_mut(tank)
        .unwrap()
        .set_facing(std::f32::consts::FRAC_PI_2);
    for tick in 2..=4 {
        game.state.tick = tick;
        game.state.ground_decals.begin_tick(tick);
        game.state
            .ground_decals
            .update_tank_trails(&game.state.entities, &game.state.map, tick);
    }
    crate::game::services::supply::recompute_supply(&mut game.state.players, &game.state.entities);

    assert_eq!(game.ground_decals_for_player(1, 0).2.len(), 1);

    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.map_metadata().clone(),
    )
    .unwrap();
    assert_eq!(
        restored
            .ground_decals_for_observer(&ObserverView::Omniscient, 0)
            .2
            .len(),
        1
    );
}

#[test]
fn hidden_tank_trail_is_discovered_later_and_survives_checkpoint_restore() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".to_string(),
            color: "#000".to_string(),
            is_ai: false,
        },
    ];
    let mut game = Game::new(&players, 7);
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, 96.0, 96.0)
        .unwrap();
    game.state.ground_decals.begin_tick(1);
    game.state
        .ground_decals
        .update_tank_trails(&game.state.entities, &game.state.map, 1);
    game.state
        .entities
        .get_mut(tank)
        .unwrap()
        .set_facing(std::f32::consts::FRAC_PI_2);
    for tick in 2..=4 {
        game.state.tick = tick;
        game.state.ground_decals.begin_tick(tick);
        game.state
            .ground_decals
            .update_tank_trails(&game.state.entities, &game.state.map, tick);
    }
    crate::game::services::supply::recompute_supply(&mut game.state.players, &game.state.entities);

    game.state.ground_decals.refresh_memory_for_player(
        2,
        &fog_with_visible_tile(2, None),
        &game.state.map,
    );
    assert!(game.ground_decals_for_player(2, 0).2.is_empty());
    let fully_visible = Fog::from_checkpoint_grids(
        4,
        4,
        BTreeMap::from([(2, vec![true; 16])]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    game.state
        .ground_decals
        .refresh_memory_for_player(2, &fully_visible, &game.state.map);
    assert_eq!(game.ground_decals_for_player(2, 0).2.len(), 1);

    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.map_metadata().clone(),
    )
    .unwrap();
    assert_eq!(restored.ground_decals_for_player(1, 0).2.len(), 1);
    assert_eq!(restored.ground_decals_for_player(2, 0).2.len(), 1);
}

#[test]
fn checkpoint_rejects_noncanonical_blast_radius() {
    let mut game = one_player_game();
    game.state
        .ground_decals
        .create_mortar_impact(&game.state.map, 48.0, 48.0)
        .unwrap();
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&payload).unwrap();
    value["groundDecals"]["decals"][0]["radiusTiles"] = serde_json::json!(999.0);
    let malformed = serde_json::to_string(&value).unwrap();
    let result = Game::restore_checkpoint_payload_text_for_test(
        &malformed,
        game.state.map.clone(),
        game.map_metadata().clone(),
    );
    assert!(
        result.is_err(),
        "noncanonical blast radii must reject restore"
    );
}

#[test]
fn checkpoint_rejects_noncanonical_revision_gaps() {
    let mut game = one_player_game();
    game.state
        .ground_decals
        .create_mortar_impact(&game.state.map, 48.0, 48.0)
        .unwrap();
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&payload).unwrap();
    value["groundDecals"]["revision"] = serde_json::json!(u32::MAX);
    let malformed = serde_json::to_string(&value).unwrap();
    let result = Game::restore_checkpoint_payload_text_for_test(
        &malformed,
        game.state.map.clone(),
        game.map_metadata().clone(),
    );
    assert!(result.is_err(), "revision gaps must reject restore");
}

#[test]
fn edge_blasts_keep_checkpoint_state_canonical() {
    let mut game = one_player_game();
    game.state
        .ground_decals
        .create_mortar_impact(&game.state.map, -16.0, 48.0)
        .unwrap();
    assert!(
        game.state
            .ground_decals
            .create_mortar_impact(&game.state.map, -200.0, 48.0)
            .is_none(),
        "a fully off-map blast has no visible mark to retain"
    );

    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.map_metadata().clone(),
    );

    assert!(
        restored.is_ok(),
        "a legal edge impact must not make the server's own checkpoint unrestorable"
    );
}
