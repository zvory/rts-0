use super::*;

mod tank_traps;

pub(super) use tank_traps::*;

const ZERO_GAP_STANDABLE_EPS_PX: f32 = 0.02;

pub(super) fn flat_dev_map(player_count: usize) -> Map {
    let mut map = Map::generate(player_count, 0xC0FF_EE01);
    for terrain in &mut map.terrain {
        *terrain = crate::protocol::terrain::GRASS;
    }
    map.base_sites.clear();
    map
}

pub(super) fn block_rect_tiles(map: &mut Map, min_x: u32, min_y: u32, max_x: u32, max_y: u32) {
    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            let idx = map.index(tx, ty);
            map.terrain[idx] = crate::protocol::terrain::ROCK;
        }
    }
}

pub(super) fn carve_rect_tiles(map: &mut Map, min_x: u32, min_y: u32, max_x: u32, max_y: u32) {
    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            let idx = map.index(tx, ty);
            map.terrain[idx] = crate::protocol::terrain::GRASS;
        }
    }
}

pub(super) fn carve_horizontal_corridor(map: &mut Map, min_x: u32, max_x: u32, center_y: u32) {
    carve_rect_tiles(map, min_x, center_y - 1, max_x, center_y + 1);
}

pub(super) fn carve_vertical_corridor(map: &mut Map, center_x: u32, min_y: u32, max_y: u32) {
    carve_rect_tiles(map, center_x - 1, min_y, center_x + 1, max_y);
}

pub(super) type ScoutCarCorridorLayout = (Map, (u32, u32), (f32, f32), (f32, f32));

pub(super) fn scout_car_snaking_corridor_map() -> ScoutCarCorridorLayout {
    let mut map = flat_dev_map(1);
    let stone_min_y = 15u32;
    let stone_max_y = 75u32;
    let exit_x = 36u32;
    let first_left_x = 26u32;
    let right_x = 56u32;
    let lower_lane_y = 68u32;
    let middle_lane_y = 64u32;
    let upper_lane_y = 60u32;

    let stone_max_x = map.size - 1;
    block_rect_tiles(&mut map, 0, stone_min_y, stone_max_x, stone_max_y);

    carve_vertical_corridor(&mut map, exit_x, lower_lane_y, stone_max_y);
    carve_horizontal_corridor(&mut map, first_left_x, exit_x, lower_lane_y);
    carve_vertical_corridor(&mut map, first_left_x, middle_lane_y, lower_lane_y);
    carve_horizontal_corridor(&mut map, first_left_x, right_x, middle_lane_y);
    carve_vertical_corridor(&mut map, right_x, upper_lane_y, middle_lane_y);
    carve_horizontal_corridor(&mut map, exit_x, right_x, upper_lane_y);
    carve_vertical_corridor(&mut map, exit_x, stone_min_y, upper_lane_y);

    let ts = config::TILE_SIZE as f32;
    let start_tile = (exit_x, stone_max_y + 5);
    let start = map.tile_center(start_tile.0, start_tile.1);
    let exit = map.tile_center(exit_x, stone_min_y - 1);
    let goal = (exit.0 + ts * 10.0, exit.1 - ts * 10.0);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = start_tile;
    }

    (map, start_tile, start, goal)
}

