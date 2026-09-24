use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "replay_384_tank_forest_lock",
    unit: EntityKind::Tank,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const REPLAY_384_TANK_FOREST_LOCK_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_384_tank_forest_lock",
    title: "Replay 384 Tank Forest Lock",
    description: "Alex’s full-health, two-kill Tank 245 from replay 384 on Wald des Todes. One tank and the local forest reproduce its approach and 26 failed escape orders with recorded per-tank destinations; no movement fix is applied.",
    launches: &LAUNCHES,
};

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_launch, parse_dev_scenario_room};
    use super::*;

    #[test]
    fn replay_384_launch_parses() {
        let launch = parse_dev_scenario_launch("replay_384_tank_forest_lock", "tank", "1", None)
            .expect("supported replay-384 launch");
        assert_eq!(launch, LAUNCHES[0]);
        assert_eq!(
            parse_dev_scenario_room("replay_384_tank_forest_lock:unit=tank:count=1"),
            Some(launch)
        );
    }
}
