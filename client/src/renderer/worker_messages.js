import { PRESENTATION_FRAME_VERSION, STATIC_MAP_PRESENTATION_VERSION } from "../presentation/frame.js";

export const RENDER_WORKER_MESSAGE_VERSION = 1;
export const RENDER_WORKER_MESSAGE = Object.freeze({
  INITIALIZE: "initialize",
  MAP_GENERATION: "mapGeneration",
  DURABLE_DECALS: "durableDecals",
  REVISIONED_GRIDS: "revisionedGrids",
  FRAME: "frame",
  RESIZE: "resize",
  CAPTURE: "capture",
  RESET_GENERATION: "resetGeneration",
  DESTROY: "destroy",
});
export const RENDER_WORKER_RESPONSE = Object.freeze({
  READY: "ready",
  RETAINED: "retained",
  PRESENTED: "presented",
  SUPERSEDED: "superseded",
  FAILED: "failed",
  DESTROYED: "destroyed",
});

const REQUEST_TYPES = new Set(Object.values(RENDER_WORKER_MESSAGE));
const RESPONSE_TYPES = new Set(Object.values(RENDER_WORKER_RESPONSE));

export function createRenderWorkerWireState() {
  return { generation: 0, visibleRevision: -1, exploredRevision: -1 };
}

export function createInitializeMessage({ canvas, widthCssPx, heightCssPx, dpr, configuration = {} }) {
  return request(RENDER_WORKER_MESSAGE.INITIALIZE, 1, {
    presentationVersion: PRESENTATION_FRAME_VERSION,
    staticMapVersion: STATIC_MAP_PRESENTATION_VERSION,
    canvas,
    widthCssPx: positiveFinite(widthCssPx, "widthCssPx"),
    heightCssPx: positiveFinite(heightCssPx, "heightCssPx"),
    dpr: boundedDpr(dpr),
    configuration: clonePlain(configuration),
  }, [canvas]);
}

export function createMapGenerationMessage(staticMap) {
  requireGeneration(staticMap?.generation);
  const terrain = cloneGridValues(staticMap?.terrain, "terrain");
  return request(RENDER_WORKER_MESSAGE.MAP_GENERATION, staticMap.generation, {
    map: {
      version: staticMap.version,
      revision: requireId(staticMap.revision, "map revision", { allowZero: false }),
      widthPx: positiveFinite(staticMap.widthPx, "map widthPx"),
      heightPx: positiveFinite(staticMap.heightPx, "map heightPx"),
      tileSizePx: positiveFinite(staticMap.tileSizePx, "map tileSizePx"),
      terrain: gridRecord(staticMap.terrain, terrain),
      resourceSites: clonePlain(staticMap.resourceSites || []),
    },
  }, [terrain.buffer]);
}

export function createFrameMessages(frame, state = createRenderWorkerWireState()) {
  validatePresentationFrame(frame);
  if (state.generation !== frame.generation) {
    state.generation = frame.generation;
    state.visibleRevision = -1;
    state.exploredRevision = -1;
  }
  const messages = [];
  const gridPayload = {};
  const transfer = [];
  if (frame.visible.revision !== state.visibleRevision) {
    const values = cloneGridValues(frame.visible, "visible");
    gridPayload.visible = gridRecord(frame.visible, values);
    transfer.push(values.buffer);
    state.visibleRevision = frame.visible.revision;
  }
  if (frame.explored.revision !== state.exploredRevision) {
    const values = cloneGridValues(frame.explored, "explored");
    gridPayload.explored = gridRecord(frame.explored, values);
    transfer.push(values.buffer);
    state.exploredRevision = frame.explored.revision;
  }
  if (transfer.length) {
    messages.push(request(RENDER_WORKER_MESSAGE.REVISIONED_GRIDS, frame.generation, {
      revisions: gridPayload,
    }, transfer));
  }

  const decals = (frame.layers?.persistentGroundMark || [])
    .filter((record) => record?.type === "groundDecal")
    .map((record) => clonePlain(record));
  if (frame.groundDecalRevision > 0 && decals.length) {
    messages.push(request(RENDER_WORKER_MESSAGE.DURABLE_DECALS, frame.generation, {
      revision: requireId(frame.groundDecalRevision, "ground decal revision"),
      decals,
    }));
  }
  messages.push(request(RENDER_WORKER_MESSAGE.FRAME, frame.generation, {
    frame: cloneFrameWithoutGridValues(frame),
  }));
  return messages;
}

