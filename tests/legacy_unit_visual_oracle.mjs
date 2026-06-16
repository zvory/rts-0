#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { STATS, PLAYER_PALETTE } from "../client/src/config.js";
import { KIND, SETUP, STATE, UNIT_KINDS } from "../client/src/protocol.js";
import {
  _drawSelectionAndHp,
  _hpBar,
  _ringRadius,
  _shadow,
  _slot,
  _tintFor,
  _vehicleShadow,
} from "../client/src/renderer/entities.js";
import {
  _deployedWeaponSetupVisual,
  _drawShotRevealUnit,
  _drawUnit,
  _tankMotionVisual,
} from "../client/src/renderer/units.js";
import { ARTILLERY_DEPLOYED_WEAPON_ANIM_MS, DEPLOYED_WEAPON_ANIM_MS } from "../client/src/renderer/palette.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const baselinePath = path.join(repoRoot, "tests/fixtures/svg/legacy-unit-oracle.baseline.json");
const update = process.argv.includes("--update");
const fixedNow = 10_000;
const viewport = Object.freeze({
  width: 256,
  height: 256,
  cameraZoom: 1,
  devicePixelRatio: 1,
  scaleMode: "NEAREST",
  antialias: false,
  transparent: true,
});
const pixelDiffThresholds = Object.freeze({
  staticMinAlphaWeightedIdenticalRatio: 0.985,
  animatedMinAlphaWeightedIdenticalRatio: 0.96,
  staticMaxOpaqueClusterPx: 12,
  animatedMaxOpaqueClusterPx: 24,
  shadowAlphaTolerance: 0.04,
  shotRevealAlphaTolerance: 0.04,
  opaqueAlphaTolerance: 0.02,
  semanticAnchorToleranceWorldPx: 1.5,
  semanticBoundsToleranceWorldPx: 2,
});

function buildOracle() {
  const samples = [];
  for (const sample of buildSampleMatrix()) {
    samples.push(measureSample(sample));
  }
  return {
    temporaryMigrationScaffold: true,
    removeInPhase: 8,
    comment: "Temporary legacy renderer oracle for plans/svg. Delete this baseline with the equivalence harness in Phase 8.",
    viewport,
    pixelDiffThresholds,
    samples,
  };
}

function buildSampleMatrix() {
  const facings = [0, Math.PI / 2, Math.PI, Math.PI * 1.5];
  const teamColors = [PLAYER_PALETTE[0], PLAYER_PALETTE[5]];
  const samples = [];
  for (const kind of UNIT_KINDS) {
    for (const facing of facings) {
      for (const teamColor of teamColors) {
        samples.push(baseSample(kind, `facing-${angleName(facing)}-${teamColor}`, { facing, teamColor }));
      }
    }
  }

  for (const kind of weaponFacingKinds()) {
    for (const offset of [0, Math.PI / 4, -Math.PI / 2, Math.PI]) {
      samples.push(baseSample(kind, `weapon-offset-${offsetName(offset)}`, {
        facing: Math.PI / 2,
        weaponFacing: Math.PI / 2 + offset,
      }));
    }
  }

  for (const kind of recoilKinds()) {
    for (const recoilProgress of [0, 0.35, 1]) {
      samples.push(baseSample(kind, `recoil-${String(recoilProgress).replace(".", "_")}`, {
        facing: 0,
        weaponFacing: Math.PI / 4,
        recoilProgress,
      }));
    }
  }

  for (const kind of setupKinds()) {
    for (const [setupState, setupProgress] of [
      [SETUP.PACKED, 0],
      [SETUP.SETTING_UP, 0.5],
      [SETUP.DEPLOYED, 1],
      [SETUP.TEARING_DOWN, 0.5],
    ]) {
      samples.push(baseSample(kind, `setup-${setupState}-${setupProgress}`, {
        facing: 0,
        weaponFacing: Math.PI / 3,
        setupState,
        setupProgress,
      }));
    }
  }

  for (const [movement, previous] of Object.entries(vehicleMotionPreviousStates())) {
    for (const kind of vehicleMotionKinds()) {
      samples.push(baseSample(kind, `motion-${movement}`, {
        facing: Math.PI / 6,
        weaponFacing: Math.PI / 2,
        previous,
        state: STATE.MOVE,
      }));
    }
  }

  samples.push(baseSample(KIND.WORKER, "worker-busy-latched-node", { latchedNode: 9001 }));
  samples.push(baseSample(KIND.WORKER, "worker-busy-build-state", { state: STATE.BUILD }));
  samples.push(baseSample(KIND.TANK, "tank-low-oil", { resources: { oil: 3 }, state: STATE.MOVE }));
  samples.push(baseSample(KIND.TANK, "tank-oil-starved", { resources: { oil: 0 }, state: STATE.MOVE }));
  samples.push(baseSample(KIND.COMMAND_CAR, "command-breakthrough-on", { breakthroughTicks: 90 }));
  samples.push(baseSample(KIND.COMMAND_CAR, "command-breakthrough-off", { breakthroughTicks: 0 }));

  for (const alpha of ["fresh", "mid", "fade"]) {
    samples.push(baseSample(KIND.RIFLEMAN, `shot-reveal-${alpha}`, {
      shotReveal: true,
      shotRevealTiming: alpha,
      recoilProgress: 0.35,
    }));
  }

  return samples;
}

