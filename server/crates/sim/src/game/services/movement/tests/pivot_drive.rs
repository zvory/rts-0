use super::*;
use crate::game::services::movement::pivot_drive::{
    vehicle_body_turn_rate, ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK,
};

#[test]
fn anti_tank_gun_body_uses_pivot_drive_turning_along_path() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let (_, gy) = map.tile_center(20, 26);
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, sx, sy)
        .expect("anti-tank gun should spawn");
    if let Some(e) = entities.get_mut(anti_tank_gun) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, anti_tank_gun, vec![(sx, gy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities
        .get(anti_tank_gun)
        .expect("anti-tank gun should exist");
    let actual_degrees_per_second = e.facing().to_degrees() * config::TICK_HZ as f32;
    assert!(
        (actual_degrees_per_second - 50.0).abs() <= 0.001,
        "anti-tank gun body should turn at 50 degrees per second, got {actual_degrees_per_second:.4}"
    );
    assert!((e.facing() - ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK).abs() <= 0.0001);
    assert!(
        moved_distance((sx, sy), (e.pos_x, e.pos_y)) < 0.01,
        "anti-tank gun should pivot before driving when the target is far off its facing"
    );
}

#[test]
fn artillery_body_keeps_baseline_pivot_drive_turn_rate() {
    assert_eq!(
        vehicle_body_turn_rate(EntityKind::Artillery),
        TANK_BODY_TURN_RATE_RAD_PER_TICK
    );
}