#[allow(clippy::type_complexity)]
pub(super) fn scout_car_wall_chokepoint_map(
    unit: EntityKind,
    unit_count: usize,
) -> (Map, (u32, u32), Vec<(f32, f32)>, (f32, f32)) {
    let mut map = flat_dev_map(1);
    let center_x = map.size / 2;
    let wall_y = map.size - 18;
    let start_tile = (center_x, wall_y + 10);
    let gap_left_x = center_x - 1;
    let gap_right_x = center_x;
    let max_tile = map.size - 1;

    block_rect_tiles(&mut map, 0, wall_y, max_tile, wall_y);
    carve_rect_tiles(&mut map, gap_left_x, wall_y, gap_right_x, wall_y);

    let ts = config::TILE_SIZE as f32;
    let center_world_x = gap_right_x as f32 * ts;
    let start_y = (start_tile.1 as f32 + 0.5) * ts;
    let spacing = wall_chokepoint_spawn_spacing(unit);
    let center_index = (unit_count.saturating_sub(1)) as f32 * 0.5;
    let starts = (0..unit_count)
        .map(|i| {
            let offset = (i as f32 - center_index) * spacing;
            (center_world_x + offset, start_y)
        })
        .collect();
    let goal_y = (wall_y as f32 + 0.5) * ts - ts * 10.0;
    let goal = (center_world_x, goal_y);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = start_tile;
    }

    (map, start_tile, starts, goal)
}

#[allow(clippy::type_complexity)]
pub(super) fn vehicle_corner_wall_map(
    unit: EntityKind,
    unit_count: usize,
) -> (Map, (u32, u32), Vec<(f32, f32)>, (f32, f32)) {
    let mut map = flat_dev_map(1);
    let center_x = map.size / 2;
    let wall_left_x = center_x;
    let wall_right_x = wall_left_x + 2;
    let wall_top_y = map.size / 2 - 8;
    let wall_bottom_y = wall_top_y + 16;

    block_rect_tiles(
        &mut map,
        wall_left_x,
        wall_top_y,
        wall_right_x,
        wall_bottom_y,
    );

    let ts = config::TILE_SIZE as f32;
    let lead_x = wall_left_x as f32 * ts - ts;
    let lead_y = (wall_top_y as f32 + 7.5) * ts;
    let start_tile = (
        ((lead_x / ts).floor() as u32).min(map.size - 1),
        ((lead_y / ts).floor() as u32).min(map.size - 1),
    );
    let (side_spacing, rear_spacing) = vehicle_corner_wall_spawn_spacing(unit);
    let starts: Vec<(f32, f32)> = match unit_count {
        1 => vec![(lead_x, lead_y)],
        3 => vec![
            (lead_x, lead_y),
            (lead_x, lead_y + rear_spacing),
            (lead_x - side_spacing, lead_y),
        ],
        5 => vec![
            (lead_x, lead_y),
            (lead_x, lead_y + rear_spacing),
            (lead_x, lead_y + rear_spacing * 2.0),
            (lead_x - side_spacing, lead_y),
            (lead_x - side_spacing * 2.0, lead_y),
        ],
        _ => unreachable!("vehicle corner wall caller validates unit count"),
    };
    let wall_right_edge = (wall_right_x + 1) as f32 * ts;
    let goal = (wall_right_edge + ts * 0.5, lead_y);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = start_tile;
    }

    (map, start_tile, starts, goal)
}

#[allow(clippy::type_complexity)]
pub(super) fn vehicle_small_block_baseline_map(
    vehicle: EntityKind,
    pair_count: usize,
) -> (
    Map,
    (u32, u32),
    Vec<(f32, f32)>,
    Vec<(f32, f32)>,
    (f32, f32),
) {
    let mut map = flat_dev_map(1);
    let center_tile = (map.size / 2, map.size / 2 + 18);
    let ts = config::TILE_SIZE as f32;
    let start_y = (center_tile.1 as f32 + 0.5) * ts;
    let center_x = (center_tile.0 as f32 + 0.5) * ts;
    let spacing = vehicle_small_block_baseline_vehicle_spacing(vehicle);
    let blocker_offset_y = ts * 3.0;
    let center_index = (pair_count.saturating_sub(1)) as f32 * 0.5;
    let vehicle_starts: Vec<(f32, f32)> = (0..pair_count)
        .map(|i| {
            let offset = (i as f32 - center_index) * spacing;
            (center_x + offset, start_y)
        })
        .collect();
    let blocker_starts = vehicle_starts
        .iter()
        .map(|(x, y)| (*x, *y - blocker_offset_y))
        .collect();
    let goal = (center_x, start_y - ts * 20.0);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = center_tile;
    }

    (map, center_tile, vehicle_starts, blocker_starts, goal)
}

