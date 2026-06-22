// SVG-authored ground decal asset and deterministic selection contracts.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { assert, assertApprox, assertDeepEqual } from "./assertions.mjs";
import { GROUND_DECAL_ASSET_MANIFEST } from "../../client/src/renderer/decals/manifest.js";
import { createGroundDecalStampPlan } from "../../client/src/renderer/decals/selection.js";
import { KIND } from "../../client/src/protocol.js";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const allAssets = [
  ...GROUND_DECAL_ASSET_MANIFEST.infantry,
  ...GROUND_DECAL_ASSET_MANIFEST.vehicleScorch,
  ...GROUND_DECAL_ASSET_MANIFEST.vehiclePaint,
];

assert(GROUND_DECAL_ASSET_MANIFEST.infantry.length >= 12, "manifest includes at least twelve infantry decal masks");
assert(GROUND_DECAL_ASSET_MANIFEST.vehicleScorch.length >= 8, "manifest includes at least eight vehicle scorch masks");
assert(GROUND_DECAL_ASSET_MANIFEST.vehiclePaint.length >= 8, "manifest includes at least eight vehicle paint masks");

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
}
