import { gfxNoFill, gfxCircle, gfxRect, gfxStrokePaths, gfxFill, gfxStroke } from "./native_graphics.js";
import {
  COLORS,
  STATS,
  PLAYER_PALETTE,
  RESOURCE_AMOUNTS,
  ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  isProducerBuilding,
} from "../config.js";
import { KIND, SETUP, STATE, isBuilding, isResource } from "../protocol.js";
import {
  DEPLOYED_WEAPON_ANIM_MS,
  SWEEP_EVICT_FRAMES,
  WEAPON_RECOIL_PX,
  ZERO_OFFSET,
} from "./palette.js";
import {
  angleDelta,
  clamp01,
  dashedLine,
  drawAntiTankGun,
  drawFacingWedge,
  drawInfantryBase,
  drawInfantryMachineGun,
  drawInfantryRifle,
  drawRotatedRect,
  drawScoutCar,
  drawTankFuelCue,
  drawTankHull,
  drawTankTracks,
  finiteNumber,
  hexToInt,
  isVehicleBodyKind,
  muzzleFlashRadius,
  normRect,
  polar,
  recoilVector,
  rectEdgePointTowardCenter,
  smoothstep01,
  tankBodyVisual,
  weaponRecoilOffset,
} from "./shared.js";
import {
  drawImpassableEdge,
  isImpassableAt,
  terrainColor,
  terrainOverlayColor,
} from "./terrain_palette.js";

export function _drawResource(e, fog) {
  const stat = STATS[e.kind] || {};
  const base = stat.size || 11;
  // Scale a little with remaining amount (clamped) so depleted nodes shrink.
  const full = RESOURCE_AMOUNTS[e.kind] || 1;
  const frac = e.remaining == null ? 1 : clamp01(e.remaining / full);
  const r = base * (0.55 + 0.45 * frac);

  const ts = (this._map && this._map.tileSize) || 32;
  const visible = !fog || fog.isVisible(Math.floor(e.x / ts), Math.floor(e.y / ts));
  const alpha = visible ? 1 : 0.7;

  const mined = !!(this._miningNodes && this._miningNodes.has(e.id));
  const remainingKey = Number.isFinite(e.remaining) ? e.remaining : "full";
  const renderKey = `${e.kind}|${remainingKey}|${mined ? 1 : 0}`;
  const g = this._staticSlot?.(
    "resources",
    e.id,
    renderKey,
  ) || this._slot("resources", e.id);
  g.position.set(e.x, e.y);
  g.alpha = alpha;
  if (g.rtsStaticRedraw === false) return;

  if (e.kind === KIND.OIL) {
    drawOilSpring(g, r);
  } else {
    // Steel stockpile: three staggered bars with top/side faces so the pile reads
    // as overlapping metal instead of flat crates.
    drawSteelBarStack(g, r);
  }

  if (mined) {
    if (e.kind === KIND.OIL) {
      const xr = r * 0.45;
      gfxStrokePaths(g, [
        [[-xr, -xr], [xr, xr]],
        [[xr, -xr], [-xr, xr]],
      ], 2.5, 0xffffff, 0.95);
    } else {
      drawSteelMiningCracks(g, r);
    }
  }
  g.rtsStaticRenderKey = renderKey;
}

export function drawResourceNodePreview(graphics, kind, radius = 11) {
  graphics.clear();
  if (kind === KIND.OIL || kind === "oil") drawOilSpring(graphics, radius);
  else drawSteelBarStack(graphics, radius);
}

function drawOilSpring(g, r) {
  // Original drum fill area was roughly 1.514r². Scale the spring to 125%,
  // landing near 2.37r² so the gusher reads clearly at gameplay zoom.
  r *= 1.25;
  gfxStroke(g, 0);
  gfxFill(g, 0xffffff, 0.9);
  gfxCircle(g, -r * 0.32, r * 0.33, r * 0.42);
  gfxCircle(g, r * 0.18, r * 0.35, r * 0.46);
  gfxCircle(g, r * 0.5, r * 0.28, r * 0.26);
  gfxRect(g, -r * 0.55, r * 0.12, r * 1.16, r * 0.36);

  gfxFill(g, 0x0f1512, 0.98);
  gfxCircle(g, -r * 0.32, r * 0.33, r * 0.38);
  gfxCircle(g, r * 0.18, r * 0.35, r * 0.42);
  gfxCircle(g, r * 0.5, r * 0.28, r * 0.22);
  gfxRect(g, -r * 0.54, r * 0.12, r * 1.14, r * 0.34);

  gfxFill(g, COLORS.oil, 1);
  gfxRect(g, -r * 0.15, -r * 0.58, r * 0.3, r * 0.95);
  gfxCircle(g, 0, -r * 0.58, r * 0.15);
  gfxCircle(g, -r * 0.09, -r * 0.22, r * 0.14);
  gfxCircle(g, r * 0.11, r * 0.03, r * 0.13);

  gfxFill(g, 0x263225, 0.72);
  gfxRect(g, r * 0.04, -r * 0.49, r * 0.08, r * 0.76);
  gfxCircle(g, r * 0.08, -r * 0.54, r * 0.06);
  gfxRect(g, -r * 0.44, r * 0.26, r * 0.78, r * 0.1);

  gfxFill(g, COLORS.oil, 0.96);
  gfxCircle(g, -r * 0.42, -r * 0.45, r * 0.12);
  gfxCircle(g, r * 0.36, -r * 0.36, r * 0.1);
  gfxCircle(g, -r * 0.26, -r * 0.73, r * 0.09);
  gfxCircle(g, r * 0.22, -r * 0.78, r * 0.08);
  gfxCircle(g, r * 0.55, -r * 0.08, r * 0.075);

  gfxFill(g, 0x0a0d0b, 0.58);
  gfxCircle(g, -r * 0.08, r * 0.38, r * 0.18);
  gfxCircle(g, r * 0.29, r * 0.31, r * 0.16);
  gfxNoFill(g);
}

