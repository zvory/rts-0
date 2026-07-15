use super::*;

const NORTHWEST_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "command_car_building_corner",
    unit: EntityKind::CommandCar,
    count: 1,
    blocker: None,
    case: None,
}];

const SOUTH_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "command_car_building_corner_south",
    unit: EntityKind::CommandCar,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const COMMAND_CAR_BUILDING_CORNER_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "command_car_building_corner",
    title: "Command Car Building Corner",
    description: "One Command Car starts inside the reduced Vehicle Works, Training Centre, and Barracks corner from the Soupman match, waits one second, then moves northwest.",
    launches: &NORTHWEST_LAUNCHES,
};

pub(super) const COMMAND_CAR_BUILDING_CORNER_SOUTH_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "command_car_building_corner_south",
    title: "Command Car Building Corner — South",
    description: "The same trapped Command Car and three-building layout, but the order target is exactly ten tiles south of its starting position.",
    launches: &SOUTH_LAUNCHES,
};
