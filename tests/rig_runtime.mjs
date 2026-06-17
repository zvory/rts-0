#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { KIND, STATE } from "../client/src/protocol.js";
import {
  WORKER_LEGACY_PARTS,
  _drawUnit,
  _rigRenderContextFor,
  createLegacyUnitPartCapture,
} from "../client/src/renderer/units.js";
import { _sweep } from "../client/src/renderer/layers.js";
import { createLiveRigDefinitions } from "../client/src/renderer/rigs/live_routing.js";
import { compileSvgRig } from "../client/src/renderer/rigs/svg_importer.js";
import { createRigRenderContext, sampleRigAnimation } from "../client/src/renderer/rigs/animation.js";
import {
  UnitRigInstance,
  createUnitRigInstance,
  renderLiveUnitRig,
  renderRigLegacyComparison,
} from "../client/src/renderer/rigs/runtime.js";
import { WORKER_RIG_SVG } from "../client/src/renderer/rigs/worker_svg.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, "fixtures/svg");
const fixedNow = 12_345;

function main() {
test("animation sampler applies game-state bindings without Pixi", () => {
  const definition = compileFixture("rig-vehicle.svg", KIND.TANK);
  const entity = {
    id: 7,
    kind: KIND.TANK,
    owner: 1,
    x: 80,
    y: 90,
    hp: 60,
    maxHp: 100,
    state: STATE.MOVE,
    facing: Math.PI / 2,
    weaponFacing: Math.PI,
  };
  const context = createRigRenderContext(entity, {
    now: fixedNow,
    state: { playerId: 1, resources: { oil: 0 }, weaponRecoil: () => 0.5 },
    colorByOwner: new Map([[1, 0x336699]]),
    vehicleMotion: { activity: 0.75 },
  });
  const sampled = sampleRigAnimation(definition, entity, context);
  assert.equal(sampled.context.teamColor, 0x336699);
  assert.equal(sampled.context.oilStarved, true);
  assert.equal(sampled.parts["part.hull"].transform.rotation, Math.PI / 2);
  assert.equal(sampled.parts["part.turret"].transform.rotation, Math.PI);
  assert.equal(sampled.parts["part.barrel"].transform.rotation, Math.PI);
});

test("rig runtime creates one container child per part and updates transforms", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const instance = createUnitRigInstance(KIND.WORKER, definition, createInspectionPixiFactory());
  assert.equal(instance.container.children.length, definition.parts.length);
  assert.deepEqual(instance.container.children.map((child) => child.rtsRigPartId), definition.parts.map((part) => part.id));

  instance.update({
    id: 1,
    kind: KIND.WORKER,
    owner: 1,
    x: 24,
    y: 32,
    facing: Math.PI / 4,
    hp: 20,
    maxHp: 30,
  }, createRigRenderContext({ id: 1, kind: KIND.WORKER, owner: 1, facing: Math.PI / 4 }, {
    now: fixedNow,
    colorByOwner: new Map([[1, 0x225588]]),
  }));

  const body = instance.parts.get("part.body").display;
  const facingTick = instance.parts.get("part.facingTick").display;
  const busyIndicator = instance.parts.get("part.busyIndicator").display;
  assert.equal(instance.container.x, 24);
  assert.equal(instance.container.y, 32);
  assert.equal(body.rotation, 0);
  assert.equal(facingTick.rotation, Math.PI / 4);
  assert.equal(busyIndicator.visible, false);
  assert.equal(body.commands.find((cmd) => cmd.op === "beginFill").color, 0x225588);
  assert.deepEqual(body.commands.find((cmd) => cmd.op === "lineStyle"), { op: "lineStyle", width: 2, color: 0x1a1712, alpha: 0.95 });

  instance.update({
    id: 1,
    kind: KIND.WORKER,
    owner: 1,
    x: 24,
    y: 32,
    facing: Math.PI / 4,
    hp: 20,
    maxHp: 30,
    state: STATE.BUILD,
  }, createRigRenderContext({ id: 1, kind: KIND.WORKER, owner: 1, facing: Math.PI / 4, state: STATE.BUILD }, {
    now: fixedNow,
    colorByOwner: new Map([[1, 0x225588]]),
  }));
  assert.equal(instance.parts.get("part.busyIndicator").display.visible, true);

  instance.destroy();
  assert.equal(instance._destroyed, true);
  assert.equal(instance.parts.size, 0);
  assert.equal(instance.container.destroyed, true);
});