#[allow(clippy::type_complexity)]
pub(super) fn factory_zero_gap_perpendicular_map(
    unit: EntityKind,
) -> (Map, (u32, u32), (f32, f32), (f32, f32), (f32, f32)) {
    let mut map = flat_dev_map(1);
    let factory_tile = (map.size / 2 - 6, map.size / 2);
    let factory_pos = services::occupancy::footprint_center(
        &map,
        EntityKind::Factory,
        factory_tile.0,
        factory_tile.1,
    );
    let rect = services::geometry::building_rect_for_footprint(
        EntityKind::Factory,
        factory_tile.0,
        factory_tile.1,
    )
    .expect("factory footprint should have a rect");
    let ts = config::TILE_SIZE as f32;
    let side_radius = vehicle_zero_gap_side_radius(unit);
    let unit_start = (
        rect.max_x + side_radius + ZERO_GAP_STANDABLE_EPS_PX,
        (rect.min_y + rect.max_y) * 0.5,
    );
    let goal = (unit_start.0 + ts * 10.0, unit_start.1);
    let start_tile = (
        ((unit_start.0 / ts).floor() as u32).min(map.size - 1),
        ((unit_start.1 / ts).floor() as u32).min(map.size - 1),
    );
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = start_tile;
    }

    (map, start_tile, factory_pos, unit_start, goal)
}

#[allow(clippy::type_complexity)]
pub(super) fn command_car_building_corner_map() -> (
    Map,
    (u32, u32),
    [(EntityKind, f32, f32); 3],
    (f32, f32),
    f32,
    (f32, f32),
) {
    let mut map = flat_dev_map(1);
    let buildings = [
        (EntityKind::Factory, 3472.0, 3728.0),
        (EntityKind::TrainingCentre, 3440.0, 3648.0),
        (EntityKind::Barracks, 3536.0, 3584.0),
    ];
    let unit_start = (3507.0, 3664.0);
    let unit_facing = 2.823_079_3;
    let goal = (3216.0, 3472.0);
    let start_tile = (
        (unit_start.0 / config::TILE_SIZE as f32) as u32,
        (unit_start.1 / config::TILE_SIZE as f32) as u32,
    );
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = start_tile;
    }

    (map, start_tile, buildings, unit_start, unit_facing, goal)
}

/// Translated geometry from replay 104 at tick 7923: a 3x3 Factory sits one clear tile below a
/// two-tile-deep terrain wall, and its rally point is almost due west. The old spawn search chose
/// the tile immediately below the wall, where the default east-facing hull fit but could not turn
/// toward the rally point.
#[allow(clippy::type_complexity)]
pub(super) fn factory_wall_rally_spawn_map() -> (
    Map,
    (u32, u32),
    (f32, f32),
    (f32, f32),
    (f32, f32),
    (f32, f32),
) {
    let mut map = flat_dev_map(1);
    let factory_tile = (map.size / 2, map.size / 2);
    block_rect_tiles(
        &mut map,
        factory_tile.0 - 2,
        factory_tile.1 - 3,
        factory_tile.0 + 17,
        factory_tile.1 - 2,
    );

    let factory_pos = services::occupancy::footprint_center(
        &map,
        EntityKind::Factory,
        factory_tile.0,
        factory_tile.1,
    );
    let trapped_spawn = map.tile_center(factory_tile.0 - 2, factory_tile.1 - 1);
    let rotation_clear_spawn = map.tile_center(factory_tile.0 - 2, factory_tile.1);
    let rally = (factory_pos.0 - 701.482, factory_pos.1 - 59.5);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = factory_tile;
    }

    (
        map,
        factory_tile,
        factory_pos,
        trapped_spawn,
        rotation_clear_spawn,
        rally,
    )
}

