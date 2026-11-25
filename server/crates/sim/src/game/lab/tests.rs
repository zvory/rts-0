use super::*;
use crate::game::entity::{PanzerfaustState, WeaponSetup};
use crate::game::services::occupancy::footprint_center;
use crate::protocol::{terrain, LabBaseSite, LabMapTile, MapDoodad};

fn lab_players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Alpha".to_string(),
            color: "#4878c8".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Bravo".to_string(),
            color: "#c84848".to_string(),
            is_ai: false,
        },
    ]
}

fn lab_metadata() -> MapMetadata {
    MapMetadata {
        name: "Chokes".to_string(),
        schema_version: crate::game::map::CURRENT_MAP_VERSION,
        content_hash: "test-map".to_string(),
    }
}

fn flat_lab_map() -> Map {
    const SIZE: u32 = 64;
    Map {
        width: SIZE,
        height: SIZE,
        terrain: vec![terrain::GRASS; (SIZE * SIZE) as usize],
        starts: vec![(16, 16), (48, 48)],
        ..Default::default()
    }
}

fn new_game() -> Game {
    Game::new_lab(&lab_players(), 0xABCD, flat_lab_map(), lab_metadata())
}

fn map_draft() -> LabMapDraft {
    let mut terrain = vec![terrain::GRASS; 64 * 64];
    terrain[0] = terrain::WATER;
    LabMapDraft {
        name: "Edited Lab Map".to_string(),
        width: 64,
        height: 64,
        terrain,
        elevation: Vec::new(),
        sun: None,
        starts: vec![LabMapTile { x: 12, y: 12 }, LabMapTile { x: 51, y: 51 }],
        base_sites: vec![LabBaseSite {
            x: 32,
            y: 32,
            steel_patches: 7,
            oil_patches: 2,
        }],
        concealment_tiles: Vec::new(),
        no_vehicle_tiles: Vec::new(),
        no_building_tiles: Vec::new(),
        no_entrenchment_tiles: Vec::new(),
        damage_reduction_tiles: Vec::new(),
        slow_movement_tiles: Vec::new(),
        unwalkable_tiles: Vec::new(),
        doodads: vec![
            MapDoodad {
                id: 9,
                type_id: "wildflower.cluster".to_string(),
                x: 900,
                y: 920,
                color: Some("#e05a91".to_string()),
            },
            MapDoodad {
                id: 3,
                type_id: "tree.spruce".to_string(),
                x: 800,
                y: 820,
                color: None,
            },
        ],
    }
}

#[test]
fn lab_map_draft_rebuilds_the_battle_on_authoritative_terrain_and_bases() {
    let mut game = new_game();
    for _ in 0..10 {
        game.tick();
    }

    let outcome = game
        .apply_lab_op(LabOp::ApplyMapDraft(Box::new(map_draft())))
        .expect("valid lab map draft");

    assert_eq!(
        outcome,
        LabOpOutcome::MapDraftApplied {
            name: "Edited Lab Map".to_string(),
            width: 64,
            height: 64,
            battle_reset: true,
        }
    );
    assert_eq!(game.tick_count(), 0);
    assert_eq!(game.state.map.terrain[0], terrain::WATER);
    assert_eq!(game.state.map.starts, vec![(12, 12), (51, 51)]);
    assert_eq!(game.state.map.base_sites, vec![(32, 32)]);
    assert_eq!(game.state.map.resource_counts_at((32, 32)).steel_patches, 7);
    assert_eq!(game.state.map.resource_counts_at((32, 32)).oil_patches, 2);
    assert_eq!(game.export_lab_map().base_sites[0].steel_patches, 7);
    assert_eq!(game.export_lab_map().base_sites[0].oil_patches, 2);
    assert_eq!(
        game.state
            .map
            .doodads
            .iter()
            .map(|doodad| doodad.id)
            .collect::<Vec<_>>(),
        vec![3, 9]
    );
    assert_eq!(game.export_lab_map().doodads, game.state.map.doodads);
    assert_eq!(game.start_payload().map.doodads, game.state.map.doodads);
    assert_eq!(game.state.map_metadata.name, "Edited Lab Map");
    assert_eq!(
        game.start_payload()
            .players
            .iter()
            .map(|player| (player.start_tile_x, player.start_tile_y))
            .collect::<Vec<_>>(),
        vec![(12, 12), (51, 51)]
    );
}

