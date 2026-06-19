// Wire protocol — JavaScript mirror of `server/src/protocol.rs`. See docs/design/protocol.md.
// Change both files together. Builders construct the exact JSON the server expects.

// --- Server -> Client message tags (the `t` field) ---
export const S = Object.freeze({
  WELCOME: "welcome",
  LOBBY: "lobby",
  MATCH_COUNTDOWN: "matchCountdown",
  START: "start",
  SNAPSHOT: "snapshot",
  ROOM_TIME_STATE: "roomTimeState",
  REPLAY_ANALYSIS: "replayAnalysis",
  JOIN_REPLAY_PROMPT: "joinReplayPrompt",
  REPLAY_BRANCH_CREATED: "replayBranchCreated",
  BRANCH_STAGING: "branchStaging",
  LAB_STATE: "labState",
  LAB_RESULT: "labResult",
  SHUTDOWN_WARNING: "shutdownWarning",
  GAME_OVER: "gameOver",
  PONG: "pong",
  COMMAND_RECEIPT: "commandReceipt",
  ERROR: "error",
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
  SET_QUICKSTART: "setQuickstart",
  SET_SPECTATOR: "setSpectator",
  COMMAND: "command",
  GIVE_UP: "giveUp",
  RETURN_TO_LOBBY: "returnToLobby",
  PING: "ping",
  NET_REPORT: "netReport",
  SET_ROOM_TIME_SPEED: "setRoomTimeSpeed",
  STEP_ROOM_TIME: "stepRoomTime",
  SEEK_ROOM_TIME: "seekRoomTime",
  SEEK_ROOM_TIME_TO: "seekRoomTimeTo",
  SET_REPLAY_VISION: "setReplayVision",
  LAB: "lab",
  REQUEST_REPLAY_BRANCH: "requestReplayBranch",
  CLAIM_BRANCH_SEAT: "claimBranchSeat",
  RELEASE_BRANCH_SEAT: "releaseBranchSeat",
  START_BRANCH: "startBranch",
  SELECT_MAP: "selectMap",
});

// --- Command discriminators (the `c` field) ---
export const CMD = Object.freeze({
  MOVE: "move",
  ATTACK_MOVE: "attackMove",
  ATTACK: "attack",
  SETUP_ANTI_TANK_GUNS: "setupAntiTankGuns",
  TEAR_DOWN_ANTI_TANK_GUNS: "tearDownAntiTankGuns",
  CHARGE: "charge",
  USE_ABILITY: "useAbility",
  RECAST_ABILITY: "recastAbility",
  SET_AUTOCAST: "setAutocast",
  GATHER: "gather",
  BUILD: "build",
  TRAIN: "train",
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
  RIFLEMAN: "rifleman",
  MACHINE_GUNNER: "machine_gunner",
  ANTI_TANK_GUN: "anti_tank_gun",
  MORTAR_TEAM: "mortar_team",
  ARTILLERY: "artillery",
  SCOUT_CAR: "scout_car",
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
  STEEL: "steel",
  OIL: "oil",
});
export const UNIT_KINDS = Object.freeze([
  KIND.WORKER,
  KIND.RIFLEMAN,
  KIND.MACHINE_GUNNER,
  KIND.ANTI_TANK_GUN,
  KIND.MORTAR_TEAM,
  KIND.ARTILLERY,
  KIND.SCOUT_CAR,
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
  BREAKTHROUGH: "breakthrough",
  EKAT_TELEPORT: "ekatTeleport",
  EKAT_LINE_SHOT: "ekatLineShot",
  EKAT_MAGIC_ANCHOR: "ekatMagicAnchor",
});

