// Authored ground decal asset and deterministic selection contracts.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

import { assert, assertApprox, assertDeepEqual } from "./assertions.mjs";
import { GROUND_DECAL_ASSET_MANIFEST } from "../../client/src/renderer/decals/manifest.js";
import { GROUND_DECAL_PNG_ATLAS } from "../../client/src/renderer/decals/atlas.generated.js";
import {
  loadGroundDecalAtlas,
  validateAtlasCoverage,
} from "../../client/src/renderer/decals/asset_loader.js";
import { createGroundDecalStampPlan } from "../../client/src/renderer/decals/selection.js";
import { KIND } from "../../client/src/protocol.js";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const allAssets = [
  ...GROUND_DECAL_ASSET_MANIFEST.infantry,
  ...GROUND_DECAL_ASSET_MANIFEST.vehicleScorch,
  ...GROUND_DECAL_ASSET_MANIFEST.vehiclePaint,
  ...GROUND_DECAL_ASSET_MANIFEST.mortarBlast,
  ...GROUND_DECAL_ASSET_MANIFEST.artilleryBlast,
];

assert(validateAtlasCoverage(GROUND_DECAL_ASSET_MANIFEST, GROUND_DECAL_PNG_ATLAS),
  "generated PNG atlas covers every authored source in deterministic manifest order");
const atlasPath = path.join(repoRoot, "client", GROUND_DECAL_PNG_ATLAS.url.slice(1));
const atlasPng = PNG.sync.read(fs.readFileSync(atlasPath));
assert(atlasPng.width === GROUND_DECAL_PNG_ATLAS.width && atlasPng.height === GROUND_DECAL_PNG_ATLAS.height,
  "checked-in PNG atlas dimensions match generated rect metadata");

{
  let fetched = "";
  let closed = 0;
  let decodeOptions = null;
  const atlas = await loadGroundDecalAtlas({
    fetchFn: async (url) => {
      fetched = url;
      return { ok: true, blob: async () => ({ type: "image/png" }) };
    },
    createImageBitmapFn: async (_blob, options) => {
      decodeOptions = options;
      return ({
      width: GROUND_DECAL_PNG_ATLAS.width,
      height: GROUND_DECAL_PNG_ATLAS.height,
      close() { closed += 1; },
      });
    },
  });
  assert(fetched === GROUND_DECAL_PNG_ATLAS.url, "runtime fetches only the worker-decodable PNG atlas");
  assert(decodeOptions?.premultiplyAlpha === "premultiply" && decodeOptions?.colorSpaceConversion === "none",
    "worker bitmap decoding pins premultiplied alpha and source colors for exact DOM-canvas parity");
  assert(atlas.infantry.length === GROUND_DECAL_ASSET_MANIFEST.infantry.length,
    "runtime readiness exposes every infantry source rect");
  assert(atlas.artilleryBlast[0].image === atlas.infantry[0].image,
    "all mask rects share one decoded worker-owned bitmap");
  atlas.destroy();
  atlas.destroy();
  assert(closed === 1, "atlas teardown closes its ImageBitmap exactly once");
}

assert(GROUND_DECAL_ASSET_MANIFEST.infantry.length === 8,
  "manifest includes four rifleman and four machine-gunner death sprites");
assert(GROUND_DECAL_ASSET_MANIFEST.vehicleScorch.length >= 8, "manifest includes at least eight vehicle scorch masks");
assert(GROUND_DECAL_ASSET_MANIFEST.vehiclePaint.length >= 8, "manifest includes at least eight vehicle paint masks");
assert(GROUND_DECAL_ASSET_MANIFEST.mortarBlast.length >= 1, "manifest includes a mortar blast decal mask");
assert(GROUND_DECAL_ASSET_MANIFEST.artilleryBlast.length >= 1, "manifest includes an artillery blast decal mask");

for (const asset of allAssets) {
  assert(asset.url.startsWith("/assets/decals/"), `decal ${asset.id} is served from the client asset path`);
  assert(Number.isInteger(asset.width) && asset.width > 0, `decal ${asset.id} declares a positive width`);
  assert(Number.isInteger(asset.height) && asset.height > 0, `decal ${asset.id} declares a positive height`);

  const localPath = path.join(repoRoot, "client", asset.url.slice(1));
  assert(fs.existsSync(localPath), `decal ${asset.id} exists at ${asset.url}`);
  if (path.extname(localPath).toLowerCase() === ".png") {
    const png = PNG.sync.read(fs.readFileSync(localPath));
    assert(png.width === asset.width && png.height === asset.height,
      `decal ${asset.id} PNG dimensions match its manifest entry`);
    const alphaValues = png.data.filter((_value, index) => index % 4 === 3);
    assert(alphaValues.some((alpha) => alpha === 0) && alphaValues.some((alpha) => alpha > 0),
      `decal ${asset.id} PNG preserves transparent cutout pixels`);
    continue;
  }
  const svg = fs.readFileSync(localPath, "utf8");
  assert(/<svg\b/i.test(svg), `decal ${asset.id} is an SVG file`);
  assert(/\bviewBox="[^"]+"/.test(svg), `decal ${asset.id} has an explicit viewBox`);
  assert(!/<script\b/i.test(svg), `decal ${asset.id} does not include script tags`);
  assert(!/<(?:image|use|foreignObject)\b/i.test(svg), `decal ${asset.id} does not include external-capable elements`);
  assert(!/\b(?:href|xlink:href)\s*=/i.test(svg), `decal ${asset.id} does not include href references`);
  assert(!/url\(/i.test(svg), `decal ${asset.id} does not include CSS url references`);
  assert(
    !/<(?:filter|mask|clipPath|linearGradient|radialGradient|pattern)\b/i.test(svg),
    `decal ${asset.id} avoids expensive or inconsistent SVG paint features`,
  );
  assert(!/\bfill="(?!#fff")/i.test(svg), `decal ${asset.id} uses white alpha-mask fills only`);
}

