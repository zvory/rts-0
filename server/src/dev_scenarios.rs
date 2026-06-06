#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DevScenarioLaunch {
    pub(crate) id: &'static str,
    pub(crate) cars: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DevScenarioSpec {
    pub(crate) id: &'static str,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) launches: &'static [DevScenarioLaunch],
}

const SCOUT_CAR_SNAKING_CORRIDOR_LAUNCHES: [DevScenarioLaunch; 2] = [
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        cars: 1,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        cars: 4,
    },
];

const DEV_SCENARIOS: [DevScenarioSpec; 1] = [DevScenarioSpec {
    id: "scout_car_snaking_corridor",
    title: "Scout Car Snaking Corridor",
    description: "Movement/pathing debug run through a narrow authored corridor.",
    launches: &SCOUT_CAR_SNAKING_CORRIDOR_LAUNCHES,
}];

pub(crate) fn all_dev_scenarios() -> &'static [DevScenarioSpec] {
    &DEV_SCENARIOS
}

pub(crate) fn parse_dev_scenario_launch(id: &str, cars: &str) -> Option<DevScenarioLaunch> {
    let cars = cars.parse::<usize>().ok()?;
    all_dev_scenarios()
        .iter()
        .flat_map(|scenario| scenario.launches.iter())
        .copied()
        .find(|launch| launch.id == id && launch.cars == cars)
}

pub(crate) fn parse_dev_scenario_room(raw: &str) -> Option<DevScenarioLaunch> {
    let (id, cars) = raw.split_once(":cars=")?;
    parse_dev_scenario_launch(id, cars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_launches() {
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "1"),
            Some(DevScenarioLaunch {
                id: "scout_car_snaking_corridor",
                cars: 1,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_snaking_corridor:cars=4"),
            Some(DevScenarioLaunch {
                id: "scout_car_snaking_corridor",
                cars: 4,
            })
        );
    }

    #[test]
    fn rejects_unknown_launches() {
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "2"),
            None
        );
        assert_eq!(parse_dev_scenario_launch("unknown", "1"), None);
        assert_eq!(parse_dev_scenario_room("scout_car_snaking_corridor"), None);
    }
}
