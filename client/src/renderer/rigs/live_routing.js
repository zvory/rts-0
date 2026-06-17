import { KIND } from "../../protocol.js";
import { compileSvgRig } from "./svg_importer.js";
import { WORKER_RIG_SVG } from "./worker_svg.js";

const LIVE_RIG_SOURCES = Object.freeze([
  [KIND.WORKER, WORKER_RIG_SVG],
]);

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
