import { assert } from "./assertions.mjs";
import { Camera } from "../../client/src/camera.js";
import { PresentationFrameAssembler } from "../../client/src/presentation/frame.js";
import {
  createCaptureMessage,
  createDurableDecalMessage,
  createDestroyMessage,
  createEditorFrameMessage,
  createFrameMessages,
  createInitializeMessage,
  createMapGenerationMessage,
  createRenderWorkerWireState,
  createResetGenerationMessage,
  createResizeMessage,
  RENDER_WORKER_MESSAGE,
  RENDER_WORKER_MESSAGE_VERSION,
  RENDER_WORKER_RESPONSE,
  validateRenderWorkerRequest,
  validateRenderWorkerResponse,
} from "../../client/src/renderer/worker_messages.js";

const map = { width: 2, height: 2, tileSize: 32, terrain: [0, 1, 2, 3], resources: [] };
const camera = new Camera(640, 480);
camera.setBounds(64, 64, 640, 480);
const assembler = new PresentationFrameAssembler({ map });
const representative = assembler.assemble({
  map,
  frameContext: { alpha: 0.5, interpolatedEntities: [
    { id: 1, kind: "rifleman", owner: 1, x: 16, y: 16 },
    { id: 2, kind: "barracks", owner: 2, x: 48, y: 48, visionOnly: true },
    { id: 3, kind: "rifleman", owner: 2, x: 32, y: 32, shotReveal: true },
  ] },
  projection: camera.projectionSnapshot(),
  fog: {
    visibleGrid: new Uint8Array([1, 0, 0, 0]),
    exploredGrid: new Uint8Array([1, 1, 0, 0]),
    visibleRevision: 4,
    exploredRevision: 7,
  },
  rememberedBuildings: [{ id: 4, kind: "barracks", owner: 2, x: 48, y: 16 }],
  trenches: [{ id: 5, x: 24, y: 24, radius: 12 }],
  groundDecals: [{ id: 6, kind: "rifleman", x: 16, y: 16, seed: 8 }],
  groundDecalRevision: 9,
  visualSamples: [{ id: "lab", kind: "trench", x: 20, y: 20 }],
  observerMapAnalysis: { regions: [{ id: "observer", x: 2, y: 3 }] },
  feedback: { mortarImpacts: [{ x: 24, y: 24 }], commandFeedback: [{ kind: "move", x: 30, y: 30 }] },
  mode: "fixedCapture",
  sourceTick: 12,
  visualTimeMs: 500,
});

assert(representative.version === 2, "representative live/replay/Lab/observer/effect frame uses PresentationFrameV2");
assert(structuredClone(representative).layers.aboveFogReveal.length === 1,
  "representative presentation frame is structurally cloneable without losing visibility layers");

const canvas = { transferMarker: true };
const init = createInitializeMessage({ canvas, widthCssPx: 640, heightCssPx: 480, dpr: 2, configuration: { nearest: true } });
assert(init.message.version === RENDER_WORKER_MESSAGE_VERSION && init.message.type === RENDER_WORKER_MESSAGE.INITIALIZE,
  "initialization carries message and presentation versions");
assert(init.transfer.length === 1 && init.transfer[0] === canvas,
  "initialization transfers the sole visible canvas instead of cloning or constructing a hidden renderer");
validateRenderWorkerRequest(init, { requireCanvas: true });

const mapMessage = createMapGenerationMessage(assembler.staticMap);
assert(mapMessage.transfer.length === 1 && mapMessage.message.payload.map.terrain.values !== assembler.staticMap.terrain.values,
  "map-generation terrain owns a detached transferable copy");
const mapClone = structuredClone(mapMessage.message, { transfer: [...mapMessage.transfer] });
assert(mapMessage.transfer[0].byteLength === 0 && mapClone.payload.map.terrain.values.length === 4,
  "map-generation transferable moves without detaching the assembler static map");
assert(assembler.staticMap.terrain.values.length === 4, "map serialization never mutates its source snapshot");

const state = createRenderWorkerWireState();
const firstMessages = createFrameMessages(representative, state);
assert(firstMessages.map((entry) => entry.message.type).join(",") === "revisionedGrids,durableDecals,frame",
  "first frame separates revisioned grids, durable decals, and dynamic frame lifetimes");