function baseSample(kind, label, overrides = {}) {
  return {
    label: `${kind}/${label}`,
    kind,
    owner: 1,
    teamColor: PLAYER_PALETTE[0],
    x: 128,
    y: 128,
    hp: Math.max(1, Math.round((STATS[kind]?.cost?.steel || 100) * 0.65)),
    maxHp: STATS[kind]?.cost?.steel || 100,
    state: STATE.IDLE,
    facing: 0,
    weaponFacing: undefined,
    recoilProgress: 0,
    setupState: SETUP.PACKED,
    setupProgress: 0,
    resources: { oil: 40 },
    breakthroughTicks: 0,
    shotReveal: false,
    shotRevealTiming: null,
    ...overrides,
  };
}

function measureSample(sample) {
  const renderer = makeRenderer(sample);
  const colorByOwner = new Map([[1, hexToInt(sample.teamColor)]]);
  const entity = {
    id: 100 + Math.abs(hashLabel(sample.label) % 10_000),
    kind: sample.kind,
    owner: sample.owner,
    x: sample.x,
    y: sample.y,
    hp: sample.hp,
    maxHp: sample.maxHp,
    state: sample.state,
    facing: sample.facing,
    setupState: sample.setupState,
    breakthroughTicks: sample.breakthroughTicks,
  };
  if (typeof sample.weaponFacing === "number") entity.weaponFacing = sample.weaponFacing;
  if (sample.latchedNode) entity.latchedNode = sample.latchedNode;
  if (sample.shotReveal) {
    entity.shotReveal = true;
    entity.shotRevealCreatedAt = fixedNow - shotRevealAge(sample.shotRevealTiming);
    entity.shotRevealExpiresAt = fixedNow + 1_000;
  }

  primeSetupVisual(renderer, entity, sample.setupProgress);
  primeVehicleMotion(renderer, entity, sample.previous);

  const state = {
    playerId: 1,
    players: [{ id: 1, color: sample.teamColor }],
    resources: sample.resources,
    weaponRecoil: () => sample.recoilProgress,
    isOwnOwner: (owner) => owner === 1,
    isAllyOwner: () => false,
    isNeutralOwner: (owner) => owner === 0,
  };

  if (sample.shotReveal) {
    renderer._drawShotRevealUnit(entity, colorByOwner, state);
  } else {
    renderer._drawUnit(entity, colorByOwner, state);
  }
  renderer._drawSelectionAndHp(entity, new Set([entity.id]), state);

  const layers = summarizeLayers(renderer);
  return sortObject({
    label: sample.label,
    kind: sample.kind,
    state: sample.state,
    facing: round(sample.facing),
    weaponFacing: round(typeof sample.weaponFacing === "number" ? sample.weaponFacing : sample.facing),
    recoilProgress: round(sample.recoilProgress),
    setupState: sample.setupState,
    setupProgress: round(sample.setupProgress),
    teamColor: sample.teamColor,
    semantic: {
      bounds: layers.combinedBounds,
      unitBounds: layers.units?.bounds || layers.shotReveals?.bounds || null,
      shadowBounds: layers.unitShadows?.bounds || layers.shotRevealShadows?.bounds || null,
      selectionRing: renderer._ringRadius(entity),
      hpBarAnchor: hpBarAnchor(entity),
      muzzleAnchor: muzzleAnchor(sample.kind, sample),
      facingTickEnd: facingTickEnd(sample.kind, sample),
      setupPartPositions: setupPartPositions(sample.kind, sample),
      movementPhase: movementPhase(renderer, entity),
      shotRevealAlpha: layers.shotReveals?.alpha ?? null,
    },
    layers,
  });
}