export function createDurableDecalMessage(frame) {
  validatePresentationFrame(frame);
  const decals = (frame.layers?.persistentGroundMark || [])
    .filter((record) => record?.type === "groundDecal")
    .map((record) => clonePlain(record));
  if (frame.groundDecalRevision <= 0 || decals.length === 0) return null;
  return request(RENDER_WORKER_MESSAGE.DURABLE_DECALS, frame.generation, {
    revision: requireId(frame.groundDecalRevision, "ground decal revision"),
    decals,
  });
}

export function createEditorFrameMessage(record, generation = 1) {
  if (record?.version !== 1 || !Number.isSafeInteger(record?.frameId) || record.frameId <= 0) {
    throw new TypeError("Map Editor worker frame requires a version-1 record and positive frame id.");
  }
  return request(RENDER_WORKER_MESSAGE.FRAME, generation, { editor: clonePlain(record) });
}

export function createResizeMessage({ generation, frameId = 0, widthCssPx, heightCssPx, dpr }) {
  return request(RENDER_WORKER_MESSAGE.RESIZE, generation, {
    frameId: requireId(frameId, "resize frame id"),
    widthCssPx: positiveFinite(widthCssPx, "widthCssPx"),
    heightCssPx: positiveFinite(heightCssPx, "heightCssPx"),
    dpr: boundedDpr(dpr),
  });
}

export function createCaptureMessage({ generation, frameId, captureId, readPixels = false }) {
  return request(RENDER_WORKER_MESSAGE.CAPTURE, generation, {
    frameId: requireId(frameId, "frame id", { allowZero: false }),
    captureId: requireId(captureId, "capture id", { allowZero: false }),
    readPixels: !!readPixels,
  });
}

export function createResetGenerationMessage(generation) {
  return request(RENDER_WORKER_MESSAGE.RESET_GENERATION, generation, {});
}

export function createDestroyMessage(generation) {
  return request(RENDER_WORKER_MESSAGE.DESTROY, generation, {});
}