#[test]
fn lab_map_draft_preserves_visual_open_terrain_variants() {
    let mut game = new_game();
    let mut draft = map_draft();
    let variants = [
        terrain::GRAVEL_A,
        terrain::GRAVEL_B,
        terrain::GRAVEL_C,
        terrain::DIRT_A,
        terrain::DIRT_B,
        terrain::DIRT_C,
        terrain::MUD_A,
        terrain::MUD_B,
        terrain::MUD_C,
        terrain::FROSTED_GROUND,
    ];
    draft.terrain[..variants.len()].copy_from_slice(&variants);

    game.apply_lab_op(LabOp::ApplyMapDraft(Box::new(draft)))
        .expect("visual open terrain draft");

    assert_eq!(&game.state.map.terrain[..variants.len()], &variants);
    assert_eq!(&game.export_lab_map().terrain[..variants.len()], &variants);
    assert_eq!(
        &game.start_payload().map.terrain[..variants.len()],
        &variants
    );
}

#[test]
fn lab_map_draft_rejects_duplicate_base_resource_records() {
    let mut game = new_game();
    let mut draft = map_draft();
    draft.base_sites.push(LabBaseSite {
        x: 32,
        y: 32,
        steel_patches: 8,
        oil_patches: 1,
    });

    let error = game
        .apply_lab_op(LabOp::ApplyMapDraft(Box::new(draft)))
        .expect_err("duplicate base coordinates must not silently overwrite resource counts");
    assert!(matches!(
        error,
        LabError::InvalidMap { reason, .. } if reason.contains("duplicate base sites")
    ));
}

#[test]
fn lab_map_draft_rejects_blocked_base_protection_area() {
    let mut game = new_game();
    let mut draft = map_draft();
    draft.terrain[12 * 64 + 16] = terrain::ROCK;

    assert!(matches!(
        game.apply_lab_op(LabOp::ApplyMapDraft(Box::new(draft))),
        Err(LabError::InvalidMap { reason, .. })
            if reason.contains("protected area")
    ));
}

#[test]
fn lab_map_draft_checks_unwalkable_clearance_without_changing_terrain() {
    let protection_radius = super::super::setup::starting_protection_radius_tiles(&lab_players());
    let start_edge_x = 12 + u32::try_from(protection_radius).expect("non-negative radius");
    for (x, y, blocked) in [
        (start_edge_x, 12, true),
        (32, 32, true),
        (start_edge_x + 1, 12, false),
    ] {
        let mut game = new_game();
        let original_hash = game.state.map.materialized_hash();
        let mut draft = map_draft();
        draft.unwalkable_tiles = vec![crate::protocol::MapTile { x, y }];
        let terrain = draft.terrain.clone();
        let result = game.apply_lab_op(LabOp::ApplyMapDraft(Box::new(draft)));
        if blocked {
            assert!(matches!(result,
                Err(LabError::InvalidMap { reason, .. }) if reason.contains("protected area")
            ));
            assert_eq!(game.state.map.materialized_hash(), original_hash);
        } else {
            result.expect("unwalkable tile beyond the protected area is legal");
            assert_eq!(game.state.map.terrain, terrain);
            assert!(!game.state.map.is_passable(x as i32, y as i32));
        }
    }
}

#[test]
fn lab_map_draft_allows_terrain_immediately_beyond_starting_unit_area() {
    let mut game = new_game();
    let mut draft = map_draft();
    let protection_radius = super::super::setup::starting_protection_radius_tiles(&lab_players());
    let rock_x = 12 + u32::try_from(protection_radius).expect("non-negative protection radius") + 1;
    draft.terrain[12 * 64 + rock_x as usize] = terrain::ROCK;

    game.apply_lab_op(LabOp::ApplyMapDraft(Box::new(draft)))
        .expect("terrain beyond the starting unit area should remain editable");
    assert_eq!(
        game.state.map.terrain[12 * 64 + rock_x as usize],
        terrain::ROCK
    );
}

