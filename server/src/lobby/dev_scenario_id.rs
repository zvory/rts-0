#[derive(Clone)]
pub(in crate::lobby) enum DevScenarioId {
    DynamicConstructionPathBlock,
    ScoutCarSnakingCorridor,
    DirectReverseOrder,
    Replay142VehicleLock,
    ScoutCarWallChokepoint,
    VehicleCornerWall,
    VehicleSmallBlockBaseline,
    FactoryZeroGapPerpendicular,
    CommandCarBuildingCorner,
    CommandCarBuildingCornerWestSouthwest,
    FactoryWallRallySpawn,
    TankTrapLineHorizontal,
    TankTrapLineVertical,
    TankTrapLineDiagonal,
    TankTrapPathingMatrix,
    EntrenchmentInspection,
    TankCoaxInspection,
    AttackMoveReloadAcquisition,
}

impl DevScenarioId {
    pub(in crate::lobby) fn from_room_id(id: &str) -> Option<Self> {
        match id {
            "dynamic_construction_path_block" => Some(Self::DynamicConstructionPathBlock),
            "scout_car_snaking_corridor" => Some(Self::ScoutCarSnakingCorridor),
            "direct_reverse_order" => Some(Self::DirectReverseOrder),
            "replay_142_vehicle_lock" => Some(Self::Replay142VehicleLock),
            "scout_car_wall_chokepoint" => Some(Self::ScoutCarWallChokepoint),
            "vehicle_corner_wall" => Some(Self::VehicleCornerWall),
            "vehicle_small_block_baseline" => Some(Self::VehicleSmallBlockBaseline),
            "factory_zero_gap_perpendicular" => Some(Self::FactoryZeroGapPerpendicular),
            "command_car_building_corner" => Some(Self::CommandCarBuildingCorner),
            "command_car_building_corner_west_southwest" => {
                Some(Self::CommandCarBuildingCornerWestSouthwest)
            }
            "factory_wall_rally_spawn" => Some(Self::FactoryWallRallySpawn),
            "tank_trap_line_horizontal" => Some(Self::TankTrapLineHorizontal),
            "tank_trap_line_vertical" => Some(Self::TankTrapLineVertical),
            "tank_trap_line_diagonal" => Some(Self::TankTrapLineDiagonal),
            "tank_trap_pathing_matrix" => Some(Self::TankTrapPathingMatrix),
            "entrenchment_inspection" => Some(Self::EntrenchmentInspection),
            "tank_coax_inspection" => Some(Self::TankCoaxInspection),
            "attack_move_reload_acquisition" => Some(Self::AttackMoveReloadAcquisition),
            _ => None,
        }
    }

    pub(in crate::lobby) fn room_id(&self) -> &'static str {
        match self {
            Self::DynamicConstructionPathBlock => "dynamic_construction_path_block",
            Self::ScoutCarSnakingCorridor => "scout_car_snaking_corridor",
            Self::DirectReverseOrder => "direct_reverse_order",
            Self::Replay142VehicleLock => "replay_142_vehicle_lock",
            Self::ScoutCarWallChokepoint => "scout_car_wall_chokepoint",
            Self::VehicleCornerWall => "vehicle_corner_wall",
            Self::VehicleSmallBlockBaseline => "vehicle_small_block_baseline",
            Self::FactoryZeroGapPerpendicular => "factory_zero_gap_perpendicular",
            Self::CommandCarBuildingCorner => "command_car_building_corner",
            Self::CommandCarBuildingCornerWestSouthwest => {
                "command_car_building_corner_west_southwest"
            }
            Self::FactoryWallRallySpawn => "factory_wall_rally_spawn",
            Self::TankTrapLineHorizontal => "tank_trap_line_horizontal",
            Self::TankTrapLineVertical => "tank_trap_line_vertical",
            Self::TankTrapLineDiagonal => "tank_trap_line_diagonal",
            Self::TankTrapPathingMatrix => "tank_trap_pathing_matrix",
            Self::EntrenchmentInspection => "entrenchment_inspection",
            Self::TankCoaxInspection => "tank_coax_inspection",
            Self::AttackMoveReloadAcquisition => "attack_move_reload_acquisition",
        }
    }
}
