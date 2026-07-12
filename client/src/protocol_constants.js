// Protocol constants and compact-code tables mirrored from rts_protocol.

// --- Server -> Client message tags (the `t` field) ---
export const S = Object.freeze({
  WELCOME: "welcome",
  LOBBY: "lobby",
  MATCH_COUNTDOWN: "matchCountdown",
  START: "start",
  SNAPSHOT: "snapshot",
  ROOM_TIME_STATE: "roomTimeState",
  LIVE_PAUSE_STATE: "livePauseState",
  OBSERVER_ANALYSIS: "observerAnalysis",
  JOIN_REPLAY_PROMPT: "joinReplayPrompt",
  BRANCH_FROM_TICK_CREATED: "branchFromTickCreated",
  BRANCH_STAGING: "branchStaging",
  LAB_STATE: "labState",
  LAB_RESULT: "labResult",
  SHUTDOWN_WARNING: "shutdownWarning",
  OBSERVATION_READY: "observationReady",
  GAME_OVER: "gameOver",
  PONG: "pong",
  COMMAND_RECEIPT: "commandReceipt",
  ERROR: "error",
});

export const LOBBY_KIND = Object.freeze({
  NORMAL: "normal",
  REPLAY: "replay",
});

// --- Client -> Server message tags ---
export const C = Object.freeze({
  JOIN: "join",
  READY: "ready",
  START: "start",
  SET_TEAM_PRESET: "setTeamPreset",
  SET_TEAM: "setTeam",
  SET_FACTION: "setFaction",
  ADD_AI: "addAi",
  SET_AI_PROFILE: "setAiProfile",
  REMOVE_AI: "removeAi",
  SET_SPECTATOR: "setSpectator",
  COMMAND: "command",
  GIVE_UP: "giveUp",
  PAUSE_GAME: "pauseGame",
  UNPAUSE_GAME: "unpauseGame",
  RETURN_TO_LOBBY: "returnToLobby",
  PING: "ping",
  NET_REPORT: "netReport",
  SET_ROOM_TIME_SPEED: "setRoomTimeSpeed",
  STEP_ROOM_TIME: "stepRoomTime",
  SEEK_ROOM_TIME: "seekRoomTime",
  SEEK_ROOM_TIME_TO: "seekRoomTimeTo",
  SET_VISION_SELECTION: "setVisionSelection",
  LAB: "lab",
  REQUEST_BRANCH_FROM_TICK: "requestBranchFromTick",
  CLAIM_BRANCH_SEAT: "claimBranchSeat",
  RELEASE_BRANCH_SEAT: "releaseBranchSeat",
  START_BRANCH: "startBranch",
  SELECT_MAP: "selectMap",
});

export const LAB_CHECKPOINT_SCENARIO = Object.freeze({
  KIND: "labCheckpointScenario",
  SCHEMA_VERSION: 1,
});

export const LAB_REPLAY = Object.freeze({
  SCHEMA: "rts.labReplay",
  KIND: "labReplay",
  SCHEMA_VERSION: 1,
  TIMELINE_KEYFRAME_INTERVAL_TICKS: 2000,
  MAX_ARTIFACT_BYTES: 8 * 1024 * 1024,
  MAX_OPERATIONS: 50000,
  MAX_OPERATION_JSON_BYTES: 64 * 1024,
  MAX_CHECKPOINT_PAYLOAD_BYTES: 4 * 1024 * 1024,
});

// --- Command discriminators (the `c` field) ---
export const CMD = Object.freeze({
  MOVE: "move",
  ATTACK_MOVE: "attackMove",
  ATTACK: "attack",
  DECONSTRUCT: "deconstruct",
  SETUP_ANTI_TANK_GUNS: "setupAntiTankGuns",
  TEAR_DOWN_ANTI_TANK_GUNS: "tearDownAntiTankGuns",
  CHARGE: "charge",
  USE_ABILITY: "useAbility",
  RECAST_ABILITY: "recastAbility",
  SET_AUTOCAST: "setAutocast",
  GATHER: "gather",
  BUILD: "build",
  TRAIN: "train",
  SET_PRODUCTION_REPEAT: "setProductionRepeat",
  RESEARCH: "research",
  CANCEL: "cancel",
  STOP: "stop",
  HOLD_POSITION: "holdPosition",
  SET_RALLY: "setRally",
});

