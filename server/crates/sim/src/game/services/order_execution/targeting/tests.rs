use super::*;

const TILE: f32 = 32.0;
const WORLD: f32 = 64.0 * TILE;
const ORIGIN: (f32, f32) = (32.0 * TILE, 32.0 * TILE);
const MIN: f32 = 25.0 * TILE;
const MAX: f32 = 65.0 * TILE;

fn lock(raw_click: (f32, f32)) -> LockedArtilleryFireTarget {
    lock_artillery_fire_target(WORLD, ORIGIN, Some(0.0), 0.0, MIN, MAX, raw_click)
        .expect("target should lock")
}

#[test]
fn target_lock_preserves_points_inside_range_band() {
    let target = lock((ORIGIN.0 + 30.0 * TILE, ORIGIN.1));

    assert!((target.x - (ORIGIN.0 + 30.0 * TILE)).abs() < 0.001);
    assert!((target.y - ORIGIN.1).abs() < 0.001);
}

#[test]
fn target_lock_pushes_close_clicks_to_minimum_range() {
    let target = lock((ORIGIN.0 + 3.0 * TILE, ORIGIN.1));

    assert!((target.x - (ORIGIN.0 + MIN)).abs() < 0.001);
    assert!((target.y - ORIGIN.1).abs() < 0.001);
}

#[test]
fn target_lock_pulls_far_clicks_to_maximum_or_map_edge() {
    let target = lock((ORIGIN.0 + 80.0 * TILE, ORIGIN.1));

    assert!((target.x - (WORLD - 1.0)).abs() < 0.001);
    assert!((target.y - ORIGIN.1).abs() < 0.001);
}

#[test]
fn target_lock_keeps_large_finite_click_direction() {
    let target = lock_artillery_fire_target(
        WORLD,
        ORIGIN,
        Some(std::f32::consts::FRAC_PI_2),
        std::f32::consts::FRAC_PI_2,
        MIN,
        MAX,
        (1.0e20, ORIGIN.1),
    )
    .expect("large finite click should still lock along its ray");

    assert!((target.x - (WORLD - 1.0)).abs() < 0.001);
    assert!((target.y - ORIGIN.1).abs() < 0.001);
}

#[test]
fn zero_length_target_uses_setup_facing_fallback() {
    let target = lock_artillery_fire_target(
        WORLD,
        ORIGIN,
        Some(std::f32::consts::FRAC_PI_2),
        0.0,
        MIN,
        MAX,
        ORIGIN,
    )
    .expect("zero-length click should lock along setup facing");

    assert!((target.x - ORIGIN.0).abs() < 0.001);
    assert!((target.y - (ORIGIN.1 + MIN)).abs() < 0.001);
}

#[test]
fn target_lock_rejects_rays_without_an_in_map_range_point() {
    let origin = (4.0 * TILE, 4.0 * TILE);

    assert!(lock_artillery_fire_target(
        WORLD,
        origin,
        Some(std::f32::consts::PI),
        std::f32::consts::PI,
        MIN,
        MAX,
        origin,
    )
    .is_none());
}
