// Client mirror of the server-validated static MapInfo doodad vocabulary. Keep the values in
// canonical server order so protocol parity detects additions, removals, and ordering drift.
export const DOODAD_TYPE = Object.freeze({
  TREE_OAK: "tree.oak",
  TREE_PINE: "tree.pine",
  TREE_BIRCH: "tree.birch",
  TREE_SPRUCE: "tree.spruce",
  TREE_ASPEN: "tree.aspen",
  TREE_ALDER: "tree.alder",
  TREE_OAK_TOPDOWN: "tree.oak.topdown",
  TREE_PINE_TOPDOWN: "tree.pine.topdown",
  TREE_BIRCH_TOPDOWN: "tree.birch.topdown",
  TREE_SPRUCE_TOPDOWN: "tree.spruce.topdown",
  TREE_ASPEN_TOPDOWN: "tree.aspen.topdown",
  TREE_ALDER_TOPDOWN: "tree.alder.topdown",
  WILDFLOWER_SINGLE: "wildflower.single",
  WILDFLOWER_CLUSTER: "wildflower.cluster",
});

export const DOODAD_TYPE_IDS = Object.freeze(Object.values(DOODAD_TYPE));
