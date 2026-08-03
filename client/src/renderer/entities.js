import { gfxNoFill, gfxEllipse, gfxPoly, gfxRect, gfxReset, gfxFill, gfxStroke } from "./native_graphics.js";
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
import { buildingProgressStatus } from "./entity_state.js";
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

export function _ownerColors(state) {
  const out = new Map();
  const players = state.players || [];
  for (let i = 0; i < players.length; i++) {
    const p = players[i];
    out.set(p.id, hexToInt(p.color || PLAYER_PALETTE[i % PLAYER_PALETTE.length]));
  }
  return out;
}

export function _tintFor(owner, colorByOwner) {
  if (owner === 0) return 0x9aa0a8;
  return colorByOwner.get(owner) ?? 0x9aa0a8;
}

export function _slot(poolName, id) {
  const pool = this._pools[poolName];
  let g = pool.get(id);
  if (!g) {
    g = new PIXI.Graphics();
    pool.set(id, g);
    this.layers[poolName].addChild(g);
    this._recordRenderDiagnostic?.(`renderer.pixi.displayObject.created.${poolName}`);
  } else {
    this._recordRenderDiagnostic?.(`renderer.pixi.displayObject.reused.${poolName}`);
  }
  this._seen[poolName].add(id);
  g.visible = true;
  g.alpha = 1;
  g.rtsStaticRedraw = true;
  delete g.rtsStaticRenderKey;
  this._recordRenderDiagnostic?.(`renderer.graphics.clear.${poolName}`);
  gfxReset(g.clear());
  return g;
}

export function _staticSlot(poolName, id, renderKey) {
  const pool = this._pools[poolName];
  let g = pool.get(id);
  if (!g) {
    g = new PIXI.Graphics();
    pool.set(id, g);
    this.layers[poolName].addChild(g);
    this._recordRenderDiagnostic?.(`renderer.pixi.displayObject.created.${poolName}`);
  }
  this._seen[poolName].add(id);
  g.visible = true;
  g.alpha = 1;

  g.rtsStaticRedraw = g.rtsStaticRenderKey !== renderKey;
  if (g.rtsStaticRedraw) {
    delete g.rtsStaticRenderKey;
    this._recordRenderDiagnostic?.(`renderer.cache.miss.${poolName}`);
    this._recordRenderDiagnostic?.(`renderer.graphics.clear.${poolName}`);
    gfxReset(g.clear());
  } else {
    this._recordRenderDiagnostic?.(`renderer.cache.hit.${poolName}`);
  }
  return g;
}

export function _shadow(g, cx, cy, radius) {
  gfxFill(g, COLORS.shadow, 0.28);
  gfxEllipse(g, cx, cy + radius * 0.35, radius, radius * 0.6);
  gfxNoFill(g);
}

export function _vehicleShadow(g, cx, cy, body, facing) {
  const rx = body.halfLen + body.clearance + 3;
  const ry = body.halfWidth + body.clearance + 3;
  const drop = ry * 0.35;
  const c = Math.cos(facing);
  const s = Math.sin(facing);
  const points = [];
  for (let i = 0; i < 24; i += 1) {
    const a = (Math.PI * 2 * i) / 24;
    const x = Math.cos(a) * rx;
    const y = Math.sin(a) * ry;
    points.push(cx + x * c - y * s, cy + drop + x * s + y * c);
  }
  gfxFill(g, COLORS.shadow, 0.28);
  gfxPoly(g, points);
  gfxNoFill(g);
}

