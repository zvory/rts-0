#[derive(Clone)]
pub(in crate::lobby) enum DevScenarioId {
    ScoutCarSnakingCorridor,
    DirectReverseOrder,
    ScoutCarWallChokepoint,
    VehicleCornerWall,
    VehicleSmallBlockBaseline,
    FactoryZeroGapPerpendicular,
    CommandCarBuildingCorner,
    FactoryWallRallySpawn,
    TankTrapLineHorizontal,
    TankTrapLineVertical,
    TankTrapLineDiagonal,
    TankTrapPathingMatrix,
    EntrenchmentInspection,
    PanzerfaustDuel,
    PanzerfaustWindupCancel,
    PanzerfaustTargetDeath,
    PanzerfaustEntrenchedRange,
    PanzerfaustMethamphetamines,
    TankCoaxInspection,
    SupplyStressActive,
}

impl DevScenarioId {
    pub(in crate::lobby) fn from_room_id(id: &str) -> Option<Self> {
        match id {
            "scout_car_snaking_corridor" => Some(Self::ScoutCarSnakingCorridor),
            "direct_reverse_order" => Some(Self::DirectReverseOrder),
            "scout_car_wall_chokepoint" => Some(Self::ScoutCarWallChokepoint),
            "vehicle_corner_wall" => Some(Self::VehicleCornerWall),
            "vehicle_small_block_baseline" => Some(Self::VehicleSmallBlockBaseline),
            "factory_zero_gap_perpendicular" => Some(Self::FactoryZeroGapPerpendicular),
            "command_car_building_corner" => Some(Self::CommandCarBuildingCorner),
            "factory_wall_rally_spawn" => Some(Self::FactoryWallRallySpawn),
            "tank_trap_line_horizontal" => Some(Self::TankTrapLineHorizontal),
            "tank_trap_line_vertical" => Some(Self::TankTrapLineVertical),
            "tank_trap_line_diagonal" => Some(Self::TankTrapLineDiagonal),
            "tank_trap_pathing_matrix" => Some(Self::TankTrapPathingMatrix),
            "entrenchment_inspection" => Some(Self::EntrenchmentInspection),
            "panzerfaust_duel" => Some(Self::PanzerfaustDuel),
            "panzerfaust_windup_cancel" => Some(Self::PanzerfaustWindupCancel),
            "panzerfaust_target_death" => Some(Self::PanzerfaustTargetDeath),
            "panzerfaust_entrenched_range" => Some(Self::PanzerfaustEntrenchedRange),
            "panzerfaust_methamphetamines" => Some(Self::PanzerfaustMethamphetamines),
            "tank_coax_inspection" => Some(Self::TankCoaxInspection),
            "supply_stress_active" => Some(Self::SupplyStressActive),
            _ => None,
        }
    }

    pub(in crate::lobby) fn room_id(&self) -> &'static str {
        match self {
            Self::ScoutCarSnakingCorridor => "scout_car_snaking_corridor",
            Self::DirectReverseOrder => "direct_reverse_order",
            Self::ScoutCarWallChokepoint => "scout_car_wall_chokepoint",
            Self::VehicleCornerWall => "vehicle_corner_wall",
            Self::VehicleSmallBlockBaseline => "vehicle_small_block_baseline",
            Self::FactoryZeroGapPerpendicular => "factory_zero_gap_perpendicular",
            Self::CommandCarBuildingCorner => "command_car_building_corner",
            Self::FactoryWallRallySpawn => "factory_wall_rally_spawn",
            Self::TankTrapLineHorizontal => "tank_trap_line_horizontal",
            Self::TankTrapLineVertical => "tank_trap_line_vertical",
            Self::TankTrapLineDiagonal => "tank_trap_line_diagonal",
            Self::TankTrapPathingMatrix => "tank_trap_pathing_matrix",
            Self::EntrenchmentInspection => "entrenchment_inspection",
            Self::PanzerfaustDuel => "panzerfaust_duel",
            Self::PanzerfaustWindupCancel => "panzerfaust_windup_cancel",
            Self::PanzerfaustTargetDeath => "panzerfaust_target_death",
            Self::PanzerfaustEntrenchedRange => "panzerfaust_entrenched_range",
            Self::PanzerfaustMethamphetamines => "panzerfaust_methamphetamines",
            Self::TankCoaxInspection => "tank_coax_inspection",
            Self::SupplyStressActive => "supply_stress_active",
        }
    }
}