test("rig runtime can update one named part group for part-level fixtures", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const instance = createUnitRigInstance(KIND.WORKER, definition, createInspectionPixiFactory());
  instance.update({
    id: 11,
    kind: KIND.WORKER,
    owner: 1,
    x: 10,
    y: 20,
    facing: Math.PI / 2,
  }, createRigRenderContext({ id: 11, kind: KIND.WORKER, owner: 1, facing: Math.PI / 2 }, {
    now: fixedNow,
    colorByOwner: new Map([[1, 0x778899]]),
  }), { includeParts: ["part.facingTick"] });

  assert.equal(instance.parts.get("part.facingTick").display.visible, true);
  assert.equal(instance.parts.get("part.body").display.visible, false);
  assert.equal(instance.parts.get("part.shadow").display.visible, false);
  assert.equal(instance.parts.get("part.facingTick").display.commands.some((cmd) => cmd.op === "lineTo"), true);
  instance.destroy();
});

test("legacy Worker part capture records stable draw names and filters output", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = {
    id: 12,
    kind: KIND.WORKER,
    owner: 1,
    x: 30,
    y: 36,
    facing: Math.PI / 4,
    state: STATE.BUILD,
    latchedNode: 9001,
  };
  const renderer = makeComparisonRenderer(definition);
  const capture = createLegacyUnitPartCapture({ includeParts: [WORKER_LEGACY_PARTS.facingTick] });

  _drawUnit.call(renderer, entity, new Map([[1, 0x334455]]), { weaponRecoil: () => 0 }, { partCapture: capture });

  assert.deepEqual(capture.records.map((record) => record.name), [
    WORKER_LEGACY_PARTS.shadow,
    WORKER_LEGACY_PARTS.body,
    WORKER_LEGACY_PARTS.busyIndicator,
    WORKER_LEGACY_PARTS.facingTick,
  ]);
  const unitCommands = renderer._pools.units.get(entity.id).commands;
  assert.equal(unitCommands.some((cmd) => cmd.op === "drawPolygon"), false);
  assert.equal(unitCommands.some((cmd) => cmd.op === "lineTo"), true);
  assert.equal(renderer._pools.unitShadows.has(entity.id), false);
});

test("side-by-side comparison path is explicit and leaves default unit draw on legacy", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 2, kind: KIND.WORKER, owner: 1, x: 40, y: 50, facing: 0, state: STATE.IDLE };
  const renderer = makeComparisonRenderer(definition);
  renderer._drawUnit(entity, new Map([[1, 0x445566]]), { weaponRecoil: () => 0 });
  assert.equal(renderer.legacyDraws, 1);
  assert.equal(renderer._rigComparisonPool.size, 0);

  renderer._rigComparisonEnabled = true;
  renderer._drawUnit(entity, new Map([[1, 0x445566]]), { weaponRecoil: () => 0 });
  assert.equal(renderer.legacyDraws, 2);
  assert.equal(renderer._rigComparisonPool.size, 1);
  assert.equal(renderer.layers.rigComparisons.children.length, 1);
  assert.equal(renderer._rigComparisonPool.get(entity.id).container.x, entity.x + 48);
});

test("live rig definitions compile Worker from the production SVG source", () => {
  const fixtureText = fs.readFileSync(path.join(fixturesDir, "rig-worker.svg"), "utf8").trim();
  assert.equal(WORKER_RIG_SVG.trim(), fixtureText);
  const definitions = createLiveRigDefinitions();
  assert.equal(definitions.has(KIND.WORKER), true);
  assert.equal(definitions.get(KIND.WORKER).id, "worker.authored");
});

test("default Worker draw uses live SVG rig without enabling comparison", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 4, kind: KIND.WORKER, owner: 1, x: 32, y: 44, facing: Math.PI / 2, state: STATE.IDLE };
  const renderer = makeComparisonRenderer(definition);
  renderer._liveRigDefinitionsByKind = new Map([[KIND.WORKER, definition]]);

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { weaponRecoil: () => 0 });

  assert.equal(renderer.legacyDraws, 0);
  assert.equal(renderer._liveRigPools.liveUnitRigShadows.size, 1);
  assert.equal(renderer._liveRigPools.liveUnitRigs.size, 1);
  assert.equal(renderer.layers.unitShadows.children.length, 1);
  assert.equal(renderer.layers.units.children.length, 1);
  assert.equal(renderer._liveRigPools.liveUnitRigShadows.get(entity.id).parts.get("part.shadow").display.visible, true);
  assert.equal(renderer._liveRigPools.liveUnitRigs.get(entity.id).parts.get("part.shadow").display.visible, false);
});

