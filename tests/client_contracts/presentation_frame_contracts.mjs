// Renderer-neutral presentation-frame contracts imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { runMatchCaptureFrame } from "../../client/src/frame_recovery.js";
import { renderFixedCaptureFrame } from "../../client/src/match_fixed_capture.js";
import {
  PresentationFrameAssembler,
  detachedRecord,
} from "../../client/src/presentation/frame.js";
import {
  copyGridSnapshotInto,
  createGridSnapshot,
  gridSnapshotValue,
} from "../../client/src/presentation/grid_snapshot.js";
import { prepareEntitySnapshots } from "../../client/src/presentation/entity_snapshot.js";
import { PRESENTATION_OUTCOME, immediatePresentationSubmission } from "../../client/src/presentation/submission.js";
import {
  PRESENTATION_LAYER_DESCRIPTORS,
  PRESENTATION_LAYER_IDS,
} from "../../client/src/presentation/layers.js";

const EXPECTED_LAYERS = [
  "staticGround",
  "persistentGroundMark",
  "fogGatedWorld",
  "rememberedWorld",
  "belowFogIntel",
  "currentFog",
  "aboveFogReveal",
  "tacticalFeedback",
  "screenOverlay",
];

assert(PRESENTATION_LAYER_IDS.join(",") === EXPECTED_LAYERS.join(","), "semantic layer ids stay exact and ordered");
assert(
  PRESENTATION_LAYER_DESCRIPTORS.every((descriptor, index) =>
    Object.isFrozen(descriptor) &&
    descriptor.id === EXPECTED_LAYERS[index] &&
    descriptor.order === index &&
    Object.keys(descriptor).join(",") === "id,order,space,visibilityPolicy,depthPolicy"),
  "semantic layer descriptors stay frozen and exact",
);

const projection = fakeProjection();
const map = {
  width: 2,
  height: 2,
  tileSize: 32,
  terrain: new Uint8Array([0, 1, 2, 3]),
  resources: [{ id: 90, kind: "steel", x: 16, y: 16, remaining: 1200 }],
};
const visibleGrid = new Uint8Array([1, 1, 0, 0]);
const exploredGrid = new Uint8Array([1, 1, 1, 0]);
const normal = {
  id: 1,
  kind: "rifleman",
  owner: 1,
  x: 20,
  y: 24,
  hp: 30,
  maxHp: 40,
  extractorActive: false,
  secretAuthoritativeVariant: { x: 999, y: 999 },
};
const intel = { id: 2, kind: "barracks", owner: 2, x: 40, y: 48, visionOnly: true };
const reveal = {
  id: 3,
  kind: "rifleman",
  owner: 2,
  x: 52,
  y: 56,
  shotReveal: true,
  shotRevealCreatedAt: 100,
  shotRevealExpiresAt: 300,
};
const invalid = { id: 4, kind: "rifleman", owner: 2, x: Number.NaN, y: 10 };
const placement = { building: "barracks", tileX: 1, tileY: 1, valid: true };
const assembler = new PresentationFrameAssembler({
  map,
  entityStats: { rifleman: { size: 10 }, barracks: { footW: 3, footH: 2 } },
});
const feedback = {
  feedbackOwnerId: 1,
  feedbackOwnerIds: [1],
  issueAsOwnerId: 1,
  showUnitRangesEnabled: true,
  showSelectedFieldOfFireEnabled: false,
  debugPathOverlaysEnabled: false,
  showAllDebugPathOverlays: false,
  placement,
  formationMovePreview: {
    points: [{ x: 10, y: 12 }, { x: 42, y: 44 }],
    slots: [{ unitId: 1, x: 18, y: 20, radius: 10 }],
  },
  commandFeedback: [{ kind: "move", x: 30, y: 30 }],
  smokes: [{ id: 8, x: 32, y: 32, radiusTiles: 2 }],
  abilityObjects: [{ id: 9, kind: "return_marker", owner: 1, x: 36, y: 36 }],
  missToasts: [{ id: 11, to: 1, createdAt: 120 }],
  enemyAntiTankGunThreats: () => [
    {
      id: 12,
      kind: "anti_tank_gun",
      owner: 2,
      x: 48,
      y: 32,
      setupState: "deployed",
      weaponFacing: Math.PI,
      threatMemory: true,
    },
    {
      id: 13,
      kind: "anti_tank_gun",
      owner: 2,
      x: 80,
      y: 32,
      setupState: "deployed",
      weaponFacing: 0,
      threatMemory: false,
    },
  ],
};
const frame = assembler.assemble({
  map,
  frameContext: {
    version: 1,
    alpha: 0.5,
    interpolatedEntities: [normal, invalid, intel, reveal],
    authoritativeEntities: [{ id: "hidden-sentinel", x: 999, y: 999 }],
    fogSourceEntities: [{ id: "hidden-fog-source", x: 998, y: 998 }],
  },
  projection,
  fog: { visibleGrid, exploredGrid, visibleRevision: 4, exploredRevision: 7 },
  feedback,
  rememberedBuildings: [{ id: 5, kind: "barracks", owner: 2, x: 48, y: 48 }],
  trenches: [{ id: 6, x: 20, y: 50, radius: 12 }],
  groundDecals: [{ id: 7, kind: "rifleman", x: 12, y: 12, seed: 4 }],
  selectionIds: new Set([1]),
  players: [
    { id: 1, teamId: 1, color: "#123456" },
    { id: 2, teamId: 2, color: "#654321" },
  ],
  playerId: 1,
  visualSamples: [{ id: "sample", kind: "trench", x: 28, y: 28 }],
  observerMapAnalysis: { regions: [{ id: "safe", x: 1, y: 2 }] },
  screenOverlay: { marquee: { x: 2, y: 3, width: 20, height: 24 } },
  visualTimeMs: 1500,
  sourceTick: 22,
});