#[test]
fn terrain_only_lab_map_draft_restarts_a_fresh_test() {
    let mut game = new_game();
    let worker_id = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
        .map(|entity| entity.id)
        .expect("starting worker");
    let (worker_x, worker_y) = game.state.map.tile_center(30, 30);
    game.apply_lab_op(LabOp::MoveEntity(LabMoveEntity {
        entity_id: worker_id,
        x: worker_x,
        y: worker_y,
    }))
    .expect("move worker to map center");
    for _ in 0..10 {
        game.tick();
    }
    assert!(game.tick_count() > 0);
    let mut terrain = game.state.map.terrain.clone();
    for y in 29..=31 {
        for x in 30..=32 {
            terrain[y * 64 + x] = terrain::ROCK;
        }
    }
    let draft = LabMapDraft {
        name: "Terrain-only edit".to_string(),
        width: 64,
        height: 64,
        terrain,
        elevation: Vec::new(),
        sun: None,
        starts: game
            .state
            .map
            .starts
            .iter()
            .map(|&(x, y)| LabMapTile { x, y })
            .collect(),
        base_sites: Vec::new(),
        concealment_tiles: Vec::new(),
        no_vehicle_tiles: Vec::new(),
        no_building_tiles: Vec::new(),
        no_entrenchment_tiles: Vec::new(),
        damage_reduction_tiles: Vec::new(),
        slow_movement_tiles: Vec::new(),
        unwalkable_tiles: Vec::new(),
        doodads: Vec::new(),
    };

    let outcome = game
        .apply_lab_op(LabOp::ApplyMapDraft(Box::new(draft)))
        .expect("terrain-only edit");

    assert_eq!(
        outcome,
        LabOpOutcome::MapDraftApplied {
            name: "Terrain-only edit".to_string(),
            width: 64,
            height: 64,
            battle_reset: true,
        }
    );
    assert_eq!(game.tick_count(), 0);
    assert_eq!(game.state.map.terrain[30 * 64 + 31], terrain::ROCK);
    assert!(
        game.state
            .entities
            .iter()
            .filter(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
            .all(|worker| (worker.pos_x(), worker.pos_y()) != (worker_x, worker_y)),
        "a fresh test must not retain the moved worker from the previous run"
    );
}

fn default_map_game() -> Game {
    let players = lab_players();
    let start_players: Vec<_> = players
        .iter()
        .map(|player| (player.id, player.team_id))
        .collect();
    let map = Map::load_for_players("Chokes", &start_players, 0xABCD).expect("default lab map");
    let metadata = Map::metadata_for_name("Chokes").expect("default map metadata");
    Game::new_lab(&players, 0xABCD, map, metadata)
}

fn tile_center(game: &Game, x: u32, y: u32) -> (f32, f32) {
    game.state.map.tile_center(x, y)
}

fn assert_angle_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected angle {expected:.4}, got {actual:.4}"
    );
}

fn free_unit_position(game: &Game, kind: EntityKind) -> (f32, f32) {
    for ty in 8..game.state.map.width.saturating_sub(8) {
        for tx in 8..game.state.map.width.saturating_sub(8) {
            let (x, y) = game.state.map.tile_center(tx, ty);
            if game
                .validate_unit_position(&game.state.entities, kind, x, y)
                .is_ok()
            {
                return (x, y);
            }
        }
    }
    panic!("no free position found for {kind:?}");
}

#[test]
fn lab_spawn_unit_repairs_supply_and_snapshot_fog() {
    let mut game = new_game();
    let before_supply = game.snapshot_for(1).supply_used;
    let (enemy_x, enemy_y) = tile_center(&game, 35, 35);
    let enemy = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 2,
            kind: EntityKind::Depot,
            x: enemy_x,
            y: enemy_y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("enemy building should spawn");
    let LabOpOutcome::Spawned {
        entity_id: enemy_id,
    } = enemy
    else {
        panic!("unexpected outcome");
    };

    assert!(
        !game
            .snapshot_for(1)
            .entities
            .iter()
            .any(|entity| entity.id == enemy_id),
        "enemy building should start outside player 1 fog"
    );

    let (scout_x, scout_y) = tile_center(&game, 30, 35);
    let spawned = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::ScoutCar,
            x: scout_x,
            y: scout_y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("scout should spawn");
    let LabOpOutcome::Spawned { entity_id } = spawned else {
        panic!("unexpected outcome");
    };
    let snapshot = game.snapshot_for(1);
    assert!(snapshot
        .entities
        .iter()
        .any(|entity| entity.id == entity_id));
    assert!(snapshot.entities.iter().any(|entity| entity.id == enemy_id));
    assert_eq!(
        snapshot.supply_used,
        before_supply + rules::economy::supply_cost(EntityKind::ScoutCar)
    );
}

#[test]
fn lab_spawn_building_keeps_intrinsic_supply_cap() {
    let mut game = new_game();
    let before_cap = game.snapshot_for(1).supply_cap;
    let (x, y) = footprint_center(&game.state.map, EntityKind::Depot, 28, 28);

    game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
        owner: 1,
        kind: EntityKind::Depot,
        x,
        y,
        completed: true,
        dao: None,
        dao_master: false,
        faction_id: None,
    }))
    .expect("depot should spawn");

    assert_eq!(game.snapshot_for(1).supply_cap, before_cap);
}

#[test]
fn lab_spawn_rejects_nodes_invalid_owners_bad_positions_and_occupied_sites() {
    let mut game = new_game();
    let (x, y) = tile_center(&game, 30, 30);

    assert!(matches!(
        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Steel,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        })),
        Err(LabError::InvalidKind { .. })
    ));
    assert!(matches!(
        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 999,
            kind: EntityKind::Worker,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        })),
        Err(LabError::InvalidOwner { owner: 999 })
    ));
    assert!(matches!(
        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Worker,
            x: f32::NAN,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        })),
        Err(LabError::Placement { .. })
    ));

    let worker = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
        .expect("starting worker")
        .clone();
    assert!(matches!(
        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Rifleman,
            x: worker.pos_x(),
            y: worker.pos_y(),
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        })),
        Err(LabError::Placement { .. })
    ));

    let resource_depot = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::ResourceDepot)
        .expect("starting resource depot")
        .clone();
    assert!(matches!(
        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Depot,
            x: resource_depot.pos_x(),
            y: resource_depot.pos_y(),
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        })),
        Err(LabError::Placement { .. })
    ));
}

