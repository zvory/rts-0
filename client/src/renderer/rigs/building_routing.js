// Building rig routing — mirrors live_routing.js for unit rigs.
//
// Compiles one SVG string per building kind into a RigDefinition at startup.
// _drawBuilding looks up the definition by kind, then uses renderLiveUnitRig
// to draw the body through the pooled buildingRigs layer. Buildings are static
// (no animation bindings), so only tint slots are evaluated at runtime.

import { KIND } from "../../protocol.js";
import { compileSvgRig } from "./svg_importer.js";
import {
  BARRACKS_BUILDING_SVG,
  CITY_CENTRE_BUILDING_SVG,
  DEPOT_BUILDING_SVG,
  FACTORY_BUILDING_SVG,
  RESEARCH_COMPLEX_BUILDING_SVG,
  STEELWORKS_BUILDING_SVG,
  TRAINING_CENTRE_BUILDING_SVG,
  ZAMOK_BUILDING_SVG,
} from "./building_svg.js";

const BUILDING_RIG_SOURCES = Object.freeze([
  [KIND.CITY_CENTRE,       CITY_CENTRE_BUILDING_SVG],
  [KIND.ZAMOK,             ZAMOK_BUILDING_SVG],
  [KIND.DEPOT,             DEPOT_BUILDING_SVG],
  [KIND.BARRACKS,          BARRACKS_BUILDING_SVG],
  [KIND.TRAINING_CENTRE,   TRAINING_CENTRE_BUILDING_SVG],
  [KIND.RESEARCH_COMPLEX,  RESEARCH_COMPLEX_BUILDING_SVG],
  [KIND.FACTORY,           FACTORY_BUILDING_SVG],
  [KIND.STEELWORKS,        STEELWORKS_BUILDING_SVG],
]);

/**
 * Compile all building SVG strings into RigDefinitions.
 * Called once at Renderer construction. Failures warn and skip the kind,
 * causing _drawBuilding to fall back to the imperative rect path.
 * @returns {Map<string, import("./schema.js").RigDefinition>}
 */
export function createBuildingRigDefinitions() {
  const definitions = new Map();
  for (const [kind, svgText] of BUILDING_RIG_SOURCES) {
    const compiled = compileSvgRig(svgText, { expectedKind: kind });
    if (compiled.ok) {
      definitions.set(kind, compiled.definition);
    } else {
      console.warn(`RTS building rig disabled for ${kind}:`, compiled.errors);
    }
  }
  return definitions;
}

/**
 * @param {Map<string, object>} definitions
 * @param {string} kind
 * @returns {object|null}
 */
export function buildingRigDefinitionFor(definitions, kind) {
  return definitions?.get?.(kind) ?? null;
}
