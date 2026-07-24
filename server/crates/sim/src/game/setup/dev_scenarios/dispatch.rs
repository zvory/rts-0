use super::*;

impl Game {
    #[allow(clippy::too_many_arguments)]
    pub fn new_dev_scenario(
        scenario_id: &str,
        unit: EntityKind,
        unit_count: usize,
        blocker: Option<EntityKind>,
        scenario_case: Option<&str>,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        match scenario_id {
            "dynamic_construction_path_block" => {
                Self::new_dynamic_construction_path_block_scenario(
                    scenario_case,
                    unit,
                    unit_count,
                    seed,
                )
            }
            "scout_car_snaking_corridor" => {
                Self::new_snaking_corridor_scenario(unit, unit_count, seed)
            }
            "direct_reverse_order" => {
                Self::new_direct_reverse_order_scenario(unit, unit_count, seed)
            }
            "scout_car_open_ground_l_path" => {
                Self::new_scout_car_open_ground_l_path_scenario(unit, unit_count, seed)
            }
            "replay_142_vehicle_lock" => {
                Self::new_replay_142_vehicle_lock_scenario(unit, unit_count, seed)
            }
            "scout_car_wall_chokepoint" => {
                Self::new_scout_car_wall_chokepoint_scenario(unit, unit_count, seed)
            }
            "vehicle_corner_wall" => Self::new_vehicle_corner_wall_scenario(unit, unit_count, seed),
            "vehicle_small_block_baseline" => {
                Self::new_vehicle_small_block_baseline_scenario(unit, unit_count, blocker, seed)
            }
            "factory_zero_gap_perpendicular" => {
                Self::new_factory_zero_gap_perpendicular_scenario(unit, unit_count, seed)
            }
            "command_car_building_corner" => {
                Self::new_command_car_corner_scenario(unit, unit_count, seed)
            }
            "command_car_building_corner_west_southwest" => {
                Self::new_command_car_corner_west_southwest_scenario(unit, unit_count, seed)
            }
            "factory_wall_rally_spawn" => {
                Self::new_factory_wall_rally_spawn_scenario(unit, unit_count, seed)
            }
            "tank_trap_line_horizontal" | "tank_trap_line_vertical" | "tank_trap_line_diagonal" => {
                Self::new_tank_trap_line_build_scenario(scenario_id, unit, unit_count, seed)
            }
            "tank_trap_pathing_matrix" => Self::new_tank_trap_pathing_scenario(
                scenario_case.ok_or_else(|| "missing Tank Trap pathing case".to_string())?,
                unit,
                unit_count,
                seed,
            ),
            "entrenchment_inspection" => {
                Self::new_entrenchment_inspection_scenario(unit, unit_count, seed)
            }
            "tank_coax_inspection" => {
                Self::new_tank_coax_inspection_scenario(unit, unit_count, seed)
            }
            "attack_move_reload_acquisition" => {
                Self::new_attack_move_reload_acquisition_scenario(unit, unit_count, seed)
            }
            "tank_under_fire_retreat" => {
                Self::new_tank_under_fire_retreat_scenario(unit, unit_count, seed)
            }
            "tank_reverse_traffic" => {
                Self::new_tank_reverse_traffic_scenario(unit, unit_count, seed)
            }
            _ => Err(format!("unknown dev scenario: {scenario_id}")),
        }
    }
}