function makeRenderer(sample) {
  const layerNames = ["unitShadows", "units", "selectionRings", "hpBars", "shotRevealShadows", "shotReveals"];
  const layers = {};
  const pools = {};
  const seen = {};
  for (const name of layerNames) {
    layers[name] = new FakeLayer(name);
    pools[name] = new Map();
    seen[name] = new Set();
  }
  return {
    _pools: pools,
    _seen: seen,
    _setupVisuals: new Map(),
    _tankMotion: new Map(),
    _map: { tileSize: 32 },
    layers,
    sample,
    _deployedWeaponSetupVisual,
    _drawSelectionAndHp,
    _drawShotRevealUnit,
    _drawUnit,
    _hpBar,
    _ringRadius,
    _shadow,
    _slot,
    _tankMotionVisual,
    _tintFor,
    _vehicleShadow,
  };
}

class FakeLayer {
  constructor(name) {
    this.name = name;
    this.children = [];
  }

  addChild(child) {
    this.children.push(child);
  }
}

class FakeGraphics {
  constructor() {
    this.commands = [];
    this.alpha = 1;
    this.visible = true;
    this.lineWidth = 0;
    this.position = makePointSetter(this, "x", "y");
    this.scale = makePointSetter(this, "scaleX", "scaleY");
    this.x = 0;
    this.y = 0;
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
    this.lineWidth = width || 0;
    this.commands.push({ op: "lineStyle", width: this.lineWidth, color, alpha });
  }

  moveTo(x, y) {
    this.commands.push({ op: "moveTo", x, y, lineWidth: this.lineWidth });
  }

  lineTo(x, y) {
    this.commands.push({ op: "lineTo", x, y, lineWidth: this.lineWidth });
  }

  drawPolygon(points) {
    this.commands.push({ op: "drawPolygon", points: normalizePolygon(points), lineWidth: this.lineWidth });
  }

  drawEllipse(x, y, rx, ry) {
    this.commands.push({ op: "drawEllipse", x, y, rx, ry, lineWidth: this.lineWidth });
  }

  drawCircle(x, y, radius) {
    this.commands.push({ op: "drawCircle", x, y, radius, lineWidth: this.lineWidth });
  }

  drawRect(x, y, width, height) {
    this.commands.push({ op: "drawRect", x, y, width, height, lineWidth: this.lineWidth });
  }
}

function summarizeLayers(renderer) {
  const out = {};
  for (const [name, pool] of Object.entries(renderer._pools)) {
    const entries = [...pool.values()];
    if (entries.length === 0) continue;
    const graphics = entries[0];
    out[name] = {
      alpha: round(graphics.alpha),
      commandCount: graphics.commands.length,
      bounds: boundsForGraphics(graphics),
      ops: histogram(graphics.commands.map((cmd) => cmd.op)),
    };
  }
  out.combinedBounds = combineBounds(Object.values(out).map((entry) => entry.bounds).filter(Boolean));
  return out;
}

