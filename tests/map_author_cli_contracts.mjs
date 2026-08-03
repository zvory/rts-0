import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { buildMapFromRecipe, renderPreviewSvg, validateMap } from "../scripts/map-author.mjs";

const recipe = {
  name: "CLI Contract",
  width: 32,
  height: 32,
  symmetry: "halfTurn",
  operations: [
    { type: "blob", material: "water", center: [3, 3], radius: [4, 3], roughness: 0.3, seed: 7 },
    // Off-angle roads are intentionally accepted. Their desirability belongs to the author.
    { type: "road", points: [[2, 8], [17, 13]], width: 3 },
    { type: "start", at: [3, 3] },
  ],
};

const map = buildMapFromRecipe(recipe);
assert.equal(map.version, 6);
assert.equal(map.width, 32);
assert.equal(map.height, 32);
assert.equal(map.startLocations.length, 2);
assert.equal(map.baseSites.length, 2);
assert.deepEqual(map.stealthTiles, []);
assert.deepEqual(map.noVehicleTiles, []);
for (let y = 0; y < map.height; y += 1) {
  for (let x = 0; x < map.width; x += 1) {
    assert.equal(map.terrain[y][x], map.terrain[map.height - 1 - y][map.width - 1 - x]);
  }
}

const validation = validateMap(map, { symmetry: "halfTurn" });
assert(validation.warnings.some((warning) => warning.includes("protected area")), "blocked base clearance is advisory");
assert(!validation.warnings.some((warning) => warning.includes("symmetry mismatches")), "generated terrain preserves symmetry");

const preview = renderPreviewSvg(map, { tilePixels: 3 });
assert(preview.startsWith("<svg"));
assert(preview.includes(">1</text>"));

const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-map-author-test-"));
try {
  const recipePath = path.join(tempRoot, "recipe.json");
  const mapPath = path.join(tempRoot, "map.json");
  const previewPath = path.join(tempRoot, "preview.svg");
  fs.writeFileSync(recipePath, JSON.stringify(recipe));

  const build = spawnSync(process.execPath, ["scripts/map-author.mjs", "build", recipePath, "--output", mapPath], {
    cwd: new URL("../", import.meta.url),
    encoding: "utf8",
  });
  assert.equal(build.status, 0, build.stderr);
  assert(build.stdout.includes("map was not rejected"), "build reports advisory warnings without failing");
  assert(fs.existsSync(mapPath));

  const validate = spawnSync(process.execPath, ["scripts/map-author.mjs", "validate", mapPath, "--symmetry", "halfTurn"], {
    cwd: new URL("../", import.meta.url),
    encoding: "utf8",
  });
  assert.equal(validate.status, 0, validate.stderr);
  assert(validate.stdout.includes("protected area"));

  const render = spawnSync(process.execPath, ["scripts/map-author.mjs", "preview", mapPath, "--output", previewPath], {
    cwd: new URL("../", import.meta.url),
    encoding: "utf8",
  });
  assert.equal(render.status, 0, render.stderr);
  assert(fs.readFileSync(previewPath, "utf8").startsWith("<svg"));
} finally {
  fs.rmSync(tempRoot, { recursive: true, force: true });
}

console.log("✅ map_author_cli_contracts.mjs: permissive recipe build, advisory validation, and preview");