pub(super) fn spawn_snaking_corridor_units(
    entities: &mut EntityStore,
    unit: EntityKind,
    unit_count: usize,
    start: (f32, f32),
) -> Result<Vec<u32>, String> {
    let north = -std::f32::consts::FRAC_PI_2;
    let (x_spacing, y_spacing) = snaking_corridor_spawn_spacing(unit)?;
    let positions: Vec<(f32, f32)> = match unit_count {
        1 => vec![start],
        4 => {
            vec![
                (start.0 - x_spacing * 0.5, start.1 - y_spacing * 0.5),
                (start.0 + x_spacing * 0.5, start.1 - y_spacing * 0.5),
                (start.0 - x_spacing * 0.5, start.1 + y_spacing * 0.5),
                (start.0 + x_spacing * 0.5, start.1 + y_spacing * 0.5),
            ]
        }
        _ => {
            return Err(format!(
                "unsupported snaking-corridor unit count {unit_count}"
            ))
        }
    };

    let mut units = Vec::with_capacity(positions.len());
    for (x, y) in positions {
        let spawned = entities
            .spawn_unit(1, unit, x, y)
            .ok_or_else(|| format!("failed to spawn {unit}"))?;
        if let Some(e) = entities.get_mut(spawned) {
            e.set_facing(north);
        }
        units.push(spawned);
    }
    Ok(units)
}

pub(super) fn spawn_wall_chokepoint_units(
    entities: &mut EntityStore,
    unit: EntityKind,
    starts: Vec<(f32, f32)>,
) -> Result<Vec<u32>, String> {
    let north = -std::f32::consts::FRAC_PI_2;
    let mut units = Vec::with_capacity(starts.len());
    for (x, y) in starts {
        let spawned = entities
            .spawn_unit(1, unit, x, y)
            .ok_or_else(|| format!("failed to spawn {unit}"))?;
        if let Some(e) = entities.get_mut(spawned) {
            e.set_facing(north);
        }
        units.push(spawned);
    }
    Ok(units)
}

pub(super) fn spawn_vehicle_small_block_baseline_units(
    entities: &mut EntityStore,
    vehicle: EntityKind,
    starts: Vec<(f32, f32)>,
) -> Result<Vec<u32>, String> {
    let north = -std::f32::consts::FRAC_PI_2;
    let mut units = Vec::with_capacity(starts.len());
    for (x, y) in starts {
        let spawned = entities
            .spawn_unit(1, vehicle, x, y)
            .ok_or_else(|| format!("failed to spawn {vehicle}"))?;
        if let Some(e) = entities.get_mut(spawned) {
            e.set_facing(north);
        }
        units.push(spawned);
    }
    Ok(units)
}

pub(super) fn spawn_vehicle_small_block_baseline_blockers(
    entities: &mut EntityStore,
    blocker: Option<EntityKind>,
    starts: Vec<(f32, f32)>,
) -> Result<(), String> {
    let Some(blocker) = blocker else {
        return Ok(());
    };
    let north = -std::f32::consts::FRAC_PI_2;
    for (x, y) in starts {
        let spawned = entities
            .spawn_unit(1, blocker, x, y)
            .ok_or_else(|| format!("failed to spawn {blocker} blocker"))?;
        if let Some(e) = entities.get_mut(spawned) {
            e.set_facing(north);
        }
    }
    Ok(())
}

pub(super) fn spawn_factory_zero_gap_perpendicular_units(
    entities: &mut EntityStore,
    unit: EntityKind,
    start: (f32, f32),
) -> Result<Vec<u32>, String> {
    let north = -std::f32::consts::FRAC_PI_2;
    let spawned = entities
        .spawn_unit(1, unit, start.0, start.1)
        .ok_or_else(|| format!("failed to spawn {unit}"))?;
    if let Some(e) = entities.get_mut(spawned) {
        e.set_facing(north);
    }
    Ok(vec![spawned])
}

