use super::*;
use crate::game::entity::{EntityKind, WeaponSetup};
use crate::game::{systems, SmokeCloudStore};
use crate::protocol::terrain;

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
            is_ai: true,
        },
    ]
}

fn three_players() -> [PlayerInit; 3] {
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
            color: "#bbb".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: true,
        },
    ]
}

fn empty_flat_game() -> Game {
    empty_flat_game_with_players(&players())
}

fn empty_flat_game_with_players(players: &[PlayerInit]) -> Game {
    let mut game = Game::new_for_replay(players, 0x1234_5678);
    for tile in &mut game.state.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    game.state.smokes = SmokeCloudStore::new();
    game.state.mortar_shells = MortarShellStore::default();
    game.state.artillery_shells = artillery::ArtilleryShellStore::default();
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    game.refresh_building_memory(&ids);
    game.refresh_anti_tank_gun_memory(&ids);
    game
}

fn advance_to_next_fog_refresh(game: &mut Game) {
    loop {
        game.tick();
        if game.tick_count().is_multiple_of(FOG_UPDATE_INTERVAL_TICKS) {
            break;
        }
    }
}

#[test]
fn exposes_hidden_remembered_building_without_live_entity() {
    let mut game = empty_flat_game();
    let scout_pos = game.state.map.tile_center(20, 20);
    let depot_pos = game.state.map.tile_center(22, 20);
    let scout = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    let depot = game
        .state
        .entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("depot should spawn");
    advance_to_next_fog_refresh(&mut game);

    let visible = game.snapshot_for(1);
    assert!(visible.entities.iter().any(|entity| entity.id == depot));
    assert!(visible
        .remembered_buildings
        .iter()
        .all(|building| building.id != depot));

    game.state.entities.remove(scout);
    let far = game.state.map.tile_center(40, 40);
    game.state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, far.0, far.1)
        .expect("far scout should spawn");
    advance_to_next_fog_refresh(&mut game);

    let hidden = game.snapshot_for(1);
    assert!(hidden.entities.iter().all(|entity| entity.id != depot));
    let remembered = hidden
        .remembered_buildings
        .iter()
        .find(|building| building.id == depot)
        .expect("scouted fogged building should be sent as stale intel");
    assert_eq!(remembered.owner, 2);
    assert_eq!(
        remembered.kind,
        crate::protocol::kind_to_wire(EntityKind::Depot)
    );
    assert_eq!((remembered.x, remembered.y), depot_pos);
    assert!(!remembered.footprint.is_empty());
}

#[test]
fn remembered_buildings_use_team_visible_observations() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#bbb".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = Game::new_for_replay(&players, 0xA11E_D001);
    for tile in &mut game.state.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    let p1_base = game.state.map.tile_center(2, 2);
    let p2_base = game.state.map.tile_center(4, 2);
    let p3_base = game.state.map.tile_center(55, 55);
    game.state
        .entities
        .spawn_building(1, EntityKind::CityCentre, p1_base.0, p1_base.1, true)
        .expect("p1 city centre should spawn");
    game.state
        .entities
        .spawn_building(2, EntityKind::CityCentre, p2_base.0, p2_base.1, true)
        .expect("p2 city centre should spawn");
    game.state
        .entities
        .spawn_building(3, EntityKind::CityCentre, p3_base.0, p3_base.1, true)
        .expect("p3 city centre should spawn");
    let scout_pos = game.state.map.tile_center(20, 20);
    let depot_pos = game.state.map.tile_center(22, 20);
    let scout = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("ally scout should spawn");
    let depot = game
        .state
        .entities
        .spawn_building(3, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("enemy depot should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    game.refresh_building_memory(&ids);

    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .any(|entity| entity.id == depot),
        "team current sight should initially project the live enemy building"
    );

    game.state.entities.remove(scout);
    let far = game.state.map.tile_center(40, 40);
    game.state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, far.0, far.1)
        .expect("far ally scout should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);

    let hidden = game.snapshot_for(1);
    assert!(hidden.entities.iter().all(|entity| entity.id != depot));
    assert!(
        hidden
            .remembered_buildings
            .iter()
            .any(|building| building.id == depot),
        "player 1 should receive stale memory from player 2's team-visible observation"
    );
}