test("non-routed units fall back to legacy procedural drawing", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 5, kind: KIND.RIFLEMAN, owner: 1, x: 32, y: 44, facing: 0, state: STATE.IDLE };
  const renderer = makeComparisonRenderer(definition);
  renderer._liveRigDefinitionsByKind = new Map([[KIND.WORKER, definition]]);

  renderer._drawUnit(entity, new Map([[1, 0x336699]]), { weaponRecoil: () => 0 });

  assert.equal(renderer.legacyDraws, 1);
  assert.equal(renderer._liveRigPools.liveUnitRigs.size, 0);
  assert.equal(renderer._pools.units.has(entity.id), true);
});

test("live rig instances are destroyed through renderer-style sweep", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 6, kind: KIND.WORKER, owner: 1, x: 10, y: 12, facing: 0, state: STATE.IDLE };
  const renderer = makeComparisonRenderer(definition);
  renderLiveUnitRig(renderer, entity, new Map([[1, 0x112233]]), {}, definition, {
    routes: [
      { poolName: "liveUnitRigShadows", layerName: "unitShadows", parts: ["part.shadow"] },
      { poolName: "liveUnitRigs", layerName: "units", parts: ["part.body", "part.busyIndicator", "part.facingTick"] },
    ],
  });
  const shadowInstance = renderer._liveRigPools.liveUnitRigShadows.get(entity.id);
  const unitInstance = renderer._liveRigPools.liveUnitRigs.get(entity.id);
  for (const seen of Object.values(renderer._seen)) seen.clear();
  renderer._unseen = new Map([[entity.id, 999]]);
  _sweep.call(renderer);
  assert.equal(shadowInstance._destroyed, true);
  assert.equal(unitInstance._destroyed, true);
  assert.equal(renderer._liveRigPools.liveUnitRigShadows.size, 0);
  assert.equal(renderer._liveRigPools.liveUnitRigs.size, 0);
});

test("comparison instances are destroyed through renderer-style teardown and sweep", () => {
  const definition = compileFixture("rig-worker.svg", KIND.WORKER);
  const entity = { id: 3, kind: KIND.WORKER, owner: 1, x: 10, y: 12, facing: 0, state: STATE.IDLE };
  const renderer = makeComparisonRenderer(definition);
  renderer._rigComparisonEnabled = true;
  renderRigLegacyComparison(renderer, entity, new Map([[1, 0x112233]]), {}, definition);
  const instance = renderer._rigComparisonPool.get(entity.id);
  assert.ok(instance instanceof UnitRigInstance);
  for (const seen of Object.values(renderer._seen)) seen.clear();
  renderer._unseen = new Map([[entity.id, 999]]);
  _sweep.call(renderer);
  assert.equal(instance._destroyed, true);
  assert.equal(renderer._rigComparisonPool.size, 0);
});
}

function compileFixture(file, expectedKind) {
  const result = compileSvgRig(fs.readFileSync(path.join(fixturesDir, file), "utf8"), { expectedKind });
  assert.equal(result.ok, true, JSON.stringify(result.errors));
  return result.definition;
}