#[test]
fn lab_move_entity_validates_collision_and_repairs_position() {
    let mut game = new_game();
    let (x, y) = tile_center(&game, 30, 30);
    let LabOpOutcome::Spawned { entity_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Rifleman,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("rifleman should spawn")
    else {
        panic!("unexpected outcome");
    };

    let (move_x, move_y) = tile_center(&game, 31, 30);
    game.apply_lab_op(LabOp::MoveEntity(LabMoveEntity {
        entity_id,
        x: move_x,
        y: move_y,
    }))
    .expect("move should be accepted");
    let moved = game.state.entities.get(entity_id).expect("moved entity");
    assert_eq!((moved.pos_x(), moved.pos_y()), (move_x, move_y));

    let resource_depot = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::ResourceDepot)
        .expect("starting resource depot")
        .clone();
    assert!(matches!(
        game.apply_lab_op(LabOp::MoveEntity(LabMoveEntity {
            entity_id,
            x: resource_depot.pos_x(),
            y: resource_depot.pos_y(),
        })),
        Err(LabError::Placement { .. })
    ));
}

#[test]
fn lab_bulk_spawn_accepts_400_rejects_401_and_is_atomic() {
    let mut game = new_game();
    let initial_ids = game.state.entities.ids();
    game.lab_delete_entities(initial_ids)
        .expect("initial entities should delete as one batch");
    let spawns = (0..400)
        .map(|index| {
            let tx = 2 + (index % 20) as u32 * 3;
            let ty = 2 + (index / 20) as u32 * 3;
            let (x, y) = tile_center(&game, tx, ty);
            LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Rifleman,
                x,
                y,
                completed: true,
                dao: None,
                dao_master: false,
                faction_id: None,
            }
        })
        .collect::<Vec<_>>();
    let outcome = game
        .lab_spawn_entities(spawns.clone())
        .expect("400 valid spawns should commit");
    assert_eq!(outcome.len(), 400);
    let before_ids = game.state.entities.ids();
    let mut too_many = spawns;
    too_many.push(too_many[0]);
    assert!(matches!(
        game.lab_spawn_entities(too_many),
        Err(LabBatchError {
            error: LabError::BatchSize { count: 401, .. },
            ..
        })
    ));
    assert_eq!(game.state.entities.ids(), before_ids);
}

#[test]
fn lab_unit_spawn_planner_is_bounded_non_mutating_and_reserves_earlier_spawns() {
    let mut game = new_game();
    let before = game.state.entities.ids();
    let candidates = [tile_center(&game, 30, 30), tile_center(&game, 34, 30)];
    let planned = game
        .lab_plan_unit_spawns(
            &[(1, EntityKind::Rifleman), (1, EntityKind::Rifleman)],
            &candidates,
        )
        .expect("bounded planning should succeed");
    assert_eq!(planned.len(), 2);
    assert_ne!((planned[0].x, planned[0].y), (planned[1].x, planned[1].y));
    assert_eq!(
        game.state.entities.ids(),
        before,
        "planning must not mutate"
    );
    game.apply_lab_op(LabOp::SpawnEntities(planned))
        .expect("the planned atomic batch should remain valid");

    let too_many = vec![(0.0, 0.0); LAB_PLACEMENT_PLAN_CANDIDATE_LIMIT + 1];
    assert!(matches!(
        game.lab_plan_unit_spawns(&[(1, EntityKind::Rifleman)], &too_many),
        Err(LabError::BatchSize { .. })
    ));
    let owned = game.lab_owned_units(1).expect("authoritative roster");
    assert!(owned.iter().any(|(_, kind)| *kind == EntityKind::Rifleman));
}

#[test]
fn lab_bulk_spawn_failure_preserves_state_and_reports_index_with_suggestions() {
    let mut game = new_game();
    let city = game
        .state
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::ResourceDepot)
        .expect("resource depot")
        .clone();
    let (x, y) = tile_center(&game, 30, 30);
    let before = game.state.entities.ids();
    let request = vec![
        LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Rifleman,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        },
        LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Rifleman,
            x: city.pos_x(),
            y: city.pos_y(),
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        },
    ];
    let first = game
        .lab_spawn_entities(request.clone())
        .expect_err("blocked second spawn should reject all");
    let second = game
        .lab_spawn_entities(request)
        .expect_err("diagnostics should be deterministic");
    assert_eq!(first.failed_index, 1);
    assert_eq!(first, second);
    let LabError::Placement {
        blockers,
        suggestions,
        ..
    } = first.error
    else {
        panic!("expected placement diagnostics");
    };
    assert!(!blockers.is_empty());
    assert!(!suggestions.is_empty());
    assert!(suggestions.len() <= LAB_PLACEMENT_SUGGESTION_LIMIT);
    assert_eq!(game.state.entities.ids(), before);
    let suggestion = suggestions[0];
    game.lab_spawn_entities(vec![LabSpawnEntity {
        owner: 1,
        kind: EntityKind::Rifleman,
        x: suggestion.0,
        y: suggestion.1,
        completed: true,
        dao: None,
        dao_master: false,
        faction_id: None,
    }])
    .expect("authoritative suggestion should be accepted");
}

