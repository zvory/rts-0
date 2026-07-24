use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "scout_car_lake_reverse_l_path",
    unit: EntityKind::ScoutCar,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const SCOUT_CAR_LAKE_REVERSE_L_PATH_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "scout_car_lake_reverse_l_path",
    title: "Scout Car Lake Reverse L Path",
    description: "After five seconds, one east-facing Scout Car starts 20 tiles northeast of a centered 15-by-15 lake and receives a move order 20 tiles southwest of it. This guards the established route around the lake against unsafe waypoint collapse and permanent corner stalls.",
    launches: &LAUNCHES,
};

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_room, DevScenarioLaunch};
    use super::*;

    #[test]
    fn parses_launch() {
        assert_eq!(
            parse_dev_scenario_room("scout_car_lake_reverse_l_path:unit=scout_car:count=1"),
            Some(DevScenarioLaunch {
                id: "scout_car_lake_reverse_l_path",
                unit: EntityKind::ScoutCar,
                count: 1,
                blocker: None,
                case: None,
            })
        );
    }
}