// --- Terrain codes (must match protocol::terrain) ---
export const TERRAIN = Object.freeze({ GRASS: 0, ROCK: 1, WATER: 2 });
export const PASSABLE = Object.freeze({ 0: true, 1: false, 2: false });

// --- Entity kinds (must match protocol::kinds) ---
export const KIND = Object.freeze({
  WORKER: "worker",
  GOLEM: "golem",
  RIFLEMAN: "rifleman",
  MACHINE_GUNNER: "machine_gunner",
  PANZERFAUST: "panzerfaust",
  ANTI_TANK_GUN: "anti_tank_gun",
  MORTAR_TEAM: "mortar_team",
  ARTILLERY: "artillery",
  SCOUT_CAR: "scout_car",
  SCOUT_PLANE: "scout_plane",
  TANK: "tank",
  COMMAND_CAR: "command_car",
  EKAT: "ekat",
  CITY_CENTRE: "city_centre",
  ZAMOK: "zamok",
  DEPOT: "depot",
  BARRACKS: "barracks",
  TRAINING_CENTRE: "training_centre",
  RESEARCH_COMPLEX: "research_complex",
  FACTORY: "factory",
  STEELWORKS: "steelworks",
  TANK_TRAP: "tank_trap",
  PUMP_JACK: "pump_jack",
  STEEL: "steel",
  OIL: "oil",
});
export const UNIT_KINDS = Object.freeze([
  KIND.WORKER,
  KIND.GOLEM,
  KIND.RIFLEMAN,
  KIND.MACHINE_GUNNER,
  KIND.PANZERFAUST,
  KIND.ANTI_TANK_GUN,
  KIND.MORTAR_TEAM,
  KIND.ARTILLERY,
  KIND.SCOUT_CAR,
  KIND.SCOUT_PLANE,
  KIND.TANK,
  KIND.COMMAND_CAR,
  KIND.EKAT,
]);
export const BUILDING_KINDS = Object.freeze([
  KIND.CITY_CENTRE,
  KIND.ZAMOK,
  KIND.DEPOT,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.RESEARCH_COMPLEX,
  KIND.FACTORY,
  KIND.STEELWORKS,
  KIND.TANK_TRAP,
  KIND.PUMP_JACK,
]);
export const RESOURCE_KINDS = Object.freeze([KIND.STEEL, KIND.OIL]);

export const isUnit = (k) => UNIT_KINDS.includes(k);
export const isBuilding = (k) => BUILDING_KINDS.includes(k);
export const isResource = (k) => RESOURCE_KINDS.includes(k);

// --- Entity states (must match protocol::states) ---
export const STATE = Object.freeze({
  IDLE: "idle",
  MOVE: "move",
  ATTACK: "attack",
  GATHER: "gather",
  BUILD: "build",
  TRAIN: "train",
  CONSTRUCT: "construct",
  DEAD: "dead",
});

export const SETUP = Object.freeze({
  PACKED: "packed",
  SETTING_UP: "setting_up",
  DEPLOYED: "deployed",
  TEARING_DOWN: "tearing_down",
});

// --- Event discriminators (the `e` field) ---
export const EVENT = Object.freeze({
  ATTACK: "attack",
  DEATH: "death",
  BUILD: "build",
  NOTICE: "notice",
  SMOKE_LAUNCH: "smokeLaunch",
  MORTAR_LAUNCH: "mortarLaunch",
  MORTAR_IMPACT: "mortarImpact",
  ARTILLERY_TARGET: "artilleryTarget",
  ARTILLERY_IMPACT: "artilleryImpact",
  ARTILLERY_FIRING: "artilleryFiring",
  PANZERFAUST_LAUNCH: "panzerfaustLaunch",
  PANZERFAUST_IMPACT: "panzerfaustImpact",
  PANZERFAUST_CONVERSION: "panzerfaustConversion",
  OVERPENETRATION: "overpenetration",
  MISS: "miss",
});

export const ABILITY_OBJECT_KIND = Object.freeze({
  RETURN_MARKER: "returnMarker",
  MAGIC_ANCHOR: "magicAnchor",
  LINE_PROJECTILE: "lineProjectile",
});

export const NOTICE_SEVERITY = Object.freeze({
  INFO: "info",
  WARN: "warn",
  ALERT: "alert",
});

