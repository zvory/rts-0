use rts_sim::game::entity::EntityKind;

mod command_car_corner;
mod scout_car_open_ground_l_path;
mod tank_retreat;
use command_car_corner::{
    COMMAND_CAR_BUILDING_CORNER_SPEC, COMMAND_CAR_BUILDING_CORNER_WEST_SOUTHWEST_SPEC,
};
use scout_car_open_ground_l_path::SCOUT_CAR_OPEN_GROUND_L_PATH_SPEC;
use tank_retreat::{TANK_REVERSE_TRAFFIC_SPEC, TANK_UNDER_FIRE_RETREAT_SPEC};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevScenarioLaunch {
    pub id: &'static str,
    pub unit: EntityKind,
    pub count: usize,
    pub blocker: Option<EntityKind>,
    pub case: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevScenarioSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub launches: &'static [DevScenarioLaunch],
}

pub const DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_HEAD_ON: &str = "head_on";
pub const DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_SLIGHT_ANGLE: &str = "slight_angle";
pub const DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_MAJOR_ANGLE: &str = "major_angle";

const DYNAMIC_CONSTRUCTION_PATH_BLOCK_LAUNCHES: [DevScenarioLaunch; 3] = [
    DevScenarioLaunch {
        id: "dynamic_construction_path_block",
        unit: EntityKind::Worker,
        count: 1,
        blocker: None,
        case: Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_HEAD_ON),
    },
    DevScenarioLaunch {
        id: "dynamic_construction_path_block",
        unit: EntityKind::Worker,
        count: 1,
        blocker: None,
        case: Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_SLIGHT_ANGLE),
    },
    DevScenarioLaunch {
        id: "dynamic_construction_path_block",
        unit: EntityKind::Worker,
        count: 1,
        blocker: None,
        case: Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_MAJOR_ANGLE),
    },
];

const SCOUT_CAR_SNAKING_CORRIDOR_LAUNCHES: [DevScenarioLaunch; 12] = [
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Worker,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Worker,
        count: 4,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Rifleman,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Rifleman,
        count: 4,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::MachineGunner,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::MachineGunner,
        count: 4,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::AntiTankGun,
        count: 4,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::ScoutCar,
        count: 4,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_snaking_corridor",
        unit: EntityKind::Tank,
        count: 4,
        blocker: None,
        case: None,
    },
];

const DIRECT_REVERSE_ORDER_LAUNCHES: [DevScenarioLaunch; 3] = [
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "direct_reverse_order",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
];

const REPLAY_142_VEHICLE_LOCK_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "replay_142_vehicle_lock",
    unit: EntityKind::ScoutCar,
    count: 2,
    blocker: None,
    case: None,
}];

const WALL_CHOKEPOINT_VEHICLE_LAUNCHES: [DevScenarioLaunch; 15] = [
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 3,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 5,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 6,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 10,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::AntiTankGun,
        count: 15,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 6,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 10,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::ScoutCar,
        count: 15,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 3,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 6,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 10,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "scout_car_wall_chokepoint",
        unit: EntityKind::Tank,
        count: 15,
        blocker: None,
        case: None,
    },
];

const VEHICLE_CORNER_WALL_LAUNCHES: [DevScenarioLaunch; 9] = [
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::AntiTankGun,
        count: 3,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::AntiTankGun,
        count: 5,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::Tank,
        count: 3,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_corner_wall",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
        case: None,
    },
];

const VEHICLE_SMALL_BLOCK_BASELINE_LAUNCHES: [DevScenarioLaunch; 30] = [
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: Some(EntityKind::Worker),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: Some(EntityKind::Worker),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: Some(EntityKind::Worker),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: Some(EntityKind::Rifleman),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: Some(EntityKind::Rifleman),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: Some(EntityKind::Rifleman),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: Some(EntityKind::MachineGunner),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: Some(EntityKind::MachineGunner),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: Some(EntityKind::MachineGunner),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: Some(EntityKind::AntiTankGun),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 3,
        blocker: Some(EntityKind::AntiTankGun),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::ScoutCar,
        count: 5,
        blocker: Some(EntityKind::AntiTankGun),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: Some(EntityKind::Worker),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: Some(EntityKind::Worker),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: Some(EntityKind::Worker),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: Some(EntityKind::Rifleman),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: Some(EntityKind::Rifleman),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: Some(EntityKind::Rifleman),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: Some(EntityKind::MachineGunner),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: Some(EntityKind::MachineGunner),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: Some(EntityKind::MachineGunner),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 1,
        blocker: Some(EntityKind::AntiTankGun),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 3,
        blocker: Some(EntityKind::AntiTankGun),
        case: None,
    },
    DevScenarioLaunch {
        id: "vehicle_small_block_baseline",
        unit: EntityKind::Tank,
        count: 5,
        blocker: Some(EntityKind::AntiTankGun),
        case: None,
    },
];

