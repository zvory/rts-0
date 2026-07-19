// High-count ground decal renderer contracts.

import { assert } from "./assertions.mjs";
import { installFakePixi } from "./pixi_fakes.mjs";
import { EVENT, KIND } from "../../client/src/protocol.js";
import { GroundDecalBuffer } from "../../client/src/state_ground_decals.js";
import {
  GROUND_DECAL_TEXTURE_WORLD_SCALE,
} from "../../client/src/renderer/decals.js";
import { Renderer } from "../../client/src/renderer/index.js";

const STRESS_DECAL_COUNT = 1200;
const CURRENT_MAP_TILES = 126;
const TILE_SIZE = 32;
const EXPECTED_DECAL_TEXTURE_SIZE = (CURRENT_MAP_TILES * TILE_SIZE) / GROUND_DECAL_TEXTURE_WORLD_SCALE;

class RecordingCanvasContext {
  constructor() {
    this.calls = [];
  }
  save() { this.calls.push(["save"]); }
  restore() { this.calls.push(["restore"]); }
  translate(x, y) { this.calls.push(["translate", x, y]); }
  rotate(angle) { this.calls.push(["rotate", angle]); }
  scale(x, y) { this.calls.push(["scale", x, y]); }
  clearRect(x, y, w, h) { this.calls.push(["clearRect", x, y, w, h]); }
  fillRect(x, y, w, h) { this.calls.push(["fillRect", x, y, w, h]); }
  beginPath() { this.calls.push(["beginPath"]); }
  moveTo(x, y) { this.calls.push(["moveTo", x, y]); }
  lineTo(x, y) { this.calls.push(["lineTo", x, y]); }
  closePath() { this.calls.push(["closePath"]); }
  ellipse(x, y, rx, ry, rotation) { this.calls.push(["ellipse", x, y, rx, ry, rotation]); }
  arc(x, y, radius, start, end) { this.calls.push(["arc", x, y, radius, start, end]); }
  fill() { this.calls.push(["fill"]); }
}

{
  const buffer = new GroundDecalBuffer();
  const events = [];
  for (let i = 0; i < STRESS_DECAL_COUNT; i += 1) {
    const ev = makeDeathEvent(i);
    events.push(ev);
    if (i % 3 === 0) events.push({ ...ev });
  }

  const queued = buffer.applySnapshotEvents(events, {
    players: [{ id: 1, color: "#4878c8" }],
    tick: 700,
  });
  const firstBatch = buffer.consumePending();
  assert(queued === STRESS_DECAL_COUNT, "stress buffer queues each death id only once");
  assert(firstBatch.length === STRESS_DECAL_COUNT, "stress buffer exposes the deduped high-count decal batch");

  buffer.applySnapshotEvents(events, {
    players: [{ id: 1, color: "#4878c8" }],
    tick: 701,
  });
  assert(buffer.consumePending().length === 0, "stress buffer keeps old death ids deduped after consumption");
}

{
  const restorePixi = installFakePixi();
  const priorDocument = globalThis.document;
  const priorImage = globalThis.Image;
  const canvasContexts = [];
  globalThis.Image = undefined;
  globalThis.document = {
    createElement(tag) {
      assert(tag === "canvas", "ground decal stress renderer only creates canvas elements");
      const ctx = new RecordingCanvasContext();
      canvasContexts.push(ctx);
      return {
        width: 0,
        height: 0,
        getContext(type) {
          assert(type === "2d", "ground decal stress renderer requests 2d canvas contexts");
          return ctx;
        },
      };
    },
  };

  try {
    const renderer = await Renderer.create(fakeParent());
    muteRenderOverlays(renderer);
    const map = {
      width: CURRENT_MAP_TILES,
      height: CURRENT_MAP_TILES,
      tileSize: TILE_SIZE,
      terrain: new Array(CURRENT_MAP_TILES * CURRENT_MAP_TILES).fill(0),
    };
    renderer.buildStaticMap(map);

    const firstBatch = makeDecalBatch(20000, STRESS_DECAL_COUNT);
    const pendingBatches = [firstBatch];
    const state = makeRenderState(map, pendingBatches);
    const stamped = renderer._drawGroundDecals(state);
    let diagnostics = renderer.groundDecalDiagnostics();
    assert(stamped === STRESS_DECAL_COUNT, "stress renderer stamps the full high-count batch");
    assert(diagnostics.totalStamped === STRESS_DECAL_COUNT, "decal diagnostics expose total stamped decals");
    assert(diagnostics.pendingDecals === 0, "decal diagnostics expose no retained pending decals after stamping");
    assert(diagnostics.textureUpdateCount === 1, "one stress batch produces one texture update");
    assert(diagnostics.textureWidth === EXPECTED_DECAL_TEXTURE_SIZE, "126x126 map decal texture width is downsampled to 1008px");
    assert(diagnostics.textureHeight === EXPECTED_DECAL_TEXTURE_SIZE, "126x126 map decal texture height is downsampled to 1008px");
    assert(diagnostics.downsample === GROUND_DECAL_TEXTURE_WORLD_SCALE, "decal diagnostics expose the texture downsample");
    assert(diagnostics.layerChildCount === 1, "stress renderer keeps one permanent decal display object");

    const decalCtx = canvasContexts[1];
    const callsAfterStamp = decalCtx.calls.length;
    for (let i = 0; i < 10; i += 1) {
      renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1);
    }
    diagnostics = renderer.groundDecalDiagnostics();
    assert(decalCtx.calls.length === callsAfterStamp, "normal render frames do not redraw old decal pixels");
    assert(diagnostics.textureUpdateCount === 1, "normal render frames do not update the decal texture without new deaths");
    assert(diagnostics.layerChildCount === 1, "normal render frames do not add historical decal display objects");

    pendingBatches.push(makeDecalBatch(40000, 9));
    renderer.render(state, { x: 0, y: 0, zoom: 1 }, null, 1);
    diagnostics = renderer.groundDecalDiagnostics();
    assert(diagnostics.totalStamped === STRESS_DECAL_COUNT + 9, "new deaths append to the existing decal texture");
    assert(diagnostics.textureUpdateCount === 2, "texture updates track new-death batches, not historical decal count");
    assert(diagnostics.layerChildCount === 1, "additional batches still use one decal display object");

    renderer.destroy();
    assert(renderer.layers.decals.children.length === 0, "renderer teardown removes the permanent decal sprite");
    renderer.destroy();
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorImage === undefined) delete globalThis.Image;
    else globalThis.Image = priorImage;
    restorePixi();
  }
}

