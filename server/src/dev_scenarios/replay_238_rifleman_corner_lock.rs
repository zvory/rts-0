use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "replay_238_rifleman_corner_lock",
    unit: EntityKind::Rifleman,
    count: 1,
    blocker: None,
    case: None,
}];

pub(super) const REPLAY_238_RIFLEMAN_CORNER_LOCK_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_238_rifleman_corner_lock",
    title: "Replay 238 Rifleman Corner Lock",
    description: "Alex's Rifleman 165 at Schone Tage tick 2,915, fixed at (2755.293, 311.360) while trying to round the northeast rock corner toward (2640, 336).",
    launches: &LAUNCHES,
};
