import { DOODAD_TYPE, DOODAD_TYPE_IDS as CONFIG_DOODAD_TYPE_IDS } from "../config.js";

export const MAX_DOODADS = 4096;

const TREE_SPECS = Object.freeze({
  oak: Object.freeze({ widthPx: 86, heightPx: 112, anchorY: 0.94, windAmplitude: 0.025, windRate: 0.00115, shadowX: 25, shadowY: 9 }),
  pine: Object.freeze({ widthPx: 76, heightPx: 122, anchorY: 0.96, windAmplitude: 0.018, windRate: 0.001, shadowX: 22, shadowY: 8 }),
  birch: Object.freeze({ widthPx: 70, heightPx: 108, anchorY: 0.95, windAmplitude: 0.032, windRate: 0.0013, shadowX: 20, shadowY: 7 }),
  spruce: Object.freeze({ widthPx: 74, heightPx: 126, anchorY: 0.96, windAmplitude: 0.016, windRate: 0.00095, shadowX: 22, shadowY: 8 }),
  aspen: Object.freeze({ widthPx: 66, heightPx: 112, anchorY: 0.95, windAmplitude: 0.034, windRate: 0.00135, shadowX: 19, shadowY: 7 }),
  alder: Object.freeze({ widthPx: 82, heightPx: 102, anchorY: 0.94, windAmplitude: 0.027, windRate: 0.0012, shadowX: 24, shadowY: 8 }),
});

const TOPDOWN_SPECS = Object.freeze({
  oak: Object.freeze({ widthPx: 96, heightPx: 88, anchorY: 0.54 }),
  pine: Object.freeze({ widthPx: 84, heightPx: 98, anchorY: 0.56 }),
  birch: Object.freeze({ widthPx: 78, heightPx: 76, anchorY: 0.53 }),
  spruce: Object.freeze({ widthPx: 88, heightPx: 104, anchorY: 0.56 }),
  aspen: Object.freeze({ widthPx: 74, heightPx: 82, anchorY: 0.54 }),
  alder: Object.freeze({ widthPx: 92, heightPx: 84, anchorY: 0.53 }),
});

const entries = {};
for (const [species, spec] of Object.entries(TREE_SPECS)) {
  const typeId = `tree.${species}`;
  entries[typeId] = treeEntry({
    typeId,
    image: `/assets/doodads/tree-${species}.png`,
    ...spec,
  });
  const topdown = TOPDOWN_SPECS[species];
  const topdownTypeId = `${typeId}.topdown`;
  entries[topdownTypeId] = treeEntry({
    typeId: topdownTypeId,
    image: `/assets/doodads/tree-${species}-topdown.png`,
    ...topdown,
    windAmplitude: spec.windAmplitude * 0.45,
    windRate: spec.windRate,
    shadowX: Math.round(topdown.widthPx * 0.3),
    shadowY: Math.round(topdown.heightPx * 0.12),
    shadowOffsetY: 0,
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
export const DOODAD_TYPE_IDS = CONFIG_DOODAD_TYPE_IDS;

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
