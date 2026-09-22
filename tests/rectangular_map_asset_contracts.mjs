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

const u16le = (value) => {
  const buffer = Buffer.alloc(2);
  buffer.writeUInt16LE(value);
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
  ...(data.sun ? [
    Buffer.from("elevation"),
    Buffer.from(data.elevation),
    Buffer.from("sun"),
    u16le(data.sun.azimuthDegrees),
    Buffer.from([data.sun.elevationDegrees, data.sun.warmth]),
  ] : []),
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
  ["1v1-no-terrain.json", [126, 126, "198d24ba4349cdb226d4a3b709c8c4934fa97da208e1778163c5a6e0b26df31b", "091632cf7f075212"]],
  ["1v1.json", [126, 126, "a5977b0436ff36b3a91c80e58c037b10a9b7b4a4720b142f7ac06f098ac1206f", "e8c24a1df76a0aeb"]],
  ["3-player-map.json", [150, 150, "f0093e6e978f4fcaf3afc9468546c22e2951109fe41d18f95eefe19a8cebbc57", "63d94e5ea4153a57"]],
  ["4_player_map.json", [166, 166, "a72ca6141adf4148671a72c1eaba9ee60afba1ef117b4734b2e234a2141a4971", "9933e46fbf89cc17"]],
  ["default-handcrafted.json", [126, 126, "154d305ff61ffef65b2f1d5558ee55835635ed2f8e180f5fa9405688d20428fb", "d83c324277828e2e"]],
  ["schone-tage.json", [166, 166, "670af98b70ff3ee5edb320941f78d224e6863f0ab93f8dc169f6895c27b036c7", "4255cadc338383cc"]],
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
    [...map.noEntrenchmentTiles].sort((left, right) => left.y - right.y || left.x - right.x),
    expectedNoEntrenchment,
    `${fileName} marks every road tile and only road tiles as no-entrenchment regardless of serialization order`,
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
  ["fixed-roster-hellhole.json", [10, "37a3b26a9765b6f6", "dcca1927f8cc94ad"]],
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
