import { gfxNoFill, gfxCircle, gfxRect, gfxLine, gfxMove, gfxFill, gfxStroke } from "./native_graphics.js";
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
import { buildingRigDefinitionFor } from "./rigs/building_routing.js";
import { isConstructionScaffold } from "./entity_state.js";
import { renderLiveUnitRig } from "./rigs/runtime.js";
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
  drawFreeRotatedRect,
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

export function _drawBuilding(e, colorByOwner, state) {
  const stat = STATS[e.kind] || {};
  const ts = (this._map && this._map.tileSize) || 32;
  const w = (stat.footW || 2) * ts;
  const h = (stat.footH || 2) * ts;
  const x0 = e.x - w / 2;
  const y0 = e.y - h / 2;

  const underConstruction = isConstructionScaffold(e);
  const bodyAlpha = underConstruction ? 0.45 : 1;
  const definition = e.kind === KIND.TANK_TRAP
    ? null
    : buildingRigDefinitionFor(this._buildingRigDefinitions, e.kind);

  // Shadow (slightly offset, under buildings).
  const shadowKey = `${e.x}|${e.y}|${w}|${h}`;
  const sh = this._staticSlot?.(
    "buildingShadows",
    e.id,
    shadowKey,
  ) || this._slot("buildingShadows", e.id);
  sh.position.set(0, 0);
  if (sh.rtsStaticRedraw !== false) {
    gfxFill(sh, COLORS.shadow, 0.3);
    gfxRect(sh, x0 + 4, y0 + 6, w, h);
    gfxNoFill(sh);
    sh.rtsStaticRenderKey = shadowKey;
  }

  const tint = e.kind !== KIND.TANK_TRAP && !definition
    ? this._tintFor(e.owner, colorByOwner)
    : null;
  const bodyKey = e.kind === KIND.TANK_TRAP
    ? `tankTrap|${e.x}|${e.y}|${ts}|${e.id}|${bodyAlpha}`
    : definition
      ? `rig|${e.kind}`
      : `fallback|${e.kind}|${e.x}|${e.y}|${w}|${h}|${bodyAlpha}|${tint}`;
  const g = this._staticSlot?.("buildings", e.id, bodyKey)
    || this._slot("buildings", e.id);
  g.position.set(0, 0);
  if (e.kind === KIND.TANK_TRAP) {
    if (g.rtsStaticRedraw !== false) {
      drawTankTrap(g, e.x, e.y, ts, e.id, bodyAlpha);
      g.rtsStaticRenderKey = bodyKey;
    }
  } else {
    // SVG rig body — look up the compiled definition and route it through the
    // buildingRigs pool into the buildings layer. Falls back to imperative
    // rect drawing if no definition is loaded (e.g. compile error on startup).
    if (definition) {
      renderLiveUnitRig(this, e, colorByOwner, state, definition, {
        routes: [{ poolName: "buildingRigs", layerName: "buildings" }],
        alpha: bodyAlpha,
      });
      if (g.rtsStaticRedraw !== false) g.rtsStaticRenderKey = bodyKey;
    } else if (g.rtsStaticRedraw !== false) {
      gfxStroke(g, 2, 0x1a1712, underConstruction ? 0.55 : 0.95);
      gfxFill(g, 0x2b2a23, bodyAlpha);
      gfxRect(g, x0, y0, w, h);
      gfxNoFill(g);

      // Player-tinted roof/yard slabs, all neutral geometry.
      gfxStroke(g, 0);
      gfxFill(g, tint, bodyAlpha * 0.82);
      if (e.kind === KIND.CITY_CENTRE) {
        gfxRect(g, x0 + w * 0.12, y0 + h * 0.18, w * 0.62, h * 0.52);
        gfxRect(g, x0 + w * 0.68, y0 + h * 0.1, w * 0.16, h * 0.32);
        gfxFill(g, 0x1a1712, bodyAlpha * 0.7);
        gfxRect(g, x0 + w * 0.76, y0 + h * 0.02, w * 0.08, h * 0.22);
      } else if (e.kind === KIND.FACTORY) {
        gfxRect(g, x0 + w * 0.12, y0 + h * 0.18, w * 0.76, h * 0.26);
        gfxRect(g, x0 + w * 0.18, y0 + h * 0.54, w * 0.64, h * 0.26);
        gfxFill(g, 0x1a1712, bodyAlpha * 0.55);
        for (let i = 0; i < 3; i++) gfxRect(g, x0 + w * (0.2 + i * 0.2), y0 + h * 0.56, w * 0.08, h * 0.22);
      } else if (e.kind === KIND.DEPOT) {
        gfxRect(g, x0 + w * 0.16, y0 + h * 0.22, w * 0.68, h * 0.2);
        gfxRect(g, x0 + w * 0.16, y0 + h * 0.52, w * 0.68, h * 0.2);
      } else {
        gfxRect(g, x0 + w * 0.12, y0 + h * 0.18, w * 0.76, h * 0.56);
        gfxFill(g, 0x1a1712, bodyAlpha * 0.42);
        gfxRect(g, x0 + w * 0.22, y0 + h * 0.26, w * 0.56, h * 0.12);
        gfxRect(g, x0 + w * 0.22, y0 + h * 0.5, w * 0.56, h * 0.12);
      }
      gfxNoFill(g);
      g.rtsStaticRenderKey = bodyKey;
    }

    // Stencil label — pooled Text reused per building id (see _icon).
    this._icon(e, e.x, e.y, Math.min(w, h) * 0.5, bodyAlpha);
  }

  const hasProductionProgress = typeof e.prodProgress === "number" && e.prodProgress > 0;
  const extractorInactive = e.kind === KIND.PUMP_JACK
    && e.extractorActive === false
    && !underConstruction;
  if (hasProductionProgress || extractorInactive) {
    const overlayKey = `${e.x}|${y0}|${w}|${e.prodProgress ?? ""}|${extractorInactive}`;
    const overlay = this._staticSlot?.(
      "buildingOverlays",
      e.id,
      overlayKey,
    ) || this._slot("buildingOverlays", e.id);
    overlay.position.set(0, 0);
    if (overlay.rtsStaticRedraw !== false) {
      if (hasProductionProgress) {
        // Unit production progress bar along the roof line.
        const bw = w * 0.78;
        const bx = e.x - bw / 2;
        const by = y0 + 6;
        gfxFill(overlay, COLORS.hpBack, 0.9);
        gfxRect(overlay, bx, by, bw, 5);
        gfxNoFill(overlay);
        gfxFill(overlay, COLORS.hpGood);
        gfxRect(overlay, bx, by, bw * clamp01(e.prodProgress), 5);
        gfxNoFill(overlay);
      }
      if (extractorInactive) {
        drawInactiveExtractorBadge(overlay, e.x, y0, ts);
      }
      overlay.rtsStaticRenderKey = overlayKey;
    }
  }

  // Queue depth label: show items waiting behind the active production slot.
  const queueDepth = (e.prodQueue ?? 0) - 1;
  this._queueLabel(e, e.x, y0 + 14, queueDepth, bodyAlpha);
}