export const REPLAY_VISION = Object.freeze({
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
export const COMPACT_SNAPSHOT_VERSION = 22;
export const SNAPSHOT_CODEC_VERSION = 1;
export const SNAPSHOT_CODEC = Object.freeze({
  COMPACT_JSON: "compact-json",
});

export const KIND_CODE = Object.freeze({
  [KIND.WORKER]: 1,
  [KIND.RIFLEMAN]: 2,
  [KIND.MACHINE_GUNNER]: 3,
  [KIND.ANTI_TANK_GUN]: 4,
  [KIND.MORTAR_TEAM]: 15,
  [KIND.ARTILLERY]: 16,
  [KIND.TANK]: 5,
  [KIND.SCOUT_CAR]: 14,
  [KIND.CITY_CENTRE]: 6,
  [KIND.DEPOT]: 7,
  [KIND.BARRACKS]: 8,
  [KIND.TRAINING_CENTRE]: 9,
  [KIND.RESEARCH_COMPLEX]: 17,
  [KIND.COMMAND_CAR]: 18,
  [KIND.EKAT]: 19,
  [KIND.ZAMOK]: 20,
  [KIND.TANK_TRAP]: 21,
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
  ANTI_TANK_GUN_UNLOCK: "anti_tank_gun_unlock",
  TANK_UNLOCK: "tank_unlock",
  ARTILLERY_UNLOCK: "artillery_unlock",
  COMMAND_CAR_UNLOCK: "command_car_unlock",
  MORTAR_AUTOCAST: "mortar_autocast",
});

export const UPGRADE_CODE = Object.freeze({
  [UPGRADE.METHAMPHETAMINES]: 1,
  [UPGRADE.ANTI_TANK_GUN_UNLOCK]: 2,
  [UPGRADE.TANK_UNLOCK]: 3,
  [UPGRADE.ARTILLERY_UNLOCK]: 4,
  [UPGRADE.MORTAR_AUTOCAST]: 5,
  [UPGRADE.COMMAND_CAR_UNLOCK]: 6,
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
});

export const ORDER_STAGE = Object.freeze({
  MOVE: "move",
  ATTACK_MOVE: "attackMove",
  ATTACK: "attack",
  GATHER: "gather",
  BUILD: "build",
  CHARGE: "charge",
  SMOKE: "smoke",
  MORTAR_FIRE: "mortarFire",
  POINT_FIRE: "pointFire",
  BREAKTHROUGH: "breakthrough",
  EKAT_TELEPORT: "ekatTeleport",
  EKAT_LINE_SHOT: "ekatLineShot",
  EKAT_MAGIC_ANCHOR: "ekatMagicAnchor",
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

const KIND_BY_CODE = Object.freeze(reverseCodes(KIND_CODE));
const STATE_BY_CODE = Object.freeze(reverseCodes(STATE_CODE));
const SETUP_BY_CODE = Object.freeze(reverseCodes(SETUP_CODE));
const EVENT_BY_CODE = Object.freeze(reverseCodes(EVENT_CODE));
const ORDER_STAGE_BY_CODE = Object.freeze(reverseCodes(ORDER_STAGE_CODE));
const ABILITY_BY_CODE = Object.freeze(reverseCodes(ABILITY_CODE));
const UPGRADE_BY_CODE = Object.freeze(reverseCodes(UPGRADE_CODE));
const ABILITY_OBJECT_KIND_BY_CODE = Object.freeze(reverseCodes(ABILITY_OBJECT_KIND_CODE));
const NOTICE_SEVERITY_BY_CODE = Object.freeze(reverseCodes(NOTICE_SEVERITY_CODE));

const MAX_COMPACT_ENTITIES = 20000;
const MAX_COMPACT_RESOURCE_DELTAS = 20000;
const MAX_COMPACT_SMOKES = 1024;
const MAX_COMPACT_ABILITY_OBJECTS = 1024;
const MAX_COMPACT_EVENTS = 5000;
const MAX_COMPACT_ORDER_PLAN = 9;
const MAX_COMPACT_ABILITIES = 8;
const MAX_COMPACT_DEBUG_WAYPOINTS = 128;
const MAX_COMPACT_VISIBLE_TILES = 65536;
const MAX_COMPACT_REMEMBERED_BUILDINGS = 20000;
const MAX_COMPACT_BUILDING_FOOTPRINT = 64;

/**
 * Parse one WebSocket server frame into a raw protocol message.
 * Compact JSON text remains the only live snapshot codec. Binary frames are
 * intentionally rejected until an experiment graduates to a documented codec.
 * @param {string|ArrayBuffer|ArrayBufferView} frame
 * @returns {object}
 */
export function parseServerFrame(frame) {
  if (typeof frame === "string") return JSON.parse(frame);
  if (frame instanceof ArrayBuffer || ArrayBuffer.isView(frame)) {
    throw new Error("unsupported binary server frame");
  }
  throw new Error("unsupported server frame type");
}

/**
 * Expand server messages into the semantic shapes the rest of the client expects.
 * Object-shaped JSON snapshots from older servers are passed through unchanged.
 * @param {object} raw parsed WebSocket JSON payload
 * @returns {object}
 */
export function decodeServerMessage(raw) {
  if (!raw || typeof raw !== "object") throw new Error("server message must be an object");
  if (raw.t === S.SNAPSHOT && raw.v !== undefined) return decodeCompactSnapshot(raw);
  return raw;
}

function decodeCompactSnapshot(raw) {
  if (raw.v !== COMPACT_SNAPSHOT_VERSION) {
    throw new Error(`unsupported compact snapshot version: ${raw.v}`);
  }

  const scalars = readArray(raw.s, "snapshot scalars", 5);
  if (scalars.length !== 5) throw new Error("compact snapshot scalar count mismatch");

  return {
    t: S.SNAPSHOT,
    tick: readU32(scalars[0], "tick"),
    steel: readU32(scalars[1], "steel"),
    oil: readU32(scalars[2], "oil"),
    supplyUsed: readU32(scalars[3], "supplyUsed"),
    supplyCap: readU32(scalars[4], "supplyCap"),
    entities: readArray(raw.e, "entities", MAX_COMPACT_ENTITIES).map(decodeCompactEntity),
    resourceDeltas: readOptionalArray(
      raw.r,
      "resourceDeltas",
      MAX_COMPACT_RESOURCE_DELTAS,
    ).map(decodeCompactResourceDelta),
    smokes: readOptionalArray(raw.sm, "smokes", MAX_COMPACT_SMOKES).map(decodeCompactSmoke),
    abilityObjects: readOptionalArray(
      raw.ao,
      "abilityObjects",
      MAX_COMPACT_ABILITY_OBJECTS,
    ).map(decodeCompactAbilityObject),
    visibleTiles: decodeVisibilityRuns(raw.fg),
    rememberedBuildings: readOptionalArray(
      raw.mb,
      "rememberedBuildings",
      MAX_COMPACT_REMEMBERED_BUILDINGS,
    ).map(decodeCompactRememberedBuilding),
    events: readOptionalArray(raw.ev, "events", MAX_COMPACT_EVENTS).map(decodeCompactEvent),
    playerResources: readOptionalArray(raw.pr, "playerResources", 32).map(
      decodeCompactPlayerResource,
    ),
    upgrades: readOptionalArray(raw.u, "upgrades", 32).map((code, index) =>
      readCode(code, UPGRADE_BY_CODE, `upgrade.${index}`),
    ),
    netStatus: decodeCompactNetStatus(raw.n),
  };
}

function decodeVisibilityRuns(record) {
  if (record == null) return [];
  const runs = readArray(record, "visibleTiles", MAX_COMPACT_VISIBLE_TILES + 1);
  if (runs.length < 2) throw new Error("visibleTiles run data must include a value and length");
  let value = readU32(runs[0], "visibleTiles.first");
  if (value !== 0 && value !== 1) throw new Error("visibleTiles.first must be 0 or 1");
  const out = [];
  for (let i = 1; i < runs.length; i++) {
    const len = readU32(runs[i], `visibleTiles.run.${i}`);
    if (len === 0) throw new Error("visibleTiles run length must be positive");
    if (out.length + len > MAX_COMPACT_VISIBLE_TILES) {
      throw new Error("visibleTiles exceeds compact bounds");
    }
    for (let j = 0; j < len; j++) out.push(value);
    value = value === 1 ? 0 : 1;
  }
  return out;
}

function decodeCompactRememberedBuilding(record, index) {
  const fields = readArray(record, `remembered building ${index}`, 7);
  if (fields.length !== 7) throw new Error(`remembered building ${index} field count mismatch`);
  return {
    id: readU32(fields[0], "rememberedBuilding.id"),
    owner: readU32(fields[1], "rememberedBuilding.owner"),
    kind: readCode(fields[2], KIND_BY_CODE, "rememberedBuilding.kind"),
    x: readNumber(fields[3], "rememberedBuilding.x"),
    y: readNumber(fields[4], "rememberedBuilding.y"),
    footprint: readArray(
      fields[5],
      "rememberedBuilding.footprint",
      MAX_COMPACT_BUILDING_FOOTPRINT,
    ).map((tile, tileIndex) => {
      const pair = readArray(tile, `rememberedBuilding.footprint.${tileIndex}`, 2);
      if (pair.length !== 2) {
        throw new Error(`rememberedBuilding.footprint.${tileIndex} field count mismatch`);
      }
      return [
        readU32(pair[0], `rememberedBuilding.footprint.${tileIndex}.x`),
        readU32(pair[1], `rememberedBuilding.footprint.${tileIndex}.y`),
      ];
    }),
    observedTick: readU32(fields[6], "rememberedBuilding.observedTick"),
  };
}

function decodeCompactSmoke(record, index) {
  const fields = readArray(record, `smoke ${index}`, 5);
  if (fields.length !== 5) throw new Error(`smoke ${index} field count mismatch`);
  return {
    id: readU32(fields[0], "smoke.id"),
    x: readNumber(fields[1], "smoke.x"),
    y: readNumber(fields[2], "smoke.y"),
    radiusTiles: readNumber(fields[3], "smoke.radiusTiles"),
    expiresIn: readU32(fields[4], "smoke.expiresIn"),
  };
}

function decodeCompactAbilityObject(record, index) {
  const fields = readArray(record, `ability object ${index}`, 9);
  if (fields.length !== 9) throw new Error(`ability object ${index} field count mismatch`);
  const object = {
    id: readU32(fields[0], "abilityObject.id"),
    owner: readU32(fields[1], "abilityObject.owner"),
    ability: readCode(fields[2], ABILITY_BY_CODE, "abilityObject.ability"),
    kind: readCode(fields[3], ABILITY_OBJECT_KIND_BY_CODE, "abilityObject.kind"),
    x: readNumber(fields[4], "abilityObject.x"),
    y: readNumber(fields[5], "abilityObject.y"),
  };
  if (fields[6] != null) object.expiresIn = readU32(fields[6], "abilityObject.expiresIn");
  if (fields[7] != null) {
    object.sourceCasterId = readU32(fields[7], "abilityObject.sourceCasterId");
  }
  if (fields[8] != null) {
    object.ownerState = decodeCompactAbilityObjectOwnerState(fields[8]);
  }
  return object;
}

function decodeCompactAbilityObjectOwnerState(record) {
  const fields = readArray(record, "abilityObject.ownerState", 6);
  if (fields.length !== 6) throw new Error("abilityObject.ownerState field count mismatch");
  const state = {};
  if (fields[0] != null) {
    state.earliestReturnTick = readU32(fields[0], "abilityObject.ownerState.earliestReturnTick");
  }
  if (fields[1] != null) state.hp = readU32(fields[1], "abilityObject.ownerState.hp");
  if (fields[2] != null) state.radius = readNumber(fields[2], "abilityObject.ownerState.radius");
  if (fields[3] != null) {
    state.destroyedLockoutTicks = readU32(
      fields[3],
      "abilityObject.ownerState.destroyedLockoutTicks",
    );
  }
  if (fields[4] != null) {
    state.distanceTraveled = readNumber(fields[4], "abilityObject.ownerState.distanceTraveled");
  }
  if (fields[5] != null) state.ticksOut = readU32(fields[5], "abilityObject.ownerState.ticksOut");
  return state;
}

function decodeCompactNetStatus(record) {
  const fields = readArray(record, "netStatus", 8);
  if (fields.length !== 5 && fields.length !== 8) throw new Error("netStatus field count mismatch");
  const flags = readU32(fields[2], "netStatus.flags");
  const status = {
    serverLagMs: readU32(fields[0], "netStatus.serverLagMs"),
    tickMs: readU32(fields[1], "netStatus.tickMs"),
    slowTick: !!(flags & 1),
    slowTickCount: readU32(fields[3], "netStatus.slowTickCount"),
    headOfLine: !!(flags & 2),
    headOfLineCount: readU32(fields[4], "netStatus.headOfLineCount"),
  };
  if (fields.length === 8) {
    status.predictionVersion = readU32(fields[5], "netStatus.predictionVersion");
    status.lastSimConsumedClientSeq = readU32(fields[6], "netStatus.lastSimConsumedClientSeq");
    status.lastSimConsumedClientTick =
      fields[7] == null ? null : readU32(fields[7], "netStatus.lastSimConsumedClientTick");
  }
  return status;
}

function decodeCompactPlayerResource(record, index) {
  const fields = readArray(record, `playerResource ${index}`, 5);
  if (fields.length < 5) throw new Error(`playerResource ${index} is too short`);
  return {
    id: readU32(fields[0], "playerResource.id"),
    steel: readU32(fields[1], "playerResource.steel"),
    oil: readU32(fields[2], "playerResource.oil"),
    supplyUsed: readU32(fields[3], "playerResource.supplyUsed"),
    supplyCap: readU32(fields[4], "playerResource.supplyCap"),
  };
}

function decodeCompactEntity(record, index) {
  const fields = readArray(record, `entity ${index}`, 30);
  if (fields.length < 8) throw new Error(`entity ${index} is too short`);
  const entity = {
    id: readU32(fields[0], "entity.id"),
    owner: readU32(fields[1], "entity.owner"),
    kind: readCode(fields[2], KIND_BY_CODE, "entity.kind"),
    x: readNumber(fields[3], "entity.x"),
    y: readNumber(fields[4], "entity.y"),
    hp: readU32(fields[5], "entity.hp"),
    maxHp: readU32(fields[6], "entity.maxHp"),
    state: readCode(fields[7], STATE_BY_CODE, "entity.state"),
  };

  assignOptional(entity, "facing", fields, 8, readNumber);
  assignOptional(entity, "weaponFacing", fields, 9, readNumber);
  assignOptionalCode(entity, "prodKind", fields, 10, KIND_BY_CODE);
  assignOptional(entity, "prodProgress", fields, 11, readNumber);
  assignOptional(entity, "prodQueue", fields, 12, readU32);
  assignOptional(entity, "buildProgress", fields, 13, readNumber);
  assignOptional(entity, "latchedNode", fields, 14, readU32);
  assignOptional(entity, "targetId", fields, 15, readU32);
  assignOptionalCode(entity, "setupState", fields, 16, SETUP_BY_CODE);
  assignOptional(entity, "remaining", fields, 17, readU32);
  assignRally(entity, fields, 18);
  assignOptional(entity, "oilUsed", fields, 19, readNumber);
  assignOptional(entity, "setupFacing", fields, 20, readNumber);
  assignOrderPlan(entity, fields, 21);
  assignOptional(entity, "chargeCooldownLeft", fields, 22, readU32);
  assignAbilities(entity, fields, 23);
  assignOptional(entity, "breakthroughTicks", fields, 24, readU32);
  assignOptional(entity, "visionOnly", fields, 25, readBool);
  assignDebugPath(entity, fields, 26);
  assignRallyPlan(entity, fields, 27);
  assignOptionalCode(entity, "prodUpgrade", fields, 28, UPGRADE_BY_CODE);
  assignOptional(entity, "buildActive", fields, 29, readBool);
  return entity;
}

/** Decode the optional rally-point slot ([x, y] world px, owner-only) into `entity.rally`. */
function assignRally(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const pair = readArray(fields[index], "entity.rally", 2);
  if (pair.length !== 2) throw new Error("entity.rally must have two elements");
  target.rally = [readNumber(pair[0], "entity.rally.x"), readNumber(pair[1], "entity.rally.y")];
}

/** Decode owner-only current + queued order stages into `entity.orderPlan`. */
function assignOrderPlan(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const markers = readArray(fields[index], "entity.orderPlan", MAX_COMPACT_ORDER_PLAN);
  target.orderPlan = markers.map((record, markerIndex) =>
    readOrderPlanMarker(record, `entity.orderPlan.${markerIndex}`),
  );
}

/** Decode owner-only building rally stages into `entity.rallyPlan`. */
function assignRallyPlan(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const markers = readArray(fields[index], "entity.rallyPlan", 4);
  target.rallyPlan = markers.map((record, markerIndex) =>
    readOrderPlanMarker(record, `entity.rallyPlan.${markerIndex}`),
  );
}

function readOrderPlanMarker(record, label) {
  const marker = readArray(record, label, 3);
  if (marker.length !== 3) {
    throw new Error(`${label} field count mismatch`);
  }
  return {
    kind: readCode(marker[0], ORDER_STAGE_BY_CODE, `${label}.kind`),
    x: readNumber(marker[1], `${label}.x`),
    y: readNumber(marker[2], `${label}.y`),
  };
}

/** Decode owner-only ability cooldown affordances into `entity.abilities`. */
function assignAbilities(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const cooldowns = readArray(fields[index], "entity.abilities", MAX_COMPACT_ABILITIES);
  target.abilities = cooldowns.map((record, abilityIndex) =>
    readAbilityCooldown(record, `entity.abilities.${abilityIndex}`),
  );
}

function readAbilityCooldown(record, label) {
  const fields = readArray(record, label, 8);
  if (fields.length < 2 || fields.length > 8) throw new Error(`${label} field count mismatch`);
  const ability = {
    ability: readCode(fields[0], ABILITY_BY_CODE, `${label}.ability`),
    cooldownLeft: readU32(fields[1], `${label}.cooldownLeft`),
  };
  if (fields.length > 2 && fields[2] != null) {
    ability.remainingUses = readU32(fields[2], `${label}.remainingUses`);
  }
  if (fields.length > 3 && fields[3] != null) {
    ability.autocastEnabled = readBool(fields[3], `${label}.autocastEnabled`);
  }
  if (fields.length > 4 && fields[4] != null) {
    ability.activeObjectId = readU32(fields[4], `${label}.activeObjectId`);
  }
  if (fields.length > 5 && fields[5] != null) {
    ability.availableTick = readU32(fields[5], `${label}.availableTick`);
  }
  if (fields.length > 6 && fields[6] != null) {
    ability.lockoutUntilTick = readU32(fields[6], `${label}.lockoutUntilTick`);
  }
  if (fields.length > 7 && fields[7] != null) {
    ability.expiresIn = readU32(fields[7], `${label}.expiresIn`);
  }
  return ability;
}

/** Decode projection-policy movement path diagnostics. */
function assignDebugPath(target, fields, index) {
  if (index >= fields.length || fields[index] == null) return;
  const record = readArray(fields[index], "entity.debugPath", 6);
  if (record.length !== 6) throw new Error("entity.debugPath field count mismatch");
  target.debugPath = {
    waypoints: readArray(record[0], "entity.debugPath.waypoints", MAX_COMPACT_DEBUG_WAYPOINTS).map(
      (point, pointIndex) => decodeCompactDebugPoint(point, `entity.debugPath.waypoints.${pointIndex}`),
    ),
    goal: record[1] == null ? null : decodeCompactDebugPoint(record[1], "entity.debugPath.goal"),
    lastRepathTick: readU32(record[2], "entity.debugPath.lastRepathTick"),
    stuckTicks: readU32(record[3], "entity.debugPath.stuckTicks"),
    staticBlockedTicks: readU32(record[4], "entity.debugPath.staticBlockedTicks"),
    totalWaypoints: readU32(record[5], "entity.debugPath.totalWaypoints"),
  };
}

function decodeCompactDebugPoint(record, label) {
  const [x, y] = decodeCompactPoint(record, label);
  return { x, y };
}

function decodeCompactResourceDelta(record, index) {
  const fields = readArray(record, `resource delta ${index}`, 2);
  if (fields.length !== 2) throw new Error(`resource delta ${index} field count mismatch`);
  return {
    id: readU32(fields[0], "resourceDelta.id"),
    remaining: readU32(fields[1], "resourceDelta.remaining"),
  };
}

function decodeCompactEvent(record, index) {
  const fields = readArray(record, `event ${index}`, 6);
  if (fields.length < 1) throw new Error(`event ${index} is too short`);
  const eventKind = readCode(fields[0], EVENT_BY_CODE, "event.kind");
  switch (eventKind) {
    case EVENT.ATTACK:
      if (fields.length !== 3 && fields.length !== 4 && fields.length !== 5) {
        throw new Error(`attack event ${index} field count mismatch`);
      }
      {
        const ev = {
        e: EVENT.ATTACK,
        from: readU32(fields[1], "event.from"),
        to: readU32(fields[2], "event.to"),
        };
        if (fields.length > 3 && fields[3] != null) {
          ev.reveal = decodeCompactAttackReveal(fields[3], index);
        }
        if (fields.length > 4 && fields[4] != null) {
          ev.toPos = decodeCompactPoint(fields[4], "event.toPos");
        }
        return ev;
      }
    case EVENT.DEATH:
      requireLength(fields, 5, `death event ${index}`);
      return {
        e: EVENT.DEATH,
        id: readU32(fields[1], "event.id"),
        x: readNumber(fields[2], "event.x"),
        y: readNumber(fields[3], "event.y"),
        kind: readCode(fields[4], KIND_BY_CODE, "event.kind"),
      };
    case EVENT.BUILD:
      requireLength(fields, 3, `build event ${index}`);
      return {
        e: EVENT.BUILD,
        id: readU32(fields[1], "event.id"),
        kind: readCode(fields[2], KIND_BY_CODE, "event.kind"),
      };
    case EVENT.NOTICE:
      if (fields.length !== 2 && fields.length !== 3 && fields.length !== 5) {
        throw new Error(`notice event ${index} field count mismatch`);
      }
      if (typeof fields[1] !== "string") throw new Error(`notice event ${index} msg must be string`);
      return decodeCompactNotice(fields, index);
    case EVENT.SMOKE_LAUNCH: {
      requireLength(fields, 4, `smoke launch event ${index}`);
      const from = decodeCompactPoint(fields[1], "event.smokeLaunch.from");
      const to = decodeCompactPoint(fields[2], "event.smokeLaunch.to");
      return {
        e: EVENT.SMOKE_LAUNCH,
        fromX: from[0],
        fromY: from[1],
        toX: to[0],
        toY: to[1],
        delayTicks: readU32(fields[3], "event.smokeLaunch.delayTicks"),
      };
    }
    case EVENT.MORTAR_LAUNCH: {
      requireLength(fields, 6, `mortar launch event ${index}`);
      const fromPoint = decodeCompactPoint(fields[2], "event.mortarLaunch.from");
      const to = decodeCompactPoint(fields[3], "event.mortarLaunch.to");
      return {
        e: EVENT.MORTAR_LAUNCH,
        from: readU32(fields[1], "event.mortarLaunch.from"),
        fromX: fromPoint[0],
        fromY: fromPoint[1],
        toX: to[0],
        toY: to[1],
        radiusTiles: readNumber(fields[4], "event.mortarLaunch.radiusTiles"),
        delayTicks: readU32(fields[5], "event.mortarLaunch.delayTicks"),
      };
    }
    case EVENT.MORTAR_IMPACT:
      if (fields.length !== 4 && fields.length !== 5 && fields.length !== 6) {
        throw new Error(`mortar impact event ${index} field count mismatch`);
      }
      {
        const ev = {
        e: EVENT.MORTAR_IMPACT,
        x: readNumber(fields[1], "event.mortarImpact.x"),
        y: readNumber(fields[2], "event.mortarImpact.y"),
        radiusTiles: readNumber(fields[3], "event.mortarImpact.radiusTiles"),
        };
        if (fields.length > 4 && fields[4] != null) {
          ev.from = readU32(fields[4], "event.mortarImpact.from");
        }
        if (fields.length > 5 && fields[5] != null) {
          ev.reveal = decodeCompactAttackReveal(fields[5], index);
        }
        return ev;
      }
    case EVENT.ARTILLERY_TARGET: {
      requireLength(fields, 5, `artillery target event ${index}`);
      const target = decodeCompactPoint(fields[2], "event.artilleryTarget.target");
      return {
        e: EVENT.ARTILLERY_TARGET,
        from: readU32(fields[1], "event.artilleryTarget.from"),
        x: target[0],
        y: target[1],
        radiusTiles: readNumber(fields[3], "event.artilleryTarget.radiusTiles"),
        delayTicks: readU32(fields[4], "event.artilleryTarget.delayTicks"),
      };
    }
    case EVENT.ARTILLERY_IMPACT:
      requireLength(fields, 4, `artillery impact event ${index}`);
      return {
        e: EVENT.ARTILLERY_IMPACT,
        x: readNumber(fields[1], "event.artilleryImpact.x"),
        y: readNumber(fields[2], "event.artilleryImpact.y"),
        radiusTiles: readNumber(fields[3], "event.artilleryImpact.radiusTiles"),
      };
    default:
      throw new Error(`unknown compact event kind ${eventKind}`);
  }
}

function decodeCompactAttackReveal(record, index) {
  const fields = readArray(record, `attack reveal ${index}`, 7);
  if (fields.length < 4) throw new Error(`attack reveal ${index} is too short`);
  const reveal = {
    owner: readU32(fields[0], "attackReveal.owner"),
    kind: readCode(fields[1], KIND_BY_CODE, "attackReveal.kind"),
    x: readNumber(fields[2], "attackReveal.x"),
    y: readNumber(fields[3], "attackReveal.y"),
  };
  assignOptional(reveal, "facing", fields, 4, readNumber);
  assignOptional(reveal, "weaponFacing", fields, 5, readNumber);
  assignOptionalCode(reveal, "setupState", fields, 6, SETUP_BY_CODE);
  return reveal;
}

function decodeCompactPoint(record, label) {
  const pair = readArray(record, label, 2);
  if (pair.length !== 2) throw new Error(`${label} must have two elements`);
  return [readNumber(pair[0], `${label}.x`), readNumber(pair[1], `${label}.y`)];
}

function decodeCompactNotice(fields, index) {
  const ev = {
    e: EVENT.NOTICE,
    msg: fields[1],
    severity: NOTICE_SEVERITY.INFO,
  };
  if (fields.length >= 3) {
    ev.severity = readCode(fields[2], NOTICE_SEVERITY_BY_CODE, `notice event ${index}.severity`);
  }
  if (fields.length === 5) {
    ev.x = readNumber(fields[3], `notice event ${index}.x`);
    ev.y = readNumber(fields[4], `notice event ${index}.y`);
  }
  return ev;
}

function assignOptional(target, field, fields, index, reader) {
  if (index >= fields.length || fields[index] == null) return;
  target[field] = reader(fields[index], `entity.${field}`);
}

function assignOptionalCode(target, field, fields, index, table) {
  if (index >= fields.length || fields[index] == null) return;
  target[field] = readCode(fields[index], table, `entity.${field}`);
}

function readOptionalArray(value, name, maxLength) {
  if (value == null) return [];
  return readArray(value, name, maxLength);
}

function readArray(value, name, maxLength) {
  if (!Array.isArray(value)) throw new Error(`${name} must be an array`);
  if (value.length > maxLength) throw new Error(`${name} exceeds max length ${maxLength}`);
  return value;
}

function readNumber(value, name) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${name} must be a finite number`);
  }
  return value;
}

function readU32(value, name) {
  const number = readNumber(value, name);
  if (!Number.isInteger(number) || number < 0 || number > 0xffffffff) {
    throw new Error(`${name} must be a u32`);
  }
  return number;
}

function readBool(value, name) {
  if (typeof value !== "boolean") throw new Error(`${name} must be a boolean`);
  return value;
}

function readCode(value, table, name) {
  const code = readU32(value, name);
  if (!Object.prototype.hasOwnProperty.call(table, code)) {
    throw new Error(`${name} has unknown code ${code}`);
  }
  return table[code];
}

function requireLength(fields, expected, name) {
  if (fields.length !== expected) throw new Error(`${name} field count mismatch`);
}

function reverseCodes(table) {
  const out = {};
  for (const [name, code] of Object.entries(table)) out[code] = name;
  return out;
}

// --- Client -> Server builders ---
export const msg = Object.freeze({
  join: (name, room = "main", spectator = false, replayOk = false) => {
    const payload = {
      t: C.JOIN,
      name,
      room,
      spectator: !!spectator,
    };
    if (replayOk) payload.replayOk = true;
    return payload;
  },
  ready: (ready) => ({ t: C.READY, ready: !!ready }),
  start: () => ({ t: C.START }),
  setTeamPreset: (preset) => ({ t: C.SET_TEAM_PRESET, preset }),
  setTeam: (id, teamId) => ({ t: C.SET_TEAM, id, teamId }),
  setFaction: (factionId) => ({ t: C.SET_FACTION, factionId }),
  addAi: (teamId = undefined, aiProfileId = undefined) => {
    const payload = { t: C.ADD_AI };
    if (teamId != null) payload.teamId = teamId;
    if (aiProfileId != null) payload.aiProfileId = aiProfileId;
    return payload;
  },
  setAiProfile: (id, aiProfileId) => ({ t: C.SET_AI_PROFILE, id, aiProfileId }),
  removeAi: (id) => ({ t: C.REMOVE_AI, id }),
  setQuickstart: (enabled) => ({ t: C.SET_QUICKSTART, enabled: !!enabled }),
  setSpectator: (spectator, id = undefined) => {
    const payload = { t: C.SET_SPECTATOR, spectator: !!spectator };
    if (id != null) payload.id = id;
    return payload;
  },
  command: (cmd, clientSeq) => ({ t: C.COMMAND, clientSeq, cmd }),
  giveUp: () => ({ t: C.GIVE_UP }),
  returnToLobby: () => ({ t: C.RETURN_TO_LOBBY }),
  ping: (ts) => ({ t: C.PING, ts }),
  netReport: (report) => ({ t: C.NET_REPORT, report }),
  setRoomTimeSpeed: (speed) => ({ t: C.SET_ROOM_TIME_SPEED, speed }),
  stepRoomTime: () => ({ t: C.STEP_ROOM_TIME }),
  seekRoomTime: (ticksBack) => ({ t: C.SEEK_ROOM_TIME, ticksBack }),
  seekRoomTimeTo: (tick) => ({ t: C.SEEK_ROOM_TIME_TO, tick }),
  setReplayVision: (vision) => ({ t: C.SET_REPLAY_VISION, vision }),
  lab: (requestId, op) => ({ t: C.LAB, requestId, op }),
  labExportScenario: (requestId, name = undefined) => {
    const op = { op: "exportScenario" };
    if (name != null) op.name = name;
    return { t: C.LAB, requestId, op };
  },
  labImportScenario: (requestId, scenario) => ({
    t: C.LAB,
    requestId,
    op: { op: "importScenario", scenario },
  }),
  labSpawnEntity: (requestId, { owner, kind, x, y, completed = false }) => ({
    t: C.LAB,
    requestId,
    op: { op: "spawnEntity", owner, kind, x, y, completed: !!completed },
  }),
  labDeleteEntity: (requestId, entityId) => ({
    t: C.LAB,
    requestId,
    op: { op: "deleteEntity", entityId },
  }),
  labMoveEntity: (requestId, entityId, x, y) => ({
    t: C.LAB,
    requestId,
    op: { op: "moveEntity", entityId, x, y },
  }),
  labSetEntityOwner: (requestId, entityId, owner) => ({
    t: C.LAB,
    requestId,
    op: { op: "setEntityOwner", entityId, owner },
  }),
  labSetPlayerResources: (requestId, playerId, steel, oil) => ({
    t: C.LAB,
    requestId,
    op: { op: "setPlayerResources", playerId, steel, oil },
  }),
  labSetCompletedResearch: (requestId, playerId, upgrade, completed) => ({
    t: C.LAB,
    requestId,
    op: { op: "setCompletedResearch", playerId, upgrade, completed: !!completed },
  }),
  labSetVision: (requestId, vision) => ({
    t: C.LAB,
    requestId,
    op: { op: "setVision", vision },
  }),
  labIssueCommandAs: (requestId, playerId, command) => ({
    t: C.LAB,
    requestId,
    op: { op: "issueCommandAs", playerId, cmd: command },
  }),
  requestReplayBranch: () => ({ t: C.REQUEST_REPLAY_BRANCH }),
  claimBranchSeat: (playerId) => ({ t: C.CLAIM_BRANCH_SEAT, playerId }),
  releaseBranchSeat: (playerId) => ({ t: C.RELEASE_BRANCH_SEAT, playerId }),
  startBranch: () => ({ t: C.START_BRANCH }),
  replayVisionAll: () => ({ t: C.SET_REPLAY_VISION, vision: { mode: REPLAY_VISION.ALL } }),
  replayVisionPlayer: (playerId) => ({
    t: C.SET_REPLAY_VISION,
    vision: { mode: REPLAY_VISION.PLAYER, playerId },
  }),
  replayVisionPlayers: (playerIds) => ({
    t: C.SET_REPLAY_VISION,
    vision: { mode: REPLAY_VISION.PLAYERS, playerIds },
  }),
  labVisionFullWorld: () => ({ mode: LAB_VISION.FULL_WORLD }),
  labVisionTeam: (teamId) => ({ mode: LAB_VISION.TEAM, teamId }),
  labVisionTeams: (teamIds) => ({ mode: LAB_VISION.TEAMS, teamIds }),
  selectMap: (map) => ({ t: C.SELECT_MAP, map }),
});

// --- Command builders (the `cmd` payload) ---
function withQueued(command, queued) {
  if (queued) command.queued = true;
  return command;
}

export const cmd = Object.freeze({
  move: (units, x, y, queued = false) => withQueued({ c: CMD.MOVE, units, x, y }, queued),
  attackMove: (units, x, y, queued = false) =>
    withQueued({ c: CMD.ATTACK_MOVE, units, x, y }, queued),
  attack: (units, target, queued = false) =>
    withQueued({ c: CMD.ATTACK, units, target }, queued),
  setupAntiTankGuns: (units, x, y, queued = false) =>
    withQueued({ c: CMD.SETUP_ANTI_TANK_GUNS, units, x, y }, queued),
  tearDownAntiTankGuns: (units) => ({ c: CMD.TEAR_DOWN_ANTI_TANK_GUNS, units }),
  charge: (units) => ({ c: CMD.CHARGE, units }),
  useAbility: (ability, units, x = null, y = null, queued = false) => {
    const command = { c: CMD.USE_ABILITY, ability, units };
    if (x != null) command.x = x;
    if (y != null) command.y = y;
    return withQueued(command, queued);
  },
  recastAbility: (ability, units, targetObjectId = null, queued = false) => {
    const command = { c: CMD.RECAST_ABILITY, ability, units };
    if (targetObjectId != null) command.targetObjectId = targetObjectId;
    return withQueued(command, queued);
  },
  setAutocast: (ability, units, enabled) => ({ c: CMD.SET_AUTOCAST, ability, units, enabled }),
  pointFire: (units, x, y, queued = false) =>
    withQueued({ c: CMD.USE_ABILITY, ability: ABILITY.POINT_FIRE, units, x, y }, queued),
  gather: (units, node, queued = false) =>
    withQueued({ c: CMD.GATHER, units, node }, queued),
  build: (units, building, tileX, tileY, queued = false) =>
    withQueued({ c: CMD.BUILD, units, building, tileX, tileY }, queued),
  train: (building, unit) => ({ c: CMD.TRAIN, building, unit }),
  research: (building, upgrade) => ({ c: CMD.RESEARCH, building, upgrade }),
  cancel: (building) => ({ c: CMD.CANCEL, building }),
  stop: (units) => ({ c: CMD.STOP, units }),
  holdPosition: (units) => ({ c: CMD.HOLD_POSITION, units }),
  setRally: (building, x, y, queued = false, kind = ORDER_STAGE.MOVE) =>
    withQueued({ c: CMD.SET_RALLY, building, x, y, kind }, queued),
});