const preparedEntities = prepareEntitySnapshots([normal, invalid, intel, reveal]);
const preparedFrame = new PresentationFrameAssembler({
  map,
  entityStats: { rifleman: { size: 10 }, barracks: { footW: 3, footH: 2 } },
}).assemble({
  map,
  frameContext: {
    version: 1,
    alpha: 0.5,
    interpolatedEntities: [normal, invalid, intel, reveal],
    preparedEntities: preparedEntities.entries,
  },
  projection,
  fog: { visibleGrid, exploredGrid, visibleRevision: 4, exploredRevision: 7 },
  selectionIds: new Set([1]),
  players: [
    { id: 1, teamId: 1, color: "#123456" },
    { id: 2, teamId: 2, color: "#654321" },
  ],
  playerId: 1,
  visualTimeMs: 1500,
  sourceTick: 22,
});
for (const layer of ["fogGatedWorld", "belowFogIntel", "aboveFogReveal"]) {
  const legacyRecords = frame.layers[layer].filter((record) => record.type.includes("Entity") || record.type === "entity");
  assert(
    JSON.stringify(preparedFrame.layers[layer]) === JSON.stringify(legacyRecords),
    `prepared ${layer} entity records serialize identically to legacy presentation detachment`,
  );
}
assert(
  preparedFrame.diagnosticsContext.droppedByCategory.entity === 1,
  "prepared malformed admitted data follows the existing bounded entity-drop diagnostic",
);

