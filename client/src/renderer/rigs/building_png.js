import { KIND } from "../../protocol.js";
import { RIG_SCHEMA_VERSION, validateRigDefinition } from "./schema.js";

const IDENTITY_TRANSFORM = Object.freeze({ x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1 });
const ZERO_POINT = Object.freeze({ x: 0, y: 0 });
const INVISIBLE_PAINT = Object.freeze({
  fill: null,
  stroke: null,
  strokeWidth: null,
  opacity: 1,
  fillOpacity: 1,
  strokeOpacity: 1,
});
const PLACEHOLDER_GEOMETRY = Object.freeze({ type: "rect", x: 0, y: 0, width: 1, height: 1 });

// Supply Depots and Tank Traps intentionally stay off this visual pass.
const BUILDING_PNG_SPECS = Object.freeze([
  [KIND.CITY_CENTRE, "city_centre", 3, 3, 384, 384],
  [KIND.BARRACKS, "barracks", 3, 2, 384, 256],
  [KIND.TRAINING_CENTRE, "training_centre", 3, 2, 384, 256],
  [KIND.RESEARCH_COMPLEX, "research_complex", 3, 3, 384, 384],
  [KIND.FACTORY, "factory", 3, 3, 384, 384],
  [KIND.STEELWORKS, "steelworks", 3, 3, 384, 384],
  [KIND.PUMP_JACK, "pump_jack", 1, 1, 128, 128],
]);
const B3_BUILDING_KINDS = new Set([
  KIND.FACTORY,
]);

function deepFreeze(value) {
  if (!value || typeof value !== "object" || Object.isFrozen(value)) return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

function part(id, drawOrder, tintSlot, opacity = 1) {
  return {
    id,
    drawOrder,
    geometry: PLACEHOLDER_GEOMETRY,
    transform: IDENTITY_TRANSFORM,
    pivot: ZERO_POINT,
    tintSlot,
    paint: opacity === 1 ? INVISIBLE_PAINT : { ...INVISIBLE_PAINT, opacity },
  };
}

function definition(kind, footW, footH) {
  const width = footW * 32;
  const height = footH * 32;
  const parts = [part("part.base", 10, "fixed"), part("part.tint", 11, "team")];
  if (kind === KIND.FACTORY) parts.push(part("part.shadow", 0, "fixed", 0.3));
  const validation = validateRigDefinition({
    id: `${kind}.building-raster`,
    kind,
    schemaVersion: RIG_SCHEMA_VERSION,
    parts,
    anchors: {
      origin: { x: 0, y: 0 },
      selection: { x: 0, y: 0 },
      hp: { x: 0, y: -height / 2 },
    },
    bounds: {
      selection: { x: -width / 2, y: -height / 2, width, height },
      hp: { x: -width * 0.4, y: -height / 2 - 7, width: width * 0.8, height: 6 },
    },
    animations: [],
    requiredRuntimeInputs: [],
  });
  if (!validation.ok) {
    throw new TypeError(`Invalid building PNG definition ${kind}: ${JSON.stringify(validation.errors)}`);
  }
  return deepFreeze(validation.definition);
}

function atlas(kind, slug, footW, footH, frameWidth, frameHeight) {
  const worldWidth = footW * 32;
  const worldHeight = footH * 32;
  const frame = (x) => ({
    x,
    y: 0,
    w: frameWidth,
    h: frameHeight,
    originX: frameWidth / 2,
    originY: frameHeight / 2,
    pixelsPerUnitX: frameWidth / worldWidth,
    pixelsPerUnitY: frameHeight / worldHeight,
  });
  const assetPass = B3_BUILDING_KINDS.has(kind)
    ? "buildings-b3-corrected-preview"
    : "buildings-b2-distinct-pass-01";
  const assetVersion = B3_BUILDING_KINDS.has(kind) ? "b3-corrected-03" : "b2-distinct-01";
  const sprites = [
    {
      id: "sprite.base",
      animationPart: "part.base",
      sourceParts: ["part.base"],
      tintSlot: "fixed",
      drawOrder: 10,
      frame: frame(0),
    },
    {
      id: "sprite.tint",
      animationPart: "part.tint",
      sourceParts: ["part.tint"],
      tintSlot: "team",
      drawOrder: 11,
      frame: frame(frameWidth),
    },
  ];
  if (kind === KIND.FACTORY) {
    sprites.push({
      id: "sprite.shadow",
      animationPart: "part.shadow",
      sourceParts: ["part.shadow"],
      tintSlot: "fixed",
      drawOrder: 0,
      frame: frame(frameWidth * 2),
    });
  }
  const columns = kind === KIND.FACTORY ? 3 : 2;
  return deepFreeze({
    enabled: true,
    unit: kind,
    image: `/assets/rigs/${assetPass}/${slug}-atlas.png?v=${assetVersion}`,
    viewBox: {
      x: -worldWidth / 2,
      y: -worldHeight / 2,
      width: worldWidth,
      height: worldHeight,
    },
    grid: {
      columns,
      rows: 1,
      width: frameWidth * columns,
      height: frameHeight,
    },
    sprites,
  });
}

const BUILDING_PNG_DEFINITIONS = new Map();
const BUILDING_PNG_ATLASES = new Map();
for (const [kind, slug, footW, footH, frameWidth, frameHeight] of BUILDING_PNG_SPECS) {
  BUILDING_PNG_DEFINITIONS.set(kind, definition(kind, footW, footH));
  BUILDING_PNG_ATLASES.set(kind, atlas(kind, slug, footW, footH, frameWidth, frameHeight));
}

export function createBuildingPngRigDefinitions() {
  return new Map(BUILDING_PNG_DEFINITIONS);
}

export function createBuildingPngRigAtlases() {
  return new Map(BUILDING_PNG_ATLASES);
}

export function buildingPngRigDefinitionFor(definitions, kind) {
  return definitions?.get?.(kind) ?? null;
}

export function buildingPngRigAtlasFor(atlases, kind) {
  return atlases?.get?.(kind) ?? null;
}
