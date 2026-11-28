#!/usr/bin/env node
// Export the current client portrait, with its PNG rig, into the wiki's recorded icon data.
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";
import { liveUnitIconMarkupFor } from "../client/src/renderer/rigs/unit_icon_sources.js";

const root = fileURLToPath(new URL("../", import.meta.url));
let markup = liveUnitIconMarkupFor("rocket_launcher").replace("<svg ", '<svg xmlns="http://www.w3.org/2000/svg" ');
if (!markup.includes('data-unit-icon-source="png-atlas-composition"')) {
  throw new Error("Rocket Truck portrait must use its current PNG rig");
}
for (const asset of new Set(markup.match(/\/assets\/[^"<> ]+/g) || [])) {
  const bytes = await fs.readFile(path.join(root, "client", asset));
  markup = markup.replaceAll(asset, `data:image/png;base64,${bytes.toString("base64")}`);
}
const browser = await puppeteer.launch({
  executablePath: process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  headless: true,
});
try {
  const page = await browser.newPage();
  const png = await page.evaluate(async (svg) => {
    const image = new Image();
    image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 192;
    canvas.getContext("2d").drawImage(image, 0, 0, 192, 192);
    return canvas.toDataURL("image/png");
  }, markup);
  const file = path.join(root, "docs/wiki/icons.json");
  const icons = JSON.parse(await fs.readFile(file, "utf8"));
  icons.units.rocket_launcher = png;
  await fs.writeFile(file, `${JSON.stringify(icons)}\n`);
  console.log("Updated wiki Rocket Truck icon from the live client PNG portrait");
} finally {
  await browser.close();
}
