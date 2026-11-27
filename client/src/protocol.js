// replayResourceHistory also carries players[] with id, lifetimeSteel/lifetimeOil and absolute steel/oil, aliveSteel/aliveOil, lostSteel/lostOil, unitSteel/unitOil values for up to four players.
// replayResourceHistory samples carry signed steel/oil trailing-five-second collection totals (client displays totals / 5 per second) and aliveSteel/aliveOil lifetime-minus-loss advantages, lostSteel/lostOil destroyed-value differences, and unitSteel/unitOil current-army-value differences.
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
  MAX_COMPACT_GROUND_DECALS,
  LAB_CHECKPOINT_SCENARIO,
  LAB_REPLAY,
  LAB_ROLE,
  LAB_VISION,
  LOBBY_KIND,
  MOVEMENT_PATH_DIAGNOSTICS,
  NOTICE,
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
  isDiscipleKind,
  daoForKind,
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
  MAX_COMPACT_GROUND_DECALS,
  LAB_CHECKPOINT_SCENARIO,
  LAB_REPLAY,
  LAB_ROLE,
  LAB_VISION,
  LOBBY_KIND,
  MOVEMENT_PATH_DIAGNOSTICS,
  NOTICE,
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
  isDiscipleKind,
  daoForKind,
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
 * Reliable welcome/pong messages carry buildId: string for the app version handshake.
 * Lobby messages carry countdownActive: boolean, including membership updates during countdown.
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

