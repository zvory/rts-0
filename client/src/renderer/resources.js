import { gfxNoFill, gfxRect, gfxStrokePaths, gfxFill, gfxStroke } from "./native_graphics.js";
import {
  COLORS,
  FOG_EXPLORED_ALPHA,
  FOG_UNEXPLORED_ALPHA,
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
    // Fuel drums: utilitarian but faction-neutral.
    // White outline improves contrast against dark ground and fog.
    gfxStroke(g, 2.5, 0xffffff, 0.95);
    gfxRect(g, -r * 0.78, -r * 0.58, r * 0.52, r * 1.09);
    gfxRect(g, -r * 0.21, -r * 0.71, r * 0.54, r * 1.23);
    gfxRect(g, r * 0.35, -r * 0.53, r * 0.46, r * 1.06);

    gfxStroke(g, 1.5, 0x1a1712, 0.85);
    gfxFill(g, COLORS.oil);
    gfxRect(g, -r * 0.75, -r * 0.55, r * 0.48, r * 1.05);
    gfxRect(g, -r * 0.18, -r * 0.68, r * 0.5, r * 1.18);
    gfxRect(g, r * 0.38, -r * 0.5, r * 0.42, r);
    gfxNoFill(g);
    gfxStroke(g, 0);
    gfxFill(g, 0x263225, 0.45);
    gfxRect(g, -r * 0.72, -r * 0.06, r * 1.48, r * 0.12);
    gfxRect(g, -r * 0.16, -r * 0.26, r * 0.46, r * 0.12);
    gfxNoFill(g);
  } else {
    // Supply crates: replaces sci-fi crystals with wartime materiel.
    gfxStroke(g, 1.2, 0x1a1712, 0.85);
    const crates = [
      { dx: -r * 0.45, dy: -r * 0.25, s: 0.65 },
      { dx: r * 0.25, dy: -r * 0.2, s: 0.7 },
      { dx: -r * 0.05, dy: r * 0.35, s: 0.8 },
    ];
    for (const c of crates) {
      const cs = r * c.s;
      gfxFill(g, COLORS.steel);
      gfxRect(g, c.dx - cs * 0.45, c.dy - cs * 0.35, cs * 0.9, cs * 0.7);
      gfxNoFill(g);
      gfxStrokePaths(g, [
        [[c.dx - cs * 0.38, c.dy], [c.dx + cs * 0.38, c.dy]],
        [[c.dx, c.dy - cs * 0.3], [c.dx, c.dy + cs * 0.3]],
      ], 1, 0x5a5134, 0.8);
      gfxStroke(g, 1.2, 0x1a1712, 0.85);
    }
  }

  // X marker over a node that a worker is actively mining.
  if (mined) {
    const xr = r * 0.45;
    const xColor = e.kind === KIND.OIL ? 0xffffff : 0x1a1712;
    gfxStrokePaths(g, [
      [[-xr, -xr], [xr, xr]],
      [[xr, -xr], [-xr, xr]],
    ], 2.5, xColor, 0.95);
  }
  g.rtsStaticRenderKey = renderKey;
}
