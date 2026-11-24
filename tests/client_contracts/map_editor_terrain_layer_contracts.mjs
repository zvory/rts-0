import assert from "node:assert/strict";
import fs from "node:fs";

import { TERRAIN } from "../../client/src/protocol.js";
import { MapEditorSession } from "../../client/src/map_editor_session.js";

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const tile = { x: 16, y: 16 };
  assert.equal("terrain" in session.draft, false, "Map Editor drafts do not retain the mixed legacy terrain grid");
  assert.equal(session.draft.ground[tile.y][tile.x], ".");
  assert.equal(session.draft.features[tile.y][tile.x], ".");

  session.beginTerrainStroke();
  session.paintTerrainTiles([tile], TERRAIN.DIRT_B);
  session.paintTerrainTiles([tile], TERRAIN.WATER);
  assert.deepEqual(session.paintTerrainTiles([tile], TERRAIN.MUD_A), [],
    "cosmetic ground never replaces or creates hidden state underneath a semantic feature");
  assert.equal(session.commitTerrainStroke(), true);
  assert.equal(session.draft.ground[tile.y][tile.x], ".");
  assert.equal(session.draft.features[tile.y][tile.x], "~");
  assert.equal(session.materialized().terrain[tile.y * 32 + tile.x], TERRAIN.WATER,
    "semantic features remain authoritative in the materialized terrain grid");

  session.beginTerrainStroke();
  assert.deepEqual(session.paintTerrainTiles([tile], TERRAIN.GRASS, { eraseFeature: true }), [
    { ...tile, code: TERRAIN.GRASS },
  ]);
  assert.equal(session.commitTerrainStroke(), true);
  assert.equal(session.materialized().terrain[tile.y * 32 + tile.x], TERRAIN.GRASS,
    "erasing a feature reveals canonical open ground");
  const exported = session.exportMap();
  assert.equal(exported.terrain[tile.y][tile.x], ".");
  assert.equal("ground" in exported, false);
  assert.equal("features" in exported, false);
}

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const tile = { x: 16, y: 16 };
  session.draft.features[tile.y] = `${".".repeat(tile.x)}=${".".repeat(31 - tile.x)}`;
  session.draft.noEntrenchmentTiles = [];
  session.beginTerrainStroke();
  assert.deepEqual(session.paintTerrainTiles([tile], TERRAIN.ROAD_BARE), [
    { ...tile, code: TERRAIN.ROAD_BARE },
  ], "repainting an unchanged road reports the restored automatic overlay");
  assert.deepEqual(session.draft.noEntrenchmentTiles, [tile],
    "repainting an imported road restores its automatic no-entrenchment overlay");
}

{
  const mapDirectory = new URL("../../server/assets/maps/", import.meta.url);
  for (const filename of fs.readdirSync(mapDirectory).filter((name) => name.endsWith(".json"))) {
    const source = JSON.parse(fs.readFileSync(new URL(filename, mapDirectory), "utf8"));
    const session = new MapEditorSession({ storage: null });
    session.loadAuthoredMap(source);
    assert.deepEqual(session.exportMap().unwalkableTiles, source.unwalkableTiles || []);
    assert.deepEqual(session.exportMap().terrain, source.terrain,
      `${filename} recomposes its legacy terrain rows byte-for-byte after the internal layer split`);
  }
}

console.log("map_editor_terrain_layer_contracts: ok");

{
  const session = new MapEditorSession({ storage: null });
  session.initializeBlank({ size: 32, playerCount: 2 });
  const tile = { x: 16, y: 16 };
  const terrain = session.exportMap().terrain;
  session.beginOverlayStroke();
  session.paintOverlayTiles([tile], { unwalkable: true });
  assert.equal(session.commitOverlayStroke(), true);
  assert.deepEqual(session.materialized().unwalkableTiles, [tile]);
  assert.deepEqual(session.exportMap().terrain, terrain, "unwalkable annotation preserves ground art");
  const exported = session.exportMap();
  session.undo();
  assert.deepEqual(session.draft.unwalkableTiles, []);
  session.redo();
  assert.deepEqual(session.draft.unwalkableTiles, [tile]);
  const loaded = new MapEditorSession({ storage: null });
  loaded.loadAuthoredMap(exported);
  assert.deepEqual(loaded.materialized().unwalkableTiles, [tile]);
  session.beginOverlayStroke();
  session.paintOverlayTiles([tile], { unwalkable: false });
  session.commitOverlayStroke();
  assert.deepEqual(session.exportMap().unwalkableTiles, []);
}