function makeComparisonRenderer(definition) {
  return {
    _rigComparisonEnabled: false,
    _rigDefinitionsByKind: new Map([[KIND.WORKER, definition]]),
    _rigComparisonPool: new Map(),
    _liveRigDefinitionsByKind: new Map(),
    _liveRigPools: {
      liveUnitRigShadows: new Map(),
      liveUnitRigs: new Map(),
      liveShotRevealRigShadows: new Map(),
      liveShotRevealRigs: new Map(),
    },
    _liveRigRoutes: {
      liveUnitRigShadows: { poolName: "liveUnitRigShadows", layerName: "unitShadows" },
      liveUnitRigs: { poolName: "liveUnitRigs", layerName: "units" },
      liveShotRevealRigShadows: { poolName: "liveShotRevealRigShadows", layerName: "shotRevealShadows" },
      liveShotRevealRigs: { poolName: "liveShotRevealRigs", layerName: "shotReveals" },
    },
    _rigPixiFactory: createInspectionPixiFactory(),
    _pools: { unitShadows: new Map(), units: new Map(), shotRevealShadows: new Map(), shotReveals: new Map() },
    _seen: {
      unitShadows: new Set(),
      units: new Set(),
      shotRevealShadows: new Set(),
      shotReveals: new Set(),
      rigComparisons: new Set(),
      liveUnitRigShadows: new Set(),
      liveUnitRigs: new Set(),
      liveShotRevealRigShadows: new Set(),
      liveShotRevealRigs: new Set(),
    },
    layers: {
      unitShadows: new FakeContainer(),
      units: new FakeContainer(),
      shotRevealShadows: new FakeContainer(),
      shotReveals: new FakeContainer(),
      rigComparisons: new FakeContainer(),
    },
    legacyDraws: 0,
    _drawUnit(_entity, _colorByOwner, _state, pools = {}) {
      const liveRouted = !pools.skipLiveRig && this._liveRigDefinitionsByKind?.has(_entity.kind);
      if ((!this._rigComparisonEnabled || pools.skipRigComparison) && !liveRouted) this.legacyDraws += 1;
      return _drawUnit.call(this, _entity, _colorByOwner, _state, pools);
    },
    _slot(poolName, id) {
      const pool = this._pools[poolName];
      let graphic = pool.get(id);
      if (!graphic) {
        graphic = new FakeGraphics();
        pool.set(id, graphic);
        this.layers[poolName].addChild(graphic);
      }
      this._seen[poolName].add(id);
      graphic.visible = true;
      graphic.alpha = 1;
      graphic.clear();
      return graphic;
    },
    _shadow(g, cx, cy, radius) {
      g.beginFill(0x000000, 0.28);
      g.drawEllipse(cx, cy + radius * 0.35, radius, radius * 0.6);
      g.endFill();
    },
    _vehicleShadow() {
      throw new Error("worker comparison test should not draw vehicle shadow");
    },
    _tintFor(owner, colorByOwner) {
      return colorByOwner.get(owner) ?? 0x9aa0a8;
    },
    _rigRenderContextFor(entity, colorByOwner, state) {
      return _rigRenderContextFor.call(this, entity, colorByOwner, state);
    },
    _deployedWeaponSetupVisual: () => ({ prongFactor: 0, barrel: false }),
    _tankMotionVisual: () => ({ activity: 0 }),
    _map: { tileSize: 32 },
  };
}

function createInspectionPixiFactory() {
  return {
    createContainer: () => new FakeContainer(),
    createGraphics: () => new FakeGraphics(),
  };
}

class FakeContainer {
  constructor() {
    this.children = [];
    this.position = makePointSetter(this, "x", "y");
    this.scale = makePointSetter(this, "scaleX", "scaleY");
    this.pivot = makePointSetter(this, "pivotX", "pivotY");
    this.x = 0;
    this.y = 0;
    this.scaleX = 1;
    this.scaleY = 1;
    this.visible = true;
    this.alpha = 1;
    this.rotation = 0;
    this.destroyed = false;
  }

  addChild(child) {
    child.parent = this;
    this.children.push(child);
  }

  removeChild(child) {
    child.parent = null;
    this.children = this.children.filter((candidate) => candidate !== child);
  }

  destroy() {
    this.destroyed = true;
  }
}

class FakeGraphics extends FakeContainer {
  constructor() {
    super();
    this.commands = [];
    this.lineWidth = 0;
  }

  clear() {
    this.commands = [];
    this.lineWidth = 0;
  }

  beginFill(color, alpha = 1) {
    this.commands.push({ op: "beginFill", color, alpha });
  }

  endFill() {
    this.commands.push({ op: "endFill" });
  }

  lineStyle(width = 0, color = 0, alpha = 1) {
    this.lineWidth = width;
    this.commands.push({ op: "lineStyle", width, color, alpha });
  }

  moveTo(x, y) {
    this.commands.push({ op: "moveTo", x, y });
  }

  lineTo(x, y) {
    this.commands.push({ op: "lineTo", x, y });
  }

  drawPolygon(points) {
    this.commands.push({ op: "drawPolygon", points });
  }

  drawCircle(x, y, radius) {
    this.commands.push({ op: "drawCircle", x, y, radius });
  }

  drawEllipse(x, y, rx, ry) {
    this.commands.push({ op: "drawEllipse", x, y, rx, ry });
  }

  drawRect(x, y, width, height) {
    this.commands.push({ op: "drawRect", x, y, width, height });
  }
}

function makePointSetter(target, xKey, yKey) {
  return {
    set(x, y = x) {
      target[xKey] = x;
      target[yKey] = y;
    },
  };
}

function test(name, fn) {
  try {
    fn();
  } catch (err) {
    console.error(`not ok - ${name}`);
    throw err;
  }
  console.log(`ok - ${name}`);
}

main();