export const ABILITY = Object.freeze({
  CHARGE: "charge",
  SMOKE: "smoke",
  MORTAR_FIRE: "mortarFire",
  POINT_FIRE: "pointFire",
  BLANKET_FIRE: "blanketFire",
  BREAKTHROUGH: "breakthrough",
  SCOUT_PLANE: "scoutPlane",
  EKAT_TELEPORT: "ekatTeleport",
  EKAT_LINE_SHOT: "ekatLineShot",
  EKAT_MAGIC_ANCHOR: "ekatMagicAnchor",
  EKAT_CONSUME_GOLEM: "ekatConsumeGolem",
  DISMISS_SCOUT_PLANE: "dismissScoutPlane",
});

export const WEAPON_KIND = Object.freeze({
  WORKER_TOOLS: "worker_tools",
  GOLEM_FISTS: "golem_fists",
  RIFLEMAN_RIFLE: "rifleman_rifle",
  MACHINE_GUNNER_MG: "machine_gunner_mg",
  SCOUT_CAR_MG: "scout_car_mg",
  ANTI_TANK_GUN: "anti_tank_gun",
  PANZERFAUST_LOADED_SHOT: "panzerfaust_loaded_shot",
  MORTAR_TEAM_MORTAR: "mortar_team_mortar",
  ARTILLERY_GUN: "artillery_gun",
  TANK_CANNON: "tank_cannon",
  TANK_COAX: "tank_coax",
});

export const VISION_SELECTION = Object.freeze({
  ALL: "all",
  PLAYER: "player",
  PLAYERS: "players",
});

export const LAB_ROLE = Object.freeze({ OPERATOR: "operator", READ_ONLY: "readOnly" });
export const LAB_VISION = Object.freeze({
  FULL_WORLD: "fullWorld",
  TEAM: "team",
  TEAMS: "teams",
});
export const MOVEMENT_PATH_DIAGNOSTICS = Object.freeze({
  NONE: "none",
  OWNER_ONLY: "ownerOnly",
  ALL: "all",
});

// --- Compact snapshot wire schema (must match protocol.rs) ---
export const PREDICTION_PROTOCOL_VERSION = 1;
export const DEFAULT_FACTION_ID = "kriegsia";
export const COMPACT_SNAPSHOT_VERSION = 36;
export const SNAPSHOT_CODEC_VERSION = 1;
export const SNAPSHOT_CODEC = Object.freeze({
  COMPACT_JSON: "compact-json",
  MESSAGEPACK_COMPACT: "messagepack-compact",
});
export const SNAPSHOT_FRAME_KIND = Object.freeze({
  TEXT: "text",
  BINARY: "binary",
});

export const KIND_CODE = Object.freeze({
  [KIND.WORKER]: 1,
  [KIND.GOLEM]: 22,
  [KIND.RIFLEMAN]: 2,
  [KIND.MACHINE_GUNNER]: 3,
  [KIND.PANZERFAUST]: 24,
  [KIND.ANTI_TANK_GUN]: 4,
  [KIND.MORTAR_TEAM]: 15,
  [KIND.ARTILLERY]: 16,
  [KIND.TANK]: 5,
  [KIND.SCOUT_CAR]: 14,
  [KIND.SCOUT_PLANE]: 25,
  [KIND.CITY_CENTRE]: 6,
  [KIND.DEPOT]: 7,
  [KIND.BARRACKS]: 8,
  [KIND.TRAINING_CENTRE]: 9,
  [KIND.RESEARCH_COMPLEX]: 17,
  [KIND.COMMAND_CAR]: 18,
  [KIND.EKAT]: 19,
  [KIND.ZAMOK]: 20,
  [KIND.TANK_TRAP]: 21,
  [KIND.PUMP_JACK]: 23,
  [KIND.FACTORY]: 10,
  [KIND.STEEL]: 11,
  [KIND.OIL]: 12,
  [KIND.STEELWORKS]: 13,
});

export const STATE_CODE = Object.freeze({
  [STATE.IDLE]: 1,
  [STATE.MOVE]: 2,
  [STATE.ATTACK]: 3,
  [STATE.GATHER]: 4,
  [STATE.BUILD]: 5,
  [STATE.TRAIN]: 6,
  [STATE.CONSTRUCT]: 7,
  [STATE.DEAD]: 8,
});

export const SETUP_CODE = Object.freeze({
  [SETUP.PACKED]: 1,
  [SETUP.SETTING_UP]: 2,
  [SETUP.DEPLOYED]: 3,
  [SETUP.TEARING_DOWN]: 4,
});