#[test]
fn lab_bulk_moves_validate_simultaneously_and_reject_conflicts_atomically() {
    let mut game = new_game();
    let (ax, ay) = tile_center(&game, 30, 30);
    let (bx, by) = tile_center(&game, 34, 30);
    let spawned = game
        .lab_spawn_entities(vec![
            LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Rifleman,
                x: ax,
                y: ay,
                completed: true,
                dao: None,
                dao_master: false,
                faction_id: None,
            },
            LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Rifleman,
                x: bx,
                y: by,
                completed: true,
                dao: None,
                dao_master: false,
                faction_id: None,
            },
        ])
        .expect("pair should spawn");
    let ids = spawned
        .iter()
        .map(|outcome| match outcome {
            LabOpOutcome::Spawned { entity_id } => *entity_id,
            _ => panic!("unexpected spawn outcome"),
        })
        .collect::<Vec<_>>();
    game.lab_apply_updates(vec![
        LabUpdate::Move(LabMoveEntity {
            entity_id: ids[0],
            x: bx,
            y: by,
        }),
        LabUpdate::Move(LabMoveEntity {
            entity_id: ids[1],
            x: ax,
            y: ay,
        }),
    ])
    .expect("simultaneous swap should succeed");
    assert_eq!(
        game.state
            .entities
            .get(ids[0])
            .map(|e| (e.pos_x(), e.pos_y())),
        Some((bx, by))
    );
    assert_eq!(
        game.state
            .entities
            .get(ids[1])
            .map(|e| (e.pos_x(), e.pos_y())),
        Some((ax, ay))
    );

    let before = game.clone_for_replay_keyframe();
    let conflict = game.lab_apply_updates(vec![
        LabUpdate::Move(LabMoveEntity {
            entity_id: ids[0],
            x: ax,
            y: ay,
        }),
        LabUpdate::Move(LabMoveEntity {
            entity_id: ids[1],
            x: ax,
            y: ay,
        }),
    ]);
    assert!(matches!(
        conflict,
        Err(LabBatchError {
            failed_index: 1,
            ..
        })
    ));
    assert_eq!(
        game.state
            .entities
            .get(ids[0])
            .map(|e| (e.pos_x(), e.pos_y())),
        before
            .state
            .entities
            .get(ids[0])
            .map(|e| (e.pos_x(), e.pos_y()))
    );
    assert_eq!(
        game.state
            .entities
            .get(ids[1])
            .map(|e| (e.pos_x(), e.pos_y())),
        before
            .state
            .entities
            .get(ids[1])
            .map(|e| (e.pos_x(), e.pos_y()))
    );
}

#[test]
fn lab_bulk_updates_reject_duplicate_targets_and_player_fields() {
    let mut game = new_game();
    let entity_id = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.is_unit())
        .map(|entity| entity.id)
        .expect("owned unit");
    let (x, y) = tile_center(&game, 30, 30);
    assert!(matches!(
        game.lab_apply_updates(vec![
            LabUpdate::Move(LabMoveEntity { entity_id, x, y }),
            LabUpdate::SetEntityOwner(LabSetEntityOwner {
                entity_id,
                owner: 2
            }),
        ]),
        Err(LabBatchError {
            failed_index: 1,
            error: LabError::DuplicateMutation { .. }
        })
    ));
    assert!(matches!(
        game.lab_apply_updates(vec![
            LabUpdate::SetPlayerResources(LabSetPlayerResources {
                player_id: 1,
                steel: 1,
                oil: 2
            }),
            LabUpdate::SetPlayerResources(LabSetPlayerResources {
                player_id: 1,
                steel: 3,
                oil: 4
            }),
        ]),
        Err(LabBatchError {
            failed_index: 1,
            error: LabError::DuplicateMutation { .. }
        })
    ));
}

