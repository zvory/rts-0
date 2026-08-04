use super::*;
use crate::protocol::terrain;

#[test]
fn worker_direct_attack_stages_outside_barracks_footprint() {
    let map = Map {
        width: 32,
        height: 32,
        terrain: vec![terrain::GRASS; 32 * 32],
        starts: vec![],
        ..Default::default()
    };
    let mut entities = EntityStore::new();
    let start = map.tile_center(3, 12);
    let target_pos = map.tile_center(20, 12);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, start.0, start.1)
        .expect("worker should spawn");
    let barracks = entities
        .spawn_building(2, EntityKind::Barracks, target_pos.0, target_pos.1, true)
        .expect("barracks should spawn");
    let rect =
        building_rect_for_entity(&map, entities.get(barracks).expect("barracks should exist"))
            .expect("barracks should have a footprint");
    let occupancy = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occupancy, 1);
    coordinator.order_attack(&mut entities, worker, barracks);
    let worker_stats = config::unit_stats(EntityKind::Worker).expect("worker stats should exist");
    let range_px = worker_stats.range_tiles as f32 * config::TILE_SIZE as f32
        + worker_stats.radius
        + crate::game::services::combat::RANGE_SLACK;

    assert!(coordinator.request_direct_attack_path(&mut entities, worker, barracks, 0.0, range_px,));

    let goal = entities
        .get(worker)
        .and_then(|worker| worker.path_goal())
        .expect("worker should receive a pursuit goal");
    assert!(
        goal.0 + worker_stats.radius <= rect.min_x,
        "building pursuit must stop at a standable point outside the footprint"
    );
    let edge_gap = (rect.min_x - goal.0).max(0.0);
    assert!(
        edge_gap <= range_px && edge_gap >= range_px - 8.0,
        "pursuit should stage just inside weapon range of the wall: {edge_gap}"
    );
}
