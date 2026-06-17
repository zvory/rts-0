#!/usr/bin/env node
import fs from "node:fs";
import { KIND } from "../client/src/protocol.js";
import {
  _deployedWeaponSetupVisual,
  _drawUnit,
  _rigRenderContextFor,
  _tankMotionVisual,
  createLegacyUnitPartCapture,
} from "../client/src/renderer/units.js";
import { _shadow, _tintFor, _vehicleShadow } from "../client/src/renderer/entities.js";
import { SVG_MIGRATION_MANIFESTS_BY_KIND } from "../tests/fixtures/svg/unit_migration_manifests.mjs";

function main() {
  const args = parseArgs(process.argv.slice(2));
  const kind = args.kind ?? KIND.WORKER;
  const manifest = SVG_MIGRATION_MANIFESTS_BY_KIND[kind];
  if (!manifest) {
    fail(`No SVG migration manifest for ${kind}. Available: ${Object.keys(SVG_MIGRATION_MANIFESTS_BY_KIND).join(", ")}`);
  }

  const baseline = JSON.parse(fs.readFileSync("tests/fixtures/svg/legacy-unit-oracle.baseline.json", "utf8"));
  const sampleLabel = args.sample ?? manifest.requiredSamples[0];
  const sample = baseline.samples.find((entry) => entry.label === sampleLabel);
  if (!sample) fail(`Sample ${sampleLabel} was not found in tests/fixtures/svg/legacy-unit-oracle.baseline.json`);
  if (sample.kind !== kind) fail(`Sample ${sampleLabel} is ${sample.kind}, not requested kind ${kind}`);

  const entity = entityFromSample(sample);
  const colorByOwner = new Map([[entity.owner, parseInt(sample.teamColor.slice(1), 16)]]);
  const state = stateFromSample(sample);
  const renderer = makeRenderer();
  primeVisualState(renderer, entity, sample);

  const parts = manifest.partMappings.map((mapping) => dumpPart(mapping, renderer, entity, colorByOwner, state));
  const out = {
    kind,
    sample: sample.label,
    note: "Debug metadata from the legacy procedural unit renderer. Use as a drafting aid, not as an SVG source of truth.",
    entity,
    parts,
  };

  console.log(JSON.stringify(out, null, 2));
}

function dumpPart(mapping, renderer, entity, colorByOwner, state) {
  renderer.reset();
  const capture = createLegacyUnitPartCapture({ includeParts: [mapping.legacyPart] });
  _drawUnit.call(renderer, entity, colorByOwner, state, {
    partCapture: capture,
    skipLiveRig: true,
    skipRigComparison: true,
  });
  const layers = Object.fromEntries(
    Object.entries(renderer._pools)
      .filter(([, pool]) => pool.has(entity.id))
      .map(([name, pool]) => [name, pool.get(entity.id).snapshot()]),
  );
  return {
    legacyPart: mapping.legacyPart,
    rigParts: mapping.rigParts,
    gates: {
      busyOnly: Boolean(mapping.busyOnly),
      fuelOnly: Boolean(mapping.fuelOnly),
    },
    thresholds: mapping.thresholds,
    captureRecords: capture.records,
    layers,
  };
}

function entityFromSample(sample) {
  const entity = {
    id: 100 + Math.abs(hashLabel(sample.label) % 10_000),
    kind: sample.kind,
    owner: sample.owner ?? 1,
    teamColor: sample.teamColor,
    x: 48,
    y: 48,
    hp: sample.hp ?? 32,
    maxHp: sample.maxHp ?? 50,
    state: sample.state,
    setupState: sample.setupState,
    facing: sample.facing,
  };
  if (typeof sample.weaponFacing === "number") entity.weaponFacing = sample.weaponFacing;
  if (sample.label.includes("latched-node")) entity.latchedNode = 9001;
  if (typeof sample.breakthroughTicks === "number") entity.breakthroughTicks = sample.breakthroughTicks;
  return entity;
}

function stateFromSample(sample) {
  return {
    playerId: 1,
    resources: sample.resources ?? { oil: 40 },
    weaponRecoil: () => sample.recoilProgress ?? 0,
    isOwnOwner: (owner) => owner === 1,
    isAllyOwner: () => false,
    isNeutralOwner: (owner) => owner === 0,
  };
}