export function _drawSelectionAndHp(e, selection, state, ownerColor = null) {
  const selected = selection.has(e.id);
  const hasHealth = Number.isFinite(e.maxHp) && e.maxHp > 0;
  const damaged = hasHealth && Number.isFinite(e.hp) && e.hp < e.maxHp;
  const alwaysShow = !!state?.showHealthBarsAlwaysEnabled;
  const progressStatus = buildingProgressStatus(e);

  if (selected) {
    const g = this._slot("selectionRings", e.id);
    const ring = this._ringRadius(e);
    // Keep the ground-projection offset screen-aligned while rotating the
    // directional footprint itself around its center.
    g.position.set(e.x, e.y + ring.cy);
    // Directional footprints track the body beneath them. Infantry keeps its
    // ground-projected oval screen-aligned so its facing does not make the
    // marker wobble around a nearly circular silhouette.
    g.rotation = usesVehicleSelectionBody(e.kind) && Number.isFinite(e.facing)
      ? e.facing
      : 0;
    let color;
    if (ownOwner(state, e.owner)) color = COLORS.selectOwn;
    else if (allyOwner(state, e.owner)) color = COLORS.selectAlly;
    else if (neutralOwner(state, e.owner)) color = COLORS.selectNeutral;
    else color = COLORS.selectEnemy;
    // Subtle halo + crisp ring. The layer sits below units, so the selected
    // silhouette stays readable while the colored outline remains distinct.
    gfxStroke(g, 4, color, 0.16);
    gfxEllipse(g, 0, 0, ring.rx, ring.ry);
    gfxStroke(g, 1.5, color, 0.78);
    gfxEllipse(g, 0, 0, ring.rx, ring.ry);
  }

  if (progressStatus || (hasHealth && (damaged || selected || alwaysShow))) {
    const g = this._hpBarSlot(e.id);
    this._hpBar(g, e, progressStatus, ownerColor);
  }
}

export function _drawAboveFogHp(e, state, ownerColor = null) {
  const hasHealth = Number.isFinite(e?.maxHp) && e.maxHp > 0;
  const damaged = Number.isFinite(e?.hp)
    && hasHealth
    && e.hp < e.maxHp;
  if (e?.aboveFogReveal !== true && e?.visionOnly !== true) return;
  if (!hasHealth) return;
  if (!damaged && !state?.showHealthBarsAlwaysEnabled) return;
  const g = this._hpBarSlot(e.id, "aboveFogHpBars");
  this._hpBar(g, e, null, ownerColor);
}

export function _hpBarSlot(id, poolName = "hpBars") {
  const pool = this._pools[poolName];
  let container = pool.get(id);
  if (!container) {
    container = new PIXI.Container();
    container.rtsBackground = new PIXI.Graphics();
    container.rtsFill = new PIXI.Graphics();
    container.rtsTicks = new PIXI.Graphics();
    container.addChild(container.rtsBackground, container.rtsFill, container.rtsTicks);
    pool.set(id, container);
    this.layers[poolName].addChild(container);
    this._recordRenderDiagnostic?.(`renderer.pixi.displayObject.created.${poolName}`);
  } else {
    this._recordRenderDiagnostic?.(`renderer.pixi.displayObject.reused.${poolName}`);
  }
  this._seen[poolName].add(id);
  container.visible = true;
  container.alpha = 1;
  return container;
}

function ownOwner(state, owner) {
  if (typeof state?.isFeedbackOwner === "function") return state.isFeedbackOwner(owner);
  return typeof state?.isOwnOwner === "function"
    ? state.isOwnOwner(owner)
    : Number(owner) === state?.playerId;
}

function allyOwner(state, owner) {
  return typeof state?.isAllyOwner === "function" && state.isAllyOwner(owner);
}

function neutralOwner(state, owner) {
  return typeof state?.isNeutralOwner === "function"
    ? state.isNeutralOwner(owner)
    : Number(owner) === 0;
}

function usesVehicleSelectionBody(kind) {
  return kind === KIND.SCOUT_PLANE || isVehicleBodyKind(kind);
}

