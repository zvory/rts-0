import { assert } from "./assertions.mjs";
import { PresentationFrameAssembler } from "../../client/src/presentation/frame.js";
import {
  PIXI_LEGACY_READ_ALLOWLIST,
  PixiPresentationAdapter,
} from "../../client/src/renderer/pixi_compatibility_adapter.js";

const EXPECTED_LEGACY_READS = [
  "state.resources.oil",
  "state._curById",
  "state._prevById",
  "state.weaponRecoil",
  "state.weaponRecoilPhase",
  "match.renderClock",
  "match.frameProfiler",
  "match.visualProfile.unitOverrides",
  "match.visualProfile.frameStripOverrides",
  "match.presentationAssembler.staticMap",
];

assert(
  PIXI_LEGACY_READ_ALLOWLIST.map((entry) => entry.id).join(",") === EXPECTED_LEGACY_READS.join(","),
  "Pixi compatibility reads are an exact ratcheted allowlist",
);
assert(
  PIXI_LEGACY_READ_ALLOWLIST.every((entry) => (
    Object.isFrozen(entry)
    && typeof entry.reviewTrigger === "string"
    && entry.reviewTrigger.trim().length > 0
    && !Object.hasOwn(entry, "removalOwner")
  )),
  "every temporary Pixi compatibility read names a concrete review trigger",
);

const map = {
  width: 2,
  height: 1,
  tileSize: 32,
  terrain: new Uint8Array([0, 1]),
  resources: [{ id: 50, kind: "steel", x: 16, y: 16 }],
};
const assembler = new PresentationFrameAssembler({ map, entityStats: { tank: { size: 14 } } });
const projection = fakeProjection();
const frameInputs = {
  map,
  frameContext: {
    alpha: 0.5,
    interpolatedEntities: [{
      id: 7,
      kind: "tank",
      owner: 1,
      x: 20,
      y: 24,
      hp: 80,
      maxHp: 100,
      state: "move",
    }],
  },
  projection,
  fog: {
    visibleGrid: new Uint8Array([1, 0]),
    exploredGrid: new Uint8Array([1, 1]),
    visibleRevision: 2,
    exploredRevision: 3,
  },
  feedback: {
    feedbackOwnerId: 1,
    feedbackOwnerIds: [1],
    showUnitRangesEnabled: true,
    commandFeedback: [{ kind: "move", x: 30, y: 30 }],
  },
  groundDecals: [{ id: 71, kind: "tank", decalClass: "scorch", x: 20, y: 24 }],
  screenOverlay: { marquee: { x: 4, y: 5, w: 10, h: 12 } },
  selectionIds: new Set([7]),
  players: [{ id: 1, teamId: 1, color: "#123456" }],
  playerId: 1,
  visualTimeMs: 500,
  sourceTick: 9,
};
const frame = assembler.assemble(frameInputs);
let stateReadCount = 0;
let destructiveReads = 0;
const current = new Map([[7, { x: 22, y: 24 }]]);
const previous = new Map([[7, { x: 18, y: 24 }]]);
const sourceState = {
  get resources() { stateReadCount += 1; return { oil: 4 }; },
  get _curById() { stateReadCount += 1; return current; },
  get _prevById() { stateReadCount += 1; return previous; },
  weaponRecoil() { stateReadCount += 1; return 0.25; },
  weaponRecoilPhase() { stateReadCount += 1; return 0.75; },
  consumePendingGroundDecals() { destructiveReads += 1; return []; },
};
const engine = fakeEngine();
const sources = {
  renderClock: { now: () => 500 },
  state: () => sourceState,
  profiler: () => ({ recordDiagnosticCounter() {} }),
  visualProfile: () => ({ unitOverrides: [{ id: "test" }], frameStripOverrides: [] }),
  staticMap: () => assembler.staticMap,
};
const adapter = new PixiPresentationAdapter(null, sources, { renderer: engine });

const first = adapter.render(frame);
const readsAfterFirst = stateReadCount;
const second = adapter.render(frame);
assert(first.presented && second.presented, "Pixi adapter presents an immutable frame and a repeated call");
assert(engine.staticMaps.length === 1, "unchanged static-map revision is materialized once into Pixi-owned staging");
assert(engine.staticMaps[0].terrain instanceof Uint8Array, "Pixi owns its copied terrain staging buffer");
assert(engine.renders.length === 2, "repeated render(frame) calls reach the backend without reassembly");
assert(stateReadCount === readsAfterFirst, "repeated render of the same frame does not query legacy state again");
assert(destructiveReads === 0, "Pixi render never consumes the shared GameState decal queue");
assert(engine.renders[0].options.reconciledGroundDecals.length === 1, "first presentation reconciles its assembled decal batch");
assert(engine.renders[1].options.reconciledGroundDecals.length === 0, "repeated presentation cannot stamp the same decal batch twice");
assert(engine.renders[0].state !== sourceState, "legacy Pixi helpers receive a frame-derived facade, not mutable GameState");
assert(engine.renders[0].state.resources.oil === 4, "allowlisted low-oil cue input is snapshotted into the facade");
assert(engine.renders[0].state._curById.get(7).x === 22, "allowlisted current pose is detached for legacy motion sampling");
assert(engine.renders[0].state.weaponRecoil(7) === 0.25, "allowlisted recoil is sampled once at assembly time");
assert(engine.renders[0].fog.isVisible(0, 0) && !engine.renders[0].fog.isVisible(1, 0), "Pixi fog facade reads backend-owned grid copies");
assert(engine.marquees[0].w === 10 && engine.marquees[0].h === 12, "screen marquee is drawn from the assembled screenOverlay layer");