const gridMessage = firstMessages[0];
assert(gridMessage.transfer.length === 2, "changed visible and explored grids use transferable buffer copies");
const frameMessage = firstMessages.at(-1).message;
assert(frameMessage.payload.frame.visible.values === null && frameMessage.payload.frame.explored.values === null,
  "dynamic frame references grid revisions without cloning large grid values again");
assert(!frameMessage.payload.frame.layers.persistentGroundMark.some((record) => record.type === "groundDecal"),
  "dynamic frame excludes decals carried by the durable-update lifetime");
for (const entry of firstMessages) validateRenderWorkerRequest(entry);

const repeat = createFrameMessages(representative, state);
assert(repeat.map((entry) => entry.message.type).join(",") === "durableDecals,frame",
  "unchanged large grids are omitted while an unacknowledged durable revision remains explicit");
assert(createDurableDecalMessage(representative).message.payload.revision === 9,
  "durable decal retention can be sent independently of a supersedable dynamic frame");
const editor = createEditorFrameMessage({ version: 1, frameId: 3, terrainUpdate: null, overlay: {} }, 2);
assert(editor.message.type === RENDER_WORKER_MESSAGE.FRAME && editor.message.payload.editor.frameId === 3,
  "Map Editor records use the same worker frame route and remain detached cloneable data");

for (const control of [
  createResizeMessage({ generation: 1, frameId: 1, widthCssPx: 800, heightCssPx: 600, dpr: 2 }),
  createCaptureMessage({ generation: 1, frameId: 1, captureId: 4, readPixels: true }),
  createResetGenerationMessage(2),
  createDestroyMessage(2),
]) validateRenderWorkerRequest(control);

for (const response of [
  { version: 1, type: RENDER_WORKER_RESPONSE.READY, generation: 1, payload: { assets: { ready: true } } },
  { version: 1, type: RENDER_WORKER_RESPONSE.RETAINED, generation: 1, payload: { revision: 9 } },
  { version: 1, type: RENDER_WORKER_RESPONSE.PRESENTED, generation: 1, payload: { frameId: 1, workerUpdateMs: 2, workerPresentMs: 1 } },
  { version: 1, type: RENDER_WORKER_RESPONSE.PRESENTED, generation: 1, payload: {
    frameId: 1, captureId: 4, workerUpdateMs: 0, workerPresentMs: 0, rgba: new ArrayBuffer(4), width: 1, height: 1,
  } },
  { version: 1, type: RENDER_WORKER_RESPONSE.SUPERSEDED, generation: 1, payload: { frameId: 1 } },
  { version: 1, type: RENDER_WORKER_RESPONSE.FAILED, generation: 1, payload: { code: "asset", message: "atlas failed" } },
  { version: 1, type: RENDER_WORKER_RESPONSE.DESTROYED, generation: 1, payload: {} },
]) validateRenderWorkerResponse(response);

assertThrows(() => validateRenderWorkerRequest({ version: 99, type: "frame", generation: 1, payload: {} }),
  "wire rejects the wrong message version");
assertThrows(() => createResizeMessage({ generation: 1, widthCssPx: Number.NaN, heightCssPx: 2, dpr: 1 }),
  "wire rejects non-finite control values");
assertThrows(() => createCaptureMessage({ generation: 1, frameId: 0, captureId: 1 }),
  "wire rejects malformed frame ids");
assertThrows(() => validateRenderWorkerResponse({
  version: 1, type: "failed", generation: 1, payload: { code: "x".repeat(81), message: "bad" },
}), "wire bounds worker failures");
assertThrows(() => validateRenderWorkerResponse({
  version: 1, type: "presented", generation: 1, payload: {
    frameId: 1, workerUpdateMs: 0, workerPresentMs: 0, rgba: new ArrayBuffer(3), width: 1, height: 1,
  },
}), "wire rejects framebuffer captures whose decoded RGBA length does not match their dimensions");

function assertThrows(fn, message) {
  let threw = false;
  try { fn(); } catch { threw = true; }
  assert(threw, message);
}
