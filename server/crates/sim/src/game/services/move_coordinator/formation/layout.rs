use super::{FormationUnit, VEHICLE_BODY_FORMATION_GAP_TILES};
use crate::game::entity::uses_oriented_vehicle_body;
use crate::game::map::Map;

/// Build one compact translated layout. Wide selections fold into ordered columns and tall
/// selections fold into ordered rows, preserving the strongest positional relationship without
/// allowing the current aspect ratio to turn the destination into a line. Original world-space
/// separation is discarded. Infantry occupies adjacent tiles; vehicles use a two-tile pitch.
pub(super) fn compact_formation_points(
    map: &Map,
    units: &[FormationUnit],
    center: (f32, f32),
) -> Vec<(f32, f32)> {
    if units.len() <= 1 {
        return units.iter().map(|_| center).collect();
    }

    let (min_x, max_x, min_y, max_y) = units.iter().fold(
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), unit| {
            (
                min_x.min(unit.pos.0),
                max_x.max(unit.pos.0),
                min_y.min(unit.pos.1),
                max_y.max(unit.pos.1),
            )
        },
    );
    let width = max_x - min_x;
    let height = max_y - min_y;
    let has_vehicle = units
        .iter()
        .any(|unit| uses_oriented_vehicle_body(unit.kind));
    let (columns, rows) = compact_grid_dimensions(units.len(), width, height, has_vehicle);
    let pitch_tiles = if has_vehicle {
        VEHICLE_BODY_FORMATION_GAP_TILES + 1
    } else {
        1
    };
    let width_tiles = (columns.min(units.len()) as u32).saturating_sub(1) * pitch_tiles;
    let height_tiles = (rows as u32).saturating_sub(1) * pitch_tiles;
    let center_tile = map.tile_of(center.0, center.1);
    let (axis, min_minor, minor_span, reverse_minor) = if width >= height {
        (
            CompactAxis::Horizontal,
            min_y,
            height,
            center.1 > (min_y + max_y) * 0.5,
        )
    } else {
        (
            CompactAxis::Vertical,
            min_x,
            width,
            center.0 > (min_x + max_x) * 0.5,
        )
    };
    let grid = CompactGrid {
        columns,
        rows,
        start_x: centered_tile_start(center_tile.0, width_tiles, map.size),
        start_y: centered_tile_start(center_tile.1, height_tiles, map.size),
        pitch_tiles,
        reverse_minor: reverse_minor && minor_span <= f32::EPSILON,
        has_vehicle,
    };

    let mut points = vec![center; units.len()];
    assign_compact_groups(map, units, &mut points, grid, axis, min_minor, minor_span);
    points
}

fn compact_grid_dimensions(
    unit_count: usize,
    width: f32,
    height: f32,
    has_vehicle: bool,
) -> (usize, usize) {
    // Larger vehicle bodies need a slightly wider blob so they do not form deep same-lane queues
    // while converging through ordinary traffic chokepoints.
    let long_side =
        (unit_count as f32).sqrt().ceil() as usize + usize::from(has_vehicle && unit_count >= 6);
    let short_side = unit_count.div_ceil(long_side);
    let columns = if width >= height {
        long_side
    } else {
        short_side
    };
    (columns, unit_count.div_ceil(columns))
}

#[derive(Clone, Copy)]
struct CompactGrid {
    columns: usize,
    rows: usize,
    start_x: u32,
    start_y: u32,
    pitch_tiles: u32,
    reverse_minor: bool,
    has_vehicle: bool,
}

#[derive(Clone, Copy)]
enum CompactAxis {
    Horizontal,
    Vertical,
}

impl CompactAxis {
    fn major_coordinate(self, unit: &FormationUnit) -> f32 {
        match self {
            Self::Horizontal => unit.pos.0,
            Self::Vertical => unit.pos.1,
        }
    }

    fn minor_coordinate(self, unit: &FormationUnit) -> f32 {
        match self {
            Self::Horizontal => unit.pos.1,
            Self::Vertical => unit.pos.0,
        }
    }

    fn group_count(self, grid: CompactGrid) -> usize {
        match self {
            Self::Horizontal => grid.columns,
            Self::Vertical => grid.rows,
        }
    }

