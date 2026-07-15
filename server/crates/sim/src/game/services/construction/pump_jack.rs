use std::collections::BTreeSet;

use super::{config, standability, EntityKind, EntityStore, Map, Occupancy, TeamRelations};

pub(super) fn all_site_unit_blockers_are_friendly(
    entities: &EntityStore,
    teams: &TeamRelations,
    owner: u32,
    builder: u32,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    entities.iter().all(|entity| {
        entity.id == builder
            || entity.hp == 0
            || !entity.is_unit()
            || !standability::unit_intersects_building_footprint(
                entity,
                EntityKind::PumpJack,
                tile_x,
                tile_y,
            )
            || teams.same_team_or_same_owner(owner, entity.owner)
    })
}

pub(super) fn eject_friendly_units_from_site(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    owner: u32,
    builder: u32,
    tile_x: u32,
    tile_y: u32,
) {
    let mut ejecting: Vec<u32> = entities
        .iter()
        .filter(|entity| {
            entity.id != builder
                && entity.hp > 0
                && entity.is_unit()
                && teams.same_team_or_same_owner(owner, entity.owner)
                && standability::unit_intersects_building_footprint(
                    entity,
                    EntityKind::PumpJack,
                    tile_x,
                    tile_y,
                )
        })
        .map(|entity| entity.id)
        .collect();
    ejecting.sort_unstable();
    if ejecting.is_empty() {
        return;
    }

    let occupancy = Occupancy::build(map, entities);
    let mut ignored_units: BTreeSet<u32> = ejecting.iter().copied().collect();
    for unit in ejecting {
        let Some((kind, facing, x, y)) = entities
            .get(unit)
            .map(|entity| (entity.kind, entity.facing(), entity.pos_x, entity.pos_y))
        else {
            ignored_units.remove(&unit);
            continue;
        };
        if let Some((next_x, next_y)) = ejection_position(
            map,
            &occupancy,
            entities,
            &ignored_units,
            kind,
            facing,
            x,
            y,
            tile_x,
            tile_y,
        ) {
            if let Some(entity) = entities.get_mut(unit) {
                entity.set_position(next_x, next_y);
            }
        }
        ignored_units.remove(&unit);
    }
}

#[allow(clippy::too_many_arguments)]
fn ejection_position(
    map: &Map,
    occupancy: &Occupancy<'_>,
    entities: &EntityStore,
    ignored_units: &BTreeSet<u32>,
    kind: EntityKind,
    facing: f32,
    x: f32,
    y: f32,
    site_tile_x: u32,
    site_tile_y: u32,
) -> Option<(f32, f32)> {
    let (start_x, start_y) = map.tile_of(x, y);
    let tile_size = config::TILE_SIZE as f32;
    let max_radius = map.size.min(i32::MAX as u32) as i32;
    for radius in 1..=max_radius {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let tile_x = start_x as i64 + dx as i64;
                let tile_y = start_y as i64 + dy as i64;
                let (Ok(tile_x), Ok(tile_y)) = (i32::try_from(tile_x), i32::try_from(tile_y))
                else {
                    continue;
                };
                if !map.in_bounds(tile_x, tile_y) {
                    continue;
                }
                let candidate_x = tile_x as f32 * tile_size + tile_size * 0.5;
                let candidate_y = tile_y as f32 * tile_size + tile_size * 0.5;
                if standability::unit_position_clear_of_building_footprint(
                    map,
                    occupancy,
                    entities,
                    ignored_units,
                    kind,
                    facing,
                    candidate_x,
                    candidate_y,
                    EntityKind::PumpJack,
                    site_tile_x,
                    site_tile_y,
                ) {
                    return Some((candidate_x, candidate_y));
                }
            }
        }
    }
    None
}
