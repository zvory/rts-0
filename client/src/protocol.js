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
  COMMAND: "command",
  PING: "ping",
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
  SOLDIER: "soldier",
  HEAVY: "heavy",
  HQ: "hq",
  DEPOT: "depot",
  BARRACKS: "barracks",
  TURRET: "turret",
  MINERALS: "minerals",
  GAS: "gas",
});
export const UNIT_KINDS = Object.freeze([KIND.WORKER, KIND.SOLDIER, KIND.HEAVY]);
export const BUILDING_KINDS = Object.freeze([KIND.HQ, KIND.DEPOT, KIND.BARRACKS, KIND.TURRET]);
export const RESOURCE_KINDS = Object.freeze([KIND.MINERALS, KIND.GAS]);

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

// --- Event discriminators (the `e` field) ---
export const EVENT = Object.freeze({
  ATTACK: "attack",
  DEATH: "death",
  BUILD: "build",
  NOTICE: "notice",
});

// --- Client -> Server builders ---
export const msg = Object.freeze({
  join: (name, room = "main") => ({ t: C.JOIN, name, room }),
  ready: (ready) => ({ t: C.READY, ready: !!ready }),
  start: () => ({ t: C.START }),
  command: (cmd) => ({ t: C.COMMAND, cmd }),
  ping: (ts) => ({ t: C.PING, ts }),
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