const FACTORY_ZERO_GAP_PERPENDICULAR_LAUNCHES: [DevScenarioLaunch; 3] = [
    DevScenarioLaunch {
        id: "factory_zero_gap_perpendicular",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "factory_zero_gap_perpendicular",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "factory_zero_gap_perpendicular",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
];

const FACTORY_WALL_RALLY_SPAWN_LAUNCHES: [DevScenarioLaunch; 3] = [
    DevScenarioLaunch {
        id: "factory_wall_rally_spawn",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "factory_wall_rally_spawn",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "factory_wall_rally_spawn",
        unit: EntityKind::CommandCar,
        count: 1,
        blocker: None,
        case: None,
    },
];

const TANK_TRAP_LINE_HORIZONTAL_LAUNCHES: [DevScenarioLaunch; 2] = [
    DevScenarioLaunch {
        id: "tank_trap_line_horizontal",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "tank_trap_line_horizontal",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
];

const TANK_TRAP_LINE_VERTICAL_LAUNCHES: [DevScenarioLaunch; 2] = [
    DevScenarioLaunch {
        id: "tank_trap_line_vertical",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "tank_trap_line_vertical",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
];

const TANK_TRAP_LINE_DIAGONAL_LAUNCHES: [DevScenarioLaunch; 2] = [
    DevScenarioLaunch {
        id: "tank_trap_line_diagonal",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: None,
    },
    DevScenarioLaunch {
        id: "tank_trap_line_diagonal",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: None,
    },
];

pub const TANK_TRAP_PATHING_CASE_FRIENDLY_VEHICLE_REROUTE: &str = "friendly_vehicle_reroute";
pub const TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE: &str = "enemy_vehicle_reroute";
pub const TANK_TRAP_PATHING_CASE_INFANTRY_PASS_THROUGH: &str = "infantry_pass_through";
pub const TANK_TRAP_PATHING_CASE_EXPLICIT_INFANTRY_ATTACK: &str = "explicit_infantry_attack";

const TANK_TRAP_PATHING_MATRIX_LAUNCHES: [DevScenarioLaunch; 11] = [
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_FRIENDLY_VEHICLE_REROUTE),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_FRIENDLY_VEHICLE_REROUTE),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::AntiTankGun,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::MortarTeam,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::Artillery,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::ScoutCar,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::Tank,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::Worker,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_INFANTRY_PASS_THROUGH),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::Rifleman,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_INFANTRY_PASS_THROUGH),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::MachineGunner,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_INFANTRY_PASS_THROUGH),
    },
    DevScenarioLaunch {
        id: "tank_trap_pathing_matrix",
        unit: EntityKind::Rifleman,
        count: 1,
        blocker: None,
        case: Some(TANK_TRAP_PATHING_CASE_EXPLICIT_INFANTRY_ATTACK),
    },
];

const ENTRENCHMENT_INSPECTION_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "entrenchment_inspection",
    unit: EntityKind::Rifleman,
    count: 1,
    blocker: None,
    case: None,
}];

const TANK_COAX_INSPECTION_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "tank_coax_inspection",
    unit: EntityKind::Tank,
    count: 1,
    blocker: None,
    case: None,
}];

const ATTACK_MOVE_RELOAD_ACQUISITION_LAUNCHES: [DevScenarioLaunch; 1] = [DevScenarioLaunch {
    id: "attack_move_reload_acquisition",
    unit: EntityKind::Tank,
    count: 1,
    blocker: None,
    case: None,
}];