{
  const decal = {
    id: 77,
    kind: KIND.RIFLEMAN,
    decalClass: "infantry",
    color: "#4878c8",
    facing: 1.25,
    seed: 123456789,
  };
  const plan = createGroundDecalStampPlan(decal);
  const repeat = createGroundDecalStampPlan({ ...decal });
  assertDeepEqual(plan, repeat, "infantry decal selection is deterministic for a fixed seed");
  assert(plan.color === 0x4878c8, "infantry decal tint uses the recovered owner player color");
  assert(
    plan.variantIndex === (decal.seed % 4),
    "rifleman death sprite comes from the first four deterministic variants",
  );
  assert(plan.scale === 1, "infantry deaths keep their normalized authored body scale");
  assert(plan.opacity === 0.94, "infantry death sprites stay readable during their hold interval");
  assert(plan.shadowOpacity >= 0.14 && plan.shadowOpacity <= 0.2, "infantry decal shadow keeps paint grounded");
}

{
  const decal = {
    id: 78,
    kind: KIND.MACHINE_GUNNER,
    decalClass: "infantry",
    color: "#d55e00",
    seed: 246813579,
  };
  const plan = createGroundDecalStampPlan(decal);
  assert(plan.variantIndex === 4 + (decal.seed % 4),
    "machine-gunner deaths select only the second four normalized variants");
  assert(plan.color === 0xd55e00, "machine-gunner death sprites inherit their owner's team color");
}

{
  const decal = {
    id: 88,
    kind: KIND.TANK,
    decalClass: "scorch",
    color: "#c85050",
    facing: 1.25,
    seed: 987654321,
  };
  const plan = createGroundDecalStampPlan(decal);
  const repeat = createGroundDecalStampPlan({ ...decal });
  assertDeepEqual(plan, repeat, "vehicle decal selection is deterministic for a fixed seed");
  assert(plan.color === 0xc85050, "vehicle paint tint uses the recovered owner player color");
  assert(
    plan.variantIndex === (decal.seed % GROUND_DECAL_ASSET_MANIFEST.vehicleScorch.length),
    "vehicle scorch variant comes from the deterministic seed",
  );
  assertApprox(plan.rotation, decal.facing, 0.121, "vehicle scorch orientation stays anchored to recovered facing");
  assert(plan.paintVariantIndex >= 0, "vehicle decals include a deterministic player-color paint mask");
  assert(plan.flipX === 1, "vehicle decals keep the authored hull nose aligned with recovered facing");
  assert(plan.scorchOpacity > plan.paintOpacity, "vehicle decals stay blackened before team-color fragments");
  assert(plan.scorchOpacity >= 0.48 && plan.scorchOpacity <= 0.6, "vehicle scorch opacity is subdued");
  assert(plan.ashOpacity >= 0.06 && plan.ashOpacity <= 0.11, "vehicle inner ash stays neutral and subtle");
  assert(plan.paintOpacity >= 0.13 && plan.paintOpacity <= 0.2, "vehicle paint opacity stays subordinate to scorch");
}

{
  const mortar = {
    id: 99,
    kind: KIND.MORTAR_TEAM,
    decalClass: "mortarBlast",
    radiusWorld: 32,
    seed: 441122,
  };
  const artillery = {
    id: 100,
    kind: KIND.ARTILLERY,
    decalClass: "artilleryBlast",
    radiusWorld: 64,
    seed: 882244,
  };
  const mortarPlan = createGroundDecalStampPlan(mortar);
  const artilleryPlan = createGroundDecalStampPlan(artillery);
  assertDeepEqual(mortarPlan, createGroundDecalStampPlan({ ...mortar }), "mortar blast selection is deterministic");
  assertDeepEqual(artilleryPlan, createGroundDecalStampPlan({ ...artillery }), "artillery blast selection is deterministic");
  assert(
    mortarPlan.variantIndex === mortar.seed % GROUND_DECAL_ASSET_MANIFEST.mortarBlast.length,
    "mortar blast uses the authored mortar mask",
  );
  assert(
    artilleryPlan.variantIndex === artillery.seed % GROUND_DECAL_ASSET_MANIFEST.artilleryBlast.length,
    "artillery blast uses the authored artillery mask",
  );
  assert(mortarPlan.scale > 0.63 && mortarPlan.scale < 0.71, "mortar blast scales to its one-tile footprint");
  assert(
    artilleryPlan.scale > 0.63 && artilleryPlan.scale < 0.71,
    "artillery blast scales its unchanged three-tile authored mask to the new two-tile footprint",
  );
  assert(mortarPlan.charScale < 0.83, "mortar blast keeps its smaller air-burst center compact");
  assert(artilleryPlan.charScale > mortarPlan.charScale, "artillery retains the broader central crater treatment");
}
