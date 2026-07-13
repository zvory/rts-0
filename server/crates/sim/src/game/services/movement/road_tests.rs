use super::movement_system;
use crate::config;
use crate::game::entity::{EntityKind, EntityStore, MovePhase, Order};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;
use crate::game::{PlayerState, ScoreState};

fn player_with_oil(id: u32) -> PlayerState {
    PlayerState {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: format!("p{id}"),
        color: "#ffffff".to_string(),
        start_tile: (0, 0),
        steel: 0,
        oil: 1_000,
        supply_used: 0,
        supply_cap: 0,
        is_ai: false,
        score: ScoreState::default(),
        upgrades: Default::default(),
        ability_cooldowns: Default::default(),
    }
}

#[test]
fn road_tile_applies_authoritative_movement_speed_multiplier() {
    let mut map = Map::generate(1, 0xC0FF_EE01);
    map.terrain.fill(crate::protocol::terrain::GRASS);
    let mut entities = EntityStore::new();
    let grass_start = map.tile_center(20, 20);
    let road_start = map.tile_center(20, 30);
    let road_index = map.index(20, 30);
    map.terrain[road_index] = crate::protocol::terrain::ROAD_HORIZONTAL;

    let grass = entities
        .spawn_unit(1, EntityKind::Rifleman, grass_start.0, grass_start.1)
        .expect("grass rifleman");
    let road = entities
        .spawn_unit(1, EntityKind::Rifleman, road_start.0, road_start.1)
        .expect("road rifleman");
    for (id, start) in [(grass, grass_start), (road, road_start)] {
        let goal = (start.0 + 64.0, start.1);
        let entity = entities.get_mut(id).expect("spawned rifleman");
        entity.set_order(Order::move_to(goal.0, goal.1));
        entity.set_path(vec![goal]);
        entity.set_path_goal(Some(goal));
        entity.mark_move_phase(MovePhase::Moving);
    }

    let occupancy = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(
        &map,
        &mut entities,
        &mut [player_with_oil(1)],
        &occupancy,
        &spatial,
        1,
    );

    let moved = |id, start: (f32, f32)| {
        let entity = entities.get(id).expect("spawned rifleman");
        ((entity.pos_x - start.0).powi(2) + (entity.pos_y - start.1).powi(2)).sqrt()
    };
    let grass_moved = moved(grass, grass_start);
    let road_moved = moved(road, road_start);
    let expected_grass = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .speed;
    assert!((grass_moved - expected_grass).abs() <= 0.001);
    assert!(
        (road_moved - expected_grass * crate::rules::terrain::ROAD_MOVEMENT_SPEED_MULTIPLIER).abs()
            <= 0.001,
        "road moved {road_moved}px while grass moved {grass_moved}px"
    );
}