export function validateRenderWorkerRequest(message, { requireCanvas = false } = {}) {
  message = validateEnvelope(message, REQUEST_TYPES);
  const payload = message.payload;
  switch (message.type) {
    case RENDER_WORKER_MESSAGE.INITIALIZE:
      requireVersion(payload?.presentationVersion, PRESENTATION_FRAME_VERSION, "presentation version");
      requireVersion(payload?.staticMapVersion, STATIC_MAP_PRESENTATION_VERSION, "static map version");
      positiveFinite(payload?.widthCssPx, "widthCssPx");
      positiveFinite(payload?.heightCssPx, "heightCssPx");
      boundedDpr(payload?.dpr);
      if (!["match", "mapEditor"].includes(payload?.configuration?.surface || "match")) {
        throw new TypeError("initialize surface must be match or mapEditor");
      }
      if (requireCanvas && !payload?.canvas) throw new TypeError("initialize requires a transferred canvas");
      break;
    case RENDER_WORKER_MESSAGE.MAP_GENERATION:
      validateGrid(payload?.map?.terrain, "terrain");
      requireId(payload?.map?.revision, "map revision", { allowZero: false });
      break;
    case RENDER_WORKER_MESSAGE.REVISIONED_GRIDS:
      if (!payload?.revisions?.visible && !payload?.revisions?.explored) {
        throw new TypeError("revisionedGrids requires at least one grid");
      }
      if (payload.revisions.visible) validateGrid(payload.revisions.visible, "visible");
      if (payload.revisions.explored) validateGrid(payload.revisions.explored, "explored");
      break;
    case RENDER_WORKER_MESSAGE.DURABLE_DECALS:
      requireId(payload?.revision, "ground decal revision");
      if (!Array.isArray(payload?.decals)) throw new TypeError("durableDecals requires records");
      break;
    case RENDER_WORKER_MESSAGE.FRAME:
      if (payload?.editor) {
        if (payload.editor.version !== 1 || !Number.isSafeInteger(payload.editor.frameId) || payload.editor.frameId <= 0) {
          throw new TypeError("invalid Map Editor worker frame");
        }
      } else {
        validatePresentationFrame(payload?.frame, { valuesOptional: true });
      }
      if (payload?.submittedAtMs != null) nonNegativeFinite(payload.submittedAtMs, "submittedAtMs");
      if (payload?.capturePixels != null && typeof payload.capturePixels !== "boolean") {
        throw new TypeError("capturePixels must be boolean");
      }
      break;
    case RENDER_WORKER_MESSAGE.RESIZE:
      positiveFinite(payload?.widthCssPx, "widthCssPx");
      positiveFinite(payload?.heightCssPx, "heightCssPx");
      boundedDpr(payload?.dpr);
      requireId(payload?.frameId ?? 0, "resize frame id");
      break;
    case RENDER_WORKER_MESSAGE.CAPTURE:
      requireId(payload?.frameId, "frame id", { allowZero: false });
      requireId(payload?.captureId, "capture id", { allowZero: false });
      if (payload?.readPixels != null && typeof payload.readPixels !== "boolean") {
        throw new TypeError("readPixels must be boolean");
      }
      break;
    default:
      break;
  }
  return message;
}

export function validateRenderWorkerResponse(message) {
  message = validateEnvelope(message, RESPONSE_TYPES);
  const payload = message.payload;
  if ([RENDER_WORKER_RESPONSE.PRESENTED, RENDER_WORKER_RESPONSE.SUPERSEDED].includes(message.type)) {
    requireId(payload?.frameId, "frame id", { allowZero: false });
  }
  if (message.type === RENDER_WORKER_RESPONSE.PRESENTED) {
    nonNegativeFinite(payload?.workerUpdateMs, "workerUpdateMs");
    nonNegativeFinite(payload?.workerPresentMs, "workerPresentMs");
    nonNegativeFinite(payload?.queueAgeMs ?? 0, "queueAgeMs");
    nonNegativeFinite(payload?.displayAgeMs ?? 0, "displayAgeMs");
    if (payload?.rgba != null && !(payload.rgba instanceof ArrayBuffer)) {
      throw new TypeError("presented capture rgba must be an ArrayBuffer");
    }
    if (payload?.rgba instanceof ArrayBuffer) {
      requireId(payload?.width, "capture width", { allowZero: false });
      requireId(payload?.height, "capture height", { allowZero: false });
      if (payload.rgba.byteLength !== payload.width * payload.height * 4) {
        throw new TypeError("presented capture rgba must match its dimensions");
      }
    }
  }
  if (message.type === RENDER_WORKER_RESPONSE.RETAINED) {
    requireId(payload?.revision, "ground decal revision");
  }
  if (message.type === RENDER_WORKER_RESPONSE.FAILED) {
    if (typeof payload?.code !== "string" || !payload.code || payload.code.length > 80) {
      throw new TypeError("failed response requires a bounded code");
    }
    if (typeof payload?.message !== "string" || payload.message.length > 500) {
      throw new TypeError("failed response message exceeds its bound");
    }
  }
  return message;
}

function request(type, generation, payload, transfer = []) {
  const message = { version: RENDER_WORKER_MESSAGE_VERSION, type, generation: requireGeneration(generation), payload };
  validateRenderWorkerRequest(message);
  return Object.freeze({ message: Object.freeze(message), transfer: Object.freeze(transfer) });
}