function drawSteelBarStack(g, r) {
  // Original crate fill area was 0.63 * (0.65² + 0.70² + 0.80²) = 0.978075r².
  // These front faces total 0.9886r², keeping the resource footprint in the same visual weight.
  drawSteelCastShadow(g, -r * 0.18, -r * 0.16, r * 1.2, r * 0.26, r * 0.08, 0.38);
  drawSteelCastShadow(g, r * 0.13, r * 0.09, r * 1.22, r * 0.27, r * 0.09, 0.44);
  drawSteelCastShadow(g, -r * 0.05, r * 0.34, r * 1.24, r * 0.28, r * 0.1, 0.5);
  drawSteelBar(g, -r * 0.18, -r * 0.16, r * 1.2, r * 0.26, r * 0.12, 0.96);
  drawSteelBar(g, r * 0.13, r * 0.09, r * 1.22, r * 0.27, r * 0.12, 1);
  drawSteelBar(g, -r * 0.05, r * 0.34, r * 1.24, r * 0.28, r * 0.13, 1);

  gfxStrokePaths(g, [
    [[-r * 0.72, -r * 0.17], [r * 0.32, -r * 0.17]],
    [[-r * 0.41, r * 0.08], [r * 0.65, r * 0.08]],
    [[-r * 0.62, r * 0.33], [r * 0.48, r * 0.33]],
  ], Math.max(1, r * 0.055), 0xd8d0b0, 0.58);
}

function drawSteelBar(g, cx, cy, width, height, depth, alpha) {
  const x = cx - width / 2;
  const y = cy - height / 2;
  const top = Math.max(1, height * 0.28);
  const side = Math.max(1, Math.min(depth, width * 0.12));
  const bottom = Math.max(1, height * 0.18);
  const seam = Math.max(0.75, height * 0.08);

  gfxStroke(g, 0);
  gfxFill(g, COLORS.steel, alpha);
  gfxRect(g, x, y, width, height);

  gfxFill(g, 0x8f876d, alpha * 0.94);
  gfxRect(g, x, y, width, top);

  gfxFill(g, 0x3f3d35, alpha * 0.9);
  gfxRect(g, x + width - side, y + top, side, height - top);

  gfxFill(g, 0x565144, alpha * 0.86);
  gfxRect(g, x, y + height - bottom, width, bottom);

  gfxFill(g, 0x24231f, alpha * 0.56);
  gfxRect(g, x, y, width, seam);
  gfxRect(g, x, y + height - seam, width, seam);
  gfxRect(g, x + width - seam, y, seam, height);

  gfxNoFill(g);
}

function drawSteelCastShadow(g, cx, cy, width, height, offset, alpha) {
  const x = cx - width / 2;
  const y = cy - height / 2;
  gfxStroke(g, 0);
  gfxFill(g, 0x0b0a08, alpha);
  gfxRect(g, x + offset, y + height, width * 0.9, Math.max(1, height * 0.22));
  gfxRect(g, x + width, y + offset, Math.max(1, offset * 0.75), height * 0.74);
  gfxNoFill(g);
}

function drawSteelMiningCracks(g, r) {
  const crackShadow = 0x050504;
  const crackHighlight = 0xb8ae8f;
  const cracks = [
    [[-r * 0.66, -r * 0.18], [-r * 0.54, -r * 0.1], [-r * 0.43, -r * 0.16]],
    [[-r * 0.2, -r * 0.2], [-r * 0.08, -r * 0.12], [r * 0.08, -r * 0.16]],
    [[-r * 0.28, r * 0.07], [-r * 0.13, r * 0.15], [r * 0.03, r * 0.1]],
    [[r * 0.23, r * 0.08], [r * 0.38, r * 0.16], [r * 0.55, r * 0.09]],
    [[-r * 0.48, r * 0.32], [-r * 0.32, r * 0.41], [-r * 0.14, r * 0.34]],
    [[r * 0.05, r * 0.31], [r * 0.22, r * 0.4], [r * 0.42, r * 0.33]],
  ];
  const highlights = cracks.map((path) => path.map(([x, y]) => [x - r * 0.016, y - r * 0.014]));

  gfxStrokePaths(g, cracks, Math.max(1.5, r * 0.085), crackShadow, 0.9);
  gfxStrokePaths(g, highlights, Math.max(0.75, r * 0.035), crackHighlight, 0.52);
}
