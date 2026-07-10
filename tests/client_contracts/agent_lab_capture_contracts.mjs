import assert from "node:assert/strict";

import { AgentLabBridge } from "../../client/src/agent_lab_bridge.js";
import { CleanPresentation, CLEAN_PRESENTATION_ATTRIBUTE } from "../../client/src/clean_presentation.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { LAB_ROLE } from "../../client/src/protocol.js";

class FakeRoot {
  constructor() {
    this.attributes = new Map();
  }

  getAttribute(name) {
    return this.attributes.get(name) || null;
  }

  setAttribute(name, value) {
    this.attributes.set(name, value);
  }

  removeAttribute(name) {
    this.attributes.delete(name);
  }
}

const root = new FakeRoot();
const presentation = new CleanPresentation({ root });
assert.equal(presentation.set(true), true, "clean presentation enters through one app-shell attribute");
assert.equal(root.getAttribute(CLEAN_PRESENTATION_ATTRIBUTE), "true", "clean presentation records active state on the app root");
assert.equal(presentation.set(false), false, "clean presentation exits reversibly");
assert.equal(root.getAttribute(CLEAN_PRESENTATION_ATTRIBUTE), null, "clean presentation exit restores normal DOM visibility state");
presentation.set(true);
presentation.destroy();
assert.equal(root.getAttribute(CLEAN_PRESENTATION_ATTRIBUTE), null, "clean presentation teardown never leaks a hidden-DOM attribute to a rematch");

const renderer = {
  _assetReadiness: new Map(),
  _missingTextureEntityIds: new Set([9]),
  _renderFrameCount: 12,
  _renderErrors: new Map(),
  groundDecalDiagnostics: () => ({ assetStatus: "idle" }),
};
Renderer.prototype._trackVisualAsset.call(renderer, "live-png:tank", Promise.resolve({ texture: true }), { kind: "tank", source: "livePngAtlas" });
await Promise.resolve();
let readiness = Renderer.prototype.captureReadiness.call(renderer, { subjectIds: [9], subjectKinds: ["tank"] });
assert.equal(readiness.assets[0].status, "ready", "renderer capture readiness retains settled live asset state");
assert.deepEqual(readiness.missingTextureSubjectIds, [9], "renderer capture readiness rejects missing-texture fallback for a selected subject");
renderer._missingTextureEntityIds.clear();
Renderer.prototype._trackVisualAsset.call(renderer, "visual-frame-strip:tank:test", Promise.reject(new Error("late atlas missing")), { kind: "tank", source: "visualFrameStrip" });
await Promise.resolve();
await Promise.resolve();
readiness = Renderer.prototype.captureReadiness.call(renderer, { subjectIds: [], subjectKinds: ["tank"] });
assert.equal(readiness.failedAssets.length, 1, "late visual asset failure remains capture-visible rather than silently falling back");
assert.match(readiness.failedAssets[0].message, /late atlas missing/, "asset failure preserves an actionable reason");

const calls = [];
const camera = {
  x: 10,
  y: 20,
  zoom: 1,
  viewW: 100,
  viewH: 80,
  screenToWorld(x, y) { return { x: this.x + x, y: this.y + y }; },
};
const subject = { id: 4, kind: "tank", owner: 1, x: 50, y: 60, hp: 100, maxHp: 100, state: "idle", orderPlan: [] };
const captureBridge = new AgentLabBridge({
  enabled: true,
  windowLike: {},
  app: {
    net: { ws: { readyState: 1 } },
    setCleanPresentation: (enabled) => calls.push(`presentation:${enabled}`),
    labClient: { state: { role: LAB_ROLE.OPERATOR, room: "capture-contract" } },
    match: {
      camera,
      handleResize: () => calls.push("resize"),
      state: {
        currRecvTime: 1,
        tick: 9,
        players: [],
        map: { name: "Default", width: 64, height: 64, tileSize: 32 },
        entityById: (id) => id === subject.id ? subject : null,
      },
      renderer: {
        captureReadiness: () => ({
          frame: 4,
          assets: [],
          ready: true,
          failedAssets: [],
          pendingAssets: [],
          renderErrors: [],
          missingTextureSubjectIds: [],
        }),
      },
      frameErrors: { count: 0 },
      capabilities: { roomTime: { available: true } },
      roomTimeControls: { roomTimeState: { currentTick: 9, speed: 0, paused: true } },
    },
  },
});
const entered = await captureBridge.presentation({ mode: "clean" });
assert.equal(entered.mode, "clean", "bridge owns a typed clean presentation transition");
assert.deepEqual(calls, ["presentation:true", "resize"], "presentation reapplies renderer/camera bounds after the DOM mode changes");
const captureStatus = captureBridge.captureReadiness({ subjectIds: [subject.id] });
assert.equal(captureStatus.ready, true, "capture readiness combines assets, fonts, frame loop, and renderer state");
assert.equal(captureStatus.subjects[0].kind, "tank", "capture readiness returns only concise selected-subject facts");
await captureBridge.presentation({ mode: "default" });
assert.equal(calls.at(-2), "presentation:false", "clean presentation exits after a capture without retaining hidden UI state");
captureBridge.destroy();

console.log("✅ agent_lab_capture_contracts.mjs: clean presentation and capture readiness contracts passed");
