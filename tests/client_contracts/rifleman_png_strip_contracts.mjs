import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { assert } from "./assertions.mjs";
import { KIND } from "../../client/src/protocol.js";
import { createLiveFrameStrips, liveFrameStripFor } from "../../client/src/renderer/rigs/frame_strip_routing.js";
import { LOADED_RIFLEMAN_RIG_KEY } from "../../client/src/renderer/rigs/live_routing.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/rifleman_png_strip.js";
import { RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/rifleman_panzerfaust_png_strip.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, "../..");
const manifestPath = path.join(repoRoot, "client/assets/rigs/rifleman-no-pack-panzerfaust-pass-01/metadata/manifest.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const runtime = manifest.runtime;

assert(runtime.normalModule === "client/src/renderer/rigs/rifleman_png_strip.js", "Rifleman manifest points at the normal runtime strip module");
assert(runtime.loadedModule === "client/src/renderer/rigs/rifleman_panzerfaust_png_strip.js", "Rifleman manifest points at the loaded runtime strip module");

for (const [label, strip, variant] of [
  ["normal Rifleman", RIFLEMAN_PNG_FRAME_STRIP, runtime.normal],
  ["loaded Panzerfaust unit", RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP, runtime.panzerfaustLoaded],
]) {
  assert(strip.unit === manifest.unit, `${label} strip unit matches the manifest unit`);
  assert(strip.image === variant.stripImageUrl, `${label} strip URL matches the manifest runtime URL`);
  assert(strip.imageVersion === variant.imageVersion, `${label} strip image version matches the manifest`);
  assert(assetUrlVersion(strip.image) === strip.imageVersion, `${label} strip URL carries its image version cache key`);
  assert(strip.frameWidth === runtime.frameWidth, `${label} frame width matches the manifest`);
  assert(strip.frameHeight === runtime.frameHeight, `${label} frame height matches the manifest`);
  assert(strip.frameCount === runtime.frameCount, `${label} frame count matches the manifest`);
  assert(strip.idleFrame === runtime.idleFrame, `${label} idle frame matches the manifest`);
  assert(JSON.stringify(strip.movementFrames) === JSON.stringify(runtime.movementFrames), `${label} movement frames match the manifest`);
  assert(JSON.stringify(strip.firingFrames) === JSON.stringify(runtime.firingFrames), `${label} firing frames match the manifest`);
  assert(JSON.stringify(strip.firingWeaponKinds) === JSON.stringify(runtime.firingWeaponKinds), `${label} firing weapon routing matches the manifest`);
  assert(strip.firingFrameHoldPhase === runtime.firingFrameHoldPhase, `${label} firing-frame timing matches the manifest`);
  assert(strip.fps === runtime.fps, `${label} FPS matches the manifest`);
  assert(strip.worldScale === runtime.worldScale, `${label} world scale matches the manifest`);
  assert(strip.originForwardPx === runtime.originForwardPx, `${label} torso origin offset matches the manifest`);
  assert(strip.firingRecoilPx === runtime.firingRecoilPx, `${label} firing recoil offset matches the manifest`);
  assert(strip.tintSlot === runtime.tintSlot, `${label} tint slot matches the manifest`);
  assert(JSON.stringify(strip.bakedColorAdjustment) === JSON.stringify(runtime.bakedColorAdjustment), `${label} baked color adjustment matches the manifest`);
  assert(JSON.stringify(strip.targetColorAdjustment) === JSON.stringify(runtime.targetColorAdjustment), `${label} darker runtime color target matches the manifest`);
  assert(strip.source.runtimeStrip === variant.runtimeStrip, `${label} source metadata points at its checked-in runtime strip`);
  assert(repoPathFromClientAssetUrl(strip.image) === variant.runtimeStrip, `${label} runtime URL maps back to the checked-in strip`);
  const runtimeStripSize = readPngDimensions(variant.runtimeStrip);
  assert(runtimeStripSize.width === runtime.frameWidth * runtime.frameCount, `${label} runtime strip width matches frame geometry`);
  assert(runtimeStripSize.height === runtime.frameHeight, `${label} runtime strip height matches frame geometry`);
}

const liveStrips = createLiveFrameStrips();
assert(liveFrameStripFor(liveStrips, KIND.RIFLEMAN) === RIFLEMAN_PNG_FRAME_STRIP,
  "ordinary and spent Riflemen use the new no-pack live strip");
assert(liveFrameStripFor(liveStrips, LOADED_RIFLEMAN_RIG_KEY) === RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP,
  "loaded Panzerfaust units use the composited Panzerfaust live strip");

function repoPathFromClientAssetUrl(assetUrl) {
  const assetPath = assetUrl.split("?")[0];
  assert(assetPath.startsWith("/assets/"), "asset URL is served from /assets");
  return `client${assetPath}`;
}

function assetUrlVersion(assetUrl) {
  return new URL(assetUrl, "http://rts.local").searchParams.get("v");
}

function readPngDimensions(repoRelativePath) {
  const buffer = fs.readFileSync(path.join(repoRoot, repoRelativePath));
  assert(buffer.toString("hex", 0, 8) === "89504e470d0a1a0a", `${repoRelativePath} is a PNG`);
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}
