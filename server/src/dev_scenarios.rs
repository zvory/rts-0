use crate::game::entity::EntityKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevScenarioLaunch {
    pub id: &'static str,
    pub unit: EntityKind,
    pub count: usize,
    pub blocker: Option<EntityKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevScenarioSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub launches: &'static [DevScenarioLaunch],
}

const EKATERINA_OPENING_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "ekaterina_opening",
    unit: EntityKind::EkaterinaEngineer,
    count: 1,
    blocker: None,
}];

const SCOUT_CAR_SNAKING_CORRIDOR_LAUNCHES: [DevScenarioLaunch; 12] = [
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Worker,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Worker,
        count: 4,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Rifleman,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Rifleman,
        count: 4,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::MachineGunner,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::MachineGunner,
        count: 4,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::AntiTankGun,
        count: 4,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::ScoutCar,
        count: 4,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Tank,
        count: 4,
        blocker: None,
    },
];

const DIRECT_REVERSE_ORDER_LAUNCHES: [DevScenarioLaunch; 3] = [
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
    },
];

const WALL_CHOKEPOINT_VEHICLE_LAUNCHES: [DevScenarioLaunch; 15] = [
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 3,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 5,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 6,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 10,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 15,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 6,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 10,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 15,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 3,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 6,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 10,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 15,
        blocker: None,
    },
];

const VEHICLE_CORNER_WALL_LAUNCHES: [DevScenarioLaunch; 9] = [
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::AntiTankGun,
        count: 3,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::AntiTankGun,
        count: 5,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::Tank,
        count: 3,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
    },
];

const VEHICLE_SMALL_BLOCK_BASELINE_LAUNCHES: [DevScenarioLaunch; 30] = [
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: Some(EntityKind::Worker),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: Some(EntityKind::Worker),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: Some(EntityKind::Worker),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: Some(EntityKind::Rifleman),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: Some(EntityKind::Rifleman),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: Some(EntityKind::Rifleman),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: Some(EntityKind::MachineGunner),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: Some(EntityKind::MachineGunner),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: Some(EntityKind::MachineGunner),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: Some(EntityKind::AntiTankGun),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: Some(EntityKind::AntiTankGun),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: Some(EntityKind::AntiTankGun),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: Some(EntityKind::Worker),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: Some(EntityKind::Worker),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: Some(EntityKind::Worker),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: Some(EntityKind::Rifleman),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: Some(EntityKind::Rifleman),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: Some(EntityKind::Rifleman),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: Some(EntityKind::MachineGunner),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: Some(EntityKind::MachineGunner),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: Some(EntityKind::MachineGunner),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: Some(EntityKind::AntiTankGun),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: Some(EntityKind::AntiTankGun),
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: Some(EntityKind::AntiTankGun),
    },
];

const FACTORY_ZERO_GAP_PERPENDICULAR_LAUNCHES: [DevScenarioLaunch; 3] = [
    DevScenarioLaunch {
        id: "factory_zero_gap_perpendicular",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "factory_zero_gap_perpendicular",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
    },
    DevScenarioLaunch {
        id: "factory_zero_gap_perpendicular",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
    },
];

const DEV_SCENARIOS: [DevScenarioSpec; 7] = [
    DevScenarioSpec {
        id: "ekaterina_opening",
        title: "Ekaterina Opening",
        description: "Phase 10 Ekaterina start, resources, supply, and first production loop.",
        launches: &EKATERINA_OPENING_LAUNCHES,
    },
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
        id: "vehicle_corner_wall",
        title: "Vehicle Corner Wall",
        description: "One, three, or five vehicles start in a flipped-L shape half a tile west of a three-tile-wide stone wall spur, then move to the east side to test cornering.",
        launches: &VEHICLE_CORNER_WALL_LAUNCHES,
    },
    DevScenarioSpec {
        id: "vehicle_small_block_baseline",
        title: "Vehicle Small-Unit Block Baseline",
        description: "Vehicles start almost bumper-to-bumper with optional small-unit blockers one tile north of each vehicle, then all vehicles move 20 tiles north.",
        launches: &VEHICLE_SMALL_BLOCK_BASELINE_LAUNCHES,
    },
    DevScenarioSpec {
        id: "factory_zero_gap_perpendicular",
        title: "Factory Zero-Gap Perpendicular",
        description: "One vehicle starts flush against the east side of a factory, waits half a second, then moves ten tiles directly east.",
        launches: &FACTORY_ZERO_GAP_PERPENDICULAR_LAUNCHES,
    },
];

pub fn all_dev_scenarios() -> &'static [DevScenarioSpec] {
    &DEV_SCENARIOS
}

pub fn parse_dev_scenario_launch(
    id: &str,
    unit: &str,
    count: &str,
    blocker: Option<&str>,
) -> Option<DevScenarioLaunch> {
    let unit = unit.parse::<EntityKind>().ok()?;
    if !unit.is_unit() {
        return None;
    }
    let count = count.parse::<usize>().ok()?;
    let blocker = parse_dev_scenario_blocker(id, blocker)?;
    all_dev_scenarios()
        .iter()
        .flat_map(|scenario| scenario.launches.iter())
        .copied()
        .find(|launch| {
            launch.id == id
                && launch.unit == unit
                && launch.count == count
                && launch.blocker == blocker
        })
}

