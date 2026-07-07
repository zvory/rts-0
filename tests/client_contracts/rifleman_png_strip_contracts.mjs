import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { assert } from "./assertions.mjs";
import { RIFLEMAN_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/rifleman_png_strip.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, "../..");
const manifestPath = path.join(repoRoot, "client/assets/rigs/rifleman-pass-02/metadata/manifest.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const runtime = manifest.runtime;
const strip = RIFLEMAN_PNG_FRAME_STRIP;

assert(runtime.module === "client/src/renderer/rigs/rifleman_png_strip.js", "Rifleman manifest points at the runtime strip module");
assert(strip.unit === manifest.unit, "Rifleman strip unit matches the manifest unit");
assert(strip.image === runtime.stripImageUrl, "Rifleman strip URL matches the manifest runtime URL");
assert(strip.image.includes(`?v=${strip.imageVersion}`), "Rifleman strip URL carries the image version cache key");
assert(strip.frameWidth === runtime.frameWidth, "Rifleman strip frame width matches the manifest");
assert(strip.frameHeight === runtime.frameHeight, "Rifleman strip frame height matches the manifest");
assert(strip.frameCount === runtime.frameCount, "Rifleman strip frame count matches the manifest");
assert(strip.idleFrame === runtime.idleFrame, "Rifleman idle frame matches the manifest");
assert(JSON.stringify(strip.movementFrames) === JSON.stringify(runtime.movementFrames), "Rifleman movement frames match the manifest");
assert(JSON.stringify(strip.firingFrames) === JSON.stringify(runtime.firingFrames), "Rifleman firing frames match the manifest");
assert(strip.firingFrameHoldPhase === runtime.firingFrameHoldPhase, "Rifleman firing hold phase matches the manifest");
assert(strip.fps === runtime.fps, "Rifleman strip FPS matches the manifest");
assert(strip.worldScale === runtime.worldScale, "Rifleman world scale matches the manifest");
assert(strip.tintSlot === runtime.tintSlot, "Rifleman tint slot matches the manifest");
assert(JSON.stringify(strip.bakedColorAdjustment) === JSON.stringify(runtime.bakedColorAdjustment), "Rifleman baked color adjustment matches the manifest");
assert(JSON.stringify(strip.source) === JSON.stringify({
  generatedSource: manifest.source.sourceSheet,
  alphaSource: manifest.source.alphaSheet,
  runtimeStrip: manifest.source.runtimeStrip,
}), "Rifleman strip source paths match the manifest");

const runtimeStripPath = repoPathFromClientAssetUrl(runtime.stripImageUrl);
assert(runtimeStripPath === manifest.source.runtimeStrip, "Rifleman runtime URL maps back to the checked-in strip");
const runtimeStripSize = readPngDimensions(runtimeStripPath);
assert(runtimeStripSize.width === runtime.frameWidth * runtime.frameCount, "Rifleman runtime strip width matches frame geometry");
assert(runtimeStripSize.height === runtime.frameHeight, "Rifleman runtime strip height matches frame geometry");

const sourceSheetSize = readPngDimensions(manifest.source.sourceSheet);
const alphaSheetSize = readPngDimensions(manifest.source.alphaSheet);
assert(JSON.stringify(alphaSheetSize) === JSON.stringify(sourceSheetSize), "Rifleman source and alpha sheets have matching dimensions");

function repoPathFromClientAssetUrl(assetUrl) {
  const assetPath = assetUrl.split("?")[0];
  assert(assetPath.startsWith("/assets/"), "asset URL is served from /assets");
  return `client${assetPath}`;
}

function readPngDimensions(repoRelativePath) {
  const buffer = fs.readFileSync(path.join(repoRoot, repoRelativePath));
  assert(buffer.toString("hex", 0, 8) === "89504e470d0a1a0a", `${repoRelativePath} is a PNG`);
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}
