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
  drawImpassableEdge,
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
  isImpassableAt,
  isVehicleBodyKind,
  muzzleFlashRadius,
  normRect,
  polar,
  recoilVector,
  rectEdgePointTowardCenter,
  smoothstep01,
  tankBodyVisual,
  terrainColor,
  terrainOverlayColor,
  weaponRecoilOffset,
} from "./shared.js";

export function _drawBuilding(e, colorByOwner, state) {
  const stat = STATS[e.kind] || {};
  const ts = (this._map && this._map.tileSize) || 32;
  const w = (stat.footW || 2) * ts;
  const h = (stat.footH || 2) * ts;
  const tint = this._tintFor(e.owner, colorByOwner);
  const x0 = e.x - w / 2;
  const y0 = e.y - h / 2;

  const underConstruction = typeof e.buildProgress === "number" && e.buildProgress < 1;
  const bodyAlpha = underConstruction ? 0.45 : 1;

  // Shadow (slightly offset, under buildings).
  const sh = this._slot("buildingShadows", e.id);
  sh.position.set(0, 0);
  sh.beginFill(COLORS.shadow, 0.3);
  sh.drawRect(x0 + 4, y0 + 6, w, h);
  sh.endFill();

  const g = this._slot("buildings", e.id);
  g.position.set(0, 0);
  g.lineStyle(2, 0x1a1712, underConstruction ? 0.55 : 0.95);
  g.beginFill(0x2b2a23, bodyAlpha);
  g.drawRect(x0, y0, w, h);
  g.endFill();

  // Player-tinted roof/yard slabs, all neutral geometry.
  g.lineStyle(0);
  g.beginFill(tint, bodyAlpha * 0.82);
  if (e.kind === KIND.CITY_CENTRE) {
    g.drawRect(x0 + w * 0.12, y0 + h * 0.18, w * 0.62, h * 0.52);
    g.drawRect(x0 + w * 0.68, y0 + h * 0.1, w * 0.16, h * 0.32);
    g.beginFill(0x1a1712, bodyAlpha * 0.7);
    g.drawRect(x0 + w * 0.76, y0 + h * 0.02, w * 0.08, h * 0.22);
  } else if (e.kind === KIND.FACTORY) {
    g.drawRect(x0 + w * 0.12, y0 + h * 0.18, w * 0.76, h * 0.26);
    g.drawRect(x0 + w * 0.18, y0 + h * 0.54, w * 0.64, h * 0.26);
    g.beginFill(0x1a1712, bodyAlpha * 0.55);
    for (let i = 0; i < 3; i++) g.drawRect(x0 + w * (0.2 + i * 0.2), y0 + h * 0.56, w * 0.08, h * 0.22);
  } else if (e.kind === KIND.DEPOT) {
    g.drawRect(x0 + w * 0.16, y0 + h * 0.22, w * 0.68, h * 0.2);
    g.drawRect(x0 + w * 0.16, y0 + h * 0.52, w * 0.68, h * 0.2);
  } else {
    g.drawRect(x0 + w * 0.12, y0 + h * 0.18, w * 0.76, h * 0.56);
    g.beginFill(0x1a1712, bodyAlpha * 0.42);
    g.drawRect(x0 + w * 0.22, y0 + h * 0.26, w * 0.56, h * 0.12);
    g.drawRect(x0 + w * 0.22, y0 + h * 0.5, w * 0.56, h * 0.12);
  }
  g.endFill();

  // Stencil label — pooled Text reused per building id (see _icon).
  this._icon(e, e.x, e.y, Math.min(w, h) * 0.5, bodyAlpha);

  if (underConstruction) {
    // Construction progress bar across the footprint base.
    const bw = w * 0.8;
    const bx = e.x - bw / 2;
    const by = y0 + h - 6;
    g.beginFill(COLORS.hpBack, 0.85);
    g.drawRect(bx, by, bw, 4);
    g.endFill();
    g.beginFill(COLORS.hpGood);
    g.drawRect(bx, by, bw * clamp01(e.buildProgress), 4);
    g.endFill();
  } else if (typeof e.prodProgress === "number" && e.prodProgress > 0) {
    // Unit production progress bar along the roof line.
    const bw = w * 0.78;
    const bx = e.x - bw / 2;
    const by = y0 + 6;
    g.beginFill(COLORS.hpBack, 0.9);
    g.drawRect(bx, by, bw, 5);
    g.endFill();
    g.beginFill(COLORS.hpGood);
    g.drawRect(bx, by, bw * clamp01(e.prodProgress), 5);
    g.endFill();
  }

  // Queue depth label: show items waiting behind the active production slot.
  const queueDepth = (e.prodQueue ?? 0) - 1;
  this._queueLabel(e, e.x, y0 + 14, queueDepth, bodyAlpha);
}
