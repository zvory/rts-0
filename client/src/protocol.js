// Wire protocol — JavaScript mirror of `server/src/protocol.rs`. See docs/design/protocol.md.
// Change both files together. Builders construct the exact JSON the server expects.

// --- Server -> Client message tags (the `t` field) ---
export const S = Object.freeze({
  WELCOME: "welcome",
  LOBBY: "lobby",
  START: "start",
  SNAPSHOT: "snapshot",
  REPLAY_STATE: "replayState",
  GAME_OVER: "gameOver",
  PONG: "pong",
  ERROR: "error",
});

// --- Client -> Server message tags ---
export const C = Object.freeze({
  JOIN: "join",
  READY: "ready",
  START: "start",
  ADD_AI: "addAi",
  REMOVE_AI: "removeAi",
  SET_QUICKSTART: "setQuickstart",
  SET_SPECTATOR: "setSpectator",
  COMMAND: "command",
  GIVE_UP: "giveUp",
  PING: "ping",
  SET_REPLAY_SPEED: "setReplaySpeed",
  SEEK_REPLAY: "seekReplay",
  SET_REPLAY_VISION: "setReplayVision",
  SELECT_MAP: "selectMap",
});

// --- Command discriminators (the `c` field) ---
export const CMD = Object.freeze({
  MOVE: "move",
  ATTACK_MOVE: "attackMove",
  ATTACK: "attack",
  SETUP_AT_GUNS: "setupAtGuns",
  TEAR_DOWN_AT_GUNS: "tearDownAtGuns",
  CHARGE: "charge",
  USE_ABILITY: "useAbility",
  GATHER: "gather",
  BUILD: "build",
  TRAIN: "train",
  CANCEL: "cancel",
  STOP: "stop",
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
  AT_TEAM: "at_team",
  SCOUT_CAR: "scout_car",
  TANK: "tank",
  CITY_CENTRE: "city_centre",
  DEPOT: "depot",
  BARRACKS: "barracks",
  TRAINING_CENTRE: "training_centre",
  FACTORY: "factory",
  STEELWORKS: "steelworks",
  STEEL: "steel",
  OIL: "oil",
});
export const UNIT_KINDS = Object.freeze([
  KIND.WORKER,
  KIND.RIFLEMAN,
  KIND.MACHINE_GUNNER,
  KIND.AT_TEAM,
  KIND.SCOUT_CAR,
  KIND.TANK,
]);
export const BUILDING_KINDS = Object.freeze([
  KIND.CITY_CENTRE,
  KIND.DEPOT,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.FACTORY,
  KIND.STEELWORKS,
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
});

export const NOTICE_SEVERITY = Object.freeze({
  INFO: "info",
  WARN: "warn",
  ALERT: "alert",
});

export const ABILITY = Object.freeze({
  CHARGE: "charge",
  SMOKE: "smoke",
});

export const REPLAY_VISION = Object.freeze({
  ALL: "all",
  PLAYER: "player",
  PLAYERS: "players",
});

// --- Compact snapshot wire schema (must match protocol.rs) ---
export const COMPACT_SNAPSHOT_VERSION = 9;

