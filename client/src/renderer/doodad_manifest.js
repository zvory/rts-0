import { DOODAD_TYPE } from "../config.js";

export const MAX_DOODADS = 4096;

const TREE_SPECS = Object.freeze({
  oak: Object.freeze({ widthPx: 119, heightPx: 112, anchorY: 0.94, windAmplitude: 0.025, windRate: 0.00115, shadowX: 34, shadowY: 9 }),
  pine: Object.freeze({ widthPx: 120, heightPx: 122, anchorY: 0.96, windAmplitude: 0.018, windRate: 0.001, shadowX: 34, shadowY: 8 }),
  spruce: Object.freeze({ widthPx: 103, heightPx: 126, anchorY: 0.96, windAmplitude: 0.016, windRate: 0.00095, shadowX: 30, shadowY: 8 }),
  alder: Object.freeze({ widthPx: 93, heightPx: 102, anchorY: 0.94, windAmplitude: 0.027, windRate: 0.0012, shadowX: 27, shadowY: 8 }),
});

const entries = {};
for (const [species, spec] of Object.entries(TREE_SPECS)) {
  const typeId = `tree.${species}`;
  entries[typeId] = treeEntry({
    typeId,
    image: `/assets/doodads/tree-${species}.png`,
    ...spec,
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
  windAmplitude: 0.065,
  windRate: 0.0018,
});
entries[DOODAD_TYPE.WILDFLOWER_CLUSTER] = Object.freeze({
  typeId: DOODAD_TYPE.WILDFLOWER_CLUSTER,
  image: "/assets/doodads/wildflower-cluster.png",
  layer: "understory",
  widthPx: 31,
  heightPx: 25,
  anchorY: 0.92,
  tintable: true,
  windAmplitude: 0.045,
  windRate: 0.00155,
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
  windAmplitude,
  windRate,
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
    windAmplitude,
    windRate,
    shadow: Object.freeze({ radiusX: shadowX, radiusY: shadowY, offsetY: shadowOffsetY }),
  });
}
