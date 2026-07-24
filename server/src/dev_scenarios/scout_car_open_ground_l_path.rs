use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "scout_car_open_ground_l_path",
    unit: EntityKind::ScoutCar,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const SCOUT_CAR_OPEN_GROUND_L_PATH_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "scout_car_open_ground_l_path",
    title: "Scout Car Open-Ground L Path",
    description: "After five seconds, one east-facing Scout Car receives a far southwest move across empty grass. The current bug makes it reverse west along the grid route before turning southwest instead of beginning one continuous forward turn toward the goal.",
    launches: &LAUNCHES,
};

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_room, DevScenarioLaunch};
    use super::*;

    #[test]
    fn parses_launch() {
        assert_eq!(
            parse_dev_scenario_room("scout_car_open_ground_l_path:unit=scout_car:count=1"),
            Some(DevScenarioLaunch {
                id: "scout_car_open_ground_l_path",
                unit: EntityKind::ScoutCar,
                count: 1,
                blocker: None,
                case: None,
            })
        );
    }
}