/** Validate the optional synchronized-resume portion of a livePauseState payload. */
export function liveResumeCountdownFromWire(raw) {
  if (!raw || typeof raw !== "object") return null;
  const durationMs = Math.floor(Number(raw.durationMs));
  const remainingMs = Math.floor(Number(raw.remainingMs));
  const setupDurationMs = Math.floor(Number(raw.setupDurationMs ?? 0));
  const words = Array.isArray(raw.words) ? raw.words.map((word) => String(word)) : [];
  if (
    !Number.isFinite(durationMs) ||
    !Number.isFinite(remainingMs) ||
    !Number.isFinite(setupDurationMs) ||
    setupDurationMs < 0 ||
    setupDurationMs >= durationMs ||
    durationMs <= 0 ||
    remainingMs <= 0 ||
    words.length === 0
  ) return null;
  return {
    durationMs,
    setupDurationMs,
    remainingMs: Math.min(remainingMs, durationMs),
    words,
  };
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
  setName: (name) => ({ t: C.SET_NAME, name }),
  setPreferredColor: (color) => ({ t: C.SET_PREFERRED_COLOR, color }),
  ready: (ready) => ({ t: C.READY, ready: !!ready }),
  matchLoadReady: (countdownId) => ({ t: C.MATCH_LOAD_READY, countdownId }),
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
  chatSend: (channel, text) => ({ t: C.CHAT_SEND, channel, text }),
  command: (cmd, clientSeq) => ({ t: C.COMMAND, clientSeq, cmd }),
  giveUp: () => ({ t: C.GIVE_UP }),
  pauseGame: () => ({ t: C.PAUSE_GAME }),
  unpauseGame: () => ({ t: C.UNPAUSE_GAME }),
  returnToLobby: () => ({ t: C.RETURN_TO_LOBBY }),
  ping: (ts) => ({ t: C.PING, ts }),
  // report.clientBuildId records the page-initialization server build observation; performanceSummary is the optional
  // fixed-shape 60-second aggregate documented in docs/design/protocol.md.
  netReport: (report) => ({ t: C.NET_REPORT, report }),
  activity: () => ({ t: C.ACTIVITY }),
  setRoomTimeSpeed: (speed) => ({ t: C.SET_ROOM_TIME_SPEED, speed }),
  stepRoomTime: () => ({ t: C.STEP_ROOM_TIME }),
  seekRoomTime: (ticksBack) => ({ t: C.SEEK_ROOM_TIME, ticksBack }),
  seekRoomTimeTo: (tick) => ({ t: C.SEEK_ROOM_TIME_TO, tick }),
  setVisionSelection: (selection) => ({ t: C.SET_VISION_SELECTION, selection }),
  requestGroundDecals: (requestId, afterRevision = 0) => ({
    t: C.REQUEST_GROUND_DECALS,
    requestId,
    afterRevision,
  }),
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
  visionSelectionOmniscient: () => ({
    t: C.SET_VISION_SELECTION,
    selection: { mode: VISION_SELECTION.OMNISCIENT },
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
  formationMove: (units, points, queued = false, attackMove = false) =>
    withQueued({ c: CMD.FORMATION_MOVE, units, points, ...(attackMove ? { attackMove: true } : {}) }, queued),
  attackMove: (units, x, y, queued = false) =>
    withQueued({ c: CMD.ATTACK_MOVE, units, x, y }, queued),
  clearObstacleArea: (units, target, queued = false) =>
    withQueued({ c: CMD.CLEAR_OBSTACLE_AREA, units, target }, queued),
  attack: (units, target, queued = false, tankTrapCluster = false) =>
    withQueued({
      c: CMD.ATTACK,
      units,
      target,
      ...(tankTrapCluster ? { tankTrapCluster: true } : {}),
    }, queued),
  repair: (units, target, queued = false) =>
    withQueued({ c: CMD.REPAIR, units, target }, queued),
  deconstruct: (units, target, queued = false) =>
    withQueued({ c: CMD.DECONSTRUCT, units, target }, queued),
  setupAntiTankGuns: (units, x, y, queued = false) =>
    withQueued({ c: CMD.SETUP_ANTI_TANK_GUNS, units, x, y }, queued),
  pointTanks: (units, x, y, queued = false) =>
    withQueued({ c: CMD.POINT_TANKS, units, x, y }, queued),
  tearDownAntiTankGuns: (units) => ({ c: CMD.TEAR_DOWN_ANTI_TANK_GUNS, units }),
  charge: (units) => ({ c: CMD.CHARGE, units }),
  vectorField: (units, x, y, directionX, directionY) => ({c: "vectorField", units, x, y, directionX, directionY}),
  useAbility: (ability, units, x = null, y = null, queued = false, targetId = null) => {
    const command = { c: CMD.USE_ABILITY, ability, units };
    if (x != null) command.x = x;
    if (y != null) command.y = y;
    if (targetId != null) command.targetId = targetId;
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
  blanketFire: (units, x, y, radiusTiles, queued = false) =>
    withQueued({ c: CMD.ARTILLERY_FIRE, units, x, y, radiusTiles }, queued),
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
  setAutoBuildSettings: (paused, reserveSteel, reserveOil) => ({
    c: CMD.SET_AUTO_BUILD_SETTINGS,
    paused: !!paused,
    reserveSteel,
    reserveOil,
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

// MG prototype: Attack.shotOrigin (compact v57, slot 6) fixes the ray origin; toPos fixes its endpoint.

// Disciple EntityView slots 45–49: team-private chosenDao; public insight, dao, daoReady and daoAdvanceTicks.

// Tournament Grounds: kind code 32; compact v62 slot 50 is team-private tournamentEntrants (0–4).
// Existing production/rally fields carry its five-second batch and pending winners.

// Proving Grounds: kind code 33; v63 slot 51 is team-private daoTrainingQueue (<=64 Dao ids).
// research(building, upgrade) queues paid training at Proving Grounds; cancel removes unpaid tail plans.

// Lab spawnEntity/spawnEntities entries accept optional dao: wind | war | magic (Novice only) and daoMaster: bool (Adepts only).

// Snapshots carry public playerFactions: [{id, factionId}] independently of private playerResources.

// Shop goods use useAbility targetId; v66 entity slot 53 carries owner/ally-only escapeTreasure (plus privileged observers).

// Dao Master rank: EntityView daoMaster is a public bool at compact slot 52 (v65);
// rts-contract owns the DTO, the sim owns advancement, and exact insight is public on visible Disciples.

// Wind Dash uses windDash (ability 21, order stage 29); no return marker.

// Static map DTOs include unwalkableTiles: [{ x, y }], defaulting to [].
// This overlay preserves terrain art and blocks ground movement/landing/placement on the server.

// Compact v70 entity slot 54: public itemRadianceTier (0–6), fog-gated with its carrier.

// Compact v71: shield ownerState adds maxHp (slot 6) and durationTicks (slot 7), public with the visible field.

// Compact v76 entity slot 55: chargeActive is a public, fog-gated Charge visual flag.
