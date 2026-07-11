use super::entity::EntityKind;
use super::lab::{LabError, LabOp, LabOpOutcome, LabSetEntityOwner, LabSpawnEntity};
use super::map::{Map, MapMetadata};
use super::services::occupancy::footprint_center;
use super::{Game, PlayerInit};
use crate::protocol::terrain;

fn lab_players() -> [PlayerInit; 2] {
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

fn lab_metadata() -> MapMetadata {
    MapMetadata {
        name: "Test".to_string(),
        schema_version: 1,
        content_hash: "test-map".to_string(),
    }
}

fn flat_lab_map() -> Map {
    const SIZE: u32 = 64;
    Map {
        size: SIZE,
        terrain: vec![terrain::GRASS; (SIZE * SIZE) as usize],
        starts: vec![(16, 16), (48, 48)],
        base_sites: Vec::new(),
    }
}

fn new_game() -> Game {
    Game::new_lab(&lab_players(), 0xABCD, flat_lab_map(), lab_metadata())
}

fn tile_center(game: &Game, x: u32, y: u32) -> (f32, f32) {
    game.state.map.tile_center(x, y)
}

#[test]
fn lab_god_mode_makes_player_units_and_buildings_ignore_damage() {
    let mut game = new_game();
    let worker_id = game.state.entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
        .expect("starting worker")
        .id;
    let worker_hp = game.state.entities.get(worker_id).expect("worker").hp;
    let (node_x, node_y) = tile_center(&game, 42, 42);
    let node_id = game.state.entities
        .spawn_node(EntityKind::Steel, node_x, node_y)
        .expect("steel node should spawn");

    game.apply_lab_op(LabOp::SetPlayerGodMode {
        player_id: 1,
        enabled: true,
    })
    .expect("god mode should enable");

    let worker = game.state.entities.get_mut(worker_id).expect("worker");
    assert!(worker.invulnerable());
    assert!(!worker.apply_damage(10, Some((2, (0.0, 0.0), 1))));
    assert_eq!(worker.hp, worker_hp);

    let (unit_x, unit_y) = tile_center(&game, 30, 30);
    let LabOpOutcome::Spawned { entity_id: unit_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Rifleman,
            x: unit_x,
            y: unit_y,
            completed: true,
        }))
        .expect("rifleman should spawn")
    else {
        panic!("unexpected outcome");
    };
    assert!(game.state.entities.get(unit_id).expect("rifleman").invulnerable());

    let (building_x, building_y) = footprint_center(&game.state.map, EntityKind::Depot, 34, 34);
    let LabOpOutcome::Spawned {
        entity_id: depot_id,
    } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Depot,
            x: building_x,
            y: building_y,
            completed: true,
        }))
        .expect("depot should spawn")
    else {
        panic!("unexpected outcome");
    };
    let depot = game.state.entities.get_mut(depot_id).expect("depot");
    let depot_hp = depot.hp;
    assert!(depot.invulnerable());
    assert!(!depot.apply_damage(10, Some((2, (0.0, 0.0), 1))));
    assert_eq!(depot.hp, depot_hp);

    let node = game.state.entities.get(node_id).expect("steel node");
    assert!(!node.invulnerable());

    game.apply_lab_op(LabOp::SetPlayerGodMode {
        player_id: 1,
        enabled: false,
    })
    .expect("god mode should disable");
    let worker = game.state.entities.get_mut(worker_id).expect("worker");
    assert!(!worker.invulnerable());
    assert!(worker.apply_damage(10, Some((2, (0.0, 0.0), 2))));
    assert_eq!(worker.hp, worker_hp - 10);

    let depot = game.state.entities.get_mut(depot_id).expect("depot");
    assert!(!depot.invulnerable());
    assert!(depot.apply_damage(10, Some((2, (0.0, 0.0), 2))));
    assert_eq!(depot.hp, depot_hp - 10);
}

#[test]
fn lab_god_mode_follows_unit_owner_changes() {
    let mut game = new_game();
    game.apply_lab_op(LabOp::SetPlayerGodMode {
        player_id: 2,
        enabled: true,
    })
    .expect("god mode should enable");

    let (x, y) = tile_center(&game, 30, 30);
    let LabOpOutcome::Spawned { entity_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Tank,
            x,
            y,
            completed: true,
        }))
        .expect("tank should spawn")
    else {
        panic!("unexpected outcome");
    };
    assert!(!game.state.entities.get(entity_id).expect("tank").invulnerable());

    game.apply_lab_op(LabOp::SetEntityOwner(LabSetEntityOwner {
        entity_id,
        owner: 2,
    }))
    .expect("owner should change");
    assert!(game.state.entities.get(entity_id).expect("tank").invulnerable());

    assert!(matches!(
        game.apply_lab_op(LabOp::SetPlayerGodMode {
            player_id: 999,
            enabled: true,
        }),
        Err(LabError::InvalidPlayer { player_id: 999 })
    ));
}
