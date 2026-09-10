use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

pub(super) const CASE_ENTRENCHED_RIFLEMEN: &str = "entrenched_riflemen";

const LAUNCHES: [DevScenarioLaunch; 2] = [
    DevScenarioLaunch {
        id: "warrior_portal_duel",
        unit: EntityKind::Warrior,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "warrior_portal_duel",
        unit: EntityKind::Warrior,
        count: 1,
        blocker: None,
        case: Some(CASE_ENTRENCHED_RIFLEMEN),
    },
];

pub(super) const SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "warrior_portal_duel",
    title: "Warrior Combat Scenarios",
    description: "Inspect either a full Portal production duel or a Warrior attack-move that begins beyond the enhanced range of two already-entrenched Riflemen.",
    launches: &LAUNCHES,
};

pub(super) fn parse_case(case: Option<&str>) -> Option<Option<&'static str>> {
    match case {
        None => Some(None),
        Some(CASE_ENTRENCHED_RIFLEMEN) => Some(Some(CASE_ENTRENCHED_RIFLEMEN)),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{parse_dev_scenario_launch_with_case, parse_dev_scenario_room};
    use super::*;

    #[test]
    fn warrior_combat_cases_parse_only_supported_launches() {
        let default =
            parse_dev_scenario_launch_with_case("warrior_portal_duel", "warrior", "1", None, None)
                .expect("default Warrior Portal duel");
        assert_eq!(default.case, None);

        let entrenched = parse_dev_scenario_room(
            "warrior_portal_duel:unit=warrior:count=1:case=entrenched_riflemen",
        )
        .expect("entrenched Riflemen case");
        assert_eq!(entrenched.case, Some(CASE_ENTRENCHED_RIFLEMEN));

        assert_eq!(
            parse_dev_scenario_launch_with_case(
                "warrior_portal_duel",
                "warrior",
                "1",
                None,
                Some("unsupported"),
            ),
            None
        );
    }
}
