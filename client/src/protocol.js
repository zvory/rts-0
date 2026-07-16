// Wire protocol — JavaScript mirror of `server/crates/protocol/src/lib.rs`.
// See docs/design/protocol.md.
// Change both files together. Builders construct the exact JSON the server expects.

import { parseProtocolFrame } from "./protocol_frame.js";
import { decodeCompactSnapshot } from "./protocol_snapshot.js";
import {
  ABILITY,
  ABILITY_CODE,
  ABILITY_OBJECT_KIND,
  ABILITY_OBJECT_KIND_CODE,
  BUILDING_KINDS,
  C,
  CMD,
  COMPACT_SNAPSHOT_VERSION,
  DEFAULT_FACTION_ID,
  EVENT,
  EVENT_CODE,
  KIND,
  KIND_CODE,
  LAB_CHECKPOINT_SCENARIO,
  LAB_REPLAY,
  LAB_ROLE,
  LAB_VISION,
  LOBBY_KIND,
  MOVEMENT_PATH_DIAGNOSTICS,
  NOTICE_SEVERITY,
  NOTICE_SEVERITY_CODE,
  ORDER_STAGE,
  ORDER_STAGE_CODE,
  PASSABLE,
  ROAD_TERRAIN_CODES,
  PREDICTION_PROTOCOL_VERSION,
  VISION_SELECTION,
  WEAPON_KIND,
  WEAPON_KIND_CODE,
  RESOURCE_KINDS,
  S,
  SETUP,
  SETUP_CODE,
  SNAPSHOT_CODEC,
  SNAPSHOT_CODEC_VERSION,
  SNAPSHOT_FRAME_KIND,
  STATE,
  STATE_CODE,
  TERRAIN,
  UNIT_KINDS,
  UPGRADE,
  UPGRADE_CODE,
  isBuilding,
  isRoadTerrain,
  isResource,
  isUnit
} from "./protocol_constants.js";

export {
  ABILITY,
  ABILITY_CODE,
  ABILITY_OBJECT_KIND,
  ABILITY_OBJECT_KIND_CODE,
  BUILDING_KINDS,
  C,
  CMD,
  COMPACT_SNAPSHOT_VERSION,
  DEFAULT_FACTION_ID,
  EVENT,
  EVENT_CODE,
  KIND,
  KIND_CODE,
  LAB_CHECKPOINT_SCENARIO,
  LAB_REPLAY,
  LAB_ROLE,
  LAB_VISION,
  LOBBY_KIND,
  MOVEMENT_PATH_DIAGNOSTICS,
  NOTICE_SEVERITY,
  NOTICE_SEVERITY_CODE,
  ORDER_STAGE,
  ORDER_STAGE_CODE,
  PASSABLE,
  ROAD_TERRAIN_CODES,
  PREDICTION_PROTOCOL_VERSION,
  VISION_SELECTION,
  WEAPON_KIND,
  WEAPON_KIND_CODE,
  RESOURCE_KINDS,
  S,
  SETUP,
  SETUP_CODE,
  SNAPSHOT_CODEC,
  SNAPSHOT_CODEC_VERSION,
  SNAPSHOT_FRAME_KIND,
  STATE,
  STATE_CODE,
  TERRAIN,
  UNIT_KINDS,
  UPGRADE,
  UPGRADE_CODE,
  isBuilding,
  isRoadTerrain,
  isResource,
  isUnit
} from "./protocol_constants.js";

/**
 * Parse one WebSocket server frame into a raw protocol message.
 * Reliable messages stay JSON text. Snapshot messages use a versioned
 * MessagePack compact binary frame.
 * @param {string|ArrayBuffer|ArrayBufferView} frame
 * @returns {object}
 */