assert(Object.isFrozen(frame), "presentation frame is frozen");
assert(Object.keys(frame.layers).join(",") === EXPECTED_LAYERS.join(","), "frame exposes exactly the locked layer keys");
assert(EXPECTED_LAYERS.every((id) => Object.isFrozen(frame.layers[id])), "every semantic layer array is frozen");
assert(frame.layers.fogGatedWorld.filter((record) => record.type === "entity").length === 1, "visible entities use fogGatedWorld");
assert(frame.layers.belowFogIntel[0]?.type === "intelEntity", "vision-only received entities stay below fog");
assert(frame.layers.aboveFogReveal[0]?.type === "shotRevealEntity", "explicit shot reveals stay above fog");
assert(frame.layers.rememberedWorld[0]?.type === "rememberedBuilding", "remembered buildings remain a distinct category");
assert(frame.layers.fogGatedWorld.some((record) => record.type === "smoke"), "smoke objects cross as already-filtered world records");
assert(frame.layers.tacticalFeedback.some((record) => record.type === "placement"), "placement feedback is assembled once into tactical feedback");
assert(
  frame.layers.tacticalFeedback.some((record) =>
    record.type === "enemyAntiTankGunThreat" && record.id === 12 && record.threatMemory === true),
  "remembered enemy anti-tank warnings cross the backend-neutral tactical-feedback boundary as stale",
);
assert(
  frame.layers.tacticalFeedback.some((record) =>
    record.type === "enemyAntiTankGunThreat" && record.id === 13 && record.threatMemory === false),
  "currently visible enemy anti-tank warnings cross the backend-neutral tactical-feedback boundary as live",
);
assert(
  frame.layers.tacticalFeedback.some((record) => record.type === "formationMovePreview" && record.points.length === 2 && record.slots.length === 1),
  "formation stroke and destination slots cross the backend-neutral tactical-feedback boundary",
);
assert(frame.layers.screenOverlay[0]?.type === "marquee", "screen marquee crosses through the screen overlay layer");
assert(frame.layers.fogGatedWorld[0].selected === true, "visual selected state is resolved before the backend boundary");
assert(frame.layers.fogGatedWorld[0].relationship === "own", "viewer relationship is resolved before the backend boundary");
assert(frame.layers.fogGatedWorld[0].teamColor === "#123456", "team color is resolved before the backend boundary");
assert(frame.layers.fogGatedWorld[0].anchors.hp.heightPx === 10, "presentation anchors use semantic mirrored size");
assert(frame.layers.fogGatedWorld[0].extractorActive === false, "extractor status crosses the presentation boundary");
assert(frame.groundDecalRevision === 0, "detached frames carry an explicit zero revision when no durable batch id was supplied");
assert(!("secretAuthoritativeVariant" in frame.layers.fogGatedWorld[0]), "unadmitted entity fields do not cross the boundary");
assert(!JSON.stringify(frame).includes("hidden-sentinel"), "authoritative variants never enter the renderer frame");
assert(!JSON.stringify(frame).includes("hidden-fog-source"), "fog-source variants never enter the renderer frame");
assert(frame.diagnosticsContext.droppedRecords === 1, "one malformed entity is dropped with bounded diagnostics");
assert(frame.layers.belowFogIntel.length === 1, "a malformed entity does not prevent later records from assembling");
assert(frame.visible.revision === 4 && frame.explored.revision === 7, "fog snapshots use post-update revisions");
assert(frame.staticMapRevision === assembler.staticMap.revision, "dynamic frames reference the separately versioned static map");
assert(assembler.staticMap.widthPx === 64 && assembler.staticMap.heightPx === 64, "static map dimensions are world pixels");
assert(assembler.staticMap.resourceSites[0].remaining === undefined, "static resource sites exclude mutable remaining amounts");
assert(Object.keys(frame.visible).join(",") === "version,revision,width,height,values", "grid snapshot exposes the cloneable V2 shape");
assert(frame.version === 2 && frame.projection.version === 2, "renderer frame and projection use PresentationFrameV2 data");
assert(
  frame.projection.kind === "orthographic" &&
    frame.projection.orthographic.originX === -288 &&
    frame.projection.orthographic.originY === -208,
  "renderer projection preserves exact orthographic coefficients without reconstructing them from focus",
);
assert(!containsFunctionOrClass(frame), "renderer frame contains only structured-cloneable records and typed arrays");
const clonedFrame = structuredClone(frame);
assert(clonedFrame !== frame && clonedFrame.layers !== frame.layers, "structuredClone detaches the full renderer frame graph");
assert(clonedFrame.visible.values !== frame.visible.values, "structuredClone detaches revisioned grid buffers");
assert(JSON.stringify(clonedFrame) === JSON.stringify(frame), "structuredClone preserves renderer-frame semantics");