#[test]
fn lab_set_owner_and_delete_repair_supply_and_references() {
    let mut game = new_game();
    let (x, y) = tile_center(&game, 30, 30);
    let LabOpOutcome::Spawned { entity_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Tank,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("tank should spawn")
    else {
        panic!("unexpected outcome");
    };

    game.apply_lab_op(LabOp::SetEntityOwner(LabSetEntityOwner {
        entity_id,
        owner: 2,
    }))
    .expect("owner change should be accepted");
    assert_eq!(game.state.entities.get(entity_id).expect("tank").owner, 2);
    assert_eq!(
        game.snapshot_for(1).supply_used,
        rules::economy::supply_cost(EntityKind::Worker) * config::STARTING_WORKERS
            + rules::economy::supply_cost(EntityKind::Rifleman) * config::STARTING_RIFLEMEN
    );
    assert!(game.snapshot_for(2).supply_used >= rules::economy::supply_cost(EntityKind::Tank));

    game.apply_lab_op(LabOp::DeleteEntity { entity_id })
        .expect("delete should be accepted");
    assert!(game.state.entities.get(entity_id).is_none());
    assert!(matches!(
        game.apply_lab_op(LabOp::DeleteEntity { entity_id }),
        Err(LabError::StaleEntity { .. })
    ));
}

#[test]
fn lab_cannot_reassign_completed_neutral_tank_trap() {
    let mut game = new_game();
    let (x, y) = tile_center(&game, 30, 30);
    let LabOpOutcome::Spawned { entity_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::TankTrap,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("Tank Trap should spawn")
    else {
        panic!("unexpected outcome");
    };

    assert!(matches!(
        game.apply_lab_op(LabOp::SetEntityOwner(LabSetEntityOwner {
            entity_id,
            owner: 2,
        })),
        Err(LabError::InvalidCommand { .. })
    ));
    assert_eq!(
        game.state
            .entities
            .get(entity_id)
            .expect("Tank Trap should remain")
            .owner,
        NEUTRAL
    );
}

#[test]
fn lab_owner_change_preserves_unit_panzerfaust_state() {
    let mut game = new_game();

    let (loaded_x, loaded_y) = tile_center(&game, 30, 30);
    let LabOpOutcome::Spawned {
        entity_id: loaded_rifleman,
    } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Panzerfaust,
            x: loaded_x,
            y: loaded_y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("Panzerfaust should spawn")
    else {
        panic!("unexpected outcome");
    };

    let (unloaded_x, unloaded_y) = tile_center(&game, 32, 30);
    let LabOpOutcome::Spawned {
        entity_id: unloaded_rifleman,
    } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 2,
            kind: EntityKind::Rifleman,
            x: unloaded_x,
            y: unloaded_y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("Rifleman should spawn")
    else {
        panic!("unexpected outcome");
    };

    game.apply_lab_op(LabOp::SetEntityOwner(LabSetEntityOwner {
        entity_id: loaded_rifleman,
        owner: 2,
    }))
    .expect("loaded Panzerfaust transfer should be accepted");
    game.apply_lab_op(LabOp::SetEntityOwner(LabSetEntityOwner {
        entity_id: unloaded_rifleman,
        owner: 1,
    }))
    .expect("Rifleman transfer should be accepted");

    let panzerfaust_state = |entity_id| {
        game.state
            .entities
            .get(entity_id)
            .and_then(|entity| entity.combat.as_ref())
            .and_then(|combat| combat.panzerfaust)
    };
    assert_eq!(
        panzerfaust_state(loaded_rifleman),
        Some(PanzerfaustState::Loaded),
        "transferring owners must not remove a Panzerfaust launcher"
    );
    assert_eq!(
        panzerfaust_state(unloaded_rifleman),
        None,
        "transferring a Rifleman must not grant a launcher"
    );
}

#[test]
fn lab_resources_and_research_validate_players_and_factions() {
    let mut game = new_game();
    game.apply_lab_op(LabOp::SetPlayerResources(LabSetPlayerResources {
        player_id: 1,
        steel: 1234,
        oil: 567,
    }))
    .expect("resources should be accepted");
    let snapshot = game.snapshot_for(1);
    assert_eq!((snapshot.steel, snapshot.oil), (1234, 567));

    game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
        player_id: 1,
        upgrade: UpgradeKind::TankUnlock,
        completed: true,
    }))
    .expect("research should be accepted");
    assert!(game
        .snapshot_for(1)
        .upgrades
        .contains(&UpgradeKind::TankUnlock.to_protocol_str().to_string()));
    game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
        player_id: 1,
        upgrade: UpgradeKind::TankUnlock,
        completed: false,
    }))
    .expect("research removal should be accepted");
    assert!(!game
        .snapshot_for(1)
        .upgrades
        .contains(&UpgradeKind::TankUnlock.to_protocol_str().to_string()));

    assert!(matches!(
        game.apply_lab_op(LabOp::SetPlayerResources(LabSetPlayerResources {
            player_id: 999,
            steel: 1,
            oil: 1,
        })),
        Err(LabError::InvalidPlayer { player_id: 999 })
    ));
}