export function parseServerFrame(frame) {
  return parseProtocolFrame(frame, {
    snapshotTag: S.SNAPSHOT,
    snapshotCodecVersion: SNAPSHOT_CODEC_VERSION,
  });
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
  setSpectator: (spectator, id = undefined) => {
    const payload = { t: C.SET_SPECTATOR, spectator: !!spectator };
    if (id != null) payload.id = id;
    return payload;
  },
  command: (cmd, clientSeq) => ({ t: C.COMMAND, clientSeq, cmd }),
  giveUp: () => ({ t: C.GIVE_UP }),
  pauseGame: () => ({ t: C.PAUSE_GAME }),
  unpauseGame: () => ({ t: C.UNPAUSE_GAME }),
  returnToLobby: () => ({ t: C.RETURN_TO_LOBBY }),
  ping: (ts) => ({ t: C.PING, ts }),
  netReport: (report) => ({ t: C.NET_REPORT, report }),
  setRoomTimeSpeed: (speed) => ({ t: C.SET_ROOM_TIME_SPEED, speed }),
  stepRoomTime: () => ({ t: C.STEP_ROOM_TIME }),
  seekRoomTime: (ticksBack) => ({ t: C.SEEK_ROOM_TIME, ticksBack }),
  seekRoomTimeTo: (tick) => ({ t: C.SEEK_ROOM_TIME_TO, tick }),
  setVisionSelection: (selection) => ({ t: C.SET_VISION_SELECTION, selection }),
  lab: (requestId, op) => ({ t: C.LAB, requestId, op }),
  // Compatibility/setup builders. Visible UI labels these as checkpoint setup actions; lab replay
  // save/open must not reuse the legacy scenario operation names.
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
  labValidateScenario: (requestId, metadata) => ({
    t: C.LAB,
    requestId,
    op: { op: "validateScenario", metadata },
  }),
  labSpawnEntity: (requestId, { owner, kind, x, y, completed = false }) => ({
    t: C.LAB,
    requestId,
    op: { op: "spawnEntity", owner, kind, x, y, completed: !!completed },
  }),
  labSpawnEntities: (requestId, spawns) => ({
    t: C.LAB,
    requestId,
    op: { op: "spawnEntities", spawns },
  }),
  labDeleteEntity: (requestId, entityId) => ({
    t: C.LAB,
    requestId,
    op: { op: "deleteEntity", entityId },
  }),
  labDeleteEntities: (requestId, entityIds) => ({
    t: C.LAB,
    requestId,
    op: { op: "deleteEntities", entityIds },
  }),
  labMoveEntity: (requestId, entityId, x, y) => ({
    t: C.LAB,
    requestId,
    op: { op: "moveEntity", entityId, x, y },
  }),
  labApplyUpdates: (requestId, updates) => ({
    t: C.LAB,
    requestId,
    op: { op: "applyUpdates", updates },
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
  labSetPlayerGodMode: (r, p, on) => ({t:C.LAB,requestId:r,op:{op:"setPlayerGodMode",playerId:p,enabled:!!on}}),
  labSetCompletedResearch: (requestId, playerId, upgrade, completed) => ({
    t: C.LAB,
    requestId,
    op: { op: "setCompletedResearch", playerId, upgrade, completed: !!completed },
  }),
  labSetVision: (requestId, vision) => ({ t: C.LAB, requestId, op: { op: "setVision", vision } }),
  labIssueCommandAs: (requestId, playerId, command, ignoreCommandLimits = false) => ({
    t: C.LAB,
    requestId,
    op: { op: "issueCommandAs", playerId, cmd: command, ignoreCommandLimits: !!ignoreCommandLimits },
  }),
  requestBranchFromTick: () => ({ t: C.REQUEST_BRANCH_FROM_TICK }),
  claimBranchSeat: (playerId) => ({ t: C.CLAIM_BRANCH_SEAT, playerId }),
  releaseBranchSeat: (playerId) => ({ t: C.RELEASE_BRANCH_SEAT, playerId }),
  startBranch: () => ({ t: C.START_BRANCH }),
  visionSelectionAll: () => ({ t: C.SET_VISION_SELECTION, selection: { mode: VISION_SELECTION.ALL } }),
  visionSelectionPlayer: (playerId) => ({
    t: C.SET_VISION_SELECTION,
    selection: { mode: VISION_SELECTION.PLAYER, playerId },
  }),
  visionSelectionPlayers: (playerIds) => ({
    t: C.SET_VISION_SELECTION,
    selection: { mode: VISION_SELECTION.PLAYERS, playerIds },
  }),
  labVisionAll: () => ({ mode: LAB_VISION.ALL }),
  labVisionTeam: (teamId) => ({ mode: LAB_VISION.TEAM, teamId }),
  // Lobby map catalog rows are {name, description, minPlayers, maxPlayers}.
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
  deconstruct: (units, target, queued = false) =>
    withQueued({ c: CMD.DECONSTRUCT, units, target }, queued),
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
  blanketFire: (units, x, y, queued = false) =>
    withQueued({ c: CMD.USE_ABILITY, ability: ABILITY.BLANKET_FIRE, units, x, y }, queued),
  gather: (units, node, queued = false) =>
    withQueued({ c: CMD.GATHER, units, node }, queued),
  build: (units, building, tileX, tileY, queued = false) =>
    withQueued({ c: CMD.BUILD, units, building, tileX, tileY }, queued),
  train: (building, unit) => ({ c: CMD.TRAIN, building, unit }),
  adjustProductionRepeat: (buildings, unit, delta) => ({
    c: CMD.ADJUST_PRODUCTION_REPEAT,
    buildings,
    unit,
    delta,
  }),
  research: (building, upgrade) => ({ c: CMD.RESEARCH, building, upgrade }),
  cancel: (building) => ({ c: CMD.CANCEL, building }),
  cancelConstruction: (building) => ({ c: CMD.CANCEL, building, construction: true }),
  stop: (units) => ({ c: CMD.STOP, units }),
  holdPosition: (units, queued = false) =>
    withQueued({ c: CMD.HOLD_POSITION, units }, queued),
  setRally: (building, x, y, queued = false, kind = ORDER_STAGE.MOVE) =>
    withQueued({ c: CMD.SET_RALLY, building, x, y, kind }, queued),
});
