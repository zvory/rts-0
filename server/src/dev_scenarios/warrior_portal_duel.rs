use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "warrior_portal_duel",
    unit: EntityKind::Warrior,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "warrior_portal_duel",
    title: "Warrior Portal Duel",
    description: "A Cultivator Portal completes a full Warrior production cycle, rallies the new swordsman into open ground, and lets ordinary combat resolve against one held Rifleman.",
    launches: &LAUNCHES,
};
