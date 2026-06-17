import { sampleRigAnimation } from "./animation.js";
import { hexToInt, lightenColor } from "../shared.js";

export function createDefaultPixiFactory(pixi = globalThis.PIXI) {
  return {
    createContainer: () => new pixi.Container(),
    createGraphics: () => new pixi.Graphics(),
  };
}

export function createUnitRigInstance(kind, definition, pixiFactory = createDefaultPixiFactory()) {
  return new UnitRigInstance(kind, definition, pixiFactory);
}

export function renderRigLegacyComparison(renderer, entity, colorByOwner, state, definition) {
  const legacyPools = { shadow: "unitShadows", unit: "units", skipRigComparison: true };
  renderer._drawUnit(entity, colorByOwner, state, legacyPools);
  if (!definition) return null;

  const poolName = "rigComparisons";
  const instance = renderer._rigComparisonPool.get(entity.id)
    ?? createUnitRigInstance(entity.kind, definition, renderer._rigPixiFactory ?? createDefaultPixiFactory());
  renderer._rigComparisonPool.set(entity.id, instance);
  renderer._seen[poolName].add(entity.id);
  if (!instance.container.parent && renderer.layers[poolName]) renderer.layers[poolName].addChild(instance.container);
  const context = renderer._rigRenderContextFor?.(entity, colorByOwner, state) ?? {};
  instance.update(entity, context);
  instance.container.x += 48;
  return instance;
}

export class UnitRigInstance {
  constructor(kind, definition, pixiFactory) {
    this.kind = kind;
    this.definition = definition;
    this._pixiFactory = pixiFactory;
    this.container = pixiFactory.createContainer();
    this.parts = new Map();
    this._destroyed = false;

    for (const part of definition.parts || []) {
      const display = pixiFactory.createGraphics();
      display.rtsRigPartId = part.id;
      this.parts.set(part.id, { definition: part, display });
      this.container.addChild(display);
    }
  }

  update(entity, renderContext = {}, options = {}) {
    if (this._destroyed) return;
    this.container.visible = true;
    this.container.alpha = renderContext.shotRevealAlpha ?? 1;
    setPoint(this.container.position, entity.x ?? 0, entity.y ?? 0);
    setPoint(this.container.scale, 1, 1);
    this.container.rotation = 0;

    const includeParts = normalizedPartSet(options.includeParts);
    const sampled = sampleRigAnimation(this.definition, entity, renderContext);
    for (const [partId, rec] of this.parts) {
      const partState = sampled.parts[partId];
      if (!partState || (includeParts && !includeParts.has(partId))) {
        rec.display.visible = false;
        continue;
      }
      applyPartState(rec.display, rec.definition, partState, sampled.context);
    }
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    for (const { display } of this.parts.values()) {
      display.destroy?.();
    }
    this.parts.clear();
    this.container.destroy?.({ children: true });
  }
}

function applyPartState(display, part, state, context) {
  display.visible = state.visible;
  display.alpha = state.alpha;
  setPoint(display.position, state.transform.x, state.transform.y);
  setPoint(display.pivot, state.pivot.x, state.pivot.y);
  setPoint(display.scale, state.transform.scaleX, state.transform.scaleY);
  display.rotation = state.transform.rotation;
  redrawPart(display, part.geometry, part.paint, tintForSlot(state.tintSlot, context));
}

function redrawPart(g, geometry, paint, tint) {
  g.clear?.();
  const fill = paint.fill == null ? null : tint ?? hexToInt(paint.fill);
  const stroke = paint.stroke == null ? null : hexToInt(paint.stroke);
  if (stroke !== null) g.lineStyle?.(paint.strokeWidth ?? 1, stroke, paint.strokeOpacity ?? 1);
  if (fill !== null) g.beginFill?.(fill, paint.fillOpacity ?? 1);
  drawGeometry(g, geometry);
  if (fill !== null) g.endFill?.();
}

function drawGeometry(g, geometry) {
  if (geometry.type === "rect") g.drawRect(geometry.x, geometry.y, geometry.width, geometry.height);
  else if (geometry.type === "circle") g.drawCircle(geometry.cx, geometry.cy, geometry.r);
  else if (geometry.type === "ellipse") g.drawEllipse(geometry.cx, geometry.cy, geometry.rx, geometry.ry);
  else if (geometry.type === "line") {
    g.moveTo(geometry.from.x, geometry.from.y);
    g.lineTo(geometry.to.x, geometry.to.y);
  } else if (geometry.type === "polygon" || geometry.type === "polyline") {
    const points = geometry.points.flatMap((point) => [point.x, point.y]);
    if (geometry.type === "polygon") g.drawPolygon(points);
    else drawPolyline(g, points);
  } else if (geometry.type === "path") {
    drawPath(g, geometry.commands);
  }
}

function drawPolyline(g, points) {
  if (points.length < 4) return;
  g.moveTo(points[0], points[1]);
  for (let i = 2; i < points.length; i += 2) g.lineTo(points[i], points[i + 1]);
}

function drawPath(g, commands) {
  for (const command of commands) {
    const v = command.values;
    if (command.command === "M") g.moveTo(v[0], v[1]);
    else if (command.command === "L") g.lineTo(v[0], v[1]);
    else if (command.command === "C") g.bezierCurveTo?.(v[0], v[1], v[2], v[3], v[4], v[5]);
    else if (command.command === "Q") g.quadraticCurveTo?.(v[0], v[1], v[2], v[3]);
    else if (command.command === "Z") g.closePath?.();
  }
}

function tintForSlot(slot, context) {
  if (slot === "team") return hexToInt(context.teamColor);
  if (slot === "team-light") return lightenColor(hexToInt(context.teamColor), 0.12);
  if (slot === "neutral") return 0x9aa0a8;
  return null;
}

function setPoint(point, x, y) {
  if (point?.set) point.set(x, y);
  else if (point) {
    point.x = x;
    point.y = y;
  }
}

function normalizedPartSet(includeParts) {
  if (includeParts == null) return null;
  if (typeof includeParts === "string") return new Set([includeParts]);
  if (Array.isArray(includeParts)) return new Set(includeParts);
  return new Set([...includeParts]);
}
