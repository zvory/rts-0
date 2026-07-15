import { sampleRigAnimation } from "./animation.js";
import { hexToInt, lightenColor } from "../shared.js";
import { normalizedPartSet, partSelectionKey } from "./part_selection.js";

const OCCUPIED_TRENCH_UNIT_SCALE = 0.85;

export function createDefaultPixiFactory(pixi = globalThis.PIXI) {
  return {
    createContainer: () => new pixi.Container(),
    createGraphics: () => new pixi.Graphics(),
  };
}

export function createUnitRigInstance(kind, definition, pixiFactory = createDefaultPixiFactory(), options = {}) {
  return new UnitRigInstance(kind, definition, pixiFactory, options);
}

export function renderLiveUnitRig(renderer, entity, colorByOwner, state, definition, options = {}) {
  if (!definition) return null;
  const context = options.renderContext ?? renderer._rigRenderContextFor?.(entity, colorByOwner, state) ?? {};
  if (typeof options.alpha === "number") context.shotRevealAlpha = options.alpha;
  const rendered = [];
  for (const route of options.routes || []) {
    const pool = renderer._liveRigPools?.[route.poolName];
    if (!pool) continue;
    let instance = pool.get(entity.id);
    if (instance && (typeof instance.matches !== "function" || !instance.matches(entity.kind, definition, route.parts))) {
      instance.destroy();
      pool.delete(entity.id);
      instance = null;
      renderer._recordRenderDiagnostic?.(`renderer.rig.instance.rebuilt.${route.poolName}`);
    }
    if (!instance) {
      instance = createUnitRigInstance(
        entity.kind,
        definition,
        renderer._rigPixiFactory ?? createDefaultPixiFactory(),
        { includeParts: route.parts },
      );
      renderer._recordRenderDiagnostic?.(`renderer.rig.instance.created.${route.poolName}`);
      renderer._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.liveRigContainer");
      renderer._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.liveRigPart", instance.parts?.size || 0);
    } else {
      renderer._recordRenderDiagnostic?.(`renderer.rig.instance.reused.${route.poolName}`);
    }
    pool.set(entity.id, instance);
    renderer._seen[route.poolName]?.add(entity.id);
    const layer = renderer.layers[route.layerName];
    if (!instance.container.parent && layer) layer.addChild(instance.container);
    instance.update(entity, context, {
      sampledAnimation: options.sampledAnimation,
      diagnostics: (label, amount = 1) => renderer._recordRenderDiagnostic?.(label, amount),
    });
    rendered.push(instance);
  }
  return rendered;
}

export class UnitRigInstance {
  constructor(kind, definition, pixiFactory, options = {}) {
    this.kind = kind;
    this.definition = definition;
    this._pixiFactory = pixiFactory;
    this.container = pixiFactory.createContainer();
    this.parts = new Map();
    const routeParts = normalizedPartSet(options.includeParts);
    this._routeParts = routeParts ? new Set(routeParts) : null;
    this._routePartKey = partSelectionKey(this._routeParts);
    this._destroyed = false;

    for (const part of definition.parts || []) {
      if (this._routeParts && !this._routeParts.has(part.id)) continue;
      const display = pixiFactory.createGraphics();
      display.rtsRigPartId = part.id;
      this.parts.set(part.id, { definition: part, display });
      this.container.addChild(display);
    }
  }

  matches(kind, definition, includeParts = null) {
    return (
      this.kind === kind &&
      this.definition === definition &&
      this._routePartKey === partSelectionKey(includeParts)
    );
  }

