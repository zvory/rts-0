import assert from "node:assert/strict";
import fs from "node:fs";

const repoRoot = new URL("../", import.meta.url);
const source = JSON.parse(fs.readFileSync(new URL("server/assets/maps/1v1.json", repoRoot), "utf8"));
const wide = JSON.parse(fs.readFileSync(new URL("server/assets/maps/1v1-wide.json", repoRoot), "utf8"));

const sourceWidth = source.terrain[0].length;
const sourceHeight = source.terrain.length;
const horizontalMargin = sourceWidth / 2;

assert.equal(wide.version, 5, "rectangular demo uses the explicit-dimensions map schema");
assert.equal(wide.name, "1v1 Wide", "rectangular demo remains selectable by its stable name");
assert.equal(wide.width, sourceWidth * 2, "rectangular demo doubles the original horizontal extent");
assert.equal(wide.height, sourceHeight, "rectangular demo preserves the original vertical extent");
assert.equal(wide.terrain.length, wide.height, "terrain row count matches the declared height");
assert(wide.terrain.every((row) => row.length === wide.width), "every terrain row matches the declared width");
assert.equal(horizontalMargin, 63, "the original 126-wide battlefield stays centered between equal margins");

for (let y = 0; y < sourceHeight; y += 1) {
  const row = wide.terrain[y];
  assert.equal(row.slice(0, horizontalMargin), ".".repeat(horizontalMargin), `row ${y} has a grass-only left margin`);
  assert.equal(row.slice(horizontalMargin, horizontalMargin + sourceWidth), source.terrain[y], `row ${y} preserves the original terrain exactly`);
  assert.equal(row.slice(horizontalMargin + sourceWidth), ".".repeat(horizontalMargin), `row ${y} has a grass-only right margin`);
}

const shiftedLocations = (locations) => locations.map((location) => ({
  ...location,
  x: location.x + horizontalMargin,
}));

assert.deepEqual(
  wide.startLocations,
  shiftedLocations(source.startLocations),
  "player starts retain their geometry and move only with the centered battlefield",
);
assert.deepEqual(
  wide.baseSites,
  shiftedLocations(source.baseSites),
  "resource base sites retain their counts and geometry and move only with the centered battlefield",
);

const sourceCoordinateCollections = Object.entries(source)
  .filter(([, value]) => Array.isArray(value) && value.some((entry) => entry && typeof entry === "object" && "x" in entry && "y" in entry))
  .map(([key]) => key)
  .sort();
assert.deepEqual(
  sourceCoordinateCollections,
  ["baseSites", "startLocations"],
  "the source map has no additional coordinate collections that the wide fixture forgot to preserve",
);

console.log("✅ rectangular_map_asset_contracts.mjs: rectangular demo asset is an exact centered extension");
