#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { PNG } from "pngjs";
import { applyColorAdjustmentToRgba } from "../client/src/renderer/rigs/color_adjustment.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
const chrome = process.env.RTS_CHROME_PATH
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const assets = [{
  source: "client/assets/rigs/scout-car-pass-02-team/generated/scout-car-pass-02-team-atlas.png",
  output: "client/assets/rigs/scout-car-pass-02-team/generated/scout-car-pass-02-team-atlas-adjusted.png",
  adjustment: { brightness: 90, saturation: 90, hue: 100 },
}];

if (!fs.existsSync(chrome)) throw new Error(`Chrome executable not found at ${chrome}`);
const imported = await import("puppeteer-core");
const puppeteer = imported.default || imported;
const browser = await puppeteer.launch({
  executablePath: chrome,
  headless: "new",
  args: ["--no-sandbox"],
});
try {
  const page = await browser.newPage();
  for (const asset of assets) {
    const sourcePath = path.join(root, asset.source);
    const outputPath = path.join(root, asset.output);
    const sourceData = fs.readFileSync(sourcePath).toString("base64");
    const dataUrl = await page.evaluate(async (pngBase64) => {
      const image = new Image();
      image.src = `data:image/png;base64,${pngBase64}`;
      await image.decode();
      const canvas = document.createElement("canvas");
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      context.imageSmoothingEnabled = false;
      context.drawImage(image, 0, 0);
      return canvas.toDataURL("image/png");
    }, sourceData);
    const png = PNG.sync.read(Buffer.from(dataUrl.slice(dataUrl.indexOf(",") + 1), "base64"));
    applyColorAdjustmentToRgba(png.data, asset.adjustment);
    const generated = PNG.sync.write(png);
    if (check) {
      if (!fs.existsSync(outputPath) || !fs.readFileSync(outputPath).equals(generated)) {
        throw new Error(`${asset.output} is stale; run scripts/generate-color-adjusted-rig-assets.mjs`);
      }
    } else {
      fs.writeFileSync(outputPath, generated);
      console.log(`wrote ${asset.output} (${png.width}x${png.height})`);
    }
  }
  await page.close();
} finally {
  await browser.close();
}