    fn minor_slot_count(self, grid: CompactGrid) -> usize {
        match self {
            Self::Horizontal => grid.rows,
            Self::Vertical => grid.columns,
        }
    }

    fn tile(self, grid: CompactGrid, group: usize, minor: usize) -> (u32, u32) {
        let minor = if grid.reverse_minor {
            self.minor_slot_count(grid) - 1 - minor
        } else {
            minor
        };
        let (column, row) = match self {
            Self::Horizontal => (group, minor),
            Self::Vertical => (minor, group),
        };
        (
            grid.start_x + column as u32 * grid.pitch_tiles,
            grid.start_y + row as u32 * grid.pitch_tiles,
        )
    }
}

fn assign_compact_groups(
    map: &Map,
    units: &[FormationUnit],
    points: &mut [(f32, f32)],
    grid: CompactGrid,
    axis: CompactAxis,
    min_minor: f32,
    minor_span: f32,
) {
    let mut ordered = (0..units.len()).collect::<Vec<_>>();
    ordered.sort_by(|&a, &b| {
        axis.major_coordinate(&units[a])
            .total_cmp(&axis.major_coordinate(&units[b]))
            .then_with(|| {
                axis.minor_coordinate(&units[a])
                    .total_cmp(&axis.minor_coordinate(&units[b]))
            })
            .then_with(|| units[a].id.cmp(&units[b].id))
    });
    let group_count = axis.group_count(grid);
    let minor_slot_count = axis.minor_slot_count(grid);
    for group in 0..group_count {
        let range = balanced_group_range(units.len(), group_count, group);
        let group_units = &mut ordered[range];
        group_units.sort_by(|&a, &b| {
            axis.minor_coordinate(&units[a])
                .total_cmp(&axis.minor_coordinate(&units[b]))
                .then_with(|| {
                    axis.major_coordinate(&units[a])
                        .total_cmp(&axis.major_coordinate(&units[b]))
                })
                .then_with(|| units[a].id.cmp(&units[b].id))
        });
        if grid.has_vehicle && minor_span <= f32::EPSILON && group * 2 < group_count {
            group_units.reverse();
        }
        let minor_offset = compact_group_start(
            units,
            group_units,
            min_minor,
            minor_span,
            minor_slot_count,
            |unit| axis.minor_coordinate(unit),
        );
        for (minor, &unit_index) in group_units.iter().enumerate() {
            let tile = axis.tile(grid, group, minor_offset + minor);
            points[unit_index] = map.tile_center(tile.0, tile.1);
        }
    }
}

fn balanced_group_range(
    item_count: usize,
    group_count: usize,
    group_index: usize,
) -> std::ops::Range<usize> {
    let base_size = item_count / group_count;
    let larger_groups = item_count % group_count;
    let larger_start = (group_count - larger_groups) / 2;
    let larger_end = larger_start + larger_groups;
    let larger_before = group_index.saturating_sub(larger_start).min(larger_groups);
    let start = group_index * base_size + larger_before;
    let size = base_size + usize::from((larger_start..larger_end).contains(&group_index));
    start..start + size
}

fn compact_group_start<F>(
    units: &[FormationUnit],
    group_units: &[usize],
    min_coordinate: f32,
    span: f32,
    slot_count: usize,
    coordinate: F,
) -> usize
where
    F: Fn(&FormationUnit) -> f32,
{
    if group_units.len() >= slot_count {
        return 0;
    }
    if span <= f32::EPSILON {
        return ((slot_count - group_units.len()) as f32 * 0.5).round() as usize;
    }
    let mean_coordinate = group_units
        .iter()
        .map(|&index| coordinate(&units[index]))
        .sum::<f32>()
        / group_units.len() as f32;
    let desired_center = ((mean_coordinate - min_coordinate) / span) * (slot_count - 1) as f32;
    let half_group = (group_units.len() - 1) as f32 * 0.5;
    (desired_center - half_group)
        .round()
        .clamp(0.0, (slot_count - group_units.len()) as f32) as usize
}

fn centered_tile_start(center: u32, span: u32, map_size: u32) -> u32 {
    center
        .saturating_sub(span.div_ceil(2))
        .min(map_size.saturating_sub(span.saturating_add(1)))
}
