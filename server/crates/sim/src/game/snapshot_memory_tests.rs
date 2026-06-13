use super::*;
use crate::game::entity::EntityKind;
use crate::game::{services, systems, SmokeCloudStore};
use crate::protocol::terrain;

fn players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            name: "Two".into(),
            color: "#000".into(),
            is_ai: true,
        },
    ]
}

fn empty_flat_game() -> Game {
    let mut game = Game::new_for_replay(&players(), 0x1234_5678);
    for tile in &mut game.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    game.smokes = SmokeCloudStore::new();
    game.mortar_shells = MortarShellStore::default();
    game.artillery_shells = artillery::ArtilleryShellStore::default();
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    game.refresh_building_memory(&ids);
    game
}

#[test]
fn exposes_hidden_remembered_building_without_live_entity() {
    let mut game = empty_flat_game();
    let scout_pos = game.map.tile_center(8, 8);
    let depot_pos = game.map.tile_center(10, 8);
    let scout = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("depot should spawn");
    game.tick();

    let visible = game.snapshot_for(1);
    assert!(visible.entities.iter().any(|entity| entity.id == depot));
    assert!(visible
        .remembered_buildings
        .iter()
        .all(|building| building.id != depot));

    game.entities.remove(scout);
    let far = game.map.tile_center(40, 40);
    game.entities
        .spawn_unit(1, EntityKind::Rifleman, far.0, far.1)
        .expect("far scout should spawn");
    game.tick();

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
fn does_not_expose_never_scouted_building_memory() {
    let mut game = empty_flat_game();
    let scout_pos = game.map.tile_center(4, 4);
    let depot_pos = game.map.tile_center(40, 40);
    game.entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("depot should spawn");
    game.tick();

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
    let scout_pos = game.map.tile_center(8, 8);
    let depot_pos = game.map.tile_center(10, 8);
    let scout = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("depot should spawn");
    game.tick();

    game.entities.remove(scout);
    let far = game.map.tile_center(40, 40);
    game.entities
        .spawn_unit(1, EntityKind::Rifleman, far.0, far.1)
        .expect("far scout should spawn");
    game.entities.remove(depot);
    game.tick();

    let stale = game.snapshot_for(1);
    assert!(stale
        .remembered_buildings
        .iter()
        .any(|building| building.id == depot));

    game.entities
        .spawn_unit(1, EntityKind::Rifleman, depot_pos.0, depot_pos.1)
        .expect("new scout should spawn");
    game.tick();

    let cleared = game.snapshot_for(1);
    assert!(cleared
        .remembered_buildings
        .iter()
        .all(|building| building.id != depot));
}