const DEV_SCENARIOS: [DevScenarioSpec; 21] = [
    DevScenarioSpec {
        id: "dynamic_construction_path_block",
        title: "Dynamic Construction Path Block",
        description: "Two workers receive simultaneous orders: one moves 20 tiles while the other starts a Barracks across its already-planned route. Select a head-on, slight-angle, or major-angle approach.",
        launches: &DYNAMIC_CONSTRUCTION_PATH_BLOCK_LAUNCHES,
    },
    DevScenarioSpec {
        id: "scout_car_snaking_corridor",
        title: "Scout Car Snaking Corridor",
        description: "Movement/pathing debug run through a narrow authored corridor.",
        launches: &SCOUT_CAR_SNAKING_CORRIDOR_LAUNCHES,
    },
    DevScenarioSpec {
        id: "direct_reverse_order",
        title: "Direct Reverse Order",
        description:
            "Single vehicle faces east, then receives a move order 15 tiles directly behind it.",
        launches: &DIRECT_REVERSE_ORDER_LAUNCHES,
    },
    SCOUT_CAR_OPEN_GROUND_L_PATH_SPEC,
    DevScenarioSpec {
        id: "replay_142_vehicle_lock",
        title: "Replay 112 Vehicle Lock",
        description: "Soupman's two touching Scout/Command Cars, three formation companions, and second-base landmark from match 142. After one second the translated tick-14,176 group order recreates their slow overlapping translation.",
        launches: &REPLAY_142_VEHICLE_LOCK_LAUNCHES,
    },
    DevScenarioSpec {
        id: "scout_car_wall_chokepoint",
        title: "Vehicle Wall Chokepoint",
        description: "Vehicles start beside each other below a stone wall gap and move north through the choke.",
        launches: &WALL_CHOKEPOINT_VEHICLE_LAUNCHES,
    },
    DevScenarioSpec {
        id: "vehicle_corner_wall",
        title: "Vehicle Corner Wall",
        description: "One, three, or five vehicles start in a flipped-L shape half a tile west of a three-tile-wide stone wall spur, then move to the east side to test cornering.",
        launches: &VEHICLE_CORNER_WALL_LAUNCHES,
    },
    DevScenarioSpec {
        id: "vehicle_small_block_baseline",
        title: "Vehicle Small-Unit Block Baseline",
        description: "Vehicles start almost bumper-to-bumper with optional small-unit blockers one tile north of each vehicle, then all vehicles move 20 tiles north.",
        launches: &VEHICLE_SMALL_BLOCK_BASELINE_LAUNCHES,
    },
    DevScenarioSpec {
        id: "factory_zero_gap_perpendicular",
        title: "Factory Zero-Gap Perpendicular",
        description: "One vehicle starts flush against the east side of a factory, waits half a second, then moves ten tiles directly east.",
        launches: &FACTORY_ZERO_GAP_PERPENDICULAR_LAUNCHES,
    },
    COMMAND_CAR_BUILDING_CORNER_SPEC,
    COMMAND_CAR_BUILDING_CORNER_WEST_SOUTHWEST_SPEC,
    DevScenarioSpec {
        id: "factory_wall_rally_spawn",
        title: "Factory Wall Rally Spawn",
        description: "A completed vehicle exits a factory below a two-tile terrain wall and rallies almost due west, reproducing replay 104 tick 7923 geometry.",
        launches: &FACTORY_WALL_RALLY_SPAWN_LAUNCHES,
    },
    DevScenarioSpec {
        id: "tank_trap_line_horizontal",
        title: "Tank Trap Horizontal Line",
        description: "Training Centre, engineers, one rifleman, and one vehicle start ready for a horizontal Tank Trap line build; after 30 seconds the test units try to cross.",
        launches: &TANK_TRAP_LINE_HORIZONTAL_LAUNCHES,
    },
    DevScenarioSpec {
        id: "tank_trap_line_vertical",
        title: "Tank Trap Vertical Line",
        description: "Training Centre, engineers, one rifleman, and one vehicle start ready for a vertical Tank Trap line build; after 30 seconds the test units try to cross.",
        launches: &TANK_TRAP_LINE_VERTICAL_LAUNCHES,
    },
    DevScenarioSpec {
        id: "tank_trap_line_diagonal",
        title: "Tank Trap Diagonal Line",
        description: "Training Centre, engineers, one rifleman, and one vehicle start ready for a diagonal Tank Trap line build; after 30 seconds the test units try to cross.",
        launches: &TANK_TRAP_LINE_DIAGONAL_LAUNCHES,
    },
    DevScenarioSpec {
        id: "tank_trap_pathing_matrix",
        title: "Tank Trap Pathing Matrix",
        description: "Prebuilt Tank Trap walls with selectable owner/pathing and attack cases.",
        launches: &TANK_TRAP_PATHING_MATRIX_LAUNCHES,
    },
    DevScenarioSpec {
        id: "entrenchment_inspection",
        title: "Entrenchment Inspection",
        description: "Seeded neutral trenches, eligible infantry, and researched dig-in units for checking trench rendering, reuse, and crowded slotting.",
        launches: &ENTRENCHMENT_INSPECTION_LAUNCHES,
    },
    DevScenarioSpec {
        id: "tank_coax_inspection",
        title: "Tank Coax Inspection",
        description: "One held Tank faces infantry-priority targets, support weapons, Ekat/Golem units, armored fallback targets, blockers, resources, smoke, and buildings around the coax arc so the secondary machine gun can be inspected without immediate cannon fire.",
        launches: &TANK_COAX_INSPECTION_LAUNCHES,
    },
    DevScenarioSpec {
        id: "attack_move_reload_acquisition",
        title: "Attack-Move Reload Acquisition",
        description: "After a ten-second inspection pause, a reloading Tank receives an attack-move through an invulnerable enemy Tank already inside its moving weapon range. The current bug lets it close to near-contact before acquiring the target; a corrected build should stop at the initial range boundary and wait for reload.",
        launches: &ATTACK_MOVE_RELOAD_ACQUISITION_LAUNCHES,
    },
    TANK_UNDER_FIRE_RETREAT_SPEC,
    TANK_REVERSE_TRAFFIC_SPEC,
];

