// Client mirror of the server-validated static MapInfo doodad vocabulary. Keep the values in
// canonical server order so protocol parity detects additions, removals, and ordering drift.
export const DOODAD_TYPE = Object.freeze({
  TREE_OAK: "tree.oak",
  TREE_PINE: "tree.pine",
  TREE_BIRCH: "tree.birch",
  TREE_SPRUCE: "tree.spruce",
  TREE_ASPEN: "tree.aspen",
  TREE_ALDER: "tree.alder",
  WILDFLOWER_SINGLE: "wildflower.single",
  WILDFLOWER_CLUSTER: "wildflower.cluster",
});

export const DOODAD_TYPE_IDS = Object.freeze(Object.values(DOODAD_TYPE));
