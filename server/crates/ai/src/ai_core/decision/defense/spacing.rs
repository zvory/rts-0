use super::*;

/// Reserve destinations jointly and reject disconnected terrain and blocked rifle firing lanes.
pub(in crate::ai_core::decision) fn separated_rifle_position(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    desired: (f32, f32),
    direction: (f32, f32),
    reserved: &[(f32, f32)],
) -> Option<(f32, f32)> {
    separated_rifle_position_where(observation, analysis, desired, direction, reserved, |_| {
        true
    })
}

pub(in crate::ai_core::decision) fn separated_rifle_position_where(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    desired: (f32, f32),
    direction: (f32, f32),
    reserved: &[(f32, f32)],
    valid: impl Fn((f32, f32)) -> bool,
) -> Option<(f32, f32)> {
    use crate::ai_core::map_analysis::AiTile;
    let ts = observation.map.tile_size as f32;
    for radius in 0_i32..=6 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let point = clamp_to_map(
                    (desired.0 + dx as f32 * ts, desired.1 + dy as f32 * ts),
                    observation.map,
                );
                let point = tile_center(
                    world_tile(observation.map, point.0, point.1),
                    observation.map.tile_size,
                );
                if !valid(point) {
                    continue;
                }
                if reserved
                    .iter()
                    .any(|other| dist2(point.0, point.1, other.0, other.1) < squared(2.75 * ts))
                {
                    continue;
                }
                let tile = world_tile(observation.map, point.0, point.1);
                if analysis.is_some_and(|map| {
                    let home = map.component_id_at(AiTile {
                        x: observation.own_start_tile.0,
                        y: observation.own_start_tile.1,
                    });
                    let destination = map.component_id_at(AiTile {
                        x: tile.0,
                        y: tile.1,
                    });
                    destination.is_none() || destination != home
                }) {
                    continue;
                }
                if defensive_position_is_open(observation, analysis, point.0, point.1)
                    && defensive_firing_lane_is_clear(observation, analysis, point, direction, 4.0)
                {
                    return Some(point);
                }
            }
        }
    }
    None
}
