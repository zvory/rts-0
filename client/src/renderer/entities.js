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
  this._recordRenderDiagnostic?.(`renderer.graphics.clear.${poolName}`);
  g.clear();
  return g;
}

export function _staticSlot(poolName, id, renderKey) {
  const pool = this._pools[poolName];
  const keys = this._graphicsRenderKeys?.[poolName];
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

  const redraw = !keys || keys.get(id) !== renderKey;
  if (redraw) {
    keys?.delete(id);
    this._recordRenderDiagnostic?.(`renderer.cache.miss.${poolName}`);
    this._recordRenderDiagnostic?.(`renderer.graphics.clear.${poolName}`);
    g.clear();
  } else {
    this._recordRenderDiagnostic?.(`renderer.cache.hit.${poolName}`);
  }
  return {
    g,
    redraw,
    commit: redraw ? () => keys?.set(id, renderKey) : () => {},
  };
}

export function _shadow(g, cx, cy, radius) {
  g.beginFill(COLORS.shadow, 0.28);
  g.drawEllipse(cx, cy + radius * 0.35, radius, radius * 0.6);
  g.endFill();
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
  g.beginFill(COLORS.shadow, 0.28);
  g.drawPolygon(points);
  g.endFill();
}

export function _drawSelectionAndHp(e, selection, state) {
  const selected = selection.has(e.id);
  const damaged = e.maxHp && e.hp < e.maxHp;
  const progressStatus = buildingProgressStatus(e);

  if (selected) {
    const g = this._slot("selectionRings", e.id);
    g.position.set(e.x, e.y);
    const ring = this._ringRadius(e);
    let color;
    if (ownOwner(state, e.owner)) color = COLORS.selectOwn;
    else if (allyOwner(state, e.owner)) color = COLORS.selectAlly;
    else if (neutralOwner(state, e.owner)) color = COLORS.selectNeutral;
    else color = COLORS.selectEnemy;
    // Glow + crisp ring.
    g.lineStyle(4, color, 0.25);
    g.drawEllipse(0, ring.cy, ring.rx, ring.ry);
    g.lineStyle(2, color, 0.95);
    g.drawEllipse(0, ring.cy, ring.rx, ring.ry);
  }

  if (progressStatus || damaged || selected) {
    const g = this._slot("hpBars", e.id);
    g.position.set(0, 0);
    this._hpBar(g, e, progressStatus);
  }
}

function ownOwner(state, owner) {
  if (typeof state?.isFeedbackOwner === "function") return state.isFeedbackOwner(owner);
  if (state?.controlPolicy?.kind === "lab") {
    if (typeof state.controlPolicy.isFeedbackOwner === "function") {
      return state.controlPolicy.isFeedbackOwner(owner, state);
    }
    if (typeof state.controlPolicy.canControlOwner === "function") {
      return state.controlPolicy.canControlOwner(owner, state);
    }
  }
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
    return { rx: body.halfLen + 4, ry: body.halfWidth + 5, cy: 2 };
  }
  const r = (stat.size || 9) + 4;
  return { rx: r, ry: r * 0.7, cy: r * 0.35 };
}

export function _hpBar(g, e, status = null) {
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
  const x0 = e.x - halfW;
  const barW = halfW * 2;
  const barH = 4;

  g.beginFill(COLORS.hpBack, 0.9);
  g.drawRect(x0 - 1, topY - 1, barW + 2, barH + 2);
  g.endFill();

  let color = COLORS.hpGood;
  if (status?.kind === "deconstruction") {
    color = COLORS.hpMid;
  } else if (status?.kind !== "construction") {
    if (frac <= 0.33) color = COLORS.hpLow;
    else if (frac <= 0.66) color = COLORS.hpMid;
  }
  g.beginFill(color);
  g.drawRect(x0, topY, barW * frac, barH);
  g.endFill();
}

export function _icon(e, cx, cy, size, alpha) {
  if (!this._iconPool) this._iconPool = new Map();
  let t = this._iconPool.get(e.id);
  const glyph = (STATS[e.kind] && STATS[e.kind].icon) || "?";
  if (!t) {
    t = new PIXI.Text(glyph, {
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
      fontSize: 24,
      fill: 0xd8d0b0,
      align: "center",
      fontWeight: "700",
    });
    t.anchor.set(0.5);
    this._iconPool.set(e.id, t);
    this.layers.buildings.addChild(t);
    this._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.iconText");
  } else {
    this._recordRenderDiagnostic?.("renderer.pixi.displayObject.reused.iconText");
  }
  if (t.text !== glyph) t.text = glyph;
  t.visible = true;
  t.alpha = 0.78 * alpha;
  t.position.set(cx, cy);
  // Scale the (fixed-size) glyph to roughly fit the footprint.
  const s = (size * 0.95) / 24;
  t.scale.set(s);
  // Track on the buildings pool's seen-set so the sweep keeps it alive.
  this._seen.buildings.add(e.id);
}

export function _queueLabel(e, cx, cy, count, bodyAlpha) {
  if (!this._queueLabelPool) this._queueLabelPool = new Map();
  let t = this._queueLabelPool.get(e.id);
  if (!t) {
    t = new PIXI.Text("", {
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
      fontSize: 11,
      fill: 0xffe080,
      align: "center",
      fontWeight: "700",
      stroke: 0x000000,
      strokeThickness: 3,
    });
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
