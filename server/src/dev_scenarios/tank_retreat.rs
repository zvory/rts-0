use super::{DevScenarioLaunch, DevScenarioSpec};
use rts_sim::game::entity::EntityKind;

const UNDER_FIRE_RETREAT_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "tank_under_fire_retreat",
    unit: EntityKind::Tank,
    count: 1,
    blocker: None,
    case: None,
}];

const REVERSE_TRAFFIC_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "tank_reverse_traffic",
    unit: EntityKind::Tank,
    count: 3,
    blocker: None,
    case: None,
}];

pub(super) const TANK_UNDER_FIRE_RETREAT_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "tank_under_fire_retreat",
    title: "Tank Under-Fire Retreat",
    description: "One reinforced Tank faces a deployed Anti-Tank Gun, takes frontal AP fire, then after 20 seconds receives a long move order directly behind it. The current baseline pivots 180 degrees before retreating.",
    launches: &UNDER_FIRE_RETREAT_LAUNCHES,
};

pub(super) const TANK_REVERSE_TRAFFIC_SPEC: DevScenarioSpec = DevScenarioSpec {
    id: "tank_reverse_traffic",
    title: "Tank Reverse Traffic",
    description: "Three reinforced outward-facing Tanks take frontal fire from three deployed Anti-Tank Guns, then after 20 seconds receive simultaneous individual moves across a shared center. Future under-fire reverse paths converge and require rearward traffic control.",
    launches: &REVERSE_TRAFFIC_LAUNCHES,
};