normal.x = 999;
placement.tileX = 99;
visibleGrid[0] = 0;
map.terrain[0] = 9;
assert(frame.layers.fogGatedWorld[0].x === 20, "entity records detach from mutable source state");
assert(frame.layers.tacticalFeedback.find((record) => record.type === "placement").tileX === 1, "feedback records detach from client intent");
assert(gridSnapshotValue(frame.visible, 0) === 1, "fog snapshot is pinned against later source mutation");
assert(gridSnapshotValue(assembler.staticMap.terrain, 0) === 0, "terrain snapshot is pinned against later source mutation");

const copied = new Uint8Array(6);
assert(copyGridSnapshotInto(frame.visible, copied, 1) === 4, "grid snapshot copies into backend-owned staging");
assert(copied.join(",") === "0,1,1,0,0,0", "grid copy respects target offset");
assert(gridSnapshotValue(frame.visible, -1) === undefined && gridSnapshotValue(frame.visible, 4) === undefined, "grid helper rejects out-of-bounds indexes");
assertThrows(() => copyGridSnapshotInto(frame.visible, new Uint8Array(3)), "grid copy rejects insufficient target capacity");

const sameRevision = assembler.assemble({
  map,
  frameContext: { alpha: 1, interpolatedEntities: [] },
  projection,
  fog: { visibleGrid: new Uint8Array([0, 0, 0, 0]), exploredGrid, visibleRevision: 4, exploredRevision: 7 },
  visualTimeMs: 1501,
});
assert(sameRevision.visible === frame.visible, "unchanged grid revision reuses the immutable snapshot object");
const nextRevision = assembler.assemble({
  map,
  frameContext: { alpha: 1, interpolatedEntities: [] },
  projection,
  fog: { visibleGrid: new Uint8Array([0, 0, 0, 0]), exploredGrid, visibleRevision: 5, exploredRevision: 7 },
  visualTimeMs: 1502,
});
assert(nextRevision.visible !== frame.visible && gridSnapshotValue(nextRevision.visible, 0) === 0, "changed grid revision creates a new detached snapshot");

const oldStaticMap = assembler.staticMap;
const resetMap = { width: 1, height: 1, tileSize: 16, terrain: [2], resources: [] };
assembler.reset({
  generation: 2,
  map: resetMap,
});
const resetFrame = assembler.assemble({
  map: resetMap,
  frameContext: { alpha: 1, interpolatedEntities: [] },
  projection,
  fog: { visibleGrid: [1], exploredGrid: [1], visibleRevision: 1, exploredRevision: 1 },
  mode: "fixedCapture",
  visualTimeMs: 2000,
});
assert(resetFrame.generation === 2 && resetFrame.frameId === 1, "Lab/replay reset starts a fresh presentation generation and frame sequence");
assert(resetFrame.diagnosticsContext.mode === "fixedCapture", "fixed capture is explicit in detached diagnostics context");
assert(gridSnapshotValue(oldStaticMap.terrain, 0) === 0, "reset does not mutate an older retained static snapshot");

const rematchAssembler = new PresentationFrameAssembler({ map: { width: 1, height: 1, tileSize: 16, terrain: [0] } });
assert(rematchAssembler.staticMap !== assembler.staticMap, "a rematch owns a fresh static presentation object");
assertThrows(() => detachedRecord({ value: new Uint8Array([1]) }), "ordinary records reject typed-array views");
assertThrows(() => createGridSnapshot({ revision: 0, width: 2, height: 2, source: [1] }), "grid snapshots reject short sources");