#[test]
fn spectator_remembered_buildings_follow_selected_player_union() {
    let mut game = empty_flat_game_with_players(&three_players());
    let p1_scout_pos = game.state.map.tile_center(20, 20);
    let p2_scout_pos = game.state.map.tile_center(20, 20);
    let depot_pos = game.state.map.tile_center(22, 20);
    let p1_scout = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, p1_scout_pos.0, p1_scout_pos.1)
        .expect("p1 scout should spawn");
    let depot = game
        .state
        .entities
        .spawn_building(3, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("enemy depot should spawn");
    advance_to_next_fog_refresh(&mut game);

    game.state.entities.remove(p1_scout);
    let p2_scout = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, p2_scout_pos.0, p2_scout_pos.1)
        .expect("p2 scout should spawn");
    advance_to_next_fog_refresh(&mut game);

    game.state.entities.remove(p2_scout);
    advance_to_next_fog_refresh(&mut game);

    let p1_view = game.snapshot_for_spectator(&[1]);
    let p2_view = game.snapshot_for_spectator(&[2]);
    let union_view = game.snapshot_for_spectator(&[1, 2]);

    let p1_memory = p1_view
        .remembered_buildings
        .iter()
        .find(|building| building.id == depot)
        .expect("p1 memory should be projected into p1 replay vision");
    let p2_memory = p2_view
        .remembered_buildings
        .iter()
        .find(|building| building.id == depot)
        .expect("p2 memory should be projected into p2 replay vision");
    assert!(
        p2_memory.observed_tick > p1_memory.observed_tick,
        "test setup should create a newer p2 observation"
    );

    let union_memories = union_view
        .remembered_buildings
        .iter()
        .filter(|building| building.id == depot)
        .collect::<Vec<_>>();
    assert_eq!(
        union_memories.len(),
        1,
        "union projection should dedupe same-building memories"
    );
    assert_eq!(union_memories[0].observed_tick, p2_memory.observed_tick);
    assert_eq!(union_memories[0].owner, 3);
}

#[test]
fn does_not_expose_never_scouted_building_memory() {
    let mut game = empty_flat_game();
    let scout_pos = game.state.map.tile_center(4, 4);
    let depot_pos = game.state.map.tile_center(40, 40);
    game.state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    let depot = game
        .state
        .entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("depot should spawn");
    advance_to_next_fog_refresh(&mut game);

    let snapshot = game.snapshot_for(1);
    assert!(snapshot.entities.iter().all(|entity| entity.id != depot));
    assert!(snapshot
        .remembered_buildings
        .iter()
        .all(|building| building.id != depot));
}

#[test]
fn keeps_destroyed_hidden_building_as_stale_intel_until_scouted() {
    let mut game = empty_flat_game();
    let scout_pos = game.state.map.tile_center(8, 8);
    let depot_pos = game.state.map.tile_center(10, 8);
    let scout = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    let depot = game
        .state
        .entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("depot should spawn");
    advance_to_next_fog_refresh(&mut game);

    game.state.entities.remove(scout);
    let far = game.state.map.tile_center(40, 40);
    game.state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, far.0, far.1)
        .expect("far scout should spawn");
    let player_ids = game.state.player_ids();
    game.recompute_live_fog(&player_ids);
    game.state.entities.remove(depot);
    advance_to_next_fog_refresh(&mut game);

    let stale = game.snapshot_for(1);
    assert!(stale
        .remembered_buildings
        .iter()
        .any(|building| building.id == depot));

    game.state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, depot_pos.0, depot_pos.1)
        .expect("new scout should spawn");
    advance_to_next_fog_refresh(&mut game);

    let cleared = game.snapshot_for(1);
    assert!(cleared
        .remembered_buildings
        .iter()
        .all(|building| building.id != depot));
}