#[test]
fn lab_rejects_research_not_in_player_faction_catalog() {
    let players = [PlayerInit {
        id: 7,
        team_id: 7,
        faction_id: "ekat".to_string(),
        name: "Ekat".to_string(),
        color: "#fff".to_string(),
        is_ai: false,
    }];
    let map = Map {
        width: 32,
        height: 32,
        terrain: vec![terrain::GRASS; 32 * 32],
        starts: vec![(8, 8)],
        ..Default::default()
    };
    let mut game = Game::new_lab(&players, 1, map, lab_metadata());

    assert!(matches!(
        game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
            player_id: 7,
            upgrade: UpgradeKind::TankUnlock,
            completed: true,
        })),
        Err(LabError::InvalidResearch { player_id: 7, .. })
    ));
}

#[test]
fn lab_checkpoint_setup_round_trips_exact_state_with_id_map() {
    let mut game = default_map_game();
    let tank_facing = -1.25;
    let tank_weapon_facing = 0.75;
    let (x, y) = free_unit_position(&game, EntityKind::Tank);
    let LabOpOutcome::Spawned { entity_id: tank_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Tank,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("tank should spawn")
    else {
        panic!("unexpected outcome");
    };
    {
        let tank = game.state.entities.get_mut(tank_id).expect("spawned tank");
        tank.set_facing(tank_facing);
        tank.set_weapon_facing(tank_weapon_facing);
        tank.set_desired_weapon_facing(tank_weapon_facing);
    }
    let (x, y) = free_unit_position(&game, EntityKind::AntiTankGun);
    let LabOpOutcome::Spawned { entity_id: gun_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::AntiTankGun,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .expect("anti-tank gun should spawn")
    else {
        panic!("unexpected outcome");
    };
    let setup_target = tile_center(&game, 40, 32);
    let setup_facing = (setup_target.1 - y).det_atan2(setup_target.0 - x);
    let gun_weapon_facing = setup_facing + 0.125;
    {
        let gun = game.state.entities.get_mut(gun_id).expect("spawned gun");
        gun.set_weapon_setup(WeaponSetup::Deployed);
        gun.set_emplacement_facing(Some(setup_facing));
        gun.set_weapon_facing(gun_weapon_facing);
        gun.set_desired_weapon_facing(gun_weapon_facing);
    }

    let scenario = game
        .export_lab_checkpoint_scenario("Checkpoint setup".to_string(), "test-build")
        .expect("checkpoint setup should export");
    assert_eq!(scenario.metadata.source_scenario, None);
    assert!(scenario
        .metadata
        .source_entity_id_map
        .iter()
        .any(|entry| entry.old_id == tank_id && entry.new_id == tank_id));
    assert!(scenario
        .metadata
        .source_entity_id_map
        .iter()
        .any(|entry| entry.old_id == gun_id && entry.new_id == gun_id));

    let mut restored = Game::restore_lab_checkpoint_scenario(scenario.clone())
        .expect("checkpoint setup should restore");
    let restored_tank = restored.state.entities.get(tank_id).expect("restored tank");
    assert_eq!(restored_tank.kind, EntityKind::Tank);
    assert_eq!(restored_tank.owner, 1);
    assert!(matches!(restored_tank.weapon_setup(), WeaponSetup::Packed));
    assert_angle_close(restored_tank.facing(), tank_facing);
    assert_angle_close(
        restored_tank.weapon_facing().unwrap_or_default(),
        tank_weapon_facing,
    );
    let restored_gun = restored.state.entities.get(gun_id).expect("restored gun");
    assert_eq!(restored_gun.kind, EntityKind::AntiTankGun);
    assert!(matches!(restored_gun.weapon_setup(), WeaponSetup::Deployed));
    assert_angle_close(
        restored_gun.emplacement_facing().unwrap_or_default(),
        setup_facing,
    );
    assert_angle_close(
        restored_gun.weapon_facing().unwrap_or_default(),
        gun_weapon_facing,
    );
    restored.tick();
}

#[test]
fn lab_cultivator_spawns_set_owner_faction_and_initialize_each_dao() {
    use crate::protocol::LabSpawnDao;
    for (dao, upgrade) in [
        (LabSpawnDao::Wind, UpgradeKind::DaoWind),
        (LabSpawnDao::War, UpgradeKind::DaoOfHeaviness),
        (LabSpawnDao::Magic, UpgradeKind::DaoOfMagic),
    ] {
        let mut game = new_game();
        let (x, y) = free_unit_position(&game, EntityKind::Warrior);
        let outcome = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 2,
                kind: EntityKind::Warrior,
                x,
                y,
                completed: true,
                dao: Some(dao),
                dao_master: false,
                faction_id: Some("cultivators"),
            }))
            .expect("specialized spawn");
        let LabOpOutcome::Spawned { entity_id } = outcome else {
            panic!("spawn outcome")
        };
        let entity = game.state.entities.get(entity_id).expect("spawned entity");
        assert_eq!(entity.dao(), Some(upgrade));
        assert_eq!(entity.hp, entity.max_hp);
        assert_eq!(entity.dao_advance_ticks, 0);
        assert_eq!(
            game.player_inits()
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "cultivators"
        );
        assert_eq!(
            game.snapshot_full_for(2)
                .player_factions
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "cultivators"
        );
        // Seat identity is public even while viewing only the opposing team's private state.
        let snapshot = game.snapshot_for_observer(&crate::game::ObserverView::Players(vec![1]));
        assert!(snapshot.player_resources.iter().all(|p| p.id == 1));
        assert_eq!(
            snapshot
                .player_factions
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "cultivators"
        );
        assert_eq!(
            game.snapshot_for(1)
                .player_factions
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "cultivators"
        );
    }
}