export function _ringRadius(e) {
  const stat = STATS[e.kind] || {};
  if (isBuilding(e.kind)) {
    const ts = (this._map && this._map.tileSize) || 32;
    const w = (stat.footW || 2) * ts;
    const h = (stat.footH || 2) * ts;
    return { rx: w * 0.6, ry: h * 0.42, cy: 0 };
  }
  if (usesVehicleSelectionBody(e.kind)) {
    const body = tankBodyVisual(stat);
    return { rx: body.halfLen + 7, ry: body.halfWidth + 7, cy: 2 };
  }
  const r = (stat.size || 9) + 6;
  return { rx: r, ry: r * 0.68, cy: r * 0.36 };
}

export function _hpBar(g, e, status = null, ownerColor = null) {
  if (!e.maxHp && !status) return;
  const frac = clamp01(status ? status.fraction : e.hp / e.maxHp);
  const stat = STATS[e.kind] || {};
  let halfW;
  let topY;
  if (isBuilding(e.kind)) {
    const ts = (this._map && this._map.tileSize) || 32;
    const w = (stat.footW || 2) * ts;
    const h = (stat.footH || 2) * ts;
    halfW = Math.min(w * 0.45, 28);
    topY = e.y - h / 2 - 8;
  } else {
    if (usesVehicleSelectionBody(e.kind)) {
      const body = tankBodyVisual(stat);
      halfW = body.halfLen * 0.8;
      topY = e.y - body.shadowRadius - 8;
    } else {
      const r = stat.size || 9;
      halfW = Math.max(10, r);
      topY = e.y - r - 8;
    }
  }
  const barW = halfW * 2;
  const barH = 4;
  const maxHp = Number.isFinite(e.maxHp) && e.maxHp > 0 ? e.maxHp : 0;
  const segmentCount = Math.max(1, Math.round(maxHp / 15));
  const dividerCount = segmentCount - 1;
  const geometryKey = `${halfW}|${barH}|${segmentCount}`;
  if (g.rtsGeometryKey !== geometryKey) {
    g.rtsGeometryKey = geometryKey;
    g.rtsBackground.clear().rect(-halfW - 1, -1, barW + 2, barH + 2).fill({ color: COLORS.hpBack, alpha: 0.9 });
    g.rtsFill.clear().rect(0, 0, barW, barH).fill(0xffffff);
    g.rtsFill.position.x = -halfW;
    g.rtsTicks.clear();
    for (let i = 1; i <= dividerCount; i++) {
      const x = -halfW + barW * (i / segmentCount);
      g.rtsTicks.rect(x - 0.375, 0, 0.75, barH);
    }
    if (dividerCount > 0) g.rtsTicks.fill({ color: 0x000000, alpha: 0.95 });
  }
  g.position.set(e.x, topY);

  let color = Number.isFinite(ownerColor)
    ? ownerColor
    : hexToInt(typeof e.teamColor === "string" ? e.teamColor : "#9aa0a8");
  if (status?.kind === "deconstruction") {
    color = COLORS.hpMid;
  } else if (status?.kind === "construction") {
    color = COLORS.hpGood;
  }
  g.rtsFill.tint = color;
  g.rtsFill.scale.set(frac, 1);
}

export function _queueLabel(e, cx, cy, count, bodyAlpha) {
  if (!this._queueLabelPool) this._queueLabelPool = new Map();
  let t = this._queueLabelPool.get(e.id);
  if (!t) {
    t = new PIXI.Text({ text: "", style: {
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
      fontSize: 11,
      fill: 0xffe080,
      align: "center",
      fontWeight: "700",
      stroke: { color: 0x000000, width: 3 },
    } });
    t.anchor.set(0.5, 0);
    this._queueLabelPool.set(e.id, t);
    this.layers.buildings.addChild(t);
    this._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.queueText");
  } else {
    this._recordRenderDiagnostic?.("renderer.pixi.displayObject.reused.queueText");
  }
  if (count > 0) {
    const label = `+${count}`;
    if (t.text !== label) t.text = label;
    t.visible = true;
    t.alpha = bodyAlpha;
    t.position.set(cx, cy);
  } else {
    t.visible = false;
  }
  this._seen.buildings.add(e.id);
}
