use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "replay_303_scout_car_forest_lock",
    unit: EntityKind::ScoutCar,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const REPLAY_303_SCOUT_CAR_FOREST_LOCK_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_303_scout_car_forest_lock",
    title: "Replay 303 Scout Car Forest Lock",
    description: "Reduces Soupman's Schone Tage replay to the seven ordered vehicles and the exact 21-tile forest footprint. Six recorded group moves run from ticks 15,283 through 15,308; Scout Car 403 locks against the forest at (2148.72, 3983.10) while the rest of the group keeps moving.",
    launches: &LAUNCHES,
};

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_launch, parse_dev_scenario_room};
    use super::*;

    #[test]
    fn replay_303_launch_parses() {
        let launch =
            parse_dev_scenario_launch("replay_303_scout_car_forest_lock", "scout_car", "1", None)
                .expect("supported replay-303 launch");
        assert_eq!(launch, LAUNCHES[0]);
        assert_eq!(
            parse_dev_scenario_room("replay_303_scout_car_forest_lock:unit=scout_car:count=1"),
            Some(launch)
        );
    }
}