function boundsForGraphics(graphics) {
  const acc = emptyBounds();
  for (const cmd of graphics.commands) {
    const pad = (cmd.lineWidth || 0) * 0.5;
    if (cmd.op === "moveTo" || cmd.op === "lineTo") {
      includePoint(acc, cmd.x, cmd.y, pad);
    } else if (cmd.op === "drawPolygon") {
      for (let i = 0; i < cmd.points.length; i += 2) includePoint(acc, cmd.points[i], cmd.points[i + 1], pad);
    } else if (cmd.op === "drawEllipse") {
      includeRect(acc, cmd.x - cmd.rx, cmd.y - cmd.ry, cmd.x + cmd.rx, cmd.y + cmd.ry, pad);
    } else if (cmd.op === "drawCircle") {
      includeRect(acc, cmd.x - cmd.radius, cmd.y - cmd.radius, cmd.x + cmd.radius, cmd.y + cmd.radius, pad);
    } else if (cmd.op === "drawRect") {
      includeRect(acc, cmd.x, cmd.y, cmd.x + cmd.width, cmd.y + cmd.height, pad);
    }
  }
  if (!Number.isFinite(acc.minX)) return null;
  return {
    minX: round(acc.minX + graphics.x),
    minY: round(acc.minY + graphics.y),
    maxX: round(acc.maxX + graphics.x),
    maxY: round(acc.maxY + graphics.y),
    width: round(acc.maxX - acc.minX),
    height: round(acc.maxY - acc.minY),
  };
}

function comparePixelSnapshots(actual, expected, thresholds, mode = "static") {
  if (!actual || !expected || actual.width !== expected.width || actual.height !== expected.height) {
    return { passed: false, identicalRatio: 0, largestOpaqueClusterPx: Infinity };
  }
  let compared = 0;
  let identical = 0;
  const mismatches = new Set();
  for (let i = 0; i < actual.alpha.length; i += 1) {
    const a = actual.alpha[i];
    const e = expected.alpha[i];
    const tolerance = Math.max(a, e) < 0.5
      ? thresholds.shadowAlphaTolerance
      : thresholds.opaqueAlphaTolerance;
    if (Math.max(a, e) > 0) compared += Math.max(a, e);
    if (Math.abs(a - e) <= tolerance) {
      identical += Math.max(a, e);
    } else if (Math.max(a, e) >= 0.5) {
      mismatches.add(i);
    }
  }
  const identicalRatio = compared > 0 ? identical / compared : 1;
  const largestOpaqueClusterPx = largestCluster(mismatches, actual.width, actual.height);
  const minRatio = mode === "animated"
    ? thresholds.animatedMinAlphaWeightedIdenticalRatio
    : thresholds.staticMinAlphaWeightedIdenticalRatio;
  const maxCluster = mode === "animated"
    ? thresholds.animatedMaxOpaqueClusterPx
    : thresholds.staticMaxOpaqueClusterPx;
  return {
    passed: identicalRatio >= minRatio && largestOpaqueClusterPx <= maxCluster,
    identicalRatio: round(identicalRatio),
    largestOpaqueClusterPx,
  };
}

function exercisePixelDiffHelper() {
  const actual = { width: 2, height: 2, alpha: [1, 0.96, 0.28, 0] };
  const expected = { width: 2, height: 2, alpha: [1, 0.95, 0.3, 0] };
  assert.equal(comparePixelSnapshots(actual, expected, pixelDiffThresholds).passed, true);
}

function largestCluster(mismatches, width, height) {
  let largest = 0;
  const seen = new Set();
  for (const start of mismatches) {
    if (seen.has(start)) continue;
    let size = 0;
    const stack = [start];
    seen.add(start);
    while (stack.length > 0) {
      const index = stack.pop();
      size += 1;
      const x = index % width;
      const y = Math.floor(index / width);
      for (const [nx, ny] of [[x - 1, y], [x + 1, y], [x, y - 1], [x, y + 1]]) {
        if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
        const next = ny * width + nx;
        if (mismatches.has(next) && !seen.has(next)) {
          seen.add(next);
          stack.push(next);
        }
      }
    }
    largest = Math.max(largest, size);
  }
  return largest;
}

function hpBarAnchor(entity) {
  const stat = STATS[entity.kind] || {};
  if (vehicleMotionKinds().includes(entity.kind) || entity.kind === KIND.ANTI_TANK_GUN) {
    const body = stat.body || {};
    const halfLen = (body.length || 0) * 0.5;
    const halfWidth = (body.width || 0) * 0.5;
    const clearance = body.clearance || 0;
    return { x: entity.x, y: round(entity.y - Math.hypot(halfLen + clearance, halfWidth + clearance) - 8) };
  }
  const r = stat.size || 9;
  return { x: entity.x, y: round(entity.y - r - 8) };
}

