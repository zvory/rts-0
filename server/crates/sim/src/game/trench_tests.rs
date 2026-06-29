use super::*;
use crate::game::entity::EntityKind;
use crate::game::{services, systems, SmokeCloudStore};
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
            is_ai: false,
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
            is_ai: false,
        },
    ]
}

fn empty_flat_game(players: &[PlayerInit]) -> Game {
    let mut game = Game::new_for_replay(players, 0xA117_4E11);
    for tile in &mut game.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    game.smokes = SmokeCloudStore::new();
    game.mortar_shells = MortarShellStore::default();
    game.artillery_shells = artillery::ArtilleryShellStore::default();
    repair_world(&mut game);
    game
}

fn repair_world(game: &mut Game) {
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.recompute_live_fog(&ids);
    game.refresh_building_memory(&ids);
    game.refresh_trench_memory(&ids);
}

#[test]
fn seeded_trenches_persist_and_project_to_full_world_snapshots() {
    let mut game = empty_flat_game(&players());
    let trench_pos = game.map.tile_center(24, 24);
    let trench = game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");

    assert!(
        game.snapshot_for(1).trenches.is_empty(),
        "player with no current or remembered vision should not receive hidden trench terrain"
    );
    assert!(game
        .snapshot_full_for(1)
        .trenches
        .iter()
        .any(|view| view.id == trench && (view.x, view.y) == trench_pos));

    game.tick();

    assert!(game
        .snapshot_full_for(1)
        .trenches
        .iter()
        .any(|view| view.id == trench && (view.x, view.y) == trench_pos));
}

#[test]
fn trench_projection_uses_visibility_then_remembered_terrain() {
    let mut game = empty_flat_game(&players());
    let scout_pos = game.map.tile_center(20, 20);
    let far_pos = game.map.tile_center(4, 50);
    let hidden_pos = game.map.tile_center(50, 50);
    let scout = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    repair_world(&mut game);

    let visible_trench = game
        .spawn_trench_for_test(scout_pos.0, scout_pos.1)
        .expect("visible trench should seed");
    let hidden_trench = game
        .spawn_trench_for_test(hidden_pos.0, hidden_pos.1)
        .expect("hidden trench should seed");

    let visible = game.snapshot_for(1);
    assert!(visible
        .trenches
        .iter()
        .any(|view| view.id == visible_trench));
    assert!(!visible.trenches.iter().any(|view| view.id == hidden_trench));

    game.entities.remove(scout);
    game.entities
        .spawn_unit(1, EntityKind::Rifleman, far_pos.0, far_pos.1)
        .expect("far scout should spawn");
    game.tick();

    let remembered = game.snapshot_for(1);
    assert!(
        remembered
            .trenches
            .iter()
            .any(|view| view.id == visible_trench),
        "discovered trench terrain should remain visible after it falls back into fog"
    );
    assert!(!remembered
        .trenches
        .iter()
        .any(|view| view.id == hidden_trench));
}

#[test]
fn spectator_projection_uses_selected_player_trench_vision() {
    let mut game = empty_flat_game(&three_players());
    let p1_base = game.map.tile_center(3, 3);
    let p2_base = game.map.tile_center(55, 3);
    let p3_base = game.map.tile_center(3, 55);
    game.entities
        .spawn_building(1, EntityKind::CityCentre, p1_base.0, p1_base.1, true)
        .expect("p1 base should spawn");
    game.entities
        .spawn_building(2, EntityKind::CityCentre, p2_base.0, p2_base.1, true)
        .expect("p2 base should spawn");
    game.entities
        .spawn_building(3, EntityKind::CityCentre, p3_base.0, p3_base.1, true)
        .expect("p3 base should spawn");
    let scout_pos = game.map.tile_center(32, 32);
    game.entities
        .spawn_unit(2, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("p2 scout should spawn");
    repair_world(&mut game);
    let trench = game
        .spawn_trench_for_test(scout_pos.0, scout_pos.1)
        .expect("trench should seed");

    assert!(game
        .snapshot_for_spectator(&[2])
        .trenches
        .iter()
        .any(|view| view.id == trench));
    assert!(!game
        .snapshot_for_spectator(&[1])
        .trenches
        .iter()
        .any(|view| view.id == trench));
    assert!(game
        .snapshot_for_spectator(&[1, 2])
        .trenches
        .iter()
        .any(|view| view.id == trench));
}
