use super::*;
use crate::game::services::movement::pivot_drive::{
    vehicle_body_turn_rate, ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK,
    PIVOT_VEHICLE_BODY_TURN_RATE_RAD_PER_TICK,
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
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities
        .get(anti_tank_gun)
        .expect("anti-tank gun should exist");
    let actual_degrees_per_second = e.facing().to_degrees() * config::TICK_HZ as f32;
    assert!(
        (actual_degrees_per_second - 110.0).abs() <= 0.001,
        "anti-tank gun body should turn at 110 degrees per second, got {actual_degrees_per_second:.4}"
    );
    assert!((e.facing() - ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK).abs() <= 0.0001);
    assert!(
        moved_distance((sx, sy), (e.pos_x(), e.pos_y())) < 0.01,
        "anti-tank gun should pivot before driving when the target is far off its facing"
    );
}

#[test]
fn artillery_body_keeps_baseline_pivot_drive_turn_rate() {
    assert_eq!(
        vehicle_body_turn_rate(EntityKind::Artillery),
        PIVOT_VEHICLE_BODY_TURN_RATE_RAD_PER_TICK
    );
}

#[test]
fn tank_body_turns_twenty_five_percent_faster_than_baseline() {
    assert_eq!(
        vehicle_body_turn_rate(EntityKind::Tank),
        TANK_BODY_TURN_RATE_RAD_PER_TICK
    );
    assert_eq!(
        TANK_BODY_TURN_RATE_RAD_PER_TICK,
        PIVOT_VEHICLE_BODY_TURN_RATE_RAD_PER_TICK * 1.25
    );
}

#[test]
fn long_pivot_route_preserves_movement_trace_with_obstacles_and_traffic() {
    use sha2::{Digest, Sha256};

    // Captured before acceptance reuse: cover a long accepted prefix and a
    // rejected shortcut with stationary traffic. Hash every movement step.
    for (blocked, expected) in [
        (
            false,
            "fe07fe1cec219f0d0d5ae9929fdc3ede9059214a3b923b59f512f3eae1a4e17e",
        ),
        (
            true,
            "de9fa7a9c0dbd9fc0c36a0a18f16265766812116264c5d180ab41ec53868d7f2",
        ),
    ] {
        let map = Map {
            width: 64,
            height: 64,
            terrain: vec![crate::protocol::terrain::GRASS; 64 * 64],
            ..Default::default()
        };
        let mut entities = EntityStore::new();
        let start = map.tile_center(8, 8);
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, start.0, start.1)
            .expect("tank should spawn");
        let route: Vec<_> = (9..=40)
            .flat_map(|tile| [map.tile_center(tile, tile - 1), map.tile_center(tile, tile)])
            .collect();
        let goal = *route.last().expect("route should have a goal");
        if let Some(unit) = entities.get_mut(tank) {
            unit.set_facing(std::f32::consts::FRAC_PI_4);
            unit.set_order(Order::attack_move_to(goal.0, goal.1));
        }
        set_path_direct(&mut entities, tank, route);
        if blocked {
            let obstacle = footprint_center(&map, EntityKind::TankTrap, 20, 20);
            entities
                .spawn_building(2, EntityKind::TankTrap, obstacle.0, obstacle.1, true)
                .expect("static obstacle should spawn");
            entities
                .spawn_unit(1, EntityKind::Tank, start.0 + 48.0, start.1 + 48.0)
                .expect("stationary traffic should spawn");
        }
        let occ = Occupancy::build(&map, &entities);
        let mut trace = Sha256::new();
        for tick in 0..64 {
            let spatial = SpatialIndex::build(&entities, map.width, map.height);
            movement_system(&map, &mut entities, &[], &occ, &spatial, tick);
            let unit = entities.get(tank).expect("tank should remain");
            // Keep the pre-commitment trace oracle: only the new decision state
            // is omitted, so every prior position/facing/path field must still match.
            let mut movement = unit.movement.clone();
            if let Some(movement) = movement.as_mut() {
                movement.pivot_drive = None;
            }
            trace.update(
                serde_json::to_vec(&(
                    unit.pos_x().to_bits(),
                    unit.pos_y().to_bits(),
                    unit.facing().to_bits(),
                    &movement,
                    unit.order(),
                ))
                .expect("movement trace should serialize"),
            );
        }
        assert_eq!(
            format!("{:x}", trace.finalize()),
            expected,
            "blocked={blocked}"
        );
    }
}
