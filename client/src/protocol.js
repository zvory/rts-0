// Wire protocol — JavaScript mirror of `server/src/protocol.rs`. See DESIGN.md §2.
// Change both files together. Builders construct the exact JSON the server expects.

// --- Server -> Client message tags (the `t` field) ---
export const S = Object.freeze({
  WELCOME: "welcome",
  LOBBY: "lobby",
  START: "start",
  SNAPSHOT: "snapshot",
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
  COMMAND: "command",
  GIVE_UP: "giveUp",
  PING: "ping",
  SET_REPLAY_SPEED: "setReplaySpeed",
});

// --- Command discriminators (the `c` field) ---
export const CMD = Object.freeze({
  MOVE: "move",
  ATTACK_MOVE: "attackMove",
  ATTACK: "attack",
  GATHER: "gather",
  BUILD: "build",
  TRAIN: "train",
  CANCEL: "cancel",
  STOP: "stop",
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
  TANK: "tank",
  INDUSTRIAL_CENTER: "industrial_center",
  DEPOT: "depot",
  BARRACKS: "barracks",
  TRAINING_CENTRE: "training_centre",
  TANK_FACTORY: "tank_factory",
  STEEL: "steel",
  OIL: "oil",
});
export const UNIT_KINDS = Object.freeze([
  KIND.WORKER,
  KIND.RIFLEMAN,
  KIND.MACHINE_GUNNER,
  KIND.AT_TEAM,
  KIND.TANK,
]);
export const BUILDING_KINDS = Object.freeze([
  KIND.INDUSTRIAL_CENTER,
  KIND.DEPOT,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.TANK_FACTORY,
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
});

// --- Compact snapshot wire schema (must match protocol.rs) ---
export const COMPACT_SNAPSHOT_VERSION = 1;

export const KIND_CODE = Object.freeze({
  [KIND.WORKER]: 1,
  [KIND.RIFLEMAN]: 2,
  [KIND.MACHINE_GUNNER]: 3,
  [KIND.AT_TEAM]: 4,
  [KIND.TANK]: 5,
  [KIND.INDUSTRIAL_CENTER]: 6,
  [KIND.DEPOT]: 7,
  [KIND.BARRACKS]: 8,
  [KIND.TRAINING_CENTRE]: 9,
  [KIND.TANK_FACTORY]: 10,
  [KIND.STEEL]: 11,
  [KIND.OIL]: 12,
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
});

const KIND_BY_CODE = Object.freeze(reverseCodes(KIND_CODE));
const STATE_BY_CODE = Object.freeze(reverseCodes(STATE_CODE));
const SETUP_BY_CODE = Object.freeze(reverseCodes(SETUP_CODE));
const EVENT_BY_CODE = Object.freeze(reverseCodes(EVENT_CODE));

const MAX_COMPACT_ENTITIES = 20000;
const MAX_COMPACT_RESOURCE_DELTAS = 20000;
const MAX_COMPACT_EVENTS = 5000;

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
    events: readOptionalArray(raw.ev, "events", MAX_COMPACT_EVENTS).map(decodeCompactEvent),
  };
}

function decodeCompactEntity(record, index) {
  const fields = readArray(record, `entity ${index}`, 18);
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
  return entity;
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
      requireLength(fields, 3, `attack event ${index}`);
      return {
        e: EVENT.ATTACK,
        from: readU32(fields[1], "event.from"),
        to: readU32(fields[2], "event.to"),
      };
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
      requireLength(fields, 2, `notice event ${index}`);
      if (typeof fields[1] !== "string") throw new Error(`notice event ${index} msg must be string`);
      return { e: EVENT.NOTICE, msg: fields[1] };
    default:
      throw new Error(`unknown compact event kind ${eventKind}`);
  }
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
  join: (name, room = "main") => ({ t: C.JOIN, name, room }),
  ready: (ready) => ({ t: C.READY, ready: !!ready }),
  start: () => ({ t: C.START }),
  addAi: () => ({ t: C.ADD_AI }),
  removeAi: (id) => ({ t: C.REMOVE_AI, id }),
  setQuickstart: (enabled) => ({ t: C.SET_QUICKSTART, enabled: !!enabled }),
  command: (cmd) => ({ t: C.COMMAND, cmd }),
  giveUp: () => ({ t: C.GIVE_UP }),
  ping: (ts) => ({ t: C.PING, ts }),
  setReplaySpeed: (speed) => ({ t: C.SET_REPLAY_SPEED, speed }),
});

// --- Command builders (the `cmd` payload) ---
export const cmd = Object.freeze({
  move: (units, x, y) => ({ c: CMD.MOVE, units, x, y }),
  attackMove: (units, x, y) => ({ c: CMD.ATTACK_MOVE, units, x, y }),
  attack: (units, target) => ({ c: CMD.ATTACK, units, target }),
  gather: (units, node) => ({ c: CMD.GATHER, units, node }),
  build: (worker, building, tileX, tileY) => ({ c: CMD.BUILD, worker, building, tileX, tileY }),
  train: (building, unit) => ({ c: CMD.TRAIN, building, unit }),
  cancel: (building) => ({ c: CMD.CANCEL, building }),
  stop: (units) => ({ c: CMD.STOP, units }),
});
