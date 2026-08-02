use std::collections::HashMap;

use crate::game::entity::{Entity, EntityStore};
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;

pub(super) fn stamp_visibility(
    grids: &mut HashMap<u32, Vec<bool>>,
    size: u32,
    store: &EntityStore,
    smokes: &SmokeCloudStore,
    map: &Map,
) {
    let concealed_units = store
        .iter()
        .filter(|entity| {
            entity.owner != 0
                && entity.hp > 0
                && entity.is_unit()
                && smokes.point_inside(entity.pos_x, entity.pos_y)
        })
        .collect::<Vec<_>>();
    for (index, first) in concealed_units.iter().enumerate() {
        for second in concealed_units.iter().skip(index + 1) {
            if first.owner == second.owner
                || !SmokeCloudStore::units_within_melee_visibility_range(first, second)
            {
                continue;
            }
            stamp_entity_tile(grids, size, first.owner, second, map);
            stamp_entity_tile(grids, size, second.owner, first, map);
        }
    }
}

fn stamp_entity_tile(
    grids: &mut HashMap<u32, Vec<bool>>,
    size: u32,
    owner: u32,
    entity: &Entity,
    map: &Map,
) {
    let Some(grid) = grids.get_mut(&owner) else {
        return;
    };
    let (tx, ty) = map.tile_of(entity.pos_x, entity.pos_y);
    if tx >= size || ty >= size {
        return;
    }
    if let Some(cell) = grid.get_mut((ty * size + tx) as usize) {
        *cell = true;
    }
}