pub(super) fn wall_chokepoint_spawn_spacing(unit: EntityKind) -> f32 {
    match unit {
        EntityKind::AntiTankGun => {
            config::ANTI_TANK_GUN_BODY_WIDTH_PX + config::ANTI_TANK_GUN_BODY_CLEARANCE_PX * 4.0
        }
        EntityKind::ScoutCar => {
            config::SCOUT_CAR_BODY_WIDTH_PX + config::SCOUT_CAR_BODY_CLEARANCE_PX * 4.0
        }
        EntityKind::Tank => config::TANK_BODY_WIDTH_PX + config::TANK_BODY_CLEARANCE_PX * 4.0,
        _ => unreachable!("wall chokepoint only supports vehicles"),
    }
}

pub(super) fn vehicle_corner_wall_spawn_spacing(unit: EntityKind) -> (f32, f32) {
    match unit {
        EntityKind::AntiTankGun => (
            config::ANTI_TANK_GUN_BODY_WIDTH_PX + config::ANTI_TANK_GUN_BODY_CLEARANCE_PX * 4.0,
            config::ANTI_TANK_GUN_BODY_LENGTH_PX + config::ANTI_TANK_GUN_BODY_CLEARANCE_PX * 4.0,
        ),
        EntityKind::ScoutCar => (
            config::SCOUT_CAR_BODY_WIDTH_PX + config::SCOUT_CAR_BODY_CLEARANCE_PX * 4.0,
            config::SCOUT_CAR_BODY_LENGTH_PX + config::SCOUT_CAR_BODY_CLEARANCE_PX * 4.0,
        ),
        EntityKind::Tank => (
            config::TANK_BODY_WIDTH_PX + config::TANK_BODY_CLEARANCE_PX * 4.0,
            config::TANK_BODY_LENGTH_PX + config::TANK_BODY_CLEARANCE_PX * 4.0,
        ),
        _ => unreachable!("vehicle corner wall only supports vehicles"),
    }
}

pub(super) fn vehicle_small_block_baseline_vehicle_spacing(vehicle: EntityKind) -> f32 {
    match vehicle {
        EntityKind::ScoutCar => config::SCOUT_CAR_BODY_WIDTH_PX + 2.0,
        EntityKind::Tank => config::TANK_BODY_WIDTH_PX + 2.0,
        _ => unreachable!("vehicle small-block baseline only supports vehicles"),
    }
}

pub(super) fn vehicle_zero_gap_side_radius(unit: EntityKind) -> f32 {
    match unit {
        EntityKind::AntiTankGun => {
            config::ANTI_TANK_GUN_BODY_WIDTH_PX * 0.5 + config::ANTI_TANK_GUN_BODY_CLEARANCE_PX
        }
        EntityKind::ScoutCar => {
            config::SCOUT_CAR_BODY_WIDTH_PX * 0.5 + config::SCOUT_CAR_BODY_CLEARANCE_PX
        }
        EntityKind::Tank => config::TANK_BODY_WIDTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX,
        _ => unreachable!("factory zero-gap only supports vehicles"),
    }
}

pub(super) fn snaking_corridor_spawn_spacing(unit: EntityKind) -> Result<(f32, f32), String> {
    match unit {
        EntityKind::AntiTankGun => Ok((
            config::ANTI_TANK_GUN_BODY_WIDTH_PX * 1.5,
            config::ANTI_TANK_GUN_BODY_LENGTH_PX * 1.5,
        )),
        EntityKind::ScoutCar => Ok((
            config::SCOUT_CAR_BODY_WIDTH_PX * 1.5,
            config::SCOUT_CAR_BODY_LENGTH_PX * 1.5,
        )),
        EntityKind::Tank => Ok((
            config::TANK_BODY_WIDTH_PX * 1.5,
            config::TANK_BODY_LENGTH_PX * 1.5,
        )),
        _ => {
            let radius = config::unit_stats(unit)
                .ok_or_else(|| format!("missing stats for snaking-corridor unit {unit}"))?
                .radius;
            let spacing = radius * 3.0;
            Ok((spacing, spacing))
        }
    }
}