// Frame-loop integration: fog updates before the final frame, one projection/feedback/assembly
// reaches the backend through render(frame), and successful presentation publishes its scene.
{
  let fogUpdated = false;
  let projectionReads = 0;
  let rendererCalls = 0;
  let published = null;
  let entityReads = 0;
  let decalReconciliations = 0;
  let decalAcknowledgements = 0;
  let outcomeStatus = PRESENTATION_OUTCOME.FAILED;
  let terminalRendererFailure = false;
  let stopCalls = 0;
  const integrationMap = { width: 1, height: 1, tileSize: 32, terrain: [0], resources: [] };
  const match = {
    running: true,
    lastFrame: 100,
    frameProfiler: null,
    health: {},
    camera: {
      audioListener() { return null; },
      projectionSnapshot() { projectionReads += 1; return projection; },
    },
    audio: null,
    input: {
      screenOverlay: { snapshot: () => ({ version: 1, marquee: null }) },
      publishSelectionScene(scene) { published = scene; },
    },
    minimap: { updateCommandTargetPreview() {}, render() {} },
    hud: { update() {} },
    advancePredictionVisual() {},
    computeAlpha() { return 0.25; },
    state: {
      map: integrationMap,
      playerId: 1,
      spectator: false,
      players: [{ id: 1, teamId: 1, color: "#112233" }],
      selection: new Set([10]),
      visibleTiles: [1],
      rememberedBuildings: [],
      trenches: [],
      reconcilePendingGroundDecals() {
        decalReconciliations += 1;
        return decalAcknowledgements === 0
          ? { revision: 1, decals: [{ id: 12, kind: "rifleman", x: 10, y: 10 }] }
          : { revision: 0, decals: [] };
      },
      acknowledgeReconciledGroundDecals(revision) {
        assert(revision === 1, "frame integration acknowledges the exact retained decal revision");
        decalAcknowledgements += 1;
      },
      tick: 9,
      entitiesInterpolated(alpha, options = {}) {
        entityReads += 1;
        return [{ id: 10, kind: "rifleman", owner: 1, x: alpha * 10, y: 10, includePrediction: options.includePrediction !== false }];
      },
      selectedEntities() { return [{ id: 10, kind: "rifleman", owner: 1, x: 10, y: 10 }]; },
    },
    fog: {
      visibleGrid: new Uint8Array(1),
      exploredGrid: new Uint8Array(1),
      visibleRevision: 0,
      exploredRevision: 0,
      update() {
        this.visibleGrid[0] = 1;
        this.exploredGrid[0] = 1;
        this.visibleRevision = 1;
        this.exploredRevision = 1;
        fogUpdated = true;
      },
    },
    clientIntent: null,
    renderClock: { now: () => 700 },
    renderer: {
      terminalFailure() { return terminalRendererFailure ? new Error("terminal worker failure") : null; },
      render(frame) {
        rendererCalls += 1;
        assert(fogUpdated, "backend runs only after fog and final frame assembly");
        assert(gridSnapshotValue(frame.visible, 0) === 1, "backend receives the post-fog presentation frame");
        assert(frame.groundDecalRevision === (decalAcknowledgements === 0 ? 1 : 0), "backend receives the exact reconciled durable revision");
        assert(frame.layers.persistentGroundMark.length === (decalAcknowledgements === 0 ? 1 : 0), "decal reconciliation runs before final assembly");
        if (outcomeStatus == null) return {};
        return immediatePresentationSubmission({
          generation: frame.generation,
          frameId: frame.frameId,
          retainedRevision: outcomeStatus === PRESENTATION_OUTCOME.PRESENTED ? frame.groundDecalRevision : 0,
          status: outcomeStatus,
          error: outcomeStatus === PRESENTATION_OUTCOME.FAILED ? new Error("planned frame failure") : null,
        });
      },
    },
    observerDiagnostics: null,
    stop() { stopCalls += 1; this.running = false; },
  };
  await runMatchCaptureFrame(match, 700);
  assert(rendererCalls === 1, "one backend call occurs for one capture frame");
  assert(projectionReads === 1, "one projection snapshot is shared by frame and SelectionScene");
  assert(entityReads === 2, "alpha-1 capture builds predicted and authoritative views without backend re-query");
  assert(decalReconciliations === 1, "one shared decal reconciliation occurs for the frame");
  assert(decalAcknowledgements === 0, "a failed backend frame retains its reconciled decal batch for retry");
  assert(match.presentationFrame.diagnosticsContext.assemblyOrdinal === 1, "one presentation assembly occurs for the frame");
  assert(published === null, "a failed backend frame does not publish a new selection scene");
  outcomeStatus = null;
  await runMatchCaptureFrame(match, 708);
  assert(decalAcknowledgements === 0, "a malformed backend result cannot acknowledge the reconciled decal batch");
  assert(published === null, "a malformed backend result cannot publish a new selection scene");
  outcomeStatus = PRESENTATION_OUTCOME.PRESENTED;
  await runMatchCaptureFrame(match, 716);
  assert(decalReconciliations === 3, "later frames keep reconciling the retained decal batch until presentation succeeds");
  assert(decalAcknowledgements === 1, "a successful backend frame acknowledges its reconciled decal batch");
  assert(published?.frameId === match.presentationFrame.frameId, "published selection scene matches the presented frame id");
  match.captureClock = { advanceTo() {} };
  match.renderer._renderFrameCount = 999;
  const fixed = await renderFixedCaptureFrame(match, 724);
  assert(fixed.rendererFrame === match.presentationFrame.frameId && fixed.rendererFrame !== 999,
    "fixed capture awaits and returns the public acknowledged frame id instead of a renderer-private counter");
  terminalRendererFailure = true;
  outcomeStatus = PRESENTATION_OUTCOME.FAILED;
  await runMatchCaptureFrame(match, 732);
  assert(stopCalls === 1 && match.running === false,
    "a terminal worker failure stops the match loop instead of assembling failed frames forever");
}

