import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";

const repoRoot = new URL("../", import.meta.url);

const fnv1a64 = (chunks) => {
  let hash = 0xcbf29ce484222325n;
  for (const chunk of chunks) {
    for (const byte of chunk) {
      hash ^= BigInt(byte);
      hash = BigInt.asUintN(64, hash * 0x100000001b3n);
    }
  }
  return hash.toString(16).padStart(16, "0");
};

const u32le = (value) => {
  const buffer = Buffer.alloc(4);
  buffer.writeUInt32LE(value);
  return buffer;
};

const usizeLe = (value) => {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(BigInt(value));
  return buffer;
};

const materializedHash = (data) => fnv1a64([
  u32le(data.width),
  u32le(data.height),
  Buffer.from(data.terrain),
  usizeLe(data.starts.length),
  ...data.starts.flatMap(({ x, y }) => [u32le(x), u32le(y)]),
  usizeLe(data.baseSites.length),
  ...data.baseSites.flatMap(({ x, y, steelPatches, oilPatches }) => [
    u32le(x),
    u32le(y),
    u32le(steelPatches),
    u32le(oilPatches),
  ]),
]);

const bundledMapContracts = new Map([
  ["1v1-no-terrain.json", [126, 126, "43229a90f176eca98bc846369c23829ec21ef651110c6130f60cd44064e0f493", "2e5f921c35ca3592"]],
  ["1v1.json", [126, 126, "dc1f3578b9b8e59dddef9dad876a43873771efac6d7cff010b65a6088f30c91d", "dab5c2b76de92943"]],
  ["3-player-map.json", [150, 150, "c22766d5f1a8eb1a5e8aad19ac9e37c9cf0204a57d407bb7bb2f730726f2d8d0", "b59f1702ca3bd0fb"]],
  ["4_player_map.json", [166, 166, "c32bc4413eba9485473d53942be5d816c00214a2382930367f38d4188e86534a", "aba66d9bf954174f"]],
  ["default-handcrafted.json", [126, 126, "7b496141deab0dd8b0dd85b13dfc5386da21d4c3ef628530296a50264a8fbf20", "37a3b26a9765b6f6"]],
  ["schone-tage.json", [166, 166, "f6707fa21414bfedbaa3b055e1f0551d75692f2952cb359a67e67a54cb1cf564", "f2d57ce723a85120"]],
]);

for (const fileName of fs.readdirSync(new URL("server/assets/maps/", repoRoot)).filter((name) => name.endsWith(".json"))) {
  const map = JSON.parse(fs.readFileSync(new URL(`server/assets/maps/${fileName}`, repoRoot), "utf8"));
  assert.equal(map.version, 10, `${fileName} uses the no-entrenchment schema`);
  assert(Array.isArray(map.forestSpans), `${fileName} declares forestSpans`);
  assert(Array.isArray(map.noBuildingTiles), `${fileName} declares noBuildingTiles`);
  assert(Array.isArray(map.noEntrenchmentTiles), `${fileName} declares noEntrenchmentTiles`);
  const expectedNoEntrenchment = [];
  for (let y = 0; y < map.height; y += 1) {
    for (let x = 0; x < map.width; x += 1) {
      if ("=-|\\/".includes(map.terrain[y][x])) expectedNoEntrenchment.push({ x, y });
    }
  }
  assert.deepEqual(
    map.noEntrenchmentTiles,
    expectedNoEntrenchment,
    `${fileName} marks every road tile and only road tiles as no-entrenchment`,
  );
}

for (const [fileName, [width, height, contentDigest, authoredHash]] of bundledMapContracts) {
  const rawMap = fs.readFileSync(new URL(`server/assets/maps/${fileName}`, repoRoot));
  const map = JSON.parse(rawMap);
  assert.equal(map.version, 10, `${fileName} uses the no-entrenchment schema`);
  assert(Array.isArray(map.noBuildingTiles), `${fileName} declares noBuildingTiles`);
  assert(Array.isArray(map.noEntrenchmentTiles), `${fileName} declares noEntrenchmentTiles`);
  assert.equal(map.width, width, `${fileName} preserves its inferred terrain width`);
  assert.equal(map.height, height, `${fileName} preserves its inferred terrain height`);
  assert.equal(map.terrain.length, height, `${fileName} terrain row count matches height`);
  assert(map.terrain.every((row) => row.length === width), `${fileName} terrain rows match width`);

  const preservedContent = {
    terrain: map.terrain,
    startLocations: map.startLocations,
    baseSites: map.baseSites,
  };
  assert.equal(
    crypto.createHash("sha256").update(JSON.stringify(preservedContent)).digest("hex"),
    contentDigest,
    `${fileName} preserves terrain and coordinate collections exactly`,
  );
  assert.equal(fnv1a64([rawMap]), authoredHash, `${fileName} authored content hash is stable`);
}

const bundledScenarioContracts = new Map([
  ["lategame.json", [9, "62f5e3ae24627171", "7918f89f6178e9c9"]],
  ["render-preview.json", [7, "9e6169128d81ed61", "f82d4bf8967c50c9"]],
  ["fixed-roster-hellhole.json", [7, "4638fcdac85871c7", "b8b0dd056c34c92d"]],
  ["tank-trap-cluster-clear.json", [7, "9e6169128d81ed61", "7918f89f6178e9c9"]],
]);

for (const [fileName, [schemaVersion, contentHash, expectedMaterializedHash]] of bundledScenarioContracts) {
  const scenario = JSON.parse(fs.readFileSync(new URL(`server/assets/lab-scenarios/${fileName}`, repoRoot), "utf8"));
  const checkpoint = JSON.parse(scenario.checkpointPayload);
  assert.equal(scenario.map.schemaVersion, schemaVersion, `${fileName} binds its authored map schema`);
  assert.equal(scenario.map.contentHash, contentHash, `${fileName} binds the exact authored map bytes`);
  assert.equal(scenario.map.data.width, 126, `${fileName} preserves its map width`);
  assert.equal(scenario.map.data.height, 126, `${fileName} preserves its map height`);
  assert.equal(scenario.map.data.terrain.length, 126 * 126, `${fileName} terrain matches its declared area`);
  assert.equal("size" in scenario.map.data, false, `${fileName} no longer carries a square-only map size`);
  assert.equal(checkpoint.fog.width, 126, `${fileName} checkpoint fog preserves its width`);
  assert.equal(checkpoint.fog.height, 126, `${fileName} checkpoint fog preserves its height`);
  assert.equal("size" in checkpoint.fog, false, `${fileName} checkpoint fog no longer carries a square-only size`);
  assert.equal(materializedHash(scenario.map.data), expectedMaterializedHash, `${fileName} materialized map hash matches its rectangular data`);
  assert.equal(scenario.map.materializedHash, expectedMaterializedHash, `${fileName} outer materialized hash is current`);
  assert.deepEqual(
    checkpoint.mapBinding,
    {
      name: scenario.map.name,
      schemaVersion,
      contentHash,
      materializedMapHash: expectedMaterializedHash,
      width: 126,
      height: 126,
      playerCount: scenario.map.data.starts.length,
    },
    `${fileName} checkpoint binding matches the migrated scenario map`,
  );
}

console.log("✅ rectangular_map_asset_contracts.mjs: bundled assets use explicit map dimensions");
