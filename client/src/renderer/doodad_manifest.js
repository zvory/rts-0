import { DOODAD_TYPE, TREE_DOODAD_GEOMETRY } from "../config.js";

export const MAX_DOODADS = 4096;

const TREE_PRESENTATION = Object.freeze({
  [DOODAD_TYPE.TREE_OAK]: Object.freeze({ shadowX: 34, shadowY: 9 }),
  [DOODAD_TYPE.TREE_PINE]: Object.freeze({ shadowX: 34, shadowY: 8 }),
  [DOODAD_TYPE.TREE_SPRUCE]: Object.freeze({ shadowX: 30, shadowY: 8 }),
  [DOODAD_TYPE.TREE_ALDER]: Object.freeze({ shadowX: 27, shadowY: 8 }),
});

const entries = {};
for (const [typeId, geometry] of Object.entries(TREE_DOODAD_GEOMETRY)) {
  const species = typeId.slice("tree.".length);
  entries[typeId] = treeEntry({
    typeId,
    image: `/assets/doodads/tree-${species}.png`,
    ...geometry,
    ...TREE_PRESENTATION[typeId],
  });
}

entries[DOODAD_TYPE.WILDFLOWER_SINGLE] = Object.freeze({
  typeId: DOODAD_TYPE.WILDFLOWER_SINGLE,
  image: "/assets/doodads/wildflower-single.png",
  layer: "understory",
  widthPx: 15,
  heightPx: 22,
  anchorY: 0.95,
  tintable: true,
});
entries[DOODAD_TYPE.WILDFLOWER_CLUSTER] = Object.freeze({
  typeId: DOODAD_TYPE.WILDFLOWER_CLUSTER,
  image: "/assets/doodads/wildflower-cluster.png",
  layer: "understory",
  widthPx: 31,
  heightPx: 25,
  anchorY: 0.92,
  tintable: true,
});

export const DOODAD_MANIFEST = Object.freeze(entries);
// Entity-backed authored objects (currently Tank Traps) are intentionally absent: the Map Editor
// previews them on the building layer and live matches render their authoritative entity snapshot.
export const DOODAD_TYPE_IDS = Object.freeze(Object.keys(DOODAD_MANIFEST));

export function doodadManifestEntry(typeId) {
  return DOODAD_MANIFEST[typeId] || null;
}

function treeEntry({
  typeId,
  image,
  widthPx,
  heightPx,
  anchorY,
  shadowX,
  shadowY,
  shadowOffsetY = 1,
}) {
  return Object.freeze({
    typeId,
    image,
    layer: "canopy",
    widthPx,
    heightPx,
    anchorY,
    shadow: Object.freeze({ radiusX: shadowX, radiusY: shadowY, offsetY: shadowOffsetY }),
  });
}
