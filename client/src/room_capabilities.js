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

const EMPTY_DIAGNOSTICS = Object.freeze({
  movementPaths: MOVEMENT_PATH_DIAGNOSTICS.NONE,
  observerAnalysis: false,
});

const EMPTY_COMMANDS = Object.freeze({
  gameplay: false,
});

export function createRoomCapabilities({ startPayload, devWatch = null, replayViewer = false } = {}) {
  const diagnostics = normalizeDiagnostics(startPayload?.diagnostics);
  const roomTime = roomTimeCapabilities({ startPayload, devWatch });
  const visibility = {
    ...EMPTY_VISIBILITY,
    replayVision: !!startPayload?.replay,
  };
  const commands = {
    ...EMPTY_COMMANDS,
    gameplay: !replayViewer && startPayload?.spectator !== true,
  };
  return Object.freeze({
    roomTime,
    diagnostics,
    visibility: Object.freeze(visibility),
    commands: Object.freeze(commands),
  });
}

function roomTimeCapabilities({ startPayload, devWatch }) {
  if (startPayload?.replay) {
    return Object.freeze({
      ...EMPTY_ROOM_TIME,
      available: true,
      setSpeed: true,
      pause: true,
      seekRelative: true,
      seekAbsolute: true,
      timeline: true,
    });
  }
  if (devWatch?.kind === "scenario") {
    return Object.freeze({
      ...EMPTY_ROOM_TIME,
      available: true,
      setSpeed: true,
      pause: true,
      step: true,
    });
  }
  return EMPTY_ROOM_TIME;
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
