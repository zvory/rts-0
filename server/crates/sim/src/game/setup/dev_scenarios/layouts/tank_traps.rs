use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game::setup::dev_scenarios) enum TankTrapLineLayout {
    Horizontal,
    Vertical,
    Diagonal,
}

impl TankTrapLineLayout {
    pub(in crate::game::setup::dev_scenarios) fn from_scenario_id(id: &str) -> Option<Self> {
        match id {
            "tank_trap_line_horizontal" => Some(Self::Horizontal),
            "tank_trap_line_vertical" => Some(Self::Vertical),
            "tank_trap_line_diagonal" => Some(Self::Diagonal),
            _ => None,
        }
    }

    pub(in crate::game::setup::dev_scenarios) fn scenario_id(self) -> &'static str {
        match self {
            Self::Horizontal => "dev:tank_trap_line_horizontal",
            Self::Vertical => "dev:tank_trap_line_vertical",
            Self::Diagonal => "dev:tank_trap_line_diagonal",
        }
    }
}

#[allow(clippy::type_complexity)]
pub(in crate::game::setup::dev_scenarios) fn tank_trap_line_build_map(
    layout: TankTrapLineLayout,
    vehicle: EntityKind,
) -> (
    Map,
    (u32, u32),
    (f32, f32),
    Vec<(f32, f32)>,
    Vec<(f32, f32)>,
    (f32, f32),
) {
    let mut map = flat_dev_map(1);
    let ts = config::TILE_SIZE as f32;
    let center = (map.size / 2, map.size / 2);
    let training_pos = services::occupancy::footprint_center(
        &map,
        EntityKind::TrainingCentre,
        center.0 - 8,
        center.1 + 6,
    );
    let worker_y = (center.1 as f32 + 0.5) * ts;
    let worker_starts = vec![
        ((center.0 as f32 - 2.0) * ts, worker_y),
        ((center.0 as f32 - 1.0) * ts, worker_y),
        (center.0 as f32 * ts, worker_y),
    ];
    let vehicle_gap = vehicle_line_start_gap(vehicle);
    let (unit_starts, goal) = match layout {
        TankTrapLineLayout::Horizontal => {
            let y = (center.1 as f32 - 3.5) * ts;
            let start_x = (center.0 as f32 - 5.0) * ts;
            (
                vec![(start_x, y - vehicle_gap), (start_x, y + vehicle_gap)],
                ((center.0 as f32 + 5.0) * ts, y),
            )
        }
        TankTrapLineLayout::Vertical => {
            let x = (center.0 as f32 - 1.5) * ts;
            let start_y = (center.1 as f32 - 7.0) * ts;
            (
                vec![(x - vehicle_gap, start_y), (x + vehicle_gap, start_y)],
                (x, (center.1 as f32 + 3.0) * ts),
            )
        }
        TankTrapLineLayout::Diagonal => {
            let start = (
                (center.0 as f32 - 6.0) * ts,
                (center.1 as f32 - 6.0) * ts,
            );
            (
                vec![start, (start.0, start.1 + vehicle_gap * 2.0)],
                ((center.0 as f32 + 4.0) * ts, (center.1 as f32 + 4.0) * ts),
            )
        }
    };
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = center;
    }

    (map, center, training_pos, worker_starts, unit_starts, goal)
}

pub(in crate::game::setup::dev_scenarios) fn spawn_tank_trap_line_test_units(
    entities: &mut EntityStore,
    vehicle: EntityKind,
    starts: Vec<(f32, f32)>,
) -> Result<Vec<u32>, String> {
    let rifleman_start = starts
        .first()
        .copied()
        .ok_or_else(|| "missing rifleman start".to_string())?;
    let vehicle_start = starts
        .get(1)
        .copied()
        .ok_or_else(|| "missing vehicle start".to_string())?;
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, rifleman_start.0, rifleman_start.1)
        .ok_or_else(|| "failed to spawn rifleman".to_string())?;
    let vehicle = entities
        .spawn_unit(1, vehicle, vehicle_start.0, vehicle_start.1)
        .ok_or_else(|| format!("failed to spawn {vehicle}"))?;
    Ok(vec![rifleman, vehicle])
}

pub(in crate::game::setup::dev_scenarios) fn spawn_tank_trap_line_workers(
    entities: &mut EntityStore,
    starts: Vec<(f32, f32)>,
) -> Result<(), String> {
    for (x, y) in starts {
        entities
            .spawn_unit(1, EntityKind::Worker, x, y)
            .ok_or_else(|| "failed to spawn Tank Trap scenario worker".to_string())?;
    }
    Ok(())
}

fn vehicle_line_start_gap(unit: EntityKind) -> f32 {
    match unit {
        EntityKind::AntiTankGun | EntityKind::MortarTeam => {
            config::ANTI_TANK_GUN_BODY_LENGTH_PX
        }
        EntityKind::Artillery => config::ARTILLERY_BODY_LENGTH_PX,
        EntityKind::ScoutCar => config::SCOUT_CAR_BODY_LENGTH_PX,
        EntityKind::Tank => config::TANK_BODY_LENGTH_PX,
        EntityKind::CommandCar => config::COMMAND_CAR_BODY_LENGTH_PX,
        _ => config::TILE_SIZE as f32,
    }
}
