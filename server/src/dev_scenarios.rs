use crate::game::entity::EntityKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevScenarioLaunch {
    pub id: &'static str,
    pub unit: EntityKind,
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevScenarioSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub launches: &'static [DevScenarioLaunch],
}

const SCOUT_CAR_SNAKING_CORRIDOR_LAUNCHES: [DevScenarioLaunch; 12] = [
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Worker,
        count: 1,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Worker,
        count: 4,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Rifleman,
        count: 1,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Rifleman,
        count: 4,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::MachineGunner,
        count: 1,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::MachineGunner,
        count: 4,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::AtTeam,
        count: 1,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::AtTeam,
        count: 4,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::ScoutCar,
        count: 1,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::ScoutCar,
        count: 4,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Tank,
        count: 1,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Tank,
        count: 4,
    },
];

const DIRECT_REVERSE_ORDER_LAUNCHES: [DevScenarioLaunch; 3] = [
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::AtTeam,
        count: 1,
    },
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::ScoutCar,
        count: 1,
    },
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::Tank,
        count: 1,
    },
];

const WALL_CHOKEPOINT_VEHICLE_LAUNCHES: [DevScenarioLaunch; 15] = [
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AtTeam,
        count: 3,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AtTeam,
        count: 5,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AtTeam,
        count: 6,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AtTeam,
        count: 10,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AtTeam,
        count: 15,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 3,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 5,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 6,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 10,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 15,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 3,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 5,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 6,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 10,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 15,
    },
];

const VEHICLE_SMALL_BLOCK_BASELINE_LAUNCHES: [DevScenarioLaunch; 6] = [
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
    },
];

const DEV_SCENARIOS: [DevScenarioSpec; 4] = [
    DevScenarioSpec {
        id: "scout_car_snaking_corridor",
        title: "Scout Car Snaking Corridor",
        description: "Movement/pathing debug run through a narrow authored corridor.",
        launches: &SCOUT_CAR_SNAKING_CORRIDOR_LAUNCHES,
    },
    DevScenarioSpec {
        id: "direct_reverse_order",
        title: "Direct Reverse Order",
        description:
            "Single vehicle faces east, then receives a move order 15 tiles directly behind it.",
        launches: &DIRECT_REVERSE_ORDER_LAUNCHES,
    },
    DevScenarioSpec {
        id: "scout_car_wall_chokepoint",
        title: "Vehicle Wall Chokepoint",
        description: "Vehicles start beside each other below a stone wall gap and move north through the choke.",
        launches: &WALL_CHOKEPOINT_VEHICLE_LAUNCHES,
    },
    DevScenarioSpec {
        id: "vehicle_small_block_baseline",
        title: "Vehicle Small-Unit Block Baseline",
        description: "Vehicles start almost bumper-to-bumper with a worker one tile north of each vehicle, then all vehicles move 20 tiles north.",
        launches: &VEHICLE_SMALL_BLOCK_BASELINE_LAUNCHES,
    },
];

pub fn all_dev_scenarios() -> &'static [DevScenarioSpec] {
    &DEV_SCENARIOS
}

pub fn parse_dev_scenario_launch(id: &str, unit: &str, count: &str) -> Option<DevScenarioLaunch> {
    let unit = unit.parse::<EntityKind>().ok()?;
    if !unit.is_unit() {
        return None;
    }
    let count = count.parse::<usize>().ok()?;
    all_dev_scenarios()
        .iter()
        .flat_map(|scenario| scenario.launches.iter())
        .copied()
        .find(|launch| launch.id == id && launch.unit == unit && launch.count == count)
}

pub fn parse_dev_scenario_room(raw: &str) -> Option<DevScenarioLaunch> {
    let (id, rest) = raw.split_once(":unit=")?;
    let (unit, count) = rest.split_once(":count=")?;
    parse_dev_scenario_launch(id, unit, count)
}

pub fn dev_scenario_unit_label(unit: EntityKind) -> &'static str {
    match unit {
        EntityKind::Worker => "worker",
        EntityKind::Rifleman => "rifleman",
        EntityKind::MachineGunner => "machine gunner",
        EntityKind::AtTeam => "AT gun",
        EntityKind::ScoutCar => "scout car",
        EntityKind::Tank => "tank",
        _ => "unit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_launches() {
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "worker", "1"),
            Some(DevScenarioLaunch {
                id: "scout_car_snaking_corridor",
                unit: EntityKind::Worker,
                count: 1,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_snaking_corridor:unit=tank:count=4"),
            Some(DevScenarioLaunch {
                id: "scout_car_snaking_corridor",
                unit: EntityKind::Tank,
                count: 4,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("direct_reverse_order:unit=at_team:count=1"),
            Some(DevScenarioLaunch {
                id: "direct_reverse_order",
                unit: EntityKind::AtTeam,
                count: 1,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=at_team:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::AtTeam,
                count: 15,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=scout_car:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::ScoutCar,
                count: 15,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=tank:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::Tank,
                count: 15,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_small_block_baseline:unit=scout_car:count=5"),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::ScoutCar,
                count: 5,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_small_block_baseline:unit=tank:count=5"),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::Tank,
                count: 5,
            })
        );
    }

    #[test]
    fn rejects_unknown_launches() {
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "tank", "2"),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "city_centre", "1"),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("direct_reverse_order", "worker", "1"),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("direct_reverse_order", "tank", "4"),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_wall_chokepoint", "worker", "3"),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_wall_chokepoint", "scout_car", "4"),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_small_block_baseline", "worker", "1"),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_small_block_baseline", "tank", "4"),
            None
        );
        assert_eq!(parse_dev_scenario_launch("unknown", "worker", "1"), None);
        assert_eq!(parse_dev_scenario_room("scout_car_snaking_corridor"), None);
    }
}