function makeRenderer() {
  return {
    _pools: { unitShadows: new Map(), units: new Map() },
    _seen: { unitShadows: new Set(), units: new Set() },
    _setupVisuals: new Map(),
    _tankMotion: new Map(),
    _map: { tileSize: 32 },
    layers: { unitShadows: new FakeContainer(), units: new FakeContainer() },
    reset() {
      this._pools = { unitShadows: new Map(), units: new Map() };
      this._seen = { unitShadows: new Set(), units: new Set() };
      this.layers = { unitShadows: new FakeContainer(), units: new FakeContainer() };
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
    _shadow,
    _vehicleShadow,
    _tintFor,
    _deployedWeaponSetupVisual,
    _tankMotionVisual,
    _rigRenderContextFor,
  };
}

function primeVisualState(renderer, entity, sample) {
  if (typeof sample.setupProgress === "number" && sample.setupProgress > 0) {
    const duration = entity.kind === KIND.ARTILLERY ? 1_100 : 800;
    renderer._setupVisuals.set(entity.id, {
      state: entity.setupState,
      startedAt: 10_000 - duration * sample.setupProgress,
    });
  }
  if (sample.previous) renderer._tankMotion.set(entity.id, sample.previous);
}

class FakeContainer {
  constructor() {
    this.children = [];
  }

  addChild(child) {
    this.children.push(child);
    return child;
  }
}

class FakeGraphics {
  constructor() {
    this.commands = [];
    this.visible = true;
    this.alpha = 1;
    this.position = {
      x: 0,
      y: 0,
      set: (x, y) => {
        this.position.x = x;
        this.position.y = y;
        this.commands.push({ op: "position.set", x, y });
      },
    };
  }

  clear() {
    this.commands.push({ op: "clear" });
  }

  lineStyle(width = 0, color = 0, alpha = 1) {
    this.commands.push({ op: "lineStyle", width, color: hex(color), alpha });
  }

  beginFill(color = 0, alpha = 1) {
    this.commands.push({ op: "beginFill", color: hex(color), alpha });
  }

  endFill() {
    this.commands.push({ op: "endFill" });
  }

  moveTo(x, y) {
    this.commands.push({ op: "moveTo", x: round(x), y: round(y) });
  }

  lineTo(x, y) {
    this.commands.push({ op: "lineTo", x: round(x), y: round(y) });
  }

  arc(x, y, radius, startAngle, endAngle) {
    this.commands.push({ op: "arc", x: round(x), y: round(y), radius: round(radius), startAngle: round(startAngle), endAngle: round(endAngle) });
  }

  drawCircle(x, y, radius) {
    this.commands.push({ op: "drawCircle", x: round(x), y: round(y), radius: round(radius) });
  }

  drawEllipse(x, y, width, height) {
    this.commands.push({ op: "drawEllipse", x: round(x), y: round(y), width: round(width), height: round(height) });
  }

  drawPolygon(points) {
    this.commands.push({ op: "drawPolygon", points: points.map(round) });
  }

  drawRect(x, y, width, height) {
    this.commands.push({ op: "drawRect", x: round(x), y: round(y), width: round(width), height: round(height) });
  }

  snapshot() {
    return {
      position: { x: round(this.position.x), y: round(this.position.y) },
      visible: this.visible,
      alpha: this.alpha,
      commands: this.commands,
    };
  }
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--kind") out.kind = argv[++i];
    else if (arg === "--sample") out.sample = argv[++i];
    else if (arg === "--help" || arg === "-h") {
      console.log("usage: node scripts/dump-legacy-unit-parts.mjs --kind worker --sample worker/facing-0-#0072b2");
      process.exit(0);
    } else fail(`Unknown argument: ${arg}`);
  }
  return out;
}

function hashLabel(label) {
  let hash = 0;
  for (let i = 0; i < label.length; i += 1) hash = ((hash << 5) - hash + label.charCodeAt(i)) | 0;
  return hash;
}

function hex(value) {
  return `#${Number(value).toString(16).padStart(6, "0")}`;
}

function round(value) {
  return Number.isFinite(value) ? Math.round(value * 1000) / 1000 : value;
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

main();
