use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "replay_142_vehicle_lock",
    unit: EntityKind::ScoutCar,
    count: 2,
    blocker: None,
    case: None,
}];

pub(super) const REPLAY_142_VEHICLE_LOCK_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "replay_142_vehicle_lock",
    title: "Replay 112 Vehicle Lock",
    description: "Soupman's two touching Scout/Command Cars, three formation companions, and second-base landmark from match 142. After one second the translated tick-14,176 group order recreates their slow overlapping translation.",
    launches: &LAUNCHES,
};