pub fn all_dev_scenarios() -> &'static [DevScenarioSpec] {
    &DEV_SCENARIOS
}

pub fn parse_dev_scenario_launch(
    id: &str,
    unit: &str,
    count: &str,
    blocker: Option<&str>,
) -> Option<DevScenarioLaunch> {
    parse_dev_scenario_launch_with_case(id, unit, count, blocker, None)
}

pub fn parse_dev_scenario_launch_with_case(
    id: &str,
    unit: &str,
    count: &str,
    blocker: Option<&str>,
    case: Option<&str>,
) -> Option<DevScenarioLaunch> {
    let unit = unit.parse::<EntityKind>().ok()?;
    if !unit.is_unit() {
        return None;
    }
    let count = count.parse::<usize>().ok()?;
    let blocker = parse_dev_scenario_blocker(id, blocker)?;
    let case = parse_dev_scenario_case(id, case)?;
    all_dev_scenarios()
        .iter()
        .flat_map(|scenario| scenario.launches.iter())
        .copied()
        .find(|launch| {
            launch.id == id
                && launch.unit == unit
                && launch.count == count
                && launch.blocker == blocker
                && launch.case == case
        })
}

pub fn parse_dev_scenario_room(raw: &str) -> Option<DevScenarioLaunch> {
    let (id, rest) = raw.split_once(":unit=")?;
    let (unit, count) = rest.split_once(":count=")?;
    let (count, suffix) = match count.split_once(':') {
        Some((count, suffix)) => (count, Some(suffix)),
        None => (count, None),
    };
    let mut blocker = None;
    let mut case = None;
    if let Some(suffix) = suffix {
        for part in suffix.split(':') {
            if let Some(value) = part.strip_prefix("blocker=") {
                blocker = Some(value);
            } else {
                let value = part.strip_prefix("case=")?;
                case = Some(value);
            }
        }
    }
    parse_dev_scenario_launch_with_case(id, unit, count, blocker, case)
}

pub fn parse_dev_scenario_blocker(id: &str, blocker: Option<&str>) -> Option<Option<EntityKind>> {
    match (id, blocker) {
        ("vehicle_small_block_baseline", None) => Some(Some(EntityKind::Worker)),
        (_, None) => Some(None),
        ("vehicle_small_block_baseline", Some("none")) => Some(None),
        ("vehicle_small_block_baseline", Some(raw)) => {
            let kind = raw.parse::<EntityKind>().ok()?;
            matches!(
                kind,
                EntityKind::Worker
                    | EntityKind::Rifleman
                    | EntityKind::MachineGunner
                    | EntityKind::AntiTankGun
            )
            .then_some(Some(kind))
        }
        (_, Some("none")) => Some(None),
        (_, Some(_)) => None,
    }
}