#[test]
fn observer_switches_restore_each_players_authoritative_anti_tank_gun_memory() {
    let mut game = empty_flat_game();
    let scout_pos = game.state.map.tile_center(20, 20);
    let gun_pos = game.state.map.tile_center(22, 20);
    let scout = game
        .state
        .entities
        .spawn_unit(2, EntityKind::ScoutCar, scout_pos.0, scout_pos.1)
        .expect("Bravo scout should spawn");
    let gun = game
        .state
        .entities
        .spawn_unit(1, EntityKind::AntiTankGun, gun_pos.0, gun_pos.1)
        .expect("Alpha anti-tank gun should spawn");
    let facing = 0.75;
    let gun_entity = game
        .state
        .entities
        .get_mut(gun)
        .expect("anti-tank gun should exist");
    gun_entity.set_weapon_setup(WeaponSetup::Deployed);
    gun_entity.set_emplacement_facing(Some(facing));
    gun_entity.set_facing(facing);
    gun_entity.set_weapon_facing(facing);
    advance_to_next_fog_refresh(&mut game);

    assert!(
        game.snapshot_for_observer(&ObserverView::Players(vec![2]))
            .entities
            .iter()
            .any(|entity| entity.id == gun),
        "Bravo should initially receive the live scouted gun"
    );

    game.state.entities.remove(scout);
    let far = game.state.map.tile_center(40, 40);
    game.state
        .entities
        .spawn_unit(2, EntityKind::ScoutCar, far.0, far.1)
        .expect("far Bravo scout should spawn");
    advance_to_next_fog_refresh(&mut game);

    let bravo_memory = game.snapshot_for_observer(&ObserverView::Players(vec![2]));
    let remembered = bravo_memory
        .remembered_anti_tank_guns
        .iter()
        .find(|memory| memory.id == gun)
        .expect("Bravo observer view should receive stale AT-gun memory");
    assert_eq!(remembered.owner, 1);
    assert_eq!(
        (remembered.x, remembered.y, remembered.facing),
        (gun_pos.0, gun_pos.1, facing)
    );
    assert!(
        game.snapshot_for_observer(&ObserverView::Players(vec![1]))
            .remembered_anti_tank_guns
            .iter()
            .all(|memory| memory.id != gun),
        "Alpha observer view must not receive a friendly threat memory"
    );
    let checkpoint = game
        .checkpoint_payload_text_for_test()
        .expect("AT-gun memory checkpoint should serialize");
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &checkpoint,
        game.state.map.clone(),
        game.map_metadata().clone(),
    )
    .expect("AT-gun memory checkpoint should restore");
    assert!(
        restored
            .snapshot_for_observer(&ObserverView::Players(vec![2]))
            .remembered_anti_tank_guns
            .iter()
            .any(|memory| memory.id == gun),
        "checkpoint restore should preserve per-player AT-gun memory for Lab time travel"
    );

    game.state
        .entities
        .get_mut(gun)
        .expect("anti-tank gun should exist")
        .set_weapon_setup(WeaponSetup::Packed);
    advance_to_next_fog_refresh(&mut game);
    assert!(
        game.snapshot_for_observer(&ObserverView::Players(vec![2]))
            .remembered_anti_tank_guns
            .iter()
            .any(|memory| memory.id == gun),
        "hidden Alpha teardown must not mutate Bravo's remembered knowledge"
    );

    game.state
        .entities
        .spawn_unit(2, EntityKind::ScoutCar, gun_pos.0, gun_pos.1)
        .expect("Bravo re-scout should spawn");
    advance_to_next_fog_refresh(&mut game);
    assert!(
        game.snapshot_for_observer(&ObserverView::Players(vec![2]))
            .remembered_anti_tank_guns
            .iter()
            .all(|memory| memory.id != gun),
        "seeing the remembered gun packed should clear Bravo's server memory"
    );
}