function makeDeathEvent(index) {
  return {
    e: EVENT.DEATH,
    id: 10000 + index,
    kind: index % 2 === 0 ? KIND.WORKER : KIND.TANK,
    x: 16 + (index % 80) * 50,
    y: 24 + Math.floor(index / 80) * 240,
  };
}

function makeDecalBatch(baseId, count) {
  const out = [];
  for (let i = 0; i < count; i += 1) {
    const decalType = i % 4;
    const infantry = decalType === 0;
    const scorch = decalType === 1;
    const mortar = decalType === 2;
    out.push({
      id: baseId + i,
      kind: infantry ? KIND.WORKER : scorch ? KIND.TANK : mortar ? KIND.MORTAR_TEAM : KIND.ARTILLERY,
      decalClass: infantry ? "infantry" : scorch ? "scorch" : mortar ? "mortarBlast" : "artilleryBlast",
      x: 16 + (i % 80) * 50,
      y: 24 + Math.floor(i / 80) * 240,
      owner: 1,
      color: infantry ? "#4878c8" : "#c85050",
      facing: (i % 32) * 0.12,
      weaponFacing: (i % 32) * 0.12,
      radiusWorld: mortar ? 48 : decalType === 3 ? 96 : undefined,
      seed: 910000 + i,
      variant: i % 4,
    });
  }
  return out;
}

function makeRenderState(map, pendingBatches) {
  return {
    playerId: 1,
    players: [{ id: 1, color: "#4878c8" }],
    selection: new Set(),
    rememberedBuildings: [],
    map,
    abilityObjects: [],
    smokes: [],
    entitiesInterpolated() {
      return [];
    },
    selectedEntities() {
      return [];
    },
    consumePendingGroundDecals() {
      return pendingBatches.shift() || [];
    },
  };
}

function fakeParent() {
  return {
    clientWidth: 640,
    clientHeight: 480,
    appendChild(view) {
      view.parentNode = this;
    },
    removeChild(view) {
      view.parentNode = null;
    },
  };
}

function muteRenderOverlays(renderer) {
  const noOp = () => {};
  for (const name of [
    "_drawAbilityObjects",
    "_drawSmokes",
    "_drawFog",
    "_drawSmokeCanisters",
    "_drawCommandFeedback",
    "_drawMortarTargets",
    "_drawMortarLaunches",
    "_drawMortarShells",
    "_drawMortarImpacts",
    "_drawArtilleryLaunches",
    "_drawArtilleryTargets",
    "_drawArtilleryImpacts",
    "_drawSelectedUnitRanges",
    "_drawSelectedMortarRanges",
    "_drawBreakthroughAuras",
    "_drawAbilityTargetPreview",
    "_drawAntiTankGunSetupPreview",
    "_drawOrderPlan",
    "_drawDebugPathOverlay",
    "_drawRallyPoints",
    "_drawResourceMiningPreview",
    "_drawMuzzleFlashes",
    "_drawPlacement",
  ]) {
    renderer[name] = noOp;
  }
}