export const UPGRADE = Object.freeze({
  METHAMPHETAMINES: "methamphetamines",
  ENTRENCHMENT: "entrenchment",
  ANTI_TANK_GUN_UNLOCK: "anti_tank_gun_unlock",
  TANK_UNLOCK: "tank_unlock",
  ARTILLERY_UNLOCK: "artillery_unlock",
  BALLISTIC_TABLES: "ballistic_tables",
  MORTAR_AUTOCAST: "mortar_autocast",
  SMOKE_PLUS: "smoke_plus",
});

export const UPGRADE_CODE = Object.freeze({
  [UPGRADE.METHAMPHETAMINES]: 1,
  [UPGRADE.ANTI_TANK_GUN_UNLOCK]: 2,
  [UPGRADE.TANK_UNLOCK]: 3,
  [UPGRADE.ARTILLERY_UNLOCK]: 4,
  [UPGRADE.MORTAR_AUTOCAST]: 5,
  [UPGRADE.BALLISTIC_TABLES]: 7,
  [UPGRADE.ENTRENCHMENT]: 8,
  [UPGRADE.SMOKE_PLUS]: 9,
});

export const WEAPON_KIND_CODE = Object.freeze({
  [WEAPON_KIND.WORKER_TOOLS]: 1,
  [WEAPON_KIND.GOLEM_FISTS]: 2,
  [WEAPON_KIND.RIFLEMAN_RIFLE]: 3,
  [WEAPON_KIND.MACHINE_GUNNER_MG]: 4,
  [WEAPON_KIND.SCOUT_CAR_MG]: 5,
  [WEAPON_KIND.ANTI_TANK_GUN]: 6,
  [WEAPON_KIND.PANZERFAUST_LOADED_SHOT]: 7,
  [WEAPON_KIND.MORTAR_TEAM_MORTAR]: 8,
  [WEAPON_KIND.ARTILLERY_GUN]: 9,
  [WEAPON_KIND.TANK_CANNON]: 10,
  [WEAPON_KIND.TANK_COAX]: 11,
});

export const EVENT_CODE = Object.freeze({
  [EVENT.ATTACK]: 1,
  [EVENT.DEATH]: 2,
  [EVENT.BUILD]: 3,
  [EVENT.NOTICE]: 4,
  [EVENT.SMOKE_LAUNCH]: 5,
  [EVENT.MORTAR_IMPACT]: 6,
  [EVENT.ARTILLERY_TARGET]: 7,
  [EVENT.ARTILLERY_IMPACT]: 8,
  [EVENT.MORTAR_LAUNCH]: 9,
  [EVENT.OVERPENETRATION]: 10,
  [EVENT.ARTILLERY_FIRING]: 11,
  [EVENT.PANZERFAUST_LAUNCH]: 12,
  [EVENT.PANZERFAUST_IMPACT]: 13,
  [EVENT.PANZERFAUST_CONVERSION]: 14,
  [EVENT.MISS]: 15,
});

export const ORDER_STAGE = Object.freeze({
  MOVE: "move",
  ATTACK_MOVE: "attackMove",
  ATTACK: "attack",
  DECONSTRUCT: "deconstruct",
  GATHER: "gather",
  BUILD: "build",
  CHARGE: "charge",
  SMOKE: "smoke",
  MORTAR_FIRE: "mortarFire",
  POINT_FIRE: "pointFire",
  BLANKET_FIRE: "blanketFire",
  BREAKTHROUGH: "breakthrough",
  SCOUT_PLANE: "scoutPlane",
  EKAT_TELEPORT: "ekatTeleport",
  EKAT_LINE_SHOT: "ekatLineShot",
  EKAT_MAGIC_ANCHOR: "ekatMagicAnchor",
  EKAT_CONSUME_GOLEM: "ekatConsumeGolem",
  DISMISS_SCOUT_PLANE: "dismissScoutPlane",
  SETUP_ANTI_TANK_GUNS: "setupAntiTankGuns",
});

