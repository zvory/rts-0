use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "replay_517_mortar_water_exit",
    unit: EntityKind::MortarTeam,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const REPLAY_517_MORTAR_WATER_EXIT_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_517_mortar_water_exit",
    title: "Replay 517 Mortar Water Exit",
    description: "Alex’s first Mortar Team from Soupman vs Alex on Classic. The original Gun Works completes production beside the shoreline, then six recorded moves run. The verified source replay exits successfully: this comparison scene does not reproduce a lock or apply a gameplay fix.",
    launches: &LAUNCHES,
};

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_launch, parse_dev_scenario_room};
    use super::*;

    #[test]
    fn replay_517_launch_parses() {
        let launch =
            parse_dev_scenario_launch("replay_517_mortar_water_exit", "mortar_team", "1", None)
                .unwrap();
        assert_eq!(launch, LAUNCHES[0]);
        assert_eq!(
            parse_dev_scenario_room("replay_517_mortar_water_exit:unit=mortar_team:count=1"),
            Some(launch)
        );
    }
}