pub fn parse_dev_scenario_case(id: &str, case: Option<&str>) -> Option<Option<&'static str>> {
    match (id, case) {
        ("dynamic_construction_path_block", Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_HEAD_ON)) => {
            Some(Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_HEAD_ON))
        }
        (
            "dynamic_construction_path_block",
            Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_SLIGHT_ANGLE),
        ) => Some(Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_SLIGHT_ANGLE)),
        (
            "dynamic_construction_path_block",
            Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_MAJOR_ANGLE),
        ) => Some(Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_MAJOR_ANGLE)),
        ("dynamic_construction_path_block", _) => None,
        ("tank_trap_pathing_matrix", Some(TANK_TRAP_PATHING_CASE_FRIENDLY_VEHICLE_REROUTE)) => {
            Some(Some(TANK_TRAP_PATHING_CASE_FRIENDLY_VEHICLE_REROUTE))
        }
        ("tank_trap_pathing_matrix", Some(TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE)) => {
            Some(Some(TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE))
        }
        ("tank_trap_pathing_matrix", Some(TANK_TRAP_PATHING_CASE_INFANTRY_PASS_THROUGH)) => {
            Some(Some(TANK_TRAP_PATHING_CASE_INFANTRY_PASS_THROUGH))
        }
        ("tank_trap_pathing_matrix", Some(TANK_TRAP_PATHING_CASE_EXPLICIT_INFANTRY_ATTACK)) => {
            Some(Some(TANK_TRAP_PATHING_CASE_EXPLICIT_INFANTRY_ATTACK))
        }
        ("tank_trap_pathing_matrix", _) => None,
        (_, None) => Some(None),
        (_, Some(_)) => None,
    }
}

pub fn dev_scenario_case_label(case: &str) -> &'static str {
    match case {
        DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_HEAD_ON => "head-on",
        DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_SLIGHT_ANGLE => "slight angle",
        DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_MAJOR_ANGLE => "major angle",
        TANK_TRAP_PATHING_CASE_FRIENDLY_VEHICLE_REROUTE => "friendly vehicle reroute",
        TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE => "enemy vehicle reroute",
        TANK_TRAP_PATHING_CASE_INFANTRY_PASS_THROUGH => "infantry pass-through",
        TANK_TRAP_PATHING_CASE_EXPLICIT_INFANTRY_ATTACK => "explicit infantry attack",
        _ => "case",
    }
}

pub fn dev_scenario_unit_label(unit: EntityKind) -> &'static str {
    match unit {
        EntityKind::Worker => "worker",
        EntityKind::Rifleman => "rifleman",
        EntityKind::MachineGunner => "machine gunner",
        EntityKind::AntiTankGun => "anti-tank gun",
        EntityKind::MortarTeam => "mortar team",
        EntityKind::Artillery => "artillery",
        EntityKind::ScoutCar => "scout car",
        EntityKind::Tank => "tank",
        EntityKind::CommandCar => "command car",
        _ => "unit",
    }
}

