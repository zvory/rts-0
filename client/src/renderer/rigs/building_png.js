import { STATS } from "../../config.js";
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
const RUNTIME_TILE_SIZE = 32;

function buildingSpec(kind, frameWidth, frameHeight, image, {
  silhouetteShadow = true,
  emblem = false,
} = {}) {
  return Object.freeze({ kind, frameWidth, frameHeight, image, silhouetteShadow, emblem });
}

// Supply Depots and Tank Traps intentionally stay off this visual pass.
const BUILDING_PNG_SPECS = Object.freeze([
  buildingSpec(KIND.RESOURCE_DEPOT, 384, 384,
    "/assets/rigs/resource-depot-worksite-preview/resource_depot-atlas.png?v=oil-silo-foundry-worksite-preview-08",
    { emblem: true }),
  buildingSpec(KIND.BARRACKS, 384, 256,
    "/assets/rigs/building-emblems-preview/barracks-atlas-m14-team-tint.png?v=building-emblems-preview-04",
    { emblem: true }),
  buildingSpec(KIND.TRAINING_CENTRE, 384, 256,
    "/assets/rigs/building-emblems-preview/training_centre-atlas-mg42-panzerfaust-team-tint.png?v=building-emblems-preview-06",
    { emblem: true }),
  buildingSpec(KIND.ENGINEERING_COMPLEX, 384, 384,
    "/assets/rigs/building-emblems-preview/engineering_complex-atlas-team-tint.png?v=building-emblems-preview-03",
    { emblem: true }),
  buildingSpec(KIND.FACTORY, 384, 384,
    "/assets/rigs/building-emblems-preview/factory-atlas-team-tint.png?v=building-emblems-preview-03",
    { emblem: true }),
  buildingSpec(KIND.STEELWORKS, 384, 384,
    "/assets/rigs/building-emblems-preview/steelworks-atlas-team-tint.png?v=building-emblems-preview-03",
    { emblem: true }),
  buildingSpec(KIND.PUMP_JACK, 128, 128,
    "/assets/rigs/buildings-b2-distinct-pass-01/pump_jack-atlas.png?v=b2-distinct-01",
    { silhouetteShadow: false }),
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

function buildingFootprint(kind) {
  const { footW, footH } = STATS[kind] || {};
  if (!Number.isInteger(footW) || footW <= 0 || !Number.isInteger(footH) || footH <= 0) {
    throw new TypeError(`Missing building footprint for PNG rig ${kind}`);
  }
  return { footW, footH };
}

function definition(spec) {
  const { kind, silhouetteShadow, emblem } = spec;
  const { footW, footH } = buildingFootprint(kind);
  const width = footW * RUNTIME_TILE_SIZE;
  const height = footH * RUNTIME_TILE_SIZE;
  const parts = [part("part.base", 10, "fixed"), part("part.tint", 11, "team")];
  if (emblem) parts.push(part("part.emblem", 12, "team"));
  if (silhouetteShadow) parts.push(part("part.shadow", 0, "fixed", 0.3));
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

function atlas(spec) {
  const { kind, frameWidth, frameHeight, image, silhouetteShadow, emblem } = spec;
  const { footW, footH } = buildingFootprint(kind);
  const worldWidth = footW * RUNTIME_TILE_SIZE;
  const worldHeight = footH * RUNTIME_TILE_SIZE;
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
  const bodyParts = ["part.base", "part.tint"];
  const shadowParts = [];
  let nextColumn = 2;
  if (silhouetteShadow) {
    sprites.push({
      id: "sprite.shadow",
      animationPart: "part.shadow",
      sourceParts: ["part.shadow"],
      tintSlot: "fixed",
      drawOrder: 0,
      frame: frame(frameWidth * nextColumn),
    });
    shadowParts.push("part.shadow");
    nextColumn += 1;
  }
  if (emblem) {
    sprites.push({
      id: "sprite.emblem",
      animationPart: "part.emblem",
      sourceParts: ["part.emblem"],
      tintSlot: "team",
      drawOrder: 12,
      frame: frame(frameWidth * nextColumn),
    });
    bodyParts.push("part.emblem");
    nextColumn += 1;
  }
  const columns = nextColumn;
  return deepFreeze({
    enabled: true,
    unit: kind,
    image,
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
    routes: {
      body: bodyParts,
      shadow: shadowParts,
    },
    sprites,
  });
}

const BUILDING_PNG_DEFINITIONS = new Map();
const BUILDING_PNG_ATLASES = new Map();
for (const spec of BUILDING_PNG_SPECS) {
  BUILDING_PNG_DEFINITIONS.set(spec.kind, definition(spec));
  BUILDING_PNG_ATLASES.set(spec.kind, atlas(spec));
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
