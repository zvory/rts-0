// Client mirror of the server-validated static MapInfo doodad vocabulary. Keep the values in
// canonical server order so protocol parity detects additions, removals, and ordering drift.
export const DOODAD_TYPE = Object.freeze({
  TREE_OAK: "tree.oak",
  TREE_PINE: "tree.pine",
  TREE_SPRUCE: "tree.spruce",
  TREE_ALDER: "tree.alder",
  WILDFLOWER_SINGLE: "wildflower.single",
  WILDFLOWER_CLUSTER: "wildflower.cluster",
  TANK_TRAP: "unit.tank_trap",
});

export const DOODAD_TYPE_IDS = Object.freeze(Object.values(DOODAD_TYPE));

// Authored tree geometry is shared by deterministic forest placement and rendering. Foliage
// bounds are world-pixel offsets from the grounded root anchor at the renderer's base size; they
// follow the visible canopies in the four 128px source sprites rather than their transparent PNG
// rectangles. Keeping this here prevents authoring from guessing independently of presentation.
export const TREE_DOODAD_GEOMETRY = Object.freeze({
  [DOODAD_TYPE.TREE_OAK]: treeGeometry(119, 112, 0.94, -55, 55, -91, -18),
  [DOODAD_TYPE.TREE_PINE]: treeGeometry(120, 122, 0.96, -55, 55, -108, -19),
  [DOODAD_TYPE.TREE_SPRUCE]: treeGeometry(103, 126, 0.96, -39, 39, -111, -10),
  [DOODAD_TYPE.TREE_ALDER]: treeGeometry(93, 102, 0.94, -40, 40, -88, -18),
});

export function doodadSizeVariation(id) {
  return 0.92 + stableNoise(id, 31) * 0.16;
}

function treeGeometry(widthPx, heightPx, anchorY, left, right, top, bottom) {
  return Object.freeze({
    widthPx,
    heightPx,
    anchorY,
    foliage: Object.freeze({ left, right, top, bottom }),
  });
}

function stableNoise(id, salt) {
  let value = (Math.imul(id | 0, 0x45d9f3b) ^ salt) >>> 0;
  value = Math.imul(value ^ (value >>> 16), 0x45d9f3b) >>> 0;
  value ^= value >>> 16;
  return value / 0xffffffff;
}
