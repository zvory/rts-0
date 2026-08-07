use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

pub const CASE_LIVING_COMMAND_CAR: &str = "living_command_car";
pub const CASE_ENEMY_SCREEN: &str = "enemy_screen";
pub const CASE_COMMAND_CAR_DEATH: &str = "command_car_death";

const LAUNCHES: [DevScenarioLaunch; 3] = [
    DevScenarioLaunch {
        id: "replay_281_tank_gap",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
        case: Some(CASE_LIVING_COMMAND_CAR),
    },
    DevScenarioLaunch {
        id: "replay_281_tank_gap",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
        case: Some(CASE_ENEMY_SCREEN),
    },
    DevScenarioLaunch {
        id: "replay_281_tank_gap",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
        case: Some(CASE_COMMAND_CAR_DEATH),
    },
];

pub(super) const REPLAY_281_TANK_GAP_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_281_tank_gap",
    title: "Replay 281 Tank Gap",
    description: "After ten seconds, Soupman's five Tanks, Scout Car, and command-capacity Command Car use the tick-13,537 attack-move formation on the exact Schone Tage seven-tile northern gap. Cases isolate friendly traffic, the deployed Machine Gunner screen, and the full low-health Command Car death sequence.",
    launches: &LAUNCHES,
};

pub(super) fn parse_case(case: Option<&str>) -> Option<Option<&'static str>> {
    match case {
        Some(CASE_LIVING_COMMAND_CAR) => Some(Some(CASE_LIVING_COMMAND_CAR)),
        Some(CASE_ENEMY_SCREEN) => Some(Some(CASE_ENEMY_SCREEN)),
        Some(CASE_COMMAND_CAR_DEATH) => Some(Some(CASE_COMMAND_CAR_DEATH)),
        _ => None,
    }
}

pub(super) fn case_label(case: &str) -> Option<&'static str> {
    match case {
        CASE_LIVING_COMMAND_CAR => Some("living command car"),
        CASE_ENEMY_SCREEN => Some("enemy screen"),
        CASE_COMMAND_CAR_DEATH => Some("command car death"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_launch_with_case, parse_dev_scenario_room};
    use super::*;

    #[test]
    fn replay_281_case_matrix_parses_only_supported_cases() {
        for case in [
            CASE_LIVING_COMMAND_CAR,
            CASE_ENEMY_SCREEN,
            CASE_COMMAND_CAR_DEATH,
        ] {
            let launch = parse_dev_scenario_launch_with_case(
                "replay_281_tank_gap",
                "tank",
                "5",
                None,
                Some(case),
            )
            .expect("supported replay-281 case");
            assert_eq!(launch.case, Some(case));
            assert_eq!(
                parse_dev_scenario_room(&format!(
                    "replay_281_tank_gap:unit=tank:count=5:case={case}"
                )),
                Some(launch)
            );
        }
        assert_eq!(
            parse_dev_scenario_launch_with_case(
                "replay_281_tank_gap",
                "tank",
                "5",
                None,
                Some("tanks_only"),
            ),
            None
        );
    }
}