function validateEnvelope(candidate, types) {
  const message = candidate?.message || candidate;
  requireVersion(message?.version, RENDER_WORKER_MESSAGE_VERSION, "worker message version");
  if (!types.has(message?.type)) throw new TypeError(`unknown render-worker message type ${String(message?.type)}`);
  requireGeneration(message?.generation);
  if (!message.payload || typeof message.payload !== "object" || Array.isArray(message.payload)) {
    throw new TypeError("render-worker message requires a payload object");
  }
  return message;
}

function validatePresentationFrame(frame, { valuesOptional = false } = {}) {
  requireVersion(frame?.version, PRESENTATION_FRAME_VERSION, "presentation frame version");
  requireGeneration(frame?.generation);
  requireId(frame?.frameId, "frame id", { allowZero: false });
  requireId(frame?.groundDecalRevision, "ground decal revision");
  nonNegativeFinite(frame?.visualTimeMs, "visualTimeMs");
  requireVersion(frame?.projection?.version, 2, "renderer projection version");
  requireId(frame?.staticMapRevision, "static map revision", { allowZero: false });
  validateGrid(frame?.visible, "visible", valuesOptional);
  validateGrid(frame?.explored, "explored", valuesOptional);
  if (!frame?.layers || typeof frame.layers !== "object") throw new TypeError("presentation frame requires layers");
  return frame;
}

function validateGrid(grid, label, valuesOptional = false) {
  requireVersion(grid?.version, 2, `${label} grid version`);
  requireId(grid?.revision, `${label} revision`);
  requireId(grid?.width, `${label} width`);
  requireId(grid?.height, `${label} height`);
  if (!valuesOptional || grid.values != null) {
    if (!(grid?.values instanceof Uint8Array) || grid.values.length !== grid.width * grid.height) {
      throw new TypeError(`${label} grid requires a shape-matched Uint8Array`);
    }
  }
}

function cloneFrameWithoutGridValues(frame) {
  const clone = structuredClone(frame);
  clone.visible = { ...clone.visible, values: null };
  clone.explored = { ...clone.explored, values: null };
  clone.layers.persistentGroundMark = clone.layers.persistentGroundMark
    .filter((record) => record?.type !== "groundDecal");
  return clone;
}

function gridRecord(grid, values) {
  return { version: grid.version, revision: grid.revision, width: grid.width, height: grid.height, values };
}

function cloneGridValues(grid, label) {
  validateGrid(grid, label);
  return new Uint8Array(grid.values);
}

function clonePlain(value) {
  const clone = structuredClone(value);
  assertPlainGraph(clone, new Set());
  return clone;
}

function assertPlainGraph(value, seen) {
  if (value == null || ["string", "boolean", "number"].includes(typeof value)) return;
  if (typeof value !== "object" || seen.has(value)) throw new TypeError("worker payload must be finite acyclic data");
  if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer) return;
  const prototype = Object.getPrototypeOf(value);
  if (!Array.isArray(value) && prototype !== Object.prototype && prototype !== null) {
    throw new TypeError("worker payload contains a class instance");
  }
  seen.add(value);
  for (const entry of Object.values(value)) assertPlainGraph(entry, seen);
  seen.delete(value);
}

function requireVersion(actual, expected, label) {
  if (actual !== expected) throw new RangeError(`${label} must equal ${expected}`);
  return actual;
}

function requireGeneration(value) {
  return requireId(value, "generation", { allowZero: false });
}

function requireId(value, label, { allowZero = true } = {}) {
  if (!Number.isSafeInteger(value) || value < (allowZero ? 0 : 1)) {
    throw new RangeError(`${label} must be a bounded non-negative integer`);
  }
  return value;
}

function nonNegativeFinite(value, label) {
  if (!Number.isFinite(value) || value < 0) throw new RangeError(`${label} must be finite and non-negative`);
  return value;
}

function positiveFinite(value, label) {
  if (!Number.isFinite(value) || value <= 0 || value > 1_000_000) {
    throw new RangeError(`${label} must be finite, positive, and bounded`);
  }
  return value;
}

function boundedDpr(value) {
  if (!Number.isFinite(value) || value <= 0 || value > 8) throw new RangeError("dpr must be in (0, 8]");
  return value;
}
