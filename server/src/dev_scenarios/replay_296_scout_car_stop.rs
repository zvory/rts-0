use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "replay_296_scout_car_stop",
    unit: EntityKind::ScoutCar,
    count: 5,
    blocker: None,
    case: None,
}];

pub(super) const REPLAY_296_SCOUT_CAR_STOP_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_296_scout_car_stop",
    title: "Replay 296 Scout Car Edge Stop",
    description: "Recreates Alex's five-Scout-Car move at replay tick 8,242. The west-edge formation assigns two cars destinations in tile column zero; both retain Move orders but receive no path, while the other three continue. Alex starts with the replay's 39 oil to demonstrate that fuel is unrelated.",
    launches: &LAUNCHES,
};

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_launch, parse_dev_scenario_room};
    use super::*;

    #[test]
    fn replay_296_launch_parses() {
        let launch = parse_dev_scenario_launch("replay_296_scout_car_stop", "scout_car", "5", None)
            .expect("supported replay-296 launch");
        assert_eq!(launch, LAUNCHES[0]);
        assert_eq!(
            parse_dev_scenario_room("replay_296_scout_car_stop:unit=scout_car:count=5"),
            Some(launch)
        );
    }
}