export const ORDER_STAGE_CODE = Object.freeze({
  [ORDER_STAGE.MOVE]: 1,
  [ORDER_STAGE.ATTACK_MOVE]: 2,
  [ORDER_STAGE.ATTACK]: 3,
  [ORDER_STAGE.GATHER]: 4,
  [ORDER_STAGE.BUILD]: 5,
  [ORDER_STAGE.SMOKE]: 6,
  [ORDER_STAGE.SETUP_ANTI_TANK_GUNS]: 7,
  [ORDER_STAGE.CHARGE]: 8,
  [ORDER_STAGE.MORTAR_FIRE]: 9,
  [ORDER_STAGE.POINT_FIRE]: 10,
  [ORDER_STAGE.BREAKTHROUGH]: 11,
  [ORDER_STAGE.EKAT_TELEPORT]: 12,
  [ORDER_STAGE.EKAT_LINE_SHOT]: 13,
  [ORDER_STAGE.EKAT_MAGIC_ANCHOR]: 14,
  [ORDER_STAGE.DECONSTRUCT]: 15,
  [ORDER_STAGE.EKAT_CONSUME_GOLEM]: 16,
  [ORDER_STAGE.BLANKET_FIRE]: 17,
  [ORDER_STAGE.DISMISS_SCOUT_PLANE]: 18,
  [ORDER_STAGE.SCOUT_PLANE]: 19,
});

export const ABILITY_CODE = Object.freeze({
  [ABILITY.CHARGE]: 1,
  [ABILITY.SMOKE]: 2,
  [ABILITY.MORTAR_FIRE]: 3,
  [ABILITY.POINT_FIRE]: 4,
  [ABILITY.BREAKTHROUGH]: 5,
  [ABILITY.EKAT_TELEPORT]: 6,
  [ABILITY.EKAT_LINE_SHOT]: 7,
  [ABILITY.EKAT_MAGIC_ANCHOR]: 8,
  [ABILITY.EKAT_CONSUME_GOLEM]: 9,
  [ABILITY.BLANKET_FIRE]: 10,
  [ABILITY.DISMISS_SCOUT_PLANE]: 11,
  [ABILITY.SCOUT_PLANE]: 12,
});

export const ABILITY_OBJECT_KIND_CODE = Object.freeze({
  [ABILITY_OBJECT_KIND.RETURN_MARKER]: 1,
  [ABILITY_OBJECT_KIND.MAGIC_ANCHOR]: 2,
  [ABILITY_OBJECT_KIND.LINE_PROJECTILE]: 3,
});

export const NOTICE_SEVERITY_CODE = Object.freeze({
  [NOTICE_SEVERITY.INFO]: 1,
  [NOTICE_SEVERITY.WARN]: 2,
  [NOTICE_SEVERITY.ALERT]: 3,
});

export const KIND_BY_CODE = Object.freeze(reverseCodes(KIND_CODE));
export const STATE_BY_CODE = Object.freeze(reverseCodes(STATE_CODE));
export const SETUP_BY_CODE = Object.freeze(reverseCodes(SETUP_CODE));
export const EVENT_BY_CODE = Object.freeze(reverseCodes(EVENT_CODE));
export const ORDER_STAGE_BY_CODE = Object.freeze(reverseCodes(ORDER_STAGE_CODE));
export const ABILITY_BY_CODE = Object.freeze(reverseCodes(ABILITY_CODE));
export const UPGRADE_BY_CODE = Object.freeze(reverseCodes(UPGRADE_CODE));
export const ABILITY_OBJECT_KIND_BY_CODE = Object.freeze(reverseCodes(ABILITY_OBJECT_KIND_CODE));
export const NOTICE_SEVERITY_BY_CODE = Object.freeze(reverseCodes(NOTICE_SEVERITY_CODE));
export const WEAPON_KIND_BY_CODE = Object.freeze(reverseCodes(WEAPON_KIND_CODE));

export const MAX_COMPACT_ENTITIES = 20000;
export const MAX_COMPACT_RESOURCE_DELTAS = 20000;
export const MAX_COMPACT_SMOKES = 1024;
export const MAX_COMPACT_ABILITY_OBJECTS = 1024;
export const MAX_COMPACT_TRENCHES = 20000;
export const MAX_COMPACT_EVENTS = 5000;
export const MAX_COMPACT_ORDER_PLAN = 9;
export const MAX_COMPACT_ABILITIES = 8;
export const MAX_COMPACT_DEBUG_WAYPOINTS = 128;
export const MAX_COMPACT_VISIBLE_TILES = 65536;
export const MAX_COMPACT_REMEMBERED_BUILDINGS = 20000;
export const MAX_COMPACT_BUILDING_FOOTPRINT = 64;

function reverseCodes(table) {
  const out = {};
  for (const [name, code] of Object.entries(table)) out[code] = name;
  return out;
}
