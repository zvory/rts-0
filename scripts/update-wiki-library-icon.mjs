#!/usr/bin/env node
// Record the Technique Library's current base and team-paint frames as a wiki portrait.
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";
import { createBuildingPngRigAtlases } from "../client/src/renderer/rigs/building_png.js";

const root = fileURLToPath(new URL("../", import.meta.url));
const kind = "technique_library";
const atlas = createBuildingPngRigAtlases().get(kind);
if (!atlas?.enabled || !atlas.sprites.some((s) => s.id === "sprite.base") ||
    !atlas.sprites.some((s) => s.id === "sprite.tint")) {
  throw new Error("Technique Library must have its current base and team-paint PNG frames");
}
const asset = new URL(atlas.image, "http://localhost").pathname;
const bytes = await fs.readFile(path.join(root, "client", asset));
const browser = await puppeteer.launch({
  executablePath: process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  headless: true,
});
try {
  const page = await browser.newPage();
  const png = await page.evaluate(async ({ source, sprites }) => {
    const image = new Image();
    image.src = source;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 192;
    const context = canvas.getContext("2d");
    for (const sprite of sprites.sort((a, b) => a.drawOrder - b.drawOrder)) {
      const { x, y, w, h } = sprite.frame;
      const layer = document.createElement("canvas");
      layer.width = w;
      layer.height = h;
      const paint = layer.getContext("2d");
      paint.drawImage(image, x, y, w, h, 0, 0, w, h);
      if (sprite.tintSlot !== "fixed") {
        paint.globalCompositeOperation = "multiply";
        paint.fillStyle = "#0072b2";
        paint.fillRect(0, 0, w, h);
        paint.globalCompositeOperation = "destination-in";
        paint.drawImage(image, x, y, w, h, 0, 0, w, h);
      }
      const scale = Math.min(canvas.width / w, canvas.height / h);
      context.drawImage(layer, (canvas.width - w * scale) / 2,
        (canvas.height - h * scale) / 2, w * scale, h * scale);
    }
    return canvas.toDataURL("image/png");
  }, {
    source: `data:image/png;base64,${bytes.toString("base64")}`,
    sprites: atlas.sprites.filter((s) => ["sprite.base", "sprite.tint"].includes(s.id)),
  });
  const file = path.join(root, "docs/wiki/icons.json");
  const icons = JSON.parse(await fs.readFile(file, "utf8"));
  icons.buildings[kind] = png;
  await fs.writeFile(file, `${JSON.stringify(icons)}\n`);
  console.log("Updated wiki Technique Library icon from its live building PNG atlas");
} finally {
  await browser.close();
}