#[test]
fn lab_spawns_all_dao_masters_with_promotion_stats_and_body() {
    use crate::game::ability::AbilityKind;

    for kind in [
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ] {
        let mut game = new_game();
        let (x, y) = free_unit_position(&game, kind);
        let stats = config::unit_stats(kind).unwrap();
        game.state.entities.spawn_unit(1, kind, x, y).unwrap();

        let nearby_x = x + stats.radius * (1.5 + 0.5 * config::DAO_MASTER_BODY_SCALE);
        let mut spawn = LabSpawnEntity {
            owner: 2,
            kind,
            x: nearby_x,
            y,
            completed: true,
            dao: None,
            dao_master: true,
            faction_id: Some("cultivators"),
        };
        assert!(game
            .validate_unit_position(&game.state.entities, kind, nearby_x, y)
            .is_ok());
        let error = game.apply_lab_op(LabOp::SpawnEntity(spawn)).unwrap_err();
        assert!(matches!(error, LabError::Placement { blockers, .. } if !blockers.is_empty()));
        assert_eq!(game.player_inits()[1].faction_id, "kriegsia");

        spawn.x += 2.0 * config::TILE_SIZE as f32;
        let LabOpOutcome::Spawned { entity_id } =
            game.apply_lab_op(LabOp::SpawnEntity(spawn)).unwrap()
        else {
            panic!("spawn outcome");
        };
        let master = game.state.entities.get(entity_id).unwrap();
        assert!(master.dao_master);
        assert_eq!(master.kind, kind);
        assert_eq!(master.insight, 0);
        assert_eq!(master.dao_advance_ticks, 0);
        assert_eq!(master.hp, master.max_hp);
        assert_eq!(
            master.max_hp,
            (stats.hp as f32 * config::DAO_MASTER_HP_MULTIPLIER).round() as u32
        );
        if kind == EntityKind::WindAdept {
            assert_eq!(
                master.ability_uses_remaining(AbilityKind::WindDash),
                Some(2)
            );
        }
    }
}

#[test]
fn lab_invalid_dao_or_faction_spawn_is_atomic() {
    let mut game = new_game();
    let (x, y) = free_unit_position(&game, EntityKind::Worker);
    let count = game.state.entities.iter().count();
    for (kind, dao, faction_id) in [
        (
            EntityKind::Worker,
            Some(crate::protocol::LabSpawnDao::War),
            Some("cultivators"),
        ),
        (EntityKind::Rifleman, None, Some("cultivators")),
        (EntityKind::Worker, None, Some("missing")),
    ] {
        assert!(game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 2,
                kind,
                x,
                y,
                completed: true,
                dao,
                dao_master: false,
                faction_id,
            }))
            .is_err());
        assert_eq!(game.state.entities.iter().count(), count);
        assert_eq!(
            game.player_inits()
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "kriegsia"
        );
    }
}

#[test]
fn lab_p2_base_novice_can_evolve_after_cultivator_spawn() {
    let mut game = new_game();
    let (x, y) = free_unit_position(&game, EntityKind::Warrior);
    let LabOpOutcome::Spawned { entity_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 2,
            kind: EntityKind::Warrior,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: Some("cultivators"),
        }))
        .unwrap()
    else {
        panic!("spawn outcome")
    };
    game.state.entities.get_mut(entity_id).unwrap().insight = config::DISCIPLE_INSIGHT_REQUIRED;
    game.issue_lab_command_as(
        2,
        Command::UseAbility {
            target_id: None,
            ability: "evolveMagic".to_string(),
            units: vec![entity_id],
            x: None,
            y: None,
            queued: false,
        },
        LabCommandOptions {
            ignore_command_limits: true,
        },
    )
    .unwrap();
    game.tick();
    assert_eq!(
        game.state.entities.get(entity_id).unwrap().dao(),
        Some(UpgradeKind::DaoOfMagic)
    );
}
