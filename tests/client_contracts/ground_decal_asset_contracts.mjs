// SVG-authored ground decal asset and deterministic selection contracts.

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
  "generated PNG atlas covers every SVG source in deterministic manifest order");
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

assert(GROUND_DECAL_ASSET_MANIFEST.infantry.length >= 12, "manifest includes at least twelve infantry decal masks");
assert(GROUND_DECAL_ASSET_MANIFEST.vehicleScorch.length >= 8, "manifest includes at least eight vehicle scorch masks");
assert(GROUND_DECAL_ASSET_MANIFEST.vehiclePaint.length >= 8, "manifest includes at least eight vehicle paint masks");
assert(GROUND_DECAL_ASSET_MANIFEST.mortarBlast.length >= 1, "manifest includes a mortar blast decal mask");
assert(GROUND_DECAL_ASSET_MANIFEST.artilleryBlast.length >= 1, "manifest includes an artillery blast decal mask");

for (const asset of allAssets) {
  assert(asset.url.startsWith("/assets/decals/"), `decal ${asset.id} is served from the client asset path`);
  assert(Number.isInteger(asset.width) && asset.width > 0, `decal ${asset.id} declares a positive width`);
  assert(Number.isInteger(asset.height) && asset.height > 0, `decal ${asset.id} declares a positive height`);

  const localPath = path.join(repoRoot, "client", asset.url.slice(1));
  assert(fs.existsSync(localPath), `decal ${asset.id} SVG exists at ${asset.url}`);
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
    plan.variantIndex === (decal.seed % GROUND_DECAL_ASSET_MANIFEST.infantry.length),
    "infantry decal variant comes from the deterministic seed",
  );
  assert(plan.scale > 0.8 && plan.scale < 1.17, "infantry decal scale stays within the authored variation range");
  assert(plan.opacity >= 0.54 && plan.opacity <= 0.7, "infantry decal tint opacity stays intentionally readable");
  assert(plan.shadowOpacity >= 0.14 && plan.shadowOpacity <= 0.2, "infantry decal shadow keeps paint grounded");
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
    radiusWorld: 48,
    seed: 441122,
  };
  const artillery = {
    id: 100,
    kind: KIND.ARTILLERY,
    decalClass: "artilleryBlast",
    radiusWorld: 96,
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
  assert(mortarPlan.scale > 0.94 && mortarPlan.scale < 1.06, "mortar blast preserves its 1.5-tile authored footprint");
  assert(artilleryPlan.scale > 0.94 && artilleryPlan.scale < 1.06, "artillery blast preserves its 3-tile authored footprint");
  assert(mortarPlan.charScale < 0.83, "mortar blast keeps its smaller air-burst center compact");
  assert(artilleryPlan.charScale > mortarPlan.charScale, "artillery retains the broader central crater treatment");
}
