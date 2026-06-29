use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::resource_placement;

use super::{validate_resource_node_position, validate_world_position, LabError};

#[cfg(test)]
#[path = "resource_nodes_tests.rs"]
mod tests;

// Restore can snap legacy coordinates, but invalid authoring should not be relocated
// across the map.
const RESOURCE_RESTORE_SEARCH_RADIUS_TILES: u32 = 7;

pub(super) fn restore_resource_node_position(
    map: &Map,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
    snap_to_spaced_tile_center: bool,
) -> Result<(f32, f32), LabError> {
    if !snap_to_spaced_tile_center {
        validate_resource_node_position(map, entities, x, y)?;
        return Ok((x, y));
    }

    validate_world_position(map, x, y)?;
    let source_tile = map.tile_of(x, y);
    let occupied_tiles = resource_placement::occupied_resource_tiles(map, entities, kind);
    let Some((center_x, center_y, _tile)) =
        resource_placement::nearest_resource_tile_center(map, x, y, |tile, center_x, center_y| {
            tile.0.abs_diff(source_tile.0) <= RESOURCE_RESTORE_SEARCH_RADIUS_TILES
                && tile.1.abs_diff(source_tile.1) <= RESOURCE_RESTORE_SEARCH_RADIUS_TILES
                && resource_placement::tile_has_one_tile_resource_gap(tile, &occupied_tiles)
                && validate_resource_node_position(map, entities, center_x, center_y).is_ok()
        })
    else {
        return Err(LabError::InvalidPosition {
            x,
            y,
            reason: "oil node must have a nearby passable tile center with one tile of spacing",
        });
    };

    Ok((center_x, center_y))
}
