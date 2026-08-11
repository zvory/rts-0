use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

pub const CASE_TICK_PERFECT: &str = "tick_perfect";
pub const CASE_MINIMAL_ATTACK_MOVE: &str = "minimal_attack_move";
pub const CASE_MINIMAL_MOVE: &str = "minimal_move";
pub const CASE_MINIMAL_NO_ENEMY: &str = "minimal_no_enemy";

const LAUNCHES: [DevScenarioLaunch; 4] = [
    launch(CASE_TICK_PERFECT),
    launch(CASE_MINIMAL_ATTACK_MOVE),
    launch(CASE_MINIMAL_NO_ENEMY),
    launch(CASE_MINIMAL_MOVE),
];

const fn launch(case: &'static str) -> DevScenarioLaunch {
    DevScenarioLaunch {
        id: "replay_281_tank_gap",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
        case: Some(case),
    }
}

pub(super) const REPLAY_281_TANK_GAP_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_281_tank_gap",
    title: "Replay 281 Tank Attack-Move Jam",
    description: "Starts at authoritative replay tick 13,536 and issues Soupman's recorded attack-move on tick 13,537. The tick-perfect case preserves every local actor and hidden state; reduced cases show that one deployed Machine Gunner makes the lead Tank stop, which queues the rear Tank behind it. Move and no-enemy controls clear the gap.",
    launches: &LAUNCHES,
};

pub(super) fn parse_case(case: Option<&str>) -> Option<Option<&'static str>> {
    match case {
        Some(CASE_TICK_PERFECT) => Some(Some(CASE_TICK_PERFECT)),
        Some(CASE_MINIMAL_ATTACK_MOVE) => Some(Some(CASE_MINIMAL_ATTACK_MOVE)),
        Some(CASE_MINIMAL_MOVE) => Some(Some(CASE_MINIMAL_MOVE)),
        Some(CASE_MINIMAL_NO_ENEMY) => Some(Some(CASE_MINIMAL_NO_ENEMY)),
        _ => None,
    }
}

pub(super) fn case_label(case: &str) -> Option<&'static str> {
    match case {
        CASE_TICK_PERFECT => Some("tick-perfect replay"),
        CASE_MINIMAL_ATTACK_MOVE => Some("minimal attack-move jam"),
        CASE_MINIMAL_MOVE => Some("minimal Move control"),
        CASE_MINIMAL_NO_ENEMY => Some("minimal no-enemy control"),
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
            CASE_TICK_PERFECT,
            CASE_MINIMAL_ATTACK_MOVE,
            CASE_MINIMAL_NO_ENEMY,
            CASE_MINIMAL_MOVE,
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
                Some("command_car_death"),
            ),
            None
        );
    }
}