const nextFrame = assembler.assemble({ ...frameInputs, visualTimeMs: 516, sourceTick: 10, groundDecals: [] });
engine.failNext = true;
assert(adapter.render(nextFrame).presented === false, "top-level Pixi failure is bounded to the current frame");
const recoveryFrame = assembler.assemble({ ...frameInputs, visualTimeMs: 532, sourceTick: 11, groundDecals: [] });
assert(adapter.render(recoveryFrame).presented === true, "a later Pixi frame still presents after a bounded backend failure");
assert(engine.errors.some(([label]) => label === "pixiPresentationFrame"), "bounded backend failure records an actionable diagnostic");

const captureClock = { now: () => 532 };
adapter.enterFixedCapture(captureClock);
adapter.presentFixedCaptureFrame();
adapter.exitFixedCapture(captureClock);
assert(engine.captureLifecycle.join(",") === "enter,present,exit", "fixed capture delegates through the same Pixi adapter lifecycle");

const replacementMap = { width: 1, height: 1, tileSize: 16, terrain: new Uint8Array([2]), resources: [] };
assembler.reset({ map: replacementMap, generation: 2 });
const replacementFrame = assembler.assemble({
  ...frameInputs,
  map: replacementMap,
  frameContext: { alpha: 1, interpolatedEntities: [] },
  fog: { visibleGrid: [1], exploredGrid: [1], visibleRevision: 1, exploredRevision: 1 },
  groundDecals: [],
  screenOverlay: null,
  visualTimeMs: 548,
  sourceTick: 0,
});
assert(adapter.render(replacementFrame).presented === true, "Lab/replay static-map reset presents a fresh generation");
assert(engine.staticMaps.length === 2 && engine.staticMaps[1].tileSize === 16, "changed static revision rebuilds Pixi-owned staging once");

adapter.destroy();
adapter.destroy();
assert(engine.destroyed === 1, "Pixi adapter delegates teardown exactly once");

function fakeEngine() {
  return {
    app: { renderer: {}, view: {} },
    _renderFrameCount: 0,
    _map: null,
    staticMaps: [],
    renders: [],
    marquees: [],
    errors: [],
    failNext: false,
    destroyed: 0,
    captureLifecycle: [],
    buildStaticMap(staticMap) {
      this._map = staticMap;
      this.staticMaps.push(staticMap);
    },
    render(state, camera, fog, alpha, options) {
      if (this.failNext) {
        this.failNext = false;
        throw new Error("planned backend failure");
      }
      this._renderFrameCount += 1;
      this.renders.push({ state, camera, fog, alpha, options });
    },
    drawSelectionBox(rect) { this.marquees.push(rect); },
    _recordRenderError(label, error) { this.errors.push([label, error.message]); },
    resize() {},
    enterFixedCapture() { this.captureLifecycle.push("enter"); },
    presentFixedCaptureFrame() { this.captureLifecycle.push("present"); },
    exitFixedCapture() { this.captureLifecycle.push("exit"); },
    captureReadiness() { return { ready: true }; },
    groundDecalDiagnostics() { return {}; },
    trenchDiagnostics() { return {}; },
    visualSampleDiagnostics() { return {}; },
    visualUnitOverrideDiagnostics() { return {}; },
    destroy() { this.destroyed += 1; },
  };
}

function fakeProjection() {
  const camera = Object.freeze({ version: 1, focus: Object.freeze({ x: 32, y: 16 }), framingScale: 2 });
  const viewport = Object.freeze({ widthCssPx: 100, heightCssPx: 80 });
  const mapBounds = Object.freeze({ minX: 0, minY: 0, maxX: 64, maxY: 32 });
  return Object.freeze({
    version: 1,
    camera,
    viewport,
    mapBounds,
    project: (point) => ({ x: point.x, y: point.y, depth: 1, clip: "inside", visible: true }),
    groundAtScreen: (point) => ({ x: point.x, y: point.y }),
    projectedExtent: () => ({ width: 2, height: 2, scaleX: 2, scaleY: 2, visible: true }),
    viewportGroundPolygon: () => [],
    viewportGroundBounds: () => mapBounds,
    containsProjected: () => true,
    snapshot: () => camera,
    audioListener: () => ({ x: 32, y: 16, referenceDistancePx: 100 }),
  });
}

console.log("✅ pixi_presentation_adapter_contracts.mjs: Pixi frame cutover contracts passed");