  update(entity, renderContext = {}, options = {}) {
    if (this._destroyed) return;
    this.container.visible = true;
    this.container.alpha = renderContext.shotRevealAlpha ?? 1;
    setPoint(this.container.position, entity.x ?? 0, entity.y ?? 0);
    const scale = renderContext.occupiedTrench ? OCCUPIED_TRENCH_UNIT_SCALE : 1;
    setPoint(this.container.scale, scale, scale);
    this.container.rotation = 0;

    const includeParts = this._routeParts ?? normalizedPartSet(options.includeParts);
    const sampled = options.sampledAnimation ?? sampleRigAnimation(
      this.definition,
      entity,
      renderContext,
      { includeParts },
    );
    for (const [partId, rec] of this.parts) {
      const partState = sampled.parts[partId];
      if (!partState || (includeParts && !includeParts.has(partId))) {
        rec.display.visible = false;
        options.diagnostics?.("renderer.rig.redraw.skipped.hidden");
        continue;
      }
      applyPartState(rec.display, rec.definition, partState, sampled.context, options.diagnostics);
    }
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this.container.parent?.removeChild?.(this.container);
    for (const { display } of this.parts.values()) {
      display.destroy?.();
    }
    this.parts.clear();
    this.container.destroy?.({ children: true });
  }
}

function applyPartState(display, part, state, context, diagnostics = null) {
  display.visible = state.visible;
  if (!state.visible) {
    diagnostics?.("renderer.rig.redraw.skipped.hidden");
    return;
  }

  applyDisplayTransform(display, displayTransform(state));
  const tint = tintForSlot(state.tintSlot, context);
  const drawKey = partDrawKey(state, tint);
  diagnostics?.("renderer.rig.redraw.attempted");
  if (display.rtsRigDrawKey === drawKey) {
    diagnostics?.("renderer.rig.redraw.skipped.unchanged");
    return;
  }

  display.clear?.();
  diagnostics?.("renderer.graphics.clear.liveRigPart");
  drawPart(display, part.geometry, part.paint, tint, state.geometryScale);
  display.rtsRigDrawKey = drawKey;
  diagnostics?.("renderer.rig.redraw.completed");
}

function displayTransform(state) {
  const localOffset = rotateOffset(state.localOffset, state.transform.rotation);
  return {
    x: state.transform.x + localOffset.x,
    y: state.transform.y + localOffset.y,
    pivotX: state.pivot.x,
    pivotY: state.pivot.y,
    scaleX: state.transform.scaleX,
    scaleY: state.transform.scaleY,
    rotation: state.transform.rotation,
    alpha: state.alpha,
  };
}

function applyDisplayTransform(display, transform) {
  const last = display.rtsRigTransform;
  if (!last || !nearly(last.alpha, transform.alpha)) display.alpha = transform.alpha;
  if (!last || !nearly(last.x, transform.x) || !nearly(last.y, transform.y)) {
    setPoint(display.position, transform.x, transform.y);
  }
  if (!last || !nearly(last.pivotX, transform.pivotX) || !nearly(last.pivotY, transform.pivotY)) {
    setPoint(display.pivot, transform.pivotX, transform.pivotY);
  }
  if (!last || !nearly(last.scaleX, transform.scaleX) || !nearly(last.scaleY, transform.scaleY)) {
    setPoint(display.scale, transform.scaleX, transform.scaleY);
  }
  if (!last || !nearly(last.rotation, transform.rotation)) display.rotation = transform.rotation;
  display.rtsRigTransform = transform;
}

function nearly(a, b) {
  return Math.abs(a - b) <= 1e-9;
}

function rotateOffset(offset, rotation) {
  if (!offset || (offset.x === 0 && offset.y === 0)) return { x: 0, y: 0 };
  const cos = Math.cos(rotation);
  const sin = Math.sin(rotation);
  return {
    x: offset.x * cos - offset.y * sin,
    y: offset.x * sin + offset.y * cos,
  };
}

function drawPart(g, geometry, paint, tint, geometryScale = null) {
  const fill = paint.fill == null ? null : tint?.fill ?? hexToInt(paint.fill);
  const stroke = paint.stroke == null ? null : tint?.stroke ?? hexToInt(paint.stroke);
  if (stroke !== null) g.lineStyle?.(paint.strokeWidth ?? 1, stroke, paint.strokeOpacity ?? 1);
  else g.lineStyle?.(0, 0, 0);
  if (fill !== null) g.beginFill?.(fill, paint.fillOpacity ?? 1);
  drawGeometry(g, geometry, geometryScale);
  if (fill !== null) g.endFill?.();
}

function partDrawKey(state, tint) {
  const geometryScale = state.geometryScale || {};
  return [
    tint?.fill ?? "",
    tint?.stroke ?? "",
    geometryScale.x ?? 1,
    geometryScale.y ?? 1,
  ].join("|");
}

