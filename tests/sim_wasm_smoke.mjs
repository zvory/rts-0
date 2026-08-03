import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { DEFAULT_FACTION_ID } from "../client/src/protocol.js";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wasmAssetDir = process.env.RTS_SIM_WASM_ASSET_DIR
  ? path.resolve(process.env.RTS_SIM_WASM_ASSET_DIR)
  : path.join(repoRoot, "client/vendor/sim-wasm");
const gluePath = path.join(wasmAssetDir, "rts_sim_wasm.js");
const wasmPath = path.join(wasmAssetDir, "rts_sim_wasm_bg.wasm");
const maxHeapDelta = Number(process.env.RTS_SIM_WASM_SMOKE_MAX_HEAP_DELTA || 2_000_000);

function assert(cond, msg) {
  if (!cond) throw new Error(msg || "Assertion failed");
}

if (!fs.existsSync(gluePath) || !fs.existsSync(wasmPath)) {
  throw new Error("generated WASM assets missing; run scripts/build-sim-wasm.sh first");
}

const { default: init, WasmPredictor } = await import(`file://${gluePath}`);
await init({ module_or_path: fs.readFileSync(wasmPath) });

const start = {
  playerId: 1,
  spectator: false,
  tick: 0,
  map: {
    width: 64,
    height: 64,
    tileSize: 32,
    terrain: new Array(64 * 64).fill(0),
    resources: [],
  },
  players: [
    {
      id: 1,
      teamId: 1,
      factionId: DEFAULT_FACTION_ID,
      name: "A",
      color: "#f00",
      startTileX: 5,
      startTileY: 5,
    },
    {
      id: 2,
      teamId: 1,
      factionId: DEFAULT_FACTION_ID,
      name: "B",
      color: "#0f0",
      startTileX: 8,
      startTileY: 5,
    },
    {
      id: 3,
      teamId: 2,
      factionId: DEFAULT_FACTION_ID,
      name: "C",
      color: "#00f",
      startTileX: 50,
      startTileY: 50,
    },
  ],
};
const baseline = {
  tick: 0,
  playerId: 1,
  steel: 75,
  oil: 0,
  supplyUsed: 1,
  supplyCap: 10,
  ownedEntities: [
    {
      id: 101,
      kind: "worker",
      x: 100,
      y: 100,
      hp: 40,
      maxHp: 40,
      state: "idle",
    },
    {
      id: 201,
      kind: "resource_depot",
      x: 240,
      y: 240,
      hp: 1000,
      maxHp: 1000,
      state: "idle",
    },
  ],
  progress: [
    {
      id: 201,
      kind: "production",
      identity: "unit:worker",
      fraction: 0.25,
      totalTicks: 495,
    },
  ],
  visibleObstacles: [
    {
      kind: "rifleman",
      x: 140,
      y: 100,
      radius: 9,
    },
  ],
};

if (global.gc) global.gc();
const before = process.memoryUsage().heapUsed;

const predictor = WasmPredictor.fromStartJson(JSON.stringify(start), 1);
predictor.importBaselineJson(JSON.stringify(baseline));
const beforeCommand = JSON.parse(predictor.renderPredictionFrameJson(0.5));
predictor.enqueueCommandJson(1, JSON.stringify({ c: "move", units: [101], x: 580, y: 100 }));
const afterCommand = JSON.parse(predictor.renderPredictionFrameJson(0.5));
assert(afterCommand.tick === beforeCommand.tick, "command enqueue does not advance global prediction time");
assert(
  JSON.stringify(afterCommand.progress) === JSON.stringify(beforeCommand.progress),
  "command enqueue does not advance progress",
);
predictor.advanceTicks(300);

const rendered = JSON.parse(predictor.renderPredictionFrameJson(0.5));
const diagnostics = JSON.parse(predictor.diagnosticsJson());
assert(rendered.tick === 300, `expected tick 300, got ${rendered.tick}`);
assert(rendered.entities.length === 1, "expected one owned prediction patch");
assert(rendered.entities[0].id === 101, "prediction patch identifies the existing owned entity");
assert(rendered.entities[0].x > 100, "worker advanced along move command");
assert(
  Object.keys(rendered.entities[0]).every((key) => ["id", "x", "y", "facing", "motion"].includes(key)),
  "prediction patch remains capability-scoped to pose and explicit motion",
);
assert(rendered.progress.length === 1, "expected one sparse progress patch");
assert(rendered.progress[0].id === 201, "progress patch identifies an authoritative owned building");
assert(
  rendered.progress[0].fraction > 0.25 && rendered.progress[0].fraction <= 0.98001,
  "progress extrapolation advances without claiming completion",
);
assert(diagnostics.pendingCommands === 1, "pending command diagnostics survive smoke");

predictor.free();
if (global.gc) global.gc();
const heapDelta = process.memoryUsage().heapUsed - before;
assert(
  heapDelta <= maxHeapDelta,
  `heap delta ${heapDelta} exceeded limit ${maxHeapDelta}`,
);

console.log(`sim_wasm_smoke: ok heapDelta=${heapDelta}`);
