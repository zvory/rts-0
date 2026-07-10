import { COLORS, PLAYER_PALETTE, STATS } from "../config.js";
import { isBuilding, isUnit } from "../protocol.js";
import {
  finiteNumber,
  hash2,
  hexToInt,
  terrainColor,
  terrainOverlayColor,
} from "./shared.js";

/** Draw the ghost for a browser-local armed Lab setup tool. */
export function drawLabToolPreview(g, preview, tileSize = 32) {
  if (!finiteNumber(preview?.x) || !finiteNumber(preview?.y)) return;
  const payload = preview.payload || {};
  if (preview.kind === "spawnEntity") {
    drawLabSpawnPreview(g, preview.x, preview.y, payload, tileSize);
    return;
  }
  if (preview.kind === "editMapTerrain") {
    drawLabTerrainPreview(g, preview.x, preview.y, payload, tileSize);
    return;
  }
  if (preview.kind === "editMapBase") {
    drawLabBasePreview(g, preview.x, preview.y, payload, tileSize);
    return;
  }
  if (preview.kind === "removeSelectableUnits") drawLabRemovePreview(g, preview.x, preview.y, tileSize);
}

function drawLabSpawnPreview(g, x, y, payload, tileSize) {
  const kind = payload?.kind;
  const color = labPreviewOwnerColor(payload?.owner);
  if (isBuilding(kind)) {
    const stat = STATS[kind] || {};
    const footW = Math.max(1, Number(stat.footW) || 2);
    const footH = Math.max(1, Number(stat.footH) || 2);
    const centerTileX = Math.floor(x / tileSize);
    const centerTileY = Math.floor(y / tileSize);
    const tileX = centerTileX - Math.floor(footW / 2);
    const tileY = centerTileY - Math.floor(footH / 2);
    const x0 = tileX * tileSize;
    const y0 = tileY * tileSize;
    const w = footW * tileSize;
    const h = footH * tileSize;
    g.lineStyle(2, color, 0.95);
    g.beginFill(color, 0.28);
    g.drawRoundedRect(x0, y0, w, h, 5);
    g.endFill();
    g.lineStyle(1, color, 0.45);
    for (let tile = 1; tile < footW; tile++) {
      g.moveTo(x0 + tile * tileSize, y0);
      g.lineTo(x0 + tile * tileSize, y0 + h);
    }
    for (let tile = 1; tile < footH; tile++) {
      g.moveTo(x0, y0 + tile * tileSize);
      g.lineTo(x0 + w, y0 + tile * tileSize);
    }
    return;
  }
  if (!isUnit(kind)) return;
  const stat = STATS[kind] || {};
  const radius = Math.max(7, Math.min(tileSize * 0.34, Number(stat.size) || tileSize * 0.27));
  g.lineStyle(2, color, 0.95);
  g.beginFill(color, 0.32);
  g.drawCircle(x, y, radius);
  g.endFill();
  g.lineStyle(1.5, color, 0.82);
  g.moveTo(x - radius * 0.7, y);
  g.lineTo(x + radius * 0.7, y);
  g.moveTo(x, y - radius * 0.7);
  g.lineTo(x, y + radius * 0.7);
}

function drawLabTerrainPreview(g, x, y, payload, tileSize) {
  const tileX = Math.floor(x / tileSize);
  const tileY = Math.floor(y / tileSize);
  const x0 = tileX * tileSize;
  const y0 = tileY * tileSize;
  const terrain = Number(payload?.terrain);
  const color = terrainColor(terrain, tileX, tileY);
  g.lineStyle(2, COLORS.placeOk, 0.95);
  g.beginFill(color, 0.64);
  g.drawRect(x0, y0, tileSize, tileSize);
  g.endFill();
  const block = tileSize / 4;
  for (let by = 0; by < 4; by++) {
    for (let bx = 0; bx < 4; bx++) {
      const noise = hash2(tileX * 17 + bx, tileY * 17 + by);
      if (noise < 0.42) continue;
      g.beginFill(terrainOverlayColor(terrain, noise), 0.32);
      g.drawRect(x0 + bx * block, y0 + by * block, Math.ceil(block), Math.ceil(block));
      g.endFill();
    }
  }
}

function drawLabBasePreview(g, x, y, payload, tileSize) {
  const radius = payload?.siteKind === "natural" ? tileSize * 0.34 : tileSize * 3;
  g.lineStyle(2, 0xe5c56c, 0.92);
  g.beginFill(0xe5c56c, 0.12);
  g.drawCircle(x, y, radius);
  g.endFill();
}

function drawLabRemovePreview(g, x, y, tileSize) {
  const arm = Math.max(14, tileSize * 0.48);
  const color = 0xe35c54;
  g.lineStyle(4, color, 0.95);
  g.moveTo(x - arm, y - arm);
  g.lineTo(x + arm, y + arm);
  g.moveTo(x + arm, y - arm);
  g.lineTo(x - arm, y + arm);
}

function labPreviewOwnerColor(owner) {
  const index = Math.max(0, Math.trunc(Number(owner) || 1) - 1) % PLAYER_PALETTE.length;
  return hexToInt(PLAYER_PALETTE[index]) || COLORS.placeOk;
}