function drawInactiveExtractorBadge(g, cx, buildingTop, tileSize) {
  const radius = Math.max(7, tileSize * 0.28);
  const cy = buildingTop - radius * 0.55;
  const diagonal = radius * 0.68;

  gfxStroke(g, 0);
  gfxFill(g, 0x211715, 0.92);
  gfxCircle(g, cx, cy, radius + 2.5);
  gfxNoFill(g);

  gfxStroke(g, Math.max(2.5, radius * 0.34), 0xe34b3f, 1);
  gfxCircle(g, cx, cy, radius);
  gfxMove(g, cx - diagonal, cy - diagonal);
  gfxLine(g, cx + diagonal, cy + diagonal);
  gfxStroke(g, 0);
}

function drawTankTrap(g, cx, cy, tileSize, id, bodyAlpha) {
  const base = tankTrapRotation(id);
  const visualScale = 1.5;
  const length = tileSize * 0.82 * visualScale;
  const beamW = tileSize * 0.16 * visualScale;
  const lipW = tileSize * 0.055 * visualScale;
  const beamAngles = [0, (Math.PI * 2) / 3, (Math.PI * 4) / 3];

  for (const a of beamAngles) {
    drawSteelBeam(g, cx, cy, base + a, length, beamW, lipW, bodyAlpha);
  }

  gfxStroke(g, 1.2, 0x1a1712, bodyAlpha * 0.75);
  gfxFill(g, 0x817b6f, bodyAlpha * 0.96);
  gfxCircle(g, cx, cy, tileSize * 0.105 * visualScale);
  gfxNoFill(g);
  gfxStroke(g, 0);
  gfxFill(g, 0xd7d2c1, bodyAlpha * 0.5);
  gfxCircle(g,
    cx - tileSize * 0.025 * visualScale,
    cy - tileSize * 0.025 * visualScale,
    tileSize * 0.035 * visualScale,
  );
  gfxNoFill(g);
}

function tankTrapRotation(id) {
  const n = Number.isFinite(id) ? id : 0;
  return ((n * 1.9634954084936207) % Math.PI) - Math.PI / 2;
}

function drawSteelBeam(g, cx, cy, angle, length, width, lipWidth, alpha) {
  gfxStroke(g, 1.6, 0x15130f, alpha * 0.95);
  gfxFill(g, 0x4d5150, alpha * 0.98);
  drawFreeRotatedRect(g, cx, cy, length, width, angle);
  gfxNoFill(g);

  const edgeOffset = width * 0.36;
  gfxStroke(g, 0);
  gfxFill(g, 0x222724, alpha * 0.68);
  drawFreeRotatedRect(
    g,
    cx + Math.cos(angle + Math.PI / 2) * edgeOffset,
    cy + Math.sin(angle + Math.PI / 2) * edgeOffset,
    length * 0.92,
    lipWidth,
    angle,
  );
  gfxNoFill(g);

  gfxFill(g, 0xa8aaa3, alpha * 0.68);
  drawFreeRotatedRect(
    g,
    cx - Math.cos(angle + Math.PI / 2) * edgeOffset,
    cy - Math.sin(angle + Math.PI / 2) * edgeOffset,
    length * 0.86,
    lipWidth,
    angle,
  );
  gfxNoFill(g);

  gfxFill(g, 0x343937, alpha * 0.9);
  drawFreeRotatedRect(g, cx, cy, width * 1.05, width * 1.05, angle + Math.PI / 4);
  gfxNoFill(g);
}