function muzzleAnchor(kind, sample) {
  const stat = STATS[kind] || {};
  const r = stat.size || 9;
  const facing = typeof sample.weaponFacing === "number" ? sample.weaponFacing : sample.facing;
  const body = stat.body;
  if (kind === KIND.RIFLEMAN) return localPolar(facing, r * 1.35);
  if (kind === KIND.MACHINE_GUNNER) return localPolar(facing, r * 1.62);
  if (kind === KIND.ANTI_TANK_GUN) return localPolar(facing, r * 1.42);
  if (kind === KIND.MORTAR_TEAM) return localPolar(facing, r * 0.9);
  if (kind === KIND.ARTILLERY && body) return localPolar(facing, body.length * 0.91);
  if (kind === KIND.SCOUT_CAR && body) return localPolar(facing, body.length * 0.62);
  if (kind === KIND.TANK && body) return localPolar(facing, Math.max(body.length * 0.4, body.length * 0.5 + 8));
  return null;
}

function facingTickEnd(kind, sample) {
  if (weaponFacingKinds().includes(kind) || vehicleMotionKinds().includes(kind) || kind === KIND.ANTI_TANK_GUN) return null;
  const r = STATS[kind]?.size || 9;
  return localPolar(sample.facing, r + 3);
}

function setupPartPositions(kind, sample) {
  if (!setupKinds().includes(kind)) return null;
  const deploy = sample.setupState === SETUP.DEPLOYED
    ? 1
    : sample.setupState === SETUP.PACKED
      ? 0
      : sample.setupProgress;
  const r = STATS[kind]?.size || 18;
  return {
    deploy: round(deploy),
    leftFoot: localRotate(r * (0.52 + deploy * 0.3), -r * (0.12 + deploy * 0.34), sample.weaponFacing ?? sample.facing),
    rightFoot: localRotate(r * (0.52 + deploy * 0.3), r * (0.12 + deploy * 0.34), sample.weaponFacing ?? sample.facing),
  };
}

function movementPhase(renderer, entity) {
  const rec = renderer._tankMotion.get(entity.id);
  if (!rec) return null;
  return {
    leftPhase: round(rec.leftPhase),
    rightPhase: round(rec.rightPhase),
  };
}

function primeSetupVisual(renderer, entity, setupProgress) {
  const duration = entity.kind === KIND.ARTILLERY
    ? ARTILLERY_DEPLOYED_WEAPON_ANIM_MS
    : DEPLOYED_WEAPON_ANIM_MS;
  renderer._setupVisuals.set(entity.id, {
    state: entity.setupState || SETUP.PACKED,
    changedAt: fixedNow - duration * setupProgress,
  });
}

function primeVehicleMotion(renderer, entity, previous) {
  if (!previous) return;
  renderer._tankMotion.set(entity.id, {
    x: entity.x + previous.dx,
    y: entity.y + previous.dy,
    facing: previous.facing,
    leftPhase: previous.leftPhase,
    rightPhase: previous.rightPhase,
  });
}

function vehicleMotionPreviousStates() {
  return {
    idle: { dx: 0, dy: 0, facing: Math.PI / 6, leftPhase: 0, rightPhase: 0 },
    forward: { dx: -8, dy: 0, facing: Math.PI / 6, leftPhase: 4, rightPhase: 4 },
    reverse: { dx: 8, dy: 0, facing: Math.PI / 6, leftPhase: 8, rightPhase: 8 },
    pivot_left: { dx: 0, dy: 0, facing: Math.PI / 3, leftPhase: 12, rightPhase: 2 },
    pivot_right: { dx: 0, dy: 0, facing: -Math.PI / 8, leftPhase: 2, rightPhase: 12 },
  };
}

function weaponFacingKinds() {
  return [KIND.MACHINE_GUNNER, KIND.ANTI_TANK_GUN, KIND.MORTAR_TEAM, KIND.ARTILLERY, KIND.SCOUT_CAR, KIND.TANK];
}

function recoilKinds() {
  return [KIND.RIFLEMAN, KIND.MACHINE_GUNNER, KIND.ANTI_TANK_GUN, KIND.MORTAR_TEAM, KIND.ARTILLERY, KIND.SCOUT_CAR, KIND.TANK];
}

