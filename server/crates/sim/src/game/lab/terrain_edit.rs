//! Repair policy for live Lab terrain edits.

use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::standability;

use super::LabError;

/// A brush may cover a unit's current body. Nudge affected units to the nearest deterministic
/// standable tile instead of rejecting the edit or resetting the whole battle.
pub(super) fn relocate_blocked_units(
    map: &Map,
    entities: &mut EntityStore,
    map_name: &str,
) -> Result<(), LabError> {
    let occupancy = Occupancy::build(map, entities);
    let mut relocations = Vec::new();
    for entity in entities.iter().filter(|entity| entity.is_unit()) {
        if entity.kind == EntityKind::ScoutPlane
            || standability::unit_static_standable_with_facing(
                map,
                &occupancy,
                entity.kind,
                entity.pos_x,
                entity.pos_y,
                entity.facing(),
            )
        {
            continue;
        }
        let Some(position) = nearest_standable_position(map, &occupancy, entity) else {
            return Err(LabError::InvalidMap {
                name: map_name.to_string(),
                reason: format!(
                    "edited terrain leaves {} {} without a standable tile",
                    entity.kind, entity.id
                ),
            });
        };
        relocations.push((entity.id, position));
    }
    for (entity_id, (x, y)) in relocations {
        if let Some(entity) = entities.get_mut(entity_id) {
            entity.set_position(x, y);
            entity.clear_orders();
        }
    }
    Ok(())
}

fn nearest_standable_position(
    map: &Map,
    occupancy: &Occupancy<'_>,
    entity: &Entity,
) -> Option<(f32, f32)> {
    let tile_size = config::TILE_SIZE as f32;
    let origin_x = (entity.pos_x / tile_size).floor() as i32;
    let origin_y = (entity.pos_y / tile_size).floor() as i32;
    let max_radius = i32::try_from(map.size).ok()?;
    for radius in 0..=max_radius {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let tile_x = origin_x.checked_add(dx)?;
                let tile_y = origin_y.checked_add(dy)?;
                if !map.in_bounds(tile_x, tile_y) {
                    continue;
                }
                let (x, y) = map.tile_center(tile_x as u32, tile_y as u32);
                if standability::unit_static_standable_with_facing(
                    map,
                    occupancy,
                    entity.kind,
                    x,
                    y,
                    entity.facing(),
                ) {
                    return Some((x, y));
                }
            }
        }
    }
    None
}
