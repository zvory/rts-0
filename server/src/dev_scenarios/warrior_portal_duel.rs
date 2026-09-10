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

pub(super) fn case_label(case: &str) -> Option<&'static str> {
    (case == CASE_ENTRENCHED_RIFLEMEN).then_some("entrenched Riflemen")
}
