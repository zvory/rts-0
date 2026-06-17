import { KIND } from "../../protocol.js";
import { compileSvgRig } from "./svg_importer.js";
import { TANK_RIG_SVG } from "./tank_svg.js";
import { WORKER_RIG_SVG } from "./worker_svg.js";

const LIVE_RIG_SOURCES = Object.freeze([
  [KIND.TANK, TANK_RIG_SVG],
  [KIND.WORKER, WORKER_RIG_SVG],
]);

const WORKER_UNIT_PARTS = Object.freeze([
  "part.body",
  "part.busyIndicator",
  "part.facingTick",
]);

const TANK_UNIT_PARTS = Object.freeze([
  "part.track.left",
  "part.track.right",
  "part.tread.left.0",
  "part.tread.left.1",
  "part.tread.left.2",
  "part.tread.left.3",
  "part.tread.left.4",
  "part.tread.left.5",
  "part.tread.left.6",
  "part.tread.left.7",
  "part.tread.left.8",
  "part.tread.right.0",
  "part.tread.right.1",
  "part.tread.right.2",
  "part.tread.right.3",
  "part.tread.right.4",
  "part.tread.right.5",
  "part.tread.right.6",
  "part.tread.right.7",
  "part.tread.right.8",
  "part.hull",
  "part.hull.shadow",
  "part.hull.nose",
  "part.hull.noseShadow",
  "part.barrel",
  "part.turret",
  "part.noseTick",
  "part.fuelCue.box",
  "part.fuelCue.x1",
  "part.fuelCue.x2",
]);

const LIVE_RIG_PARTS = Object.freeze({
  [KIND.TANK]: Object.freeze({
    shadow: Object.freeze(["part.shadow"]),
    unit: TANK_UNIT_PARTS,
  }),
  [KIND.WORKER]: Object.freeze({
    shadow: Object.freeze(["part.shadow"]),
    unit: WORKER_UNIT_PARTS,
  }),
});

export function createLiveRigDefinitions() {
  const definitions = new Map();
  for (const [kind, svgText] of LIVE_RIG_SOURCES) {
    const compiled = compileSvgRig(svgText, { expectedKind: kind });
    if (compiled.ok) definitions.set(kind, compiled.definition);
    else console.warn(`RTS live rig disabled for ${kind}: ${JSON.stringify(compiled.errors)}`);
  }
  return definitions;
}

export function liveRigDefinitionFor(definitions, kind) {
  return definitions?.get?.(kind) ?? null;
}

export function liveRigRoutesFor(kind, pools = {}) {
  const parts = LIVE_RIG_PARTS[kind];
  if (!parts) return [];
  return [
    {
      poolName: pools.liveRigShadow || "liveUnitRigShadows",
      layerName: pools.shadow || "unitShadows",
      parts: parts.shadow,
    },
    {
      poolName: pools.liveRigUnit || "liveUnitRigs",
      layerName: pools.unit || "units",
      parts: parts.unit,
    },
  ];
}