pub fn parse_dev_scenario_room(raw: &str) -> Option<DevScenarioLaunch> {
    let (id, rest) = raw.split_once(":unit=")?;
    let (unit, count) = rest.split_once(":count=")?;
    let (count, blocker) = match count.split_once(":blocker=") {
        Some((count, blocker)) => (count, Some(blocker)),
        None => (count, None),
    };
    parse_dev_scenario_launch(id, unit, count, blocker)
}

pub fn parse_dev_scenario_blocker(id: &str, blocker: Option<&str>) -> Option<Option<EntityKind>> {
    match (id, blocker) {
        ("ekaterina_opening", None) => Some(None),
        ("vehicle_small_block_baseline", None) => Some(Some(EntityKind::Worker)),
        (_, None) => Some(None),
        ("vehicle_small_block_baseline", Some("none")) => Some(None),
        ("vehicle_small_block_baseline", Some(raw)) => {
            let kind = raw.parse::<EntityKind>().ok()?;
            matches!(
                kind,
                EntityKind::Worker
                    | EntityKind::Rifleman
                    | EntityKind::MachineGunner
                    | EntityKind::AntiTankGun
            )
            .then_some(Some(kind))
        }
        (_, Some("none")) => Some(None),
        (_, Some(_)) => None,
    }
}

pub fn dev_scenario_unit_label(unit: EntityKind) -> &'static str {
    match unit {
        EntityKind::Worker => "worker",
        EntityKind::EkaterinaEngineer => "engineer",
        EntityKind::Rifleman => "rifleman",
        EntityKind::MachineGunner => "machine gunner",
        EntityKind::AntiTankGun => "anti-tank gun",
        EntityKind::ScoutCar => "scout car",
        EntityKind::Tank => "tank",
        _ => "unit",
    }
}

pub fn dev_scenario_blocker_label(blocker: Option<EntityKind>) -> &'static str {
    match blocker {
        None => "none",
        Some(EntityKind::Worker) => "worker",
        Some(EntityKind::Rifleman) => "rifleman",
        Some(EntityKind::MachineGunner) => "machine gunner",
        Some(EntityKind::AntiTankGun) => "anti-tank gun",
        Some(_) => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_launches() {
        assert_eq!(
            parse_dev_scenario_room("ekaterina_opening:unit=ekaterina_engineer:count=1"),
            Some(DevScenarioLaunch {
                id: "ekaterina_opening",
                unit: EntityKind::EkaterinaEngineer,
                count: 1,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "worker", "1", None),
            Some(DevScenarioLaunch {
                id: "scout_car_snaking_corridor",
                unit: EntityKind::Worker,
                count: 1,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_snaking_corridor:unit=tank:count=4"),
            Some(DevScenarioLaunch {
                id: "scout_car_snaking_corridor",
                unit: EntityKind::Tank,
                count: 4,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("direct_reverse_order:unit=anti_tank_gun:count=1"),
            Some(DevScenarioLaunch {
                id: "direct_reverse_order",
                unit: EntityKind::AntiTankGun,
                count: 1,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=anti_tank_gun:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::AntiTankGun,
                count: 15,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=scout_car:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::ScoutCar,
                count: 15,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=tank:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::Tank,
                count: 15,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_corner_wall:unit=anti_tank_gun:count=5"),
            Some(DevScenarioLaunch {
                id: "vehicle_corner_wall",
                unit: EntityKind::AntiTankGun,
                count: 5,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_small_block_baseline:unit=scout_car:count=5"),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::ScoutCar,
                count: 5,
                blocker: Some(EntityKind::Worker),
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_small_block_baseline:unit=tank:count=5"),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::Tank,
                count: 5,
                blocker: Some(EntityKind::Worker),
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_small_block_baseline:unit=tank:count=5:blocker=none"),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::Tank,
                count: 5,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("factory_zero_gap_perpendicular:unit=tank:count=1"),
            Some(DevScenarioLaunch {
                id: "factory_zero_gap_perpendicular",
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_launch(
                "vehicle_small_block_baseline",
                "scout_car",
                "3",
                Some("machine_gunner")
            ),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::ScoutCar,
                count: 3,
                blocker: Some(EntityKind::MachineGunner),
            })
        );
    }

    #[test]
    fn rejects_unknown_launches() {
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "tank", "2", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "city_centre", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("direct_reverse_order", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("direct_reverse_order", "tank", "4", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_wall_chokepoint", "worker", "3", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_wall_chokepoint", "scout_car", "4", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_corner_wall", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_corner_wall", "tank", "4", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_small_block_baseline", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_small_block_baseline", "tank", "4", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_small_block_baseline", "tank", "5", Some("tank")),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("factory_zero_gap_perpendicular", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("factory_zero_gap_perpendicular", "tank", "3", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("unknown", "worker", "1", None),
            None
        );
        assert_eq!(parse_dev_scenario_room("scout_car_snaking_corridor"), None);
    }
}
