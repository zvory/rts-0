import { MOVEMENT_PATH_DIAGNOSTICS } from "./protocol.js";

const EMPTY_ROOM_TIME = Object.freeze({
  available: false,
  setSpeed: false,
  pause: false,
  step: false,
  seekRelative: false,
  seekAbsolute: false,
  timeline: false,
});

const EMPTY_VISIBILITY = Object.freeze({
  replayVision: false,
});

const EMPTY_MATCH_CONTROLS = Object.freeze({
  pause: false,
});

const EMPTY_DIAGNOSTICS = Object.freeze({
  movementPaths: MOVEMENT_PATH_DIAGNOSTICS.NONE,
  observerAnalysis: false,
});

const EMPTY_COMMANDS = Object.freeze({
  gameplay: false,
});

export function createRoomCapabilities({ startPayload } = {}) {
  const source = startPayload?.capabilities || {};
  const diagnostics = normalizeDiagnostics(startPayload?.diagnostics);
  const roomTime = roomTimeCapabilities(source.roomTime);
  const matchControls = normalizeMatchControls(source.matchControls);
  const visibility = normalizeVisibility(source.visibility);
  const commands = normalizeCommands(source.commands);
  return Object.freeze({
    roomTime,
    matchControls: Object.freeze(matchControls),
    diagnostics,
    visibility: Object.freeze(visibility),
    commands: Object.freeze(commands),
  });
}

function roomTimeCapabilities(roomTime) {
  if (!roomTime?.available) return EMPTY_ROOM_TIME;
  return Object.freeze({
    ...EMPTY_ROOM_TIME,
    available: true,
    setSpeed: roomTime.setSpeed === true,
    pause: roomTime.pause === true,
    step: roomTime.step === true,
    seekRelative: roomTime.seekRelative === true,
    seekAbsolute: roomTime.seekAbsolute === true,
    timeline: roomTime.timeline === true,
  });
}

function normalizeMatchControls(matchControls) {
  return Object.freeze({
    ...EMPTY_MATCH_CONTROLS,
    pause: matchControls?.pause === true,
  });
}

function normalizeVisibility(visibility) {
  return Object.freeze({
    ...EMPTY_VISIBILITY,
    replayVision: visibility?.replayVision === true,
  });
}

function normalizeCommands(commands) {
  return Object.freeze({
    ...EMPTY_COMMANDS,
    gameplay: commands?.gameplay === true,
  });
}

function normalizeDiagnostics(diagnostics) {
  const movementPaths = Object.values(MOVEMENT_PATH_DIAGNOSTICS).includes(diagnostics?.movementPaths)
    ? diagnostics.movementPaths
    : MOVEMENT_PATH_DIAGNOSTICS.NONE;
  return Object.freeze({
    ...EMPTY_DIAGNOSTICS,
    movementPaths,
    observerAnalysis: diagnostics?.observerAnalysis === true,
  });
}
