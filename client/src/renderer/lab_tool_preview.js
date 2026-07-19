import { gfxNoFill, gfxCircle, gfxRoundRect, gfxLine, gfxMove, gfxFill, gfxStroke } from "./native_graphics.js";
import { COLORS, PLAYER_PALETTE, STATS } from "../config.js";
import { isBuilding, isUnit } from "../protocol.js";
import {
  finiteNumber,
  hexToInt,
} from "./shared.js";

/** Draw the ghost for a browser-local armed Lab setup tool. */
export function drawLabToolPreview(g, preview, tileSize = 32) {
  if (!finiteNumber(preview?.x) || !finiteNumber(preview?.y)) return;
  const payload = preview.payload || {};
  if (preview.kind === "spawnEntity") {
    drawLabSpawnPreview(g, preview.x, preview.y, payload, tileSize);
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
    gfxStroke(g, 2, color, 0.95);
    gfxFill(g, color, 0.28);
    gfxRoundRect(g, x0, y0, w, h, 5);
    gfxNoFill(g);
    gfxStroke(g, 1, color, 0.45);
    for (let tile = 1; tile < footW; tile++) {
      gfxMove(g, x0 + tile * tileSize, y0);
      gfxLine(g, x0 + tile * tileSize, y0 + h);
    }
    for (let tile = 1; tile < footH; tile++) {
      gfxMove(g, x0, y0 + tile * tileSize);
      gfxLine(g, x0 + w, y0 + tile * tileSize);
    }
    return;
  }
  if (!isUnit(kind)) return;
  const stat = STATS[kind] || {};
  const radius = Math.max(7, Math.min(tileSize * 0.34, Number(stat.size) || tileSize * 0.27));
  gfxStroke(g, 2, color, 0.95);
  gfxFill(g, color, 0.32);
  gfxCircle(g, x, y, radius);
  gfxNoFill(g);
  gfxStroke(g, 1.5, color, 0.82);
  gfxMove(g, x - radius * 0.7, y);
  gfxLine(g, x + radius * 0.7, y);
  gfxMove(g, x, y - radius * 0.7);
  gfxLine(g, x, y + radius * 0.7);
}


function drawLabRemovePreview(g, x, y, tileSize) {
  const arm = Math.max(14, tileSize * 0.48);
  const color = 0xe35c54;
  gfxStroke(g, 4, color, 0.95);
  gfxMove(g, x - arm, y - arm);
  gfxLine(g, x + arm, y + arm);
  gfxMove(g, x + arm, y - arm);
  gfxLine(g, x - arm, y + arm);
}

function labPreviewOwnerColor(owner) {
  return labPreviewPlayerColor(Math.max(0, Math.trunc(Number(owner) || 1) - 1));
}

function labPreviewPlayerColor(playerIndex) {
  const index = Math.max(0, Math.trunc(Number(playerIndex) || 0)) % PLAYER_PALETTE.length;
  return hexToInt(PLAYER_PALETTE[index]) || COLORS.placeOk;
}
