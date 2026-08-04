use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "move_reload_acquisition",
    unit: EntityKind::Tank,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const MOVE_RELOAD_ACQUISITION_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "move_reload_acquisition",
    title: "Move Reload Acquisition",
    description: "After a ten-second inspection pause, a reloading Tank receives a plain move past an invulnerable Anti-Tank Gun. When the gun enters cannon range, the Tank should acquire it and turn its turret during reload without stopping its commanded movement.",
    launches: &LAUNCHES,
};

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_launch, parse_dev_scenario_room, DevScenarioLaunch};
    use super::*;

    #[test]
    fn parses_launch() {
        assert_eq!(
            parse_dev_scenario_room("move_reload_acquisition:unit=tank:count=1"),
            Some(DevScenarioLaunch {
                id: "move_reload_acquisition",
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_launch("move_reload_acquisition", "rifleman", "1", None),
            None
        );
    }
}
