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
    description: "Organic reproduction of Soupman's replay-220 worker rally lock: a Resource Depot produces four workers at the real production cadence and rallies them across the map. Each worker naturally enters the same route beside a deployed Machine Gunner.",
    launches: &LAUNCHES,
};