function setupKinds() {
  return [KIND.MACHINE_GUNNER, KIND.ANTI_TANK_GUN, KIND.MORTAR_TEAM, KIND.ARTILLERY];
}

function vehicleMotionKinds() {
  return [KIND.ARTILLERY, KIND.SCOUT_CAR, KIND.COMMAND_CAR, KIND.TANK];
}

function shotRevealAge(timing) {
  if (timing === "mid") return 500;
  if (timing === "fade") return 900;
  return 0;
}

function hexToInt(hex) {
  return parseInt(String(hex).replace("#", ""), 16);
}

function localPolar(a, d) {
  return { x: round(Math.cos(a) * d), y: round(Math.sin(a) * d) };
}

function localRotate(x, y, a) {
  return {
    x: round(x * Math.cos(a) - y * Math.sin(a)),
    y: round(x * Math.sin(a) + y * Math.cos(a)),
  };
}

function angleName(a) {
  return String(round(a)).replace(".", "_");
}

function offsetName(offset) {
  return String(round(offset)).replace("-", "neg_").replace(".", "_");
}

function hashLabel(label) {
  let hash = 0;
  for (let i = 0; i < label.length; i += 1) hash = ((hash << 5) - hash + label.charCodeAt(i)) | 0;
  return hash;
}

function histogram(values) {
  const counts = {};
  for (const value of values) counts[value] = (counts[value] || 0) + 1;
  return sortObject(counts);
}

function makePointSetter(target, xKey, yKey) {
  return {
    set(x, y = x) {
      target[xKey] = x;
      target[yKey] = y;
    },
  };
}

function normalizePolygon(points) {
  if (Array.isArray(points)) return points;
  return [...points];
}

function emptyBounds() {
  return { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity };
}

function includePoint(acc, x, y, pad = 0) {
  if (!Number.isFinite(x) || !Number.isFinite(y)) throw new Error(`non-finite render point ${x},${y}`);
  acc.minX = Math.min(acc.minX, x - pad);
  acc.minY = Math.min(acc.minY, y - pad);
  acc.maxX = Math.max(acc.maxX, x + pad);
  acc.maxY = Math.max(acc.maxY, y + pad);
}

function includeRect(acc, minX, minY, maxX, maxY, pad = 0) {
  includePoint(acc, minX, minY, pad);
  includePoint(acc, maxX, maxY, pad);
}

function combineBounds(bounds) {
  if (bounds.length === 0) return null;
  const acc = emptyBounds();
  for (const b of bounds) includeRect(acc, b.minX, b.minY, b.maxX, b.maxY);
  return {
    minX: round(acc.minX),
    minY: round(acc.minY),
    maxX: round(acc.maxX),
    maxY: round(acc.maxY),
    width: round(acc.maxX - acc.minX),
    height: round(acc.maxY - acc.minY),
  };
}

function round(value) {
  if (!Number.isFinite(value)) return value;
  const rounded = Number(value.toFixed(3));
  return Object.is(rounded, -0) ? 0 : rounded;
}

function sortObject(value) {
  if (Array.isArray(value)) return value.map(sortObject);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.keys(value).sort().map((key) => [key, sortObject(value[key])]));
}

function installPixiStub() {
  globalThis.PIXI = {
    Graphics: FakeGraphics,
    settings: { SCALE_MODE: "NEAREST" },
    SCALE_MODES: { NEAREST: "NEAREST" },
  };
}

function installDeterministicClock(now) {
  Object.defineProperty(globalThis, "performance", {
    configurable: true,
    value: { now: () => now },
  });
}

function main() {
  installPixiStub();
  installDeterministicClock(fixedNow);

  const oracle = buildOracle();

  if (update) {
    fs.mkdirSync(path.dirname(baselinePath), { recursive: true });
    fs.writeFileSync(baselinePath, `${JSON.stringify(oracle, null, 2)}\n`);
    console.log(`updated ${path.relative(repoRoot, baselinePath)} (${oracle.samples.length} samples)`);
  } else {
    const expected = JSON.parse(fs.readFileSync(baselinePath, "utf8"));
    assert.deepEqual(oracle, expected);
    exercisePixelDiffHelper();
    console.log(`legacy unit visual oracle passed (${oracle.samples.length} samples)`);
  }
}

main();