export const KIND_CODE = Object.freeze({
  [KIND.WORKER]: 1,
  [KIND.RIFLEMAN]: 2,
  [KIND.MACHINE_GUNNER]: 3,
  [KIND.AT_TEAM]: 4,
  [KIND.TANK]: 5,
  [KIND.SCOUT_CAR]: 14,
  [KIND.CITY_CENTRE]: 6,
  [KIND.DEPOT]: 7,
  [KIND.BARRACKS]: 8,
  [KIND.TRAINING_CENTRE]: 9,
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

export const EVENT_CODE = Object.freeze({
  [EVENT.ATTACK]: 1,
  [EVENT.DEATH]: 2,
  [EVENT.BUILD]: 3,
  [EVENT.NOTICE]: 4,
  [EVENT.SMOKE_LAUNCH]: 5,
});

export const ORDER_STAGE = Object.freeze({
  MOVE: "move",
  ATTACK_MOVE: "attackMove",
  ATTACK: "attack",
  GATHER: "gather",
  BUILD: "build",
  CHARGE: "charge",
  SMOKE: "smoke",
  SETUP_AT_GUNS: "setupAtGuns",
});

export const ORDER_STAGE_CODE = Object.freeze({
  [ORDER_STAGE.MOVE]: 1,
  [ORDER_STAGE.ATTACK_MOVE]: 2,
  [ORDER_STAGE.ATTACK]: 3,
  [ORDER_STAGE.GATHER]: 4,
  [ORDER_STAGE.BUILD]: 5,
  [ORDER_STAGE.SMOKE]: 6,
  [ORDER_STAGE.SETUP_AT_GUNS]: 7,
  [ORDER_STAGE.CHARGE]: 8,
});

export const ABILITY_CODE = Object.freeze({
  [ABILITY.CHARGE]: 1,
  [ABILITY.SMOKE]: 2,
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
const NOTICE_SEVERITY_BY_CODE = Object.freeze(reverseCodes(NOTICE_SEVERITY_CODE));

const MAX_COMPACT_ENTITIES = 20000;
const MAX_COMPACT_RESOURCE_DELTAS = 20000;
const MAX_COMPACT_SMOKES = 1024;
const MAX_COMPACT_EVENTS = 5000;
const MAX_COMPACT_ORDER_PLAN = 9;
const MAX_COMPACT_ABILITIES = 8;
const MAX_COMPACT_DEBUG_WAYPOINTS = 128;
const MAX_COMPACT_VISIBLE_TILES = 65536;

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
    visibleTiles: decodeVisibilityRuns(raw.fg),
    events: readOptionalArray(raw.ev, "events", MAX_COMPACT_EVENTS).map(decodeCompactEvent),
    playerResources: readOptionalArray(raw.pr, "playerResources", 32).map(
      decodeCompactPlayerResource,
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

function decodeCompactNetStatus(record) {
  const fields = readArray(record, "netStatus", 5);
  if (fields.length !== 5) throw new Error("netStatus field count mismatch");
  const flags = readU32(fields[2], "netStatus.flags");
  return {
    serverLagMs: readU32(fields[0], "netStatus.serverLagMs"),
    tickMs: readU32(fields[1], "netStatus.tickMs"),
    slowTick: !!(flags & 1),
    slowTickCount: readU32(fields[3], "netStatus.slowTickCount"),
    headOfLine: !!(flags & 2),
    headOfLineCount: readU32(fields[4], "netStatus.headOfLineCount"),
  };
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
  const fields = readArray(record, `entity ${index}`, 26);
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
  assignOptional(entity, "visionOnly", fields, 24, readBool);
  assignDebugPath(entity, fields, 25);
  assignRallyPlan(entity, fields, 26);
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
  const fields = readArray(record, label, 3);
  if (fields.length !== 2 && fields.length !== 3) throw new Error(`${label} field count mismatch`);
  const ability = {
    ability: readCode(fields[0], ABILITY_BY_CODE, `${label}.ability`),
    cooldownLeft: readU32(fields[1], `${label}.cooldownLeft`),
  };
  if (fields.length > 2 && fields[2] != null) {
    ability.remainingUses = readU32(fields[2], `${label}.remainingUses`);
  }
  return ability;
}

/** Decode lobby-debug-mode owner-only path diagnostics. */
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
  const fields = readArray(record, `event ${index}`, 5);
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
  join: (name, room = "main", spectator = false) => ({
    t: C.JOIN,
    name,
    room,
    spectator: !!spectator,
  }),
  ready: (ready) => ({ t: C.READY, ready: !!ready }),
  start: () => ({ t: C.START }),
  addAi: () => ({ t: C.ADD_AI }),
  removeAi: (id) => ({ t: C.REMOVE_AI, id }),
  setQuickstart: (enabled) => ({ t: C.SET_QUICKSTART, enabled: !!enabled }),
  setSpectator: (spectator) => ({ t: C.SET_SPECTATOR, spectator: !!spectator }),
  command: (cmd) => ({ t: C.COMMAND, cmd }),
  giveUp: () => ({ t: C.GIVE_UP }),
  ping: (ts) => ({ t: C.PING, ts }),
  setReplaySpeed: (speed) => ({ t: C.SET_REPLAY_SPEED, speed }),
  seekReplay: (ticksBack) => ({ t: C.SEEK_REPLAY, ticksBack }),
  setReplayVision: (vision) => ({ t: C.SET_REPLAY_VISION, vision }),
  replayVisionAll: () => ({ t: C.SET_REPLAY_VISION, vision: { mode: REPLAY_VISION.ALL } }),
  replayVisionPlayer: (playerId) => ({
    t: C.SET_REPLAY_VISION,
    vision: { mode: REPLAY_VISION.PLAYER, playerId },
  }),
  replayVisionPlayers: (playerIds) => ({
    t: C.SET_REPLAY_VISION,
    vision: { mode: REPLAY_VISION.PLAYERS, playerIds },
  }),
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
  setupAtGuns: (units, x, y, queued = false) =>
    withQueued({ c: CMD.SETUP_AT_GUNS, units, x, y }, queued),
  tearDownAtGuns: (units) => ({ c: CMD.TEAR_DOWN_AT_GUNS, units }),
  charge: (units) => ({ c: CMD.CHARGE, units }),
  useAbility: (ability, units, x = null, y = null, queued = false) => {
    const command = { c: CMD.USE_ABILITY, ability, units };
    if (x != null) command.x = x;
    if (y != null) command.y = y;
    return withQueued(command, queued);
  },
  gather: (units, node, queued = false) =>
    withQueued({ c: CMD.GATHER, units, node }, queued),
  build: (worker, building, tileX, tileY, queued = false) =>
    withQueued({ c: CMD.BUILD, worker, building, tileX, tileY }, queued),
  train: (building, unit) => ({ c: CMD.TRAIN, building, unit }),
  cancel: (building) => ({ c: CMD.CANCEL, building }),
  stop: (units) => ({ c: CMD.STOP, units }),
  setRally: (building, x, y, queued = false, kind = ORDER_STAGE.MOVE) =>
    withQueued({ c: CMD.SET_RALLY, building, x, y, kind }, queued),
});
