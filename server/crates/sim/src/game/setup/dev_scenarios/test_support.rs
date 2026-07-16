use super::*;

pub(super) fn speed_scaled_escape_deadline_ticks(
    unit: EntityKind,
    escape_distance_px: f32,
    route_multiplier: u32,
) -> u32 {
    let speed_px_per_tick = config::unit_stats(unit)
        .unwrap_or_else(|| panic!("{unit} should have movement stats"))
        .speed;
    assert!(
        speed_px_per_tick.is_finite() && speed_px_per_tick > 0.0,
        "{unit} should have a positive finite movement speed"
    );
    let free_flow_ticks = (escape_distance_px.max(0.0) / speed_px_per_tick).ceil() as u32;
    free_flow_ticks
        .saturating_mul(route_multiplier)
        .saturating_add(config::TICK_HZ * 2)
}
