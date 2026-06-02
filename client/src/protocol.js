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
  PING: "ping",
  SET_REPLAY_SPEED: "setReplaySpeed",
  CLIENT_PERF: "clientPerf",
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

// --- Client -> Server builders ---
export const msg = Object.freeze({
  join: (name, room = "main") => ({ t: C.JOIN, name, room }),
  ready: (ready) => ({ t: C.READY, ready: !!ready }),
  start: () => ({ t: C.START }),
  addAi: () => ({ t: C.ADD_AI }),
  removeAi: (id) => ({ t: C.REMOVE_AI, id }),
  setQuickstart: (enabled) => ({ t: C.SET_QUICKSTART, enabled: !!enabled }),
  command: (cmd) => ({ t: C.COMMAND, cmd }),
  ping: (ts) => ({ t: C.PING, ts }),
  setReplaySpeed: (speed) => ({ t: C.SET_REPLAY_SPEED, speed }),
  clientPerf: (report) => ({ t: C.CLIENT_PERF, report }),
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
