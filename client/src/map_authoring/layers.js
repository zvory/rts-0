export const MAP_AUTHORING_LAYER = Object.freeze({
  BASE: "base",
  FOREST: "forest",
  CONCEALMENT: "concealment",
  NO_VEHICLE: "no-vehicle",
  NO_BUILDING: "no-building",
  NO_ENTRENCHMENT: "no-entrenchment",
  DAMAGE_REDUCTION: "damage-reduction",
  SLOW_MOVEMENT: "slow-movement",
  TREES: "trees",
  GAMEPLAY_DOODADS: "gameplay-doodads",
  DECORATIVE_DOODADS: "decorative-doodads",
});

export const MAP_AUTHORING_LAYERS = Object.freeze([
  Object.freeze({
    id: MAP_AUTHORING_LAYER.BASE,
    label: "Terrain & bases",
    description: "Terrain, start locations, and base sites",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.FOREST,
    label: "Forest",
    description: "Composite tiles with trees and all five gameplay effects",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.CONCEALMENT,
    label: "Concealment",
    description: "Tiles that conceal units",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.NO_VEHICLE,
    label: "No vehicles",
    description: "Tiles blocked to vehicles",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.NO_BUILDING,
    label: "No buildings",
    description: "Tiles blocked to building placement",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.NO_ENTRENCHMENT,
    label: "No entrenchment",
    description: "Tiles where infantry cannot dig or occupy trenches",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.DAMAGE_REDUCTION,
    label: "Damage reduction",
    description: "Tiles that reduce incoming damage by 25%",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.SLOW_MOVEMENT,
    label: "Slowed movement",
    description: "Tiles that reduce movement speed by 25%",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.TREES,
    label: "Trees",
    description: "Authored tree visuals and trunks",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.GAMEPLAY_DOODADS,
    label: "Gameplay doodads",
    description: "Mechanically meaningful objects such as Tank Traps",
  }),
  Object.freeze({
    id: MAP_AUTHORING_LAYER.DECORATIVE_DOODADS,
    label: "Decorative doodads",
    description: "Mechanically inert decoration such as wildflowers",
  }),
]);

export const MAP_AUTHORING_LAYER_IDS = Object.freeze(MAP_AUTHORING_LAYERS.map(({ id }) => id));

const LAYER_IDS = new Set(MAP_AUTHORING_LAYER_IDS);
const TREE_TYPE_PREFIX = "tree.";
const DECORATIVE_DOODAD_TYPE_PREFIX = "wildflower.";

export function defaultMapAuthoringLayerVisibility(visible = true) {
  return Object.fromEntries(MAP_AUTHORING_LAYER_IDS.map((id) => [id, !!visible]));
}

export function normalizeMapAuthoringLayerVisibility(source, { fallback = true } = {}) {
  const visibility = defaultMapAuthoringLayerVisibility(fallback);
  if (!source || typeof source !== "object" || Array.isArray(source)) return visibility;
  for (const id of MAP_AUTHORING_LAYER_IDS) {
    if (typeof source[id] === "boolean") visibility[id] = source[id];
  }
  return visibility;
}

export function validateMapAuthoringLayerVisibility(source) {
  if (!source || typeof source !== "object" || Array.isArray(source)) {
    throw new TypeError("Map authoring layer visibility must be an object");
  }
  for (const key of Object.keys(source)) {
    if (!LAYER_IDS.has(key)) throw new RangeError(`Unsupported map authoring layer ${JSON.stringify(key)}`);
  }
  for (const id of MAP_AUTHORING_LAYER_IDS) {
    if (typeof source[id] !== "boolean") {
      throw new TypeError(`Map authoring layer ${JSON.stringify(id)} visibility must be boolean`);
    }
  }
  return source;
}

export function mapAuthoringLayerVisibilityFromSelection(selection) {
  if (selection == null || selection === "" || selection === "all") {
    return defaultMapAuthoringLayerVisibility(true);
  }
  const values = typeof selection === "string"
    ? selection.split(",").map((value) => value.trim()).filter(Boolean)
    : Array.from(selection || []);
  const visibility = defaultMapAuthoringLayerVisibility(false);
  for (const id of values) {
    if (!LAYER_IDS.has(id)) {
      throw new RangeError(
        `Unsupported map authoring layer ${JSON.stringify(id)}; expected ${MAP_AUTHORING_LAYER_IDS.join(", ")}`,
      );
    }
    visibility[id] = true;
  }
  return visibility;
}

export function mapAuthoringDoodadLayer(typeId) {
  if (typeof typeId === "string" && typeId.startsWith(TREE_TYPE_PREFIX)) {
    return MAP_AUTHORING_LAYER.TREES;
  }
  if (typeof typeId === "string" && typeId.startsWith(DECORATIVE_DOODAD_TYPE_PREFIX)) {
    return MAP_AUTHORING_LAYER.DECORATIVE_DOODADS;
  }
  return MAP_AUTHORING_LAYER.GAMEPLAY_DOODADS;
}

export function mapAuthoringDoodadVisible(record, visibility) {
  return visibility?.[mapAuthoringDoodadLayer(record?.typeId)] !== false;
}