function fakeProjection() {
  const camera = Object.freeze({
    version: 1,
    focus: Object.freeze({ x: 32, y: 32 }),
    framingScale: 1,
    boundsPolicy: "mapOverscroll",
  });
  const viewport = Object.freeze({ widthCssPx: 640, heightCssPx: 480 });
  const mapBounds = Object.freeze({ minX: 0, minY: 0, maxX: 64, maxY: 64 });
  const orthographic = Object.freeze({
    originX: -288,
    originY: -208,
    framingScale: 1,
    worldWidthPx: 64,
    worldHeightPx: 64,
    viewportWidthCssPx: 640,
    viewportHeightCssPx: 480,
  });
  return Object.freeze({
    version: 1,
    camera,
    viewport,
    mapBounds,
    orthographic,
    project: (point) => ({ x: point.x, y: point.y, depth: 1, clip: "inside", visible: true }),
    groundAtScreen: (point) => ({ x: point.x, y: point.y }),
    projectedExtent: () => ({ width: 1, height: 1, scaleX: 1, scaleY: 1, visible: true }),
    viewportGroundPolygon: () => [],
    viewportGroundBounds: () => mapBounds,
    containsProjected: () => true,
    snapshot: () => camera,
    audioListener: () => ({ x: 32, y: 32, referenceDistancePx: 640 }),
  });
}

function containsFunctionOrClass(value, seen = new Set()) {
  if (typeof value === "function") return true;
  if (value == null || typeof value !== "object" || seen.has(value)) return false;
  if (ArrayBuffer.isView(value)) return false;
  if (value instanceof Map || value instanceof Set) return true;
  const prototype = Object.getPrototypeOf(value);
  if (!Array.isArray(value) && prototype !== Object.prototype && prototype !== null) return true;
  seen.add(value);
  return Object.values(value).some((entry) => containsFunctionOrClass(entry, seen));
}

function assertThrows(fn, message) {
  let threw = false;
  try { fn(); } catch { threw = true; }
  assert(threw, message);
}