pub fn dev_scenario_blocker_label(blocker: Option<EntityKind>) -> &'static str {
    match blocker {
        None => "none",
        Some(EntityKind::Worker) => "worker",
        Some(EntityKind::Rifleman) => "rifleman",
        Some(EntityKind::MachineGunner) => "machine gunner",
        Some(EntityKind::AntiTankGun) => "anti-tank gun",
        Some(_) => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_launches() {
        assert_eq!(
            parse_dev_scenario_room(
                "dynamic_construction_path_block:unit=worker:count=1:case=slight_angle"
            ),
            Some(DevScenarioLaunch {
                id: "dynamic_construction_path_block",
                unit: EntityKind::Worker,
                count: 1,
                blocker: None,
                case: Some(DYNAMIC_CONSTRUCTION_PATH_BLOCK_CASE_SLIGHT_ANGLE),
            })
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "worker", "1", None),
            Some(DevScenarioLaunch {
                id: "scout_car_snaking_corridor",
                unit: EntityKind::Worker,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("attack_move_reload_acquisition:unit=tank:count=1"),
            Some(DevScenarioLaunch {
                id: "attack_move_reload_acquisition",
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_snaking_corridor:unit=tank:count=4"),
            Some(DevScenarioLaunch {
                id: "scout_car_snaking_corridor",
                unit: EntityKind::Tank,
                count: 4,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("direct_reverse_order:unit=anti_tank_gun:count=1"),
            Some(DevScenarioLaunch {
                id: "direct_reverse_order",
                unit: EntityKind::AntiTankGun,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=anti_tank_gun:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::AntiTankGun,
                count: 15,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=scout_car:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::ScoutCar,
                count: 15,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("scout_car_wall_chokepoint:unit=tank:count=15"),
            Some(DevScenarioLaunch {
                id: "scout_car_wall_chokepoint",
                unit: EntityKind::Tank,
                count: 15,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_corner_wall:unit=anti_tank_gun:count=5"),
            Some(DevScenarioLaunch {
                id: "vehicle_corner_wall",
                unit: EntityKind::AntiTankGun,
                count: 5,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_small_block_baseline:unit=scout_car:count=5"),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::ScoutCar,
                count: 5,
                blocker: Some(EntityKind::Worker),
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_small_block_baseline:unit=tank:count=5"),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::Tank,
                count: 5,
                blocker: Some(EntityKind::Worker),
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("vehicle_small_block_baseline:unit=tank:count=5:blocker=none"),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::Tank,
                count: 5,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("factory_zero_gap_perpendicular:unit=tank:count=1"),
            Some(DevScenarioLaunch {
                id: "factory_zero_gap_perpendicular",
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("replay_142_vehicle_lock:unit=scout_car:count=2"),
            Some(DevScenarioLaunch {
                id: "replay_142_vehicle_lock",
                unit: EntityKind::ScoutCar,
                count: 2,
                blocker: None,
                case: None,
            })
        );
        for id in [
            "command_car_building_corner",
            "command_car_building_corner_west_southwest",
        ] {
            assert_eq!(
                parse_dev_scenario_room(&format!("{id}:unit=command_car:count=1")),
                Some(DevScenarioLaunch {
                    id,
                    unit: EntityKind::CommandCar,
                    count: 1,
                    blocker: None,
                    case: None,
                })
            );
        }
        assert_eq!(
            parse_dev_scenario_room("factory_wall_rally_spawn:unit=command_car:count=1"),
            Some(DevScenarioLaunch {
                id: "factory_wall_rally_spawn",
                unit: EntityKind::CommandCar,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("tank_trap_line_horizontal:unit=scout_car:count=1"),
            Some(DevScenarioLaunch {
                id: "tank_trap_line_horizontal",
                unit: EntityKind::ScoutCar,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("tank_trap_line_vertical:unit=tank:count=1"),
            Some(DevScenarioLaunch {
                id: "tank_trap_line_vertical",
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("tank_trap_line_diagonal:unit=tank:count=1"),
            Some(DevScenarioLaunch {
                id: "tank_trap_line_diagonal",
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room(
                "tank_trap_pathing_matrix:unit=scout_car:count=1:case=friendly_vehicle_reroute"
            ),
            Some(DevScenarioLaunch {
                id: "tank_trap_pathing_matrix",
                unit: EntityKind::ScoutCar,
                count: 1,
                blocker: None,
                case: Some(TANK_TRAP_PATHING_CASE_FRIENDLY_VEHICLE_REROUTE),
            })
        );
        assert_eq!(
            parse_dev_scenario_room(
                "tank_trap_pathing_matrix:unit=mortar_team:count=1:case=enemy_vehicle_reroute"
            ),
            Some(DevScenarioLaunch {
                id: "tank_trap_pathing_matrix",
                unit: EntityKind::MortarTeam,
                count: 1,
                blocker: None,
                case: Some(TANK_TRAP_PATHING_CASE_ENEMY_VEHICLE_REROUTE),
            })
        );
        assert_eq!(
            parse_dev_scenario_room(
                "tank_trap_pathing_matrix:unit=machine_gunner:count=1:case=infantry_pass_through"
            ),
            Some(DevScenarioLaunch {
                id: "tank_trap_pathing_matrix",
                unit: EntityKind::MachineGunner,
                count: 1,
                blocker: None,
                case: Some(TANK_TRAP_PATHING_CASE_INFANTRY_PASS_THROUGH),
            })
        );
        assert_eq!(
            parse_dev_scenario_room(
                "tank_trap_pathing_matrix:unit=rifleman:count=1:case=explicit_infantry_attack"
            ),
            Some(DevScenarioLaunch {
                id: "tank_trap_pathing_matrix",
                unit: EntityKind::Rifleman,
                count: 1,
                blocker: None,
                case: Some(TANK_TRAP_PATHING_CASE_EXPLICIT_INFANTRY_ATTACK),
            })
        );
        assert_eq!(
            parse_dev_scenario_room("entrenchment_inspection:unit=rifleman:count=1"),
            Some(DevScenarioLaunch {
                id: "entrenchment_inspection",
                unit: EntityKind::Rifleman,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_room("tank_coax_inspection:unit=tank:count=1"),
            Some(DevScenarioLaunch {
                id: "tank_coax_inspection",
                unit: EntityKind::Tank,
                count: 1,
                blocker: None,
                case: None,
            })
        );
        assert_eq!(
            parse_dev_scenario_launch(
                "vehicle_small_block_baseline",
                "scout_car",
                "3",
                Some("machine_gunner")
            ),
            Some(DevScenarioLaunch {
                id: "vehicle_small_block_baseline",
                unit: EntityKind::ScoutCar,
                count: 3,
                blocker: Some(EntityKind::MachineGunner),
                case: None,
            })
        );
    }

    #[test]
    fn rejects_unknown_launches() {
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "tank", "2", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_snaking_corridor", "city_centre", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("direct_reverse_order", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("direct_reverse_order", "tank", "4", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_wall_chokepoint", "worker", "3", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("scout_car_wall_chokepoint", "scout_car", "4", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_corner_wall", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_corner_wall", "tank", "4", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_small_block_baseline", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_small_block_baseline", "tank", "4", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("vehicle_small_block_baseline", "tank", "5", Some("tank")),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("factory_zero_gap_perpendicular", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("factory_zero_gap_perpendicular", "tank", "3", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("replay_142_vehicle_lock", "command_car", "2", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("replay_142_vehicle_lock", "scout_car", "1", None),
            None
        );
        for id in [
            "command_car_building_corner",
            "command_car_building_corner_west_southwest",
        ] {
            assert_eq!(parse_dev_scenario_launch(id, "tank", "1", None), None);
            assert_eq!(
                parse_dev_scenario_launch(id, "command_car", "2", None),
                None
            );
        }
        assert_eq!(
            parse_dev_scenario_launch("factory_wall_rally_spawn", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("factory_wall_rally_spawn", "tank", "3", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("tank_trap_line_horizontal", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("tank_trap_line_vertical", "tank", "2", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch_with_case(
                "tank_trap_pathing_matrix",
                "rifleman",
                "1",
                None,
                Some("friendly_vehicle_reroute")
            ),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch_with_case(
                "tank_trap_pathing_matrix",
                "worker",
                "1",
                None,
                Some("enemy_vehicle_reroute")
            ),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch_with_case(
                "tank_trap_pathing_matrix",
                "tank",
                "1",
                None,
                Some("infantry_pass_through")
            ),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch_with_case(
                "tank_trap_pathing_matrix",
                "rifleman",
                "2",
                None,
                Some("explicit_infantry_attack")
            ),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("tank_trap_pathing_matrix", "tank", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("entrenchment_inspection", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("entrenchment_inspection", "rifleman", "2", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("panzerfaust_duel", "rifleman", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("panzerfaust_windup_cancel", "panzerfaust", "2", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("panzerfaust_target_death", "tank", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("panzerfaust_entrenched_range", "panzerfaust", "2", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("panzerfaust_methamphetamines", "worker", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("tank_coax_inspection", "scout_car", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("tank_coax_inspection", "tank", "2", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("attack_move_reload_acquisition", "rifleman", "1", None),
            None
        );
        assert_eq!(
            parse_dev_scenario_launch("unknown", "worker", "1", None),
            None
        );
        assert_eq!(parse_dev_scenario_room("scout_car_snaking_corridor"), None);
    }
}
