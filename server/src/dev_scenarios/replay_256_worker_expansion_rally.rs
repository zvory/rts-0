use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "replay_256_worker_expansion_rally",
    unit: EntityKind::Worker,
    count: 4,
    blocker: None,
    case: None,
}];

pub(super) const REPLAY_256_WORKER_EXPANSION_RALLY_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_256_worker_expansion_rally",
    title: "Replay 256 Worker Rally Oscillation",
    description: "Minimal reproduction of Soupman's replay-220 worker rally lock: four workers have distinct far-side rally goals and one stale waypoint beside a stationary unit. A corrected build drops that unreachable intermediate waypoint and continues toward each goal.",
    launches: &LAUNCHES,
};