function drawGeometry(g, geometry, geometryScale = null) {
  const sx = geometryScale?.x ?? 1;
  const sy = geometryScale?.y ?? 1;
  if (geometry.type === "rect") drawRectAsPolygon(g, geometry, sx, sy);
  else if (geometry.type === "circle") {
    if (nearly(sx, sy)) g.drawCircle(geometry.cx * sx, geometry.cy * sy, geometry.r * sx);
    else g.drawEllipse(geometry.cx * sx, geometry.cy * sy, Math.abs(geometry.r * sx), Math.abs(geometry.r * sy));
  } else if (geometry.type === "ellipse") g.drawEllipse(geometry.cx * sx, geometry.cy * sy, Math.abs(geometry.rx * sx), Math.abs(geometry.ry * sy));
  else if (geometry.type === "line") {
    g.moveTo(geometry.from.x * sx, geometry.from.y * sy);
    g.lineTo(geometry.to.x * sx, geometry.to.y * sy);
  } else if (geometry.type === "polygon" || geometry.type === "polyline") {
    const points = geometry.points.flatMap((point) => [point.x * sx, point.y * sy]);
    if (geometry.type === "polygon") g.drawPolygon(points);
    else drawPolyline(g, points);
  } else if (geometry.type === "path") {
    drawPath(g, geometry.commands, sx, sy);
  }
}

function drawRectAsPolygon(g, geometry, sx = 1, sy = 1) {
  g.drawPolygon([
    geometry.x * sx, geometry.y * sy,
    (geometry.x + geometry.width) * sx, geometry.y * sy,
    (geometry.x + geometry.width) * sx, (geometry.y + geometry.height) * sy,
    geometry.x * sx, (geometry.y + geometry.height) * sy,
  ]);
}

function drawPolyline(g, points) {
  if (points.length < 4) return;
  g.moveTo(points[0], points[1]);
  for (let i = 2; i < points.length; i += 2) g.lineTo(points[i], points[i + 1]);
}

function drawPath(g, commands, sx = 1, sy = 1) {
  for (const command of commands) {
    const v = scalePathValues(command.values, sx, sy);
    if (command.command === "M") g.moveTo(v[0], v[1]);
    else if (command.command === "L") g.lineTo(v[0], v[1]);
    else if (command.command === "C") g.bezierCurveTo?.(v[0], v[1], v[2], v[3], v[4], v[5]);
    else if (command.command === "Q") g.quadraticCurveTo?.(v[0], v[1], v[2], v[3]);
    else if (command.command === "Z") g.closePath?.();
  }
}

function scalePathValues(values, sx, sy) {
  if (sx === 1 && sy === 1) return values;
  return values.map((value, index) => value * (index % 2 === 0 ? sx : sy));
}

function tintForSlot(slot, context) {
  if (slot === "team") return { fill: hexToInt(context.teamColor) };
  if (slot === "team-light") return { fill: lightenColor(hexToInt(context.teamColor), 0.12) };
  if (slot === "team-light-soft") return { fill: lightenColor(hexToInt(context.teamColor), 0.06) };
  if (slot === "team-light-strong") return { fill: lightenColor(hexToInt(context.teamColor), 0.16) };
  if (slot === "team-light-08") return { fill: lightenColor(hexToInt(context.teamColor), 0.08) };
  if (slot === "team-light-10") return { fill: lightenColor(hexToInt(context.teamColor), 0.10) };
  if (slot === "team-light-14") return { fill: lightenColor(hexToInt(context.teamColor), 0.14) };
  if (slot === "team-light-24") return { fill: lightenColor(hexToInt(context.teamColor), 0.24) };
  if (slot === "team-stroke") return { stroke: hexToInt(context.teamColor) };
  if (slot === "team-fill-stroke") {
    const team = hexToInt(context.teamColor);
    return { fill: team, stroke: team };
  }
  if (slot === "neutral") return { fill: 0x9aa0a8 };
  return null;
}

function setPoint(point, x, y) {
  if (point?.set) point.set(x, y);
  else if (point) {
    point.x = x;
    point.y = y;
  }
}
